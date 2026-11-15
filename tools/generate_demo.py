#!/usr/bin/env python3
"""Generate the bundled demo song, click SFX and demo chart.

The chart and the lead melody are generated from the same event list, so the
notes in the chart always land on audible lead notes.

Usage (from the repository root):
    python3 tools/generate_demo.py

Requires ffmpeg (libvorbis) on PATH.  Everything is deterministic.
"""

from __future__ import annotations

import json
import math
import random
import struct
import subprocess
import tempfile
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
AUDIO_DIR = ROOT / "assets" / "audio"
CHART_DIR = ROOT / "charts"

BPM = 120.0
BEATS = 64
SAMPLE_RATE = 44_100
SECONDS = BEATS * 60.0 / BPM
TAIL = 1.5

rng = random.Random(20261114)

# ---------------------------------------------------------------- composition

# Chord progression: one chord per bar (4 beats), 16 bars.
PROGRESSION = [
    ("Am", 57, "minor"),
    ("F", 53, "major"),
    ("C", 60, "major"),
    ("G", 55, "major"),
]

MINOR_DEGREES = [0, 3, 7, 10, 14, 17]
MAJOR_DEGREES = [0, 4, 7, 12, 16, 19]

# (step inside a 16th-note bar, lane)
INTRO = [(0, 0), (4, 1), (8, 2), (12, 3)]
DRIVE_A = [(0, 0), (2, 2), (4, 1), (6, 3), (8, 2), (10, 4), (12, 3), (14, 5)]
DRIVE_B = [(0, 5), (2, 3), (4, 4), (6, 2), (8, 3), (10, 1), (12, 2), (14, 0)]
SYNC = [(0, 0), (3, 3), (4, 1), (7, 4), (8, 2), (11, 5), (12, 3), (15, 1)]
BURST = [(i, [0, 2, 1, 3, 2, 4, 3, 5, 4, 2, 3, 1, 2, 4, 5, 3][i]) for i in range(16)]
FINALE = [(0, 0), (0, 3), (2, 2), (4, 1), (4, 4), (6, 3), (8, 2), (8, 5), (10, 4), (12, 3), (14, 5)]

BAR_PATTERNS = [
    None,
    INTRO,
    INTRO,
    DRIVE_A,
    DRIVE_B,
    DRIVE_A,
    SYNC,
    DRIVE_B,
    DRIVE_A,
    SYNC,
    DRIVE_B,
    BURST,
    SYNC,
    BURST,
    DRIVE_A,
    FINALE,
]

# Extra chord hits (same beat, two lanes) to make the chart feel bigger.
DOUBLE_BARS = {0, 3, 8, 12, 14}


def midi_hz(note: int) -> float:
    return 440.0 * 2.0 ** ((note - 69) / 12.0)


def events_for_bar(bar: int):
    pattern = BAR_PATTERNS[bar]
    if pattern is None:
        return []
    events = list(pattern)
    if bar in DOUBLE_BARS:
        # add a mirrored lane on the first / middle hit of the bar
        extra = []
        for step, lane in pattern[:2]:
            doubled = (lane + 3) % 6
            if all(not (s == step and l == doubled) for s, l in events):
                extra.append((step, doubled))
        if bar != 12:
            events.extend(extra)
    return sorted(events)


CHART_EVENTS = []
for bar in range(16):
    for step, lane in events_for_bar(bar):
        beat = bar * 4.0 + step / 4.0
        CHART_EVENTS.append((beat, lane))

# ------------------------------------------------------------------- synthesis

NUM_FRAMES = int((SECONDS + TAIL) * SAMPLE_RATE)
left = [0.0] * NUM_FRAMES
right = [0.0] * NUM_FRAMES


def add_sample(buf, index: int, value: float):
    if 0 <= index < NUM_FRAMES:
        buf[index] += value


def add_stereo(start: float, samples, pan: float = 0.5, gain: float = 1.0):
    """Mix a mono sample list into the stereo buffers with equal-power pan."""
    base = int(start * SAMPLE_RATE)
    pan = max(0.0, min(1.0, pan))
    l_gain = math.sqrt(1.0 - pan) * gain
    r_gain = math.sqrt(pan) * gain
    for i, s in enumerate(samples):
        j = base + i
        if 0 <= j < NUM_FRAMES:
            left[j] += s * l_gain
            right[j] += s * r_gain


def env_ad(n: int, attack: float, decay: float) -> list[float]:
    out = []
    a = max(1, int(attack * SAMPLE_RATE))
    for i in range(n):
        if i < a:
            e = i / a
        else:
            e = math.exp(-(i - a) / (decay * SAMPLE_RATE))
        out.append(e)
    return out


def synth_lead(freq: float, duration: float, bright: float = 1.0):
    n = int(duration * SAMPLE_RATE)
    env = env_ad(n, 0.004, 0.20)
    out = []
    for i in range(n):
        t = i / SAMPLE_RATE
        phase = freq * t
        # Detuned square + sine sub gives a chiptune / synthwave character.
        sq = 1.0 if math.sin(2 * math.pi * phase) >= 0 else -1.0
        sq2 = 1.0 if math.sin(2 * math.pi * phase * 1.005) >= 0 else -1.0
        sub = math.sin(2 * math.pi * phase * 0.5)
        saw = 2.0 * ((phase * 1.002) % 1.0) - 1.0
        v = 0.42 * (sq + sq2) + 0.30 * sub + 0.12 * saw * bright
        out.append(v * env[i] * 0.22)
    return out


def synth_pad(freqs, duration: float):
    n = int(duration * SAMPLE_RATE)
    out = []
    for i in range(n):
        t = i / SAMPLE_RATE
        attack = min(1.0, t / 0.35)
        release = min(1.0, max(0.0, (duration - t) / 0.45))
        e = attack * release
        v = 0.0
        for k, f in enumerate(freqs):
            detune = 1.0 + (k - 1) * 0.0015
            v += math.sin(2 * math.pi * f * detune * t) * (0.10 if k == 0 else 0.055)
        out.append(v * e)
    return out


def synth_bass(freq: float, duration: float):
    n = int(duration * SAMPLE_RATE)
    env = env_ad(n, 0.008, 0.28)
    out = []
    for i in range(n):
        t = i / SAMPLE_RATE
        phase = freq * t
        sq = 1.0 if math.sin(2 * math.pi * phase) >= 0 else -1.0
        v = 0.55 * math.sin(2 * math.pi * phase) + 0.35 * sq
        out.append(v * env[i] * 0.26)
    return out


def synth_kick(duration: float = 0.32):
    n = int(duration * SAMPLE_RATE)
    out = []
    for i in range(n):
        t = i / SAMPLE_RATE
        f = 46.0 + 95.0 * math.exp(-t * 38.0)
        e = math.exp(-t * 10.0)
        out.append(math.sin(2 * math.pi * f * t) * e * 0.75)
    return out


def synth_snare(duration: float = 0.19):
    n = int(duration * SAMPLE_RATE)
    out = []
    for i in range(n):
        t = i / SAMPLE_RATE
        e = math.exp(-t * 26.0)
        noise = rng.uniform(-1.0, 1.0)
        body = math.sin(2 * math.pi * 185.0 * t)
        out.append((0.55 * noise + 0.35 * body) * e * 0.30)
    return out


def synth_hat(duration: float = 0.055, open_hat: bool = False):
    n = int(duration * SAMPLE_RATE)
    out = []
    prev = 0.0
    for i in range(n):
        t = i / SAMPLE_RATE
        e = math.exp(-t * (60.0 if not open_hat else 18.0))
        noise = rng.uniform(-1.0, 1.0)
        # crude high-pass to keep it crisp
        hp = noise - prev
        prev = noise
        out.append(hp * e * 0.12)
    return out


def chord_notes(root: int, quality: str):
    third = 3 if quality == "minor" else 4
    fifth = 7
    return [root, root + third, root + fifth, root + 12, root + 12 + third, root + 12 + fifth]


# Chord pads + bass
for bar in range(16):
    _, root, quality = PROGRESSION[bar % 4]
    notes = chord_notes(root, quality)
    pad = synth_pad([midi_hz(n + 12) for n in notes[:4]], 4 * 60.0 / BPM)
    add_stereo(bar * 4 * 60.0 / BPM, pad, pan=0.5, gain=0.85)
    # bass on every beat, with a lift on the last beat
    for beat_in_bar in range(4):
        b = root - 12
        if beat_in_bar == 3:
            b += 7
        add_stereo(
            (bar * 4 + beat_in_bar) * 60.0 / BPM,
            synth_bass(midi_hz(b), 0.46),
            pan=0.5,
            gain=0.95,
        )

# Lead events (these are also the chart notes)
for bar in range(16):
    _, root, quality = PROGRESSION[bar % 4]
    degrees = MINOR_DEGREES if quality == "minor" else MAJOR_DEGREES
    for step, lane in events_for_bar(bar):
        beat = bar * 4 + step / 4.0
        pitch = root + degrees[lane]
        pan = 0.18 + 0.64 * (lane / 5.0)
        gain = 0.95 if bar not in (1, 2) else 0.55
        add_stereo(beat * 60.0 / BPM, synth_lead(midi_hz(pitch), 0.24), pan=pan, gain=gain)

# Drums
for bar in range(16):
    for step in range(16):
        beat = bar * 4 + step / 4.0
        t = beat * 60.0 / BPM
        if step in (0, 8):
            add_stereo(t, synth_kick(), pan=0.5, gain=1.0)
        if step in (4, 12):
            add_stereo(t, synth_snare(), pan=0.5, gain=0.9)
        if step % 2 == 0:
            add_stereo(t, synth_hat(open_hat=(step == 14)), pan=0.58, gain=0.75)

# A short crash on the first beat of the last phrase.
add_stereo(56 * 60.0 / BPM, synth_hat(0.5, open_hat=True), pan=0.5, gain=2.0)

# Simple echo to glue the lead into the mix.
delay = int(0.25 * SAMPLE_RATE)  # dotted 8th at 120 BPM
feedback = 0.26
for i in range(delay, NUM_FRAMES):
    left[i] += right[i - delay] * feedback * 0.35
    right[i] += left[i - delay] * feedback * 0.35

# Soft clip + normalize.
peak = 0.0
for i in range(NUM_FRAMES):
    left[i] = math.tanh(left[i] * 1.15)
    right[i] = math.tanh(right[i] * 1.15)
    peak = max(peak, abs(left[i]), abs(right[i]))
scale = 0.90 / max(peak, 1e-6)


def write_wav(path: Path, channels: int, frames: list[list[float]]):
    with wave.open(str(path), "wb") as f:
        f.setnchannels(channels)
        f.setsampwidth(2)
        f.setframerate(SAMPLE_RATE)
        data = bytearray()
        for i in range(len(frames[0])):
            for ch in range(channels):
                v = int(max(-1.0, min(1.0, frames[ch][i] * scale)) * 32767)
                data += struct.pack("<h", v)
        f.writeframes(data)


def to_ogg(wav_path: Path, ogg_path: Path):
    subprocess.run(
        [
            "ffmpeg",
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            str(wav_path),
            "-c:a",
            "libvorbis",
            "-q:a",
            "5",
            str(ogg_path),
        ],
        check=True,
    )


AUDIO_DIR.mkdir(parents=True, exist_ok=True)
CHART_DIR.mkdir(parents=True, exist_ok=True)

with tempfile.TemporaryDirectory() as tmp:
    tmp = Path(tmp)
    wav_path = tmp / "hex_demo.wav"
    write_wav(wav_path, 2, [left, right])
    to_ogg(wav_path, AUDIO_DIR / "hex_demo.ogg")

    # Click SFX -----------------------------------------------------------
    def click(freq: float, duration: float, gain: float):
        n = int(duration * SAMPLE_RATE)
        out = [0.0] * n
        for i in range(n):
            t = i / SAMPLE_RATE
            e = math.exp(-t * 55.0)
            out[i] = (math.sin(2 * math.pi * freq * t) + 0.4 * math.sin(2 * math.pi * freq * 2.02 * t)) * e * gain
        buf = [out, out]
        return buf

    for name, freq, dur, gain in [
        ("click", 1180.0, 0.035, 0.32),
        ("click_accent", 1680.0, 0.055, 0.36),
    ]:
        path = tmp / f"{name}.wav"
        buf = click(freq, dur, gain)
        # write_wav normalizes globally; these are already in range.
        with wave.open(str(path), "wb") as f:
            f.setnchannels(2)
            f.setsampwidth(2)
            f.setframerate(SAMPLE_RATE)
            data = bytearray()
            for i in range(len(buf[0])):
                for ch in range(2):
                    v = int(max(-1.0, min(1.0, buf[ch][i])) * 32767)
                    data += struct.pack("<h", v)
            f.writeframes(data)
        to_ogg(path, AUDIO_DIR / f"{name}.ogg")

# ----------------------------------------------------------------------- chart

notes = [{"lane": lane, "beat": round(beat, 3)} for beat, lane in CHART_EVENTS]
notes.sort(key=lambda n: (n["beat"], n["lane"]))
chart = {
    "format": 1,
    "title": "Neon Hexagon",
    "artist": "NotationEditor",
    "charter": "NotationEditor",
    "difficulty": "Normal / 6K",
    "bpm": BPM,
    "offset_ms": 0.0,
    "approach_ms": 1800.0,
    "audio": "audio/hex_demo.ogg",
    "notes": notes,
}
(CHART_DIR / "hex_demo.json").write_text(
    json.dumps(chart, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
)

print(f"wrote {AUDIO_DIR / 'hex_demo.ogg'} ({SECONDS + TAIL:.1f}s)")
print(f"wrote {AUDIO_DIR / 'click.ogg'}, {AUDIO_DIR / 'click_accent.ogg'}")
print(f"wrote {CHART_DIR / 'hex_demo.json'} ({len(notes)} notes, {BPM:g} BPM)")
