use bevy::prelude::*;

use crate::{NOTE_SIZE, TRACK_COUNT, TRACK_WIDTH, grid::GridBounds, score::Score};

#[derive(Component, Clone)]
pub struct Note {
    pub track: usize,
    pub beat: usize,
}

pub fn render_notes(score: Res<Score>, grid_bounds: Res<GridBounds>, mut gizmos: Gizmos) {
    let horizontal_step = (grid_bounds.x_max - grid_bounds.x_min) / TRACK_COUNT as f32;

    for note in &score.notes {
        // 计算音符的 x 位置，基于网格边界和 track
        let x = grid_bounds.x_min + (note.track as f32 + 0.5) * horizontal_step;
        // 音符的 y 位置直接使用 time
        let y = note.beat * TRACK_WIDTH;
        gizmos.circle(
            Vec3::new(x as f32, y as f32, 0.0), // 圆心的位置
            NOTE_SIZE as f32 / 2.,              // 圆的半径
            Color::srgb(1.0, 0.0, 0.0),         // 圆的颜色
        );
    }
}
