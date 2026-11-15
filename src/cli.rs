//! Command-line handling and chart loading with a friendly fallback.

use std::path::PathBuf;

use bevy::prelude::*;

use crate::chart::{Chart, ChartError};
use crate::{AppState, ChartSource, LoadedChart};

pub const DEFAULT_CHART_PATH: &str = "charts/hex_demo.json";

#[derive(Resource, Debug, Clone)]
pub struct CliArgs {
    pub chart: Option<PathBuf>,
    pub start_state: AppState,
    pub selftest: bool,
    pub help: bool,
    pub autoplay: bool,
    pub screenshot: Option<PathBuf>,
    pub screenshot_delay: f32,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            chart: None,
            start_state: AppState::Menu,
            selftest: false,
            help: false,
            autoplay: false,
            screenshot: None,
            screenshot_delay: 3.0,
        }
    }
}

impl CliArgs {
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Self, String> {
        let mut parsed = CliArgs::default();
        let mut start_override: Option<AppState> = None;
        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--chart" | "-c" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| "--chart requires a path".to_string())?;
                    parsed.chart = Some(PathBuf::from(value));
                }
                "--play" | "-p" => {
                    if start_override.is_some() {
                        return Err("choose at most one of --play/--editor".to_string());
                    }
                    start_override = Some(AppState::Play);
                }
                "--editor" | "-e" => {
                    if start_override.is_some() {
                        return Err("choose at most one of --play/--editor".to_string());
                    }
                    start_override = Some(AppState::Editor);
                }
                "--selftest" => parsed.selftest = true,
                "--autoplay" => parsed.autoplay = true,
                "--screenshot" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| "--screenshot requires a path".to_string())?;
                    parsed.screenshot = Some(PathBuf::from(value));
                }
                "--screenshot-delay" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| "--screenshot-delay requires seconds".to_string())?;
                    let seconds: f32 = value
                        .parse()
                        .map_err(|_| format!("invalid delay {value:?}"))?;
                    if !(0.1..=120.0).contains(&seconds) {
                        return Err("--screenshot-delay must be between 0.1 and 120".to_string());
                    }
                    parsed.screenshot_delay = seconds;
                }
                "--help" | "-h" => parsed.help = true,
                other => return Err(format!("unknown argument {other:?} (try --help)")),
            }
        }
        parsed.start_state = start_override.unwrap_or(AppState::Menu);
        Ok(parsed)
    }
}

pub fn print_help() {
    println!(
        r#"NotationEditor - a hexagonal rhythm game and chart editor

USAGE:
  notation_editor [OPTIONS]

OPTIONS:
  -c, --chart <PATH>          Chart JSON to load (default: {DEFAULT_CHART_PATH})
  -p, --play                  Start directly in play mode
  -e, --editor                Start directly in the editor
      --autoplay              Hit every note automatically (demo mode)
      --screenshot <PATH>     Save one screenshot and exit
      --screenshot-delay <S>  Seconds to wait before the screenshot (default 3)
      --selftest              Run the headless chart/judgement self-test
  -h, --help                  Show this help

EXAMPLES:
  cargo run
  cargo run -- --chart charts/hex_demo.json --play
  cargo run -- --editor
  cargo run -- --play --autoplay
  cargo run -- --selftest
"#
    );
}

/// Load the requested chart, falling back to the shipped demo. This never
/// returns an error so the game always has something to play.
pub fn load_chart(args: &CliArgs) -> LoadedChart {
    let default_path = PathBuf::from(DEFAULT_CHART_PATH);

    if let Some(requested) = &args.chart {
        match Chart::load(requested) {
            Ok(chart) => {
                info!(
                    "loaded chart {} ({} notes)",
                    requested.display(),
                    chart.note_count()
                );
                return LoadedChart {
                    chart,
                    path: requested.clone(),
                    source: ChartSource::Disk,
                };
            }
            Err(err) => {
                eprintln!("warning: could not load {}: {err}", requested.display());
                eprintln!("warning: falling back to the bundled demo chart");
            }
        }
    }

    if default_path.is_file() {
        match Chart::load(&default_path) {
            Ok(chart) => {
                info!(
                    "loaded chart {} ({} notes)",
                    default_path.display(),
                    chart.note_count()
                );
                return LoadedChart {
                    chart,
                    path: default_path,
                    source: ChartSource::Disk,
                };
            }
            Err(err) => {
                eprintln!("warning: could not load bundled chart: {err}");
            }
        }
    }

    let chart = Chart::builtin_demo().unwrap_or_else(|err| {
        eprintln!("warning: embedded chart is invalid ({err}); using an empty chart");
        Chart::default()
    });
    LoadedChart {
        chart,
        path: default_path,
        source: ChartSource::Builtin,
    }
}

pub fn describe_chart_error(err: &ChartError) -> String {
    match err {
        ChartError::Io(io) => format!("I/O error: {io}"),
        ChartError::Json(json) => format!("JSON error: {json}"),
        ChartError::Invalid(errors) => format!("Invalid chart: {}", errors.join("; ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_defaults() {
        let parsed = CliArgs::parse(args(&[])).unwrap();
        assert_eq!(parsed.start_state, AppState::Menu);
        assert!(!parsed.selftest);
        assert!(!parsed.autoplay);
        assert!(parsed.chart.is_none());
        assert!(parsed.screenshot.is_none());
    }

    #[test]
    fn parses_chart_and_start_state() {
        let parsed = CliArgs::parse(args(&["--chart", "charts/x.json", "--editor"])).unwrap();
        assert_eq!(parsed.chart, Some(PathBuf::from("charts/x.json")));
        assert_eq!(parsed.start_state, AppState::Editor);
    }

    #[test]
    fn rejects_unknown_and_missing_values() {
        assert!(CliArgs::parse(args(&["--nope"])).is_err());
        assert!(CliArgs::parse(args(&["--chart"])).is_err());
        assert!(CliArgs::parse(args(&["--play", "--editor"])).is_err());
    }

    #[test]
    fn parses_screenshot_and_autoplay() {
        let parsed = CliArgs::parse(args(&[
            "--play",
            "--autoplay",
            "--screenshot",
            "docs/a.png",
            "--screenshot-delay",
            "5",
        ]))
        .unwrap();
        assert!(parsed.autoplay);
        assert_eq!(parsed.screenshot, Some(PathBuf::from("docs/a.png")));
        assert_eq!(parsed.screenshot_delay, 5.0);
    }

    #[test]
    fn help_flag_is_parsed() {
        assert!(CliArgs::parse(args(&["--help"])).unwrap().help);
    }
}
