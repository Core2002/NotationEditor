//! Audio waveform decoding for the chart editor's background display.
//!
//! Gameplay audio is played by Bevy; this module only produces a compact peak
//! envelope used to draw the song behind the timeline.

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Number of frames that are reduced into one envelope value before the final
/// resampling pass. 64 frames ≈ 1.5 ms at 44.1 kHz.
const BLOCK_FRAMES: usize = 64;

#[derive(Debug, Clone)]
pub struct Waveform {
    /// Normalised envelope in `0.0..=1.0`.
    pub peaks: Vec<f32>,
    pub duration_secs: f64,
}

impl Waveform {
    pub fn sample_at_time(&self, seconds: f64) -> f32 {
        if self.peaks.is_empty() || self.duration_secs <= 0.0 {
            return 0.0;
        }
        let t = (seconds / self.duration_secs).clamp(0.0, 1.0);
        let index = ((self.peaks.len() - 1) as f64 * t).round() as usize;
        self.peaks[index]
    }
}

/// Decode `path` and return a normalised envelope with `buckets` entries.
pub fn load_waveform(path: &Path, buckets: usize) -> Result<Waveform, String> {
    let file = File::open(path).map_err(|err| format!("cannot open {}: {err}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(extension);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|err| format!("unsupported audio format: {err}"))?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| "audio file has no default track".to_string())?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|err| format!("unsupported codec: {err}"))?;

    let mut raw_peaks: Vec<f32> = Vec::new();
    let mut block_peak = 0.0_f32;
    let mut block_frames = 0usize;
    let mut total_frames = 0u64;
    let mut sample_rate = 44_100u32;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(err) => return Err(format!("failed to read audio packet: {err}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(err) => return Err(format!("failed to decode audio: {err}")),
        };

        let spec = *decoded.spec();
        sample_rate = spec.rate;
        let channels = spec.channels.count().max(1);
        let mut samples = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        samples.copy_interleaved_ref(decoded);
        let interleaved = samples.samples();
        for frame in interleaved.chunks(channels) {
            let peak = frame
                .iter()
                .fold(0.0_f32, |acc, sample| acc.max(sample.abs()));
            block_peak = block_peak.max(peak);
            block_frames += 1;
            total_frames += 1;
            if block_frames >= BLOCK_FRAMES {
                raw_peaks.push(block_peak);
                block_peak = 0.0;
                block_frames = 0;
            }
        }
    }

    if block_frames > 0 {
        raw_peaks.push(block_peak);
    }
    if raw_peaks.is_empty() || total_frames == 0 {
        return Err("audio track is empty".to_string());
    }

    let duration_secs = total_frames as f64 / sample_rate as f64;
    Ok(Waveform {
        peaks: downsample_peaks(&raw_peaks, buckets),
        duration_secs,
    })
}

/// Resample a peak list into exactly `buckets` values using block maxima and
/// a gentle curve so quiet sections remain visible.
pub fn downsample_peaks(raw: &[f32], buckets: usize) -> Vec<f32> {
    if raw.is_empty() || buckets == 0 {
        return Vec::new();
    }
    let global_peak = raw.iter().fold(0.0_f32, |acc, value| acc.max(*value));
    let normalizer = if global_peak > 1e-6 {
        1.0 / global_peak
    } else {
        0.0
    };
    let mut out = Vec::with_capacity(buckets);
    for bucket in 0..buckets {
        let start = bucket * raw.len() / buckets;
        let end = ((bucket + 1) * raw.len() / buckets).max(start + 1);
        let max = raw[start..end.min(raw.len())]
            .iter()
            .fold(0.0_f32, |acc, value| acc.max(*value));
        out.push((max * normalizer).powf(0.72));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn downsample_returns_requested_length() {
        let raw: Vec<f32> = (0..1_000).map(|i| i as f32 / 1_000.0).collect();
        let peaks = downsample_peaks(&raw, 128);
        assert_eq!(peaks.len(), 128);
        assert!(peaks.iter().all(|value| (0.0..=1.0).contains(value)));
        assert!(peaks[0] < peaks[127]);
    }

    #[test]
    fn downsample_handles_short_and_empty_input() {
        assert!(downsample_peaks(&[], 10).is_empty());
        assert!(downsample_peaks(&[0.5], 0).is_empty());
        let peaks = downsample_peaks(&[0.25, 1.0], 16);
        assert_eq!(peaks.len(), 16);
        assert!(peaks.iter().any(|value| (*value - 1.0).abs() < 1e-5));
    }

    fn write_test_wav(path: &Path) {
        let sample_rate = 44_100u32;
        let frames = 11_025u32;
        let mut data = Vec::with_capacity(frames as usize * 2);
        for i in 0..frames {
            let t = i as f32 / sample_rate as f32;
            let sample = (t * 440.0 * std::f32::consts::TAU).sin() * 0.5;
            data.extend_from_slice(&((sample * 32_767.0) as i16).to_le_bytes());
        }
        let mut file = File::create(path).unwrap();
        file.write_all(b"RIFF").unwrap();
        file.write_all(&(36 + data.len() as u32).to_le_bytes())
            .unwrap();
        file.write_all(b"WAVEfmt ").unwrap();
        file.write_all(&16u32.to_le_bytes()).unwrap();
        file.write_all(&1u16.to_le_bytes()).unwrap();
        file.write_all(&1u16.to_le_bytes()).unwrap();
        file.write_all(&sample_rate.to_le_bytes()).unwrap();
        file.write_all(&(sample_rate * 2).to_le_bytes()).unwrap();
        file.write_all(&2u16.to_le_bytes()).unwrap();
        file.write_all(&16u16.to_le_bytes()).unwrap();
        file.write_all(b"data").unwrap();
        file.write_all(&(data.len() as u32).to_le_bytes()).unwrap();
        file.write_all(&data).unwrap();
    }

    #[test]
    fn decodes_a_real_wav_file() {
        let path = std::env::temp_dir().join(format!(
            "notation_editor_waveform_{}.wav",
            std::process::id()
        ));
        write_test_wav(&path);
        let waveform = load_waveform(&path, 64).expect("wav should decode");
        assert!((waveform.duration_secs - 0.25).abs() < 0.02);
        assert_eq!(waveform.peaks.len(), 64);
        assert!(waveform.peaks.iter().any(|peak| *peak > 0.5));
        let _ = std::fs::remove_file(path);
    }
}
