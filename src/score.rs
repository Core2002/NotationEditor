use bevy::prelude::*;

use crate::NOTE_SIZE;

// 定义 Note 结构体
#[derive(Debug, Clone)]
pub struct Note {
    pub track: usize,
    pub time: f32,
}

#[derive(Resource)]
pub struct Score {
    pub notes: Vec<Note>,
}

impl Score {
    pub fn new() -> Self {
        Self { notes: Vec::new() }
    }

    // 实现 add_note 方法，并添加调试输出
    pub fn add_note(&mut self, track: usize, time: f32) {
        let note = Note { track, time };
        self.notes.push(note);
        println!("Added note: track={}, time={}", track, time); // 调试输出
    }

    // 实现 remove_note 方法，并添加调试输出
    pub fn remove_note(&mut self, track: usize, time: f32) {
        let initial_count = self.notes.len();
        self.notes
            .retain(|note| !(note.track == track && ((note.time - time).abs() < NOTE_SIZE)));
        let removed_count = initial_count - self.notes.len();
        if removed_count > 0 {
            println!(
                "Removed {} note(s): track={}, time={}",
                removed_count, track, time
            ); // 调试输出
        } else {
            println!("No note found to remove: track={}, time={}", track, time); // 调试输出
        }
    }
}
