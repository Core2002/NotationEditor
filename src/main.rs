mod debug_tools;
mod grid;
mod input;
mod note;
mod score;
mod time_domain_graph;

use bevy::prelude::*;
use grid::GridBounds;
use score::Score;
use time_domain_graph::{TimeDomainGraph, decode_mp3};

const TRACK_COUNT: usize = 6;
const NOTE_SIZE: usize = 40;
const SCROLL_SPEED: f32 = 80.0;

const BPM: usize = 60;

const TRACK_WIDTH: usize = NOTE_SIZE * 2; // 每个轨道的宽度

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(Score::new())
        .insert_resource(GridBounds::from_track_count_and_time(TRACK_COUNT, BPM, 1.0))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                input::handle,
                note::render_notes,
                grid::render_grid,
                time_domain_graph::render_graph,
                debug_tools::camera_control,
            ),
        )
        .run();
}

fn setup(mut commands: Commands) {
    let samples = decode_mp3("b.wav");

    commands.spawn((
        Camera2d::default(),
        Transform::from_xyz((TRACK_WIDTH * TRACK_COUNT) as f32 / 2.0, 0.0, 0.0),
        GlobalTransform::default(),
        TimeDomainGraph { samples },
    ));
}
