//! Results screen shown after a run.

use bevy::color::Alpha;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;

use crate::chart::{format_score, format_time};
use crate::game::GameSession;
use crate::geometry::hex_vertices;
use crate::theme::{self, ACCENT, ButtonVisual, MUTED, PANEL, TEXT, text_font};
use crate::{AppState, GameOrigin};

pub struct ResultsPlugin;

impl Plugin for ResultsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Results), setup_results)
            .add_systems(
                Update,
                (results_input, results_buttons, results_background)
                    .chain()
                    .run_if(in_state(AppState::Results)),
            );
    }
}

#[derive(Component, Clone, Copy)]
enum ResultAction {
    Retry,
    Back,
}

fn setup_results(mut commands: Commands, session: Res<GameSession>, origin: Res<GameOrigin>) {
    let summary = session.summary();
    let rank = summary.rank;
    let rank_color = theme::rank_color(rank);
    let back_label = match *origin {
        GameOrigin::Editor => "BACK TO EDITOR",
        GameOrigin::Menu => "BACK TO MENU",
    };

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(4.0),
                ..default()
            },
            StateScoped(AppState::Results),
            Name::new("Results Root"),
        ))
        .with_children(|root| {
            root.spawn((Text::new("RESULTS"), text_font(14.0), TextColor(MUTED)));
            root.spawn((
                Text::new(summary.title.clone()),
                text_font(34.0),
                TextColor(TEXT),
            ));
            root.spawn((
                Text::new(format!(
                    "{}   |   {} notes   |   {}",
                    summary.difficulty,
                    summary.total_notes,
                    format_time(session.chart.chart_duration_secs())
                )),
                text_font(13.0),
                TextColor(MUTED),
            ));
            root.spawn((Node {
                height: Val::Px(2.0),
                ..default()
            },));

            root.spawn((
                Text::new(rank.letter()),
                text_font(150.0),
                TextColor(rank_color),
            ));

            if summary.all_perfect {
                root.spawn((
                    Text::new("ALL PERFECT"),
                    text_font(18.0),
                    TextColor(theme::rank_color(crate::game::Rank::S)),
                ));
            } else if summary.full_combo {
                root.spawn((Text::new("FULL COMBO"), text_font(18.0), TextColor(ACCENT)));
            } else {
                root.spawn((Node {
                    height: Val::Px(22.0),
                    ..default()
                },));
            }

            root.spawn((
                Text::new(format!(
                    "{}  /  {}",
                    format_score(summary.score),
                    format_score(summary.max_score)
                )),
                text_font(40.0),
                TextColor(TEXT),
            ));
            root.spawn((
                Text::new(format!("{:.2}%  accuracy", summary.accuracy * 100.0)),
                text_font(14.0),
                TextColor(MUTED),
            ));

            root.spawn((Node {
                height: Val::Px(12.0),
                ..default()
            },));

            root.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(26.0), Val::Px(12.0)),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(34.0),
                    ..default()
                },
                BackgroundColor(PANEL),
                BorderRadius::all(Val::Px(12.0)),
            ))
            .with_children(|panel| {
                stat(
                    panel,
                    "PERFECT",
                    summary.counts[0],
                    theme::judgment_color(crate::game::Judgment::Perfect),
                );
                stat(
                    panel,
                    "GREAT",
                    summary.counts[1],
                    theme::judgment_color(crate::game::Judgment::Great),
                );
                stat(
                    panel,
                    "GOOD",
                    summary.counts[2],
                    theme::judgment_color(crate::game::Judgment::Good),
                );
                stat(
                    panel,
                    "MISS",
                    summary.counts[3],
                    theme::judgment_color(crate::game::Judgment::Miss),
                );
                stat(panel, "MAX COMBO", summary.max_combo, ACCENT);
            });

            root.spawn((Node {
                height: Val::Px(18.0),
                ..default()
            },));

            root.spawn((Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                ..default()
            },))
                .with_children(|row| {
                    spawn_result_button(row, "RETRY", ResultAction::Retry, true);
                    spawn_result_button(row, back_label, ResultAction::Back, false);
                });

            root.spawn((
                Text::new("ENTER / R  retry      ESC  back"),
                text_font(12.0),
                TextColor(theme::FAINT),
            ));
        });
}

fn stat(parent: &mut ChildSpawnerCommands, label: &str, value: u32, color: Color) {
    parent
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        },))
        .with_children(|column| {
            column.spawn((Text::new(label), text_font(10.0), TextColor(MUTED)));
            column.spawn((
                Text::new(value.to_string()),
                text_font(24.0),
                TextColor(color),
            ));
        });
}

fn spawn_result_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: ResultAction,
    primary: bool,
) {
    let visual = if primary {
        ButtonVisual {
            normal: Color::srgba(0.05, 0.20, 0.25, 0.95),
            hover: Color::srgba(0.07, 0.30, 0.36, 0.98),
            pressed: theme::ACCENT_DIM,
        }
    } else {
        ButtonVisual::default()
    };
    parent
        .spawn((
            Button,
            action,
            visual,
            Node {
                width: Val::Px(250.0),
                height: Val::Px(48.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(visual.normal),
            BorderRadius::all(Val::Px(10.0)),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                text_font(19.0),
                TextColor(if primary { ACCENT } else { TEXT }),
            ));
        });
}

fn results_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<AppState>>,
    origin: Res<GameOrigin>,
) {
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::KeyR) {
        next.set(AppState::Play);
    } else if keys.just_pressed(KeyCode::Escape) {
        next.set(match *origin {
            GameOrigin::Editor => AppState::Editor,
            GameOrigin::Menu => AppState::Menu,
        });
    }
}

#[allow(clippy::type_complexity)]
fn results_buttons(
    buttons: Query<(&Interaction, &ResultAction), (Changed<Interaction>, With<Button>)>,
    mut next: ResMut<NextState<AppState>>,
    origin: Res<GameOrigin>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            ResultAction::Retry => next.set(AppState::Play),
            ResultAction::Back => {
                next.set(match *origin {
                    GameOrigin::Editor => AppState::Editor,
                    GameOrigin::Menu => AppState::Menu,
                });
            }
        }
    }
}

fn results_background(
    time: Res<Time<Real>>,
    session: Res<GameSession>,
    mut gizmos: Gizmos,
    mut stars: Local<Option<Vec<(Vec2, f32, f32)>>>,
) {
    let stars = stars.get_or_insert_with(|| theme::star_field(70));
    let t = time.elapsed_secs();
    for (position, size, phase) in stars.iter() {
        let alpha = 0.12 + 0.15 * (t + phase).sin().max(0.0);
        gizmos.circle_2d(*position, *size, Color::srgba(0.5, 0.7, 1.0, alpha));
    }
    let center = Vec2::new(0.0, -10.0);
    let color = theme::rank_color(session.rank());
    for (index, radius) in [170.0_f32, 260.0, 350.0].into_iter().enumerate() {
        let rotation = t * (0.03 + index as f32 * 0.01) * if index % 2 == 0 { 1.0 } else { -1.0 };
        let points = hex_vertices(center, radius)
            .map(|point| center + (point - center).rotate(Vec2::from_angle(rotation)));
        for edge in 0..6 {
            gizmos.line_2d(points[edge], points[(edge + 1) % 6], color.with_alpha(0.07));
        }
    }
}
