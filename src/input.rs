use bevy::{
    ecs::{
        event::EventReader,
        system::{Query, Res, ResMut},
    },
    input::{
        ButtonInput,
        mouse::{MouseButton, MouseWheel},
    },
    math::Vec2,
    render::camera::Camera,
    transform::components::{GlobalTransform, Transform},
    window::Window,
};

use crate::{SCROLL_SPEED, TRACK_COUNT, TRACK_WIDTH, note::Note, score::Score};

pub fn handle(
    mut score: ResMut<Score>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    mut scroll_evr: EventReader<MouseWheel>,
    mut camera_query: Query<(&Camera, &mut Transform, &GlobalTransform)>,
    windows: Query<&Window>,
) {
    let (camera, mut camera_transform, global_transform) = camera_query.single_mut();
    let window = windows.single();

    handle_scroll(&mut camera_transform, &mut scroll_evr);
    handle_mouse_click(
        &mut score,
        &camera,
        global_transform,
        &mouse_button_input,
        window,
    );
}

fn handle_scroll(camera_transform: &mut Transform, scroll_evr: &mut EventReader<MouseWheel>) {
    for ev in scroll_evr.read() {
        camera_transform.translation.y += ev.y * SCROLL_SPEED;
    }
}

fn handle_mouse_click(
    score: &mut ResMut<Score>,
    camera: &Camera,
    global_transform: &GlobalTransform,
    mouse_button_input: &Res<ButtonInput<MouseButton>>,
    window: &Window,
) {
    if let Some(mouse_pos) = window.cursor_position() {
        if let Ok(ray) = camera.viewport_to_world(global_transform, mouse_pos) {
            let world_pos = Vec2::new(ray.origin.x, ray.origin.y + TRACK_WIDTH as f32 / 2.0);

            if mouse_button_input.just_pressed(MouseButton::Right) {
                remove_note(score, world_pos);
            }
            if mouse_button_input.just_pressed(MouseButton::Left) {
                add_note(score, world_pos);
            }
        }
    }
}

fn add_note(score: &mut ResMut<Score>, world_pos: Vec2) {
    let (track, beat) = calculate_track_and_bate(world_pos);
    score.add_note(Note { track, beat });
}

fn remove_note(score: &mut ResMut<Score>, world_pos: Vec2) {
    let (track, beat) = calculate_track_and_bate(world_pos);
    score.remove_note(Note { track, beat });
}

fn calculate_track_and_bate(world_pos: Vec2) -> (usize, usize) {
    let track = ((world_pos.x / TRACK_WIDTH as f32).floor() as usize).clamp(0, TRACK_COUNT - 1);
    let beat = (world_pos.y / TRACK_WIDTH as f32) as usize;

    // 加入调试信息
    println!(
        "World Position: ({}, {}), Track: {}, Bate: {}",
        world_pos.x, world_pos.y, track, beat
    );

    (track, beat)
}
