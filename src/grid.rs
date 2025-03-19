use bevy::prelude::*;

use crate::TRACK_COUNT;

#[derive(Resource)]
pub struct GridBounds {
    pub x_min: f32,
    pub x_max: f32,
    pub y_min: f32,
    pub y_max: f32,
}

impl GridBounds {
    pub fn new(x_min: f32, x_max: f32, y_min: f32, y_max: f32) -> Self {
        GridBounds {
            x_min,
            x_max,
            y_min,
            y_max,
        }
    }
}

pub fn render_grid(grid_bounds: Res<GridBounds>, mut gizmos: Gizmos) {
    let horizontal_step = (grid_bounds.x_max - grid_bounds.x_min) / TRACK_COUNT as f32;
    let vertical_step = (grid_bounds.y_max - grid_bounds.y_min) / 10.0;

    // 水平网格线
    for i in 0..=TRACK_COUNT {
        let x = grid_bounds.x_min + i as f32 * horizontal_step;
        gizmos.line(
            Vec3::new(x, grid_bounds.y_min, 0.0),
            Vec3::new(x, grid_bounds.y_max, 0.0),
            Color::rgba(0.5, 0.5, 0.5, 0.8), // 调整颜色和透明度
        );
    }

    // 垂直网格线
    for i in 0..=10 {
        let y = grid_bounds.y_min + i as f32 * vertical_step;
        gizmos.line(
            Vec3::new(grid_bounds.x_min, y, 0.0),
            Vec3::new(grid_bounds.x_max, y, 0.0),
            Color::rgba(0.5, 0.5, 0.5, 0.8), // 调整颜色和透明度
        );
    }
}
