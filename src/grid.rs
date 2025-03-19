use bevy::prelude::*;

use crate::{NOTE_SIZE, TRACK_COUNT};

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
    let vertical_step = (grid_bounds.y_max - grid_bounds.y_min) / TRACK_COUNT as f32;
    let grid_color = Color::rgba(0.5, 0.5, 0.5, 0.8); // 网格线颜色

    // 绘制网格线
    draw_grid_lines(
        &grid_bounds,
        &mut gizmos,
        TRACK_COUNT,
        horizontal_step,
        true, // 水平线
        grid_color,
    );
    draw_grid_lines(
        &grid_bounds,
        &mut gizmos,
        10,
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
