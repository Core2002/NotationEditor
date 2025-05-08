use bevy::prelude::*;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

pub fn decode_mp3(file_path: &str) -> Vec<i16> {
    let mut samples = Vec::new();
    let file = std::fs::File::open(file_path).expect("Failed to open file");
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    hint.mime_type("audio/mpeg");

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &Default::default(), &Default::default())
        .expect("Unsupported format");

    let mut format = probed.format;
    let track = format.default_track().expect("No audio track found");
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &Default::default())
        .expect("Unsupported codec");

    while let Ok(packet) = format.next_packet() {
        if let Ok(audio_buf) = decoder.decode(&packet) {
            if let AudioBufferRef::S16(typed) = audio_buf {
                samples.extend(typed.chan(0));
            } else {
                panic!("Unsupported audio format");
            }
        }
    }
    samples
}

#[derive(Component)]
pub struct TimeDomainGraph {
    pub samples: Vec<i16>,
}

pub fn render_graph(mut gizmos: Gizmos, time_domain: Query<&TimeDomainGraph>) {
    if let Ok(graph) = time_domain.single() {
        let sample_count = graph.samples.len();
        if sample_count == 0 {
            return;
        }

        // Normalize samples to [-1, 1] range
        let max_sample = i16::MAX as f32;
        let normalized_samples: Vec<f32> = graph
            .samples
            .iter()
            .map(|&s| s as f32 / max_sample)
            .collect();

        let tick_interval = 1.0; // Time interval in seconds
        let sample_rate = 44100; // Sample rate of the audio (adjust as needed)
        let samples_per_tick = (tick_interval * sample_rate as f32) as usize;

        // Graph parameters
        let width = sample_rate as f32; // Width of the graph in world units
        let height = 400.0; // Height of the graph in world units
        let center = Vec2::ZERO; // Center position of the graph

        // Calculate the x step between samples
        let x_step = width / (sample_count as f32);

        // Draw the graph
        for i in (0..sample_count - 1).step_by(8) {
            let x1 = center.x - width / 2.0 + i as f32 * x_step;
            let y1 = center.y + normalized_samples[i] * height / 2.0;

            let x2 = center.x - width / 2.0 + (i + 1) as f32 * x_step;
            let y2 = center.y + normalized_samples[i + 1] * height / 2.0;

            gizmos.line_2d(
                Vec2::new(y1, x1),
                Vec2::new(y2, x2),
                Color::srgba(0., 255., 0., 0.4),
            );
        }

        // Draw time ticks
        for i in (0..sample_count).step_by(samples_per_tick) {
            let x = center.x - width / 2.0 + i as f32 * x_step;
            gizmos.line_2d(
                Vec2::new(center.y - 10.0, x), // Start of the tick (slightly below the x-axis)
                Vec2::new(center.y + 10.0, x), // End of the tick (slightly above the x-axis)
                Color::srgb(255., 0., 0.),
            );
        }
    }
}
