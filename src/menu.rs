//! Title screen.

use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

use crate::chart::format_time;
use crate::geometry::{Layout, hex_vertices};
use crate::theme::{self, ACCENT, ACCENT_DIM, ButtonVisual, MUTED, PANEL, TEXT, text_font};
use crate::{AppState, ChartSource, GameOrigin, LoadedChart};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), setup_menu)
            .add_systems(
                Update,
                (menu_input, menu_buttons, menu_background)
                    .chain()
                    .run_if(in_state(AppState::Menu)),
            );
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    Play,
    Editor,
    Quit,
}

fn setup_menu(mut commands: Commands, loaded: Res<LoadedChart>) {
    let chart = &loaded.chart;

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(8.0),
                ..default()
            },
            StateScoped(AppState::Menu),
            Name::new("Menu Root"),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("NOTATION EDITOR"),
                text_font(64.0),
                TextColor(TEXT),
            ));
            root.spawn((
                Text::new("HEXAGONAL RHYTHM GAME  /  CHART EDITOR"),
                text_font(16.0),
                TextColor(ACCENT),
            ));
            root.spawn((Node {
                height: Val::Px(18.0),
                ..default()
            },));

            root.spawn((
                Node {
                    width: Val::Px(620.0),
                    padding: UiRect::axes(Val::Px(22.0), Val::Px(16.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(3.0),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|panel| {
                panel.spawn((Text::new("NOW LOADED"), text_font(12.0), TextColor(MUTED)));
                panel.spawn((
                    Text::new(chart.title.clone()),
                    text_font(30.0),
                    TextColor(TEXT),
                ));
                panel.spawn((
                    Text::new(format!(
                        "{}   |   {}   |   {:.0} BPM",
                        chart.artist, chart.difficulty, chart.bpm
                    )),
                    text_font(13.0),
                    TextColor(MUTED),
                ));
                panel.spawn((
                    Text::new(format!(
                        "{} notes   |   {} duration",
                        chart.note_count(),
                        format_time(chart.chart_duration_secs())
                    )),
                    text_font(12.0),
                    TextColor(MUTED),
                ));
                panel.spawn((
                    Text::new(match loaded.source {
                        ChartSource::Disk => format!("{}", loaded.path.display()),
                        ChartSource::Builtin => {
                            "built-in demo (requested chart could not be loaded)".to_string()
                        }
                    }),
                    text_font(11.0),
                    TextColor(theme::FAINT),
                ));
            });

            root.spawn((Node {
                height: Val::Px(16.0),
                ..default()
            },));

            spawn_button(root, "PLAY", MenuAction::Play, true);
            spawn_button(root, "EDIT CHART", MenuAction::Editor, false);
            spawn_button(root, "QUIT", MenuAction::Quit, false);

            root.spawn((Node {
                height: Val::Px(18.0),
                ..default()
            },));
            root.spawn((
                Text::new("ENTER / SPACE  play      E  editor      Q / ESC  quit"),
                text_font(13.0),
                TextColor(MUTED),
            ));
        });
}

fn spawn_button(parent: &mut ChildSpawnerCommands, label: &str, action: MenuAction, primary: bool) {
    let visual = if primary {
        ButtonVisual {
            normal: Color::srgba(0.05, 0.20, 0.25, 0.95),
            hover: Color::srgba(0.07, 0.30, 0.36, 0.98),
            pressed: ACCENT_DIM,
        }
    } else {
        ButtonVisual::default()
    };
    let text_color = if primary { ACCENT } else { TEXT };
    parent
        .spawn((
            Button,
            action,
            visual,
            Node {
                width: Val::Px(300.0),
                height: Val::Px(52.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(visual.normal),
            BorderRadius::all(Val::Px(10.0)),
        ))
        .with_children(|button| {
            button.spawn((Text::new(label), text_font(22.0), TextColor(text_color)));
        });
}

fn menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: EventWriter<AppExit>,
    mut origin: ResMut<GameOrigin>,
) {
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
        *origin = GameOrigin::Menu;
        next.set(AppState::Play);
    } else if keys.just_pressed(KeyCode::KeyE) {
        next.set(AppState::Editor);
    } else if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
    }
}

#[allow(clippy::type_complexity)]
fn menu_buttons(
    buttons: Query<(&Interaction, &MenuAction), (Changed<Interaction>, With<Button>)>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: EventWriter<AppExit>,
    mut origin: ResMut<GameOrigin>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            MenuAction::Play => {
                *origin = GameOrigin::Menu;
                next.set(AppState::Play);
            }
            MenuAction::Editor => next.set(AppState::Editor),
            MenuAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

fn menu_background(
    time: Res<Time<Real>>,
    mut gizmos: Gizmos,
    mut stars: Local<Option<Vec<(Vec2, f32, f32)>>>,
) {
    let stars = stars.get_or_insert_with(|| theme::star_field(110));
    let t = time.elapsed_secs();
    for (position, size, phase) in stars.iter() {
        let twinkle = 0.20 + 0.25 * (t * 1.3 + *phase).sin().max(0.0);
        gizmos.circle_2d(*position, *size, Color::srgba(0.55, 0.72, 1.0, twinkle));
    }

    let center = Vec2::new(0.0, -10.0);
    let layout = Layout::default();
    for lane in 0..6u8 {
        let end = center + layout.lane_dir(lane) * 430.0;
        gizmos.line_2d(center, end, Color::srgba(0.25, 0.42, 0.62, 0.10));
    }
    for (index, radius) in [150.0_f32, 240.0, 340.0].into_iter().enumerate() {
        let rotation = t * (0.018 + index as f32 * 0.008) * if index % 2 == 0 { 1.0 } else { -1.0 };
        let points = hex_vertices(center, radius)
            .map(|point| center + (point - center).rotate(Vec2::from_angle(rotation)));
        let color = Color::srgba(0.30, 0.62, 0.90, 0.09 + index as f32 * 0.02);
        draw_closed_loop(&mut gizmos, &points, color);
    }
    let pulse = 0.5 + 0.5 * (t * 1.7).sin();
    let points = hex_vertices(center, 92.0 + pulse * 8.0);
    draw_closed_loop(&mut gizmos, &points, Color::srgba(0.35, 0.92, 1.0, 0.16));
    gizmos.circle_2d(
        center,
        5.0 + pulse * 2.0,
        Color::srgba(0.55, 1.0, 1.0, 0.25),
    );
}

fn draw_closed_loop(gizmos: &mut Gizmos, points: &[Vec2], color: Color) {
    for index in 0..points.len() {
        let next = (index + 1) % points.len();
        gizmos.line_2d(points[index], points[next], color);
    }
}
