//! Fly camera: WASD + mouse look, Space up, Shift down, Ctrl fast.
//! The cursor is grabbed; Esc releases it and a click grabs it again.

use bevy::camera::Hdr;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::ecs::message::MessageReader;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseMotion, MouseWheel};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorMoved, CursorOptions, PrimaryWindow, WindowFocused};
use terravibe_sim::BlockQuery;
use terravibe_sim::interact::raycast;

use crate::session::AppState;

pub const CAMERA_START: Vec3 = Vec3::new(0.0, 100.0, 0.0);
const SPEED: f32 = 20.0;
const FAST: f32 = 5.0;
const SENSITIVITY: f32 = 0.002;

#[derive(Component)]
pub struct FlyCamera {
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}

/// Player look sensitivity multiplier (1.0 = default).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct LookSensitivity(pub f32);

impl Default for LookSensitivity {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Pure helper for updating camera angles given mouse motion, sensitivity, invert-Y and pitch limits.
pub fn update_camera_angles(
    yaw: f32,
    pitch: f32,
    delta: Vec2,
    sensitivity: f32,
    invert_y: bool,
    pitch_limits: (f32, f32),
) -> (f32, f32) {
    let new_yaw = yaw - delta.x * SENSITIVITY * sensitivity;
    let pitch_delta = delta.y * SENSITIVITY * sensitivity;
    let new_pitch = if invert_y {
        pitch + pitch_delta
    } else {
        pitch - pitch_delta
    };
    (new_yaw, new_pitch.clamp(pitch_limits.0, pitch_limits.1))
}
/// `TERRAVIBE_CAMERA`, or toggled with F4); `Player` is the first-person body.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ControlMode {
    Fly,
    Player,
}

/// Player view: orbiting the avatar (default) or from the eyes. V toggles.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CameraMode {
    Third,
    First,
}

/// Third-person orbit distance, blocks (scroll wheel, [`MIN_DIST`]..=[`MAX_DIST`]).
#[derive(Resource)]
pub struct OrbitDist(pub f32);

/// Smoothed third-person orbit distance after exponential wall pull-in damping.
#[derive(Resource)]
pub struct SmoothedOrbitDist(pub f32);

pub const MIN_DIST: f32 = 3.0;
pub const MAX_DIST: f32 = 9.0;
pub const DEFAULT_DIST: f32 = 5.5;
/// Third-person camera pivot offsets relative to the player's eye (blocks):
/// zero lateral offset keeps straight-ahead aim aligned with the center crosshair,
/// while raising the pivot slightly clears the player's head.
pub const SHOULDER_RIGHT: f32 = 0.0;
pub const SHOULDER_UP: f32 = 0.15;
/// Fast pull-in rate when obstructed by a wall, and smooth ease-out rate when clearing.
const PULL_IN_FAST_RATE: f32 = 24.0;
const PULL_IN_EASE_OUT_RATE: f32 = 7.0;
/// Subtle dynamic FOV boost (degrees) when sprinting / Hermes Boots active.
const SPRINT_FOV_BOOST_DEG: f32 = 5.5;
/// Third-person pitch limits, radians (positive looks up).
const THIRD_PITCH: (f32, f32) = (-std::f32::consts::FRAC_PI_3, 1.2217); // -60 and 70 degrees
/// The camera stops this far before the first solid block on its ray.
const PULL_IN_GAP: f32 = 0.3;
/// Scroll lines to blocks of zoom.
const ZOOM_STEP: f32 = 0.5;

/// Exponentially damps third-person orbit distance: fast pull-in on wall obstruction,
/// smooth ease-out when clearing.
pub fn damp_pull_in(current: f32, target: f32, dt: f32) -> f32 {
    let dt = dt.max(0.0);
    if dt == 0.0 {
        return current;
    }
    let rate = if target < current {
        PULL_IN_FAST_RATE
    } else {
        PULL_IN_EASE_OUT_RATE
    };
    let alpha = 1.0 - (-rate * dt).exp();
    current + (target - current) * alpha
}

/// Computes target vertical FOV (degrees) with a subtle boost when sprinting / Hermes Boots.
pub fn target_fov_deg(
    base_fov_deg: f32,
    horiz_speed: f32,
    hermes_sprinting: bool,
    reduced_motion: bool,
) -> f32 {
    if reduced_motion {
        return base_fov_deg;
    }
    let speed_factor = ((horiz_speed - 0.11) / 0.12).clamp(0.0, 1.0);
    let boost = if hermes_sprinting {
        SPRINT_FOV_BOOST_DEG
    } else {
        SPRINT_FOV_BOOST_DEG * 0.45 * speed_factor
    };
    base_fov_deg + boost * speed_factor.max(if hermes_sprinting { 1.0 } else { 0.0 })
}

/// Distance along the ray from `eye` toward `dir` (unit) at which the camera
/// can sit: `dist`, or `PULL_IN_GAP` before the first solid block.
pub fn pull_in(world: &impl BlockQuery, eye: Vec3, dir: Vec3, dist: f32) -> f32 {
    let Some(hit) = raycast(world, eye.into(), dir.into(), dist + PULL_IN_GAP) else {
        return dist;
    };
    // Slab test of the ray against the hit block for the entry distance.
    let lo = Vec3::new(
        hit.block[0] as f32,
        hit.block[1] as f32,
        hit.block[2] as f32,
    );
    let (mut t0, mut t1) = (0.0f32, f32::INFINITY);
    for a in 0..3 {
        if dir[a].abs() < 1e-9 {
            continue;
        }
        let (n, f) = ((lo[a] - eye[a]) / dir[a], (lo[a] + 1.0 - eye[a]) / dir[a]);
        t0 = t0.max(n.min(f));
        t1 = t1.min(n.max(f));
    }
    if t0 > t1 {
        return dist;
    }
    (t0 - PULL_IN_GAP).clamp(0.0, dist)
}

fn third_person(m: Res<ControlMode>, c: Res<CameraMode>) -> bool {
    *m == ControlMode::Player && *c == CameraMode::Third
}

/// Alt or Ctrl + scroll zooms the third-person camera; bare wheel scrolls the hotbar.
fn zoom(
    mut wheel: MessageReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    mut dist: ResMut<OrbitDist>,
) {
    let modifier = keys.pressed(KeyCode::AltLeft)
        || keys.pressed(KeyCode::AltRight)
        || keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight);
    for e in wheel.read() {
        if modifier {
            dist.0 = (dist.0 - e.y * ZOOM_STEP).clamp(MIN_DIST, MAX_DIST);
        }
    }
}

pub fn fly_mode(m: Res<ControlMode>) -> bool {
    *m == ControlMode::Fly
}

fn toggle_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<ControlMode>,
    mut cam: ResMut<CameraMode>,
) {
    if keys.just_pressed(KeyCode::KeyV) {
        *cam = match *cam {
            CameraMode::Third => CameraMode::First,
            CameraMode::First => CameraMode::Third,
        };
    }
    if keys.just_pressed(KeyCode::F4) {
        *mode = match *mode {
            ControlMode::Fly => ControlMode::Player,
            ControlMode::Player => ControlMode::Fly,
        };
    }
}

pub struct FlyCameraPlugin;

/// Camera shake trauma in `[0.0, 1.0]`, squared for perceptual falloff.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct CameraTrauma {
    pub trauma: f32,
}

impl CameraTrauma {
    const MAX_OFFSET: f32 = 0.25;
    const DECAY_PER_SEC: f32 = 1.8;

    pub fn add(&mut self, amount: f32) {
        if amount > 0.0 {
            self.trauma = (self.trauma + amount).clamp(0.0, 1.0);
        }
    }

    pub fn decay(&mut self, dt: f32) {
        self.trauma = (self.trauma - dt.max(0.0) * Self::DECAY_PER_SEC).max(0.0);
    }

    /// Deterministic shake offset at time `t`; zero when `screen_shake == 0.0` or `reduced_motion == true`.
    pub fn offset(&self, settings: &crate::settings::Settings, t: f32) -> Vec3 {
        if settings.screen_shake <= 0.0 || settings.reduced_motion || self.trauma <= 0.0 {
            return Vec3::ZERO;
        }
        let mag =
            self.trauma * self.trauma * settings.screen_shake.clamp(0.0, 2.0) * Self::MAX_OFFSET;
        Vec3::new(
            ((t * 37.0).sin() * 0.6 + (t * 71.0).cos() * 0.4) * mag,
            ((t * 43.0).cos() * 0.6 + (t * 89.0).sin() * 0.4) * mag,
            ((t * 53.0).sin() * 0.5 + (t * 97.0).cos() * 0.5) * mag * 0.5,
        )
    }
}

/// Hitstop / slow-motion state controlling `Time<Virtual>` speed.
#[derive(Resource, Debug, Clone, Copy)]
pub struct HitstopState {
    pub remaining: f32,
    pub time_scale: f32,
}

impl Default for HitstopState {
    fn default() -> Self {
        Self {
            remaining: 0.0,
            time_scale: 1.0,
        }
    }
}

impl HitstopState {
    /// Triggers a brief freeze-frame if `settings.hitstop` is enabled.
    pub fn trigger(&mut self, secs: f32, settings: &crate::settings::Settings) {
        if !settings.hitstop || secs <= 0.0 {
            return;
        }
        self.remaining = self.remaining.max(secs);
        self.time_scale = 0.05;
    }

    /// Triggers a slow-motion beat (e.g. boss defeat) if `settings.hitstop` is enabled.
    pub fn trigger_slowmo(&mut self, secs: f32, scale: f32, settings: &crate::settings::Settings) {
        if !settings.hitstop || secs <= 0.0 {
            return;
        }
        self.remaining = self.remaining.max(secs);
        self.time_scale = scale.clamp(0.05, 1.0);
    }
}

fn update_combat_camera_fx(
    time_real: Res<Time<Real>>,
    mut vtime: ResMut<Time<Virtual>>,
    mut trauma: ResMut<CameraTrauma>,
    mut hitstop: ResMut<HitstopState>,
    mut hits: MessageReader<crate::hud::combat_feedback::CombatHitEvent>,
    settings: Option<Res<crate::settings::Settings>>,
) {
    use terravibe_sim::net::{DamageKind, HitTargetKind};
    let s = settings.as_deref().copied().unwrap_or_default();
    for ev in hits.read() {
        if ev.target_kind == HitTargetKind::Player {
            trauma.add(0.45);
        } else if ev.killed && ev.target_kind == HitTargetKind::Boss {
            trauma.add(0.65);
            hitstop.trigger_slowmo(0.65, 0.25, &s);
        } else if ev.killed {
            trauma.add(0.35);
            hitstop.trigger(0.065, &s);
        } else if ev.damage_kind == DamageKind::Melee {
            trauma.add(if ev.crit { 0.30 } else { 0.18 });
            hitstop.trigger(if ev.crit { 0.055 } else { 0.04 }, &s);
        }
    }
    let dt = time_real.delta_secs();
    trauma.decay(dt);
    if hitstop.remaining > 0.0 {
        hitstop.remaining = (hitstop.remaining - dt).max(0.0);
        if hitstop.remaining > 0.0 {
            vtime.set_relative_speed(hitstop.time_scale);
        } else {
            hitstop.time_scale = 1.0;
            vtime.set_relative_speed(1.0);
        }
    }
}

fn reset_combat_camera_fx(
    mut vtime: ResMut<Time<Virtual>>,
    mut trauma: ResMut<CameraTrauma>,
    mut hitstop: ResMut<HitstopState>,
) {
    vtime.set_relative_speed(1.0);
    trauma.trauma = 0.0;
    *hitstop = HitstopState::default();
}

impl Plugin for FlyCameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CameraMode::Third)
            .insert_resource(OrbitDist(DEFAULT_DIST))
            .insert_resource(SmoothedOrbitDist(DEFAULT_DIST))
            .init_resource::<LookSensitivity>()
            .init_resource::<CameraTrauma>()
            .init_resource::<HitstopState>()
            .add_systems(OnEnter(AppState::InGame), (spawn_camera, grab_cursor))
            .add_systems(
                OnExit(AppState::InGame),
                (release_cursor, reset_combat_camera_fx),
            )
            .add_systems(
                Update,
                (
                    look.run_if(crate::menu::menu_closed),
                    fly.run_if(fly_mode).run_if(crate::menu::menu_closed),
                    zoom.run_if(third_person).run_if(crate::menu::menu_closed),
                    toggle_grab,
                    reassert_grab,
                    toggle_mode.run_if(crate::menu::menu_closed),
                    update_combat_camera_fx,
                    sync_camera_atmosphere_fx,
                )
                    .run_if(|b: Res<super::BenchMode>| !b.0)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                log_input_debug.run_if(resource_exists::<InputDebug>),
            );
    }
}

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputDebug;

#[derive(Default)]
pub(crate) struct InputDebugStats {
    timer: f32,
    motion_count: u32,
    cursor_moved_count: u32,
    accum_delta: Vec2,
}

pub(crate) fn log_input_debug(
    time: Res<Time>,
    windows: Query<(&CursorOptions, &Window), With<PrimaryWindow>>,
    motion: Option<Res<AccumulatedMouseMotion>>,
    mut motion_reader: Option<MessageReader<MouseMotion>>,
    mut cursor_reader: Option<MessageReader<CursorMoved>>,
    mut stats: Local<InputDebugStats>,
) {
    stats.timer += time.delta_secs();
    if let Some(ref m) = motion {
        stats.accum_delta += m.delta;
    }
    if let Some(ref mut mr) = motion_reader {
        stats.motion_count += mr.read().count() as u32;
    }
    if let Some(ref mut cr) = cursor_reader {
        stats.cursor_moved_count += cr.read().count() as u32;
    }

    if stats.timer >= 1.0 {
        let backend = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            "wayland"
        } else {
            "x11/other"
        };
        if let Ok((opts, win)) = windows.single() {
            let msg = format!(
                "[input-debug] backend={backend} grab={:?} visible={} focused={} mouse_motions={} accum_delta={:?} cursor_moved={}",
                opts.grab_mode,
                opts.visible,
                win.focused,
                stats.motion_count,
                stats.accum_delta,
                stats.cursor_moved_count,
            );
            println!("{msg}");
        } else {
            let msg = format!(
                "[input-debug] backend={backend} (no primary window) mouse_motions={} accum_delta={:?} cursor_moved={}",
                stats.motion_count, stats.accum_delta, stats.cursor_moved_count,
            );
            println!("{msg}");
        }
        stats.timer = 0.0;
        stats.motion_count = 0;
        stats.cursor_moved_count = 0;
        stats.accum_delta = Vec2::ZERO;
    }
}

/// Benchmark path: radius of the circle around spawn, in blocks.
pub const BENCH_RADIUS: f32 = 60.0;
/// Benchmark path: seconds per lap.
pub const BENCH_LAP_SECS: f32 = 30.0;
/// Benchmark path: pitch while flying (looking slightly down).
const BENCH_PITCH: f32 = -0.25;

/// Camera pose `t` seconds into the benchmark flight: a circle of
/// `BENCH_RADIUS` around (0, 0) at height `y`, looking along the path.
pub fn bench_pose(t: f32, y: f32) -> Transform {
    let a = t / BENCH_LAP_SECS * std::f32::consts::TAU;
    let pos = Vec3::new(BENCH_RADIUS * a.cos(), y, BENCH_RADIUS * a.sin());
    // Counter-clockwise tangent in x/z; yaw follows `aim_at`'s convention.
    let dir = Vec3::new(-a.sin(), 0.0, a.cos());
    let yaw = (-dir.x).atan2(-dir.z);
    Transform::from_translation(pos).with_rotation(Quat::from_euler(
        EulerRot::YXZ,
        yaw,
        BENCH_PITCH,
        0.0,
    ))
}

/// Points the camera at `target`.
#[allow(dead_code)] // kept for scripted shots; the demo slime that used it is gone
pub fn aim_at(cam: &mut FlyCamera, tf: &mut Transform, target: Vec3) {
    let d = (target - tf.translation).normalize_or(Vec3::NEG_Z);
    cam.yaw = (-d.x).atan2(-d.z);
    cam.pitch = d.y.clamp(-1.0, 1.0).asin();
    tf.rotation = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);
}

/// Debug env var `x,y,z,yaw_deg,pitch_deg`: overrides the camera start and
/// disables aiming at the slime (for screenshots and benchmarks).
pub(crate) const ENV_CAMERA: &str = "TERRAVIBE_CAMERA";

/// Present when `TERRAVIBE_CAMERA` placed the camera.
#[derive(Resource)]
pub struct CameraOverride;

fn parse_camera(s: &str) -> Option<(Vec3, f32, f32)> {
    let v: Vec<f32> = s
        .split(',')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    let [x, y, z, yaw, pitch] = v[..] else {
        return None;
    };
    Some((Vec3::new(x, y, z), yaw.to_radians(), pitch.to_radians()))
}

/// Cave depth fog density in `0.0..=1.0`, disabled when `atmosphere_fx` is false.
pub fn cave_fog_density(depth: f32, atmosphere_fx: bool) -> f32 {
    if !atmosphere_fx || depth <= 0.0 {
        return 0.0;
    }
    (depth / 64.0).clamp(0.0, 1.0)
}

fn default_distance_fog() -> DistanceFog {
    DistanceFog {
        color: Color::srgb(0.53, 0.72, 0.95),
        falloff: FogFalloff::Linear {
            start: 80.0,
            end: 320.0,
        },
        ..default()
    }
}

pub(crate) fn sync_camera_atmosphere_fx(
    mut commands: Commands,
    settings: Option<Res<crate::settings::Settings>>,
    cam_q: Query<
        (
            Entity,
            Has<Bloom>,
            Has<DistanceFog>,
            Has<Hdr>,
            Option<&Tonemapping>,
        ),
        With<FlyCamera>,
    >,
) {
    let enabled = settings.as_ref().is_none_or(|s| s.atmosphere_fx);
    let bloom_enabled = enabled
        && settings
            .as_ref()
            .is_none_or(|s| s.quality_preset != crate::settings::QualityPreset::Low);
    for (entity, has_bloom, has_fog, has_hdr, tonemapping) in &cam_q {
        if enabled {
            if !has_hdr {
                commands.entity(entity).insert(Hdr);
            }
            if bloom_enabled {
                if !has_bloom {
                    commands.entity(entity).insert(Bloom::NATURAL);
                }
            } else if has_bloom {
                commands.entity(entity).remove::<Bloom>();
            }
            if !has_fog {
                commands.entity(entity).insert(default_distance_fog());
            }
            if tonemapping.is_none() || tonemapping == Some(&Tonemapping::None) {
                commands.entity(entity).insert(Tonemapping::TonyMcMapface);
            }
        } else {
            if has_bloom {
                commands.entity(entity).remove::<Bloom>();
            }
            if has_fog {
                commands.entity(entity).remove::<DistanceFog>();
            }
            if has_hdr {
                commands.entity(entity).remove::<Hdr>();
            }
            if tonemapping != Some(&Tonemapping::None) {
                commands.entity(entity).insert(Tonemapping::None);
            }
        }
    }
}

fn spawn_camera(mut commands: Commands, settings: Option<Res<crate::settings::Settings>>) {
    let (pos, yaw, pitch) = match std::env::var(ENV_CAMERA).ok().as_deref().map(parse_camera) {
        Some(Some(c)) => {
            commands.insert_resource(CameraOverride);
            c
        }
        Some(None) => {
            warn!("{ENV_CAMERA} must be x,y,z,yaw_deg,pitch_deg; ignored");
            (CAMERA_START, 0.0, -0.4)
        }
        None => (CAMERA_START, 0.0, -0.4),
    };
    let fov_deg = settings.as_ref().map_or(70.0, |s| s.fov);
    let atmosphere_fx = settings.as_ref().is_none_or(|s| s.atmosphere_fx);
    let mut entity = commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: fov_deg.to_radians(),
            far: 1000.0,
            ..default()
        }),
        Transform::from_translation(pos).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            yaw,
            pitch,
            0.0,
        )),
        FlyCamera { yaw, pitch },
        IsDefaultUiCamera,
        DespawnOnExit(AppState::InGame),
    ));
    if atmosphere_fx {
        entity.insert((
            Hdr,
            Bloom::NATURAL,
            Tonemapping::TonyMcMapface,
            default_distance_fog(),
        ));
    } else {
        entity.insert(Tonemapping::None);
    }
}

pub(crate) fn set_grab(opts: &mut CursorOptions, grab: bool) {
    opts.grab_mode = if grab {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    opts.visible = !grab;
}

fn grab_cursor(opts: Option<Single<&mut CursorOptions, With<PrimaryWindow>>>) {
    if let Some(mut opts) = opts {
        set_grab(&mut opts, true);
    }
}

fn release_cursor(opts: Option<Single<&mut CursorOptions, With<PrimaryWindow>>>) {
    if let Some(mut opts) = opts {
        set_grab(&mut opts, false);
    }
}

fn any_panel_open(
    session_error: Option<&crate::session::SessionError>,
    town: Option<&crate::hud::town::TownPanel>,
    chest: Option<&crate::hud::chest::ChestPanel>,
    game_menu: Option<&crate::session::GameMenu>,
    inventory_open: Option<&crate::hud::inventory::InventoryOpen>,
    victory: Option<&crate::hud::victory::VictoryModalState>,
    creative: Option<&crate::menu::creative::CreativeMenuOpen>,
) -> bool {
    if session_error.is_some_and(|e| e.0.is_some()) {
        return true;
    }
    if town.is_some_and(|t| t.is_open()) {
        return true;
    }
    if chest.is_some_and(|c| c.open.is_some()) {
        return true;
    }
    if game_menu.is_some_and(|gm| gm.panel.is_some()) {
        return true;
    }
    if inventory_open.is_some_and(|inv| inv.0) {
        return true;
    }
    if victory.is_some_and(|v| v.active) {
        return true;
    }
    if creative.is_some_and(|c| c.0) {
        return true;
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn toggle_grab(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut opts: Single<&mut CursorOptions, With<PrimaryWindow>>,
    chest: Option<Res<crate::hud::chest::ChestPanel>>,
    town: Option<Res<crate::hud::town::TownPanel>>,
    game_menu: Option<Res<crate::session::GameMenu>>,
    session_error: Option<Res<crate::session::SessionError>>,
    inventory_open: Option<Res<crate::hud::inventory::InventoryOpen>>,
    victory: Option<Res<crate::hud::victory::VictoryModalState>>,
    creative: Option<Res<crate::menu::creative::CreativeMenuOpen>>,
) {
    if session_error.as_ref().is_some_and(|e| e.0.is_some()) {
        if opts.grab_mode != CursorGrabMode::None || !opts.visible {
            set_grab(&mut opts, false);
        }
        return;
    }
    if any_panel_open(
        session_error.as_deref(),
        town.as_deref(),
        chest.as_deref(),
        game_menu.as_deref(),
        inventory_open.as_deref(),
        victory.as_deref(),
        creative.as_deref(),
    ) {
        return;
    }
    if game_menu.is_none() && keys.just_pressed(KeyCode::Escape) {
        set_grab(&mut opts, false);
    } else if mouse.just_pressed(MouseButton::Left) && opts.grab_mode == CursorGrabMode::None {
        set_grab(&mut opts, true);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn reassert_grab(
    mut focus_events: Option<MessageReader<WindowFocused>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut opts: Single<&mut CursorOptions, With<PrimaryWindow>>,
    chest: Option<Res<crate::hud::chest::ChestPanel>>,
    town: Option<Res<crate::hud::town::TownPanel>>,
    game_menu: Option<Res<crate::session::GameMenu>>,
    session_error: Option<Res<crate::session::SessionError>>,
    inventory_open: Option<Res<crate::hud::inventory::InventoryOpen>>,
    victory: Option<Res<crate::hud::victory::VictoryModalState>>,
    creative: Option<Res<crate::menu::creative::CreativeMenuOpen>>,
) {
    if any_panel_open(
        session_error.as_deref(),
        town.as_deref(),
        chest.as_deref(),
        game_menu.as_deref(),
        inventory_open.as_deref(),
        victory.as_deref(),
        creative.as_deref(),
    ) {
        return;
    }

    let mut gained_focus = false;
    if let Some(ref mut fe) = focus_events {
        for ev in fe.read() {
            if ev.focused {
                gained_focus = true;
            }
        }
    }

    if gained_focus || mouse.just_pressed(MouseButton::Left) {
        set_grab(&mut opts, true);
        opts.set_changed();
    }
}

pub(crate) fn look(
    motion: Res<AccumulatedMouseMotion>,
    opts: Single<&CursorOptions, With<PrimaryWindow>>,
    mut q: Single<(&mut FlyCamera, &mut Transform)>,
    mode: Res<ControlMode>,
    cmode: Res<CameraMode>,
    sensitivity: Option<Res<LookSensitivity>>,
    settings: Option<Res<crate::settings::Settings>>,
) {
    if opts.grab_mode == CursorGrabMode::None || motion.delta == Vec2::ZERO {
        return;
    }
    let limits = if *mode == ControlMode::Player && *cmode == CameraMode::Third {
        THIRD_PITCH
    } else {
        (-1.54, 1.54)
    };
    let sens = sensitivity.map_or(1.0, |s| s.0);
    let invert_y = settings.is_some_and(|s| s.invert_y);
    let (cam, tf) = &mut *q;
    let (new_yaw, new_pitch) =
        update_camera_angles(cam.yaw, cam.pitch, motion.delta, sens, invert_y, limits);
    cam.yaw = new_yaw;
    cam.pitch = new_pitch;
    tf.rotation = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);
}

fn fly(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut tf: Single<&mut Transform, With<FlyCamera>>,
) {
    let fwd = tf.forward().with_y(0.0).normalize_or_zero();
    let right = tf.right().with_y(0.0).normalize_or_zero();
    let mut dir = Vec3::ZERO;
    for (key, v) in [
        (KeyCode::KeyW, fwd),
        (KeyCode::KeyS, -fwd),
        (KeyCode::KeyD, right),
        (KeyCode::KeyA, -right),
        (KeyCode::Space, Vec3::Y),
        (KeyCode::ShiftLeft, Vec3::NEG_Y),
    ] {
        if keys.pressed(key) {
            dir += v;
        }
    }
    let speed = if keys.pressed(KeyCode::ControlLeft) {
        SPEED * FAST
    } else {
        SPEED
    };
    tf.translation += dir.normalize_or_zero() * speed * time.delta_secs();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::Messages;
    use terravibe_sim::{AIR, BlockId, DIRT};

    /// Solid wherever `z >= wall`, air elsewhere.
    struct Wall(i32);

    impl BlockQuery for Wall {
        fn block(&self, _x: i32, _y: i32, z: i32) -> BlockId {
            if z >= self.0 { DIRT } else { AIR }
        }
    }

    #[test]
    fn camera_pulls_in_before_wall() {
        // The wall's face is 2 blocks behind the eye (+z is back here).
        let d = pull_in(&Wall(2), Vec3::new(0.5, 5.5, 0.0), Vec3::Z, 5.5);
        assert!(d <= 1.7 + 1e-4 && d > 1.0, "{d}");
    }

    #[test]
    fn camera_full_distance_in_open_air() {
        let d = pull_in(&Wall(1000), Vec3::new(0.5, 5.5, 0.0), Vec3::Z, 5.5);
        assert_eq!(d, 5.5);
    }

    #[test]
    fn zoom_only_runs_with_alt_or_ctrl() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(OrbitDist(DEFAULT_DIST))
            .add_message::<MouseWheel>();
        app.add_systems(Update, zoom);

        let wheel_ev = |y| MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        };

        // Bare wheel: does NOT zoom
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MouseWheel>>()
            .write(wheel_ev(1.0));
        app.update();
        assert_eq!(app.world().resource::<OrbitDist>().0, DEFAULT_DIST);

        // Alt held: zooms
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::AltLeft);
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MouseWheel>>()
            .write(wheel_ev(1.0));
        app.update();
        assert!(app.world().resource::<OrbitDist>().0 < DEFAULT_DIST);
    }

    #[test]
    fn camera_override_parses() {
        let (p, yaw, pitch) = parse_camera("1, 2.5,-3,90,-45").unwrap();
        assert_eq!(p, Vec3::new(1.0, 2.5, -3.0));
        assert!((yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((pitch + std::f32::consts::FRAC_PI_4).abs() < 1e-6);
        assert!(parse_camera("1,2,3").is_none());
        assert!(parse_camera("a,2,3,4,5").is_none());
    }

    #[test]
    fn bench_pose_circles_and_looks_ahead() {
        let p0 = bench_pose(0.0, 80.0);
        assert!((p0.translation - Vec3::new(BENCH_RADIUS, 80.0, 0.0)).length() < 1e-3);
        let lap = bench_pose(BENCH_LAP_SECS, 80.0);
        assert!((lap.translation - p0.translation).length() < 1e-2);
        // At angle 0 the path heads +Z.
        let fwd = p0.forward().with_y(0.0).normalize();
        assert!((fwd - Vec3::Z).length() < 1e-3);
    }

    #[test]
    fn session_error_in_ingame_releases_cursor() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(bevy::window::WindowPlugin::default())
            .init_resource::<crate::session::SessionError>()
            .add_systems(Update, toggle_grab);

        if let Ok(mut opts) = app
            .world_mut()
            .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
        {
            opts.grab_mode = CursorGrabMode::Locked;
            opts.visible = false;
        }

        app.world_mut()
            .resource_mut::<crate::session::SessionError>()
            .0 = Some("connection lost".into());
        app.update();

        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(opts.grab_mode, CursorGrabMode::None);
        assert!(opts.visible);

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(opts.grab_mode, CursorGrabMode::None);
        assert!(opts.visible);
    }

    #[test]
    fn regrab_on_focus_gain_when_no_panel_open() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(bevy::window::WindowPlugin::default())
            .add_systems(Update, (toggle_grab, reassert_grab));

        #[derive(Resource, Default)]
        struct ChangedLog(bool);
        app.init_resource::<ChangedLog>();
        app.add_systems(
            PostUpdate,
            |opts: Single<Ref<CursorOptions>, With<PrimaryWindow>>, mut log: ResMut<ChangedLog>| {
                if opts.is_changed() {
                    log.0 = true;
                }
            },
        );

        let win_entity = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();

        if let Ok(mut opts) = app
            .world_mut()
            .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
        {
            opts.grab_mode = CursorGrabMode::Locked;
            opts.visible = false;
        }

        app.update();
        app.world_mut().resource_mut::<ChangedLog>().0 = false;

        app.world_mut()
            .resource_mut::<Messages<WindowFocused>>()
            .write(WindowFocused {
                window: win_entity,
                focused: true,
            });
        app.update();

        assert!(
            app.world().resource::<ChangedLog>().0,
            "focus gain must reassert grab by marking CursorOptions changed"
        );
        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(opts.grab_mode, CursorGrabMode::Locked);
    }

    #[test]
    fn click_reasserts_grab_even_if_ecs_says_locked() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(bevy::window::WindowPlugin::default())
            .add_systems(Update, (toggle_grab, reassert_grab));

        #[derive(Resource, Default)]
        struct ChangedLog(bool);
        app.init_resource::<ChangedLog>();
        app.add_systems(
            PostUpdate,
            |opts: Single<Ref<CursorOptions>, With<PrimaryWindow>>, mut log: ResMut<ChangedLog>| {
                if opts.is_changed() {
                    log.0 = true;
                }
            },
        );

        if let Ok(mut opts) = app
            .world_mut()
            .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
        {
            opts.grab_mode = CursorGrabMode::Locked;
            opts.visible = false;
        }

        app.update();
        app.world_mut().resource_mut::<ChangedLog>().0 = false;

        app.world_mut()
            .resource_mut::<Messages<bevy::input::mouse::MouseButtonInput>>()
            .write(bevy::input::mouse::MouseButtonInput {
                button: MouseButton::Left,
                state: bevy::input::ButtonState::Pressed,
                window: Entity::PLACEHOLDER,
            });
        app.update();

        assert!(
            app.world().resource::<ChangedLog>().0,
            "left click must reassert grab even when ecs says locked"
        );
        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(opts.grab_mode, CursorGrabMode::Locked);
    }

    #[test]
    fn no_regrab_while_inventory_open() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(bevy::window::WindowPlugin::default())
            .insert_resource(crate::hud::inventory::InventoryOpen(true))
            .add_systems(Update, (toggle_grab, reassert_grab));

        #[derive(Resource, Default)]
        struct ChangedLog(bool);
        app.init_resource::<ChangedLog>();
        app.add_systems(
            PostUpdate,
            |opts: Single<Ref<CursorOptions>, With<PrimaryWindow>>, mut log: ResMut<ChangedLog>| {
                if opts.is_changed() {
                    log.0 = true;
                }
            },
        );

        let win_entity = app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();

        if let Ok(mut opts) = app
            .world_mut()
            .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
        {
            opts.grab_mode = CursorGrabMode::None;
            opts.visible = true;
        }

        app.update();
        app.world_mut().resource_mut::<ChangedLog>().0 = false;

        // Gaining focus and clicking while inventory is open must NOT re-grab cursor
        app.world_mut()
            .resource_mut::<Messages<WindowFocused>>()
            .write(WindowFocused {
                window: win_entity,
                focused: true,
            });
        app.world_mut()
            .resource_mut::<Messages<bevy::input::mouse::MouseButtonInput>>()
            .write(bevy::input::mouse::MouseButtonInput {
                button: MouseButton::Left,
                state: bevy::input::ButtonState::Pressed,
                window: Entity::PLACEHOLDER,
            });
        app.update();

        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(opts.grab_mode, CursorGrabMode::None);
        assert!(
            !app.world().resource::<ChangedLog>().0,
            "must not change grab while inventory is open"
        );
    }

    #[test]
    fn no_grab_while_creative_menu_open() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(bevy::window::WindowPlugin::default())
            .insert_resource(crate::menu::creative::CreativeMenuOpen(true))
            .add_systems(Update, (toggle_grab, reassert_grab));

        if let Ok(mut opts) = app
            .world_mut()
            .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
        {
            opts.grab_mode = CursorGrabMode::None;
            opts.visible = true;
        }

        app.update();
        app.world_mut()
            .resource_mut::<Messages<bevy::input::mouse::MouseButtonInput>>()
            .write(bevy::input::mouse::MouseButtonInput {
                button: MouseButton::Left,
                state: bevy::input::ButtonState::Pressed,
                window: Entity::PLACEHOLDER,
            });
        app.update();

        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(opts.grab_mode, CursorGrabMode::None);
        assert!(
            opts.visible,
            "cursor must stay visible while creative menu is open"
        );
    }

    #[test]
    fn camera_trauma_zero_when_disabled_or_reduced_motion() {
        let mut trauma = CameraTrauma::default();
        trauma.add(0.8);
        assert!(trauma.trauma > 0.0);

        let enabled = crate::settings::Settings {
            screen_shake: 1.0,
            reduced_motion: false,
            ..Default::default()
        };
        assert_ne!(trauma.offset(&enabled, 0.25), Vec3::ZERO);

        let zero_shake = crate::settings::Settings {
            screen_shake: 0.0,
            reduced_motion: false,
            ..Default::default()
        };
        assert_eq!(trauma.offset(&zero_shake, 0.25), Vec3::ZERO);

        let reduced = crate::settings::Settings {
            screen_shake: 1.0,
            reduced_motion: true,
            ..Default::default()
        };
        assert_eq!(trauma.offset(&reduced, 0.25), Vec3::ZERO);
    }

    #[test]
    fn hitstop_respects_settings_toggle() {
        let mut hs = HitstopState::default();
        let off = crate::settings::Settings {
            hitstop: false,
            ..Default::default()
        };
        hs.trigger(0.06, &off);
        assert_eq!(hs.remaining, 0.0);

        let on = crate::settings::Settings {
            hitstop: true,
            ..Default::default()
        };
        hs.trigger(0.06, &on);
        assert!(hs.remaining > 0.0);
    }

    #[test]
    fn cave_fog_scales_with_depth_and_toggle() {
        assert_eq!(cave_fog_density(0.0, true), 0.0);
        assert_eq!(cave_fog_density(-10.0, true), 0.0);
        let shallow = cave_fog_density(20.0, true);
        let deep = cave_fog_density(64.0, true);
        assert!(shallow > 0.0 && shallow < 1.0);
        assert!(deep > shallow && deep <= 1.0);
        assert_eq!(cave_fog_density(64.0, false), 0.0);
    }

    #[test]
    fn fly_camera_atmosphere_fx_toggles_bloom_tonemapping_and_fog() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(crate::settings::Settings {
                atmosphere_fx: true,
                ..Default::default()
            })
            .add_systems(Update, sync_camera_atmosphere_fx);

        let cam = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                FlyCamera {
                    yaw: 0.0,
                    pitch: 0.0,
                },
            ))
            .id();

        app.update();
        assert!(app.world().entity(cam).contains::<Bloom>());
        assert!(app.world().entity(cam).contains::<DistanceFog>());
        assert!(app.world().entity(cam).contains::<Hdr>());
        assert_ne!(
            app.world().entity(cam).get::<Tonemapping>().copied(),
            Some(Tonemapping::None)
        );

        app.world_mut()
            .resource_mut::<crate::settings::Settings>()
            .atmosphere_fx = false;
        app.update();
        assert!(!app.world().entity(cam).contains::<Bloom>());
        assert!(!app.world().entity(cam).contains::<DistanceFog>());
        assert!(!app.world().entity(cam).contains::<Hdr>());
        assert_eq!(
            app.world().entity(cam).get::<Tonemapping>().copied(),
            Some(Tonemapping::None)
        );

        app.world_mut()
            .resource_mut::<crate::settings::Settings>()
            .atmosphere_fx = true;
        app.world_mut()
            .resource_mut::<crate::settings::Settings>()
            .quality_preset = crate::settings::QualityPreset::Low;
        app.update();
        assert!(!app.world().entity(cam).contains::<Bloom>());
        assert!(app.world().entity(cam).contains::<DistanceFog>());
    }

    #[test]
    fn damp_pull_in_is_faster_when_obstructed_than_clearing() {
        let pulled_in = damp_pull_in(5.5, 2.0, 0.05);
        let eased_out = damp_pull_in(2.0, 5.5, 0.05);
        let pull_frac = (5.5 - pulled_in) / 3.5;
        let ease_frac = (eased_out - 2.0) / 3.5;
        assert!(
            pull_frac > ease_frac * 1.5,
            "pull-in ({pull_frac}) should be much faster than ease-out ({ease_frac})"
        );
    }

    #[test]
    fn dynamic_fov_boosts_on_hermes_sprint_and_respects_reduced_motion() {
        let base = 70.0;
        assert_eq!(target_fov_deg(base, 0.0, false, false), base);
        let boosted = target_fov_deg(base, 0.25, true, false);
        assert!(boosted > base + 3.0 && boosted <= base + 6.0);
        assert_eq!(target_fov_deg(base, 0.25, true, true), base);
    }

    #[test]
    fn third_person_pivot_centered_above_head() {
        assert_eq!(SHOULDER_RIGHT, 0.0);
        assert_eq!(SHOULDER_UP, 0.15);
    }
}
