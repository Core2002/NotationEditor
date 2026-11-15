//! Chart editor: scrolling six-lane timeline, live hex preview, save and
//! playtest loop.

use std::path::PathBuf;
use std::time::Instant;

use bevy::color::Alpha;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::audio::{ClickAssets, audio_disk_path, play_click};
use crate::chart::{Chart, LANE_COUNT, format_time, snap_beat};
use crate::geometry::{Layout, hex_vertices};
use crate::theme::{self, ACCENT, DANGER, MUTED, PANEL, TEXT, WARNING, text_font};
use crate::waveform::{Waveform, load_waveform};
use crate::{AppState, ChartSource, GameOrigin, LoadedChart};

const TL_LEFT: f32 = -620.0;
const TL_RIGHT: f32 = 240.0;
const TL_TOP: f32 = 330.0;
const TL_BOTTOM: f32 = -330.0;
const PLAYHEAD_Y: f32 = -190.0;
const LANE_WIDTH: f32 = (TL_RIGHT - TL_LEFT) / LANE_COUNT as f32;
const PREVIEW_CENTER: Vec2 = Vec2::new(455.0, 10.0);
const DEFAULT_ZOOM: f32 = 80.0;
const MIN_ZOOM: f32 = 45.0;
const MAX_ZOOM: f32 = 220.0;
const SNAP_OPTIONS: [u32; 8] = [1, 2, 3, 4, 6, 8, 12, 16];
const LABEL_POOL: usize = 24;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Editor), setup_editor)
            .add_systems(OnExit(AppState::Editor), cleanup_editor)
            .add_systems(
                Update,
                (
                    editor_input,
                    editor_advance,
                    update_timeline_labels,
                    render_editor,
                    update_editor_ui,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

#[derive(Resource)]
struct EditorSession {
    chart: Chart,
    path: PathBuf,
    playhead: f64,
    playing: bool,
    snap: u32,
    zoom: f32,
    dirty: bool,
    quit_armed: bool,
    status: String,
    status_color: Color,
    waveform: Option<Waveform>,
    last_click_beat: i64,
    metronome: bool,
}

impl EditorSession {
    fn timeline_end(&self) -> f64 {
        self.chart.editor_end_beat()
    }

    fn set_status(&mut self, text: impl Into<String>, color: Color) {
        self.status = text.into();
        self.status_color = color;
    }

    fn seek(&mut self, beat: f64) {
        self.playhead = beat.clamp(0.0, self.timeline_end());
        self.playing = false;
        self.quit_armed = false;
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum EditorField {
    Title,
    Path,
    Info,
    Playhead,
    Status,
}

#[derive(Component)]
struct TimelineLabel {
    slot: usize,
}

fn setup_editor(mut commands: Commands, loaded: Res<LoadedChart>, ui: Res<theme::UiAssets>) {
    let chart = loaded.chart.clone();
    let path = loaded.path.clone();
    let waveform = load_waveform_for_chart(&chart);
    let start_status = match &loaded.source {
        ChartSource::Disk => "Ready   |   Ctrl+S save   |   ENTER playtest".to_string(),
        ChartSource::Builtin => {
            "Ready (built-in demo)   |   Ctrl+S save   |   ENTER playtest".to_string()
        }
    };

    commands.insert_resource(EditorSession {
        chart,
        path,
        playhead: 0.0,
        playing: false,
        snap: 4,
        zoom: DEFAULT_ZOOM,
        dirty: false,
        quit_armed: false,
        status: start_status,
        status_color: MUTED,
        waveform,
        last_click_beat: -1,
        metronome: true,
    });

    // World-space panels behind the gizmo drawing.
    spawn_panel(
        &mut commands,
        &ui,
        Vec2::new((TL_LEFT + TL_RIGHT) * 0.5, 0.0),
        Vec2::new(TL_RIGHT - TL_LEFT, TL_TOP - TL_BOTTOM),
        Color::srgba(0.020, 0.029, 0.052, 0.94),
    );
    spawn_panel(
        &mut commands,
        &ui,
        Vec2::new(PREVIEW_CENTER.x, 15.0),
        Vec2::new(330.0, 620.0),
        Color::srgba(0.026, 0.038, 0.068, 0.94),
    );
    spawn_panel(
        &mut commands,
        &ui,
        Vec2::new((TL_LEFT + TL_RIGHT) * 0.5, TL_TOP + 2.0),
        Vec2::new(TL_RIGHT - TL_LEFT, 2.0),
        Color::srgba(0.25, 0.72, 0.95, 0.25),
    );

    // Pool of measure labels.
    for slot in 0..LABEL_POOL {
        commands.spawn((
            TimelineLabel { slot },
            Text2d::new(""),
            TextFont::from_font_size(11.0),
            TextColor(MUTED),
            Anchor::CenterLeft,
            Transform::from_xyz(TL_LEFT + 8.0, 0.0, 2.0),
            Visibility::Hidden,
            StateScoped(AppState::Editor),
        ));
    }
    commands.spawn((
        Text2d::new("TIMELINE"),
        TextFont::from_font_size(11.0),
        TextColor(theme::FAINT),
        Anchor::CenterLeft,
        Transform::from_xyz(TL_LEFT + 12.0, TL_TOP - 56.0, 2.0),
        StateScoped(AppState::Editor),
    ));
    commands.spawn((
        Text2d::new("PLAYFIELD PREVIEW"),
        TextFont::from_font_size(11.0),
        TextColor(theme::FAINT),
        Transform::from_xyz(PREVIEW_CENTER.x, TL_TOP - 56.0, 2.0),
        StateScoped(AppState::Editor),
    ));

    spawn_editor_ui(&mut commands);
}

fn load_waveform_for_chart(chart: &Chart) -> Option<Waveform> {
    let audio = chart.audio.as_deref()?;
    let path = audio_disk_path(audio);
    if !path.is_file() {
        return None;
    }
    let start = Instant::now();
    match load_waveform(&path, 2048) {
        Ok(waveform) => {
            info!(
                "decoded waveform for {} in {:?} ({:.2}s)",
                path.display(),
                start.elapsed(),
                waveform.duration_secs
            );
            Some(waveform)
        }
        Err(err) => {
            warn!("could not decode waveform for {}: {err}", path.display());
            None
        }
    }
}

fn spawn_panel(
    commands: &mut Commands,
    ui: &theme::UiAssets,
    center: Vec2,
    size: Vec2,
    color: Color,
) {
    commands.spawn((
        Sprite {
            image: ui.white.clone(),
            color,
            custom_size: Some(size),
            ..default()
        },
        Transform::from_xyz(center.x, center.y, -10.0),
        StateScoped(AppState::Editor),
    ));
}

fn spawn_editor_ui(commands: &mut Commands) {
    // Opaque header strip so the timeline does not show through the labels.
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            height: Val::Px(78.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.010, 0.016, 0.030, 0.94)),
        StateScoped(AppState::Editor),
    ));

    // Title panel, top left.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                left: Val::Px(20.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                ..default()
            },
            StateScoped(AppState::Editor),
        ))
        .with_children(|panel| {
            panel.spawn((Text::new("CHART EDITOR"), text_font(10.0), TextColor(MUTED)));
            panel.spawn((
                Text::new(""),
                text_font(23.0),
                TextColor(TEXT),
                EditorField::Title,
            ));
            panel.spawn((
                Text::new(""),
                text_font(10.0),
                TextColor(theme::FAINT),
                EditorField::Path,
            ));
        });

    // Playhead / chart info, top right.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                right: Val::Px(20.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(1.0),
                ..default()
            },
            StateScoped(AppState::Editor),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new(""),
                text_font(14.0),
                TextColor(TEXT),
                EditorField::Playhead,
            ));
            panel.spawn((
                Text::new(""),
                text_font(11.0),
                TextColor(MUTED),
                EditorField::Info,
            ));
        });

    // Help bar.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(20.0),
                right: Val::Px(20.0),
                bottom: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderRadius::all(Val::Px(9.0)),
            StateScoped(AppState::Editor),
        ))
        .with_children(|bar| {
            bar.spawn((
                Text::new(
                    "SPACE play/pause    |    LEFT/RIGHT seek    |    WHEEL scroll    |    CTRL+WHEEL zoom    |    [ ] snap    |    -/= BPM    |    ,/. offset    |    M metro",
                ),
                text_font(11.0),
                TextColor(MUTED),
            ));
            bar.spawn((
                Text::new(
                    "LEFT CLICK add note    |    RIGHT CLICK remove note    |    CTRL+S save    |    ENTER save + playtest    |    ESC menu",
                ),
                text_font(11.0),
                TextColor(MUTED),
            ));
        });

    // Floating status toast.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(72.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            StateScoped(AppState::Editor),
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(""),
                text_font(15.0),
                TextColor(MUTED),
                EditorField::Status,
            ));
        });
}

fn cleanup_editor(mut commands: Commands) {
    commands.remove_resource::<EditorSession>();
}

#[allow(clippy::too_many_arguments)]
fn editor_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: EventReader<MouseWheel>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut session: ResMut<EditorSession>,
    mut loaded: ResMut<LoadedChart>,
    mut origin: ResMut<GameOrigin>,
    mut next: ResMut<NextState<AppState>>,
) {
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);

    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        save_editor(&mut session, &mut loaded);
        return;
    }

    if keys.just_pressed(KeyCode::Enter) {
        if save_editor(&mut session, &mut loaded) {
            loaded.chart = session.chart.clone();
            loaded.path = session.path.clone();
            *origin = GameOrigin::Editor;
            next.set(AppState::Play);
        }
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        if session.dirty && !session.quit_armed {
            session.quit_armed = true;
            session.set_status("Unsaved changes - press ESC again to discard them", WARNING);
        } else {
            next.set(AppState::Menu);
        }
        return;
    }

    if keys.just_pressed(KeyCode::Space) {
        session.playing = !session.playing;
        session.quit_armed = false;
        if session.playing {
            let end = session.timeline_end();
            if session.playhead >= end - 0.01 {
                session.playhead = 0.0;
            }
            session.last_click_beat = session.playhead.floor() as i64;
            session.set_status("Preview playing", ACCENT);
        } else {
            session.set_status("Preview paused", MUTED);
        }
    }

    let seek_step = if shift { 4.0 } else { 1.0 };
    if keys.just_pressed(KeyCode::ArrowLeft) {
        let target = session.playhead - seek_step;
        session.seek(target);
        session.set_status("Seek", MUTED);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        let target = session.playhead + seek_step;
        session.seek(target);
        session.set_status("Seek", MUTED);
    }
    if keys.just_pressed(KeyCode::Home) {
        session.seek(0.0);
    }
    if keys.just_pressed(KeyCode::End) {
        let end = session.timeline_end();
        session.seek(end);
    }
    if keys.just_pressed(KeyCode::PageUp) {
        let target = session.playhead + 4.0;
        session.seek(target);
    }
    if keys.just_pressed(KeyCode::PageDown) {
        let target = session.playhead - 4.0;
        session.seek(target);
    }

    if keys.just_pressed(KeyCode::BracketLeft) {
        let index = SNAP_OPTIONS
            .iter()
            .position(|value| *value == session.snap)
            .unwrap_or(3);
        session.snap = SNAP_OPTIONS[index.saturating_sub(1)];
        session.quit_armed = false;
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        let index = SNAP_OPTIONS
            .iter()
            .position(|value| *value == session.snap)
            .unwrap_or(3);
        session.snap = SNAP_OPTIONS[(index + 1).min(SNAP_OPTIONS.len() - 1)];
        session.quit_armed = false;
    }

    let bump = if shift { 10.0 } else { 1.0 };
    if keys.just_pressed(KeyCode::Minus) {
        let bpm = (session.chart.bpm - bump).clamp(40.0, 300.0);
        session.chart.bpm = bpm;
        session.dirty = true;
        session.quit_armed = false;
        session.set_status(format!("BPM set to {bpm:.0}"), TEXT);
    }
    if keys.just_pressed(KeyCode::Equal) {
        let bpm = (session.chart.bpm + bump).clamp(40.0, 300.0);
        session.chart.bpm = bpm;
        session.dirty = true;
        session.quit_armed = false;
        session.set_status(format!("BPM set to {bpm:.0}"), TEXT);
    }
    let offset_bump = if shift { 100.0 } else { 10.0 };
    if keys.just_pressed(KeyCode::Comma) {
        let offset = (session.chart.offset_ms - offset_bump).max(0.0);
        session.chart.offset_ms = offset;
        session.dirty = true;
        session.quit_armed = false;
        session.set_status(format!("Offset set to {offset:.0} ms"), TEXT);
    }
    if keys.just_pressed(KeyCode::Period) {
        let offset = (session.chart.offset_ms + offset_bump).min(60_000.0);
        session.chart.offset_ms = offset;
        session.dirty = true;
        session.quit_armed = false;
        session.set_status(format!("Offset set to {offset:.0} ms"), TEXT);
    }
    if keys.just_pressed(KeyCode::KeyM) {
        let enabled = !session.metronome;
        session.metronome = enabled;
        session.set_status(
            if enabled {
                "Metronome on"
            } else {
                "Metronome off"
            },
            MUTED,
        );
    }

    // Mouse wheel: scroll or zoom.
    for event in wheel.read() {
        if ctrl {
            let factor = 1.0 + event.y * 0.09;
            let zoom = (session.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
            session.zoom = zoom;
            session.set_status(format!("Zoom {zoom:.0} px/beat"), MUTED);
        } else {
            let step = if shift { 0.25 } else { 0.75 };
            let target = session.playhead + event.y as f64 * step;
            session.seek(target);
        }
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let Ok(world) = camera.viewport_to_world_2d(camera_transform, cursor) else {
        return;
    };
    if !in_timeline(world) {
        return;
    }
    let Some((lane, beat)) = timeline_hit(world, &session) else {
        return;
    };

    if mouse.just_pressed(MouseButton::Left) {
        session.quit_armed = false;
        if session.chart.add_note(lane, beat) {
            session.dirty = true;
            session.set_status(
                format!("Added note  lane {}  beat {:.2}", lane, beat),
                ACCENT,
            );
        } else {
            session.set_status("A note already exists there", WARNING);
        }
    }
    if mouse.just_pressed(MouseButton::Right) {
        session.quit_armed = false;
        let tolerance = 0.45 / session.snap as f64;
        if session.chart.remove_note_near(lane, beat, tolerance) {
            session.dirty = true;
            session.set_status(
                format!("Removed note  lane {}  beat {:.2}", lane, beat),
                MUTED,
            );
        } else {
            session.set_status("No note close enough to remove", WARNING);
        }
    }
}

fn save_editor(session: &mut EditorSession, loaded: &mut LoadedChart) -> bool {
    match session.chart.save(&session.path) {
        Ok(()) => {
            session.dirty = false;
            session.quit_armed = false;
            session.set_status(format!("Saved {}", session.path.display()), theme::SUCCESS);
            loaded.chart = session.chart.clone();
            loaded.path = session.path.clone();
            loaded.source = ChartSource::Disk;
            true
        }
        Err(err) => {
            session.set_status(format!("Save failed: {err}"), DANGER);
            false
        }
    }
}

fn in_timeline(world: Vec2) -> bool {
    (TL_LEFT..=TL_RIGHT).contains(&world.x) && (TL_BOTTOM..=TL_TOP).contains(&world.y)
}

fn timeline_hit(world: Vec2, session: &EditorSession) -> Option<(u8, f64)> {
    if !in_timeline(world) {
        return None;
    }
    let lane = ((world.x - TL_LEFT) / LANE_WIDTH).floor();
    if !(0.0..LANE_COUNT as f32).contains(&lane) {
        return None;
    }
    let raw_beat = session.playhead + (world.y - PLAYHEAD_Y) as f64 / session.zoom as f64;
    if raw_beat < 0.0 {
        return None;
    }
    let snapped = snap_beat(raw_beat, session.snap).max(0.0);
    if snapped > session.timeline_end() + 0.001 {
        return None;
    }
    Some((lane as u8, snapped))
}

fn editor_advance(
    time: Res<Time<Real>>,
    mut session: ResMut<EditorSession>,
    clicks: Res<ClickAssets>,
    mut commands: Commands,
) {
    if !session.playing {
        return;
    }
    let seconds_per_beat = 60.0 / session.chart.bpm;
    session.playhead += time.delta_secs_f64() / seconds_per_beat;
    let end = session.timeline_end();
    if session.playhead >= end {
        session.playhead = end;
        session.playing = false;
        session.set_status("Reached the end of the timeline", MUTED);
    }
    if session.metronome {
        let current = session.playhead.floor() as i64;
        while session.last_click_beat < current {
            session.last_click_beat += 1;
            if session.last_click_beat >= 0 {
                play_click(
                    &mut commands,
                    &clicks,
                    session.last_click_beat % 4 == 0,
                    if session.last_click_beat % 4 == 0 {
                        0.30
                    } else {
                        0.22
                    },
                );
            }
        }
    }
}

fn update_timeline_labels(
    session: Res<EditorSession>,
    mut labels: Query<(&TimelineLabel, &mut Text2d, &mut Transform, &mut Visibility)>,
) {
    let mut assignments: Vec<Option<(f64, f32)>> = vec![None; LABEL_POOL];
    let visible_bottom = session.playhead + (TL_BOTTOM - PLAYHEAD_Y) as f64 / session.zoom as f64;
    let visible_top = session.playhead + (TL_TOP - PLAYHEAD_Y) as f64 / session.zoom as f64;
    let first_measure = (visible_bottom / 4.0).floor() as i64 - 1;
    let last_measure = (visible_top / 4.0).ceil() as i64 + 1;
    for measure in first_measure..=last_measure {
        let beat = measure as f64 * 4.0;
        let y = beat_to_y(beat, session.playhead, session.zoom);
        if y < TL_BOTTOM || y > TL_TOP {
            continue;
        }
        let slot = measure.rem_euclid(LABEL_POOL as i64) as usize;
        assignments[slot] = Some((measure as f64, y));
    }

    for (label, mut text, mut transform, mut visibility) in &mut labels {
        if let Some((measure, y)) = assignments[label.slot] {
            text.0 = format!("{}", measure as i64 + 1);
            transform.translation.y = y + 10.0;
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

fn render_editor(session: Res<EditorSession>, mut gizmos: Gizmos) {
    let visible_bottom = session.playhead + (TL_BOTTOM - PLAYHEAD_Y) as f64 / session.zoom as f64;
    let visible_top = session.playhead + (TL_TOP - PLAYHEAD_Y) as f64 / session.zoom as f64;

    // Waveform envelope behind the lanes. Drawing two polylines keeps the grid
    // readable, unlike a wall of horizontal amplitude lines.
    if let Some(waveform) = &session.waveform {
        let center_x = (TL_LEFT + TL_RIGHT) * 0.5;
        let mut left_points = Vec::with_capacity(200);
        let mut right_points = Vec::with_capacity(200);
        let mut y = TL_BOTTOM;
        while y <= TL_TOP {
            let beat = session.playhead + (y - PLAYHEAD_Y) as f64 / session.zoom as f64;
            let seconds = session.chart.seconds_from_beat(beat);
            let amp = if seconds >= 0.0 && seconds <= waveform.duration_secs {
                waveform.sample_at_time(seconds)
            } else {
                0.0
            };
            left_points.push(Vec2::new(center_x - amp * 285.0, y));
            right_points.push(Vec2::new(center_x + amp * 285.0, y));
            y += 4.0;
        }
        let color = Color::srgba(0.25, 0.70, 0.90, 0.12);
        gizmos.linestrip_2d(left_points, color);
        gizmos.linestrip_2d(right_points, color);
    }

    // Lane separators.
    let mut lane_x = TL_LEFT;
    for _ in 0..=LANE_COUNT {
        gizmos.line_2d(
            Vec2::new(lane_x, TL_BOTTOM),
            Vec2::new(lane_x, TL_TOP),
            Color::srgba(0.30, 0.42, 0.62, 0.16),
        );
        lane_x += LANE_WIDTH;
    }

    // Beat grid.
    let subdivision = if session.zoom >= 150.0 {
        0.25
    } else if session.zoom >= 75.0 {
        0.5
    } else {
        1.0
    };
    let first = (visible_bottom / subdivision).floor() as i64 - 1;
    let last = (visible_top / subdivision).ceil() as i64 + 1;
    let count = (last - first).clamp(0, 800);
    for index in 0..count {
        let beat = (first + index) as f64 * subdivision;
        let y = beat_to_y(beat, session.playhead, session.zoom);
        if y < TL_BOTTOM || y > TL_TOP {
            continue;
        }
        let integer = (beat - beat.round()).abs() < 1e-6;
        let measure = integer && (beat.round() as i64).rem_euclid(4) == 0;
        let color = if measure {
            Color::srgba(0.40, 0.60, 0.90, 0.30)
        } else if integer {
            Color::srgba(0.36, 0.48, 0.68, 0.15)
        } else {
            Color::srgba(0.30, 0.40, 0.58, 0.065)
        };
        gizmos.line_2d(Vec2::new(TL_LEFT, y), Vec2::new(TL_RIGHT, y), color);
    }

    // Notes.
    for note in &session.chart.notes {
        if note.beat < visible_bottom - 1.0 || note.beat > visible_top + 1.0 {
            continue;
        }
        let position = Vec2::new(
            TL_LEFT + (note.lane as f32 + 0.5) * LANE_WIDTH,
            beat_to_y(note.beat, session.playhead, session.zoom),
        );
        let color = theme::lane_color(note.lane);
        let points = hex_vertices(position, 11.0);
        for edge in 0..6 {
            gizmos.line_2d(points[edge], points[(edge + 1) % 6], color);
        }
        gizmos.circle_2d(position, 3.2, color);
    }

    // Playhead: three close lines make it read as a thick, bright cursor.
    for (offset, alpha) in [
        (-2.0, 0.32),
        (-1.0, 0.75),
        (0.0, 1.0),
        (1.0, 0.75),
        (2.0, 0.32),
    ] {
        gizmos.line_2d(
            Vec2::new(TL_LEFT, PLAYHEAD_Y + offset),
            Vec2::new(TL_RIGHT, PLAYHEAD_Y + offset),
            Color::srgba(0.55, 0.96, 1.0, alpha),
        );
    }
    gizmos.linestrip_2d(
        [
            Vec2::new(TL_LEFT + 2.0, PLAYHEAD_Y + 9.0),
            Vec2::new(TL_LEFT - 7.0, PLAYHEAD_Y + 9.0),
            Vec2::new(TL_LEFT + 2.0, PLAYHEAD_Y),
        ],
        Color::srgba(0.55, 0.96, 1.0, 0.95),
    );

    // Preview playfield.
    let preview = Layout {
        center: PREVIEW_CENTER,
        judge_radius: 44.0,
        spawn_radius: 158.0,
        note_size: 20.0,
    };
    let outer = preview.outer_hex();
    draw_closed_loop(&mut gizmos, &outer, Color::srgba(0.35, 0.62, 0.90, 0.40));
    let judge = preview.judge_hex();
    draw_closed_loop(&mut gizmos, &judge, Color::srgba(0.55, 0.95, 1.0, 0.55));
    for lane in 0..6u8 {
        let color = theme::lane_color(lane);
        gizmos.line_2d(
            preview.judge_point(lane),
            preview.spawn_point(lane),
            color.with_alpha(0.18),
        );
        gizmos.circle_2d(preview.judge_point(lane), 4.0, color.with_alpha(0.55));
    }
    let now = session.chart.seconds_from_beat(session.playhead);
    let approach = session.chart.approach_secs();
    for note in &session.chart.notes {
        let hit_time = session.chart.hit_time(note);
        if let Some(motion) = preview.note_motion(note.lane, hit_time, now, approach) {
            let color = theme::lane_color(note.lane);
            let points = hex_vertices(motion.position, 6.5 * motion.scale);
            for edge in 0..6 {
                gizmos.line_2d(
                    points[edge],
                    points[(edge + 1) % 6],
                    color.with_alpha(motion.alpha),
                );
            }
        }
    }
    let phase = session.playhead.rem_euclid(1.0) as f32;
    let pulse = 1.0 - phase;
    gizmos.circle_2d(
        PREVIEW_CENTER,
        29.0 + pulse * 7.0,
        Color::srgba(0.55, 0.95, 1.0, 0.10 + 0.15 * pulse),
    );
}

fn update_editor_ui(
    session: Res<EditorSession>,
    mut fields: Query<(&EditorField, &mut Text, &mut TextColor)>,
) {
    for (field, mut text, mut color) in &mut fields {
        match field {
            EditorField::Title => {
                text.0 = format!(
                    "{}{}",
                    session.chart.title,
                    if session.dirty { "  *" } else { "" }
                );
            }
            EditorField::Path => {
                text.0 = session.path.display().to_string();
            }
            EditorField::Info => {
                text.0 = format!(
                    "BPM {:.0}   |   SNAP 1/{}   |   ZOOM {:.0}   |   NOTES {}   |   METRO {}",
                    session.chart.bpm,
                    session.snap,
                    session.zoom,
                    session.chart.note_count(),
                    if session.metronome { "ON" } else { "OFF" }
                );
            }
            EditorField::Playhead => {
                let beat = session.playhead;
                let bar = (beat / 4.0).floor() as i64 + 1;
                let seconds = session.chart.seconds_from_beat(beat);
                text.0 = format!(
                    "BEAT {:>6.2}    |    BAR {:>3}    |    {}",
                    beat,
                    bar,
                    format_time(seconds)
                );
            }
            EditorField::Status => {
                text.0 = session.status.clone();
                color.0 = session.status_color;
            }
        }
    }
}

fn beat_to_y(beat: f64, playhead: f64, zoom: f32) -> f32 {
    PLAYHEAD_Y + (beat - playhead) as f32 * zoom
}

fn draw_closed_loop(gizmos: &mut Gizmos, points: &[Vec2], color: Color) {
    for index in 0..points.len() {
        gizmos.line_2d(points[index], points[(index + 1) % points.len()], color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(playhead: f64, zoom: f32, snap: u32) -> EditorSession {
        EditorSession {
            chart: Chart::default(),
            path: PathBuf::from("charts/test.json"),
            playhead,
            playing: false,
            snap,
            zoom,
            dirty: false,
            quit_armed: false,
            status: String::new(),
            status_color: MUTED,
            waveform: None,
            last_click_beat: -1,
            metronome: true,
        }
    }

    #[test]
    fn beat_to_y_roundtrip() {
        for beat in [0.0, 4.0, 12.5, 63.75] {
            let y = beat_to_y(beat, 4.0, 80.0);
            let recovered = 4.0 + (y - PLAYHEAD_Y) as f64 / 80.0;
            assert!((recovered - beat).abs() < 1e-4);
        }
        assert!((beat_to_y(4.0, 4.0, 80.0) - PLAYHEAD_Y).abs() < 1e-4);
    }

    #[test]
    fn timeline_hit_maps_and_snaps() {
        let session = session(4.0, 80.0, 4);
        let x = TL_LEFT + 2.5 * LANE_WIDTH;
        let y = beat_to_y(6.13, 4.0, 80.0);
        let (lane, beat) = timeline_hit(Vec2::new(x, y), &session).unwrap();
        assert_eq!(lane, 2);
        assert!((beat - 6.25).abs() < 1e-9);
    }

    #[test]
    fn timeline_hit_rejects_outside_and_negative() {
        let session = session(0.0, 80.0, 4);
        assert!(timeline_hit(Vec2::new(TL_LEFT - 1.0, 0.0), &session).is_none());
        assert!(timeline_hit(Vec2::new(0.0, TL_TOP + 1.0), &session).is_none());
        // Below beat 0 cannot be snapped to a valid note.
        assert!(timeline_hit(Vec2::new(TL_LEFT + 10.0, PLAYHEAD_Y - 100.0), &session).is_none());
    }

    #[test]
    fn seek_clamps_to_timeline() {
        let mut session = session(0.0, 80.0, 4);
        session.seek(-5.0);
        assert_eq!(session.playhead, 0.0);
        let end = session.timeline_end();
        session.seek(end + 100.0);
        assert!((session.playhead - end).abs() < 1e-9);
    }

    #[test]
    fn save_updates_loaded_chart_and_clears_dirty() {
        let dir = std::env::temp_dir().join(format!(
            "notation_editor_editor_save_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("chart.json");
        let mut session = session(0.0, 80.0, 4);
        session.path = path.clone();
        session.chart.add_note(2, 3.0);
        session.dirty = true;
        let mut loaded = LoadedChart {
            chart: Chart::default(),
            path: PathBuf::from("charts/other.json"),
            source: ChartSource::Builtin,
        };
        assert!(save_editor(&mut session, &mut loaded));
        assert!(!session.dirty);
        assert_eq!(loaded.chart.notes.len(), 1);
        assert_eq!(loaded.path, path);
        assert_eq!(loaded.source, ChartSource::Disk);
        let reloaded = Chart::load(&path).unwrap();
        assert_eq!(reloaded.notes, session.chart.notes);
        let _ = std::fs::remove_dir_all(dir);
    }
}
