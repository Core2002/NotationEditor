mod grid;
mod input;
mod note;
mod score;

use bevy::prelude::*;
use grid::GridBounds;
use score::Score;

const TRACK_COUNT: usize = 6;
const NOTE_SIZE: f32 = 40.0;
const SCROLL_SPEED: f32 = 20.0;

const TRACK_WIDTH: f32 = NOTE_SIZE * 2.0; // 每个轨道的宽度

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Notation Editor".into(),
                resolution: (800.0, 600.0).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(Score::new())
        .insert_resource(GridBounds::new(
            0.0,
            TRACK_WIDTH * TRACK_COUNT as f32,
            0.0,
            TRACK_WIDTH * TRACK_COUNT as f32,
        ))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (input::handle, note::render_notes, grid::render_grid),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera2d::default(),
        Transform::from_xyz(TRACK_WIDTH * TRACK_COUNT as f32 / 2.0, 0.0, 0.0),
        GlobalTransform::default(),
    ));
}
