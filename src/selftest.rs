//! Headless self-test: verifies chart IO and judgements without opening a
//! window or touching an audio device. Run with `cargo run -- --selftest`.

use std::path::PathBuf;

use crate::chart::{Chart, format_score};
use crate::game::{GameSession, Judgment};

pub fn run() -> i32 {
    println!("NotationEditor self-test");
    println!("========================");

    let chart = match Chart::builtin_demo() {
        Ok(chart) => chart,
        Err(err) => {
            eprintln!("FAIL: embedded chart is invalid: {err}");
            return 1;
        }
    };
    println!(
        "chart: {}  |  {}  |  {} notes  |  {:.0} BPM",
        chart.title,
        chart.difficulty,
        chart.note_count(),
        chart.bpm
    );

    if let Err(errors) = chart.validate() {
        eprintln!("FAIL: chart validation: {}", errors.join("; "));
        return 1;
    }
    println!("PASS chart validation");

    // JSON round-trip.
    let json = match chart.to_json() {
        Ok(json) => json,
        Err(err) => {
            eprintln!("FAIL: serialise chart: {err}");
            return 1;
        }
    };
    match Chart::from_json(&json) {
        Ok(parsed) if parsed == chart => println!("PASS JSON round-trip"),
        Ok(_) => {
            eprintln!("FAIL: JSON round-trip changed the chart");
            return 1;
        }
        Err(err) => {
            eprintln!("FAIL: parse chart: {err}");
            return 1;
        }
    }

    // Save/load round-trip in a temp directory.
    let temp_dir = std::env::temp_dir().join(format!(
        "notation_editor_selftest_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    let temp_path: PathBuf = temp_dir.join("chart.json");
    match chart.save(&temp_path).and_then(|_| Chart::load(&temp_path)) {
        Ok(loaded) if loaded == chart => println!("PASS save/load round-trip"),
        Ok(_) => {
            eprintln!("FAIL: save/load changed the chart");
            let _ = std::fs::remove_dir_all(&temp_dir);
            return 1;
        }
        Err(err) => {
            eprintln!("FAIL: save/load: {err}");
            let _ = std::fs::remove_dir_all(&temp_dir);
            return 1;
        }
    }
    let _ = std::fs::remove_dir_all(&temp_dir);

    // Perfect autoplay.
    let mut perfect = GameSession::new(chart.clone());
    let note_count = perfect.notes.len();
    for index in 0..note_count {
        let note = perfect.notes[index];
        perfect.tap(note.note.lane, note.hit_time);
    }
    let missed = perfect.advance(perfect.last_hit_time + 10.0);
    if !missed.is_empty() {
        eprintln!("FAIL: perfect run produced {} misses", missed.len());
        return 1;
    }
    let perfect_summary = perfect.summary();
    if perfect_summary.counts[Judgment::Perfect.index()] as usize != note_count
        || perfect_summary.score != perfect_summary.max_score
        || perfect_summary.rank != crate::game::Rank::S
        || !perfect_summary.all_perfect
    {
        eprintln!("FAIL: perfect autoplay result is wrong: {perfect_summary:?}");
        return 1;
    }
    println!(
        "PASS perfect autoplay ({} notes, {} pts, accuracy {:.2}%, rank {})",
        perfect_summary.total_notes,
        format_score(perfect_summary.score),
        perfect_summary.accuracy * 100.0,
        perfect_summary.rank.letter()
    );

    // Mixed run: miss every 5th, great every 7th, good every 11th.
    let mut mixed = GameSession::new(chart.clone());
    for index in 0..mixed.notes.len() {
        if index % 5 == 4 {
            continue;
        }
        let note = mixed.notes[index];
        let offset = if index % 7 == 3 {
            0.060
        } else if index % 11 == 5 {
            -0.100
        } else {
            0.0
        };
        // Only tap if this note is still pending. A shifted tap can hit a
        // neighbouring note first in pathological cases; that is acceptable
        // for this smoke test as long as every note ends up judged.
        if mixed.notes[index].judgment.is_none() {
            mixed.tap(note.note.lane, note.hit_time + offset);
        }
    }
    mixed.advance(mixed.last_hit_time + 10.0);
    let summary = mixed.summary();
    let total_judged: u32 = summary.counts.iter().sum();
    if total_judged as usize != note_count {
        eprintln!(
            "FAIL: mixed run judged {} of {} notes",
            total_judged, note_count
        );
        return 1;
    }
    println!(
        "PASS mixed run (P{} G{} O{} M{}, combo {}, accuracy {:.2}%, rank {})",
        summary.counts[0],
        summary.counts[1],
        summary.counts[2],
        summary.counts[3],
        summary.max_combo,
        summary.accuracy * 100.0,
        summary.rank.letter()
    );

    println!("========================");
    println!("all checks passed");
    0
}
