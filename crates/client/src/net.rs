//! Client side of the in-process server: starts it, sends hello/inputs/spectate
//! positions, applies chunks and acks, and reconciles the predicted player.

use std::collections::VecDeque;
use std::sync::Mutex;

use bevy::prelude::*;
use terravibe_sim::crafting::{CraftError, RecipeId};
use terravibe_sim::net::{ClientMsg, ClientTransport, Disconnected, ServerMsg};
use terravibe_sim::player::Player;
use terravibe_sim::{
    BlockQuery, ChunkPos, PlayerBody, PlayerInput, WorldBounds, step_body_bounded,
};

pub use crate::session::join_server;
use crate::session::{AppState, Automation, DisconnectState, SelectedWorld, SessionError};
use crate::view::SEED;
use crate::view::camera::{ControlMode, FlyCamera};
use crate::view::chunks::{ChunkStream, stream_chunks};
use crate::view::drops::DropMsg;
use crate::view::enemies::EnemyMsg;
use crate::view::player::{
    LoadedBlocks, LocalPlayer, PlayerEvents, PlayerView, SampledInput, eye_of,
};

/// Spectator position updates per second.
const SPECTATE_HZ: f32 = 10.0;
/// Predicted eye moves below this (blocks) are not smoothed.
const SMOOTH_MIN: f32 = 0.01;
/// Predicted eye moves above this (blocks) snap instead of smoothing.
const SNAP_DIST: f32 = 2.0;
/// Seconds over which a correction fades out.
pub const SMOOTH_SECS: f32 = 0.1;

/// The client's end of the server connection.
#[derive(Resource)]
pub struct Net(Mutex<Box<dyn ClientTransport>>);

impl Net {
    pub fn new(client: impl ClientTransport + 'static) -> Self {
        Self(Mutex::new(Box::new(client)))
    }

    pub fn send(&self, m: ClientMsg) -> Result<(), Disconnected> {
        self.0.lock().unwrap().send(m)
    }

    fn try_recv(&self) -> Result<Option<ServerMsg>, Disconnected> {
        self.0.lock().unwrap().try_recv()
    }
}

/// A server answer to one `Craft` request, for the crafting panel.
#[derive(Message, Clone, Debug)]
pub struct CraftResults {
    pub recipe: RecipeId,
    pub crafted: u32,
    pub error: Option<CraftError>,
}

/// Chest panel updates from the server.
#[derive(Message, Clone, Debug)]
pub enum ChestMsg {
    Contents {
        pos: [i32; 3],
        slots: Vec<Option<terravibe_sim::inventory::ItemStack>>,
    },
    Closed,
}

/// The server's copy of the local player's inventory.
#[derive(Resource)]
pub struct LocalInventory(pub terravibe_sim::inventory::Inventory);

/// The stack on the cursor, from `ServerMsg::Held`.
#[derive(Resource, Default)]
pub struct LocalHeld(pub Option<terravibe_sim::inventory::ItemStack>);

/// The material stack in the Guide's crafting lookup slot, from `ServerMsg::GuideSlot`.
#[derive(Resource, Default)]
pub struct LocalGuideSlot(pub Option<terravibe_sim::inventory::ItemStack>);

/// Whether Creative God Mode is active on the server, synced via `ServerMsg::GodMode`.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientGodMode(pub bool);

/// The server's worn armor (head, body, legs), from `ServerMsg::Armor`.
#[derive(Resource)]
pub struct LocalArmor(pub [Option<terravibe_sim::item::ItemId>; 3]);

/// The player's equipped accessories (5 slots). Client-local until the server syncs them.
#[derive(Resource, Default)]
pub struct LocalAccessories(pub [Option<terravibe_sim::item::ItemId>; 5]);

/// Prefixes for the player's 5 equipped accessories.
#[derive(Resource)]
pub struct LocalAccessoryPrefixes(pub [terravibe_sim::item::Prefix; 5]);

impl Default for LocalAccessoryPrefixes {
    fn default() -> Self {
        Self([terravibe_sim::item::Prefix(0); 5])
    }
}

/// The player's vanity / social armor (3 slots) and accessories (5 slots + prefixes),
/// synced from `ServerMsg::Vanity`.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct ClientVanity {
    pub armor: [Option<terravibe_sim::item::ItemId>; 3],
    pub accessories: [Option<terravibe_sim::item::ItemId>; 5],
    pub accessory_prefixes: [terravibe_sim::item::Prefix; 5],
}

impl Default for ClientVanity {
    fn default() -> Self {
        Self {
            armor: [None; 3],
            accessories: [None; 5],
            accessory_prefixes: [terravibe_sim::item::Prefix(0); 5],
        }
    }
}

/// The player's progression milestones, updated on `ServerMsg::SyncFacts`.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq, Deref, DerefMut)]
pub struct LocalCharacterFacts(pub terravibe_sim::facts::CharacterFacts);

/// Client resource representing character progression facts.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CharacterFacts {
    pub onboarding_dismissed: bool,
    pub advice_seen: u32,
    pub first_night_survived: bool,
    pub has_shelter: bool,
    pub has_ore: bool,
}

impl From<terravibe_sim::facts::CharacterFacts> for CharacterFacts {
    fn from(f: terravibe_sim::facts::CharacterFacts) -> Self {
        Self {
            onboarding_dismissed: f.onboarding_dismissed,
            advice_seen: f.advice_seen,
            first_night_survived: f.first_night_survived,
            has_shelter: f.has_shelter,
            has_ore: f.has_ore,
        }
    }
}

impl From<CharacterFacts> for terravibe_sim::facts::CharacterFacts {
    fn from(f: CharacterFacts) -> Self {
        Self {
            onboarding_dismissed: f.onboarding_dismissed,
            advice_seen: f.advice_seen,
            first_night_survived: f.first_night_survived,
            has_shelter: f.has_shelter,
            has_ore: f.has_ore,
        }
    }
}

/// The world's progression milestones, updated on `ServerMsg::SyncFacts`.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalWorldFacts(pub terravibe_sim::facts::WorldFacts);

/// Server world-clock tick, updated on `ServerMsg::Time`.
#[derive(Resource, Default)]
pub struct ClientClock {
    pub tick: u32,
    pub override_tick: Option<u32>,
}

impl ClientClock {
    pub fn effective_tick(&self) -> u32 {
        self.override_tick.unwrap_or(self.tick)
    }
}

/// Progress of the world handshake and initial chunk download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadingStage {
    /// Waiting for the client to send `Hello`.
    Connecting,
    /// Hello sent; waiting for the server's `Welcome` and spawn position.
    Generating,
    /// Spawn known; streaming the 3x3 columns around it.
    ReceivingChunks { received: u32, total: u32 },
    /// All surrounding chunks are loaded.
    Ready,
}

/// Connection and prediction state.
#[derive(Resource, Default)]
pub struct NetState {
    hello_sent: bool,
    spectate_in: f32,
    /// Last sequence number sent.
    pub(crate) seq: u32,
    /// Inputs sent but not yet acked, with their sequence numbers.
    pub(crate) pending: VecDeque<(u32, PlayerInput)>,
    /// Spawn from `Welcome`; prediction starts once its surroundings are loaded.
    spawn: Option<[f32; 3]>,
    pub bounds: WorldBounds,
    pub(crate) ready: bool,
    /// Columns around the spawn that are fully loaded.
    chunks_received: u32,
    /// Columns around the spawn that the world bounds require.
    chunks_total: u32,
    disconnected: bool,
}

impl NetState {
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Current loading stage, as shown by the loading card.
    pub fn loading_stage(&self) -> LoadingStage {
        if self.ready {
            return LoadingStage::Ready;
        }
        if self.spawn.is_none() {
            return if self.hello_sent {
                LoadingStage::Generating
            } else {
                LoadingStage::Connecting
            };
        }
        LoadingStage::ReceivingChunks {
            received: self.chunks_received,
            total: self.chunks_total,
        }
    }

    #[allow(dead_code)]
    pub fn is_disconnected(&self) -> bool {
        self.disconnected
    }

    /// True once the chunks around the spawn have arrived, so prediction and
    /// the server see the same world.
    pub(crate) fn check_ready(&mut self, stream: &ChunkStream) -> bool {
        if self.ready {
            return true;
        }
        let Some(s) = self.spawn else { return false };
        let c = ChunkPos::of_block(s[0].floor() as i32, 0, s[2].floor() as i32);
        let g = stream.generated();
        let mut received = 0;
        let mut total = 0;
        for x in c.x - 1..=c.x + 1 {
            for z in c.z - 1..=c.z + 1 {
                if self.bounds.chunk_inside(ChunkPos { x, y: 0, z }) {
                    total += 1;
                    if terravibe_sim::net::vertical_layers_for_y(s[1])
                        .all(|y| g.contains_key(&ChunkPos { x, y, z }))
                    {
                        received += 1;
                    }
                }
            }
        }
        self.chunks_received = received;
        self.chunks_total = total;
        self.ready = received == total;
        self.ready
    }
}

#[cfg(test)]
impl NetState {
    pub(crate) fn mark_hello_sent(&mut self) {
        self.hello_sent = true;
    }

    pub(crate) fn set_spawn(&mut self, spawn: [f32; 3]) {
        self.spawn = Some(spawn);
        self.chunks_received = 0;
        self.chunks_total = 9;
    }

    pub(crate) fn mark_ready(&mut self) {
        self.ready = true;
    }
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<CraftResults>()
            .add_message::<ChestMsg>()
            .add_message::<crate::hud::SaveEvent>()
            .add_message::<EnemyMsg>()
            .add_message::<DropMsg>()
            .add_message::<crate::view::projectiles::ProjectilesMsg>()
            .add_message::<crate::view::stars::StarsMsg>()
            .add_message::<crate::hud::town::TownMsg>()
            .add_message::<crate::hud::combat_feedback::CombatHitEvent>()
            .add_message::<PlayerEvents>()
            .add_message::<crate::view::interact::UseEvents>()
            .init_resource::<crate::hud::boss_bar::BossState>()
            .init_resource::<LocalCharacterFacts>()
            .init_resource::<CharacterFacts>()
            .init_resource::<LocalGuideSlot>()
            .init_resource::<ClientVanity>()
            .init_resource::<ClientGodMode>()
            .add_systems(OnExit(AppState::InGame), reset_god_mode)
            .add_systems(
                Update,
                (
                    pump.before(stream_chunks)
                        .run_if(in_state(AppState::InGame))
                        // A failed session start never inserts NetState; skip instead
                        // of panicking in param validation every frame.
                        .run_if(resource_exists::<NetState>),
                    handle_quick_potion_keys.run_if(in_state(AppState::InGame)),
                ),
            );
    }
}

/// Replays the inputs the server has not acked on top of its state.
/// Drops pending inputs with `seq <= ack` and returns the predicted body.
pub fn reconcile_bounded(
    server_body: &PlayerBody,
    ack: u32,
    pending: &mut VecDeque<(u32, PlayerInput)>,
    q: &impl BlockQuery,
    bounds: Option<&WorldBounds>,
) -> PlayerBody {
    while pending.front().is_some_and(|(s, _)| *s <= ack) {
        pending.pop_front();
    }
    let mut body = server_body.clone();
    for (_, input) in pending.iter() {
        step_body_bounded(q, &mut body, input, bounds);
    }
    body
}

/// Session-related inputs to pump.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct SessionIn<'w> {
    selected_world: Option<Res<'w, SelectedWorld>>,
    selected_char: Option<Res<'w, crate::session::SelectedCharacter>>,
    automation: Option<Res<'w, Automation>>,
    session_err: Option<ResMut<'w, SessionError>>,
}

/// Message writers for world-state updates, bundled to stay under Bevy's parameter limit.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct WorldOut<'w> {
    enemies: MessageWriter<'w, EnemyMsg>,
    drops: MessageWriter<'w, DropMsg>,
    crafts: MessageWriter<'w, CraftResults>,
    chest: MessageWriter<'w, ChestMsg>,
    projectiles: MessageWriter<'w, crate::view::projectiles::ProjectilesMsg>,
    stars: MessageWriter<'w, crate::view::stars::StarsMsg>,
    boss: Option<ResMut<'w, crate::hud::boss_bar::BossState>>,
    wof_boss: Option<ResMut<'w, crate::hud::boss_bar::WofBossState>>,
    town: MessageWriter<'w, crate::hud::town::TownMsg>,
    death_modal: Option<ResMut<'w, crate::hud::death::DeathModalState>>,
    victory_modal: Option<ResMut<'w, crate::hud::victory::VictoryModalState>>,
    saves: MessageWriter<'w, crate::hud::SaveEvent>,
    pickup_toasts: Option<ResMut<'w, crate::hud::pickup_toast::PickupToastQueue>>,
    combat_hits: MessageWriter<'w, crate::hud::combat_feedback::CombatHitEvent>,
    sfx: Option<ResMut<'w, Messages<crate::audio::SfxEvent>>>,
    weather: Option<ResMut<'w, crate::view::weather::WeatherFxState>>,
    god_mode: Option<ResMut<'w, ClientGodMode>>,
    local_char_facts: Option<Res<'w, LocalCharacterFacts>>,
    char_facts: Option<Res<'w, CharacterFacts>>,
}

fn reset_god_mode(mut god_mode: ResMut<ClientGodMode>) {
    god_mode.0 = false;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn pump(
    mut commands: Commands,
    net: Option<Res<Net>>,
    session: SessionIn,
    mut st: ResMut<NetState>,
    mode: Option<Res<ControlMode>>,
    time: Option<Res<Time<Real>>>,
    mut stream: Option<ResMut<ChunkStream>>,
    mut player: Option<ResMut<LocalPlayer>>,
    mut view: Option<ResMut<PlayerView>>,
    mut sampled: Option<ResMut<SampledInput>>,
    mut cam: Option<Single<(&mut FlyCamera, &mut Transform)>>,
    mut events: MessageWriter<PlayerEvents>,
    mut uses: MessageWriter<crate::view::interact::UseEvents>,
    mut exit: MessageWriter<AppExit>,
    mut clock: Option<ResMut<ClientClock>>,
    mut out: WorldOut,
) {
    let Some(net) = net else { return };
    let WorldOut {
        enemies,
        drops,
        crafts,
        chest,
        projectiles,
        stars,
        boss,
        wof_boss,
        town,
        death_modal,
        victory_modal,
        saves,
        pickup_toasts,
        combat_hits,
        sfx,
        weather,
        god_mode,
        local_char_facts,
        char_facts,
    } = &mut out;
    if st.disconnected {
        return;
    }
    let seed = session.selected_world.as_ref().map_or(SEED, |w| w.seed);
    let control_mode = mode.as_ref().map_or(ControlMode::Player, |m| **m);
    let delta_secs = time.as_ref().map_or(1.0 / 60.0, |t| t.delta_secs());
    let mut gone = false;
    match control_mode {
        ControlMode::Player if !st.hello_sent => {
            st.hello_sent = true;
            let (char_name, char_bytes) = session
                .selected_char
                .as_ref()
                .and_then(|sc| sc.0.as_ref())
                .map(|c| (Some(c.name.clone()), Some(c.to_bytes())))
                .unwrap_or((None, None));
            gone |= net
                .send(ClientMsg::Hello {
                    seed,
                    name: char_name,
                    character: char_bytes,
                })
                .is_err();
        }
        ControlMode::Fly => {
            // The tunnel demo flies a fixed camera but still joins, so the HUD
            // and the player-bound demo setup get their server state.
            if !st.hello_sent && crate::view::interact::demo_is_tunnel() {
                st.hello_sent = true;
                let (char_name, char_bytes) = session
                    .selected_char
                    .as_ref()
                    .and_then(|sc| sc.0.as_ref())
                    .map(|c| (Some(c.name.clone()), Some(c.to_bytes())))
                    .unwrap_or((None, None));
                gone |= net
                    .send(ClientMsg::Hello {
                        seed,
                        name: char_name,
                        character: char_bytes,
                    })
                    .is_err();
            }
            st.spectate_in -= delta_secs;
            if st.spectate_in <= 0.0 {
                st.spectate_in = 1.0 / SPECTATE_HZ;
                if let Some(cam) = cam.as_ref() {
                    let p = cam.1.translation;
                    gone |= net.send(ClientMsg::Spectate { pos: p.into() }).is_err();
                }
            }
        }
        ControlMode::Player => {}
    }

    let mut pending_player: Option<Player> = None;
    loop {
        let msg = match net.try_recv() {
            Ok(Some(m)) => m,
            Ok(None) => break,
            Err(Disconnected) => {
                gone = true;
                break;
            }
        };
        match msg {
            ServerMsg::Welcome { spawn, size, .. } => {
                let bounds = WorldBounds::new(size);
                st.spawn = Some(spawn);
                st.bounds = bounds;
                st.ready = false;
                st.chunks_received = 0;
                st.chunks_total = 9;
                st.seq = 0;
                st.pending.clear();
                if let Some(stream) = stream.as_mut() {
                    stream.set_bounds(bounds);
                }
                if let Some(sampled) = sampled.as_mut() {
                    **sampled = SampledInput::default();
                }
                let p = Player::new(spawn);
                let eye = eye_of(&p);
                commands.insert_resource(PlayerView {
                    prev_eye: eye,
                    eye,
                    offset: Vec3::ZERO,
                });
                pending_player = Some(p);
                if let Some(cam) = cam.as_mut() {
                    let (c, tf) = &mut **cam;
                    c.pitch = 0.0; // level view at eye height
                    tf.rotation = Quat::from_euler(EulerRot::YXZ, c.yaw, 0.0, 0.0);
                }
            }
            ServerMsg::Chunk { pos, chunk } => {
                if let Some(stream) = stream.as_mut() {
                    stream.insert_chunk(pos, chunk);
                    st.check_ready(stream);
                }
            }
            ServerMsg::Unload { pos } => {
                if let Some(stream) = stream.as_mut() {
                    stream.remove_chunk(pos);
                }
            }
            ServerMsg::PlayerState {
                ack,
                body,
                vitals,
                dead,
                respawn_ticks,
            } => {
                let (Some(player), Some(view)) = (player.as_mut(), view.as_mut()) else {
                    continue;
                };
                let old_eye = eye_of(&player.0);
                let empty_generated = std::collections::HashMap::new();
                let q = LoadedBlocks(stream.as_ref().map_or(&empty_generated, |s| s.generated()));
                // A dead body is frozen on the server: do not replay on top of it.
                let body = if dead {
                    st.pending.clear();
                    body
                } else {
                    let bounds = st.bounds;
                    reconcile_bounded(&body, ack, &mut st.pending, &q, Some(&bounds))
                };
                player.0.body = body;
                player.0.vitals = vitals;
                player.0.dead = dead;
                player.0.respawn_ticks = respawn_ticks;
                if let Some(dm) = death_modal.as_mut() {
                    dm.respawn_ticks = respawn_ticks;
                    if !dead && dm.active {
                        dm.active = false;
                    }
                }
                let delta = eye_of(&player.0) - old_eye;
                view.prev_eye += delta;
                view.eye += delta;
                let dist = delta.length();
                if dist > SNAP_DIST {
                    view.prev_eye = view.eye;
                    view.offset = Vec3::ZERO;
                } else if dist > SMOOTH_MIN {
                    view.offset -= delta;
                }
            }
            ServerMsg::Event(e) => {
                if matches!(e, terravibe_sim::player::PlayerEvent::Respawned) {
                    if let Some(dm) = death_modal.as_mut() {
                        dm.active = false;
                    }
                }
                // ServerMsg::PlayerDied is the unified authority for player death events.
                if !matches!(e, terravibe_sim::player::PlayerEvent::Died { .. }) {
                    events.write(PlayerEvents(e));
                }
            }
            ServerMsg::BlockChanged { pos, block } => {
                if let Some(stream) = stream.as_mut() {
                    stream.set_block(pos, block);
                }
            }
            ServerMsg::WallChanged { pos, on } => {
                if let Some(stream) = stream.as_mut() {
                    stream.set_wall(pos, on);
                }
            }
            ServerMsg::LiquidDelta { pos, cells } => {
                if let Some(stream) = stream.as_mut() {
                    stream.apply_liquid_delta(pos, &cells);
                }
            }
            // Consumed by Stage 6 UI (task 4); no client display yet.
            ServerMsg::HouseStatus { ok, reason } => {
                town.write(crate::hud::town::TownMsg::House { ok, reason });
            }
            // Homeless state is not displayed yet.
            ServerMsg::TownNpcInfo { .. } => {}
            ServerMsg::Dialogue { npc, text } => {
                town.write(crate::hud::town::TownMsg::Dialogue { npc, text });
            }
            ServerMsg::NpcSpeech { id, text } => {
                town.write(crate::hud::town::TownMsg::Speech { id, text });
            }
            ServerMsg::ShopContents { items } => {
                town.write(crate::hud::town::TownMsg::Shop { items });
            }
            ServerMsg::ShopResult { ok, cannot_afford } => {
                town.write(crate::hud::town::TownMsg::Result { ok, cannot_afford });
            }
            ServerMsg::NurseHealed {
                ok,
                healed,
                cost_copper,
            } => {
                town.write(crate::hud::town::TownMsg::NurseHealed {
                    ok,
                    healed,
                    cost_copper,
                });
            }
            ServerMsg::ReforgeResult {
                ok,
                slot,
                new_prefix,
                cost_copper,
            } => {
                town.write(crate::hud::town::TownMsg::ReforgeResult {
                    ok,
                    slot,
                    new_prefix,
                    cost_copper,
                });
            }
            ServerMsg::Inventory(inv) => commands.insert_resource(LocalInventory(inv)),
            ServerMsg::Armor { pieces } => {
                commands.insert_resource(LocalArmor(pieces));
                if let Some(ref mut p) = pending_player {
                    p.armor = pieces;
                    p.sync_accessories();
                } else if let Some(ref mut lp) = player {
                    lp.0.armor = pieces;
                    lp.0.sync_accessories();
                }
            }
            ServerMsg::Accessories { pieces, prefixes } => {
                commands.insert_resource(LocalAccessories(pieces));
                commands.insert_resource(LocalAccessoryPrefixes(prefixes));
                if let Some(ref mut p) = pending_player {
                    p.accessories = pieces;
                    p.accessory_prefixes = prefixes;
                    p.sync_accessories();
                } else if let Some(ref mut lp) = player {
                    lp.0.accessories = pieces;
                    lp.0.accessory_prefixes = prefixes;
                    lp.0.sync_accessories();
                }
            }
            ServerMsg::Held(h) => commands.insert_resource(LocalHeld(h)),
            ServerMsg::Use(e) => {
                uses.write(crate::view::interact::UseEvents(e));
            }
            ServerMsg::Time { tick } => {
                if let Some(clock) = clock.as_mut() {
                    clock.tick = tick;
                }
            }
            ServerMsg::WeatherState { kind, .. } => {
                if let Some(w) = weather.as_mut() {
                    w.kind = kind;
                }
            }
            ServerMsg::ChestContents { pos, slots } => {
                chest.write(ChestMsg::Contents { pos, slots });
            }
            ServerMsg::Projectiles { list } => {
                projectiles.write(crate::view::projectiles::ProjectilesMsg(list));
            }
            ServerMsg::Stars { list } => {
                stars.write(crate::view::stars::StarsMsg(list));
            }
            ServerMsg::Boss {
                pos,
                vel,
                life,
                life_max,
                phase,
                state,
                spin,
            } => {
                if let Some(boss) = boss.as_mut() {
                    let spawned = boss.0.is_none();
                    boss.set(crate::hud::boss_bar::BossInfo {
                        pos,
                        vel,
                        life,
                        life_max,
                        phase,
                        state,
                        spin,
                    });
                    if spawned && let Some(bus) = sfx.as_mut() {
                        bus.write(crate::audio::SfxEvent(crate::audio::SfxKind::BossRoar));
                    }
                }
            }
            ServerMsg::BossGone => {
                if let Some(boss) = boss.as_mut() {
                    boss.clear();
                }
            }
            ServerMsg::WofState {
                wall_x,
                dir,
                life,
                life_max,
            } => {
                if let Some(wof) = wof_boss.as_mut() {
                    let spawned = wof.0.is_none();
                    wof.set(crate::hud::boss_bar::WofBossInfo {
                        wall_x,
                        dir,
                        life,
                        life_max,
                    });
                    if spawned && let Some(bus) = sfx.as_mut() {
                        bus.write(crate::audio::SfxEvent(crate::audio::SfxKind::BossRoar));
                    }
                }
            }
            ServerMsg::WofGone => {
                if let Some(wof) = wof_boss.as_mut() {
                    wof.clear();
                }
            }
            ServerMsg::WallOfFleshDefeated => {
                if let Some(vm) = victory_modal.as_mut() {
                    vm.active = true;
                    vm.fade_alpha = 0.0;
                }
            }
            ServerMsg::ChestClosed => {
                chest.write(ChestMsg::Closed);
            }
            ServerMsg::PlayerHit { amount } => {
                events.write(PlayerEvents(terravibe_sim::player::PlayerEvent::Hurt {
                    amount,
                }));
            }
            ServerMsg::PlayerDied {
                cause,
                coins_lost,
                survived_ticks,
            } => {
                events.write(PlayerEvents(terravibe_sim::player::PlayerEvent::Died {
                    cause,
                }));
                if let Some(dm) = death_modal.as_mut() {
                    dm.active = true;
                    dm.cause = cause;
                    dm.coins_lost = coins_lost;
                    dm.survived_ticks = survived_ticks;
                    dm.respawn_ticks = terravibe_sim::player::vitals::RESPAWN_TICKS;
                    dm.fade_alpha = 0.0;
                }
            }
            ServerMsg::EnemySpawn { id, kind, pos } => {
                enemies.write(EnemyMsg::Spawned {
                    id,
                    kind,
                    pos: Vec3::from(pos),
                });
            }
            ServerMsg::EnemyState {
                id, pos, vel, hp, ..
            } => {
                enemies.write(EnemyMsg::Moved {
                    id,
                    pos: Vec3::from(pos),
                    vel: Vec3::from(vel),
                    hp,
                });
            }
            ServerMsg::EnemyDespawn { id } => {
                enemies.write(EnemyMsg::Despawned { id });
            }
            ServerMsg::DropSpawn { id, stack, pos } => {
                drops.write(DropMsg::Spawned {
                    id,
                    stack,
                    pos: Vec3::from(pos),
                });
            }
            ServerMsg::DropDespawn { id } => {
                drops.write(DropMsg::Despawned { id });
            }
            ServerMsg::DropPickup { id, item, count } => {
                drops.write(DropMsg::Despawned { id });
                if let Some(q) = pickup_toasts.as_mut() {
                    let now = time.as_ref().map_or(0.0, |t| t.elapsed_secs_f64());
                    q.push(item, count, now);
                }
            }
            ServerMsg::CraftResult {
                recipe,
                crafted,
                error,
            } => {
                crafts.write(CraftResults {
                    recipe,
                    crafted,
                    error,
                });
            }
            ServerMsg::CharacterState(bytes) => {
                if let Ok(c) = terravibe_sim::Character::from_bytes(&bytes) {
                    let root = terravibe_server::characters::characters_dir();
                    let _ = terravibe_server::characters::save_character(&root, &c);
                }
                saves.write(crate::hud::SaveEvent::WorldSaved);
            }
            ServerMsg::SyncFacts {
                world,
                mut character,
            } => {
                let already_dismissed = local_char_facts
                    .as_ref()
                    .is_some_and(|lcf| lcf.0.onboarding_dismissed)
                    || char_facts
                        .as_ref()
                        .is_some_and(|cf| cf.onboarding_dismissed);
                if already_dismissed {
                    character.onboarding_dismissed = true;
                }
                let advice = terravibe_sim::facts::current_guide_advice(&world, &character);
                commands.insert_resource(crate::menu::game::GuideAdviceState {
                    current_advice: advice.to_string(),
                });
                commands.insert_resource(LocalCharacterFacts(character));
                commands.insert_resource(CharacterFacts::from(character));
                commands.insert_resource(LocalWorldFacts(world));
            }
            ServerMsg::CombatHit {
                pos,
                attacker_pos,
                amount,
                crit,
                damage_kind,
                target_kind,
                killed,
            } => {
                combat_hits.write(crate::hud::combat_feedback::CombatHitEvent {
                    pos: Vec3::from(pos),
                    attacker_pos: attacker_pos.map(Vec3::from),
                    amount,
                    crit,
                    damage_kind,
                    target_kind,
                    killed,
                });
            }
            ServerMsg::EnemyTelegraph { id, kind } => {
                enemies.write(EnemyMsg::Telegraph { id, kind });
                if let Some(bus) = sfx.as_mut() {
                    bus.write(crate::audio::SfxEvent(crate::audio::SfxKind::Telegraph));
                }
            }
            ServerMsg::Vanity {
                armor,
                accessories,
                accessory_prefixes,
            } => {
                commands.insert_resource(ClientVanity {
                    armor,
                    accessories,
                    accessory_prefixes,
                });
                if let Some(ref mut p) = pending_player {
                    p.vanity_armor = armor;
                    p.vanity_accessories = accessories;
                } else if let Some(ref mut lp) = player {
                    lp.0.vanity_armor = armor;
                    lp.0.vanity_accessories = accessories;
                }
            }
            ServerMsg::GodMode(g) => {
                if let Some(gm) = god_mode.as_deref_mut() {
                    gm.0 = g;
                }
            }
            ServerMsg::GuideSlot(g) => {
                commands.insert_resource(LocalGuideSlot(g));
            }
        }
    }

    if let Some(p) = pending_player {
        commands.insert_resource(LocalPlayer(p));
    }
    if gone {
        if let Some(gm) = god_mode.as_deref_mut() {
            gm.0 = false;
        }
        st.disconnected = true;
        let reason = "Connection to server lost (server disconnected).".to_string();
        error!("{reason}");
        if let Some(mut err) = session.session_err {
            err.0 = Some(reason.clone());
        }
        commands.insert_resource(DisconnectState { reason });
        if session.automation.is_some() {
            exit.write(AppExit::error());
        }
    }
}

/// Sends `QuickHeal` (`H`), `QuickMana` (`J`), or `QuickBuff` (`B`) while in-game.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_quick_potion_keys(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    settings: Option<Res<crate::settings::Settings>>,
    player: Option<Res<LocalPlayer>>,
    inv_open: Option<Res<crate::hud::inventory::InventoryOpen>>,
    menu: Option<Res<crate::session::GameMenu>>,
    creative: Option<Res<crate::menu::creative::CreativeMenuOpen>>,
    victory: Option<Res<crate::hud::victory::VictoryModalState>>,
    net: Option<Res<Net>>,
) {
    let (Some(keys), Some(net)) = (keys, net) else {
        return;
    };
    if player.as_ref().is_some_and(|p| p.0.dead)
        || inv_open.as_ref().is_some_and(|o| o.0)
        || menu.as_ref().is_some_and(|m| m.panel.is_some())
        || creative.as_ref().is_some_and(|c| c.0)
        || victory.as_ref().is_some_and(|v| v.active)
    {
        return;
    }
    let bindings = settings.map(|s| s.bindings).unwrap_or_default();
    if keys.just_pressed(bindings.key(crate::input_map::GameAction::QuickHeal)) {
        let _ = net.send(ClientMsg::QuickHeal);
    }
    if keys.just_pressed(bindings.key(crate::input_map::GameAction::QuickMana)) {
        let _ = net.send(ClientMsg::QuickMana);
    }
    if keys.just_pressed(bindings.key(crate::input_map::GameAction::QuickBuff)) {
        let _ = net.send(ClientMsg::QuickBuff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use terravibe_sim::{AIR, BlockId, BlockQuery, DIRT, PlayerBody, PlayerInput, step_body};

    struct Floor;
    impl BlockQuery for Floor {
        fn block(&self, _x: i32, y: i32, _z: i32) -> BlockId {
            if y < 0 { DIRT } else { AIR }
        }
    }

    fn inputs() -> VecDeque<(u32, PlayerInput)> {
        (1..=10)
            .map(|s| {
                let i = PlayerInput {
                    forward: 1.0,
                    ..Default::default()
                };
                (s, i)
            })
            .collect()
    }

    fn after(n: usize) -> PlayerBody {
        let mut b = PlayerBody::new([0.5, 0.0, 0.5]);
        for (_, i) in inputs().iter().take(n) {
            step_body(&Floor, &mut b, i);
        }
        b
    }

    #[test]
    fn reconcile_replays_unacked() {
        let mut pending = inputs();
        let got = reconcile_bounded(&after(4), 4, &mut pending, &Floor, None);
        assert_eq!(got, after(10));
        assert_eq!(pending.len(), 6);
    }

    #[test]
    fn reconcile_all_acked_returns_server() {
        let mut pending = inputs();
        let server = after(10);
        let got = reconcile_bounded(&server, 10, &mut pending, &Floor, None);
        assert_eq!(got, server);
        assert!(pending.is_empty());
    }

    #[test]
    fn sync_facts_updates_guide_advice_and_local_facts() {
        let (_tx_c, _rx_c) = std::sync::mpsc::channel::<ClientMsg>();
        let (to_client, from_server) = std::sync::mpsc::channel::<ServerMsg>();
        struct MockTransport(std::sync::Mutex<std::sync::mpsc::Receiver<ServerMsg>>);
        impl terravibe_sim::net::ClientTransport for MockTransport {
            fn send(&self, _m: ClientMsg) -> Result<(), terravibe_sim::net::Disconnected> {
                Ok(())
            }
            fn try_recv(&self) -> Result<Option<ServerMsg>, terravibe_sim::net::Disconnected> {
                Ok(self.0.lock().unwrap().try_recv().ok())
            }
        }
        let transport = MockTransport(std::sync::Mutex::new(from_server));
        to_client
            .send(ServerMsg::SyncFacts {
                world: terravibe_sim::facts::WorldFacts {
                    merchant_housed: true,
                    downed_eye: true,
                    ..default()
                },
                character: terravibe_sim::facts::CharacterFacts {
                    first_night_survived: true,
                    has_shelter: true,
                    has_ore: true,
                    ..default()
                },
            })
            .unwrap();

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(Net::new(transport));
        app.insert_resource(NetState {
            hello_sent: true,
            ..default()
        });
        app.init_resource::<Messages<PlayerEvents>>();
        app.init_resource::<Messages<crate::view::interact::UseEvents>>();
        app.init_resource::<Messages<AppExit>>();
        app.init_resource::<Messages<crate::view::enemies::EnemyMsg>>();
        app.init_resource::<Messages<crate::view::drops::DropMsg>>();
        app.init_resource::<Messages<CraftResults>>();
        app.init_resource::<Messages<ChestMsg>>();
        app.init_resource::<Messages<crate::view::projectiles::ProjectilesMsg>>();
        app.init_resource::<Messages<crate::view::stars::StarsMsg>>();
        app.init_resource::<Messages<crate::hud::town::TownMsg>>();
        app.init_resource::<Messages<crate::hud::SaveEvent>>();
        app.init_resource::<Messages<crate::hud::combat_feedback::CombatHitEvent>>();
        app.init_resource::<crate::hud::death::DeathModalState>();
        app.init_resource::<crate::hud::boss_bar::BossState>();
        app.init_resource::<crate::menu::game::GuideAdviceState>();
        app.add_systems(Update, pump);
        app.update();

        let advice = app
            .world()
            .resource::<crate::menu::game::GuideAdviceState>();
        assert!(advice.current_advice.contains("Corruption"));
        let char_facts = app.world().resource::<LocalCharacterFacts>();
        assert!(char_facts.0.first_night_survived);
        assert!(char_facts.0.has_shelter);
        assert!(char_facts.0.has_ore);
        let world_facts = app.world().resource::<LocalWorldFacts>();
        assert!(world_facts.0.downed_eye);
    }

    #[test]
    fn quick_potion_keys_send_heal_mana_buff_when_in_game() {
        #[derive(Clone)]
        struct SendRecorder(std::sync::Arc<Mutex<Vec<ClientMsg>>>);
        impl ClientTransport for SendRecorder {
            fn send(&self, m: ClientMsg) -> Result<(), Disconnected> {
                self.0.lock().unwrap().push(m);
                Ok(())
            }
            fn try_recv(&self) -> Result<Option<ServerMsg>, Disconnected> {
                Ok(None)
            }
        }

        let sent = std::sync::Arc::new(Mutex::new(Vec::new()));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(crate::settings::Settings::default())
            .insert_resource(Net::new(SendRecorder(sent.clone())))
            .add_systems(Update, handle_quick_potion_keys);

        for key in [KeyCode::KeyH, KeyCode::KeyJ, KeyCode::KeyB] {
            let mut k = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            k.clear();
            k.press(key);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(key);
        }

        let msgs = sent.lock().unwrap();
        assert_eq!(msgs.len(), 3);
        assert!(matches!(msgs[0], ClientMsg::QuickHeal));
        assert!(matches!(msgs[1], ClientMsg::QuickMana));
        assert!(matches!(msgs[2], ClientMsg::QuickBuff));
    }

    #[test]
    fn god_mode_updates_and_resets_on_exit_or_disconnect() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_resource::<ClientGodMode>()
            .add_systems(OnExit(AppState::InGame), reset_god_mode);

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        app.world_mut().resource_mut::<ClientGodMode>().0 = true;
        assert!(app.world().resource::<ClientGodMode>().0);

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Menu);
        app.update();
        assert!(!app.world().resource::<ClientGodMode>().0);
    }

    #[test]
    fn stale_sync_facts_never_reopens_dismissed_onboarding() {
        let (to_client, from_server) = std::sync::mpsc::channel();
        struct MockTransport(std::sync::Mutex<std::sync::mpsc::Receiver<ServerMsg>>);
        impl terravibe_sim::net::ClientTransport for MockTransport {
            fn send(&self, _m: ClientMsg) -> Result<(), terravibe_sim::net::Disconnected> {
                Ok(())
            }
            fn try_recv(&self) -> Result<Option<ServerMsg>, terravibe_sim::net::Disconnected> {
                Ok(self.0.lock().unwrap().try_recv().ok())
            }
        }
        to_client
            .send(ServerMsg::SyncFacts {
                world: terravibe_sim::facts::WorldFacts::default(),
                character: terravibe_sim::facts::CharacterFacts {
                    onboarding_dismissed: false,
                    has_ore: true,
                    ..default()
                },
            })
            .unwrap();

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(Net::new(MockTransport(std::sync::Mutex::new(from_server))));
        app.insert_resource(NetState {
            hello_sent: true,
            ..default()
        });
        app.insert_resource(CharacterFacts {
            onboarding_dismissed: true,
            ..default()
        });
        app.insert_resource(LocalCharacterFacts(terravibe_sim::facts::CharacterFacts {
            onboarding_dismissed: true,
            ..default()
        }));
        app.init_resource::<Messages<PlayerEvents>>();
        app.init_resource::<Messages<crate::view::interact::UseEvents>>();
        app.init_resource::<Messages<AppExit>>();
        app.init_resource::<Messages<crate::view::enemies::EnemyMsg>>();
        app.init_resource::<Messages<crate::view::drops::DropMsg>>();
        app.init_resource::<Messages<CraftResults>>();
        app.init_resource::<Messages<ChestMsg>>();
        app.init_resource::<Messages<crate::view::projectiles::ProjectilesMsg>>();
        app.init_resource::<Messages<crate::view::stars::StarsMsg>>();
        app.init_resource::<Messages<crate::hud::town::TownMsg>>();
        app.init_resource::<Messages<crate::hud::SaveEvent>>();
        app.init_resource::<Messages<crate::hud::combat_feedback::CombatHitEvent>>();
        app.init_resource::<crate::hud::death::DeathModalState>();
        app.init_resource::<crate::hud::boss_bar::BossState>();
        app.init_resource::<crate::menu::game::GuideAdviceState>();
        app.add_systems(Update, pump);
        app.update();

        assert!(
            app.world()
                .resource::<CharacterFacts>()
                .onboarding_dismissed
        );
        assert!(
            app.world()
                .resource::<LocalCharacterFacts>()
                .0
                .onboarding_dismissed
        );
        assert!(app.world().resource::<LocalCharacterFacts>().0.has_ore);
    }
}
