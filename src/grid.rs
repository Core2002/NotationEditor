use bevy::prelude::*;

use crate::TRACK_WIDTH;

#[derive(Resource)]
pub struct GridBounds {
    pub x_min: f32,
    pub x_max: f32,
    pub y_min: f32,
    pub y_max: f32,
    pub track_count: usize,
    pub bpm: usize,
    pub time_min: f32,
}

impl GridBounds {
    pub fn from_track_count_and_time(track_count: usize, bpm: usize, time_min: f32) -> Self {
        let x_min = 0.0;
        let x_max = (TRACK_WIDTH * track_count) as f32;
        let y_min = 0.0;
        let y_max = (TRACK_WIDTH * bpm) as f32 * time_min;
        GridBounds {
            x_min,
            x_max,
            y_min,
            y_max,
            track_count,
            bpm,
            time_min,
        }
    }
}

pub fn render_grid(grid_bounds: Res<GridBounds>, mut gizmos: Gizmos) {
    let horizontal_step = (grid_bounds.x_max - grid_bounds.x_min) / grid_bounds.track_count as f32;
    let vertical_step =
        (grid_bounds.y_max - grid_bounds.y_min) / (grid_bounds.bpm as f32 * grid_bounds.time_min);
    let grid_color = Color::rgba(0.5, 0.5, 0.5, 0.8); // 网格线颜色

    // 绘制网格线
    draw_grid_lines(
        &grid_bounds,
        &mut gizmos,
        grid_bounds.track_count,
        horizontal_step,
        true, // 水平线
        grid_color,
    );
    draw_grid_lines(
        &grid_bounds,
        &mut gizmos,
        grid_bounds.bpm * grid_bounds.time_min as usize,
        vertical_step,
        false, // 垂直线
        grid_color,
    );
}

fn draw_grid_lines(
    grid_bounds: &GridBounds,
    gizmos: &mut Gizmos,
    line_count: usize,
    step: f32,
    is_horizontal: bool,
    color: Color,
) {
    for i in 0..=line_count {
        let pos = grid_bounds.y_min + i as f32 * step;
        let (start, end) = if is_horizontal {
            (
                Vec3::new(grid_bounds.x_min + i as f32 * step, grid_bounds.y_min, 0.0),
                Vec3::new(grid_bounds.x_min + i as f32 * step, grid_bounds.y_max, 0.0),
            )
        } else {
            (
                Vec3::new(grid_bounds.x_min, pos, 0.0),
                Vec3::new(grid_bounds.x_max, pos, 0.0),
            )
        };
        gizmos.line(start, end, color);
    }
}
