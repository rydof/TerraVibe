//! First-person controller: samples input in `Update`, sends it to the server
//! and predicts the player at a fixed 60 Hz, and places the camera at the
//! interpolated eye in `PostUpdate`.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use terravibe_assets::TerrariaContent;
use terravibe_sim::net::ClientMsg;
use terravibe_sim::player::{Player, PlayerEvent, tick_player_bounded};
use terravibe_sim::{
    BlockId, BlockQuery, CHUNK_SIZE, Chunk, ChunkPos, Liquid, LiquidKind, PlayerInput,
    SOLID_UNLOADED,
};

use bevy::input::mouse::MouseWheel;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use super::camera::{
    CameraMode, ControlMode, FlyCamera, OrbitDist, SHOULDER_RIGHT, SHOULDER_UP, SmoothedOrbitDist,
    damp_pull_in, look, pull_in, target_fov_deg,
};
use super::chunks::ChunkStream;
use crate::net::{Net, NetState, SMOOTH_SECS};
use terravibe_sim::net::DamageKind;

use crate::audio::{FootstepMaterial, SfxEvent, SfxKind, footstep_material_for_block};
use crate::session::AppState;

/// Eye height above the feet, in blocks.
pub(crate) const EYE_HEIGHT: f32 = 1.62;

/// The local player: a predicted copy, reconciled against the server's acks.
#[derive(Resource)]
pub struct LocalPlayer(pub Player);

/// Player events for other systems (sound, HUD).
#[derive(Message)]
pub struct PlayerEvents(pub PlayerEvent);

/// Block lookup over the generated chunks; missing chunks are solid.
pub struct LoadedBlocks<'a>(pub &'a HashMap<ChunkPos, Arc<Chunk>>);

impl BlockQuery for LoadedBlocks<'_> {
    fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
        match self.0.get(&ChunkPos::of_block(x, y, z)) {
            Some(c) => c.get(
                x.rem_euclid(CHUNK_SIZE) as usize,
                y.rem_euclid(CHUNK_SIZE) as usize,
                z.rem_euclid(CHUNK_SIZE) as usize,
            ),
            None => SOLID_UNLOADED,
        }
    }
    fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid> {
        let c = self.0.get(&ChunkPos::of_block(x, y, z))?;
        c.liquid(
            x.rem_euclid(CHUNK_SIZE) as usize,
            y.rem_euclid(CHUNK_SIZE) as usize,
            z.rem_euclid(CHUNK_SIZE) as usize,
        )
    }
}

/// Eye positions of the last two fixed ticks, for render interpolation.
#[derive(Resource)]
pub(crate) struct PlayerView {
    pub(crate) prev_eye: Vec3,
    pub(crate) eye: Vec3,
    /// Correction smoothing: added to the camera, decays to zero.
    pub(crate) offset: Vec3,
}

/// Input sampled in `Update`, consumed by `FixedUpdate`.
#[derive(Resource, Default)]
pub(crate) struct SampledInput {
    pub(crate) input: PlayerInput,
    /// A jump press that no fixed tick has seen yet.
    jump_latch: bool,
}

/// Raw wav bytes for player sounds.
pub struct PlayerSounds {
    hits: Vec<Vec<u8>>,
    killed: Option<Vec<u8>>,
}

impl PlayerSounds {
    /// Loads what exists; missing sounds are skipped with a warning.
    pub fn load(c: &TerrariaContent) -> Self {
        let get = |name: String| match c.sound(&name) {
            Ok(s) => Some(s.to_wav()),
            Err(e) => {
                warn!("{name}: {e}");
                None
            }
        };
        PlayerSounds {
            hits: (0..3)
                .filter_map(|i| get(format!("Sounds/Player_Hit_{i}")))
                .collect(),
            killed: get("Sounds/Player_Killed".into()),
        }
    }
}

#[derive(Resource)]
#[allow(dead_code)]
struct PlayerAudio {
    hits: Vec<Handle<AudioSource>>,
    killed: Option<Handle<AudioSource>>,
}

pub struct PlayerPlugin {
    pub sounds: PlayerSounds,
}

fn player_mode(m: Res<ControlMode>) -> bool {
    *m == ControlMode::Player
}

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        let mut add = |bytes: &Vec<u8>| {
            let bytes: Arc<[u8]> = bytes.clone().into();
            app.world_mut()
                .resource_mut::<Assets<AudioSource>>()
                .add(AudioSource { bytes })
        };
        let audio = PlayerAudio {
            hits: self.sounds.hits.iter().map(&mut add).collect(),
            killed: self.sounds.killed.as_ref().map(&mut add),
        };
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .insert_resource(audio)
            .init_resource::<SampledInput>()
            .init_resource::<FootstepState>()
            .add_message::<PlayerEvents>()
            .add_systems(
                Update,
                (
                    sample_input
                        .after(look)
                        .run_if(resource_exists::<LocalPlayer>)
                        .run_if(crate::menu::menu_closed),
                    play_sounds,
                )
                    .run_if(player_mode)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                FixedUpdate,
                fixed_tick
                    .run_if(player_mode)
                    .run_if(resource_exists::<LocalPlayer>)
                    .run_if(resource_exists::<PlayerView>)
                    .run_if(in_state(AppState::InGame))
                    .run_if(crate::menu::menu_closed),
            )
            .add_systems(
                PostUpdate,
                place_camera
                    .run_if(player_mode)
                    .run_if(resource_exists::<PlayerView>)
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

pub(crate) fn eye_of(p: &Player) -> Vec3 {
    Vec3::from(p.body.pos) + Vec3::Y * EYE_HEIGHT
}

/// Jump for one fixed tick: the first tick after a press sees the latched
/// press; later ticks in the same frame see only the key state at that frame.
fn jump_for_tick(latch: bool, held: bool) -> bool {
    latch || held
}

const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Digit0,
];

/// Hotbar slot after the mouse wheel moved `scroll` lines (up = previous slot).
fn scrolled_slot(slot: u8, scroll: f32) -> u8 {
    let step = -(scroll.signum() as i32) * (scroll != 0.0) as i32;
    (slot as i32 + step).rem_euclid(10) as u8
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sample_input(
    mouse: Res<ButtonInput<MouseButton>>,
    mut wheel: MessageReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    cam: Single<&FlyCamera>,
    opts: Single<&CursorOptions, With<PrimaryWindow>>,
    mut s: ResMut<SampledInput>,
    _cmode: Res<CameraMode>,
    inventory: Option<Res<crate::hud::inventory::InventoryOpen>>,
    chest: Option<Res<crate::hud::chest::ChestPanel>>,
    town: Option<Res<crate::hud::town::TownPanel>>,
    victory: Option<Res<crate::hud::victory::VictoryModalState>>,
    creative: Option<Res<crate::menu::creative::CreativeMenuOpen>>,
    settings: Option<Res<crate::settings::Settings>>,
    gamepads: Query<&Gamepad>,
) {
    let mut slot = s.input.slot;
    for (i, k) in DIGITS.iter().enumerate() {
        if keys.just_pressed(*k) {
            slot = i as u8;
        }
    }
    // Bare wheel scrolls hotbar; Alt or Ctrl + wheel is reserved for camera zoom.
    let modifier = keys.pressed(KeyCode::AltLeft)
        || keys.pressed(KeyCode::AltRight)
        || keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight);
    for e in wheel.read() {
        if !modifier {
            slot = scrolled_slot(slot, e.y);
        }
    }
    for gp in &gamepads {
        let delta = crate::hud::hotbar::gamepad_hotbar_delta(gp);
        if delta != 0 {
            slot = crate::hud::hotbar::cycle_slot(slot, delta);
        }
    }
    // An open inventory, chest or shop never attacks, mines or places, whatever the
    // cursor mode.
    let ui_open = opts.grab_mode == CursorGrabMode::None
        || inventory.is_some_and(|i| i.0)
        || chest.is_some_and(|c| c.open.is_some())
        || town.is_some_and(|t| t.shop_open())
        || victory.is_some_and(|v| v.active)
        || creative.is_some_and(|c| c.0);
    if ui_open {
        s.input = PlayerInput {
            yaw: cam.yaw,
            pitch: cam.pitch,
            slot,
            ..default()
        };
        return;
    }
    let bindings = settings
        .as_deref()
        .map_or_else(crate::input_map::InputBindings::default, |s| s.bindings);
    let axis = |pos: KeyCode, neg: KeyCode| {
        f32::from(u8::from(keys.pressed(pos))) - f32::from(u8::from(keys.pressed(neg)))
    };
    let jump_key = bindings.key(crate::input_map::GameAction::Jump);
    let back_key = bindings.key(crate::input_map::GameAction::MoveBack);
    let gp_move = gamepads
        .iter()
        .next()
        .map(|g| crate::gamepad::read_gamepad_move_look(g, crate::gamepad::STICK_DEADZONE).0)
        .unwrap_or(Vec2::ZERO);
    let gp_jump = gamepads.iter().any(|g| g.pressed(GamepadButton::South));
    let gp_jump_just = gamepads
        .iter()
        .any(|g| g.just_pressed(GamepadButton::South));
    let gp_attack = gamepads
        .iter()
        .any(|g| crate::view::interact::gamepad_attack_and_use(g).0);
    if keys.just_pressed(jump_key) || gp_jump_just {
        s.jump_latch = true;
    }
    s.input = PlayerInput {
        forward: (axis(
            bindings.key(crate::input_map::GameAction::MoveForward),
            back_key,
        ) + gp_move.y)
            .clamp(-1.0, 1.0),
        strafe: (axis(
            bindings.key(crate::input_map::GameAction::MoveRight),
            bindings.key(crate::input_map::GameAction::MoveLeft),
        ) + gp_move.x)
            .clamp(-1.0, 1.0),
        jump: keys.pressed(jump_key) || gp_jump,
        // S also drops through one-way platforms (Terraria: controlDown).
        drop: keys.pressed(back_key),
        yaw: cam.yaw,
        pitch: cam.pitch,
        use_held: mouse.pressed(MouseButton::Left) || gp_attack,
        slot,
    };
}

/// Horizontal distance (in blocks) between consecutive footstep sounds.
const FOOTSTEP_STRIDE: f32 = 1.8;

/// Tracks ground/liquid transitions and stride accumulation for footstep and landing SFX.
#[derive(Resource, Default)]
pub(crate) struct FootstepState {
    pub was_on_ground: bool,
    pub was_wet: bool,
    pub step_accum: f32,
}

/// Determines the [`FootstepMaterial`] at the player's feet.
pub(crate) fn player_footstep_material(
    q: &impl BlockQuery,
    pos: [f32; 3],
    wet: Option<LiquidKind>,
) -> FootstepMaterial {
    if wet.is_some() {
        return FootstepMaterial::Liquid;
    }
    let fx = pos[0].floor() as i32;
    let fy = (pos[1] - 0.1).floor() as i32;
    let fz = pos[2].floor() as i32;
    if q.liquid(fx, fy, fz).is_some() {
        return FootstepMaterial::Liquid;
    }
    footstep_material_for_block(q.block(fx, fy, fz))
}

/// Advances [`FootstepState`] by one fixed tick and returns any footstep or landing [`SfxKind`].
pub(crate) fn step_footstep_tracker(
    state: &mut FootstepState,
    prev_pos: [f32; 3],
    pos: [f32; 3],
    on_ground: bool,
    wet: Option<LiquidKind>,
    material: FootstepMaterial,
) -> Option<SfxKind> {
    let is_wet = wet.is_some();
    let landed = !state.was_on_ground && on_ground;
    let entered_liquid = !state.was_wet && is_wet;
    state.was_on_ground = on_ground;
    state.was_wet = is_wet;

    if entered_liquid {
        state.step_accum = 0.0;
        return Some(SfxKind::Land(FootstepMaterial::Liquid));
    }
    if landed {
        state.step_accum = 0.0;
        return Some(SfxKind::Land(material));
    }
    if !on_ground {
        state.step_accum = 0.0;
        return None;
    }

    let dx = pos[0] - prev_pos[0];
    let dz = pos[2] - prev_pos[2];
    let horiz = (dx * dx + dz * dz).sqrt();
    if horiz > 0.001 {
        state.step_accum += horiz;
        if state.step_accum >= FOOTSTEP_STRIDE {
            state.step_accum -= FOOTSTEP_STRIDE;
            return Some(SfxKind::Footstep(material));
        }
    } else {
        state.step_accum = 0.0;
    }
    None
}

/// Sends this tick's input to the server and predicts it locally. Events from
/// prediction are dropped (the server's events play the sounds).
fn fixed_tick(
    stream: Res<ChunkStream>,
    net: Res<Net>,
    mut st: ResMut<NetState>,
    mut player: ResMut<LocalPlayer>,
    mut view: ResMut<PlayerView>,
    mut sampled: ResMut<SampledInput>,
    mut footsteps: ResMut<FootstepState>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    if !st.check_ready(&stream) {
        return; // frozen until the spawn surroundings are loaded
    }
    let input = PlayerInput {
        jump: jump_for_tick(sampled.jump_latch, sampled.input.jump),
        ..sampled.input
    };
    sampled.jump_latch = false;
    st.seq += 1;
    let seq = st.seq;
    // A send error means the server is gone; `net::pump` reports it.
    let _ = net.send(ClientMsg::Input { seq, input });
    st.pending.push_back((seq, input));
    view.prev_eye = view.eye;
    if !player.0.dead {
        let prev_pos = player.0.body.pos;
        let q = LoadedBlocks(stream.generated());
        tick_player_bounded(&q, &mut player.0, &input, Some(&st.bounds));
        let mat = player_footstep_material(&q, player.0.body.pos, player.0.body.wet);
        if let Some(kind) = step_footstep_tracker(
            &mut footsteps,
            prev_pos,
            player.0.body.pos,
            player.0.body.on_ground,
            player.0.body.wet,
            mat,
        ) {
            sfx.write(SfxEvent(kind));
        }
    }
    view.eye = eye_of(&player.0);
}

#[allow(clippy::too_many_arguments)]
fn place_camera(
    mut view: ResMut<PlayerView>,
    fixed: Res<Time<Fixed>>,
    time: Res<Time<Real>>,
    mut cam_q: Single<(&mut Transform, &mut Projection), With<FlyCamera>>,
    cmode: Res<CameraMode>,
    dist: Res<OrbitDist>,
    mut smoothed_dist: Option<ResMut<SmoothedOrbitDist>>,
    stream: Res<ChunkStream>,
    player: Option<Res<LocalPlayer>>,
    trauma: Option<Res<super::camera::CameraTrauma>>,
    settings: Option<Res<crate::settings::Settings>>,
) {
    let dt = time.delta_secs();
    let k = (1.0 - dt / SMOOTH_SECS).max(0.0);
    view.offset *= k;
    let eye = view.prev_eye.lerp(view.eye, fixed.overstep_fraction()) + view.offset;
    let (ref mut tf, ref mut proj) = *cam_q;
    let base = if *cmode == CameraMode::Third {
        let blocks = LoadedBlocks(stream.generated());
        // Vertical pivot offset so the crosshair clears the player's head while
        // keeping straight-ahead aim centered, clamped by wall pull-in.
        let shoulder_vec = *tf.right() * SHOULDER_RIGHT + *tf.up() * SHOULDER_UP;
        let shoulder_len = shoulder_vec.length();
        let shoulder_eye = if shoulder_len > 1e-5 {
            let dir = shoulder_vec / shoulder_len;
            eye + dir * pull_in(&blocks, eye, dir, shoulder_len)
        } else {
            eye
        };
        let back = *tf.back();
        let target_d = pull_in(&blocks, shoulder_eye, back, dist.0);
        let d = if let Some(ref mut sd) = smoothed_dist {
            sd.0 = damp_pull_in(sd.0, target_d, dt);
            sd.0
        } else {
            target_d
        };
        shoulder_eye + back * d
    } else {
        if let Some(ref mut sd) = smoothed_dist {
            sd.0 = dist.0;
        }
        eye
    };
    let s = settings.as_deref().copied().unwrap_or_default();
    let shake = trauma
        .as_ref()
        .map(|tr| tr.offset(&s, time.elapsed_secs()))
        .unwrap_or(Vec3::ZERO);
    tf.translation = base + shake;

    if let Projection::Perspective(persp) = &mut **proj {
        let (horiz_speed, hermes_sprinting) = player
            .as_ref()
            .map(|p| {
                let b = &p.0.body;
                let hs = b.vel[0].hypot(b.vel[2]);
                let hs_boost = b.has_hermes_boots && b.run_ticks >= 20 && hs > 0.12;
                (hs, hs_boost)
            })
            .unwrap_or((0.0, false));
        let desired_fov =
            target_fov_deg(s.fov, horiz_speed, hermes_sprinting, s.reduced_motion).to_radians();
        let alpha = 1.0 - (-8.0 * dt.max(0.0)).exp();
        persp.fov += (desired_fov - persp.fov) * alpha;
    }
}

fn play_sounds(mut events: MessageReader<PlayerEvents>, mut sfx: MessageWriter<SfxEvent>) {
    for PlayerEvents(e) in events.read() {
        match e {
            PlayerEvent::Hurt { .. } => {
                sfx.write(SfxEvent(SfxKind::PlayerHurt(DamageKind::Unknown)));
            }
            PlayerEvent::Died { .. } => {
                sfx.write(SfxEvent(SfxKind::Death));
            }
            _ => {}
        }
    }
}

/// Angle through which a melee item swings at normalized progress `t` in [0, 1].
/// Terraria's useStyle 1 raises the blade back (0.7 rad) and brings it down forward (-1.4 rad).
pub fn swing_angle(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    0.7 - 2.1 * (t * t * (3.0 - 2.0 * t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::DIRT;

    #[test]
    fn jump_one_tick_for_tap_and_every_tick_when_held() {
        // Tap released before the frame: only the first tick jumps.
        let mut latch = true;
        let ticks: Vec<bool> = (0..3)
            .map(|_| {
                let j = jump_for_tick(latch, false);
                latch = false;
                j
            })
            .collect();
        assert_eq!(ticks, [true, false, false]);
        // Held key: every tick.
        assert!(jump_for_tick(true, true) && jump_for_tick(false, true));
    }

    #[test]
    fn melee_swing_arc_raises_then_comes_down_forward() {
        // Terraria's useStyle 1: the blade starts raised behind the head, comes
        // down through the forward arc, and ends at the bottom of the swing.
        assert!((swing_angle(0.0) - 0.7).abs() < 1e-6);
        assert!((swing_angle(0.5) + 0.35).abs() < 1e-6);
        assert!((swing_angle(1.0) + 1.4).abs() < 1e-6);
        let mut prev = f32::INFINITY;
        for i in 0..=100 {
            let a = swing_angle(i as f32 / 100.0);
            assert!(
                a <= prev + 1e-6,
                "arc goes back up at t={}",
                i as f32 / 100.0
            );
            prev = a;
        }
    }

    #[test]
    fn loaded_blocks_query() {
        let mut m: HashMap<ChunkPos, Arc<Chunk>> = HashMap::new();
        m.insert(ChunkPos { x: 0, y: 0, z: 0 }, Arc::new(Chunk::filled(DIRT)));
        let q = LoadedBlocks(&m);
        assert_eq!(q.block(3, 5, 7), DIRT);
        assert_eq!(q.block(0, 40, 0), SOLID_UNLOADED);
        assert_eq!(q.block(-1, 0, 0), SOLID_UNLOADED);
    }

    #[test]
    fn left_click_does_not_attack_while_shop_open() {
        use crate::hud::chest::ChestPanel;
        use crate::hud::inventory::InventoryOpen;
        use crate::hud::town::TownPanel;
        use terravibe_sim::enemy::EnemyId;
        use terravibe_sim::item::TORCH;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<SampledInput>()
            .init_resource::<InventoryOpen>()
            .init_resource::<ChestPanel>()
            .add_message::<MouseWheel>()
            .insert_resource(CameraMode::First)
            .insert_resource(TownPanel {
                npc: Some(EnemyId(1)),
                shop: vec![(TORCH, 50)],
                ..default()
            });
        app.world_mut().spawn(FlyCamera {
            yaw: 0.0,
            pitch: 0.0,
        });
        let mut window = Window::default();
        window.resolution.set(1280.0, 720.0);
        let opts = CursorOptions {
            grab_mode: CursorGrabMode::Locked,
            visible: false,
            ..default()
        };
        app.world_mut().spawn((window, PrimaryWindow, opts));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.add_systems(Update, sample_input);

        app.update();
        assert!(
            !app.world().resource::<SampledInput>().input.use_held,
            "left click must not attack while the shop is open"
        );

        // A chest open on its own (without the inventory flag) gates too.
        app.world_mut().resource_mut::<TownPanel>().shop.clear();
        app.world_mut().resource_mut::<ChestPanel>().open = Some([0, 0, 0]);
        app.update();
        assert!(!app.world().resource::<SampledInput>().input.use_held);

        // Control: with no panel open the same click does use the held item.
        app.world_mut().resource_mut::<ChestPanel>().open = None;
        app.update();
        assert!(app.world().resource::<SampledInput>().input.use_held);
    }

    #[test]
    fn bare_wheel_scrolls_hotbar_and_alt_or_ctrl_suppresses_it() {
        use crate::hud::chest::ChestPanel;
        use crate::hud::inventory::InventoryOpen;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<SampledInput>()
            .init_resource::<InventoryOpen>()
            .init_resource::<ChestPanel>()
            .add_message::<MouseWheel>()
            .insert_resource(CameraMode::Third);
        app.world_mut().spawn(FlyCamera {
            yaw: 0.0,
            pitch: 0.0,
        });
        let window = Window::default();
        let opts = CursorOptions {
            grab_mode: CursorGrabMode::Locked,
            visible: false,
            ..default()
        };
        app.world_mut().spawn((window, PrimaryWindow, opts));
        app.add_systems(Update, sample_input);

        let wheel_ev = |y| MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        };

        // Bare mouse wheel in third person scrolls hotbar
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MouseWheel>>()
            .write(wheel_ev(-1.0)); // down = next slot (0 -> 1)
        app.update();
        assert_eq!(app.world().resource::<SampledInput>().input.slot, 1);

        // Alt held down: mouse wheel does NOT scroll hotbar
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::AltLeft);
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MouseWheel>>()
            .write(wheel_ev(-1.0));
        app.update();
        assert_eq!(app.world().resource::<SampledInput>().input.slot, 1);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::AltLeft);

        // Ctrl held down: mouse wheel does NOT scroll hotbar
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ControlLeft);
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MouseWheel>>()
            .write(wheel_ev(-1.0));
        app.update();
        assert_eq!(app.world().resource::<SampledInput>().input.slot, 1);
    }

    #[test]
    fn footstep_tracker_emits_land_and_stride_footsteps_with_material() {
        let mut tracker = FootstepState::default();
        // Landing on Stone
        assert_eq!(
            step_footstep_tracker(
                &mut tracker,
                [0.0, 10.5, 0.0],
                [0.0, 10.0, 0.0],
                true,
                None,
                FootstepMaterial::Stone,
            ),
            Some(SfxKind::Land(FootstepMaterial::Stone))
        );
        // Moving horizontally < stride does not emit yet
        assert_eq!(
            step_footstep_tracker(
                &mut tracker,
                [0.0, 10.0, 0.0],
                [1.0, 10.0, 0.0],
                true,
                None,
                FootstepMaterial::Wood,
            ),
            None
        );
        // Reaching stride emits Footstep(Wood)
        assert_eq!(
            step_footstep_tracker(
                &mut tracker,
                [1.0, 10.0, 0.0],
                [2.0, 10.0, 0.0],
                true,
                None,
                FootstepMaterial::Wood,
            ),
            Some(SfxKind::Footstep(FootstepMaterial::Wood))
        );
        // Entering liquid emits Land(Liquid)
        assert_eq!(
            step_footstep_tracker(
                &mut tracker,
                [2.0, 10.0, 0.0],
                [2.5, 9.5, 0.0],
                false,
                Some(LiquidKind::Water),
                FootstepMaterial::Liquid,
            ),
            Some(SfxKind::Land(FootstepMaterial::Liquid))
        );
    }
}
