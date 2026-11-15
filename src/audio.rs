//! Audio asset preloading and small playback helpers.

use std::path::PathBuf;

use bevy::audio::{AudioPlayer, PlaybackSettings, Volume};
use bevy::prelude::*;

use crate::{AppState, LoadedChart, asset_root};

/// Short metronome sounds used by the editor and by charts without audio.
#[derive(Resource, Clone)]
pub struct ClickAssets {
    pub normal: Handle<AudioSource>,
    pub accent: Handle<AudioSource>,
}

/// The optional song associated with the loaded chart.
#[derive(Resource, Default)]
pub struct ChartAudio {
    pub handle: Option<Handle<AudioSource>>,
    /// Path relative to `assets/`, if an audio file was found.
    pub asset_path: Option<String>,
}

/// Marks the entity that plays the chart song.
#[derive(Component)]
pub struct SongAudio;

pub struct AudioAssetsPlugin;

impl Plugin for AudioAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChartAudio>()
            .add_systems(Startup, (load_click_assets, preload_chart_audio));
    }
}

fn load_click_assets(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(ClickAssets {
        normal: assets.load("audio/click.ogg"),
        accent: assets.load("audio/click_accent.ogg"),
    });
}

fn preload_chart_audio(
    loaded: Res<LoadedChart>,
    assets: Res<AssetServer>,
    mut audio: ResMut<ChartAudio>,
) {
    let Some(audio_name) = loaded.chart.audio.as_deref() else {
        audio.handle = None;
        audio.asset_path = None;
        return;
    };
    let relative = normalize_asset_path(audio_name);
    let disk_path = asset_root().join(&relative);
    if disk_path.is_file() {
        audio.asset_path = Some(relative.clone());
        audio.handle = Some(assets.load(relative));
        info!("loaded chart audio: {}", disk_path.display());
    } else {
        warn!(
            "chart audio is missing: {} (falling back to the metronome)",
            disk_path.display()
        );
        audio.handle = None;
        audio.asset_path = None;
    }
}

/// Strip a leading `assets/` and any leading slash. Bevy's asset server always
/// resolves paths relative to the configured asset root.
pub fn normalize_asset_path(path: &str) -> String {
    let path = path.trim().replace('\\', "/");
    let path = path.trim_start_matches('/');
    path.strip_prefix("assets/").unwrap_or(path).to_string()
}

/// Absolute (relative to CWD) path of a chart audio file.
pub fn audio_disk_path(asset_path: &str) -> PathBuf {
    asset_root().join(normalize_asset_path(asset_path))
}

pub fn spawn_song(commands: &mut Commands, handle: &Handle<AudioSource>) {
    commands.spawn((
        AudioPlayer::new(handle.clone()),
        PlaybackSettings::ONCE.with_volume(Volume::Linear(0.72)),
        SongAudio,
        StateScoped(AppState::Play),
    ));
}

pub fn play_click(commands: &mut Commands, clicks: &ClickAssets, accent: bool, volume: f32) {
    let handle = if accent {
        clicks.accent.clone()
    } else {
        clicks.normal.clone()
    };
    commands.spawn((
        AudioPlayer::new(handle),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
    ));
}

#[cfg(test)]
mod tests {
    use super::normalize_asset_path;

    #[test]
    fn asset_paths_are_normalised() {
        assert_eq!(normalize_asset_path("audio/song.ogg"), "audio/song.ogg");
        assert_eq!(normalize_asset_path("/audio/song.ogg"), "audio/song.ogg");
        assert_eq!(
            normalize_asset_path("assets/audio/song.ogg"),
            "audio/song.ogg"
        );
        assert_eq!(normalize_asset_path("audio\\song.ogg"), "audio/song.ogg");
    }
}
