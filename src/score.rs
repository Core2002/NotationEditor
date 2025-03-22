use bevy::prelude::*;

use crate::{NOTE_SIZE, note::Note};

#[derive(Resource)]
pub struct Score {
    pub notes: Vec<Note>,
}

impl Score {
    pub fn new() -> Self {
        Self { notes: Vec::new() }
    }

    // 实现 add_note 方法，并添加调试输出
    pub fn add_note(&mut self, note: Note) {
        self.notes.push(note.clone());
        println!("Added note: track={}, time={}", note.track, note.beat); // 调试输出
    }

    // 实现 remove_note 方法，并添加调试输出
    pub fn remove_note(&mut self, note: Note) {
        let initial_count = self.notes.len();
        self.notes.retain(|note_iter| {
            !(note_iter.track == note.track && note_iter.beat == note.beat)
        });
        let removed_count = initial_count - self.notes.len();
        if removed_count > 0 {
            println!(
                "Removed {} note(s): track={}, time={}",
                removed_count, note.track, note.beat
            ); // 调试输出
        } else {
            println!(
                "No note found to remove: track={}, time={}",
                note.track, note.beat
            ); // 调试输出
        }
    }
}
