//! Chart data model, validation and persistence.
//!
//! A chart stores notes in *beats* instead of seconds so that the same file can
//! be re-timed simply by changing the BPM.  Beat 0 happens at `offset_ms`
//! milliseconds into the audio track.

use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: u32 = 1;
pub const LANE_COUNT: usize = 6;
pub const MIN_BPM: f64 = 30.0;
pub const MAX_BPM: f64 = 300.0;
pub const DEFAULT_APPROACH_MS: f64 = 1_800.0;
pub const MIN_APPROACH_MS: f64 = 500.0;
pub const MAX_APPROACH_MS: f64 = 5_000.0;
pub const MAX_NOTES: usize = 50_000;
const DUPLICATE_EPSILON: f64 = 1e-6;

/// A single tap note.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChartNote {
    /// Lane index in `0..LANE_COUNT`.
    pub lane: u8,
    /// Position in beats from the start of the chart.
    pub beat: f64,
}

impl ChartNote {
    pub fn new(lane: u8, beat: f64) -> Self {
        Self { lane, beat }
    }
}

/// The on-disk chart format (version 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chart {
    /// Format version. Used for migrations.
    #[serde(default = "default_format")]
    pub format: u32,
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default = "default_artist")]
    pub artist: String,
    #[serde(default = "default_charter")]
    pub charter: String,
    #[serde(default = "default_difficulty")]
    pub difficulty: String,
    /// Beats per minute. Must be in `MIN_BPM..=MAX_BPM`.
    #[serde(default = "default_bpm")]
    pub bpm: f64,
    /// Audio time of beat 0, in milliseconds.
    #[serde(default)]
    pub offset_ms: f64,
    /// How long a note takes to travel from the outer ring to the judge ring.
    #[serde(default = "default_approach_ms")]
    pub approach_ms: f64,
    /// Path relative to the Bevy `assets/` directory. `None` means the chart is
    /// played with a generated metronome instead of a song.
    #[serde(default)]
    pub audio: Option<String>,
    #[serde(default)]
    pub notes: Vec<ChartNote>,
}

fn default_format() -> u32 {
    FORMAT_VERSION
}
fn default_title() -> String {
    "Untitled".to_string()
}
fn default_artist() -> String {
    "Unknown".to_string()
}
fn default_charter() -> String {
    "Unknown".to_string()
}
fn default_difficulty() -> String {
    "Normal".to_string()
}
fn default_bpm() -> f64 {
    120.0
}
fn default_approach_ms() -> f64 {
    DEFAULT_APPROACH_MS
}

impl Default for Chart {
    fn default() -> Self {
        Self {
            format: FORMAT_VERSION,
            title: default_title(),
            artist: default_artist(),
            charter: default_charter(),
            difficulty: default_difficulty(),
            bpm: 120.0,
            offset_ms: 0.0,
            approach_ms: DEFAULT_APPROACH_MS,
            audio: None,
            notes: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub enum ChartError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Invalid(Vec<String>),
}

impl fmt::Display for ChartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChartError::Io(err) => write!(f, "I/O error: {err}"),
            ChartError::Json(err) => write!(f, "JSON error: {err}"),
            ChartError::Invalid(errors) => {
                write!(f, "invalid chart: {}", errors.join("; "))
            }
        }
    }
}

impl std::error::Error for ChartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ChartError::Io(err) => Some(err),
            ChartError::Json(err) => Some(err),
            ChartError::Invalid(_) => None,
        }
    }
}

impl From<std::io::Error> for ChartError {
    fn from(value: std::io::Error) -> Self {
        ChartError::Io(value)
    }
}

impl From<serde_json::Error> for ChartError {
    fn from(value: serde_json::Error) -> Self {
        ChartError::Json(value)
    }
}

impl Chart {
    /// Convert a beat position to seconds of audio time.
    pub fn seconds_from_beat(&self, beat: f64) -> f64 {
        self.offset_ms / 1_000.0 + beat * 60.0 / self.bpm
    }

    /// Convert seconds of audio time to a beat position.
    pub fn beat_from_seconds(&self, seconds: f64) -> f64 {
        (seconds - self.offset_ms / 1_000.0) * self.bpm / 60.0
    }

    /// Audio time at which a note must be hit.
    pub fn hit_time(&self, note: &ChartNote) -> f64 {
        self.seconds_from_beat(note.beat)
    }

    pub fn approach_secs(&self) -> f64 {
        self.approach_ms / 1_000.0
    }

    pub fn note_count(&self) -> usize {
        self.notes.len()
    }

    pub fn last_beat(&self) -> f64 {
        self.notes
            .iter()
            .map(|note| note.beat)
            .fold(0.0_f64, f64::max)
    }

    /// A comfortable end position for the editor timeline.
    pub fn editor_end_beat(&self) -> f64 {
        (self.last_beat() + 16.0).max(32.0)
    }

    /// Total playable length in seconds (last note plus a short tail).
    pub fn chart_duration_secs(&self) -> f64 {
        self.seconds_from_beat(self.last_beat()).max(0.0)
    }

    pub fn notes_in_lane(&self, lane: usize) -> impl Iterator<Item = &ChartNote> {
        self.notes
            .iter()
            .filter(move |note| note.lane as usize == lane)
    }

    /// Insert a note. Returns `false` if the lane is invalid, the beat is
    /// invalid, or a note already exists at that lane/beat.
    pub fn add_note(&mut self, lane: u8, beat: f64) -> bool {
        if lane as usize >= LANE_COUNT || !beat.is_finite() || beat < 0.0 {
            return false;
        }
        if self
            .notes
            .iter()
            .any(|note| note.lane == lane && (note.beat - beat).abs() < DUPLICATE_EPSILON)
        {
            return false;
        }
        self.notes.push(ChartNote::new(lane, beat));
        self.sort_notes();
        true
    }

    /// Remove the closest note in `lane` within `tolerance` beats.
    pub fn remove_note_near(&mut self, lane: u8, beat: f64, tolerance: f64) -> bool {
        let mut best: Option<(usize, f64)> = None;
        for (index, note) in self.notes.iter().enumerate() {
            if note.lane != lane {
                continue;
            }
            let distance = (note.beat - beat).abs();
            if distance <= tolerance && best.is_none_or(|(_, d)| distance < d) {
                best = Some((index, distance));
            }
        }
        if let Some((index, _)) = best {
            self.notes.remove(index);
            true
        } else {
            false
        }
    }

    pub fn has_note_at(&self, lane: u8, beat: f64) -> bool {
        self.notes
            .iter()
            .any(|note| note.lane == lane && (note.beat - beat).abs() < DUPLICATE_EPSILON)
    }

    pub fn sort_notes(&mut self) {
        self.notes
            .sort_by(|a, b| a.beat.total_cmp(&b.beat).then_with(|| a.lane.cmp(&b.lane)));
    }

    /// Check semantics and invariants. Notes do not have to be pre-sorted.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.format != FORMAT_VERSION {
            errors.push(format!(
                "unsupported format {} (expected {FORMAT_VERSION})",
                self.format
            ));
        }
        if self.title.trim().is_empty() {
            errors.push("title must not be empty".to_string());
        }
        if !self.bpm.is_finite() || !(MIN_BPM..=MAX_BPM).contains(&self.bpm) {
            errors.push(format!(
                "bpm must be between {MIN_BPM} and {MAX_BPM}, got {}",
                self.bpm
            ));
        }
        if !self.offset_ms.is_finite() || !(0.0..=60_000.0).contains(&self.offset_ms) {
            errors.push(format!(
                "offset_ms must be between 0 and 60000, got {}",
                self.offset_ms
            ));
        }
        if !self.approach_ms.is_finite()
            || !(MIN_APPROACH_MS..=MAX_APPROACH_MS).contains(&self.approach_ms)
        {
            errors.push(format!(
                "approach_ms must be between {MIN_APPROACH_MS} and {MAX_APPROACH_MS}, got {}",
                self.approach_ms
            ));
        }
        if let Some(audio) = &self.audio {
            if audio.trim().is_empty() {
                errors.push("audio must be a non-empty relative path or null".to_string());
            } else {
                let path = Path::new(audio);
                if path.is_absolute()
                    || path
                        .components()
                        .any(|part| matches!(part, Component::ParentDir | Component::RootDir))
                {
                    errors.push(format!("audio path must be relative to assets/: {audio:?}"));
                }
            }
        }
        if self.notes.len() > MAX_NOTES {
            errors.push(format!(
                "too many notes: {} (max {MAX_NOTES})",
                self.notes.len()
            ));
        }
        for (index, note) in self.notes.iter().enumerate() {
            if note.lane as usize >= LANE_COUNT {
                errors.push(format!(
                    "note #{index}: lane {} is out of range 0..{}",
                    note.lane, LANE_COUNT
                ));
            }
            if !note.beat.is_finite() || !(0.0..=1_000_000.0).contains(&note.beat) {
                errors.push(format!(
                    "note #{index}: beat must be between 0 and 1000000, got {}",
                    note.beat
                ));
            }
        }

        let mut sorted = self.notes.clone();
        sorted.sort_by(|a, b| a.beat.total_cmp(&b.beat).then_with(|| a.lane.cmp(&b.lane)));
        for pair in sorted.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if a.lane == b.lane && (a.beat - b.beat).abs() < DUPLICATE_EPSILON {
                errors.push(format!("duplicate note at lane {} beat {}", a.lane, a.beat));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Parse a chart from JSON and normalise note order.
    pub fn from_json(json: &str) -> Result<Self, ChartError> {
        let mut chart: Chart = serde_json::from_str(json)?;
        if let Err(errors) = chart.validate() {
            return Err(ChartError::Invalid(errors));
        }
        chart.sort_notes();
        Ok(chart)
    }

    pub fn to_json(&self) -> Result<String, ChartError> {
        if let Err(errors) = self.validate() {
            return Err(ChartError::Invalid(errors));
        }
        let mut sorted = self.clone();
        sorted.sort_notes();
        Ok(serde_json::to_string_pretty(&sorted)?)
    }

    /// Load a chart from disk. Notes are sorted after a successful load.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ChartError> {
        let text = fs::read_to_string(path.as_ref())?;
        Self::from_json(&text)
    }

    /// Save a chart atomically (write to a sibling temp file, then rename).
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ChartError> {
        let path = path.as_ref();
        let json = self.to_json()?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let tmp: PathBuf = path.with_extension("json.tmp");
        fs::write(&tmp, json + "\n")?;
        fs::rename(&tmp, path)?;
        Ok(())
    }

    /// The chart that ships with the executable. Used as a fallback when the
    /// requested chart cannot be loaded (and by `--selftest`).
    pub fn builtin_demo() -> Result<Self, ChartError> {
        Self::from_json(include_str!("../charts/hex_demo.json"))
    }
}

/// Round a beat position to the nearest `1/division` of a beat.
pub fn snap_beat(beat: f64, division: u32) -> f64 {
    if division == 0 {
        return beat;
    }
    (beat * division as f64).round() / division as f64
}

/// Format a score with thousands separators.
pub fn format_score(score: u32) -> String {
    let raw = score.to_string();
    let mut out = String::with_capacity(raw.len() + raw.len() / 3);
    for (index, ch) in raw.chars().enumerate() {
        if index > 0 && (raw.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// `mm:ss.cs`
pub fn format_time(seconds: f64) -> String {
    let seconds = seconds.max(0.0);
    let minutes = (seconds / 60.0).floor() as u64;
    let secs = seconds % 60.0;
    format!("{minutes:02}:{secs:05.2}")
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    fn note(lane: u8, beat: f64) -> ChartNote {
        ChartNote::new(lane, beat)
    }

    #[test]
    fn time_conversion_roundtrip() {
        let mut chart = Chart::default();
        chart.bpm = 150.0;
        chart.offset_ms = 250.0;
        for beat in [0.0, 0.25, 1.0, 3.5, 64.0] {
            let secs = chart.seconds_from_beat(beat);
            assert!((chart.beat_from_seconds(secs) - beat).abs() < 1e-9);
        }
        assert!((chart.seconds_from_beat(4.0) - 1.85).abs() < 1e-9);
    }

    #[test]
    fn add_note_rejects_duplicates_and_bad_lanes() {
        let mut chart = Chart::default();
        assert!(chart.add_note(0, 1.0));
        assert!(!chart.add_note(0, 1.0));
        assert!(chart.add_note(0, 1.5));
        assert!(!chart.add_note(6, 2.0));
        assert!(!chart.add_note(1, -1.0));
        assert!(!chart.add_note(1, f64::NAN));
        assert_eq!(chart.notes.len(), 2);
    }

    #[test]
    fn notes_are_sorted_after_add() {
        let mut chart = Chart::default();
        chart.add_note(5, 3.0);
        chart.add_note(1, 1.0);
        chart.add_note(3, 2.0);
        assert_eq!(chart.notes, vec![note(1, 1.0), note(3, 2.0), note(5, 3.0)]);
    }

    #[test]
    fn remove_note_uses_tolerance() {
        let mut chart = Chart::default();
        chart.add_note(2, 4.0);
        assert!(!chart.remove_note_near(2, 4.4, 0.1));
        assert!(chart.remove_note_near(2, 4.06, 0.1));
        assert!(chart.notes.is_empty());
    }

    #[test]
    fn validation_catches_invalid_values() {
        let mut chart = Chart::default();
        chart.bpm = 10.0;
        chart.offset_ms = -1.0;
        chart.approach_ms = 100.0;
        chart.audio = Some("../secret.ogg".to_string());
        chart.notes.push(note(7, 1.0));
        let errors = chart.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("bpm")));
        assert!(errors.iter().any(|e| e.contains("offset_ms")));
        assert!(errors.iter().any(|e| e.contains("approach_ms")));
        assert!(errors.iter().any(|e| e.contains("audio")));
        assert!(errors.iter().any(|e| e.contains("lane")));
    }

    #[test]
    fn validation_catches_duplicates_even_when_unsorted() {
        let mut chart = Chart::default();
        chart.notes = vec![note(1, 2.0), note(1, 1.0), note(1, 1.0)];
        let errors = chart.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("duplicate")));
    }

    #[test]
    fn json_roundtrip_preserves_everything() {
        let mut chart = Chart::default();
        chart.title = "Test Chart".into();
        chart.artist = "Unit Test".into();
        chart.bpm = 174.0;
        chart.offset_ms = 125.5;
        chart.approach_ms = 2_200.0;
        chart.audio = Some("audio/test.ogg".into());
        chart.add_note(0, 1.0);
        chart.add_note(5, 1.0);
        chart.add_note(2, 0.25);
        let json = chart.to_json().unwrap();
        let parsed = Chart::from_json(&json).unwrap();
        assert_eq!(parsed, chart);
    }

    #[test]
    fn json_parsing_fills_defaults() {
        let chart = Chart::from_json(r#"{"notes": []}"#).unwrap();
        assert_eq!(chart.bpm, 120.0);
        assert_eq!(chart.format, FORMAT_VERSION);
        assert_eq!(chart.approach_ms, DEFAULT_APPROACH_MS);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "notation_editor_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("nested").join("chart.json");
        let mut chart = Chart::default();
        chart.add_note(3, 2.5);
        chart.save(&path).unwrap();
        let loaded = Chart::load(&path).unwrap();
        assert_eq!(loaded.notes, chart.notes);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn clamp_helpers() {
        assert!((snap_beat(1.24, 4) - 1.25).abs() < 1e-9);
        assert!((snap_beat(1.24, 1) - 1.0).abs() < 1e-9);
        assert!((snap_beat(1.26, 2) - 1.5).abs() < 1e-9);
        assert_eq!(format_score(1_234_567), "1,234,567");
        assert_eq!(format_score(999), "999");
        assert_eq!(format_time(65.25), "01:05.25");
    }

    #[test]
    fn builtin_demo_is_valid() {
        let chart = Chart::builtin_demo().unwrap();
        assert!(chart.note_count() > 100);
        assert_eq!(chart.title, "Neon Hexagon");
        assert!(chart.notes.windows(2).all(|pair| {
            pair[0].beat < pair[1].beat
                || (pair[0].beat == pair[1].beat && pair[0].lane < pair[1].lane)
        }));
    }
}
