use bevy::{
    core_pipeline::core_2d::Camera2d,
    ecs::{
        query::With,
        system::{Query, Res},
    },
    input::{ButtonInput, keyboard::KeyCode},
    log::info,
    transform::components::Transform,
};

// This system prints when `Ctrl + Shift + A` is pressed
pub fn camera_control(
    input: Res<ButtonInput<KeyCode>>,
    mut camera_query: Query<&mut Transform, With<Camera2d>>,
) {
    let mut transform = camera_query.single_mut().unwrap();

    let shift = input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);

    if ctrl && shift && input.just_pressed(KeyCode::KeyA) {
        info!("Just pressed Ctrl + Shift + A!");
        transform.scale *= 1.1;
    }

    if ctrl && shift && input.just_pressed(KeyCode::KeyB) {
        info!("Just pressed Ctrl + Shift + B!");
        transform.scale /= 1.1;
    }
}
