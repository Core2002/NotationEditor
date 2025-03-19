use bevy::prelude::*;

use crate::{NOTE_SIZE, TRACK_COUNT, grid::GridBounds, score::Score};

#[derive(Component, Clone)]
pub struct Note {
    pub track: usize,
    pub time: f32,
}

pub fn render_notes(score: Res<Score>, grid_bounds: Res<GridBounds>, mut gizmos: Gizmos) {
    let horizontal_step = (grid_bounds.x_max - grid_bounds.x_min) / TRACK_COUNT as f32;

    for note in &score.notes {
        // 计算音符的 x 位置，基于网格边界和 track
        let x = grid_bounds.x_min + (note.track as f32 + 0.5) * horizontal_step;
        // 音符的 y 位置直接使用 time
        let y = note.time;
        gizmos.circle(
            Vec3::new(x, y, 0.0),       // 圆心的位置
            NOTE_SIZE / 2.0,            // 圆的半径
            Color::srgb(1.0, 0.0, 0.0), // 圆的颜色
        );
    }
}
