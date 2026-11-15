//! NotationEditor - a hexagonal rhythm game and chart editor.
//!
//! The crate is split into a pure core (`chart`, `game`, `geometry`) and Bevy
//! screens (`menu`, `play`, `editor`, `results`).  The core has no rendering or
//! audio dependencies, which keeps it testable headlessly.

pub mod audio;
pub mod chart;
pub mod cli;
pub mod editor;
pub mod game;
pub mod geometry;
pub mod menu;
pub mod play;
pub mod results;
pub mod selftest;
pub mod theme;
pub mod waveform;

use std::fs;
use std::path::{Path, PathBuf};

use bevy::asset::{AssetPlugin, load_internal_asset};
use bevy::core_pipeline::blit::BlitPlugin;
use bevy::core_pipeline::core_2d::Core2dPlugin;
use bevy::core_pipeline::tonemapping::TonemappingPlugin;
use bevy::core_pipeline::upscaling::UpscalingPlugin;
use bevy::pbr::PbrPlugin;
use bevy::prelude::*;
use bevy::render::camera::{OrthographicProjection, ScalingMode};
use bevy::render::render_resource::Shader;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::sprite::SpritePlugin;
use bevy::window::WindowPlugin;

use chart::Chart;

pub const WINDOW_WIDTH: u32 = 1280;
pub const WINDOW_HEIGHT: u32 = 800;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[states(scoped_entities)]
pub enum AppState {
    #[default]
    Menu,
    Play,
    Editor,
    Results,
}

/// Where the currently loaded chart came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartSource {
    Disk,
    Builtin,
}

#[derive(Resource, Clone)]
pub struct LoadedChart {
    pub chart: Chart,
    pub path: PathBuf,
    pub source: ChartSource,
}

/// Which screen started the current play session. Used by the results screen.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameOrigin {
    Menu,
    Editor,
}

/// Build and run the Bevy app. Returns a process exit code.
pub fn run() -> i32 {
    let cli = match cli::CliArgs::parse(std::env::args().skip(1)) {
        Ok(cli) => cli,
        Err(err) => {
            eprintln!("error: {err}");
            cli::print_help();
            return 2;
        }
    };
    if cli.help {
        cli::print_help();
        return 0;
    }
    if cli.selftest {
        return selftest::run();
    }

    let loaded = cli::load_chart(&cli);

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "NotationEditor  |  Hexagonal Rhythm Game & Chart Editor".to_string(),
                    resolution: (WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32).into(),
                    resizable: true,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: asset_root().to_string_lossy().to_string(),
                ..default()
            })
            // This is a 2D game; skipping PBR removes unused 3D pipelines. The
            // core pipeline's mip-generation plugin also exercises compute/depth
            // paths that are not available on every GLES driver (e.g. llvmpipe),
            // so build the 2D graph explicitly.
            .disable::<PbrPlugin>()
            .disable::<bevy::core_pipeline::CorePipelinePlugin>()
            .add_before::<SpritePlugin>(Core2dPlugin)
            .add_before::<SpritePlugin>(BlitPlugin)
            .add_before::<SpritePlugin>(TonemappingPlugin)
            .add_before::<SpritePlugin>(UpscalingPlugin),
    );

    app.add_plugins(FullscreenShaderPlugin);

    let ui_assets = {
        let mut images = app.world_mut().resource_mut::<Assets<Image>>();
        theme::build_textures(&mut images)
    };

    let screenshot = cli.screenshot.clone();
    let screenshot_delay = cli.screenshot_delay;

    app.insert_resource(ClearColor(theme::BG));
    app.insert_resource(ui_assets);
    app.insert_resource(cli);
    app.insert_resource(loaded);
    app.insert_resource(GameOrigin::Menu);
    app.insert_resource(geometry::Layout::default());
    app.insert_state(AppState::Menu);

    app.add_systems(Startup, (setup_camera, start_from_cli));
    app.add_systems(Update, theme::button_visuals);
    app.add_plugins((
        audio::AudioAssetsPlugin,
        menu::MenuPlugin,
        play::PlayPlugin,
        editor::EditorPlugin,
        results::ResultsPlugin,
    ));

    if let Some(path) = screenshot {
        app.insert_resource(ScreenshotRequest {
            path,
            delay: screenshot_delay,
            timer: 0.0,
            captured: false,
            exit_timer: 0.0,
        });
        app.add_systems(Update, screenshot_tick);
    }

    match app.run() {
        AppExit::Success => 0,
        AppExit::Error(code) => code.get() as i32,
    }
}

/// Root directory for Bevy assets. In a dev checkout the executable lives in
/// `target/debug`, so Bevy would look in `target/debug/assets` by default.
/// Prefer assets next to the executable (distribution) and fall back to the
/// cargo manifest directory (cargo run / cargo install).
pub fn asset_root() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let beside_exe = parent.join("assets");
            if beside_exe.is_dir() {
                return beside_exe;
            }
        }
    }
    let manifest_assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    if manifest_assets.is_dir() {
        return manifest_assets;
    }
    PathBuf::from("assets")
}

#[derive(Resource)]
struct ScreenshotRequest {
    path: PathBuf,
    delay: f32,
    timer: f32,
    captured: bool,
    exit_timer: f32,
}

fn screenshot_tick(
    time: Res<Time<Real>>,
    mut request: ResMut<ScreenshotRequest>,
    mut commands: Commands,
    mut exit: EventWriter<AppExit>,
) {
    if request.captured {
        request.exit_timer += time.delta_secs();
        if request.exit_timer > 1.5 {
            exit.write(AppExit::Success);
        }
        return;
    }
    request.timer += time.delta_secs();
    if request.timer < request.delay {
        return;
    }
    if let Some(parent) = request.path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    info!(
        "capturing screenshot to {} after {:.1}s",
        request.path.display(),
        request.timer
    );
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(request.path.clone()));
    request.captured = true;
}

fn start_from_cli(cli: Res<cli::CliArgs>, mut next: ResMut<NextState<AppState>>) {
    if cli.start_state != AppState::Menu {
        next.set(cli.start_state);
    }
}

/// `CorePipelinePlugin` normally loads this. We build the 2D graph manually
/// (to avoid 3D compute pipelines on software GL), so load it ourselves under
/// the fixed handle the built-in blit/fullscreen pipelines expect.
struct FullscreenShaderPlugin;

impl Plugin for FullscreenShaderPlugin {
    fn build(&self, app: &mut App) {
        const FULLSCREEN_SHADER: Handle<Shader> =
            bevy::asset::weak_handle!("481fb759-d0b1-4175-8319-c439acde30a2");
        load_internal_asset!(
            app,
            FULLSCREEN_SHADER,
            "shaders/fullscreen.wgsl",
            Shader::from_wgsl
        );
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Msaa::Off,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: WINDOW_WIDTH as f32,
                min_height: WINDOW_HEIGHT as f32,
            },
            ..OrthographicProjection::default_2d()
        }),
        Name::new("Main Camera"),
    ));
}
