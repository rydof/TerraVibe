//! `--bench`: wait for the view to finish streaming, fly a fixed circle,
//! record real frame times and print one `BENCH` line, then exit
//! (0 on pass, 2 on fail or timeout).

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::window::PrimaryWindow;

use crate::hud::boss_bar::{BossInfo, BossState};
use crate::session::AppState;
use crate::view::camera::{FlyCamera, bench_pose};
use crate::view::chunks::{ChunkStream, stream_chunks};
use crate::view::enemies::EnemyMsg;
use crate::view::surface_y;
use terravibe_sim::enemy::{EnemyId, EnemyKind};

/// Longest wait for the initial view to be fully meshed.
const WAIT_TIMEOUT: Duration = Duration::from_secs(120);
/// Flying but not recording, so the first moving frames don't count.
const WARMUP: Duration = Duration::from_secs(3);
/// Recording window.
const RECORD: Duration = Duration::from_secs(30);
/// Fullscreen on GNOME/Wayland presents capped at the 60 Hz refresh for the
/// first 5-12 s after the window maps (compositor transition, every present
/// mode). Before warm-up, wait for this many consecutive frames under
/// `SETTLED_MS`, for at most `SETTLE_TIMEOUT`.
const SETTLED_FRAMES: u32 = 30;
const SETTLED_MS: f32 = 14.0;
const SETTLE_TIMEOUT: Duration = Duration::from_secs(20);
/// Flight height above the surface at spawn.
const HEIGHT_ABOVE_SURFACE: f32 = 20.0;

/// Which benchmark `TERRAVIBE_BENCH_SCENARIO` selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scenario {
    /// Flying circle over the streamed terrain (default).
    Terrain,
    /// The terrain flight plus 12 client-spawned enemies in a camera-following
    /// formation (synthetic benchmark placement, not gameplay).
    Creatures,
    /// The terrain flight plus a synthetic Eye of Cthulhu held on the camera's
    /// aim point (synthetic benchmark placement, not gameplay).
    Boss,
    /// The terrain flight while the server ticks an active 20,000-cell liquid flood.
    LiquidStress,
}

impl Scenario {
    fn parse(v: Option<&str>) -> Self {
        match v {
            Some("creatures") => Scenario::Creatures,
            Some("boss") => Scenario::Boss,
            Some("liquid_stress") | Some("liquid-stress") => Scenario::LiquidStress,
            _ => Scenario::Terrain,
        }
    }
}

/// Boss scenario: the ball centre sits this far along the camera's view.
const BOSS_AHEAD: f32 = 20.0;
/// The rig's ball offset (55/16 blocks above the boss's feet position).
const BOSS_BALL_HEIGHT: f32 = 55.0 / 16.0;
const BOSS_LIFE: i32 = 2800;
/// Phase 1 for this many seconds of flight, then phase 2.
const BOSS_PHASE_2_SECS: f32 = 15.0;

const FORMATION_RIGHT: [f32; 4] = [-4.5, -1.5, 1.5, 4.5];
const FORMATION_UP: [f32; 3] = [-2.0, 0.0, 2.0];
const FORMATION_AHEAD: f32 = 16.0;
const FORMATION_KINDS: [EnemyKind; 4] = [
    EnemyKind::Slime,
    EnemyKind::Zombie,
    EnemyKind::DemonEye,
    EnemyKind::EaterOfSouls,
];

/// Synthetic benchmark formation (not gameplay placement): 4 columns x 3 rows
/// centred 16 blocks along `cam`'s forward, kinds by column. Row `r` is
/// `FORMATION_UP[r]` blocks above the centre; columns spread along the
/// camera's right.
fn creature_formation(cam: &Transform) -> Vec<(EnemyId, EnemyKind, Vec3)> {
    let centre = cam.translation + cam.forward().as_vec3() * FORMATION_AHEAD;
    let right = cam.right().as_vec3();
    let mut out = Vec::with_capacity(12);
    for (row, &dy) in FORMATION_UP.iter().enumerate() {
        for (col, &dx) in FORMATION_RIGHT.iter().enumerate() {
            let i = row * FORMATION_RIGHT.len() + col;
            out.push((
                EnemyId(10_000 + i as u32),
                FORMATION_KINDS[col],
                centre + right * dx + Vec3::Y * dy,
            ));
        }
    }
    out
}

/// Synthetic boss for the boss scenario at `t` seconds of flight: the ball
/// centre is `BOSS_AHEAD` blocks along `cam`'s view, and `pos` is the feet.
fn boss_info(cam: &Transform, t: f32) -> BossInfo {
    let aim = cam.translation + cam.forward().as_vec3() * BOSS_AHEAD;
    BossInfo {
        pos: (aim - Vec3::Y * BOSS_BALL_HEIGHT).to_array(),
        vel: [0.0; 3],
        life: BOSS_LIFE,
        life_max: BOSS_LIFE,
        phase: if t < BOSS_PHASE_2_SECS { 1 } else { 2 },
        state: 0,
        spin: 0.0,
    }
}

/// Set first thing in `main`; `startup_ms` counts from here.
pub static PROCESS_START: OnceLock<Instant> = OnceLock::new();

#[derive(Debug, PartialEq)]
pub struct BenchSummary {
    pub frames: usize,
    pub avg_fps: f32,
    pub p50_ms: f32,
    pub p99_ms: f32,
    pub max_ms: f32,
    /// Frames slower than 16.7 ms.
    pub over_16_7: usize,
}

impl BenchSummary {
    #[cfg(test)]
    pub fn passes(&self) -> bool {
        self.passes_for(Scenario::Terrain)
    }

    fn passes_for(&self, scenario: Scenario) -> bool {
        let min_fps = match scenario {
            Scenario::LiquidStress => 55.0,
            _ => 60.0,
        };
        self.avg_fps >= min_fps && self.p99_ms <= 16.7
    }
}

/// Summarises per-frame times in milliseconds. `p50_ms` and `p99_ms` are
/// elements `ceil(0.5 n) - 1` and `ceil(0.99 n) - 1` of the sorted times.
pub fn summarize(frame_times_ms: &[f32]) -> BenchSummary {
    let n = frame_times_ms.len();
    if n == 0 {
        return BenchSummary {
            frames: 0,
            avg_fps: 0.0,
            p50_ms: 0.0,
            p99_ms: 0.0,
            max_ms: 0.0,
            over_16_7: 0,
        };
    }
    let total: f64 = frame_times_ms.iter().map(|&t| f64::from(t)).sum();
    let mut sorted = frame_times_ms.to_vec();
    sorted.sort_by(f32::total_cmp);
    let idx = ((0.99 * n as f64).ceil() as usize).saturating_sub(1);
    let mid = ((0.5 * n as f64).ceil() as usize).saturating_sub(1);
    BenchSummary {
        frames: n,
        avg_fps: (n as f64 * 1000.0 / total) as f32,
        p50_ms: sorted[mid],
        p99_ms: sorted[idx],
        max_ms: sorted[n - 1],
        over_16_7: sorted.iter().filter(|&&t| t > 16.7).count(),
    }
}

pub struct BenchPlugin;

impl Plugin for BenchPlugin {
    fn build(&self, app: &mut App) {
        let surface = surface_y(0, 0).unwrap_or(64) as f32;
        let scenario = Scenario::parse(std::env::var("TERRAVIBE_BENCH_SCENARIO").ok().as_deref());
        app.insert_resource(Bench {
            scenario,
            y: surface + 1.0 + HEIGHT_ABOVE_SURFACE,
            phase: Phase::Waiting,
            times_ms: Vec::new(),
            startup_ms: 0.0,
        })
        .add_systems(PostStartup, place_camera)
        .add_systems(Update, drive.after(stream_chunks))
        .add_systems(
            Update,
            spawn_creatures
                .after(drive)
                .run_if(in_state(AppState::InGame)),
        )
        .add_systems(
            Update,
            spawn_boss.after(drive).run_if(in_state(AppState::InGame)),
        );
    }
}

enum Phase {
    Waiting,
    /// Meshed at `start`; `fast` consecutive frames under `SETTLED_MS` so far.
    Settling {
        start: Duration,
        fast: u32,
    },
    /// Flying since `start` (real elapsed time).
    Flying {
        start: Duration,
    },
    /// Reported; the app exits after this frame.
    Done,
}

#[derive(Resource)]
struct Bench {
    scenario: Scenario,
    y: f32,
    phase: Phase,
    times_ms: Vec<f32>,
    /// Process start to the first frame with the view meshed.
    startup_ms: f32,
}

impl Bench {
    /// Camera pose at `t` seconds of flight; the same path in every scenario.
    fn pose(&self, t: f32) -> Transform {
        bench_pose(t, self.y)
    }
}

fn place_camera(bench: Res<Bench>, mut tf: Single<&mut Transform, With<FlyCamera>>) {
    **tf = bench.pose(0.0);
}

/// Creatures scenario: spawn the formation on the first InGame frame (the
/// rigs load in `Startup`), then move the same 12 IDs with the camera every frame.
fn spawn_creatures(
    bench: Res<Bench>,
    cam: Single<&Transform, With<FlyCamera>>,
    mut spawned: Local<bool>,
    mut msgs: MessageWriter<EnemyMsg>,
) {
    if bench.scenario != Scenario::Creatures {
        return;
    }
    for (id, kind, pos) in creature_formation(&cam) {
        msgs.write(if *spawned {
            EnemyMsg::Moved {
                id,
                pos,
                vel: Vec3::ZERO,
                hp: i32::MAX,
            }
        } else {
            EnemyMsg::Spawned { id, kind, pos }
        });
    }
    *spawned = true;
}

/// Boss scenario: write the synthetic boss every frame after `drive`, so the
/// net pump cannot clear it. Phase follows the flight clock.
fn spawn_boss(
    bench: Res<Bench>,
    time: Res<Time<Real>>,
    cam: Single<&Transform, With<FlyCamera>>,
    mut boss: ResMut<BossState>,
) {
    if bench.scenario != Scenario::Boss {
        return;
    }
    let t = match bench.phase {
        Phase::Flying { start } => (time.elapsed() - start).as_secs_f32(),
        _ => 0.0,
    };
    boss.set(boss_info(&cam, t));
}

fn drive(
    mut bench: ResMut<Bench>,
    stream: Res<ChunkStream>,
    time: Res<Time<Real>>,
    adapter: Option<Res<RenderAdapterInfo>>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    mut tf: Single<&mut Transform, With<FlyCamera>>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed();
    let b = &mut *bench;
    match b.phase {
        Phase::Waiting => {
            **tf = b.pose(0.0);
            if stream.fully_meshed(tf.translation) {
                b.startup_ms = PROCESS_START
                    .get()
                    .map_or(0.0, |t| t.elapsed().as_secs_f32() * 1000.0);
                info!(
                    "bench: view meshed after {:.1?} ({} meshes)",
                    now,
                    stream.mesh_count()
                );
                if let Some(w) = window {
                    info!(
                        "bench: window {}x{}",
                        w.physical_width(),
                        w.physical_height()
                    );
                }
                b.phase = Phase::Settling {
                    start: now,
                    fast: 0,
                };
            } else if now >= WAIT_TIMEOUT {
                eprintln!(
                    "bench: timed out after {WAIT_TIMEOUT:?} waiting for the view to mesh \
                     ({} meshes)",
                    stream.mesh_count()
                );
                report(
                    &summarize(&[]),
                    &stream,
                    adapter.as_deref(),
                    b.startup_ms,
                    false,
                );
                exit.write(AppExit::from_code(2));
                b.phase = Phase::Done;
            }
        }
        Phase::Settling { start, fast } => {
            **tf = b.pose(0.0);
            let fast = if time.delta_secs() * 1000.0 < SETTLED_MS {
                fast + 1
            } else {
                0
            };
            if fast >= SETTLED_FRAMES || now - start >= SETTLE_TIMEOUT {
                info!(
                    "bench: present settled after {:.1?} (timed out: {})",
                    now - start,
                    fast < SETTLED_FRAMES
                );
                b.phase = Phase::Flying { start: now };
            } else {
                b.phase = Phase::Settling { start, fast };
            }
        }
        Phase::Done => {}
        Phase::Flying { start } => {
            let t = now - start;
            **tf = b.pose(t.as_secs_f32());
            if t > WARMUP {
                b.times_ms.push(time.delta_secs() * 1000.0);
            }
            if t >= WARMUP + RECORD {
                let s = summarize(&b.times_ms);
                let pass = s.passes_for(b.scenario);
                report(&s, &stream, adapter.as_deref(), b.startup_ms, pass);
                exit.write(if pass {
                    AppExit::Success
                } else {
                    AppExit::from_code(2)
                });
                b.phase = Phase::Done;
            }
        }
    }
}

fn report(
    s: &BenchSummary,
    stream: &ChunkStream,
    adapter: Option<&RenderAdapterInfo>,
    startup_ms: f32,
    pass: bool,
) {
    let (name, backend) = adapter.map_or(("unknown".to_string(), "unknown".to_string()), |a| {
        (a.name.clone(), format!("{:?}", a.backend))
    });
    println!(
        "{}",
        format_report(s, &name, &backend, stream.quads(), startup_ms, pass)
    );
}

fn format_report(
    s: &BenchSummary,
    name: &str,
    backend: &str,
    quads: usize,
    startup_ms: f32,
    pass: bool,
) -> String {
    format!(
        "BENCH adapter=\"{name}\" backend={backend} frames={} avg_fps={:.1} p50_ms={:.2} p99_ms={:.2} \
         quads={quads} max_ms={:.2} over_16_7={} startup_ms={startup_ms:.0} pass={pass}",
        s.frames, s.avg_fps, s.p50_ms, s.p99_ms, s.max_ms, s.over_16_7
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_parsing() {
        assert_eq!(Scenario::parse(None), Scenario::Terrain);
        assert_eq!(Scenario::parse(Some("terrain")), Scenario::Terrain);
        assert_eq!(Scenario::parse(Some("creatures")), Scenario::Creatures);
        assert_eq!(Scenario::parse(Some("boss")), Scenario::Boss);
        assert_eq!(
            Scenario::parse(Some("liquid_stress")),
            Scenario::LiquidStress
        );
        assert_eq!(
            Scenario::parse(Some("liquid-stress")),
            Scenario::LiquidStress
        );
        assert_eq!(Scenario::parse(Some("bogus")), Scenario::Terrain);
    }

    #[test]
    fn boss_state_centres_ball() {
        for t in [0.0, 7.5] {
            let cam = bench_pose(t, 85.0);
            let b = boss_info(&cam, t);
            // The rig root is the boss's feet; the ball centre sits 55/16
            // blocks above it, on the aim point 20 blocks along the view.
            let centre = Vec3::from(b.pos) + Vec3::Y * (55.0 / 16.0);
            let aim = cam.translation + cam.forward().as_vec3() * 20.0;
            assert!((centre - aim).length() < 1e-4, "{centre:?} != {aim:?}");
            assert_eq!(b.vel, [0.0; 3]);
            assert_eq!((b.life, b.life_max), (2800, 2800));
        }

        let cam = bench_pose(0.0, 85.0);
        assert_eq!(boss_info(&cam, 0.0).phase, 1);
        assert_eq!(boss_info(&cam, 14.9).phase, 1);
        assert_eq!(boss_info(&cam, 15.0).phase, 2);
    }

    #[test]
    fn creature_formation_follows_camera() {
        use std::collections::HashSet;
        use terravibe_sim::enemy::EnemyKind::*;
        for t in [0.0, 5.0] {
            let cam = bench_pose(t, 85.0);
            let l = creature_formation(&cam);
            assert_eq!(l.len(), 12);
            let ids: Vec<u32> = l.iter().map(|e| e.0.0).collect();
            assert_eq!(ids, (10_000..10_012).collect::<Vec<_>>());
            for k in [Slime, Zombie, DemonEye, EaterOfSouls] {
                assert_eq!(l.iter().filter(|e| e.1 == k).count(), 3, "{k:?}");
            }
            let cells: HashSet<(i32, i32, i32)> = l
                .iter()
                .map(|e| {
                    (
                        (e.2.x * 4.0) as i32,
                        (e.2.y * 4.0) as i32,
                        (e.2.z * 4.0) as i32,
                    )
                })
                .collect();
            assert_eq!(cells.len(), 12);
            let fwd = cam.forward().as_vec3();
            for (_, kind, p) in &l {
                let ang = fwd
                    .angle_between((*p - cam.translation).normalize())
                    .to_degrees();
                assert!(ang < 35.0, "{kind:?} {ang} deg at t={t}");
            }
        }
    }

    #[test]
    fn bench_report_has_startup_ms() {
        let line = format_report(&summarize(&[10.0; 4]), "gpu", "Vulkan", 7, 1234.4, true);
        assert!(line.contains("startup_ms=1234 pass=true"), "{line}");
    }

    #[test]
    fn summary_basic() {
        let s = summarize(&[10.0; 100]);
        assert_eq!(s.frames, 100);
        assert!((s.avg_fps - 100.0).abs() < 0.01);
        assert_eq!(s.p99_ms, 10.0);
    }

    #[test]
    fn p99_catches_spike() {
        let mut t = vec![10.0; 99];
        t.push(40.0);
        let s = summarize(&t);
        assert_eq!((s.p99_ms, s.max_ms, s.over_16_7), (10.0, 40.0, 1));
        t.push(40.0);
        assert_eq!(summarize(&t).p99_ms, 40.0);
    }

    #[test]
    fn p50_is_median_and_reported() {
        let mut t = vec![5.0; 50];
        t.extend([20.0; 50]);
        let s = summarize(&t);
        assert_eq!((s.p50_ms, s.p99_ms), (5.0, 20.0));
        assert_eq!(summarize(&[1.0, 2.0, 3.0]).p50_ms, 2.0);
        let line = format_report(&s, "gpu", "Vulkan", 7, 1.0, true);
        assert!(line.contains("p50_ms=5.00 p99_ms=20.00"), "{line}");
    }

    #[test]
    fn pass_threshold() {
        assert!(
            BenchSummary {
                frames: 1,
                avg_fps: 60.0,
                p50_ms: 10.0,
                p99_ms: 16.7,
                max_ms: 30.0,
                over_16_7: 1,
            }
            .passes()
        );
        assert!(
            !BenchSummary {
                frames: 1,
                avg_fps: 59.9,
                p50_ms: 10.0,
                p99_ms: 10.0,
                max_ms: 10.0,
                over_16_7: 0,
            }
            .passes()
        );
        assert!(
            BenchSummary {
                frames: 1,
                avg_fps: 55.0,
                p50_ms: 10.0,
                p99_ms: 16.7,
                max_ms: 16.7,
                over_16_7: 0,
            }
            .passes_for(Scenario::LiquidStress)
        );
        assert!(
            !BenchSummary {
                frames: 1,
                avg_fps: 54.9,
                p50_ms: 10.0,
                p99_ms: 10.0,
                max_ms: 10.0,
                over_16_7: 0,
            }
            .passes_for(Scenario::LiquidStress)
        );
    }
}
