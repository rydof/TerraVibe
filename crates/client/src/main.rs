use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use bevy::app::{TaskPoolOptions, TaskPoolPlugin, TaskPoolThreadAssignmentPolicy};
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::settings::{PowerPreference, WgpuSettings};
use bevy::window::{MonitorSelection, PresentMode, WindowMode, WindowResolution};
use terravibe_assets::{ENV_ASSETS, TerrariaContent, extract_pack};

mod art;
pub mod audio;
mod bench;
pub mod gamepad;
mod hud;
pub mod input_map;
pub mod last_played;
mod light;
pub mod menu;
mod net;
pub mod qa;
pub mod session;
mod settings;
pub mod ui_theme;
mod view;

const USAGE: &str = "usage: terravibe [--terraria-dir PATH] [--assets-dir PATH] [--bench] [--liquid-stress] [--input-debug] [--music-debug]\n       \
                     terravibe [--world NAME] [--save-dir PATH] [--qa SCENARIO] [--qa-phase N] [--qa-out DIR]\n       \
                     terravibe extract <OUT_DIR> [--terraria-dir PATH]";

#[derive(Debug, PartialEq)]
enum Cli {
    Run {
        terraria_dir: Option<PathBuf>,
        assets_dir: Option<PathBuf>,
        bench: bool,
        liquid_stress: bool,
        input_debug: bool,
        music_debug: bool,
        world: Option<String>,
        save_dir: Option<PathBuf>,
        qa: Option<String>,
        qa_phase: Option<u32>,
        qa_out: Option<PathBuf>,
    },
    Extract {
        out: PathBuf,
        terraria_dir: Option<PathBuf>,
    },
}

/// Parses arguments (without the program name). `Err` holds the offending argument.
fn parse(args: &[String]) -> Result<Cli, String> {
    let extract = args.first().is_some_and(|a| a == "extract");
    let mut rest = args.iter().skip(usize::from(extract));
    let mut terraria_dir = None;
    let mut assets_dir = None;
    let mut bench = false;
    let mut liquid_stress = false;
    let mut input_debug = false;
    let mut music_debug = false;
    let mut world = None;
    let mut save_dir = None;
    let mut qa = None;
    let mut qa_phase = None;
    let mut qa_out = None;
    let mut out = None;

    while let Some(a) = rest.next() {
        let mut path_value = || {
            rest.next()
                .map(PathBuf::from)
                .ok_or(format!("{a} needs a PATH"))
        };
        match a.as_str() {
            "--terraria-dir" => terraria_dir = Some(path_value()?),
            "--assets-dir" if !extract => assets_dir = Some(path_value()?),
            "--bench" if !extract => bench = true,
            "--liquid-stress" if !extract => liquid_stress = true,
            "--input-debug" if !extract => input_debug = true,
            "--music-debug" if !extract => music_debug = true,
            "--world" if !extract => {
                world = Some(rest.next().ok_or(format!("{a} needs a NAME"))?.clone());
            }
            "--save-dir" if !extract => save_dir = Some(path_value()?),
            "--qa" if !extract => {
                qa = Some(rest.next().ok_or(format!("{a} needs a SCENARIO"))?.clone());
            }
            "--qa-phase" if !extract => {
                let s = rest.next().ok_or(format!("{a} needs a NUMBER"))?;
                let n = s
                    .parse::<u32>()
                    .map_err(|_| format!("{a} needs an integer, got '{s}'"))?;
                qa_phase = Some(n);
            }
            "--qa-out" if !extract => {
                qa_out = Some(
                    rest.next()
                        .map(PathBuf::from)
                        .ok_or(format!("{a} needs a DIR"))?,
                );
            }
            s if extract && out.is_none() && !s.starts_with('-') => out = Some(PathBuf::from(s)),
            _ => return Err(format!("unknown argument: {a}")),
        }
    }
    if extract {
        let out = out.ok_or("extract needs <OUT_DIR>")?;
        return Ok(Cli::Extract { out, terraria_dir });
    }
    Ok(Cli::Run {
        terraria_dir,
        assets_dir,
        bench,
        liquid_stress,
        input_debug,
        music_debug,
        world,
        save_dir,
        qa,
        qa_phase,
        qa_out,
    })
}

fn main() -> ExitCode {
    let _ = bench::PROCESS_START.set(Instant::now());
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            ExitCode::from(2)
        }
        Ok(Cli::Extract { out, terraria_dir }) => extract(out, terraria_dir),
        Ok(Cli::Run {
            terraria_dir,
            assets_dir,
            bench,
            liquid_stress,
            input_debug,
            music_debug,
            world,
            save_dir: _,
            qa,
            qa_phase,
            qa_out,
        }) => {
            if liquid_stress {
                // SAFETY: called on the main thread before any worker threads spawn.
                unsafe {
                    std::env::set_var("TERRAVIBE_BENCH_SCENARIO", "liquid_stress");
                }
            }
            if let Some(ref scenario_name) = qa {
                if qa_phase == Some(0) {
                    if let Some(s) = qa::SCENARIOS.iter().find(|s| s.name == scenario_name) {
                        println!("{}", qa::discovery_output(s));
                        return ExitCode::SUCCESS;
                    } else {
                        eprintln!("unknown qa scenario: {scenario_name}");
                        return ExitCode::from(1);
                    }
                }
            }

            let qa_scenario = if let Some(ref scenario_name) = qa {
                let Some(s) = qa::SCENARIOS.iter().find(|s| s.name == scenario_name) else {
                    eprintln!("unknown qa scenario: {scenario_name}");
                    return ExitCode::from(1);
                };
                let phase = qa_phase.unwrap_or(1) as u8;
                let out = qa_out.unwrap_or_else(|| PathBuf::from("."));
                Some((s.name.to_string(), phase, out))
            } else {
                None
            };

            let save_cfg =
                terravibe_server::SaveCfg::from_args_env(&args, |k| std::env::var(k).ok());
            install_panic_hook(save_cfg.root.join("crashes"));
            let world = world.or_else(|| {
                std::env::var("TERRAVIBE_WORLD")
                    .ok()
                    .filter(|s| !s.is_empty())
            });
            let is_automation = bench
                || std::env::var_os(view::ENV_SCREENSHOT).is_some()
                || std::env::var_os("TERRAVIBE_DEMO").is_some();
            let mode = session::launch_mode(is_automation, world);

            let direct_world = match mode {
                session::LaunchMode::DirectWorld { ref name } => {
                    match session::direct_world_info(&save_cfg.root, name) {
                        Ok(info) => Some(info),
                        Err(e) => {
                            eprintln!("error: {e}");
                            return ExitCode::from(1);
                        }
                    }
                }
                _ => None,
            };

            let env_assets = std::env::var_os(ENV_ASSETS).filter(|v| !v.is_empty());
            let pack_dir = assets_dir.or_else(|| {
                let local = art::art_dir().join("terraria-pack");
                local_pack_candidate(&local, env_assets.as_deref())
            });
            let content = match TerrariaContent::open(pack_dir, terraria_dir) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(1);
                }
            };
            run(
                content,
                bench,
                input_debug,
                music_debug,
                is_automation,
                save_cfg.root,
                mode,
                direct_world,
                qa_scenario,
            )
        }
    }
}

/// Returns `Some(local)` only when `env_assets` is `None` or empty and `local/pack.txt` is a file.
fn local_pack_candidate(local: &Path, env_assets: Option<&OsStr>) -> Option<PathBuf> {
    let override_active = env_assets.is_some_and(|v| !v.is_empty());
    if !override_active && local.join("pack.txt").is_file() {
        Some(local.to_path_buf())
    } else {
        None
    }
}

/// Writes a panic/crash report to a timestamped log file inside `dir`.
pub fn write_crash_log(dir: &Path, msg: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let path = dir.join(format!("crash-{ts}-{}.log", std::process::id()));
    std::fs::write(&path, msg)?;
    Ok(path)
}

/// Installs a panic hook that writes a crash log before forwarding to the previous hook.
pub fn install_panic_hook(crash_dir: PathBuf) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let bt = std::backtrace::Backtrace::capture();
        let msg = format!("{info}\n\nBacktrace:\n{bt}");
        let _ = write_crash_log(&crash_dir, &msg);
        prev(info);
    }));
}

fn extract(out: PathBuf, terraria_dir: Option<PathBuf>) -> ExitCode {
    let start = Instant::now();
    let result = TerrariaContent::locate(terraria_dir).and_then(|src| extract_pack(&src, &out));
    match result {
        Ok(stats) => {
            println!(
                "extracted to {}: {} textures, {} sounds, {} fonts, {} music, {} skipped in {:.1?}",
                out.display(),
                stats.textures,
                stats.sounds,
                stats.fonts,
                stats.music,
                stats.skipped,
                start.elapsed()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

/// Uncapped present mode for the bench. On Wayland, `AutoNoVsync` resolves to
/// `Immediate`, which ran capped at the 60 Hz refresh in fullscreen (p99 ~17 ms);
/// `Mailbox` (always offered by Wayland Vulkan WSI) is not capped.
fn bench_present_mode() -> PresentMode {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        PresentMode::Mailbox
    } else {
        PresentMode::AutoNoVsync
    }
}

/// Env var: substring of the monitor name that automated runs (bench, screenshot)
/// open on. Defaults to the laptop panel so they never cover an external monitor.
const ENV_MONITOR: &str = "TERRAVIBE_MONITOR";

/// Whether `name` is the monitor automated runs should use.
fn is_target_monitor(name: &str, wanted: Option<&str>) -> bool {
    match wanted {
        Some(w) => name.contains(w),
        None => name.starts_with("eDP") || name.starts_with("LVDS") || name.starts_with("DSI"),
    }
}

/// Moves the window, fullscreen, onto the target monitor once monitors are known.
fn pin_to_internal_monitor(
    mut done: Local<bool>,
    monitors: Query<(Entity, &bevy::window::Monitor)>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    if *done || monitors.is_empty() {
        return;
    }
    *done = true;
    let wanted = std::env::var(ENV_MONITOR).ok();
    let target = monitors.iter().find(|(_, m)| {
        m.name
            .as_deref()
            .is_some_and(|n| is_target_monitor(n, wanted.as_deref()))
    });
    let (Some((entity, monitor)), Ok(mut window)) = (target, windows.single_mut()) else {
        warn!("no internal monitor found; leaving the window where it is");
        return;
    };
    info!("automated run on monitor {:?}", monitor.name);
    window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Entity(entity));
}

pub fn initial_window(automation: bool, s: Option<&settings::Settings>) -> Window {
    if automation {
        Window {
            title: "TerraVibe bench".into(),
            resolution: WindowResolution::new(1920, 1080).with_scale_factor_override(1.0),
            present_mode: bench_present_mode(),
            mode: WindowMode::Windowed,
            ..default()
        }
    } else {
        let default_settings = settings::Settings::default();
        let s = s.unwrap_or(&default_settings);
        settings::window_for(s)
    }
}

fn run(
    content: TerrariaContent,
    bench: bool,
    input_debug: bool,
    music_debug: bool,
    is_automation: bool,
    save_root: PathBuf,
    mode: session::LaunchMode,
    direct_world: Option<terravibe_server::WorldInfo>,
    qa_scenario: Option<(String, u8, PathBuf)>,
) -> ExitCode {
    let settings = settings::load(|k| std::env::var(k).ok());
    let menu_sounds = menu::sounds::MenuSounds::load(&content);
    let is_qa = qa_scenario.is_some();
    let has_env_monitor = std::env::var(ENV_MONITOR).is_ok();
    let num_cpus = std::thread::available_parallelism().map_or(4, |n| n.get());
    let async_threads = (num_cpus / 4).clamp(2, 3);
    let mut app = App::new();
    if input_debug {
        app.insert_resource(view::camera::InputDebug);
    }
    app.add_plugins(
        DefaultPlugins
            .set(TaskPoolPlugin {
                task_pool_options: TaskPoolOptions {
                    io: TaskPoolThreadAssignmentPolicy {
                        min_threads: 1,
                        max_threads: 2,
                        percent: 0.15,
                        on_thread_spawn: None,
                        on_thread_destroy: None,
                    },
                    async_compute: TaskPoolThreadAssignmentPolicy {
                        min_threads: 2,
                        max_threads: async_threads,
                        percent: 0.25,
                        on_thread_spawn: None,
                        on_thread_destroy: None,
                    },
                    ..default()
                },
            })
            .set(WindowPlugin {
                primary_window: Some(initial_window(is_automation, Some(&settings))),
                ..default()
            })
            .set(RenderPlugin {
                // Prefer the discrete GPU on hybrid laptops.
                render_creation: WgpuSettings {
                    power_preference: PowerPreference::HighPerformance,
                    ..default()
                }
                .into(),
                ..default()
            }),
    )
    .insert_resource(settings)
    .insert_resource(session::SaveRoot(save_root))
    .add_plugins(session::SessionPlugin)
    .add_plugins(menu::MenuPlugin {
        sounds: menu_sounds,
    })
    .add_plugins(audio::MusicPlugin {
        content: Some(std::sync::Arc::new(content.clone())),
        debug_logging: music_debug,
    })
    .add_plugins(audio::SfxPlugin {
        content: Some(std::sync::Arc::new(content.clone())),
    })
    .add_plugins(audio::AmbientPlugin)
    .add_plugins(gamepad::GamepadPlugin)
    .add_plugins(view::ViewPlugin { content, bench })
    .add_systems(
        Update,
        pin_to_internal_monitor.run_if(move || is_automation || is_qa || has_env_monitor),
    );

    if let Some((scenario, phase, out)) = qa_scenario {
        app.add_plugins(qa::QaPlugin::new(scenario, phase, out));
    }

    match mode {
        session::LaunchMode::Automation => {
            app.insert_resource(session::Automation);
            app.insert_resource(session::SelectedWorld {
                name: "Automation".into(),
                seed: view::SEED,
                size: terravibe_sim::WorldSize::Small,
                persist: false,
            });
            app.world_mut()
                .resource_mut::<NextState<session::AppState>>()
                .set(session::AppState::InGame);
        }
        session::LaunchMode::DirectWorld { .. } => {
            let info = direct_world.expect("validated before run");
            app.insert_resource(session::SelectedWorld {
                name: info.name,
                seed: info.seed,
                size: info.size,
                persist: true,
            });
            app.world_mut()
                .resource_mut::<NextState<session::AppState>>()
                .set(session::AppState::InGame);
        }
        session::LaunchMode::Menu => {}
    }

    let exit = app.run();
    // The app is dropped here, which ends the server loop; wait for its save.
    net::join_server();
    match exit {
        AppExit::Success => ExitCode::SUCCESS,
        AppExit::Error(code) => ExitCode::from(code.get()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Cli, String> {
        parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn forced_window_beats_settings() {
        let s = settings::Settings {
            width: 800,
            height: 600,
            ..default()
        };
        let w_auto = initial_window(true, Some(&s));
        assert_eq!(w_auto.resolution.width() as u32, 1920);
        assert_eq!(w_auto.resolution.height() as u32, 1080);
        let w_norm = initial_window(false, Some(&s));
        assert_eq!(w_norm.resolution.width() as u32, 800);
        assert_eq!(w_norm.resolution.height() as u32, 600);
    }

    #[test]
    fn target_monitor_defaults_to_laptop_panel() {
        assert!(is_target_monitor("eDP-1", None));
        assert!(!is_target_monitor("DP-1", None));
        assert!(is_target_monitor("HDMI-A-1", Some("HDMI")));
    }

    #[test]
    fn parses_run_flags() {
        assert_eq!(
            p(&["--bench", "--terraria-dir", "/t", "--assets-dir", "/a"]),
            Ok(Cli::Run {
                terraria_dir: Some("/t".into()),
                assets_dir: Some("/a".into()),
                bench: true,
                liquid_stress: false,
                input_debug: false,
                music_debug: false,
                world: None,
                save_dir: None,
                qa: None,
                qa_phase: None,
                qa_out: None,
            })
        );
        assert_eq!(
            p(&["--input-debug"]),
            Ok(Cli::Run {
                terraria_dir: None,
                assets_dir: None,
                bench: false,
                liquid_stress: false,
                input_debug: true,
                music_debug: false,
                world: None,
                save_dir: None,
                qa: None,
                qa_phase: None,
                qa_out: None,
            })
        );
        assert_eq!(
            p(&[
                "--qa",
                "menu_title",
                "--qa-phase",
                "1",
                "--qa-out",
                "/tmp/qa"
            ]),
            Ok(Cli::Run {
                terraria_dir: None,
                assets_dir: None,
                bench: false,
                liquid_stress: false,
                input_debug: false,
                music_debug: false,
                world: None,
                save_dir: None,
                qa: Some("menu_title".into()),
                qa_phase: Some(1),
                qa_out: Some(PathBuf::from("/tmp/qa")),
            })
        );
        assert_eq!(
            p(&["--bench", "--liquid-stress"]),
            Ok(Cli::Run {
                terraria_dir: None,
                assets_dir: None,
                bench: true,
                liquid_stress: true,
                input_debug: false,
                music_debug: false,
                world: None,
                save_dir: None,
                qa: None,
                qa_phase: None,
                qa_out: None,
            })
        );
        assert_eq!(
            p(&[]),
            Ok(Cli::Run {
                terraria_dir: None,
                assets_dir: None,
                bench: false,
                liquid_stress: false,
                input_debug: false,
                music_debug: false,
                world: None,
                save_dir: None,
                qa: None,
                qa_phase: None,
                qa_out: None,
            })
        );
    }

    #[test]
    fn parses_extract() {
        assert_eq!(
            p(&["extract", "/out", "--terraria-dir", "/t"]),
            Ok(Cli::Extract {
                out: "/out".into(),
                terraria_dir: Some("/t".into())
            })
        );
    }

    #[test]
    fn rejects_bad_args() {
        assert!(p(&["--nope"]).is_err());
        assert!(p(&["--terraria-dir"]).is_err());
        assert!(p(&["extract"]).is_err());
        assert!(p(&["extract", "/a", "/b"]).is_err());
        assert!(p(&["extract", "/a", "--bench"]).is_err());
    }

    #[test]
    fn local_pack_candidate_used_only_without_override() {
        let dir = std::env::temp_dir().join(format!("tv-local-pack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pack.txt"), "x").unwrap();

        assert_eq!(local_pack_candidate(&dir, None), Some(dir.clone()));
        assert_eq!(
            local_pack_candidate(&dir, Some(std::ffi::OsStr::new(""))),
            Some(dir.clone())
        );
        assert_eq!(
            local_pack_candidate(&dir, Some(std::ffi::OsStr::new("/other"))),
            None
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_crash_log_creates_readable_file() {
        let dir = std::env::temp_dir().join(format!("tv-crash-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path =
            write_crash_log(&dir, "panic: test crash").expect("write_crash_log must succeed");
        assert!(path.exists(), "crash log file must exist");
        assert!(path.starts_with(&dir), "crash log must be inside dir");
        let contents = std::fs::read_to_string(&path).expect("crash log must be readable UTF-8");
        assert!(contents.contains("panic: test crash"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
