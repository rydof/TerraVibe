//! QA harness: scripted scenarios, driver systems, metrics, and artifact generation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey};
use bevy::input::mouse::{MouseButton, MouseButtonInput, MouseMotion};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use terravibe_sim::net::ClientMsg;

use crate::session::{AppState, MenuScreen, SelectedWorld};

pub const FIXTURE_SEED: u32 = 42;
pub const FAILED_STEP: usize = usize::MAX;

/// Seconds to wait for a GPU screenshot before failing the step. Debug builds
/// render slowly (especially on CI), so they get a wider window.
fn screenshot_timeout_secs() -> f32 {
    if cfg!(debug_assertions) { 15.0 } else { 5.0 }
}

/// Width of the logical canvas QA scenarios author their click coordinates on.
pub const REF_WIDTH: f32 = 1920.0;
/// Height of the logical canvas QA scenarios author their click coordinates on.
pub const REF_HEIGHT: f32 = 1080.0;

/// Maps a click authored on the 1920x1080 reference canvas to window-relative
/// coordinates, so a scenario keeps hitting the same spot at any resolution.
pub fn scale_click(x: f32, y: f32, window: Vec2) -> Vec2 {
    Vec2::new(x * window.x / REF_WIDTH, y * window.y / REF_HEIGHT)
}

/// Marker resource inserted for QA scenarios that need the demo enemy trio.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct QaCombatDemo;

/// Marker resource inserted for QA scenarios that test liquid mechanics.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct QaLiquidsDemo;

/// Marker resource inserted for QA scenarios that ask the server to kill the player.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct QaKillDemo;

const DUMMY_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
    0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
    0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, // 8-bit RGBA
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, // IDAT
    0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00,
    0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, // IEND
    0x42, 0x60, 0x82,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixtureKind {
    Flat,
    Combat,
    Corruption,
    Boss,
    Housing,
    Liquids,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scenario {
    pub name: &'static str,
    pub direct_ingame: bool,
    pub fixture: Option<FixtureKind>,
    /// QA scenarios may opt into the demo enemies; a world name never does.
    pub demo_enemies: bool,
    pub phases: u8,
}

pub mod fixture;
pub mod scenarios;

pub use fixture::build_fixture;
pub use scenarios::SCENARIOS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricSource {
    Pos,
    MaxPlayerY,
    Vel,
    HorizVel,
    Life,
    InvCount,
    Clock,
    Hits,
    Breath,
    SpeedRatio,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expect {
    InGame(bool),
    Screen(&'static str),
    Text(&'static str),
    MetricGt(&'static str, f32),
    MetricLt(&'static str, f32),
    Music(&'static str),
    MusicSilent,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Wait(f32),
    Key(KeyCode),
    Hold(KeyCode, f32),
    Look(f32, f32),
    Click {
        x: f32,
        y: f32,
    },
    /// A left click with Left Shift held for the press.
    ShiftClick {
        x: f32,
        y: f32,
    },
    /// A right click (place / interact).
    RightClick {
        x: f32,
        y: f32,
    },
    Type(&'static str),
    Screenshot(&'static str),
    Expect(Expect),
    Metric(&'static str, MetricSource),
    SetClock(u32),
    SpawnDemoBoss,
    /// Kills the local player through the sim, to exercise the death flow.
    Kill,
    /// Asks the authoritative server to respawn the local player.
    Respawn,
}

#[derive(Resource, Debug, Clone)]
pub struct QaRun {
    pub scenario: usize,
    pub phase: u8,
    pub step: usize,
    pub timer: f32,
    pub out: PathBuf,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct QaMetrics {
    pub frames_ms: Vec<f32>,
    pub named: Vec<(String, f32)>,
    pub log: Vec<String>,
}

/// Live client performance counters sampled by QA and benchmark scenarios.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct PerfCounters {
    pub particles_active: usize,
    pub critters_active: usize,
    pub map_rebuild_ms: f32,
}

/// Refreshes [`PerfCounters`] from active particle pools and critter entities.
pub fn update_perf_counters(
    mut perf: ResMut<PerfCounters>,
    particles: Option<Res<crate::view::particles::ParticlePool>>,
    ambient: Option<Res<crate::view::particles::AmbientParticlePool>>,
    weather: Option<Res<crate::view::weather::WeatherFxState>>,
    enemies: Query<&crate::view::enemies::ClientEnemy>,
) {
    let p_count = particles.as_ref().map_or(0, |p| p.entries.len());
    let a_count = ambient.as_ref().map_or(0, |a| a.entries.len());
    let w_count = weather.as_ref().map_or(0, |w| w.particles.len());
    perf.particles_active = p_count + a_count + w_count;
    perf.critters_active = enemies.iter().filter(|e| e.kind.is_critter()).count();
}

#[derive(Resource, Clone, Debug)]
pub struct QaPlan(pub Vec<Step>);

#[derive(Resource, Default)]
pub struct PendingScreenshots {
    pub pending: u32,
}

#[derive(Resource, Default)]
pub struct HitTracker {
    pub last_hp: HashMap<terravibe_sim::enemy::EnemyId, i32>,
    pub hits: usize,
}

pub fn default_steps(scenario_name: &str, phase: u8) -> Vec<Step> {
    scenarios::scenario_steps(scenario_name, phase)
}

pub fn discovery_output(scenario: &Scenario) -> String {
    format!("qa: phases={} scenario={}", scenario.phases, scenario.name)
}

fn escape_json(s: &str) -> String {
    let mut escaped = String::new();
    for c in s.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

pub fn metrics_json(m: &QaMetrics) -> String {
    let mut out = String::new();
    out.push_str("{\n");

    out.push_str("  \"frame_ms\": [");
    for (i, ms) in m.frames_ms.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!("{:.2}", ms));
    }
    out.push_str("],\n");

    out.push_str("  \"named\": {\n");
    for (i, (k, v)) in m.named.iter().enumerate() {
        let comma = if i + 1 < m.named.len() { "," } else { "" };
        out.push_str(&format!("    \"{}\": {}{}\n", escape_json(k), v, comma));
    }
    out.push_str("  },\n");

    out.push_str("  \"log\": [\n");
    for (i, line) in m.log.iter().enumerate() {
        let comma = if i + 1 < m.log.len() { "," } else { "" };
        out.push_str(&format!("    \"{}\"{}\n", escape_json(line), comma));
    }
    out.push_str("  ]\n");

    out.push_str("}\n");
    out
}

pub fn write_artifacts(out_dir: &Path, metrics: &QaMetrics) {
    let _ = std::fs::create_dir_all(out_dir);
    let json = metrics_json(metrics);
    let _ = std::fs::write(out_dir.join("metrics.json"), json);
    let log_content = metrics.log.join("\n") + "\n";
    let _ = std::fs::write(out_dir.join("run.log"), log_content);
}

fn keycode_to_key(code: KeyCode) -> Key {
    match code {
        KeyCode::Escape => Key::Escape,
        KeyCode::Enter => Key::Enter,
        KeyCode::Tab => Key::Tab,
        KeyCode::Space => Key::Space,
        KeyCode::ArrowUp => Key::ArrowUp,
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::Backspace => Key::Backspace,
        _ => Key::Unidentified(NativeKey::Unidentified),
    }
}

fn char_to_keycode(c: char) -> KeyCode {
    match c.to_ascii_uppercase() {
        'A' => KeyCode::KeyA,
        'B' => KeyCode::KeyB,
        'C' => KeyCode::KeyC,
        'D' => KeyCode::KeyD,
        'E' => KeyCode::KeyE,
        'F' => KeyCode::KeyF,
        'G' => KeyCode::KeyG,
        'H' => KeyCode::KeyH,
        'I' => KeyCode::KeyI,
        'J' => KeyCode::KeyJ,
        'K' => KeyCode::KeyK,
        'L' => KeyCode::KeyL,
        'M' => KeyCode::KeyM,
        'N' => KeyCode::KeyN,
        'O' => KeyCode::KeyO,
        'P' => KeyCode::KeyP,
        'Q' => KeyCode::KeyQ,
        'R' => KeyCode::KeyR,
        'S' => KeyCode::KeyS,
        'T' => KeyCode::KeyT,
        'U' => KeyCode::KeyU,
        'V' => KeyCode::KeyV,
        'W' => KeyCode::KeyW,
        'X' => KeyCode::KeyX,
        'Y' => KeyCode::KeyY,
        'Z' => KeyCode::KeyZ,
        '0' => KeyCode::Digit0,
        '1' => KeyCode::Digit1,
        '2' => KeyCode::Digit2,
        '3' => KeyCode::Digit3,
        '4' => KeyCode::Digit4,
        '5' => KeyCode::Digit5,
        '6' => KeyCode::Digit6,
        '7' => KeyCode::Digit7,
        '8' => KeyCode::Digit8,
        '9' => KeyCode::Digit9,
        ' ' => KeyCode::Space,
        '\n' => KeyCode::Enter,
        '-' | '_' => KeyCode::Minus,
        _ => KeyCode::KeyA,
    }
}

pub fn track_enemy_hits(
    msgs: Option<MessageReader<crate::view::enemies::EnemyMsg>>,
    tracker: Option<ResMut<HitTracker>>,
) {
    let (Some(mut msgs), Some(mut tracker)) = (msgs, tracker) else {
        return;
    };
    for msg in msgs.read() {
        if let crate::view::enemies::EnemyMsg::Moved { id, hp, .. } = *msg {
            if let Some(&prev) = tracker.last_hp.get(&id) {
                if hp < prev && prev != i32::MAX {
                    tracker.hits += 1;
                }
            }
            tracker.last_hp.insert(id, hp);
        }
    }
}

#[derive(SystemParam)]
pub struct QaInputWriters<'w> {
    pub key: Option<MessageWriter<'w, KeyboardInput>>,
    pub mouse_button: Option<MessageWriter<'w, MouseButtonInput>>,
    pub mouse_motion: Option<MessageWriter<'w, MouseMotion>>,
    pub cursor_moved: Option<MessageWriter<'w, bevy::window::CursorMoved>>,
    pub exit: Option<MessageWriter<'w, AppExit>>,
}

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct QaTestFixture;

#[derive(SystemParam)]
pub struct QaWorldState<'w> {
    pub player: Option<Res<'w, crate::view::player::LocalPlayer>>,
    pub inventory: Option<Res<'w, crate::net::LocalInventory>>,
    pub clock: Option<ResMut<'w, crate::net::ClientClock>>,
    pub boss: Option<ResMut<'w, crate::hud::boss_bar::BossState>>,
    pub music: Option<Res<'w, crate::audio::music::MusicManager>>,
    pub tracker: Option<Res<'w, HitTracker>>,
    pub pending_shots: Option<ResMut<'w, PendingScreenshots>>,
    pub test_fixture: Option<Res<'w, QaTestFixture>>,
}

pub fn qa_driver(
    mut commands: Commands,
    mut qa_run: ResMut<QaRun>,
    mut qa_metrics: ResMut<QaMetrics>,
    plan: Option<Res<QaPlan>>,
    time: Option<Res<Time<Real>>>,
    mut next_app_state: Option<ResMut<NextState<AppState>>>,
    app_state: Option<Res<State<AppState>>>,
    menu_screen: Option<Res<State<MenuScreen>>>,
    menu_texts: Query<&crate::menu::MenuText>,
    hud_texts: Query<&crate::hud::text::HudText>,
    net: Option<Res<crate::net::Net>>,
    mut windows: Query<(Entity, &mut Window, Option<&PrimaryWindow>)>,
    mut writers: QaInputWriters,
    mut state: QaWorldState,
) {
    if qa_run.step == FAILED_STEP {
        return;
    }

    let delta = time.as_ref().map(|t| t.delta_secs()).unwrap_or(0.01666);
    qa_metrics.frames_ms.push(delta * 1000.0);

    let scenario = if qa_run.scenario < SCENARIOS.len() {
        &SCENARIOS[qa_run.scenario]
    } else {
        &SCENARIOS[0]
    };

    if qa_run.phase == 0 {
        let disc = discovery_output(scenario);
        println!("{disc}");
        qa_metrics.log.push(disc);
        if let Some(ref mut ex) = writers.exit {
            ex.write(AppExit::Success);
        }
        return;
    }

    // Re-entering InGame while already there would tear the session down and start a
    // second one, leaving a stale `SessionStopping` that freezes the Game Menu.
    let already_ingame = app_state
        .as_ref()
        .is_some_and(|s| *s.get() == AppState::InGame);
    if qa_run.step == 0 && qa_run.timer == 0.0 && scenario.direct_ingame && !already_ingame {
        if let Some(ref mut state) = next_app_state {
            state.set(AppState::InGame);
        }
    }

    let Some(plan) = plan else { return };
    let steps = &plan.0;

    if qa_run.step >= steps.len() {
        if state.pending_shots.as_ref().is_some_and(|p| p.pending > 0) {
            if qa_run.timer < screenshot_timeout_secs() {
                qa_run.timer += delta;
                return;
            } else {
                let err = "qa: pending screenshots timed out at phase completion".to_string();
                qa_metrics.log.push(err.clone());
                eprintln!("{err}");
                write_artifacts(&qa_run.out, &qa_metrics);
                if let Some(ref mut ex) = writers.exit {
                    ex.write(AppExit::error());
                }
                qa_run.step = FAILED_STEP;
                return;
            }
        }
        write_artifacts(&qa_run.out, &qa_metrics);
        if let Some(ref mut ex) = writers.exit {
            ex.write(AppExit::Success);
        }
        return;
    }

    let step = &steps[qa_run.step];

    // Find primary or first window
    let mut primary_win = None;
    let mut first_win = None;
    for (entity, win, prim) in &mut windows {
        if prim.is_some() {
            primary_win = Some((entity, win));
            break;
        } else if first_win.is_none() {
            first_win = Some((entity, win));
        }
    }
    let mut primary_win = primary_win.or(first_win);

    match step {
        Step::Wait(duration) => {
            qa_run.timer += delta;
            if qa_run.timer >= *duration {
                qa_metrics.log.push(format!(
                    "step {}: wait {:.2}s completed",
                    qa_run.step, duration
                ));
                qa_run.step += 1;
                qa_run.timer = 0.0;
            }
        }
        Step::Key(key_code) => {
            let win_entity = primary_win
                .as_ref()
                .map(|(e, _)| *e)
                .unwrap_or(Entity::PLACEHOLDER);
            if qa_run.timer == 0.0 {
                if let Some(ref mut kw) = writers.key {
                    kw.write(KeyboardInput {
                        key_code: *key_code,
                        logical_key: keycode_to_key(*key_code),
                        state: ButtonState::Pressed,
                        window: win_entity,
                        repeat: false,
                        text: None,
                    });
                }
                qa_run.timer = 1.0;
            } else if qa_run.timer == 1.0 {
                if let Some(ref mut kw) = writers.key {
                    kw.write(KeyboardInput {
                        key_code: *key_code,
                        logical_key: keycode_to_key(*key_code),
                        state: ButtonState::Released,
                        window: win_entity,
                        repeat: false,
                        text: None,
                    });
                }
                qa_run.timer = 2.0;
            } else {
                // Settling frame: input systems processed Released in frame 2;
                // State transitions and UI spawns have completed.
                qa_metrics
                    .log
                    .push(format!("step {}: key {:?} tapped", qa_run.step, key_code));
                qa_run.step += 1;
                qa_run.timer = 0.0;
            }
        }
        Step::Hold(key_code, duration) => {
            let win_entity = primary_win
                .as_ref()
                .map(|(e, _)| *e)
                .unwrap_or(Entity::PLACEHOLDER);
            if qa_run.timer == 0.0 {
                if let Some(ref mut kw) = writers.key {
                    kw.write(KeyboardInput {
                        key_code: *key_code,
                        logical_key: keycode_to_key(*key_code),
                        state: ButtonState::Pressed,
                        window: win_entity,
                        repeat: false,
                        text: None,
                    });
                }
            }
            qa_run.timer += delta;
            if qa_run.timer >= *duration {
                if let Some(ref mut kw) = writers.key {
                    kw.write(KeyboardInput {
                        key_code: *key_code,
                        logical_key: keycode_to_key(*key_code),
                        state: ButtonState::Released,
                        window: win_entity,
                        repeat: false,
                        text: None,
                    });
                }
                qa_metrics.log.push(format!(
                    "step {}: hold {:?} for {:.2}s completed",
                    qa_run.step, key_code, duration
                ));
                qa_run.step += 1;
                qa_run.timer = 0.0;
            }
        }
        Step::Look(dx, dy) => {
            if let Some(ref mut mw) = writers.mouse_motion {
                mw.write(MouseMotion {
                    delta: Vec2::new(*dx, *dy),
                });
            }
            qa_metrics
                .log
                .push(format!("step {}: look ({:.2}, {:.2})", qa_run.step, dx, dy));
            qa_run.step += 1;
            qa_run.timer = 0.0;
        }
        Step::Click { x, y } | Step::ShiftClick { x, y } | Step::RightClick { x, y } => {
            let shift = matches!(step, Step::ShiftClick { .. });
            let button = if matches!(step, Step::RightClick { .. }) {
                MouseButton::Right
            } else {
                MouseButton::Left
            };
            let label = if button == MouseButton::Right {
                "right click"
            } else {
                "click"
            };
            let (win_entity, win_ref) = match primary_win.as_mut() {
                Some((e, w)) => (*e, Some(w)),
                None => (Entity::PLACEHOLDER, None),
            };
            let point = win_ref.as_ref().map_or(Vec2::new(*x, *y), |w| {
                scale_click(*x, *y, Vec2::new(w.width(), w.height()))
            });
            if let Some(win) = win_ref {
                win.set_cursor_position(Some(point));
            }
            if let Some(ref mut cm) = writers.cursor_moved {
                cm.write(bevy::window::CursorMoved {
                    window: win_entity,
                    position: point,
                    delta: None,
                });
            }
            if qa_run.timer == 0.0 {
                if shift && let Some(ref mut kw) = writers.key {
                    kw.write(KeyboardInput {
                        key_code: KeyCode::ShiftLeft,
                        logical_key: Key::Shift,
                        state: ButtonState::Pressed,
                        window: win_entity,
                        repeat: false,
                        text: None,
                    });
                }
                if let Some(ref mut mw) = writers.mouse_button {
                    mw.write(MouseButtonInput {
                        button,
                        state: ButtonState::Pressed,
                        window: win_entity,
                    });
                }
                qa_run.timer = 1.0;
            } else if qa_run.timer == 1.0 {
                if let Some(ref mut mw) = writers.mouse_button {
                    mw.write(MouseButtonInput {
                        button,
                        state: ButtonState::Released,
                        window: win_entity,
                    });
                }
                if shift && let Some(ref mut kw) = writers.key {
                    kw.write(KeyboardInput {
                        key_code: KeyCode::ShiftLeft,
                        logical_key: Key::Shift,
                        state: ButtonState::Released,
                        window: win_entity,
                        repeat: false,
                        text: None,
                    });
                }
                qa_run.timer = 2.0;
            } else {
                // Settling frame
                qa_metrics.log.push(format!(
                    "step {}: {label} ({:.1}, {:.1}) -> window ({:.1}, {:.1})",
                    qa_run.step, x, y, point.x, point.y
                ));
                qa_run.step += 1;
                qa_run.timer = 0.0;
            }
        }
        Step::Type(text) => {
            let chars: Vec<char> = text.chars().collect();
            let win_entity = primary_win
                .as_ref()
                .map(|(e, _)| *e)
                .unwrap_or(Entity::PLACEHOLDER);
            let idx = (qa_run.timer as usize) / 2;
            if idx < chars.len() {
                let c = chars[idx];
                let is_pressed = (qa_run.timer as usize) % 2 == 0;
                let state = if is_pressed {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                };
                if let Some(ref mut kw) = writers.key {
                    kw.write(KeyboardInput {
                        key_code: char_to_keycode(c),
                        logical_key: Key::Character(c.to_string().into()),
                        state,
                        window: win_entity,
                        repeat: false,
                        text: Some(c.to_string().into()),
                    });
                }
                qa_run.timer += 1.0;
                if !is_pressed && idx + 1 == chars.len() {
                    qa_metrics
                        .log
                        .push(format!("step {}: typed {:?}", qa_run.step, text));
                    qa_run.step += 1;
                    qa_run.timer = 0.0;
                }
            } else {
                qa_run.step += 1;
                qa_run.timer = 0.0;
            }
        }
        Step::Screenshot(name) => {
            let shot_path = qa_run.out.join(format!("{name}.png"));
            let is_test_stub =
                state.test_fixture.is_some() && (name.starts_with("test_") || *name == "test_shot");

            if qa_run.timer == 0.0 {
                if let Some(parent) = shot_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Some(ref p) = state.player {
                    let pos = p.0.body.pos;
                    qa_metrics.named.push((format!("{name}_pos_x"), pos[0]));
                    qa_metrics.named.push((format!("{name}_pos_y"), pos[1]));
                    qa_metrics.named.push((format!("{name}_pos_z"), pos[2]));
                    qa_metrics.log.push(format!(
                        "step {}: screenshot {} at [{:.2}, {:.2}, {:.2}]",
                        qa_run.step, name, pos[0], pos[1], pos[2]
                    ));
                } else {
                    qa_metrics.log.push(format!(
                        "step {}: screenshot {} (no player)",
                        qa_run.step, name
                    ));
                }

                if primary_win.is_some() {
                    if let Some(ref mut p) = state.pending_shots {
                        p.pending += 1;
                    }
                    commands
                        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                        .observe(bevy::render::view::screenshot::save_to_disk(shot_path.clone()))
                        .observe(
                            |_: On<bevy::render::view::screenshot::ScreenshotCaptured>,
                             mut pending: Option<ResMut<PendingScreenshots>>| {
                                if let Some(ref mut p) = pending {
                                    p.pending = p.pending.saturating_sub(1);
                                }
                            },
                        );
                    qa_run.timer = 0.001;
                } else if is_test_stub {
                    let _ = std::fs::write(&shot_path, DUMMY_PNG);
                    qa_metrics.log.push(format!(
                        "step {}: screenshot {} (test fixture stub)",
                        qa_run.step, name
                    ));
                    qa_run.step += 1;
                    qa_run.timer = 0.0;
                } else {
                    let err = format!(
                        "step {}: screenshot {} failed: no primary window available",
                        qa_run.step, name
                    );
                    qa_metrics.log.push(err.clone());
                    eprintln!("qa: {err}");
                    write_artifacts(&qa_run.out, &qa_metrics);
                    if let Some(ref mut ex) = writers.exit {
                        ex.write(AppExit::error());
                    }
                    qa_run.step = FAILED_STEP;
                    return;
                }
            } else {
                qa_run.timer += delta;
                let captured = state.pending_shots.as_ref().is_some_and(|p| p.pending == 0);
                if captured {
                    let valid = shot_path.exists()
                        && (is_test_stub
                            || shot_path.metadata().map(|m| m.len() > 100).unwrap_or(false));
                    if valid {
                        let len = shot_path.metadata().map(|m| m.len()).unwrap_or(0);
                        qa_metrics.log.push(format!(
                            "step {}: screenshot {} captured ({} bytes)",
                            qa_run.step, name, len
                        ));
                        qa_run.step += 1;
                        qa_run.timer = 0.0;
                    } else {
                        let err = format!(
                            "step {}: screenshot {} captured event received but file is invalid or empty",
                            qa_run.step, name
                        );
                        qa_metrics.log.push(err.clone());
                        eprintln!("qa: {err}");
                        write_artifacts(&qa_run.out, &qa_metrics);
                        if let Some(ref mut ex) = writers.exit {
                            ex.write(AppExit::error());
                        }
                        qa_run.step = FAILED_STEP;
                    }
                } else if qa_run.timer >= screenshot_timeout_secs() {
                    let err = format!(
                        "step {}: screenshot {} timed out after {:.1}s waiting for GPU capture",
                        qa_run.step, name, qa_run.timer
                    );
                    qa_metrics.log.push(err.clone());
                    eprintln!("qa: {err}");
                    write_artifacts(&qa_run.out, &qa_metrics);
                    if let Some(ref mut ex) = writers.exit {
                        ex.write(AppExit::error());
                    }
                    qa_run.step = FAILED_STEP;
                }
            }
        }
        Step::Expect(expect) => {
            let success = match expect {
                Expect::InGame(want) => {
                    let is_ingame = app_state
                        .as_ref()
                        .is_some_and(|s| *s.get() == AppState::InGame);
                    is_ingame == *want
                }
                Expect::Screen(name) => menu_screen
                    .as_ref()
                    .is_some_and(|s| format!("{:?}", s.get()).eq_ignore_ascii_case(name)),
                Expect::Text(expected_text) => {
                    menu_texts.iter().any(|t| t.text.contains(expected_text))
                        || hud_texts.iter().any(|t| t.text.contains(expected_text))
                }
                Expect::MetricGt(key, threshold) => qa_metrics
                    .named
                    .iter()
                    .rev()
                    .find(|(k, _)| k == key)
                    .is_some_and(|(_, val)| *val > *threshold),
                Expect::MetricLt(key, threshold) => qa_metrics
                    .named
                    .iter()
                    .rev()
                    .find(|(k, _)| k == key)
                    .is_some_and(|(_, val)| *val < *threshold),
                Expect::Music(want) => state
                    .music
                    .as_ref()
                    .and_then(|m| m.state.current)
                    .is_some_and(|s| format!("{s:?}").eq_ignore_ascii_case(want)),
                Expect::MusicSilent => state
                    .music
                    .as_ref()
                    .map_or(true, |m| m.current_entity.is_none()),
            };
            if success {
                qa_metrics
                    .log
                    .push(format!("step {}: expect passed: {:?}", qa_run.step, expect));
                qa_run.step += 1;
                qa_run.timer = 0.0;
            } else {
                let err = format!("step {}: expectation failed: {:?}", qa_run.step, expect);
                qa_metrics.log.push(err.clone());
                eprintln!("qa: {err}");
                write_artifacts(&qa_run.out, &qa_metrics);
                if let Some(ref mut ex) = writers.exit {
                    ex.write(AppExit::error());
                }
                qa_run.step = FAILED_STEP;
            }
        }
        Step::Metric(name, source) => {
            let val = match source {
                MetricSource::Pos => state
                    .player
                    .as_ref()
                    .map(|p| p.0.body.pos[0])
                    .unwrap_or(0.0),
                MetricSource::MaxPlayerY => state
                    .player
                    .as_ref()
                    .map(|p| p.0.body.fall_peak_y)
                    .unwrap_or(0.0),
                MetricSource::Vel => state
                    .player
                    .as_ref()
                    .map(|p| {
                        let v = p.0.body.vel;
                        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
                    })
                    .unwrap_or(0.0),
                MetricSource::HorizVel => state
                    .player
                    .as_ref()
                    .map(|p| {
                        let v = p.0.body.vel;
                        (v[0] * v[0] + v[2] * v[2]).sqrt()
                    })
                    .unwrap_or(0.0),
                MetricSource::Life => state
                    .player
                    .as_ref()
                    .map(|p| p.0.vitals.life as f32)
                    .unwrap_or(0.0),
                MetricSource::InvCount => state
                    .inventory
                    .as_ref()
                    .map(|inv| inv.0.slots.iter().flatten().map(|s| s.count as f32).sum())
                    .unwrap_or(0.0),
                MetricSource::Clock => state.clock.as_ref().map(|c| c.tick as f32).unwrap_or(0.0),
                MetricSource::Hits => state.tracker.as_ref().map(|t| t.hits as f32).unwrap_or(0.0),
                MetricSource::Breath => state
                    .player
                    .as_ref()
                    .map(|p| p.0.vitals.breath as f32)
                    .unwrap_or(200.0),
                MetricSource::SpeedRatio => {
                    let dry = qa_metrics
                        .named
                        .iter()
                        .rev()
                        .find(|(k, _)| k == "speed_dry")
                        .map(|(_, v)| *v)
                        .unwrap_or(0.128);
                    let cur = state
                        .player
                        .as_ref()
                        .map(|p| {
                            let v = p.0.body.vel;
                            (v[0] * v[0] + v[2] * v[2]).sqrt()
                        })
                        .unwrap_or(0.0);
                    if dry > 0.001 { cur / dry } else { 0.5 }
                }
            };
            qa_metrics.named.push((name.to_string(), val));
            qa_metrics.log.push(format!(
                "step {}: metric {} = {:.2}",
                qa_run.step, name, val
            ));
            qa_run.step += 1;
            qa_run.timer = 0.0;
        }
        Step::SetClock(tick) => {
            if let Some(ref mut c) = state.clock {
                c.tick = *tick;
                c.override_tick = Some(*tick);
            }
            qa_metrics
                .log
                .push(format!("step {}: set clock to {}", qa_run.step, tick));
            qa_run.step += 1;
            qa_run.timer = 0.0;
        }
        Step::SpawnDemoBoss => {
            if let Some(ref mut b) = state.boss {
                b.set(crate::hud::boss_bar::BossInfo {
                    pos: [0.0, 50.0, 0.0],
                    vel: [0.0, 0.0, 0.0],
                    life: 2800,
                    life_max: 2800,
                    phase: 0,
                    state: 0,
                    spin: 0.0,
                });
            }
            qa_metrics
                .log
                .push(format!("step {}: spawned demo boss", qa_run.step));
            qa_run.step += 1;
            qa_run.timer = 0.0;
        }
        Step::Kill => match net.as_ref().map(|n| n.send(ClientMsg::QaKill)) {
            Some(Ok(())) => {
                qa_metrics
                    .log
                    .push(format!("step {}: qa kill requested", qa_run.step));
                qa_run.step += 1;
                qa_run.timer = 0.0;
            }
            Some(Err(e)) => {
                let err = format!("qa: Step::Kill send failed: {e:?}");
                qa_metrics.log.push(err.clone());
                eprintln!("{err}");
                write_artifacts(&qa_run.out, &qa_metrics);
                if let Some(ref mut ex) = writers.exit {
                    ex.write(AppExit::error());
                }
                qa_run.step = FAILED_STEP;
            }
            None => {
                let err = "qa: Step::Kill requires Net".to_string();
                qa_metrics.log.push(err.clone());
                eprintln!("{err}");
                write_artifacts(&qa_run.out, &qa_metrics);
                if let Some(ref mut ex) = writers.exit {
                    ex.write(AppExit::error());
                }
                qa_run.step = FAILED_STEP;
            }
        },
        Step::Respawn => match net.as_ref().map(|n| n.send(ClientMsg::Respawn)) {
            Some(Ok(())) => {
                qa_metrics
                    .log
                    .push(format!("step {}: respawn requested", qa_run.step));
                qa_run.step += 1;
                qa_run.timer = 0.0;
            }
            Some(Err(e)) => {
                let err = format!("qa: Step::Respawn send failed: {e:?}");
                qa_metrics.log.push(err.clone());
                eprintln!("{err}");
                write_artifacts(&qa_run.out, &qa_metrics);
                if let Some(ref mut ex) = writers.exit {
                    ex.write(AppExit::error());
                }
                qa_run.step = FAILED_STEP;
            }
            None => {
                let err = "qa: Step::Respawn requires Net".to_string();
                qa_metrics.log.push(err.clone());
                eprintln!("{err}");
                write_artifacts(&qa_run.out, &qa_metrics);
                if let Some(ref mut ex) = writers.exit {
                    ex.write(AppExit::error());
                }
                qa_run.step = FAILED_STEP;
            }
        },
    }
}

pub fn qa_on_exit(
    qa_run: Res<QaRun>,
    mut qa_metrics: ResMut<QaMetrics>,
    perf: Option<Res<PerfCounters>>,
    exits: Option<MessageReader<AppExit>>,
) {
    if qa_run.step != FAILED_STEP && exits.as_ref().is_some_and(|r| !r.is_empty()) {
        if let Some(perf) = perf {
            qa_metrics
                .named
                .push(("particles_active".to_string(), perf.particles_active as f32));
            qa_metrics
                .named
                .push(("critters_active".to_string(), perf.critters_active as f32));
            qa_metrics
                .named
                .push(("map_rebuild_ms".to_string(), perf.map_rebuild_ms));
        }
        write_artifacts(&qa_run.out, &qa_metrics);
    }
}

pub struct QaPlugin {
    pub scenario: String,
    pub phase: u8,
    pub out: PathBuf,
    pub custom_steps: Option<Vec<Step>>,
}

impl QaPlugin {
    pub fn new(scenario: impl Into<String>, phase: u8, out: impl Into<PathBuf>) -> Self {
        Self {
            scenario: scenario.into(),
            phase,
            out: out.into(),
            custom_steps: None,
        }
    }
}

impl Plugin for QaPlugin {
    fn build(&self, app: &mut App) {
        let scenario_idx = SCENARIOS
            .iter()
            .position(|s| s.name == self.scenario)
            .unwrap_or(0);
        let scenario = &SCENARIOS[scenario_idx];

        let steps = self
            .custom_steps
            .clone()
            .unwrap_or_else(|| default_steps(&self.scenario, self.phase));

        app.insert_resource(QaRun {
            scenario: scenario_idx,
            phase: self.phase,
            step: 0,
            timer: 0.0,
            out: self.out.clone(),
        });
        app.insert_resource(QaMetrics::default());
        if steps.iter().any(|s| matches!(s, Step::Kill)) {
            app.insert_resource(QaKillDemo);
        }
        app.insert_resource(QaPlan(steps));
        app.init_resource::<HitTracker>();
        app.init_resource::<PendingScreenshots>();
        app.init_resource::<PerfCounters>();
        if scenario.demo_enemies {
            app.insert_resource(QaCombatDemo);
        }
        if scenario.name == "liquids" {
            app.insert_resource(QaLiquidsDemo);
        }

        if scenario.direct_ingame {
            app.insert_resource(SelectedWorld {
                name: format!("qa-{}", scenario.name),
                seed: FIXTURE_SEED,
                size: terravibe_sim::WorldSize::Small,
                persist: scenario.fixture.is_some(),
            });
            if let Some(mut state) = app.world_mut().get_resource_mut::<NextState<AppState>>() {
                state.set(AppState::InGame);
            }
        }

        if let Some(fixture_kind) = scenario.fixture {
            if let Some(save_root) = app.world().get_resource::<crate::session::SaveRoot>() {
                let world_name = format!("qa-{}", scenario.name);
                if let Err(e) = fixture::build_fixture(fixture_kind, &save_root.0, &world_name) {
                    eprintln!("qa: failed to build fixture {fixture_kind:?}: {e}");
                }
            }
        }

        app.add_systems(Update, (track_enemy_hits, update_perf_counters, qa_driver));
        app.add_systems(PostUpdate, qa_on_exit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session;

    struct TempDirGuard(PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_dir(tag: &str) -> (PathBuf, TempDirGuard) {
        let p = std::env::temp_dir().join(format!("tv_qa_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("create temp dir");
        (p.clone(), TempDirGuard(p))
    }

    #[test]
    fn direct_ingame_scenario_does_not_reenter_ingame() {
        let (out, _guard) = test_dir("no_reenter");
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<session::AppState>();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);
        app.insert_resource(QaRun {
            scenario: SCENARIOS
                .iter()
                .position(|s| s.name == "visual_audit")
                .unwrap(),
            phase: 1,
            step: 0,
            timer: 0.0,
            out,
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(QaPlan(vec![Step::Wait(1.0)]));
        app.world_mut()
            .resource_mut::<NextState<session::AppState>>()
            .set(session::AppState::InGame);
        app.add_systems(Update, qa_driver);

        app.update();
        app.update();

        assert!(
            matches!(
                app.world().resource::<NextState<session::AppState>>(),
                NextState::Unchanged
            ),
            "the driver must not queue a second InGame transition"
        );
    }

    #[test]
    fn failed_expectation_exits_nonzero() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<session::AppState>();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);
        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out: PathBuf::from("/tmp"),
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(QaPlan(vec![Step::Expect(Expect::InGame(true))]));
        app.add_systems(Update, qa_driver);

        app.update();

        let exits = app.world().resource::<Messages<AppExit>>();
        let mut cursor = exits.get_cursor();
        let last_exit = cursor.read(exits).last();
        assert!(
            matches!(last_exit, Some(AppExit::Error(_))),
            "expected AppExit::Error, got {:?}",
            last_exit
        );
    }

    #[test]
    fn failed_expectation_sentinel_never_emits_success_on_subsequent_updates() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<session::AppState>();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);
        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out: PathBuf::from("/tmp"),
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(QaPlan(vec![Step::Expect(Expect::InGame(true))]));
        app.add_systems(Update, qa_driver);

        app.update();

        // On subsequent updates, AppExit::Success must NEVER be emitted
        for _ in 0..5 {
            app.update();
        }

        let exits = app.world().resource::<Messages<AppExit>>();
        let mut cursor = exits.get_cursor();
        let all_exits: Vec<_> = cursor.read(exits).cloned().collect();
        assert!(
            !all_exits.iter().any(|e| matches!(e, AppExit::Success)),
            "AppExit::Success was emitted after expectation failure: {:?}",
            all_exits
        );
    }

    #[test]
    fn successful_phase_exits_success_and_writes_artifacts() {
        let (out, _guard) = test_dir("succ");
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<session::AppState>();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);

        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out: out.clone(),
        });
        app.insert_resource(QaTestFixture);
        app.insert_resource(QaMetrics::default());
        app.insert_resource(QaPlan(vec![
            Step::Wait(0.0),
            Step::Screenshot("test_shot"),
            Step::Expect(Expect::InGame(true)),
        ]));
        app.world_mut()
            .resource_mut::<NextState<session::AppState>>()
            .set(session::AppState::InGame);
        app.add_systems(Update, qa_driver);

        for _ in 0..10 {
            app.update();
            let exits = app.world().resource::<Messages<AppExit>>();
            let mut cursor = exits.get_cursor();
            if cursor.read(exits).any(|e| matches!(e, AppExit::Success)) {
                break;
            }
        }

        let exits = app.world().resource::<Messages<AppExit>>();
        let mut cursor = exits.get_cursor();
        let last_exit = cursor.read(exits).last();
        assert!(
            matches!(last_exit, Some(AppExit::Success)),
            "expected AppExit::Success, got {:?}",
            last_exit
        );
        assert!(out.join("test_shot.png").exists(), "PNG must exist");
        assert!(out.join("metrics.json").exists(), "metrics.json must exist");
        assert!(out.join("run.log").exists(), "run.log must exist");
    }

    #[test]
    fn click_emits_press_then_release_across_two_frames() {
        let mut app = App::new();
        app.add_message::<MouseButtonInput>();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);

        let _win = app
            .world_mut()
            .spawn(Window {
                resolution: bevy::window::WindowResolution::new(1920, 1080),
                ..default()
            })
            .id();

        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out: PathBuf::from("/tmp"),
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(QaPlan(vec![Step::Click { x: 150.0, y: 250.0 }]));

        #[derive(Resource, Default)]
        struct ClickLog(Vec<(ButtonState, Option<Vec2>)>);

        app.init_resource::<ClickLog>();
        let log_clicks = |mut reader: MessageReader<MouseButtonInput>,
                          windows: Query<&Window>,
                          mut log: ResMut<ClickLog>| {
            for ev in reader.read() {
                let cursor = windows.single().ok().and_then(|w| w.cursor_position());
                log.0.push((ev.state, cursor));
            }
        };

        app.add_systems(Update, (qa_driver, log_clicks).chain());

        app.update();
        app.update();

        let log = app.world().resource::<ClickLog>();
        assert_eq!(log.0.len(), 2, "expected 2 events, got {:?}", log.0);
        assert_eq!(log.0[0].0, ButtonState::Pressed);
        assert_eq!(log.0[1].0, ButtonState::Released);
        assert_eq!(log.0[0].1, Some(Vec2::new(150.0, 250.0)));
        assert_eq!(log.0[1].1, Some(Vec2::new(150.0, 250.0)));
    }

    #[test]
    fn qa_click_scales_to_window_size() {
        // The 1920x1080 reference canvas centre maps to the centre of any window.
        let point = Vec2::new(960.0, 540.0);
        assert_eq!(
            scale_click(point.x, point.y, Vec2::new(1920.0, 1080.0)),
            point
        );

        let mut app = App::new();
        app.add_message::<MouseButtonInput>();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);

        app.world_mut().spawn(Window {
            resolution: bevy::window::WindowResolution::new(1280, 720),
            ..default()
        });

        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out: PathBuf::from("/tmp"),
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(QaPlan(vec![Step::Click { x: 960.0, y: 540.0 }]));

        #[derive(Resource, Default)]
        struct ClickLog(Vec<(ButtonState, Option<Vec2>)>);

        app.init_resource::<ClickLog>();
        let log_clicks = |mut reader: MessageReader<MouseButtonInput>,
                          windows: Query<&Window>,
                          mut log: ResMut<ClickLog>| {
            for ev in reader.read() {
                let cursor = windows.single().ok().and_then(|w| w.cursor_position());
                log.0.push((ev.state, cursor));
            }
        };

        app.add_systems(Update, (qa_driver, log_clicks).chain());

        app.update();
        app.update();

        let log = app.world().resource::<ClickLog>();
        let expected = Vec2::new(640.0, 360.0);
        assert_eq!(
            log.0,
            vec![
                (ButtonState::Pressed, Some(expected)),
                (ButtonState::Released, Some(expected))
            ],
            "960x540 on the 1920x1080 canvas must land at 640x360 in a 1280x720 window"
        );

        // Same relative spot: the scaled point keeps the reference proportions.
        let rel = expected / Vec2::new(1280.0, 720.0);
        assert!((rel.x - 0.5).abs() < f32::EPSILON && (rel.y - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn qa_click_scales_physical_size_on_hidpi() {
        // Reference 960x540 on a 1280x720 window at scale factor 2.0:
        // logical canvas 640x360 -> (320, 180), and `set_cursor_position`
        // derives the physical position (640, 360) via the scale factor.
        let mut app = App::new();
        app.add_message::<MouseButtonInput>();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);

        let window = app
            .world_mut()
            .spawn(Window {
                resolution: bevy::window::WindowResolution::new(1280, 720)
                    .with_scale_factor_override(2.0),
                ..default()
            })
            .id();

        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out: PathBuf::from("/tmp"),
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(QaPlan(vec![Step::Click { x: 960.0, y: 540.0 }]));
        app.add_systems(Update, qa_driver);

        app.update();

        let win = app.world().get::<Window>(window).unwrap();
        assert_eq!(
            win.physical_cursor_position(),
            Some(Vec2::new(640.0, 360.0)),
            "set_cursor_position must derive the physical position from the scale factor"
        );
        assert_eq!(
            win.cursor_position(),
            Some(Vec2::new(320.0, 180.0)),
            "the logical cursor must keep the same relative spot"
        );
    }

    #[test]
    fn qa_driver_executes_kill_and_respawn_steps() {
        use std::sync::{Arc, Mutex};

        #[derive(Clone)]
        struct RecordingTransport(Arc<Mutex<Vec<ClientMsg>>>);
        impl terravibe_sim::net::ClientTransport for RecordingTransport {
            fn send(&self, m: ClientMsg) -> Result<(), terravibe_sim::net::Disconnected> {
                self.0.lock().unwrap().push(m);
                Ok(())
            }
            fn try_recv(
                &self,
            ) -> Result<Option<terravibe_sim::net::ServerMsg>, terravibe_sim::net::Disconnected>
            {
                Ok(None)
            }
        }

        let sent = Arc::new(Mutex::new(Vec::new()));
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<session::AppState>();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);
        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out: PathBuf::from("/tmp"),
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(crate::net::Net::new(RecordingTransport(sent.clone())));
        app.insert_resource(QaPlan(vec![Step::Kill, Step::Respawn]));
        app.add_systems(Update, qa_driver);

        // The driver advances one step per update.
        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<QaRun>().step,
            2,
            "both steps must run"
        );
        let msgs = sent.lock().unwrap();
        assert_eq!(msgs.len(), 2, "expected QaKill then Respawn, got {msgs:?}");
        assert!(
            matches!(msgs[0], ClientMsg::QaKill),
            "first message must be QaKill, got {:?}",
            msgs[0]
        );
        assert!(
            matches!(msgs[1], ClientMsg::Respawn),
            "second message must be Respawn, got {:?}",
            msgs[1]
        );
    }

    #[test]
    fn kill_step_fails_loud_without_net() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<session::AppState>();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);
        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out: PathBuf::from("/tmp"),
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(QaPlan(vec![Step::Kill]));
        app.add_systems(Update, qa_driver);

        app.update();

        assert_eq!(app.world().resource::<QaRun>().step, FAILED_STEP);
        let exits = app.world().resource::<Messages<AppExit>>();
        let mut cursor = exits.get_cursor();
        let last_exit = cursor.read(exits).last();
        assert!(
            matches!(last_exit, Some(AppExit::Error(_))),
            "expected AppExit::Error, got {:?}",
            last_exit
        );
    }

    #[test]
    fn respawn_step_fails_loud_when_send_fails() {
        struct FailingTransport;
        impl terravibe_sim::net::ClientTransport for FailingTransport {
            fn send(&self, _m: ClientMsg) -> Result<(), terravibe_sim::net::Disconnected> {
                Err(terravibe_sim::net::Disconnected)
            }
            fn try_recv(
                &self,
            ) -> Result<Option<terravibe_sim::net::ServerMsg>, terravibe_sim::net::Disconnected>
            {
                Ok(None)
            }
        }

        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<session::AppState>();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);
        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out: PathBuf::from("/tmp"),
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(crate::net::Net::new(FailingTransport));
        app.insert_resource(QaPlan(vec![Step::Respawn]));
        app.add_systems(Update, qa_driver);

        app.update();

        assert_eq!(app.world().resource::<QaRun>().step, FAILED_STEP);
        let exits = app.world().resource::<Messages<AppExit>>();
        let mut cursor = exits.get_cursor();
        let last_exit = cursor.read(exits).last();
        assert!(
            matches!(last_exit, Some(AppExit::Error(_))),
            "expected AppExit::Error, got {:?}",
            last_exit
        );
    }

    #[test]
    fn right_click_emits_press_then_release_across_two_frames() {
        let mut app = App::new();
        app.add_message::<MouseButtonInput>();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);

        let _win = app
            .world_mut()
            .spawn(Window {
                resolution: bevy::window::WindowResolution::new(1920, 1080),
                ..default()
            })
            .id();

        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out: PathBuf::from("/tmp"),
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(QaPlan(vec![Step::RightClick { x: 150.0, y: 250.0 }]));

        #[derive(Resource, Default)]
        struct ClickLog(Vec<(MouseButton, ButtonState, Option<Vec2>)>);

        app.init_resource::<ClickLog>();
        let log_clicks = |mut reader: MessageReader<MouseButtonInput>,
                          windows: Query<&Window>,
                          mut log: ResMut<ClickLog>| {
            for ev in reader.read() {
                let cursor = windows.single().ok().and_then(|w| w.cursor_position());
                log.0.push((ev.button, ev.state, cursor));
            }
        };

        app.add_systems(Update, (qa_driver, log_clicks).chain());

        app.update();
        app.update();

        let log = app.world().resource::<ClickLog>();
        assert_eq!(log.0.len(), 2, "expected 2 events, got {:?}", log.0);
        assert_eq!(log.0[0].0, MouseButton::Right);
        assert_eq!(log.0[0].1, ButtonState::Pressed);
        assert_eq!(log.0[1].0, MouseButton::Right);
        assert_eq!(log.0[1].1, ButtonState::Released);
        assert_eq!(log.0[0].2, Some(Vec2::new(150.0, 250.0)));
        assert_eq!(log.0[1].2, Some(Vec2::new(150.0, 250.0)));
    }

    #[test]
    fn first_hour_scenario_is_registered() {
        let sc = SCENARIOS.iter().find(|s| s.name == "first_hour");
        assert!(
            sc.is_some(),
            "first_hour scenario must be present in SCENARIOS"
        );
        let sc = sc.unwrap();
        assert!(sc.direct_ingame, "first_hour must start in game");
        assert_eq!(sc.fixture, Some(FixtureKind::Housing));
        assert!(!sc.demo_enemies);
        assert_eq!(sc.phases, 1);

        let steps = super::scenarios::scenario_steps("first_hour", 1);
        assert!(steps.contains(&Step::Kill));
        assert!(steps.contains(&Step::Respawn));
        assert!(steps.contains(&Step::Expect(Expect::Text("Welcome to TerraVibe"))));
        assert!(steps.contains(&Step::Screenshot("first_hour_onboarding")));
        assert!(steps.contains(&Step::Screenshot("first_hour_death")));
        assert!(steps.contains(&Step::Screenshot("first_hour_respawned")));
    }

    #[test]
    fn visual_audit_uses_flat_fixture() {
        let sc = SCENARIOS
            .iter()
            .find(|s| s.name == "visual_audit")
            .expect("visual_audit scenario must be registered");
        assert_eq!(
            sc.fixture,
            Some(FixtureKind::Flat),
            "visual_audit must pre-build flat chunks so generation cannot time out"
        );
    }

    #[test]
    fn qa_combat_scenario_enables_demo_enemies() {
        let (out, _guard) = test_dir("combat_demo");
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<session::AppState>();
        app.add_plugins(QaPlugin::new("combat", 1, out));

        let combat = SCENARIOS
            .iter()
            .find(|s| s.name == "combat")
            .expect("combat scenario must be registered");
        assert!(
            combat.demo_enemies,
            "combat scenario must request demo enemies"
        );
        assert!(
            app.world().contains_resource::<QaCombatDemo>(),
            "QaPlugin must insert QaCombatDemo for the combat scenario"
        );

        // The session turns the marker into ServerCfg::demo_enemies.
        let qa_combat = app.world().contains_resource::<QaCombatDemo>();
        let cfg = session::session_cfg(
            "qa-combat",
            false,
            PathBuf::from("/tmp/terravibe-qa-combat"),
            qa_combat,
            false,
            false,
            false,
            None,
        );
        assert!(cfg.demo_enemies);
        assert_eq!(cfg.clock_start, terravibe_sim::time::DAY_TICKS);
    }

    #[test]
    fn qa_plugin_inserts_kill_marker_for_first_hour() {
        let (out, _guard) = test_dir("kill_demo");
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<session::AppState>();
        app.add_plugins(QaPlugin::new("first_hour", 1, out));

        assert!(
            app.world().contains_resource::<QaKillDemo>(),
            "QaPlugin must insert QaKillDemo when the scenario kills the player"
        );

        // The session turns the marker into ServerCfg::qa_kill.
        let qa_kill = app.world().contains_resource::<QaKillDemo>();
        let cfg = session::session_cfg(
            "qa-first_hour",
            false,
            PathBuf::from("/tmp/terravibe-qa-first-hour"),
            false,
            false,
            qa_kill,
            false,
            None,
        );
        assert!(cfg.qa_kill);
    }

    #[test]
    fn qa_plugin_does_not_enable_qa_kill_without_a_kill_step() {
        let (out, _guard) = test_dir("no_kill_demo");
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<session::AppState>();
        app.add_plugins(QaPlugin::new("menu_title", 1, out));

        assert!(!app.world().contains_resource::<QaKillDemo>());
    }

    #[test]
    fn menu_scenario_does_not_enable_demo_enemies() {
        let (out, _guard) = test_dir("menu_no_demo");
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<session::AppState>();
        app.add_plugins(QaPlugin::new("menu_title", 1, out));

        assert!(!app.world().contains_resource::<QaCombatDemo>());
    }

    #[test]
    fn metrics_json_contains_frame_ms_named_and_log() {
        let metrics = QaMetrics {
            frames_ms: vec![16.6, 16.7],
            named: vec![("jump_peak_y".to_string(), 42.5)],
            log: vec!["phase started".to_string(), "phase finished".to_string()],
        };
        let json = metrics_json(&metrics);
        assert!(json.trim().starts_with('{'), "must start with '{{': {json}");
        assert!(json.trim().ends_with('}'), "must end with '}}': {json}");
        assert!(
            json.contains("\"frame_ms\""),
            "must contain frame_ms: {json}"
        );
        assert!(
            json.contains("\"jump_peak_y\""),
            "must contain named metric: {json}"
        );
        assert!(json.contains("42.5"), "must contain metric value: {json}");
        assert!(
            json.contains("phase started"),
            "must contain log entry: {json}"
        );
        assert!(
            json.contains("phase finished"),
            "must contain log entry: {json}"
        );
    }

    #[test]
    fn scenarios_have_unique_names_and_phases() {
        let mut names = std::collections::HashSet::new();
        for s in SCENARIOS {
            assert!(names.insert(s.name), "duplicate scenario name: {}", s.name);
            assert!(s.phases >= 1, "phases must be >= 1 for {}", s.name);
            if s.name.starts_with("menu_") {
                assert!(
                    !s.direct_ingame,
                    "menu scenario {} must have direct_ingame=false",
                    s.name
                );
            } else {
                assert!(
                    s.direct_ingame,
                    "gameplay scenario {} must have direct_ingame=true",
                    s.name
                );
            }
        }
    }

    #[test]
    fn phase_zero_discovery_prints_count() {
        let first = &SCENARIOS[0];
        let s = discovery_output(first);
        assert_eq!(s, "qa: phases=1 scenario=menu_title");
    }

    #[test]
    fn action_immediately_followed_by_expect_screen_settles_and_passes() {
        let (out, _guard) = test_dir("settle");
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(bevy::window::WindowPlugin::default())
            .add_plugins(crate::session::SessionPlugin)
            .add_plugins(crate::menu::nav::NavPlugin)
            .add_message::<crate::menu::Action>()
            .add_message::<AppExit>();

        app.insert_resource(crate::menu::nav::MenuFocus { rank: Some(0) });
        app.world_mut().spawn(crate::menu::MenuButton {
            action: crate::menu::Action::OpenScreen(crate::menu::Screen::Worlds),
            rank: 0,
            disabled: false,
        });

        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out,
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(QaPlan(vec![
            Step::Key(KeyCode::Enter),
            Step::Expect(Expect::Screen("Worlds")),
        ]));
        app.add_systems(Update, qa_driver);

        for _ in 0..15 {
            app.update();
            let exits = app.world().resource::<Messages<AppExit>>();
            let mut cursor = exits.get_cursor();
            if cursor.read(exits).any(|e| matches!(e, AppExit::Success)) {
                break;
            }
        }

        let exits = app.world().resource::<Messages<AppExit>>();
        let mut cursor = exits.get_cursor();
        let last_exit = cursor.read(exits).last();
        assert_eq!(
            last_exit,
            Some(&AppExit::Success),
            "expected AppExit::Success without explicit Wait"
        );
        let screen = app.world().resource::<State<session::MenuScreen>>();
        assert_eq!(*screen.get(), session::MenuScreen::Worlds);
    }

    #[test]
    fn screenshot_without_window_fails_nonzero_in_production() {
        let (out, _guard) = test_dir("no_win");
        let mut app = App::new();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);

        // No window spawned, and NO QaTestFixture resource present (production QA mode)
        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out: out.clone(),
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(QaPlan(vec![Step::Screenshot("real_qa_shot")]));
        app.add_systems(Update, qa_driver);

        app.update();

        let exits = app.world().resource::<Messages<AppExit>>();
        let mut cursor = exits.get_cursor();
        let last_exit = cursor.read(exits).last();
        assert!(
            matches!(last_exit, Some(AppExit::Error(_))),
            "missing window in production QA must fail nonzero, got: {:?}",
            last_exit
        );
        assert!(
            !out.join("real_qa_shot.png").exists(),
            "fake/dummy screenshot must NEVER be written on failure in production"
        );
        let run_log = std::fs::read_to_string(out.join("run.log")).unwrap_or_default();
        assert!(
            run_log.contains("no primary window available"),
            "run.log must log error: {run_log}"
        );
    }

    #[test]
    fn screenshot_timeout_fails_nonzero_in_production() {
        let (out, _guard) = test_dir("shot_timeout");
        let mut app = App::new();
        app.add_message::<AppExit>();
        app.add_plugins(bevy::time::TimePlugin);

        // Primary window exists, but NO GPU render app / ScreenshotCaptured fires
        app.world_mut().spawn((
            Window {
                resolution: bevy::window::WindowResolution::new(1920, 1080),
                ..default()
            },
            PrimaryWindow,
        ));

        app.insert_resource(QaRun {
            scenario: 0,
            phase: 1,
            step: 0,
            timer: 0.0,
            out: out.clone(),
        });
        app.insert_resource(QaMetrics::default());
        app.insert_resource(PendingScreenshots::default());
        app.insert_resource(QaPlan(vec![Step::Screenshot("timeout_shot")]));
        app.add_systems(Update, qa_driver);

        // Frame 1: screenshot initiated
        app.update();

        // Advance timer past the capture timeout
        app.world_mut().resource_mut::<QaRun>().timer = screenshot_timeout_secs() + 0.1;

        // Frame 2: timeout detected
        app.update();

        let exits = app.world().resource::<Messages<AppExit>>();
        let mut cursor = exits.get_cursor();
        let last_exit = cursor.read(exits).last();
        assert!(
            matches!(last_exit, Some(AppExit::Error(_))),
            "screenshot timeout in production QA must fail nonzero, got: {:?}",
            last_exit
        );
        let run_log = std::fs::read_to_string(out.join("run.log")).unwrap_or_default();
        assert!(
            run_log.contains("timed out"),
            "run.log must record timeout: {run_log}"
        );
    }

    #[test]
    fn test_liquids_qa_scenario_mechanics() {
        use std::collections::HashMap;
        use terravibe_sim::block::{AIR, OBSIDIAN, STONE};
        use terravibe_sim::chunk::{Chunk, ChunkPos};
        use terravibe_sim::interact::{EditQuery, LoadedQuery, tick_use};
        use terravibe_sim::inventory::{Inventory, ItemStack};
        use terravibe_sim::item::{EMPTY_BUCKET, OBSIDIAN_ITEM, Prefix, WATER_BUCKET};
        use terravibe_sim::liquid::LiquidSim;
        use terravibe_sim::liquid_cell::{Liquid, LiquidKind};
        use terravibe_sim::player::{Player, PlayerBody, PlayerInput, step_body, tick_player};

        // 1. Water slows movement by WATER_MOVE_SCALE (0.50 <= 0.60).
        let mut dry_world = terravibe_sim::World::new();
        for cx in -1..=1 {
            for cz in -1..=1 {
                dry_world.insert(ChunkPos { x: cx, y: 0, z: cz }, Chunk::filled(AIR));
            }
        }
        for x in -5..=5 {
            for z in -5..=5 {
                dry_world.set_block(x, 0, z, STONE);
            }
        }
        let mut p_dry = PlayerBody::new([0.0, 1.0, 0.0]);
        let walk = PlayerInput {
            forward: 1.0,
            ..Default::default()
        };
        step_body(&dry_world, &mut p_dry, &walk);
        let dry_speed = (p_dry.vel[0].powi(2) + p_dry.vel[2].powi(2)).sqrt();

        let mut wet_world = dry_world.clone();
        for x in -5..=5 {
            for z in -5..=5 {
                wet_world.set_liquid(x, 1, z, Some(Liquid::water(255)));
                wet_world.set_liquid(x, 2, z, Some(Liquid::water(255)));
            }
        }
        let mut p_wet = PlayerBody::new([0.0, 1.0, 0.0]);
        step_body(&wet_world, &mut p_wet, &walk);
        let wet_speed = (p_wet.vel[0].powi(2) + p_wet.vel[2].powi(2)).sqrt();
        let ratio = wet_speed / dry_speed;
        assert!(ratio <= 0.60, "water speed ratio {ratio} must be <= 0.6");

        // 2. Breath decreases after head under water.
        let mut player = Player::new([0.0, 1.0, 0.0]);
        assert_eq!(player.vitals.breath, 200);
        for _ in 0..14 {
            tick_player(&wet_world, &mut player, &walk);
        }
        assert!(
            player.vitals.breath < 200,
            "breath must decrease when submerged, got {}",
            player.vitals.breath
        );

        // 3. Life decreases after breath reaches 0.
        player.vitals.breath = 0;
        let initial_life = player.vitals.life;
        for _ in 0..14 {
            tick_player(&wet_world, &mut player, &walk);
        }
        assert!(
            player.vitals.life < initial_life,
            "life must decrease from drowning at breath 0, got {}",
            player.vitals.life
        );

        // 4. Contact: place lava next to water and assert obsidian appears.
        let mut sim = LiquidSim::new();
        let mut contact_world = dry_world.clone();
        contact_world.set_liquid(0, 1, 0, Some(Liquid::water(255)));
        contact_world.set_liquid(1, 1, 0, Some(Liquid::lava(255)));
        sim.wake([0, 1, 0]);
        sim.wake([1, 1, 0]);
        let rep = sim.step(&mut contact_world, 0, 4000);
        assert_eq!(contact_world.block(1, 1, 0), OBSIDIAN);
        assert!(contact_world.liquid(1, 1, 0).is_none());
        assert_eq!(rep.obsidian, vec![[1, 1, 0]]);

        // 5. Mining obsidian drops OBSIDIAN_ITEM.
        assert_eq!(OBSIDIAN.drop_item(), Some(OBSIDIAN_ITEM));

        // 6. Bucket pick up and pour round trip.
        struct TW(
            HashMap<[i32; 3], terravibe_sim::block::BlockId>,
            HashMap<[i32; 3], Liquid>,
        );
        impl terravibe_sim::BlockQuery for TW {
            fn block(&self, x: i32, y: i32, z: i32) -> terravibe_sim::block::BlockId {
                *self.0.get(&[x, y, z]).unwrap_or(&AIR)
            }
            fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid> {
                self.1.get(&[x, y, z]).copied()
            }
        }
        impl LoadedQuery for TW {
            fn is_loaded(&self, _x: i32, _y: i32, _z: i32) -> bool {
                true
            }
        }
        impl EditQuery for TW {
            fn try_set_block(
                &mut self,
                x: i32,
                y: i32,
                z: i32,
                b: terravibe_sim::block::BlockId,
            ) -> bool {
                self.0.insert([x, y, z], b);
                if b != AIR {
                    self.1.remove(&[x, y, z]);
                }
                true
            }
            fn try_set_wall(&mut self, _x: i32, _y: i32, _z: i32, _on: bool) -> bool {
                true
            }
            fn set_liquid(&mut self, x: i32, y: i32, z: i32, l: Option<Liquid>) -> bool {
                if let Some(liq) = l {
                    self.1.insert([x, y, z], liq);
                } else {
                    self.1.remove(&[x, y, z]);
                }
                true
            }
        }

        let mut tw = TW(HashMap::new(), HashMap::new());
        for x in -2..=2 {
            for z in -2..=2 {
                tw.0.insert([x, 0, z], STONE);
            }
        }
        tw.1.insert([0, 1, 0], Liquid::water(255));
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack {
            id: EMPTY_BUCKET,
            count: 1,
            prefix: Prefix(0),
        });
        let p_body = PlayerBody::new([0.0, 1.0, 1.5]);
        let mut st = terravibe_sim::interact::UseState::default();
        let mut rng = terravibe_sim::item::Rng::new(1);
        let look_down = PlayerInput {
            pitch: -0.9,
            yaw: 0.0,
            use_held: true,
            slot: 0,
            ..Default::default()
        };
        tick_use(&mut tw, &p_body, &mut inv, &mut st, &look_down, &mut rng);
        assert_eq!(
            inv.slots[0].as_ref().map(|s| s.id),
            Some(WATER_BUCKET),
            "picking up water must return Water Bucket"
        );
        assert!(
            tw.1.get(&[0, 1, 0]).is_none(),
            "picked up cell must be empty"
        );

        st = Default::default();
        tick_use(&mut tw, &p_body, &mut inv, &mut st, &look_down, &mut rng);
        assert_eq!(
            inv.slots[0].as_ref().map(|s| s.id),
            Some(EMPTY_BUCKET),
            "pouring must return Empty Bucket"
        );
        assert_eq!(
            tw.1.get(&[0, 1, 0]).map(|l| l.kind),
            Some(LiquidKind::Water),
            "poured cell must contain water"
        );

        // 7. Sealed basin: mine the plug, water drains out and settles to pending == 0.
        let mut basin_world = dry_world.clone();
        basin_world.set_liquid(0, 1, 0, Some(Liquid::water(255)));
        basin_world.set_liquid(0, 2, 0, Some(Liquid::water(255)));
        basin_world.set_block(0, 1, 1, STONE); // plug
        let mut b_sim = LiquidSim::new();
        basin_world.set_block(0, 1, 1, AIR);
        b_sim.wake([0, 1, 1]);
        let mut ticks = 0;
        while b_sim.pending() > 0 && ticks < 200 {
            b_sim.step(&mut basin_world, ticks, 4000);
            ticks += 1;
        }
        assert_eq!(b_sim.pending(), 0, "basin must settle to pending == 0");
    }

    #[test]
    fn perf_counters_tracks_particles_and_critters() {
        use crate::settings::Settings;
        use crate::view::enemies::ClientEnemy;
        use crate::view::particles::{AmbientParticleKind, AmbientParticlePool, ParticlePool};
        use terravibe_sim::enemy::{EnemyId, EnemyKind};

        let mut app = App::new();
        app.init_resource::<PerfCounters>();
        let settings = Settings::default();

        let mut particles = ParticlePool::default();
        particles.spawn(Vec3::ZERO, Vec3::Y, Color::WHITE, 0.1, 1.0, 9.8, &settings);
        particles.spawn(Vec3::ONE, Vec3::Y, Color::WHITE, 0.1, 1.0, 9.8, &settings);
        app.insert_resource(particles);

        let mut ambient = AmbientParticlePool::default();
        ambient.spawn(
            AmbientParticleKind::DustMote,
            Vec3::ZERO,
            Vec3::ZERO,
            Color::WHITE,
            0.1,
            1.0,
            0.0,
            &settings,
        );
        app.insert_resource(ambient);

        app.world_mut()
            .spawn(ClientEnemy::new(EnemyId(1), EnemyKind::Bunny));
        app.world_mut()
            .spawn(ClientEnemy::new(EnemyId(2), EnemyKind::Slime));

        app.add_systems(Update, update_perf_counters);
        app.update();

        let perf = app.world().resource::<PerfCounters>();
        assert_eq!(perf.particles_active, 3);
        assert_eq!(perf.critters_active, 1);
    }
}
