//! The playable hexagonal rhythm game screen.

use bevy::audio::AudioSink;
use bevy::color::Alpha;
use bevy::prelude::*;

use crate::audio::{ChartAudio, ClickAssets, SongAudio, play_click, spawn_song};
use crate::chart::{Chart, format_score, format_time};
use crate::cli::CliArgs;
use crate::game::{GameSession, Judgment};
use crate::geometry::{LANE_KEYS, Layout, hex_vertices};
use crate::theme::{self, ACCENT, MUTED, PANEL, TEXT, text_font};
use crate::{AppState, LoadedChart};

const LANE_KEYCODES: [KeyCode; 6] = [
    KeyCode::KeyS,
    KeyCode::KeyD,
    KeyCode::KeyF,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
];

pub struct PlayPlugin;

impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Play), setup_game)
            .add_systems(OnExit(AppState::Play), cleanup_game)
            .add_systems(
                Update,
                (
                    game_logic,
                    update_note_visuals,
                    update_lane_labels,
                    render_playfield,
                    update_fx,
                    update_hud,
                )
                    .chain()
                    .run_if(in_state(AppState::Play)),
            );
    }
}

#[derive(Resource)]
struct GameClock {
    time: f64,
    paused: bool,
    finished: bool,
}

#[derive(Resource, Default)]
struct GameAudioState {
    metronome: bool,
    last_click_beat: i64,
}

#[derive(Resource)]
struct GameFeedback {
    text: String,
    color: Color,
    timer: f32,
}

impl Default for GameFeedback {
    fn default() -> Self {
        Self {
            text: String::new(),
            color: TEXT,
            timer: 0.0,
        }
    }
}

impl GameFeedback {
    fn clear(&mut self) {
        self.text.clear();
        self.timer = 0.0;
    }

    fn show(&mut self, judgment: Judgment, delta_ms: f64) {
        self.color = theme::judgment_color(judgment);
        self.text = if judgment == Judgment::Miss {
            "MISS".to_string()
        } else if delta_ms.abs() < 1.0 {
            judgment.label().to_string()
        } else {
            format!("{}  {:+.0}ms", judgment.label(), delta_ms)
        };
        self.timer = 0.55;
    }
}

#[derive(Resource, Default)]
struct GameFx {
    bursts: Vec<FxBurst>,
}

#[derive(Clone, Copy)]
struct FxBurst {
    lane: u8,
    age: f32,
    miss: bool,
}

impl GameFx {
    fn hit(&mut self, lane: u8) {
        self.bursts.push(FxBurst {
            lane,
            age: 0.0,
            miss: false,
        });
    }

    fn miss(&mut self, lane: u8) {
        self.bursts.push(FxBurst {
            lane,
            age: 0.0,
            miss: true,
        });
    }
}

#[derive(Component)]
struct NoteVisual {
    index: usize,
}

#[derive(Component)]
struct LaneKeyLabel {
    lane: u8,
}

#[derive(Component, Clone, Copy)]
enum HudField {
    Score,
    Accuracy,
    Combo,
    Counts,
    Time,
    Feedback,
}

#[derive(Component)]
struct ProgressFill;
#[derive(Component)]
struct PauseOverlay;

fn setup_game(
    mut commands: Commands,
    loaded: Res<LoadedChart>,
    chart_audio: Option<Res<ChartAudio>>,
    ui: Res<theme::UiAssets>,
    layout: Res<Layout>,
) {
    let chart = loaded.chart.clone();
    let session = GameSession::new(chart.clone());
    let approach = chart.approach_secs();
    let song_handle = chart_audio.and_then(|audio| audio.handle.clone());

    commands.insert_resource(GameClock {
        time: 0.0,
        paused: false,
        finished: false,
    });
    commands.insert_resource(GameAudioState {
        metronome: song_handle.is_none(),
        last_click_beat: -1,
    });
    commands.insert_resource(GameFeedback::default());
    commands.insert_resource(GameFx::default());

    if let Some(handle) = &song_handle {
        spawn_song(&mut commands, handle);
    }

    // Ambient glow discs behind the playfield.
    commands.spawn((
        Sprite {
            image: ui.glow.clone(),
            color: Color::srgba(0.05, 0.16, 0.32, 0.16),
            custom_size: Some(Vec2::splat(760.0)),
            ..default()
        },
        Transform::from_xyz(layout.center.x, layout.center.y, -3.0),
        StateScoped(AppState::Play),
    ));
    commands.spawn((
        Sprite {
            image: ui.glow.clone(),
            color: Color::srgba(0.10, 0.28, 0.48, 0.30),
            custom_size: Some(Vec2::splat(250.0)),
            ..default()
        },
        Transform::from_xyz(layout.center.x, layout.center.y, -1.0),
        StateScoped(AppState::Play),
    ));

    // One sprite per note; indices match `GameSession::notes`.
    for (index, runtime) in session.notes.iter().enumerate() {
        commands.spawn((
            NoteVisual { index },
            Sprite {
                image: ui.glow.clone(),
                color: theme::lane_color(runtime.note.lane),
                custom_size: Some(Vec2::splat(layout.note_size)),
                ..default()
            },
            Transform::from_xyz(layout.center.x, layout.center.y, 3.0),
            Visibility::Hidden,
            StateScoped(AppState::Play),
        ));
    }

    // Key legends around the ring.
    for lane in 0..6u8 {
        let position = layout.judge_point(lane) + layout.lane_dir(lane) * 55.0;
        commands.spawn((
            LaneKeyLabel { lane },
            Text2d::new(LANE_KEYS[lane as usize]),
            TextFont::from_font_size(19.0),
            TextColor(theme::lane_color(lane).with_alpha(0.78)),
            Transform::from_xyz(position.x, position.y, 4.0),
            StateScoped(AppState::Play),
        ));
    }

    spawn_hud(&mut commands, &chart);

    info!(
        "started '{}' ({} notes, approach {:.2}s, audio: {})",
        chart.title,
        session.notes.len(),
        approach,
        if song_handle.is_some() {
            "song"
        } else {
            "metronome"
        }
    );
    commands.insert_resource(session);
}

fn cleanup_game(mut commands: Commands) {
    commands.remove_resource::<GameClock>();
    commands.remove_resource::<GameAudioState>();
    commands.remove_resource::<GameFeedback>();
    commands.remove_resource::<GameFx>();
}

#[allow(clippy::too_many_arguments)]
fn game_logic(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut clock: ResMut<GameClock>,
    mut session: ResMut<GameSession>,
    mut fx: ResMut<GameFx>,
    mut feedback: ResMut<GameFeedback>,
    mut audio_state: ResMut<GameAudioState>,
    mut next: ResMut<NextState<AppState>>,
    mut commands: Commands,
    clicks: Res<ClickAssets>,
    chart_audio: Option<Res<ChartAudio>>,
    cli: Res<CliArgs>,
    song_entities: Query<Entity, With<SongAudio>>,
    sinks: Query<&AudioSink, With<SongAudio>>,
) {
    if clock.finished {
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        clock.paused = !clock.paused;
        if let Ok(sink) = sinks.single() {
            if clock.paused {
                sink.pause();
            } else {
                sink.play();
            }
        }
    }

    if clock.paused {
        if let Ok(sink) = sinks.single() {
            sink.pause();
        }
        if keys.just_pressed(KeyCode::KeyR) {
            restart_game(
                &mut commands,
                &mut session,
                &mut clock,
                &mut fx,
                &mut feedback,
                &mut audio_state,
                chart_audio.as_deref(),
                &song_entities,
            );
        } else if keys.just_pressed(KeyCode::KeyQ) {
            next.set(AppState::Menu);
        }
        return;
    }

    clock.time += time.delta_secs_f64();

    if cli.autoplay {
        for hit in session.autoplay(clock.time) {
            fx.hit(hit.lane);
            feedback.show(hit.judgment, hit.delta_ms);
        }
    }

    // Taps are resolved before automatic misses so a press on the same frame as
    // a note leaving its window is still credited.
    for (lane, key) in LANE_KEYCODES.iter().enumerate() {
        if keys.just_pressed(*key) {
            if let Some(hit) = session.tap(lane as u8, clock.time) {
                fx.hit(lane as u8);
                feedback.show(hit.judgment, hit.delta_ms);
            }
        }
    }

    for miss in session.advance(clock.time) {
        fx.miss(miss.lane);
        feedback.show(Judgment::Miss, 0.0);
    }

    if audio_state.metronome {
        let current_beat = session.chart.beat_from_seconds(clock.time).floor() as i64;
        while audio_state.last_click_beat < current_beat {
            audio_state.last_click_beat += 1;
            if audio_state.last_click_beat >= 0 {
                play_click(
                    &mut commands,
                    &clicks,
                    audio_state.last_click_beat % 4 == 0,
                    0.22,
                );
            }
        }
    }

    if let Ok(sink) = sinks.single() {
        // No-op when already playing; covers the first frame after setup.
        sink.play();
    }

    if session.is_finished(clock.time) {
        clock.finished = true;
        next.set(AppState::Results);
    }
}

#[allow(clippy::too_many_arguments)]
fn restart_game(
    commands: &mut Commands,
    session: &mut GameSession,
    clock: &mut GameClock,
    fx: &mut GameFx,
    feedback: &mut GameFeedback,
    audio_state: &mut GameAudioState,
    chart_audio: Option<&ChartAudio>,
    song_entities: &Query<Entity, With<SongAudio>>,
) {
    for entity in song_entities.iter() {
        commands.entity(entity).despawn();
    }
    if let Some(handle) = chart_audio.and_then(|audio| audio.handle.as_ref()) {
        spawn_song(commands, handle);
    }
    session.reset();
    clock.time = 0.0;
    clock.paused = false;
    clock.finished = false;
    fx.bursts.clear();
    feedback.clear();
    audio_state.last_click_beat = -1;
}

fn update_note_visuals(
    session: Res<GameSession>,
    clock: Res<GameClock>,
    layout: Res<Layout>,
    mut notes: Query<(&NoteVisual, &mut Transform, &mut Sprite, &mut Visibility)>,
) {
    let approach = session.chart.approach_secs();
    for (visual, mut transform, mut sprite, mut visibility) in &mut notes {
        let Some(runtime) = session.notes.get(visual.index) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        if runtime.judgment.is_some() {
            *visibility = Visibility::Hidden;
            continue;
        }
        match layout.note_motion(runtime.note.lane, runtime.hit_time, clock.time, approach) {
            Some(motion) => {
                *visibility = Visibility::Visible;
                transform.translation = motion.position.extend(3.0);
                transform.scale = Vec3::splat(motion.scale);
                sprite.color = theme::lane_color(runtime.note.lane).with_alpha(motion.alpha);
            }
            None => *visibility = Visibility::Hidden,
        }
    }
}

fn update_lane_labels(
    keys: Res<ButtonInput<KeyCode>>,
    mut labels: Query<(&LaneKeyLabel, &mut TextColor)>,
) {
    for (label, mut color) in &mut labels {
        let pressed = keys.pressed(LANE_KEYCODES[label.lane as usize]);
        color.0 = theme::lane_color(label.lane).with_alpha(if pressed { 1.0 } else { 0.62 });
    }
}

fn render_playfield(
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<GameSession>,
    clock: Res<GameClock>,
    layout: Res<Layout>,
    mut gizmos: Gizmos,
) {
    let center = layout.center;

    // Outer ring.
    let outer = layout.outer_hex();
    draw_closed_loop(&mut gizmos, &outer, Color::srgba(0.35, 0.62, 0.90, 0.42));
    // Faint concentric decoration.
    for (scale, alpha) in [(1.10, 0.10), (1.22, 0.05)] {
        let points = hex_vertices(
            center,
            layout.spawn_radius / 30.0_f32.to_radians().cos() * scale,
        );
        draw_closed_loop(&mut gizmos, &points, Color::srgba(0.25, 0.50, 0.80, alpha));
    }

    // Lane rails and target rings.
    for lane in 0..6u8 {
        let pressed = keys.pressed(LANE_KEYCODES[lane as usize]);
        let color = theme::lane_color(lane);
        let start = layout.judge_point(lane);
        let end = layout.spawn_point(lane);
        gizmos.line_2d(
            start,
            end,
            color.with_alpha(if pressed { 0.55 } else { 0.13 }),
        );
        let target_radius = if pressed { 17.0 } else { 12.0 };
        gizmos.circle_2d(
            start,
            target_radius,
            color.with_alpha(if pressed { 0.95 } else { 0.40 }),
        );
        let ticks = 4;
        for tick in 1..=ticks {
            let r = layout.judge_radius
                + (layout.spawn_radius - layout.judge_radius) * tick as f32 / ticks as f32;
            let p = layout.lane_point(lane, r);
            let dir = layout.lane_dir(lane);
            let normal = Vec2::new(-dir.y, dir.x);
            gizmos.line_2d(p - normal * 2.0, p + normal * 2.0, color.with_alpha(0.10));
        }
    }

    // Judgement hexagon.
    let judge = layout.judge_hex();
    draw_closed_loop(&mut gizmos, &judge, Color::srgba(0.55, 0.95, 1.0, 0.65));

    // Beat pulse travelling through the judge ring.
    let beat = session.chart.beat_from_seconds(clock.time);
    let phase = beat.rem_euclid(1.0) as f32;
    let pulse = 1.0 - phase;
    let ring = layout
        .judge_hex()
        .map(|point| center + (point - center) * (1.0 + pulse * 0.08));
    draw_closed_loop(
        &mut gizmos,
        &ring,
        Color::srgba(0.55, 0.95, 1.0, 0.10 + 0.20 * pulse),
    );

    // Small decorative dot at the centre.
    gizmos.circle_2d(center, 4.0 + pulse * 2.0, ACCENT.with_alpha(0.35));
}

fn update_fx(
    time: Res<Time<Real>>,
    mut fx: ResMut<GameFx>,
    layout: Res<Layout>,
    mut gizmos: Gizmos,
) {
    let dt = time.delta_secs();
    for burst in &mut fx.bursts {
        burst.age += dt;
        let t = (burst.age / 0.45).clamp(0.0, 1.0);
        let fade = (1.0 - t).powi(2);
        let position = layout.judge_point(burst.lane);
        if burst.miss {
            let color = theme::DANGER.with_alpha(fade * 0.8);
            let size = 10.0 + t * 26.0;
            gizmos.line_2d(
                position + Vec2::new(-size, -size),
                position + Vec2::new(size, size),
                color,
            );
            gizmos.line_2d(
                position + Vec2::new(-size, size),
                position + Vec2::new(size, -size),
                color,
            );
        } else {
            let color = theme::lane_color(burst.lane);
            gizmos.circle_2d(position, 12.0 + t * 60.0, color.with_alpha(fade * 0.7));
            for index in 0..6 {
                let angle = std::f32::consts::TAU * index as f32 / 6.0 + burst.lane as f32 * 0.5;
                let dir = Vec2::new(angle.cos(), angle.sin());
                gizmos.line_2d(
                    position + dir * (14.0 + t * 34.0),
                    position + dir * (22.0 + t * 62.0),
                    color.with_alpha(fade),
                );
            }
        }
    }
    fx.bursts.retain(|burst| burst.age < 0.45);
}

fn update_hud(
    time: Res<Time<Real>>,
    clock: Res<GameClock>,
    session: Res<GameSession>,
    mut feedback: ResMut<GameFeedback>,
    mut fields: Query<(&HudField, &mut Text, &mut TextColor)>,
    mut progress_fill: Query<&mut Node, With<ProgressFill>>,
    mut overlay: Query<&mut Visibility, With<PauseOverlay>>,
) {
    for (field, mut text, mut color) in &mut fields {
        match field {
            HudField::Score => text.0 = format_score(session.score),
            HudField::Accuracy => {
                text.0 = format!("{:.2}%", session.accuracy() * 100.0);
            }
            HudField::Combo => {
                if session.combo >= 2 {
                    text.0 = session.combo.to_string();
                    color.0 = if session.combo >= 20 { ACCENT } else { TEXT };
                } else {
                    text.0 = "-".to_string();
                    color.0 = MUTED;
                }
            }
            HudField::Counts => {
                text.0 = format!(
                    "PERFECT {:>3}   GREAT {:>3}\nGOOD {:>3}   MISS {:>3}",
                    session.perfect(),
                    session.great(),
                    session.good(),
                    session.miss()
                );
            }
            HudField::Time => {
                text.0 = format!(
                    "{}  /  {}",
                    format_time(clock.time),
                    format_time(session.chart.chart_duration_secs())
                );
            }
            HudField::Feedback => {
                if feedback.timer > 0.0 {
                    feedback.timer = (feedback.timer - time.delta_secs()).max(0.0);
                    let alpha = (feedback.timer / 0.55).clamp(0.0, 1.0);
                    text.0 = feedback.text.clone();
                    color.0 = feedback.color.with_alpha(alpha);
                } else {
                    text.0.clear();
                }
            }
        }
    }
    if let Ok(mut node) = progress_fill.single_mut() {
        node.width = Val::Percent(session.progress().clamp(0.0, 1.0) * 100.0);
    }
    if let Ok(mut visibility) = overlay.single_mut() {
        *visibility = if clock.paused {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

fn spawn_hud(commands: &mut Commands, chart: &Chart) {
    // Score panel, top left.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(20.0),
                left: Val::Px(24.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderRadius::all(Val::Px(12.0)),
            StateScoped(AppState::Play),
        ))
        .with_children(|panel| {
            panel.spawn((Text::new("SCORE"), text_font(11.0), TextColor(MUTED)));
            panel.spawn((
                Text::new("0"),
                text_font(32.0),
                TextColor(TEXT),
                HudField::Score,
            ));
            panel.spawn((Text::new("ACCURACY"), text_font(11.0), TextColor(MUTED)));
            panel.spawn((
                Text::new("100.00%"),
                text_font(18.0),
                TextColor(ACCENT),
                HudField::Accuracy,
            ));
        });

    // Judgement counts, top right.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(20.0),
                right: Val::Px(24.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderRadius::all(Val::Px(12.0)),
            StateScoped(AppState::Play),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new(chart.title.clone()),
                text_font(13.0),
                TextColor(MUTED),
            ));
            panel.spawn((
                Text::new("PERFECT   0   GREAT   0\nGOOD   0   MISS   0"),
                text_font(13.0),
                TextColor(TEXT),
                HudField::Counts,
            ));
            panel.spawn((
                Text::new("00:00.00  /  00:00.00"),
                text_font(11.0),
                TextColor(MUTED),
                HudField::Time,
            ));
        });

    // Combo, top centre.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(24.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            StateScoped(AppState::Play),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("-"),
                text_font(50.0),
                TextColor(MUTED),
                HudField::Combo,
            ));
            panel.spawn((Text::new("COMBO"), text_font(11.0), TextColor(MUTED)));
        });

    // Judgement feedback.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(150.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            StateScoped(AppState::Play),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new(""),
                text_font(30.0),
                TextColor(TEXT),
                HudField::Feedback,
            ));
        });

    // Progress bar.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(26.0),
                right: Val::Px(26.0),
                bottom: Val::Px(26.0),
                height: Val::Px(7.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.10, 0.13, 0.20, 0.95)),
            BorderRadius::all(Val::Px(4.0)),
            StateScoped(AppState::Play),
        ))
        .with_children(|bar| {
            bar.spawn((
                Node {
                    width: Val::Percent(0.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(ACCENT),
                BorderRadius::all(Val::Px(4.0)),
                ProgressFill,
            ));
        });

    // Pause overlay.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.01, 0.03, 0.72)),
            Visibility::Hidden,
            PauseOverlay,
            StateScoped(AppState::Play),
        ))
        .with_children(|overlay| {
            overlay.spawn((Text::new("PAUSED"), text_font(64.0), TextColor(TEXT)));
            overlay.spawn((
                Text::new("ESC  resume      R  restart      Q  menu"),
                text_font(15.0),
                TextColor(MUTED),
            ));
            overlay.spawn((
                Text::new(format!(
                    "{}   |   {}   |   {:.0} BPM",
                    chart.title, chart.difficulty, chart.bpm
                )),
                text_font(13.0),
                TextColor(theme::FAINT),
            ));
        });
}

fn draw_closed_loop(gizmos: &mut Gizmos, points: &[Vec2], color: Color) {
    for index in 0..points.len() {
        gizmos.line_2d(points[index], points[(index + 1) % points.len()], color);
    }
}
