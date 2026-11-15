//! Timing windows, hit judgement, scoring and session summaries.
//!
//! This module is deliberately free of Bevy ECS types so it can be exercised
//! headlessly (`--selftest`) and in unit tests.

use bevy::prelude::Resource;

use crate::chart::{Chart, ChartNote};

/// Judgement windows in milliseconds. A note that is not hit within
/// `GOOD_WINDOW_MS` becomes a miss automatically.
pub const PERFECT_WINDOW_MS: f64 = 45.0;
pub const GREAT_WINDOW_MS: f64 = 90.0;
pub const GOOD_WINDOW_MS: f64 = 140.0;

/// Extra time after the last note before the results screen appears.
pub const END_DELAY_SECS: f64 = 2.2;

pub const POINTS_PERFECT: u32 = 1_000;
pub const POINTS_GREAT: u32 = 700;
pub const POINTS_GOOD: u32 = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Judgment {
    Perfect,
    Great,
    Good,
    Miss,
}

impl Judgment {
    pub const ALL: [Judgment; 4] = [
        Judgment::Perfect,
        Judgment::Great,
        Judgment::Good,
        Judgment::Miss,
    ];

    pub const fn index(self) -> usize {
        match self {
            Judgment::Perfect => 0,
            Judgment::Great => 1,
            Judgment::Good => 2,
            Judgment::Miss => 3,
        }
    }

    pub const fn points(self) -> u32 {
        match self {
            Judgment::Perfect => POINTS_PERFECT,
            Judgment::Great => POINTS_GREAT,
            Judgment::Good => POINTS_GOOD,
            Judgment::Miss => 0,
        }
    }

    pub const fn weight(self) -> f64 {
        self.points() as f64 / POINTS_PERFECT as f64
    }

    pub const fn label(self) -> &'static str {
        match self {
            Judgment::Perfect => "PERFECT",
            Judgment::Great => "GREAT",
            Judgment::Good => "GOOD",
            Judgment::Miss => "MISS",
        }
    }

    pub const fn is_hit(self) -> bool {
        !matches!(self, Judgment::Miss)
    }

    /// Classify an absolute timing error in seconds.
    pub fn from_error_secs(error: f64) -> Option<Judgment> {
        let ms = error.abs() * 1_000.0;
        if ms <= PERFECT_WINDOW_MS {
            Some(Judgment::Perfect)
        } else if ms <= GREAT_WINDOW_MS {
            Some(Judgment::Great)
        } else if ms <= GOOD_WINDOW_MS {
            Some(Judgment::Good)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeNote {
    pub note: ChartNote,
    pub hit_time: f64,
    pub judgment: Option<Judgment>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitResult {
    pub note_index: usize,
    pub lane: u8,
    pub beat: f64,
    pub judgment: Judgment,
    /// Signed error in milliseconds. Negative = hit early.
    pub delta_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MissEvent {
    pub note_index: usize,
    pub lane: u8,
    pub beat: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    S,
    A,
    B,
    C,
    D,
}

impl Rank {
    pub const fn letter(self) -> &'static str {
        match self {
            Rank::S => "S",
            Rank::A => "A",
            Rank::B => "B",
            Rank::C => "C",
            Rank::D => "D",
        }
    }

    pub fn from_accuracy(accuracy: f64) -> Self {
        if accuracy >= 0.95 {
            Rank::S
        } else if accuracy >= 0.90 {
            Rank::A
        } else if accuracy >= 0.80 {
            Rank::B
        } else if accuracy >= 0.70 {
            Rank::C
        } else {
            Rank::D
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameSummary {
    pub title: String,
    pub difficulty: String,
    pub score: u32,
    pub max_score: u32,
    pub accuracy: f64,
    pub max_combo: u32,
    pub counts: [u32; 4],
    pub total_notes: usize,
    pub full_combo: bool,
    pub all_perfect: bool,
    pub rank: Rank,
}

/// Mutable state for one play of a chart.
#[derive(Debug, Clone, Resource)]
pub struct GameSession {
    pub chart: Chart,
    pub notes: Vec<RuntimeNote>,
    pub score: u32,
    pub combo: u32,
    pub max_combo: u32,
    /// Indexed by [`Judgment::index`]: perfect, great, good, miss.
    pub counts: [u32; 4],
    pub judged: usize,
    pub last_hit_time: f64,
}

impl GameSession {
    pub fn new(chart: Chart) -> Self {
        let notes: Vec<RuntimeNote> = chart
            .notes
            .iter()
            .map(|note| RuntimeNote {
                note: *note,
                hit_time: chart.hit_time(note),
                judgment: None,
            })
            .collect();
        let last_hit_time = notes
            .iter()
            .map(|note| note.hit_time)
            .fold(0.0_f64, f64::max);
        Self {
            chart,
            notes,
            score: 0,
            combo: 0,
            max_combo: 0,
            counts: [0; 4],
            judged: 0,
            last_hit_time,
        }
    }

    /// Reset all judgements while keeping the same chart and note order.
    pub fn reset(&mut self) {
        for note in &mut self.notes {
            note.judgment = None;
        }
        self.score = 0;
        self.combo = 0;
        self.max_combo = 0;
        self.counts = [0; 4];
        self.judged = 0;
    }

    pub fn total_notes(&self) -> usize {
        self.notes.len()
    }

    pub fn perfect(&self) -> u32 {
        self.counts[Judgment::Perfect.index()]
    }
    pub fn great(&self) -> u32 {
        self.counts[Judgment::Great.index()]
    }
    pub fn good(&self) -> u32 {
        self.counts[Judgment::Good.index()]
    }
    pub fn miss(&self) -> u32 {
        self.counts[Judgment::Miss.index()]
    }

    pub fn max_score(&self) -> u32 {
        self.total_notes() as u32 * POINTS_PERFECT
    }

    pub fn points_earned(&self) -> u32 {
        Judgment::ALL
            .iter()
            .map(|judgment| self.counts[judgment.index()] * judgment.points())
            .sum()
    }

    /// Live accuracy over judged notes. Returns 1.0 before the first note.
    pub fn accuracy(&self) -> f64 {
        if self.judged == 0 {
            1.0
        } else {
            self.points_earned() as f64 / (self.judged as f64 * POINTS_PERFECT as f64)
        }
    }

    /// Accuracy over the entire chart, including notes not yet judged as 0.
    pub fn final_accuracy(&self) -> f64 {
        if self.notes.is_empty() {
            1.0
        } else {
            self.points_earned() as f64 / (self.notes.len() as f64 * POINTS_PERFECT as f64)
        }
    }

    pub fn rank(&self) -> Rank {
        Rank::from_accuracy(self.final_accuracy())
    }

    pub fn full_combo(&self) -> bool {
        !self.notes.is_empty() && self.miss() == 0
    }

    pub fn all_perfect(&self) -> bool {
        !self.notes.is_empty() && self.perfect() as usize == self.notes.len()
    }

    pub fn progress(&self) -> f32 {
        if self.notes.is_empty() {
            1.0
        } else {
            self.judged as f32 / self.notes.len() as f32
        }
    }

    /// Hit the closest pending note in `lane` within the GOOD window.
    /// `None` means this was a ghost tap (no note close enough).
    pub fn tap(&mut self, lane: u8, now: f64) -> Option<HitResult> {
        if lane as usize >= crate::chart::LANE_COUNT {
            return None;
        }
        let mut best: Option<(usize, f64)> = None;
        for (index, note) in self.notes.iter().enumerate() {
            if note.judgment.is_some() || note.note.lane != lane {
                continue;
            }
            let delta = now - note.hit_time;
            let abs = delta.abs();
            if abs * 1_000.0 > GOOD_WINDOW_MS {
                continue;
            }
            if best.is_none_or(|(_, best_abs)| abs < best_abs) {
                best = Some((index, abs));
            }
        }
        let (index, _) = best?;
        let delta = now - self.notes[index].hit_time;
        let judgment = Judgment::from_error_secs(delta)?;
        self.apply(index, judgment);
        Some(HitResult {
            note_index: index,
            lane,
            beat: self.notes[index].note.beat,
            judgment,
            delta_ms: delta * 1_000.0,
        })
    }

    /// Tap every note whose hit time has arrived. Used by `--autoplay` and by
    /// the self-test. Taps are performed at each note's exact hit time so the
    /// result is a perfect run.
    pub fn autoplay(&mut self, now: f64) -> Vec<HitResult> {
        let due: Vec<(u8, f64)> = self
            .notes
            .iter()
            .filter(|note| note.judgment.is_none() && note.hit_time <= now + 0.001)
            .map(|note| (note.note.lane, note.hit_time))
            .collect();
        let mut results = Vec::with_capacity(due.len());
        for (lane, hit_time) in due {
            if let Some(hit) = self.tap(lane, hit_time) {
                results.push(hit);
            }
        }
        results
    }

    /// Mark every overdue note as a miss and return them for effects/HUD.
    pub fn advance(&mut self, now: f64) -> Vec<MissEvent> {
        let mut missed = Vec::new();
        for index in 0..self.notes.len() {
            if self.notes[index].judgment.is_some() {
                continue;
            }
            let late = now - self.notes[index].hit_time;
            if late * 1_000.0 > GOOD_WINDOW_MS {
                self.apply(index, Judgment::Miss);
                missed.push(MissEvent {
                    note_index: index,
                    lane: self.notes[index].note.lane,
                    beat: self.notes[index].note.beat,
                });
            }
        }
        missed
    }

    fn apply(&mut self, index: usize, judgment: Judgment) {
        debug_assert!(self.notes[index].judgment.is_none());
        self.notes[index].judgment = Some(judgment);
        self.counts[judgment.index()] += 1;
        self.judged += 1;
        self.score = self.points_earned();
        if judgment.is_hit() {
            self.combo += 1;
            self.max_combo = self.max_combo.max(self.combo);
        } else {
            self.combo = 0;
        }
    }

    pub fn is_finished(&self, now: f64) -> bool {
        self.judged == self.notes.len() && now >= self.last_hit_time + END_DELAY_SECS
    }

    pub fn summary(&self) -> GameSummary {
        GameSummary {
            title: self.chart.title.clone(),
            difficulty: self.chart.difficulty.clone(),
            score: self.score,
            max_score: self.max_score(),
            accuracy: self.final_accuracy(),
            max_combo: self.max_combo,
            counts: self.counts,
            total_notes: self.notes.len(),
            full_combo: self.full_combo(),
            all_perfect: self.all_perfect(),
            rank: self.rank(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::chart::ChartNote;

    fn test_chart(bpm: f64, notes: &[(u8, f64)]) -> Chart {
        let mut chart = Chart::default();
        chart.bpm = bpm;
        chart.notes = notes
            .iter()
            .map(|&(lane, beat)| ChartNote::new(lane, beat))
            .collect();
        chart
    }

    #[test]
    fn window_classification() {
        assert_eq!(Judgment::from_error_secs(0.0), Some(Judgment::Perfect));
        assert_eq!(Judgment::from_error_secs(0.045), Some(Judgment::Perfect));
        assert_eq!(Judgment::from_error_secs(0.046), Some(Judgment::Great));
        assert_eq!(Judgment::from_error_secs(-0.09), Some(Judgment::Great));
        assert_eq!(Judgment::from_error_secs(0.10), Some(Judgment::Good));
        assert_eq!(Judgment::from_error_secs(0.141), None);
    }

    #[test]
    fn perfect_tap_scores_maximum() {
        let mut session = GameSession::new(test_chart(120.0, &[(0, 0.0), (2, 1.0)]));
        let hit_time = session.notes[0].hit_time;
        let hit = session.tap(0, hit_time).unwrap();
        assert_eq!(hit.judgment, Judgment::Perfect);
        assert_eq!(hit.delta_ms, 0.0);
        assert_eq!(session.score, 1_000);
        assert_eq!(session.combo, 1);
        session.tap(2, session.notes[1].hit_time).unwrap();
        assert_eq!(session.score, 2_000);
        assert_eq!(session.combo, 2);
        assert_eq!(session.max_combo, 2);
        assert!(session.all_perfect());
        assert!(session.full_combo());
        assert_eq!(session.rank(), Rank::S);
    }

    #[test]
    fn timings_map_to_great_and_good() {
        let mut session = GameSession::new(test_chart(120.0, &[(0, 0.0), (0, 1.0)]));
        let t0 = session.notes[0].hit_time;
        let hit = session.tap(0, t0 + 0.060).unwrap();
        assert_eq!(hit.judgment, Judgment::Great);
        assert!((hit.delta_ms - 60.0).abs() < 1e-6);
        assert_eq!(session.score, POINTS_GREAT);

        let t1 = session.notes[1].hit_time;
        let hit = session.tap(0, t1 - 0.120).unwrap();
        assert_eq!(hit.judgment, Judgment::Good);
        assert_eq!(session.score, POINTS_GREAT + POINTS_GOOD);
        assert_eq!(session.combo, 2);
    }

    #[test]
    fn ghost_taps_do_nothing() {
        let mut session = GameSession::new(test_chart(120.0, &[(0, 1.0)]));
        assert!(session.tap(0, 0.0).is_none());
        assert!(session.tap(1, 0.5).is_none());
        assert_eq!(session.judged, 0);
        assert_eq!(session.score, 0);
        assert_eq!(session.combo, 0);
    }

    #[test]
    fn overdue_notes_become_misses_and_break_combo() {
        let mut session = GameSession::new(test_chart(120.0, &[(0, 0.0), (1, 1.0)]));
        session.tap(0, 0.0).unwrap();
        assert_eq!(session.combo, 1);
        let missed = session.advance(10.0);
        assert_eq!(missed.len(), 1);
        assert_eq!(missed[0].lane, 1);
        assert_eq!(session.combo, 0);
        assert_eq!(session.miss(), 1);
        assert_eq!(session.judged, 2);
        assert!(!session.full_combo());
        assert_eq!(session.accuracy(), 0.5);
        assert_eq!(session.final_accuracy(), 0.5);
        assert_eq!(session.rank(), Rank::D);
    }

    #[test]
    fn misses_only_happen_after_good_window() {
        let mut session = GameSession::new(test_chart(120.0, &[(0, 0.0)]));
        let t = session.notes[0].hit_time;
        assert!(session.advance(t + 0.139).is_empty());
        assert_eq!(session.judged, 0);
        assert_eq!(session.advance(t + 0.141).len(), 1);
    }

    #[test]
    fn tap_chooses_closest_pending_note() {
        let mut session = GameSession::new(test_chart(120.0, &[(0, 0.0), (0, 0.2)]));
        let earlier = session.notes[0].hit_time;
        let later = session.notes[1].hit_time;
        // Exactly between the two: the later note is 50 ms away, the earlier
        // one is also 50 ms away. Our scan keeps the first strict minimum,
        // which is the earlier note.
        let hit = session.tap(0, (earlier + later) / 2.0).unwrap();
        assert_eq!(hit.note_index, 0);
        // Near the later note, only the later one is in range.
        let hit = session.tap(0, later + 0.01).unwrap();
        assert_eq!(hit.note_index, 1);
        assert_eq!(session.combo, 2);
    }

    #[test]
    fn reset_restores_fresh_state() {
        let mut session = GameSession::new(test_chart(120.0, &[(0, 0.0), (1, 1.0)]));
        session.tap(0, 0.0).unwrap();
        session.advance(10.0);
        session.reset();
        assert_eq!(session.score, 0);
        assert_eq!(session.judged, 0);
        assert_eq!(session.combo, 0);
        assert_eq!(session.max_combo, 0);
        assert_eq!(session.counts, [0; 4]);
        assert!(session.notes.iter().all(|note| note.judgment.is_none()));
    }

    #[test]
    fn finish_requires_all_notes_and_delay() {
        let mut session = GameSession::new(test_chart(120.0, &[(0, 0.0)]));
        let t = session.notes[0].hit_time;
        session.tap(0, t).unwrap();
        assert!(!session.is_finished(t));
        assert!(session.is_finished(t + END_DELAY_SECS));
    }

    #[test]
    fn autoplay_is_perfect() {
        let chart = test_chart(120.0, &[(0, 0.0), (1, 0.5), (2, 1.0), (3, 1.5)]);
        let total = chart.notes.len();
        let mut session = GameSession::new(chart);
        session.autoplay(10.0);
        session.advance(10.0);
        assert_eq!(session.perfect() as usize, total);
        assert_eq!(session.judged, total);
        assert_eq!(session.score, session.max_score());
        assert!(session.all_perfect());
    }

    #[test]
    fn summary_contains_final_values() {
        let mut session = GameSession::new(test_chart(120.0, &[(0, 0.0), (1, 0.5)]));
        session.tap(0, 0.0).unwrap();
        session.advance(10.0);
        let summary = session.summary();
        assert_eq!(summary.total_notes, 2);
        assert_eq!(summary.score, 1_000);
        assert_eq!(summary.max_score, 2_000);
        assert_eq!(summary.accuracy, 0.5);
        assert_eq!(summary.max_combo, 1);
        assert_eq!(summary.rank, Rank::D);
        assert!(!summary.full_combo);
        assert!(!summary.all_perfect);
    }
}
