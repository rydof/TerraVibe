//! Authoritative in-process server: owns generation, chunks and players.

pub mod characters;
mod r#gen;
pub mod save;
mod stream;
pub mod worlds;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use terravibe_sim::boss::{self, BossEvent, EyeBoss};
use terravibe_sim::chest::ChestInv;
use terravibe_sim::combat::{
    ON_FIRE_TICKS, apply_hit, damage_taken, hurt_player_with_cause, melee_reach,
    swing_hits_with_reach,
};
use terravibe_sim::crafting::{CraftError, RecipeId, craft_with_rng, recipe, station_in_reach};
use terravibe_sim::drop::{
    Drop, DropId, heart_drop, loot_for, tick_drop, try_pickup, try_pickup_heart,
};
use terravibe_sim::enemy::{Enemy, EnemyId, EnemyKind, tick_enemy};
use terravibe_sim::interact::{
    EYE_HEIGHT, EditQuery, LoadedQuery, UseEvent, UseState, apply_consumable, tick_use_with_mana,
};
use terravibe_sim::inventory::{Inventory, ItemStack};
use terravibe_sim::item::{
    BIRD_ITEM, BUG_NET, BUNNY_ITEM, CAVE_CRITTER_ITEM, ENCHANTED_BOOMERANG, FALLEN_STAR,
    FIERY_GREATSWORD, FLAMARANG, HEART, ItemClass, ItemId, Prefix, Rng, SPEAR, SQUIRREL_ITEM,
    SUSPICIOUS_LOOKING_EYE, item_data, item_exists, prefix_stats, roll_prefix,
};
use terravibe_sim::liquid::{LiquidSim, LiquidWorld, MAX_CELL_UPDATES_PER_TICK};

pub const LIQUID_DELTA_MAX: usize = 512;
use terravibe_sim::net::{
    ClientMsg, CreativeCmd, DamageKind, Disconnected, HitTargetKind, InProcClient,
    PROTOCOL_VERSION, PlayerId, RejectReason, ServerConn, ServerMsg, VIEW_RADIUS_BLOCKS,
    in_process_pair,
};
use terravibe_sim::player::consts::{HALF_WIDTH, HEIGHT};
use terravibe_sim::player::{DeathCause, Player, PlayerEvent, PlayerInput, tick_player_bounded};
use terravibe_sim::projectile::{
    MAX_PROJECTILES, ProjKind, Projectile, Step, hit_enemies_detailed, step_with_target,
};
use terravibe_sim::slots::{self, Held};
use terravibe_sim::spawn::{SpawnWorld, Spawner, tick_spawner};
use terravibe_sim::star::{Star, StarStep, night_setup, spawn_star, step_star};
use terravibe_sim::time::WorldClock;
use terravibe_sim::town_npc::{self, House, TownKind, Wander};
use terravibe_sim::weather::WeatherState;
use terravibe_sim::{
    AIR, BlockId, BlockQuery, CHEST, CHUNK_SIZE, CharacterFacts, Chunk, ChunkPos, Liquid,
    SOLID_UNLOADED, WorldBounds, WorldFacts, WorldSize, generate_chunk,
};

use r#gen::Generator;
pub use save::{Autosaver, PlayerSave, SaveCfg, SaveDir, TownSave, WorldSnapshot};
use stream::{Interest, LAYERS};
pub use worlds::{NameError, WorldInfo, create_world, delete_world, list_worlds, validate_name};

/// Physics view of the server world: missing chunks are solid.
#[derive(Clone, Copy)]
struct WorldQ<'a>(&'a HashMap<ChunkPos, Arc<Chunk>>);

impl BlockQuery for WorldQ<'_> {
    fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
        match self.0.get(&ChunkPos::of_block(x, y, z)) {
            Some(c) => {
                let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
                c.get(l(x), l(y), l(z))
            }
            None => SOLID_UNLOADED,
        }
    }
    fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid> {
        let c = self.0.get(&ChunkPos::of_block(x, y, z))?;
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        c.liquid(l(x), l(y), l(z))
    }
}

impl LoadedQuery for WorldQ<'_> {
    fn is_loaded(&self, x: i32, y: i32, z: i32) -> bool {
        self.0.contains_key(&ChunkPos::of_block(x, y, z))
    }
    fn has_wall(&self, x: i32, y: i32, z: i32) -> bool {
        self.0.get(&ChunkPos::of_block(x, y, z)).is_some_and(|c| {
            let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
            c.wall(l(x), l(y), l(z))
        })
    }
}

impl WorldQ<'_> {
    /// Top non-air block, scanning downward from the top of the world until a solid/non-air block
    /// is found (returning `None` if the top chunk or an intermediate air chunk is unloaded).
    fn surface_y(&self, x: i32, z: i32) -> Option<i32> {
        for cy in LAYERS.clone().rev() {
            if !self.is_loaded(x, cy * CHUNK_SIZE, z) {
                return None;
            }
            let y_lo = cy * CHUNK_SIZE;
            let y_hi = y_lo + CHUNK_SIZE - 1;
            if let Some(y) = (y_lo..=y_hi).rev().find(|&y| self.block(x, y, z) != AIR) {
                return Some(y);
            }
        }
        None
    }

    fn has_clearance(&self, x: i32, y: i32, z: i32) -> bool {
        !self.block(x, y, z).is_solid()
            && !self.block(x, y + 1, z).is_solid()
            && self.liquid(x, y, z).is_none()
            && self.liquid(x, y + 1, z).is_none()
    }
}

/// Natural spawning needs the biome seed as well as the loaded-block view.
struct SpawnQ<'a> {
    world: WorldQ<'a>,
    seed: u32,
    size: WorldSize,
}

impl SpawnWorld for SpawnQ<'_> {
    fn surface_y(&self, x: i32, z: i32) -> Option<i32> {
        if let Some(sy) = self.world.surface_y(x, z) {
            return Some(sy);
        }
        if LAYERS
            .clone()
            .any(|cy| self.world.is_loaded(x, cy * CHUNK_SIZE, z))
        {
            Some(terravibe_sim::worldgen::terrain::surface_height_for_size(
                self.seed, x, z, self.size,
            ))
        } else {
            None
        }
    }

    fn corrupt_at(&self, x: i32, z: i32) -> bool {
        terravibe_sim::worldgen::corruption::corruption_at_for_size(self.seed, x, z, self.size)
            > 0.0
    }

    fn has_clearance(&self, x: i32, y: i32, z: i32) -> bool {
        self.world.has_clearance(x, y, z)
    }
}

/// Server world with an edit log, for `tick_use`. Edits copy the chunk on write.
struct EditQ<'a> {
    world: &'a mut HashMap<ChunkPos, Arc<Chunk>>,
    log: &'a mut Vec<([i32; 3], BlockId)>,
    chests: &'a mut HashMap<[i32; 3], ChestInv>,
    /// Successful wall edits (`pos`, `on`), broadcast as `WallChanged`.
    wall_log: &'a mut Vec<([i32; 3], bool)>,
    /// Successful liquid edits (`pos`), broadcast as `LiquidDelta` and woken in `liquid_sim`.
    liquid_log: &'a mut Vec<[i32; 3]>,
}

impl BlockQuery for EditQ<'_> {
    fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
        WorldQ(self.world).block(x, y, z)
    }
    fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid> {
        WorldQ(self.world).liquid(x, y, z)
    }
}

impl LoadedQuery for EditQ<'_> {
    fn is_loaded(&self, x: i32, y: i32, z: i32) -> bool {
        WorldQ(self.world).is_loaded(x, y, z)
    }
    fn has_wall(&self, x: i32, y: i32, z: i32) -> bool {
        WorldQ(self.world).has_wall(x, y, z)
    }
}

impl EditQuery for EditQ<'_> {
    fn try_set_block(&mut self, x: i32, y: i32, z: i32, b: BlockId) -> bool {
        let pos = [x, y, z];
        let old = WorldQ(self.world).block(x, y, z);
        // Chest.CanDestroyChest (gameplay spec): a chest holding items cannot be broken.
        if old == CHEST && b != CHEST && self.chests.get(&pos).is_some_and(|c| !c.is_empty()) {
            return false;
        }
        let Some(c) = self.world.get_mut(&ChunkPos::of_block(x, y, z)) else {
            return false;
        };
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        let ch = Arc::make_mut(c);
        ch.set(l(x), l(y), l(z), b);
        if b.is_solid() {
            ch.set_liquid(l(x), l(y), l(z), None);
        }
        if old == CHEST && b != CHEST {
            self.chests.remove(&pos);
        } else if b == CHEST && old != CHEST {
            self.chests.entry(pos).or_default();
        }
        self.log.push((pos, b));
        true
    }

    fn try_set_wall(&mut self, x: i32, y: i32, z: i32, on: bool) -> bool {
        let Some(c) = self.world.get_mut(&ChunkPos::of_block(x, y, z)) else {
            return false;
        };
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        Arc::make_mut(c).set_wall(l(x), l(y), l(z), on);
        self.wall_log.push(([x, y, z], on));
        true
    }

    fn set_liquid(&mut self, x: i32, y: i32, z: i32, l: Option<Liquid>) -> bool {
        let Some(c) = self.world.get_mut(&ChunkPos::of_block(x, y, z)) else {
            return false;
        };
        let l_fn = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        Arc::make_mut(c).set_liquid(l_fn(x), l_fn(y), l_fn(z), l);
        self.liquid_log.push([x, y, z]);
        true
    }
}

/// Adapter exposing the server's chunk map to `LiquidSim` with a 1-entry chunk cache.
struct ServerLiquidWorld<'a> {
    world: &'a mut HashMap<ChunkPos, Arc<Chunk>>,
    read_cp: std::cell::Cell<ChunkPos>,
    read_ptr: std::cell::Cell<*const Chunk>,
    write_cp: ChunkPos,
    write_ptr: *mut Chunk,
}

impl<'a> ServerLiquidWorld<'a> {
    fn new(world: &'a mut HashMap<ChunkPos, Arc<Chunk>>) -> Self {
        let sentinel = ChunkPos {
            x: i32::MIN,
            y: i32::MIN,
            z: i32::MIN,
        };
        Self {
            world,
            read_cp: std::cell::Cell::new(sentinel),
            read_ptr: std::cell::Cell::new(std::ptr::null()),
            write_cp: sentinel,
            write_ptr: std::ptr::null_mut(),
        }
    }

    #[inline]
    fn get_chunk(&self, cp: ChunkPos) -> Option<&Chunk> {
        let ptr = self.read_ptr.get();
        if !ptr.is_null() && self.read_cp.get() == cp {
            return Some(unsafe { &*ptr });
        }
        let arc = self.world.get(&cp)?;
        let c: &Chunk = arc;
        self.read_cp.set(cp);
        self.read_ptr.set(c as *const Chunk);
        Some(c)
    }

    #[inline]
    fn get_chunk_mut(&mut self, cp: ChunkPos) -> Option<&mut Chunk> {
        if !self.write_ptr.is_null() && self.write_cp == cp {
            return Some(unsafe { &mut *self.write_ptr });
        }
        let arc = self.world.get_mut(&cp)?;
        let c: &mut Chunk = Arc::make_mut(arc);
        let ptr = c as *mut Chunk;
        self.write_cp = cp;
        self.write_ptr = ptr;
        self.read_cp.set(cp);
        self.read_ptr.set(ptr as *const Chunk);
        Some(c)
    }
}

impl LiquidWorld for ServerLiquidWorld<'_> {
    #[inline]
    fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
        let cp = ChunkPos::of_block(x, y, z);
        let Some(c) = self.get_chunk(cp) else {
            return AIR;
        };
        c.get((x & 31) as usize, (y & 31) as usize, (z & 31) as usize)
    }

    #[inline]
    fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid> {
        let cp = ChunkPos::of_block(x, y, z);
        let c = self.get_chunk(cp)?;
        c.liquid((x & 31) as usize, (y & 31) as usize, (z & 31) as usize)
    }

    #[inline]
    fn loaded(&self, x: i32, y: i32, z: i32) -> bool {
        let cp = ChunkPos::of_block(x, y, z);
        self.get_chunk(cp).is_some()
    }

    #[inline]
    fn set_liquid(&mut self, x: i32, y: i32, z: i32, l: Option<Liquid>) {
        let cp = ChunkPos::of_block(x, y, z);
        if let Some(c) = self.get_chunk_mut(cp) {
            c.set_liquid((x & 31) as usize, (y & 31) as usize, (z & 31) as usize, l);
        }
    }

    #[inline]
    fn set_block(&mut self, x: i32, y: i32, z: i32, b: BlockId) {
        let cp = ChunkPos::of_block(x, y, z);
        if let Some(c) = self.get_chunk_mut(cp) {
            let lx = (x & 31) as usize;
            let ly = (y & 31) as usize;
            let lz = (z & 31) as usize;
            c.set(lx, ly, lz, b);
            if b.is_solid() {
                c.set_liquid(lx, ly, lz, None);
            }
        }
    }

    #[inline]
    fn cell_info(&self, x: i32, y: i32, z: i32) -> (bool, BlockId, Option<Liquid>) {
        let cp = ChunkPos::of_block(x, y, z);
        let Some(c) = self.get_chunk(cp) else {
            return (false, AIR, None);
        };
        let lx = (x & 31) as usize;
        let ly = (y & 31) as usize;
        let lz = (z & 31) as usize;
        (true, c.get(lx, ly, lz), c.liquid(lx, ly, lz))
    }

    fn for_each_liquid_in_chunk(&self, cp: ChunkPos, mut f: impl FnMut([i32; 3])) {
        let Some(c) = self.get_chunk(cp) else {
            return;
        };
        let Some(amounts) = c.liquid_amounts() else {
            return;
        };
        let bx = cp.x * CHUNK_SIZE;
        let by = cp.y * CHUNK_SIZE;
        let bz = cp.z * CHUNK_SIZE;
        for lz in 0..CHUNK_SIZE {
            let z = bz + lz;
            let z_off = (lz as usize) << 10;
            for ly in 0..CHUNK_SIZE {
                let y = by + ly;
                let yz_off = z_off | ((ly as usize) << 5);
                for lx in 0..CHUNK_SIZE {
                    if amounts[yz_off | (lx as usize)] > 0 {
                        f([bx + lx, y, z]);
                    }
                }
            }
        }
    }
}

/// Spawn position: standing on the highest surface within one block of the
/// origin, so the player never starts in a one-block dimple.
pub fn spawn_point(seed: u32) -> [f32; 3] {
    let (mut bx, mut bz, mut top) = (0, 0, i32::MIN);
    for (x, z) in [
        (0, 0),
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (-1, 1),
        (1, -1),
        (-1, -1),
    ] {
        // Ground height only, and only bare columns: never spawn on a tree or plant.
        let y = terravibe_sim::worldgen::terrain::surface_height(seed, x, z);
        if surface_y(seed, x, z) != Some(y) {
            continue;
        }
        if y > top {
            (bx, bz, top) = (x, z, y);
        }
    }
    if top == i32::MIN {
        top = surface_y(seed, 0, 0).unwrap_or(0);
    }
    [bx as f32 + 0.5, (top + 1) as f32, bz as f32 + 0.5]
}

/// Y of the top non-air block in world column (x, z).
pub fn surface_y(seed: u32, x: i32, z: i32) -> Option<i32> {
    let h = terravibe_sim::worldgen::terrain::surface_height(seed, x, z);
    let cy0 = (h / CHUNK_SIZE).clamp(0, LAYERS.end - 1);
    let cy1 = (cy0 + 1).min(LAYERS.end - 1);
    let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
    for cy in (cy0..=cy1).rev() {
        let chunk = generate_chunk(seed, ChunkPos::of_block(x, cy * CHUNK_SIZE, z));
        let y_lo = cy * CHUNK_SIZE;
        if let Some(ly) = (0..CHUNK_SIZE as usize)
            .rev()
            .find(|&ly| chunk.get(l(x), ly, l(z)) != AIR)
        {
            return Some(y_lo + ly as i32);
        }
    }
    None
}

pub struct Conn {
    pub io: Box<dyn ServerConn>,
    pub id: PlayerId,
    pub player: Option<Player>,
    pub character: Option<terravibe_sim::Character>,
    pub character_facts: CharacterFacts,
    pub inputs: Vec<(u32, PlayerInput)>,
    /// Queued `Craft` requests, answered after input replay.
    pub crafts: Vec<(RecipeId, u16)>,
    pub interest: Interest,
    pub dead: bool,
    pub inv: Inventory,
    /// Cursor stack; never persisted, returned on close, disconnect and save.
    pub held: Held,
    /// Guide crafting lookup slot; never persisted, returned on close, disconnect and save.
    pub guide_slot: Option<terravibe_sim::inventory::ItemStack>,
    pub trash: Option<terravibe_sim::inventory::ItemStack>,
    /// `ClickSlot` / `CloseInventory` requests, handled after chest requests.
    pub slot_reqs: Vec<ClientMsg>,
    pub use_state: UseState,
    /// Server-side rolls for item use (bonus wood), deterministic per player.
    pub rng: Rng,
    pub sent_inv: Option<Inventory>,
    pub sent_armor: Option<[Option<ItemId>; 3]>,
    pub sent_accessories: Option<([Option<ItemId>; 5], [Prefix; 5])>,
    pub sent_vanity: Option<([Option<ItemId>; 3], [Option<ItemId>; 5], [Prefix; 5])>,
    pub sent_facts: Option<(WorldFacts, CharacterFacts)>,
    /// Enemies this client has been sent a spawn for.
    pub known_enemies: HashSet<EnemyId>,
    pub known_drops: HashSet<DropId>,
    /// Last acked input seq, and the (life, life_max, breath, dead, mana, mana_max) last sent in a `PlayerState`.
    pub last_ack: u32,
    pub sent_hp: Option<(i32, i32, i32, bool, i32, i32)>,
    /// Save slot name from `Hello`.
    pub name: String,
    /// Chest block this player has open.
    pub open_chest: Option<[i32; 3]>,
    /// Chest requests, handled after input replay.
    pub chest_reqs: Vec<ClientMsg>,
    /// Door toggles requested this tick, handled after input replay.
    pub tile_uses: Vec<[i32; 3]>,
    pub wall_breaks: Vec<[i32; 3]>,
    pub house_query: Option<[i32; 3]>,
    /// Whether the last `Projectiles` sent was non-empty.
    pub sent_proj: bool,
    pub sent_stars: bool,
    /// Talk / shop requests, answered in arrival order by `process_town_reqs`.
    pub town_reqs: Vec<ClientMsg>,
    /// The Merchant this player last talked to (sales go to him).
    pub shop_npc: Option<EnemyId>,
    pub spawn_tick: u32,
    /// Last eye origin, unit look vector, and whether primary use was held (Magic Missile steering).
    pub last_aim: Option<([f32; 3], [f32; 3], bool)>,
    /// True for the local in-process host connection (exempt from password & max_players).
    pub is_host: bool,
    /// Last received look yaw/pitch from `ClientMsg::Input`.
    pub yaw: f32,
    pub pitch: f32,
    /// Last `(armor, vanity_armor, held)` broadcast to other players in `RemotePlayerEquipment`.
    pub sent_remote_equip: Option<([Option<ItemId>; 3], [Option<ItemId>; 3], Option<ItemId>)>,
    /// Other players already spawned for this client via `RemotePlayerSpawn`.
    pub known_remotes: HashSet<PlayerId>,
}

/// Enemies are streamed to a player within this many blocks and removed
/// when farther than this from every player.
const ENEMY_RADIUS: f32 = 100.0;
/// `EnemyState` cadence: every 6 ticks = 10 Hz.
const ENEMY_STATE_TICKS: u32 = 6;
/// Autosave cadence: 60 s of server time.
const AUTOSAVE_TICKS: u32 = 3600;
/// Drops are streamed to a player within this many blocks.
const DROP_RADIUS: f32 = 100.0;

pub struct ServerCfg {
    pub natural_spawns: bool,
    /// Spawn one zombie, demon eye and slime ahead of spawn, then nothing else.
    pub demo_enemies: bool,
    /// `TERRAVIBE_DEMO=boss`: one frozen Eye of Cthulhu in front of the spawn (screenshots).
    pub demo_boss: bool,
    /// `TERRAVIBE_DEMO=npc`: one frozen Guide in front of the spawn (screenshots).
    pub demo_npc: bool,
    /// `TERRAVIBE_BENCH_SCENARIO=liquid_stress`: seed an active liquid flood near spawn.
    pub liquid_stress: bool,
    /// QA liquids scenario: starts player with low breath so drowning can be verified.
    pub qa_liquids: bool,
    /// QA kill scenario: honour `ClientMsg::QaKill` requests.
    pub qa_kill: bool,
    pub clock_start: u32,
    pub size: WorldSize,
    /// Persist the world here (`None` = session only, as in tests).
    pub save: Option<SaveCfg>,
    /// Maximum concurrent players allowed (host + guests). Default: 8.
    pub max_players: u8,
    /// Optional server password required from non-host clients on `Hello`.
    pub password: Option<String>,
}

impl Default for ServerCfg {
    fn default() -> Self {
        ServerCfg {
            natural_spawns: true,
            demo_enemies: false,
            demo_boss: false,
            demo_npc: false,
            liquid_stress: false,
            qa_liquids: false,
            qa_kill: false,
            // Main.time = 13500 for a new world (gameplay spec): 8:15 AM.
            clock_start: 13_500,
            size: WorldSize::Small,
            save: None,
            max_players: 8,
            password: None,
        }
    }
}

pub struct Server {
    seed: u32,
    spawn: [f32; 3],
    pub bounds: WorldBounds,
    world: HashMap<ChunkPos, Arc<Chunk>>,
    /// Chunks touched by a successful edit. They stay resident for the life of
    /// the session so walking away and back cannot regenerate pristine terrain
    /// (mined tunnels, placed blocks). Only edited chunks accumulate; the rest
    /// still follow streaming interest.
    edited: HashSet<ChunkPos>,
    generator: Generator,
    pub conns: Vec<Conn>,
    next_id: PlayerId,
    clock: WorldClock,
    enemies: Vec<Enemy>,
    next_enemy_id: u32,
    pub drops: Vec<Drop>,
    next_drop_id: u32,
    spawner: Spawner,
    critter_ticks: u32,
    dungeon_spawn_ticks: u32,
    eow_out_of_biome_ticks: u32,
    critter_rng: Rng,
    rng: Rng,
    cfg: ServerCfg,
    demo_done: bool,
    saver: Option<Autosaver>,
    /// Edited chunks not yet written by a save.
    dirty: HashSet<ChunkPos>,
    /// Saved players by name (loaded at start, refreshed on disconnect).
    saved_players: HashMap<String, PlayerSave>,
    /// Chest contents by block position (never stored in chunk data).
    chests: HashMap<[i32; 3], ChestInv>,
    /// Wall edits made this tick, drained into `WallChanged` broadcasts.
    wall_edits: Vec<([i32; 3], bool)>,
    save_ticks: u32,
    /// Live arrows, capped at `MAX_PROJECTILES`.
    projectiles: Vec<Projectile>,
    next_proj_id: u32,
    /// Live falling stars.
    stars: Vec<Star>,
    next_star_id: u32,
    star_boost: f32,
    /// The live Eye of Cthulhu: its `Enemy` id and AI registers. The body is in `enemies`.
    boss: Option<(EnemyId, EyeBoss)>,
    /// The live Wall of Flesh state machine. Parts are in `enemies`.
    wof: Option<terravibe_sim::wof::WofBoss>,
    /// A natural night spawn has been rolled and waits for `SPAWN_AFTER_NIGHT_TICK`.
    eye_pending: bool,
    /// Whether the previous tick was day (to find the start of a night).
    was_day: bool,
    /// Valid rooms found by the 2 s scan; a house holds at most one resident.
    houses: Vec<House>,
    /// Count of room checks performed (for rate limiting verification).
    pub room_checks: u64,
    /// Wander state per town NPC.
    town_ai: HashMap<EnemyId, Wander>,
    /// The Guide has been created at least once in this world.
    guide_spawned: bool,
    /// Authoritative liquid flow simulation.
    liquid_sim: LiquidSim,
    /// Spawn a 20,000-cell water flood once around spawn for `--liquid-stress` benchmarks.
    liquid_stress_done: bool,
    /// Broken shadow orbs in this world.
    pub shadow_orbs_broken: u32,
    /// Whether Skeletron has been defeated in this world.
    pub downed_skeletron: bool,
    /// Locked dungeon chests requiring a golden key.
    pub locked_chests: HashSet<[i32; 3]>,
    /// Locked underworld house chests requiring a shadow key.
    pub shadow_chests: HashSet<[i32; 3]>,
    /// Town NPCs that died and await a daytime, boss-free respawn check.
    dead_town_npcs: HashSet<TownKind>,
    /// World progression facts.
    pub facts: WorldFacts,
    /// Authoritative weather state.
    pub weather: WeatherState,
    /// Active Blood Moon night event.
    pub blood_moon: bool,
    /// Remaining Goblin Army enemies to defeat.
    pub goblin_army_remaining: u32,
}

pub(crate) fn is_pos_in_corruption(
    world: &HashMap<ChunkPos, Arc<Chunk>>,
    seed: u32,
    size: WorldSize,
    pos: [f32; 3],
) -> bool {
    let (px, pz) = (pos[0].floor() as i32, pos[2].floor() as i32);
    if terravibe_sim::worldgen::corruption::corruption_at_for_size(seed, px, pz, size) > 0.0 {
        return true;
    }
    let py = pos[1].floor() as i32;
    let w = WorldQ(world);
    for dx in -10..=10 {
        for dz in -10..=10 {
            for dy in -5..=5 {
                let b = w.block(px + dx, py + dy, pz + dz);
                if matches!(
                    b,
                    terravibe_sim::block::EBONSTONE
                        | terravibe_sim::block::CORRUPT_GRASS
                        | terravibe_sim::block::SHADOW_ORB
                ) {
                    return true;
                }
            }
        }
    }
    false
}

impl Server {
    pub fn new(seed: u32) -> Server {
        Server::with_cfg(seed, ServerCfg::default())
    }

    pub fn with_cfg(mut seed: u32, mut cfg: ServerCfg) -> Server {
        let mut world = HashMap::new();
        let mut edited = HashSet::new();
        let mut saved_players = HashMap::new();
        let mut chests = HashMap::new();
        let mut houses = Vec::new();
        let mut guide_spawned = false;
        let mut locked_chests = HashSet::new();
        let mut shadow_chests = HashSet::new();
        let mut facts = WorldFacts::default();
        let mut weather = WeatherState::default();
        let mut saver = None;
        if let Some(sc) = cfg.save.take() {
            match SaveDir::open(&sc.root, &sc.world).and_then(|d| Ok((d.load_world()?, d))) {
                Ok((snap, dir)) => {
                    if let Some(snap) = snap {
                        seed = snap.seed;
                        cfg.clock_start = snap.clock_tick;
                        cfg.size = snap.size;
                        facts = snap.facts;
                        weather = snap.weather;
                        let bounds_load = WorldBounds::new(cfg.size);
                        for (p, c) in snap.chunks {
                            if bounds_load.chunk_inside(p) {
                                edited.insert(p);
                                world.insert(p, c);
                            }
                        }
                        saved_players = save::players_map(snap.players);
                        chests = snap.chests.into_iter().collect();
                        locked_chests = snap.locked_chests.into_iter().collect();
                        shadow_chests = snap.shadow_chests.into_iter().collect();
                        guide_spawned = snap.towns.guide_spawned;
                        houses = snap
                            .towns
                            .npcs
                            .into_iter()
                            .map(|(k, home, origin)| House {
                                origin,
                                home,
                                npc: Some(k),
                            })
                            .collect();
                        if houses.iter().any(|h| h.npc == Some(TownKind::Merchant)) {
                            facts.merchant_housed = true;
                        }
                    }
                    saver = Some(Autosaver::new(dir));
                }
                // Unusable save: run without persistence rather than overwrite it.
                Err(e) => eprintln!("terravibe: save disabled, cannot load world: {e}"),
            }
        }
        let bounds = WorldBounds::new(cfg.size);
        let mut liquid_sim = LiquidSim::new();
        if !world.is_empty() {
            let liquid_chunks: Vec<ChunkPos> = world
                .iter()
                .filter(|(_, c)| c.has_liquid())
                .map(|(&p, _)| p)
                .collect();
            let adapter = ServerLiquidWorld::new(&mut world);
            for p in liquid_chunks {
                liquid_sim.wake_chunk(&adapter, p);
            }
        }
        Server {
            seed,
            spawn: spawn_point(seed),
            bounds,
            world,
            edited,
            generator: Generator::new(seed, cfg.size),
            conns: Vec::new(),
            next_id: 1,
            clock: WorldClock {
                tick: cfg.clock_start,
            },
            enemies: Vec::new(),
            next_enemy_id: 1,
            drops: Vec::new(),
            next_drop_id: 1,
            spawner: Spawner::default(),
            critter_ticks: 0,
            dungeon_spawn_ticks: 180,
            eow_out_of_biome_ticks: 0,
            critter_rng: Rng::new(seed as u64 ^ 0x9E37_79B9_7F4A_7C15),
            rng: Rng::new(seed as u64 ^ 0xE4E3_71E5),
            cfg,
            demo_done: false,
            saver,
            dirty: HashSet::new(),
            saved_players,
            chests,
            wall_edits: Vec::new(),
            save_ticks: 0,
            projectiles: Vec::new(),
            next_proj_id: 0,
            stars: Vec::new(),
            next_star_id: 1,
            star_boost: 1.0,
            boss: None,
            wof: None,
            eye_pending: false,
            was_day: true,
            houses,
            room_checks: 0,
            town_ai: HashMap::new(),
            guide_spawned,
            liquid_sim,
            liquid_stress_done: false,
            shadow_orbs_broken: facts.shadow_orbs_broken,
            downed_skeletron: facts.downed_skeletron,
            locked_chests,
            shadow_chests,
            dead_town_npcs: HashSet::new(),
            facts,
            weather,
            blood_moon: false,
            goblin_army_remaining: 0,
        }
    }

    pub fn handle_shadow_orb_broken(&mut self, pos: [i32; 3]) {
        let loot =
            terravibe_sim::drop::roll_shadow_orb_loot(self.shadow_orbs_broken, &mut self.rng);
        self.shadow_orbs_broken += 1;
        self.facts.shadow_orbs_broken = self.shadow_orbs_broken;
        let drop_pos = [
            pos[0] as f32 + 0.5,
            pos[1] as f32 + 0.5,
            pos[2] as f32 + 0.5,
        ];
        for stack in loot {
            self.add_drop(stack, drop_pos);
        }
        if self.shadow_orbs_broken % 3 == 0 {
            self.spawn_eater_of_worlds(drop_pos);
        }
        if !self.facts.meteorite_crashed && (self.shadow_orbs_broken == 1 || self.rng.below(2) == 0)
        {
            self.crash_meteorite();
        }
    }

    /// Carves a surface meteorite crater away from spawn and oceans, placing `METEORITE_ORE`.
    pub fn crash_meteorite(&mut self) -> Option<[i32; 3]> {
        use terravibe_sim::world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, WorldBounds};
        use terravibe_sim::worldgen::terrain::surface_height_for_size;

        let bounds = WorldBounds::new(self.cfg.size);
        let max_r = (bounds.half - OCEAN_WIDTH - BEACH_WIDTH - 20).max(110);
        let mut center = None;
        for _ in 0..24 {
            let angle = (self.rng.next_u32() as f64 / u32::MAX as f64) * std::f64::consts::TAU;
            let dist = 90.0
                + (self.rng.next_u32() as f64 / u32::MAX as f64) * (max_r as f64 - 90.0).max(20.0);
            let cx = (angle.cos() * dist).round() as i32;
            let cz = (angle.sin() * dist).round() as i32;
            if !self.is_corrupt_at(cx, cz) {
                let cy = surface_height_for_size(self.seed, cx, cz, self.cfg.size);
                center = Some([cx, cy, cz]);
                break;
            }
        }
        let [cx, cy, cz] = center?;
        self.facts.meteorite_crashed = true;
        let radius = 6i32;
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    let d2 = dx * dx + dy * dy + dz * dz;
                    let (wx, wy, wz) = (cx + dx, cy + dy, cz + dz);
                    let pos = ChunkPos::of_block(wx, wy, wz);
                    self.world.entry(pos).or_insert_with(|| {
                        Arc::new(terravibe_sim::worldgen::terrain::generate_chunk_sized(
                            self.seed,
                            pos,
                            self.cfg.size,
                        ))
                    });
                    if d2 <= 14 && wy >= cy - 2 {
                        self.set_block([wx, wy, wz], terravibe_sim::block::AIR);
                    } else if d2 <= radius * radius && wy <= cy + 1 {
                        self.set_block([wx, wy, wz], terravibe_sim::block::METEORITE_ORE);
                    }
                }
            }
        }
        for c in &self.conns {
            if c.player.is_some() {
                let _ = c.io.send(ServerMsg::SyncFacts {
                    world: self.facts,
                    character: c.character_facts,
                });
            }
        }
        Some([cx, cy, cz])
    }

    pub fn handle_pot_broken(&mut self, pos: [i32; 3]) {
        let loot = terravibe_sim::drop::roll_pot_loot(&mut self.rng);
        let drop_pos = [
            pos[0] as f32 + 0.5,
            pos[1] as f32 + 0.5,
            pos[2] as f32 + 0.5,
        ];
        for stack in loot {
            self.add_drop(stack, drop_pos);
        }
    }

    pub fn is_corrupt_at(&self, x: i32, z: i32) -> bool {
        terravibe_sim::worldgen::corruption::corruption_at_for_size(self.seed, x, z, self.cfg.size)
            > 0.0
    }

    pub fn is_in_corruption(&self, pos: [f32; 3]) -> bool {
        is_pos_in_corruption(&self.world, self.seed, self.cfg.size, pos)
    }

    pub fn any_boss_alive(&self) -> bool {
        self.boss.is_some()
            || self.wof.is_some()
            || self.enemies.iter().any(|e| e.hp > 0 && e.kind.is_boss())
    }

    pub fn has_eater_of_worlds(&self) -> bool {
        self.enemies
            .iter()
            .any(|e| e.hp > 0 && terravibe_sim::enemy::is_eater_of_worlds(e.kind))
    }

    pub fn spawn_eater_of_worlds(&mut self, at: [f32; 3]) {
        if self.has_eater_of_worlds() {
            return;
        }
        let spawn_pos = [at[0], at[1] - 8.0, at[2]];
        let segments = terravibe_sim::enemy::spawn_eater_of_worlds(
            self.next_enemy_id,
            spawn_pos,
            terravibe_sim::enemy::EATER_OF_WORLDS_DEFAULT_SEGMENTS,
        );
        self.next_enemy_id += segments.len() as u32;
        self.enemies.extend(segments);
    }

    pub fn spawn_bone_serpent(&mut self, at: [f32; 3]) {
        let segments = terravibe_sim::enemy::spawn_bone_serpent(
            self.next_enemy_id,
            at,
            terravibe_sim::enemy::BONE_SERPENT_DEFAULT_SEGMENTS,
        );
        self.next_enemy_id += segments.len() as u32;
        self.enemies.extend(segments);
    }

    pub fn connect(&mut self, io: Box<dyn ServerConn>) {
        self.connect_inner(io, false);
    }

    pub fn connect_host(&mut self, io: Box<dyn ServerConn>) -> PlayerId {
        self.connect_inner(io, true)
    }

    fn connect_inner(&mut self, io: Box<dyn ServerConn>, is_host: bool) -> PlayerId {
        let id = self.next_id;
        self.next_id += 1;
        self.conns.push(Conn {
            io,
            id,
            player: None,
            character: None,
            character_facts: CharacterFacts::default(),
            inputs: Vec::new(),
            crafts: Vec::new(),
            interest: Interest::default(),
            dead: false,
            inv: Inventory::starting(&mut Rng::new(self.seed as u64 ^ id as u64)),
            held: Held::default(),
            guide_slot: None,
            trash: None,
            slot_reqs: Vec::new(),
            use_state: UseState::default(),
            rng: Rng::new(self.seed as u64 ^ id as u64),
            sent_inv: None,
            known_enemies: HashSet::new(),
            known_drops: HashSet::new(),
            sent_armor: None,
            sent_accessories: None,
            sent_vanity: None,
            sent_facts: None,
            last_ack: 0,
            sent_hp: None,
            name: "player".into(),
            open_chest: None,
            chest_reqs: Vec::new(),
            tile_uses: Vec::new(),
            wall_breaks: Vec::new(),
            house_query: None,
            sent_proj: false,
            sent_stars: false,
            town_reqs: Vec::new(),
            shop_npc: None,
            spawn_tick: 0,
            last_aim: None,
            is_host,
            yaw: 0.0,
            pitch: 0.0,
            sent_remote_equip: None,
            known_remotes: HashSet::new(),
        });
        id
    }

    pub fn has_conn(&self, id: PlayerId) -> bool {
        self.conns.iter().any(|c| c.id == id && !c.dead)
    }

    pub fn connection_count(&self) -> usize {
        self.conns.len()
    }

    pub fn liquid_pending(&self) -> usize {
        self.liquid_sim.pending()
    }

    pub fn set_block(&mut self, p: [i32; 3], b: BlockId) {
        let cp = ChunkPos::of_block(p[0], p[1], p[2]);
        if let Some(c) = self.world.get_mut(&cp) {
            let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
            let ch = Arc::make_mut(c);
            ch.set(l(p[0]), l(p[1]), l(p[2]), b);
            if b.is_solid() {
                ch.set_liquid(l(p[0]), l(p[1]), l(p[2]), None);
            }
            self.edited.insert(cp);
            self.dirty.insert(cp);
            self.liquid_sim.wake(p);
        }
    }

    /// Steps active liquids under the per-tick budget and marks modified chunks dirty/edited.
    fn tick_liquids(&mut self, tick: u32) {
        if self.cfg.liquid_stress && !self.liquid_stress_done {
            let [sx, sy, sz] = self.spawn;
            let base = [sx.floor() as i32, sy.floor() as i32 + 2, sz.floor() as i32];
            // Wait until the chunks holding the 32x32x24 basin around spawn are resident.
            let ready = (0..2).all(|cx| {
                (0..2).all(|cz| {
                    let cp0 = ChunkPos::of_block(base[0] + cx * 32, base[1], base[2] + cz * 32);
                    let cp1 =
                        ChunkPos::of_block(base[0] + cx * 32, base[1] + 24, base[2] + cz * 32);
                    self.world.contains_key(&cp0) && self.world.contains_key(&cp1)
                })
            });
            if ready {
                self.seed_liquid_flood(base, 20_000);
                self.liquid_stress_done = true;
            }
        }
        if self.liquid_sim.pending() == 0 {
            return;
        }
        let report = {
            let mut adapter = ServerLiquidWorld::new(&mut self.world);
            self.liquid_sim
                .step(&mut adapter, tick, MAX_CELL_UPDATES_PER_TICK)
        };
        let mut deltas_by_chunk: HashMap<ChunkPos, Vec<(u16, u16)>> = HashMap::new();
        for pos in &report.changed {
            let cp = ChunkPos::of_block(pos[0], pos[1], pos[2]);
            self.edited.insert(cp);
            self.dirty.insert(cp);
            let lx = pos[0].rem_euclid(CHUNK_SIZE) as usize;
            let ly = pos[1].rem_euclid(CHUNK_SIZE) as usize;
            let lz = pos[2].rem_euclid(CHUNK_SIZE) as usize;
            let idx = (lx + ly * 32 + lz * 1024) as u16;
            let packed = Liquid::pack(self.world.get(&cp).and_then(|c| c.liquid(lx, ly, lz)));
            deltas_by_chunk.entry(cp).or_default().push((idx, packed));
        }
        for (cp, cells) in deltas_by_chunk {
            if cells.len() <= LIQUID_DELTA_MAX {
                let msg = ServerMsg::LiquidDelta { pos: cp, cells };
                for c in &mut self.conns {
                    if c.interest.sent.contains(&cp) {
                        c.dead |= c.io.send(msg.clone()).is_err();
                    }
                }
            } else if let Some(chunk) = self.world.get(&cp) {
                let msg = ServerMsg::Chunk {
                    pos: cp,
                    chunk: Arc::clone(chunk),
                };
                for c in &mut self.conns {
                    if c.interest.sent.contains(&cp) {
                        c.dead |= c.io.send(msg.clone()).is_err();
                    }
                }
            }
        }
        for &pos in &report.obsidian {
            let cp = ChunkPos::of_block(pos[0], pos[1], pos[2]);
            self.edited.insert(cp);
            self.dirty.insert(cp);
            for c in &mut self.conns {
                if c.interest.sent.contains(&cp) {
                    c.dead |=
                        c.io.send(ServerMsg::BlockChanged {
                            pos,
                            block: terravibe_sim::OBSIDIAN,
                        })
                        .is_err();
                }
            }
        }
    }

    /// Builds a sealed stone basin starting at `base` and seeds `cell_count` active water cells
    /// so they flow, equalise and settle deterministically.
    pub fn seed_liquid_flood(&mut self, base: [i32; 3], cell_count: usize) {
        let [bx, by, bz] = base;
        let width = 61i32;
        let depth = 52i32;
        let height = 13i32;

        // Ensure all chunks covering the basin exist in `self.world`.
        for x in (bx - 32..=bx + width + 32).step_by(32) {
            for y in (by - 32..=by + height + 32).step_by(32) {
                for z in (bz - 32..=bz + depth + 32).step_by(32) {
                    let cp = ChunkPos::of_block(x, y, z);
                    self.world
                        .entry(cp)
                        .or_insert_with(|| Arc::new(Chunk::filled(AIR)));
                    self.edited.insert(cp);
                }
            }
        }

        let mut adapter = ServerLiquidWorld::new(&mut self.world);
        for x in 0..width {
            for z in 0..depth {
                for y in 0..height {
                    let wx = bx + x;
                    let wy = by + y;
                    let wz = bz + z;
                    let is_wall = x == 0
                        || x == width - 1
                        || z == 0
                        || z == depth - 1
                        || y == 0
                        || y == height - 1;
                    if is_wall || x % 3 == 0 {
                        adapter.set_block(wx, wy, wz, terravibe_sim::STONE);
                    } else {
                        adapter.set_block(wx, wy, wz, AIR);
                        adapter.set_liquid(wx, wy, wz, None);
                    }
                }
            }
        }

        let mut placed = 0usize;
        // 40 liquid x-columns (`x % 3 != 0` in `1..60`) x 50 z-columns (`1..51`) = 2,000 water cells per layer.
        // Across 10 layers (`y = 1..=10`), that is 20,000 real water cells.
        // Each 2-cell pair `(x % 3 == 1: 255, x % 3 == 2: 253)` equalises in 1 step to `(254, 254)`
        // so all 20,000 water cells flow, equalise, and drain the pending queue monotonically to 0.
        'outer: for y in (1..=10).rev() {
            for x in 1..width - 1 {
                if x % 3 == 0 {
                    continue;
                }
                for z in 1..depth - 1 {
                    if placed >= cell_count {
                        break 'outer;
                    }
                    let wx = bx + x;
                    let wy = by + y;
                    let wz = bz + z;
                    let amt = if y == 10 && x % 3 == 2 { 253 } else { 255 };
                    adapter.set_liquid(wx, wy, wz, Some(Liquid::water(amt)));
                    self.liquid_sim.wake([wx, wy, wz]);
                    placed += 1;
                }
            }
        }
    }

    /// One 60 Hz step. Never blocks on generation.
    pub fn tick(&mut self) {
        let spawn = self.spawn;
        self.clock.advance(1);
        let tick = self.clock.tick;
        let mut respawn_reqs = Vec::new();
        let mut qa_kill_reqs = Vec::new();
        let mut creative_reqs: Vec<(usize, ClientMsg)> = Vec::new();
        let mut newly_joined: Vec<usize> = Vec::new();
        let mut chat_msgs: Vec<(usize, String)> = Vec::new();
        let mut active_players = self
            .conns
            .iter()
            .filter(|c| c.player.is_some() && !c.dead)
            .count();
        for (cid, c) in self.conns.iter_mut().enumerate() {
            loop {
                match c.io.try_recv() {
                    Ok(Some(ClientMsg::Hello {
                        name,
                        character,
                        protocol_version,
                        password,
                        ..
                    })) => {
                        if c.player.is_some() {
                            continue;
                        }
                        if !c.is_host {
                            if protocol_version != PROTOCOL_VERSION {
                                let _ = c.io.send(ServerMsg::Rejected {
                                    reason: RejectReason::ProtocolMismatch,
                                });
                                c.dead = true;
                                break;
                            }
                            if active_players >= self.cfg.max_players as usize {
                                let _ = c.io.send(ServerMsg::Rejected {
                                    reason: RejectReason::ServerFull,
                                });
                                c.dead = true;
                                break;
                            }
                            if let Some(expected) = self.cfg.password.as_deref() {
                                let ok = password
                                    .as_deref()
                                    .is_some_and(|p| ct_eq(p.as_bytes(), expected.as_bytes()));
                                if !ok {
                                    let _ = c.io.send(ServerMsg::Rejected {
                                        reason: RejectReason::BadPassword,
                                    });
                                    c.dead = true;
                                    break;
                                }
                            }
                        }
                        let char_opt =
                            character.and_then(|b| terravibe_sim::Character::from_bytes(&b).ok());
                        let mut pl = Player::new(spawn);
                        if let Some(ch) = char_opt {
                            c.name = save::sanitize(&ch.name, "player");
                            c.inv = ch.inventory.clone();
                            pl.armor = ch.armor;
                            pl.accessories = ch.accessories;
                            pl.accessory_prefixes = ch.accessory_prefixes;
                            pl.vanity_armor = ch.vanity_armor;
                            pl.vanity_accessories = ch.vanity_accessories;
                            pl.vanity_accessory_prefixes = ch.vanity_accessory_prefixes;
                            pl.vitals.life_max = ch.life_max;
                            pl.vitals.life = ch.life.clamp(1, ch.life_max.max(1));
                            pl.vitals.mana_max = ch.mana_max;
                            pl.sync_accessories();
                            pl.vitals.mana = ch.mana.clamp(0, pl.effective_mana_max());
                            pl.vitals.potion_delay_ticks = ch.potion_delay;
                            pl.vitals.obsidian_skin_ticks = ch.obsidian_skin_ticks.max(0);
                            if let Some(ps) = self.saved_players.get(&c.name) {
                                pl.body = terravibe_sim::player::PlayerBody::new(ps.pos);
                            }
                            self.bounds
                                .clamp_body(&mut pl.body.pos, &mut pl.body.vel, HALF_WIDTH);
                            c.character_facts = ch.facts;
                            c.character = Some(ch);
                        } else {
                            c.name = save::sanitize(name.as_deref().unwrap_or(""), "player");
                            if let Some(ps) = self.saved_players.get(&c.name) {
                                c.inv = ps.inventory.clone();
                                pl.armor = ps.armor;
                                pl.vitals.life_max = ps.life_max;
                                pl.vitals.life = ps.life.clamp(1, ps.life_max.max(1));
                                pl.vitals.potion_delay_ticks = ps.potion_delay;
                                pl.body = terravibe_sim::player::PlayerBody::new(ps.pos);
                                self.bounds.clamp_body(
                                    &mut pl.body.pos,
                                    &mut pl.body.vel,
                                    HALF_WIDTH,
                                );
                            }
                        }
                        if self.cfg.qa_liquids {
                            pl.vitals.breath = 4;
                        }
                        let spawn = pl.body.pos;
                        c.sent_hp = Some((
                            pl.vitals.life,
                            pl.vitals.life_max,
                            pl.vitals.breath,
                            pl.dead,
                            pl.vitals.mana,
                            pl.vitals.mana_max,
                        ));
                        c.player = Some(pl);
                        c.spawn_tick = tick;
                        c.interest.centre = Some(spawn);
                        c.dead |=
                            c.io.send(ServerMsg::Welcome {
                                id: c.id,
                                spawn,
                                size: self.cfg.size,
                            })
                            .is_err();
                        c.dead |= c.io.send(ServerMsg::Time { tick }).is_err();
                        c.dead |=
                            c.io.send(ServerMsg::WeatherState {
                                kind: self.weather.kind,
                                intensity: self.weather.intensity,
                            })
                            .is_err();
                        if !c.dead {
                            active_players += 1;
                            newly_joined.push(cid);
                        }
                    }
                    Ok(Some(ClientMsg::Input { seq, input })) => c.inputs.push((seq, input)),
                    Ok(Some(ClientMsg::Spectate { pos })) => c.interest.centre = Some(pos),
                    Ok(Some(ClientMsg::Craft { recipe, count })) => c.crafts.push((recipe, count)),
                    Ok(Some(ClientMsg::Equip { from })) => {
                        if let Some(p) = c.player.as_mut() {
                            p.equip(&mut c.inv, from as usize);
                        }
                    }
                    Ok(Some(ClientMsg::Unequip { slot })) => {
                        if let Some(p) = c.player.as_mut() {
                            p.unequip(&mut c.inv, slot);
                        }
                    }
                    Ok(Some(
                        m @ (ClientMsg::OpenChest { .. }
                        | ClientMsg::CloseChest
                        | ClientMsg::ChestMove { .. }
                        | ClientMsg::ChestQuickStack),
                    )) => c.chest_reqs.push(m),
                    Ok(Some(
                        m @ (ClientMsg::ClickSlot { .. }
                        | ClientMsg::CloseInventory
                        | ClientMsg::CloseGuideCrafting
                        | ClientMsg::GuideSlotShift { .. }
                        | ClientMsg::TrashShift { .. }
                        | ClientMsg::SortInventory { .. }
                        | ClientMsg::DropItem { .. }
                        | ClientMsg::QuickHeal
                        | ClientMsg::QuickMana
                        | ClientMsg::QuickBuff),
                    )) => c.slot_reqs.push(m),
                    Ok(Some(ClientMsg::DismissOnboarding)) => {
                        c.character_facts.onboarding_dismissed = true;
                        if let Some(ch) = c.character.as_mut() {
                            ch.facts.onboarding_dismissed = true;
                        }
                    }
                    Ok(Some(
                        m @ (ClientMsg::TalkNpc { .. }
                        | ClientMsg::ShopBuy { .. }
                        | ClientMsg::ShopSell { .. }
                        | ClientMsg::GuideAdvice
                        | ClientMsg::AssignHousing { .. }
                        | ClientMsg::HealAtNurse { .. }
                        | ClientMsg::ReforgeItem { .. }),
                    )) => c.town_reqs.push(m),
                    Ok(Some(ClientMsg::UseTile { pos })) => c.tile_uses.push(pos),
                    Ok(Some(ClientMsg::BreakWall { pos })) => c.wall_breaks.push(pos),
                    Ok(Some(ClientMsg::QueryHouse { pos })) => c.house_query = Some(pos),
                    Ok(Some(ClientMsg::Respawn)) => respawn_reqs.push(cid),
                    Ok(Some(ClientMsg::QaKill)) => qa_kill_reqs.push(cid),
                    Ok(Some(
                        m @ (ClientMsg::CreativeGive { .. } | ClientMsg::CreativeAction(_)),
                    )) => {
                        creative_reqs.push((cid, m));
                    }
                    Ok(Some(ClientMsg::Chat { text })) => {
                        chat_msgs.push((cid, text));
                    }
                    Ok(Some(ClientMsg::Bye)) | Err(Disconnected) => {
                        c.dead = true;
                        break;
                    }
                    Ok(None) => break,
                }
            }
        }

        if !newly_joined.is_empty() {
            for joined_idx in newly_joined {
                self.announce_player_joined(joined_idx);
            }
            self.broadcast_player_list();
        }

        for (cid, text) in chat_msgs {
            let Some(c) = self
                .conns
                .get(cid)
                .filter(|c| !c.dead && c.player.is_some())
            else {
                continue;
            };
            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }
            let clamped: String = trimmed.chars().take(200).collect();
            let sender = Some(c.name.clone());
            self.broadcast_to_joined(ServerMsg::Chat {
                sender,
                text: clamped,
            });
        }

        for cid in respawn_reqs {
            self.handle_client_msg(cid, ClientMsg::Respawn);
        }

        for cid in qa_kill_reqs {
            self.handle_client_msg(cid, ClientMsg::QaKill);
        }

        for (cid, msg) in creative_reqs {
            self.handle_client_msg(cid, msg);
        }

        let weather_changed = self.weather.step(self.seed, tick);
        if tick % 60 == 0 {
            for c in &mut self.conns {
                c.dead |= c.io.send(ServerMsg::Time { tick }).is_err();
            }
        }
        if weather_changed || tick % 60 == 0 {
            for c in &mut self.conns {
                c.dead |=
                    c.io.send(ServerMsg::WeatherState {
                        kind: self.weather.kind,
                        intensity: self.weather.intensity,
                    })
                    .is_err();
            }
        }

        // One tick_player per input, in arrival order.
        let mut edits: Vec<([i32; 3], BlockId)> = Vec::new();
        let mut liquid_edits: Vec<[i32; 3]> = Vec::new();
        let mut new_drops: Vec<(terravibe_sim::inventory::ItemStack, [f32; 3])> = Vec::new();
        let mut shots: Vec<(
            PlayerId,
            ItemId,
            ItemId,
            Prefix,
            [f32; 3],
            [f32; 3],
            f32,
            i32,
            i32,
        )> = Vec::new();
        let mut casts: Vec<(PlayerId, ItemId, Prefix, [f32; 3], [f32; 3])> = Vec::new();
        let mut throws: Vec<(PlayerId, ItemId, [f32; 3], [f32; 3])> = Vec::new();
        let mut grapples: Vec<(PlayerId, [f32; 3], [f32; 3])> = Vec::new();
        let mut melee_combat_hits: Vec<ServerMsg> = Vec::new();
        let mut summons: Vec<[f32; 3]> = Vec::new();
        let mut start_goblin_army = false;
        let mut boss_summons: Vec<(EnemyKind, [f32; 3])> = Vec::new();
        let mut eater_summons: Vec<[f32; 3]> = Vec::new();
        let mut critter_spawns: Vec<(EnemyKind, [f32; 3])> = Vec::new();
        let mut broken_orbs: Vec<[i32; 3]> = Vec::new();
        let mut broken_pots: Vec<[i32; 3]> = Vec::new();
        let mut player_deaths = Vec::new();
        let eow_alive = self.has_eater_of_worlds();
        for (cid, c) in self.conns.iter_mut().enumerate() {
            let inputs = std::mem::take(&mut c.inputs);
            let Some(p) = c.player.as_mut() else { continue };
            let Some(&(last_seq, _)) = inputs.last() else {
                continue;
            };
            let mut events = Vec::new();
            let mut uses = Vec::new();
            let mut swings: Vec<(ItemId, Prefix, f32, f32)> = Vec::new();
            for (_, input) in &inputs {
                if self.cfg.qa_liquids && p.body.wet.is_some() {
                    p.vitals.breath = 0;
                }
                events.extend(tick_player_bounded(
                    &WorldQ(&self.world),
                    p,
                    input,
                    Some(&self.bounds),
                ));
                c.yaw = input.yaw;
                c.pitch = input.pitch;
                if p.dead {
                    c.last_aim = None;
                    continue;
                }
                let eye = [p.body.pos[0], p.body.pos[1] + EYE_HEIGHT, p.body.pos[2]];
                let (sy, cy) = input.yaw.sin_cos();
                let (sp, cp) = input.pitch.sin_cos();
                c.last_aim = Some((eye, [-sy * cp, sp, -cy * cp], input.use_held));
                let mut w = EditQ {
                    world: &mut self.world,
                    log: &mut edits,
                    chests: &mut self.chests,
                    wall_log: &mut self.wall_edits,
                    liquid_log: &mut liquid_edits,
                };
                let mana_avail = if p.has_meteor_set()
                    && c.inv
                        .selected_stack()
                        .is_some_and(|s| s.id == terravibe_sim::item::SPACE_GUN)
                {
                    Some(i32::MAX)
                } else {
                    Some(p.vitals.mana)
                };
                let used = tick_use_with_mana(
                    &mut w,
                    &p.body,
                    &mut c.inv,
                    &mut c.use_state,
                    input,
                    &mut c.rng,
                    mana_avail,
                );
                for u in &used {
                    if let UseEvent::Swing { item, prefix } = u {
                        swings.push((*item, *prefix, input.yaw, input.pitch));
                    }
                }
                for u in used {
                    match u {
                        UseEvent::ItemDrop { stack, pos } => new_drops.push((stack, pos)),
                        UseEvent::Shoot {
                            bow,
                            ammo,
                            prefix,
                            origin,
                            dir,
                        } => shots.push((
                            c.id,
                            bow,
                            ammo,
                            prefix,
                            origin,
                            dir,
                            p.ranged_damage_mult(),
                            p.ranged_crit_bonus(),
                            p.armor_penetration(),
                        )),
                        UseEvent::Cast {
                            wand,
                            prefix,
                            origin,
                            dir,
                        } => {
                            casts.push((c.id, wand, prefix, origin, dir));
                        }
                        UseEvent::Throw { item, origin, dir } => {
                            throws.push((c.id, item, origin, dir));
                        }
                        UseEvent::Grapple { origin, dir } => {
                            grapples.push((c.id, origin, dir));
                        }
                        // Server-authoritative crystal use: life_max reaches the client
                        // in the PlayerState sent below.
                        UseEvent::Consume { item } if item == SUSPICIOUS_LOOKING_EYE => {
                            // gameplay spec case 43: night only, one Eye at a time; the item is
                            // spent only when the boss really spawns (gameplay spec).
                            if boss::can_summon(self.clock.is_day(), self.boss.is_some())
                                && c.inv.selected_stack().is_some_and(|s| s.id == item)
                            {
                                c.inv.take_one_selected();
                                summons.push(p.body.pos);
                            }
                        }
                        UseEvent::Consume { item } if item == terravibe_sim::item::WORM_FOOD => {
                            if !eow_alive
                                && eater_summons.is_empty()
                                && is_pos_in_corruption(
                                    &self.world,
                                    self.seed,
                                    self.cfg.size,
                                    p.body.pos,
                                )
                                && c.inv.selected_stack().is_some_and(|s| s.id == item)
                            {
                                c.inv.take_one_selected();
                                eater_summons.push(p.body.pos);
                            }
                        }
                        UseEvent::Consume { item }
                            if item == terravibe_sim::item::GOBLIN_BATTLE_STANDARD =>
                        {
                            if self.goblin_army_remaining == 0
                                && c.inv.selected_stack().is_some_and(|s| s.id == item)
                            {
                                c.inv.take_one_selected();
                                start_goblin_army = true;
                            }
                        }
                        UseEvent::Consume { item } if item == terravibe_sim::item::SLIME_CROWN => {
                            if !self.enemies.iter().any(|e| e.kind == EnemyKind::KingSlime)
                                && c.inv.selected_stack().is_some_and(|s| s.id == item)
                            {
                                c.inv.take_one_selected();
                                boss_summons.push((
                                    EnemyKind::KingSlime,
                                    [p.body.pos[0] + 10.0, p.body.pos[1] + 6.0, p.body.pos[2]],
                                ));
                            }
                        }
                        UseEvent::Consume { item } if item == terravibe_sim::item::ABEEMINATION => {
                            if !self.enemies.iter().any(|e| e.kind == EnemyKind::QueenBee)
                                && c.inv.selected_stack().is_some_and(|s| s.id == item)
                            {
                                c.inv.take_one_selected();
                                boss_summons.push((
                                    EnemyKind::QueenBee,
                                    [p.body.pos[0] + 10.0, p.body.pos[1] + 8.0, p.body.pos[2]],
                                ));
                            }
                        }
                        UseEvent::Consume { item }
                            if matches!(
                                item,
                                BUNNY_ITEM | BIRD_ITEM | SQUIRREL_ITEM | CAVE_CRITTER_ITEM
                            ) =>
                        {
                            if c.inv.selected_stack().is_some_and(|s| s.id == item) {
                                c.inv.take_one_selected();
                                let kind = match item {
                                    BUNNY_ITEM => EnemyKind::Bunny,
                                    BIRD_ITEM => EnemyKind::Bird,
                                    SQUIRREL_ITEM => EnemyKind::Squirrel,
                                    _ => EnemyKind::CaveCritter,
                                };
                                let dir = [-sy * cp, sp, -cy * cp];
                                let spawn_pos = [
                                    p.body.pos[0] + dir[0] * 1.5,
                                    p.body.pos[1] + 0.25,
                                    p.body.pos[2] + dir[2] * 1.5,
                                ];
                                critter_spawns.push((kind, spawn_pos));
                            }
                        }
                        UseEvent::Consume { item }
                            if item == terravibe_sim::item::RECALL_POTION =>
                        {
                            if apply_consumable(item, &mut p.vitals, &mut c.inv)
                                || (c.inv.selected_stack().is_some_and(|s| s.id == item))
                            {
                                c.inv.take_one_selected();
                                p.body.pos = self.spawn;
                                p.body.vel = [0.0; 3];
                                p.body.fall_peak_y = self.spawn[1];
                            }
                        }
                        UseEvent::Consume { item } => {
                            apply_consumable(item, &mut p.vitals, &mut c.inv);
                        }
                        UseEvent::ShadowOrbBroken { pos } => {
                            broken_orbs.push(pos);
                            uses.push(UseEvent::ShadowOrbBroken { pos });
                        }
                        UseEvent::TileBroken { pos, tile } => {
                            if tile == terravibe_sim::block::POT {
                                broken_pots.push(pos);
                            } else if tile == terravibe_sim::block::BEE_LARVA
                                && !self.enemies.iter().any(|e| e.kind == EnemyKind::QueenBee)
                            {
                                boss_summons.push((
                                    EnemyKind::QueenBee,
                                    [
                                        pos[0] as f32 + 0.5,
                                        pos[1] as f32 + 3.0,
                                        pos[2] as f32 + 0.5,
                                    ],
                                ));
                            } else if matches!(
                                tile,
                                terravibe_sim::block::MUD | terravibe_sim::block::JUNGLE_GRASS
                            ) && pos[1] < 245
                                && terravibe_sim::worldgen::biome::dominant(
                                    &terravibe_sim::worldgen::biome::biome_weights_for_size(
                                        self.seed,
                                        pos[0],
                                        pos[2],
                                        self.cfg.size,
                                    ),
                                ) == terravibe_sim::worldgen::biome::Biome::Jungle
                                && self.rng.below(8) == 0
                            {
                                new_drops.push((
                                    ItemStack {
                                        id: terravibe_sim::item::JUNGLE_SPORES,
                                        count: 1,
                                        prefix: Prefix(0),
                                    },
                                    [
                                        pos[0] as f32 + 0.5,
                                        pos[1] as f32 + 0.5,
                                        pos[2] as f32 + 0.5,
                                    ],
                                ));
                            }
                            uses.push(UseEvent::TileBroken { pos, tile });
                        }
                        u => uses.push(u),
                    }
                }
            }
            // Melee swings hit enemies in the view cone when the swing starts,
            // or spawn projectiles for boomerang and spear.
            let eye = [p.body.pos[0], p.body.pos[1] + EYE_HEIGHT, p.body.pos[2]];
            for (item, prefix, yaw, pitch) in swings {
                let ps = prefix_stats(prefix);
                if item == terravibe_sim::item::MAGIC_MIRROR {
                    p.body.pos = self.spawn;
                    p.body.vel = [0.0; 3];
                    p.body.fall_peak_y = self.spawn[1];
                    continue;
                }
                if item == terravibe_sim::item::STARFURY && self.projectiles.len() < MAX_PROJECTILES
                {
                    let (sy, cy) = yaw.sin_cos();
                    let (sp, cp) = pitch.sin_cos();
                    let dir = [-sy * cp, sp, -cy * cp];
                    let target_pos = [
                        eye[0] + dir[0] * 10.0,
                        eye[1] + dir[1] * 10.0,
                        eye[2] + dir[2] * 10.0,
                    ];
                    let star_id = self.next_proj_id;
                    self.next_proj_id = self.next_proj_id.wrapping_add(1);
                    let star_dmg =
                        (36.0 * ps.damage * p.melee_damage_mult()).round_ties_even() as i32;
                    let star_crit = item_data(item).crit + ps.crit + p.accessory_crit_bonus();
                    self.projectiles.push(Projectile::spawn_starfury_star(
                        star_id,
                        c.id,
                        star_dmg,
                        item_data(item).knock_back * ps.knock_back,
                        star_crit,
                        target_pos,
                    ));
                }
                if item == BUG_NET {
                    const BUG_NET_REACH: f32 = 3.8;
                    let mut best: Option<(usize, f32)> = None;
                    for (idx, e) in self.enemies.iter().enumerate() {
                        if e.hp <= 0 || !e.kind.is_critter() {
                            continue;
                        }
                        let st = e.kind.stats();
                        let min = [e.pos[0] - st.half[0], e.pos[1], e.pos[2] - st.half[0]];
                        let max = [
                            e.pos[0] + st.half[0],
                            e.pos[1] + 2.0 * st.half[1],
                            e.pos[2] + st.half[0],
                        ];
                        let closest = [
                            eye[0].clamp(min[0], max[0]),
                            eye[1].clamp(min[1], max[1]),
                            eye[2].clamp(min[2], max[2]),
                        ];
                        let dx = closest[0] - eye[0];
                        let dy = closest[1] - eye[1];
                        let dz = closest[2] - eye[2];
                        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                        if dist <= BUG_NET_REACH && best.is_none_or(|(_, bd)| dist < bd) {
                            best = Some((idx, dist));
                        }
                    }
                    if let Some((idx, _)) = best {
                        let critter_item = match self.enemies[idx].kind {
                            EnemyKind::Bunny => BUNNY_ITEM,
                            EnemyKind::Bird => BIRD_ITEM,
                            EnemyKind::Squirrel => SQUIRREL_ITEM,
                            _ => CAVE_CRITTER_ITEM,
                        };
                        let stack = ItemStack {
                            id: critter_item,
                            count: 1,
                            prefix: Prefix(0),
                        };
                        if c.inv.add(stack).is_none() {
                            self.enemies.swap_remove(idx);
                        }
                    }
                    continue;
                }
                let d = item_data(item);
                if !matches!(d.class, ItemClass::Melee | ItemClass::Tool) || d.damage <= 0 {
                    continue;
                }
                // Both boomerangs spawn a returning projectile; the Flamarang
                // (Molten tier) shares the flight but pierces enemies.
                if matches!(
                    item,
                    ENCHANTED_BOOMERANG | FLAMARANG | terravibe_sim::item::THORN_CHAKRAM
                ) {
                    if self.projectiles.len() < MAX_PROJECTILES
                        && !self.projectiles.iter().any(|pr| {
                            pr.owner == c.id
                                && matches!(
                                    pr.kind,
                                    ProjKind::Boomerang
                                        | ProjKind::Flamarang
                                        | ProjKind::ThornChakram
                                )
                        })
                    {
                        let (sy, cy) = yaw.sin_cos();
                        let (sp, cp) = pitch.sin_cos();
                        let dir = [-sy * cp, sp, -cy * cp];
                        if let Some(mut proj) =
                            Projectile::throw_boomerang(self.next_proj_id, c.id, item, eye, dir)
                        {
                            proj.damage = (proj.damage as f32 * ps.damage * p.melee_damage_mult())
                                .round_ties_even() as i32;
                            proj.knock_back *= ps.knock_back;
                            proj.crit += ps.crit + p.melee_crit_bonus();
                            proj.armor_penetration = p.armor_penetration();
                            for v in &mut proj.vel {
                                *v *= ps.velocity;
                            }
                            self.next_proj_id = self.next_proj_id.wrapping_add(1);
                            self.projectiles.push(proj);
                        }
                    }
                    continue;
                }
                if item == SPEAR || item == terravibe_sim::item::DARK_LANCE {
                    if self.projectiles.len() < MAX_PROJECTILES
                        && !self
                            .projectiles
                            .iter()
                            .any(|pr| pr.owner == c.id && pr.kind == ProjKind::Spear)
                    {
                        let (sy, cy) = yaw.sin_cos();
                        let (sp, cp) = pitch.sin_cos();
                        let dir = [-sy * cp, sp, -cy * cp];
                        if let Some(mut proj) =
                            Projectile::thrust_spear(self.next_proj_id, c.id, item, eye, dir)
                        {
                            proj.damage = (proj.damage as f32 * ps.damage * p.melee_damage_mult())
                                .round_ties_even() as i32;
                            proj.knock_back *= ps.knock_back;
                            proj.crit += ps.crit + p.melee_crit_bonus();
                            proj.armor_penetration = p.armor_penetration();
                            self.next_proj_id = self.next_proj_id.wrapping_add(1);
                            self.projectiles.push(proj);
                        }
                    }
                    continue;
                }
                if matches!(
                    item,
                    terravibe_sim::item::BALL_O_HURT
                        | terravibe_sim::item::BLUE_MOON
                        | terravibe_sim::item::SUNFURY
                        | terravibe_sim::item::VALOR
                        | terravibe_sim::item::AMAZON
                ) {
                    if self.projectiles.len() < MAX_PROJECTILES
                        && !self.projectiles.iter().any(|pr| {
                            pr.owner == c.id && matches!(pr.kind, ProjKind::Flail | ProjKind::Yoyo)
                        })
                    {
                        let (sy, cy) = yaw.sin_cos();
                        let (sp, cp) = pitch.sin_cos();
                        let dir = [-sy * cp, sp, -cy * cp];
                        if let Some(mut proj) =
                            Projectile::throw_flail(self.next_proj_id, c.id, item, eye, dir)
                        {
                            proj.damage = (proj.damage as f32 * ps.damage * p.melee_damage_mult())
                                .round_ties_even() as i32;
                            proj.knock_back *= ps.knock_back;
                            proj.crit += ps.crit + p.melee_crit_bonus();
                            proj.armor_penetration = p.armor_penetration();
                            for v in &mut proj.vel {
                                *v *= ps.velocity;
                            }
                            self.next_proj_id = self.next_proj_id.wrapping_add(1);
                            self.projectiles.push(proj);
                        }
                    }
                    continue;
                }
                let (melee_damage, melee_crit) = if item == terravibe_sim::item::FIRECRACKER {
                    ((d.damage as f32 * ps.damage).round_ties_even() as i32, 0)
                } else {
                    (
                        (d.damage as f32 * ps.damage * p.melee_damage_mult()).round_ties_even()
                            as i32,
                        d.crit + ps.crit + p.melee_crit_bonus(),
                    )
                };
                let mut exploded = false;
                // Prefix size scales melee reach; the Firecracker keeps its 2.75x.
                let reach = melee_reach(item)
                    * if item == terravibe_sim::item::FIRECRACKER {
                        1.0
                    } else {
                        ps.scale
                    };
                let cone = swing_hits_with_reach(eye, yaw, pitch, reach, &self.enemies);
                // The primary target is the cone hit whose centre is nearest the
                // eye (swing_hits is not distance-sorted); ties break on the id.
                let primary = cone
                    .iter()
                    .filter_map(|id| {
                        let e = self.enemies.iter().find(|e| e.id == *id)?;
                        let c = [e.pos[0], e.pos[1] + e.kind.stats().half[1], e.pos[2]];
                        let d = [c[0] - eye[0], c[1] - eye[1], c[2] - eye[2]];
                        Some((d[0] * d[0] + d[1] * d[1] + d[2] * d[2], *id))
                    })
                    .min_by(|(da, ia), (db, ib)| da.total_cmp(db).then(ia.0.cmp(&ib.0)))
                    .map(|(_, id)| id);
                let primary_center = primary.and_then(|pid| {
                    self.enemies
                        .iter()
                        .find(|e| e.id == pid)
                        .map(|e| [e.pos[0], e.pos[1] + e.kind.stats().half[1], e.pos[2]])
                });
                let direct_hits: Vec<EnemyId> = if item == terravibe_sim::item::FIRECRACKER {
                    primary.into_iter().collect()
                } else {
                    cone.clone()
                };
                for id in direct_hits.iter().copied() {
                    let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) else {
                        continue;
                    };
                    let e_def = e.kind.stats().defense;
                    let pen = p.armor_penetration();
                    let pen_bonus = if pen > 0 {
                        e_def / 2 - (e_def - pen).max(0) / 2
                    } else {
                        0
                    };
                    let swing_dmg = if item == terravibe_sim::item::BREAKER_BLADE
                        && e.hp * 10 >= e.kind.stats().hp * 9
                    {
                        (melee_damage as f32 * 2.5).round_ties_even() as i32 + pen_bonus
                    } else {
                        melee_damage + pen_bonus
                    };
                    let Some(hit) = apply_hit(
                        e,
                        p.body.pos,
                        swing_dmg,
                        d.knock_back * ps.knock_back,
                        melee_crit,
                        &mut self.rng,
                    ) else {
                        continue;
                    };
                    let burn_chance = match item {
                        FIERY_GREATSWORD | terravibe_sim::item::FIRECRACKER => 50,
                        terravibe_sim::item::BLADE_OF_GRASS => 25,
                        terravibe_sim::item::MOLTEN_HAMAXE => 20,
                        terravibe_sim::item::MOLTEN_PICKAXE => 10,
                        _ => 0,
                    };
                    if burn_chance > 0 && self.rng.below(100) < burn_chance {
                        e.apply_on_fire(ON_FIRE_TICKS);
                    }
                    let hit_pos = [e.pos[0], e.pos[1] + e.kind.stats().half[1], e.pos[2]];
                    let target_kind = if e.kind.is_boss() {
                        HitTargetKind::Boss
                    } else {
                        HitTargetKind::Enemy
                    };
                    melee_combat_hits.push(ServerMsg::CombatHit {
                        pos: hit_pos,
                        attacker_pos: Some(p.body.pos),
                        amount: hit.damage,
                        crit: hit.crit,
                        damage_kind: DamageKind::Melee,
                        target_kind,
                        killed: hit.killed,
                    });
                    // Fiery Greatsword & Firecracker: a successful hit on the primary
                    // target explodes — 75% of its damage to up to two other enemies
                    // within 5 blocks that the direct strike did not already hit,
                    // defence applies and they always burn.
                    if (item == FIERY_GREATSWORD || item == terravibe_sim::item::FIRECRACKER)
                        && !exploded
                        && Some(id) == primary
                    {
                        exploded = true;
                        if let Some(primary_center) = primary_center {
                            let splash = (hit.damage as f32 * 0.75) as i32;
                            let targets: Vec<EnemyId> = self
                                .enemies
                                .iter()
                                .filter(|o| {
                                    !direct_hits.contains(&o.id)
                                        && o.hp > 0
                                        && !o.kind.is_friendly()
                                })
                                .filter(|o| {
                                    let c = [o.pos[0], o.pos[1] + o.kind.stats().half[1], o.pos[2]];
                                    let dx = c[0] - primary_center[0];
                                    let dy = c[1] - primary_center[1];
                                    let dz = c[2] - primary_center[2];
                                    (dx * dx + dy * dy + dz * dz).sqrt() <= 5.0
                                })
                                .take(2)
                                .map(|o| o.id)
                                .collect();
                            for tid in targets {
                                let Some(o) = self.enemies.iter_mut().find(|o| o.id == tid) else {
                                    continue;
                                };
                                let Some(oh) = apply_hit(
                                    o,
                                    p.body.pos,
                                    splash,
                                    d.knock_back,
                                    0,
                                    &mut self.rng,
                                ) else {
                                    continue;
                                };
                                o.apply_on_fire(ON_FIRE_TICKS);
                                let hit_pos =
                                    [o.pos[0], o.pos[1] + o.kind.stats().half[1], o.pos[2]];
                                melee_combat_hits.push(ServerMsg::CombatHit {
                                    pos: hit_pos,
                                    attacker_pos: Some(p.body.pos),
                                    amount: oh.damage,
                                    crit: oh.crit,
                                    damage_kind: DamageKind::Melee,
                                    target_kind: if o.kind.is_boss() {
                                        HitTargetKind::Boss
                                    } else {
                                        HitTargetKind::Enemy
                                    },
                                    killed: oh.killed,
                                });
                            }
                        }
                    }
                }
            }
            c.interest.centre = Some(p.body.pos);
            c.last_ack = last_seq;
            c.sent_hp = Some((
                p.vitals.life,
                p.vitals.life_max,
                p.vitals.breath,
                p.dead,
                p.vitals.mana,
                p.vitals.mana_max,
            ));
            for e in &events {
                if let PlayerEvent::Died { cause } = *e {
                    player_deaths.push((cid, cause));
                }
                if let PlayerEvent::Respawned = *e {
                    c.spawn_tick = tick;
                }
            }
            let mut ok =
                c.io.send(ServerMsg::PlayerState {
                    ack: last_seq,
                    body: p.body.clone(),
                    vitals: p.vitals.clone(),
                    dead: p.dead,
                    respawn_ticks: p.respawn_ticks,
                })
                .is_ok();
            for e in events {
                ok &= c.io.send(ServerMsg::Event(e)).is_ok();
            }
            for u in uses {
                ok &= c.io.send(ServerMsg::Use(u)).is_ok();
            }
            c.dead |= !ok;
        }
        for (cid, cause) in player_deaths {
            self.on_player_killed(cid, cause);
        }
        for msg in melee_combat_hits {
            self.send_all_players(msg);
        }
        // Crafting runs after input replay so a workbench placed this tick
        // counts, and independent of whether the connection sent input.
        for c in &mut self.conns {
            let crafts = std::mem::take(&mut c.crafts);
            // No player (before Hello) or dead: no state change, no result.
            let Some(feet) = c.player.as_ref().filter(|p| !p.dead).map(|p| p.body.pos) else {
                continue;
            };
            for (id, count) in crafts {
                let res = match recipe(id) {
                    None => Err(CraftError::UnknownRecipe),
                    Some(r)
                        if r.station.is_some_and(|st| {
                            !station_in_reach(&WorldQ(&self.world), feet, st)
                        }) =>
                    {
                        Err(CraftError::MissingStation)
                    }
                    Some(_) => craft_with_rng(&mut c.inv, id, count, &mut c.rng),
                };
                let (crafted, error) = match res {
                    Ok(n) => (n, None),
                    Err(e) => (0, Some(e)),
                };
                c.dead |=
                    c.io.send(ServerMsg::CraftResult {
                        recipe: id,
                        crafted,
                        error,
                    })
                    .is_err();
            }
        }
        self.process_tile_uses(&mut edits);
        self.process_chests();
        self.process_slot_reqs();
        self.process_town_reqs();
        for c in &mut self.conns {
            if let Some(p) = c.player.as_ref().filter(|p| c.sent_armor != Some(p.armor)) {
                c.dead |= c.io.send(ServerMsg::Armor { pieces: p.armor }).is_err();
                c.sent_armor = Some(p.armor);
            }
            if let Some(p) = c
                .player
                .as_ref()
                .filter(|p| c.sent_accessories != Some((p.accessories, p.accessory_prefixes)))
            {
                c.dead |=
                    c.io.send(ServerMsg::Accessories {
                        pieces: p.accessories,
                        prefixes: p.accessory_prefixes,
                    })
                    .is_err();
                c.sent_accessories = Some((p.accessories, p.accessory_prefixes));
            }
            if let Some(p) = c.player.as_ref().filter(|p| {
                c.sent_vanity
                    != Some((
                        p.vanity_armor,
                        p.vanity_accessories,
                        p.vanity_accessory_prefixes,
                    ))
            }) {
                c.dead |=
                    c.io.send(ServerMsg::Vanity {
                        armor: p.vanity_armor,
                        accessories: p.vanity_accessories,
                        accessory_prefixes: p.vanity_accessory_prefixes,
                    })
                    .is_err();
                c.sent_vanity = Some((
                    p.vanity_armor,
                    p.vanity_accessories,
                    p.vanity_accessory_prefixes,
                ));
            }
            if c.player.is_some() && c.sent_inv.as_ref() != Some(&c.inv) {
                c.dead |= c.io.send(ServerMsg::Inventory(c.inv.clone())).is_err();
                c.sent_inv = Some(c.inv.clone());
            }
            if !c.character_facts.has_ore && Self::inventory_has_ore(&c.inv, &c.held) {
                c.character_facts.has_ore = true;
                if let Some(ch) = c.character.as_mut() {
                    ch.facts.has_ore = true;
                }
            }
            if !c.character_facts.has_shelter
                && (!self.houses.is_empty() || self.facts.merchant_housed)
            {
                c.character_facts.has_shelter = true;
                if let Some(ch) = c.character.as_mut() {
                    ch.facts.has_shelter = true;
                }
            }
            let char_facts = c.character_facts;
            let current_facts = (self.facts, char_facts);
            if c.player.is_some() && c.sent_facts != Some(current_facts) {
                c.dead |=
                    c.io.send(ServerMsg::SyncFacts {
                        world: self.facts,
                        character: char_facts,
                    })
                    .is_err();
                c.sent_facts = Some(current_facts);
            }
        }
        if start_goblin_army && self.goblin_army_remaining == 0 {
            self.goblin_army_remaining = 80;
        }
        for (kind, pos) in boss_summons {
            if !self.enemies.iter().any(|e| e.kind == kind) {
                self.add_enemy(kind, pos);
            }
        }
        for (pos, block) in edits {
            let cp = ChunkPos::of_block(pos[0], pos[1], pos[2]);
            // EditQ only logs successful writes, so this chunk has real
            // player changes and must outlive its streaming interest.
            self.edited.insert(cp);
            self.dirty.insert(cp);
            self.liquid_sim.wake(pos);
            for c in &mut self.conns {
                if c.interest.sent.contains(&cp) {
                    c.dead |= c.io.send(ServerMsg::BlockChanged { pos, block }).is_err();
                }
            }
        }
        for pos in liquid_edits {
            let cp = ChunkPos::of_block(pos[0], pos[1], pos[2]);
            self.edited.insert(cp);
            self.dirty.insert(cp);
            self.liquid_sim.wake(pos);
            let lx = pos[0].rem_euclid(CHUNK_SIZE) as usize;
            let ly = pos[1].rem_euclid(CHUNK_SIZE) as usize;
            let lz = pos[2].rem_euclid(CHUNK_SIZE) as usize;
            let idx = (lx + ly * 32 + lz * 1024) as u16;
            let packed = Liquid::pack(self.world.get(&cp).and_then(|c| c.liquid(lx, ly, lz)));
            let msg = ServerMsg::LiquidDelta {
                pos: cp,
                cells: vec![(idx, packed)],
            };
            for c in &mut self.conns {
                if c.interest.sent.contains(&cp) {
                    c.dead |= c.io.send(msg.clone()).is_err();
                }
            }
        }
        self.tick_liquids(tick);
        for (pos, on) in std::mem::take(&mut self.wall_edits) {
            let cp = ChunkPos::of_block(pos[0], pos[1], pos[2]);
            self.edited.insert(cp);
            self.dirty.insert(cp);
            for c in &mut self.conns {
                if c.interest.sent.contains(&cp) {
                    c.dead |= c.io.send(ServerMsg::WallChanged { pos, on }).is_err();
                }
            }
        }
        for (stack, pos) in new_drops {
            self.add_drop(stack, pos);
        }
        for (kind, pos) in critter_spawns {
            self.add_enemy(kind, pos);
        }
        for pos in broken_orbs {
            self.handle_shadow_orb_broken(pos);
        }
        for pos in broken_pots {
            self.handle_pot_broken(pos);
        }
        for (owner, bow, ammo, prefix, origin, dir, ranged_mult, ranged_crit, armor_pen) in shots {
            if self.projectiles.len() < MAX_PROJECTILES {
                let id = self.next_proj_id;
                if let Some(mut p) = Projectile::fire(id, owner, bow, ammo, prefix, origin, dir) {
                    p.damage = (p.damage as f32 * ranged_mult).round_ties_even() as i32;
                    p.crit += ranged_crit;
                    p.armor_penetration = armor_pen;
                    self.next_proj_id = self.next_proj_id.wrapping_add(1);
                    self.projectiles.push(p);
                }
            }
        }
        for (owner, wand, prefix, origin, dir) in casts {
            if self.projectiles.len() < MAX_PROJECTILES {
                let id = self.next_proj_id;
                if let Some(conn) = self.conns.iter_mut().find(|c| c.id == owner) {
                    if let Some(player) = conn.player.as_mut() {
                        let magic_mult = player.magic_damage_mult();
                        let magic_crit = player.magic_crit_bonus();
                        let armor_pen = player.armor_penetration();
                        let base_eff = player.effective_mana_cost(wand);
                        let eff_mana = if base_eff == 0 {
                            0
                        } else {
                            terravibe_sim::item::mana_cost(base_eff, prefix)
                        };
                        let proj = if wand == terravibe_sim::item::VILETHORN {
                            Projectile::cast_vilethorn(
                                id,
                                owner,
                                wand,
                                prefix,
                                origin,
                                dir,
                                &mut player.vitals,
                            )
                        } else if wand == terravibe_sim::item::MAGIC_MISSILE {
                            Projectile::cast_magic_missile(
                                id,
                                owner,
                                wand,
                                prefix,
                                origin,
                                dir,
                                &mut player.vitals,
                            )
                        } else if wand == terravibe_sim::item::AQUA_SCEPTER {
                            Projectile::cast_aqua_scepter(
                                id,
                                owner,
                                wand,
                                prefix,
                                origin,
                                dir,
                                &mut player.vitals,
                            )
                        } else if wand == terravibe_sim::item::LASER_RIFLE {
                            Projectile::cast_laser(
                                id,
                                owner,
                                wand,
                                prefix,
                                origin,
                                dir,
                                &mut player.vitals,
                            )
                        } else if wand == terravibe_sim::item::WATER_BOLT {
                            Projectile::cast_water_bolt(
                                id,
                                owner,
                                wand,
                                prefix,
                                origin,
                                dir,
                                &mut player.vitals,
                                Some(eff_mana),
                            )
                        } else if wand == terravibe_sim::item::SPACE_GUN {
                            Projectile::cast_space_gun(
                                id,
                                owner,
                                wand,
                                prefix,
                                origin,
                                dir,
                                &mut player.vitals,
                                Some(eff_mana),
                            )
                        } else if wand == terravibe_sim::item::FLAMELASH {
                            Projectile::cast_flamelash(
                                id,
                                owner,
                                wand,
                                prefix,
                                origin,
                                dir,
                                &mut player.vitals,
                                Some(eff_mana),
                            )
                        } else if wand == terravibe_sim::item::FLOWER_OF_FIRE {
                            Projectile::cast_flower_of_fire(
                                id,
                                owner,
                                wand,
                                prefix,
                                origin,
                                dir,
                                &mut player.vitals,
                                Some(eff_mana),
                            )
                        } else if wand == terravibe_sim::item::BEE_GUN {
                            Projectile::cast_bee_gun(
                                id,
                                owner,
                                wand,
                                prefix,
                                origin,
                                dir,
                                &mut player.vitals,
                                Some(eff_mana),
                            )
                        } else {
                            Projectile::cast_spark(
                                id,
                                owner,
                                wand,
                                prefix,
                                origin,
                                dir,
                                &mut player.vitals,
                            )
                        };
                        if let Some(mut p) = proj {
                            p.damage = (p.damage as f32 * magic_mult).round_ties_even() as i32;
                            p.crit += magic_crit;
                            p.armor_penetration = armor_pen;
                            self.next_proj_id = self.next_proj_id.wrapping_add(1);
                            self.projectiles.push(p);
                        }
                    }
                }
            }
        }
        for (owner, item, origin, dir) in throws {
            if self.projectiles.len() < MAX_PROJECTILES {
                let id = self.next_proj_id;
                self.next_proj_id = self.next_proj_id.wrapping_add(1);
                self.projectiles
                    .push(Projectile::throw_explosive(id, owner, item, origin, dir));
            }
        }
        for (owner, origin, dir) in grapples {
            self.projectiles
                .retain(|pr| !(pr.owner == owner && pr.kind == ProjKind::GrappleHook));
            if self.projectiles.len() < MAX_PROJECTILES {
                let id = self.next_proj_id;
                self.next_proj_id = self.next_proj_id.wrapping_add(1);
                self.projectiles
                    .push(Projectile::fire_grapple(id, owner, origin, dir));
            }
        }
        if tick % 30 == 0 {
            let wq = WorldQ(&self.world);
            for c in &mut self.conns {
                let Some(p) = c.player.as_mut().filter(|p| !p.dead) else {
                    continue;
                };
                let px = p.body.pos[0].floor() as i32;
                let py = p.body.pos[1].floor() as i32;
                let pz = p.body.pos[2].floor() as i32;
                let mut has_fire = false;
                let mut has_lantern = false;
                for dy in -6..=6 {
                    for dz in -12..=12 {
                        for dx in -12..=12 {
                            let b = wq.block(px + dx, py + dy, pz + dz);
                            if b == terravibe_sim::block::CAMPFIRE {
                                has_fire = true;
                            } else if b == terravibe_sim::block::HEART_LANTERN {
                                has_lantern = true;
                            }
                            if has_fire && has_lantern {
                                break;
                            }
                        }
                        if has_fire && has_lantern {
                            break;
                        }
                    }
                    if has_fire && has_lantern {
                        break;
                    }
                }
                p.vitals.campfire_active = has_fire;
                p.vitals.heart_lantern_active = has_lantern;
            }
        }
        for at in summons {
            self.spawn_boss(boss::spawn_pos(at));
        }
        for at in eater_summons {
            self.spawn_eater_of_worlds(at);
        }
        self.tick_projectiles();
        self.tick_stars();
        self.tick_boss();
        self.tick_wof();
        self.tick_enemies();
        self.tick_drops();
        self.sync_player_state();
        self.broadcast_remote_players(tick);
        self.reap();

        self.stream_chunks();
        self.save_ticks += 1;
        if self.save_ticks >= AUTOSAVE_TICKS {
            self.autosave();
        }
    }

    /// Door toggles. Reach, block type and "nobody standing in the doorway" are
    /// checked in `interact::toggle_door`; edits go through the normal log so
    /// they are broadcast, marked dirty and saved like any other block change.
    fn process_tile_uses(&mut self, edits: &mut Vec<([i32; 3], BlockId)>) {
        for i in 0..self.conns.len() {
            let uses = std::mem::take(&mut self.conns[i].tile_uses);
            let Some(user) = self.conns[i]
                .player
                .as_ref()
                .filter(|p| !p.dead)
                .map(|p| p.body.clone())
            else {
                continue;
            };
            for pos in uses {
                let bodies: Vec<&terravibe_sim::PlayerBody> = self
                    .conns
                    .iter()
                    .filter_map(|c| c.player.as_ref().filter(|p| !p.dead).map(|p| &p.body))
                    .collect();
                let mut w = EditQ {
                    world: &mut self.world,
                    log: edits,
                    chests: &mut self.chests,
                    wall_log: &mut self.wall_edits,
                    liquid_log: &mut Vec::new(),
                };
                terravibe_sim::interact::toggle_door(&mut w, &user, &bodies, pos);
            }
            if let Some(pos) = self.conns[i].house_query.take()
                && terravibe_sim::interact::in_reach(&user, pos)
            {
                self.room_checks += 1;
                let r = terravibe_sim::housing::check_room(&WorldQ(&self.world), pos);
                let (ok, reason) = terravibe_sim::housing::status(&r);
                let c = &mut self.conns[i];
                c.dead |= c.io.send(ServerMsg::HouseStatus { ok, reason }).is_err();
            }
            let has_hammer = self.conns[i]
                .inv
                .selected_stack()
                .is_some_and(|s| item_data(s.id).hammer > 0);
            for pos in std::mem::take(&mut self.conns[i].wall_breaks) {
                if !has_hammer {
                    continue;
                }
                let mut w = EditQ {
                    world: &mut self.world,
                    log: edits,
                    chests: &mut self.chests,
                    wall_log: &mut self.wall_edits,
                    liquid_log: &mut Vec::new(),
                };
                if let Some(item) = terravibe_sim::interact::break_wall(&mut w, &user, pos) {
                    let stack = terravibe_sim::inventory::ItemStack {
                        id: item,
                        count: 1,
                        prefix: terravibe_sim::item::Prefix(0),
                    };
                    self.add_drop(
                        stack,
                        [
                            pos[0] as f32 + 0.5,
                            pos[1] as f32 + 0.5,
                            pos[2] as f32 + 0.5,
                        ],
                    );
                }
            }
        }
    }

    /// Chest open/close/move requests, plus closing chests that were broken
    /// or walked away from. Reach, block type and exclusivity are checked here.
    fn process_chests(&mut self) {
        for i in 0..self.conns.len() {
            let reqs = std::mem::take(&mut self.conns[i].chest_reqs);
            let taken: HashSet<[i32; 3]> = self
                .conns
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .filter_map(|(_, c)| c.open_chest)
                .collect();
            let world = WorldQ(&self.world);
            let chests = &mut self.chests;
            let locked_chests = &mut self.locked_chests;
            let shadow_chests = &mut self.shadow_chests;
            let c = &mut self.conns[i];
            let reach = |c: &Conn, pos: [i32; 3]| {
                c.player.as_ref().is_some_and(|p| {
                    !p.dead
                        && world.block(pos[0], pos[1], pos[2]) == CHEST
                        && terravibe_sim::interact::in_reach(&p.body, pos)
                })
            };
            let mut ok = true;
            let mut send = |c: &Conn, m: ServerMsg| ok &= c.io.send(m).is_ok();
            for r in reqs {
                match r {
                    ClientMsg::OpenChest { pos } => {
                        if c.open_chest.take().is_some() {
                            send(c, ServerMsg::ChestClosed);
                        }
                        if reach(c, pos) && !taken.contains(&pos) {
                            if shadow_chests.contains(&pos) {
                                if terravibe_sim::interact::has_shadow_key(&c.inv) {
                                    shadow_chests.remove(&pos);
                                    c.open_chest = Some(pos);
                                    let slots = chests.entry(pos).or_default().slots.to_vec();
                                    send(c, ServerMsg::ChestContents { pos, slots });
                                } else {
                                    send(c, ServerMsg::ChestClosed);
                                    send(
                                        c,
                                        ServerMsg::Dialogue {
                                            npc: EnemyId(0),
                                            text: "Locked Shadow Chest: Requires a Shadow Key from the Dungeon.".to_string(),
                                        },
                                    );
                                }
                            } else if locked_chests.contains(&pos) {
                                match terravibe_sim::interact::try_unlock_chest(&mut c.inv, true) {
                                    Ok(true) => {
                                        locked_chests.remove(&pos);
                                        send(c, ServerMsg::Inventory(c.inv.clone()));
                                        c.open_chest = Some(pos);
                                        let slots = chests.entry(pos).or_default().slots.to_vec();
                                        send(c, ServerMsg::ChestContents { pos, slots });
                                    }
                                    _ => {
                                        send(c, ServerMsg::ChestClosed);
                                        send(
                                            c,
                                            ServerMsg::Dialogue {
                                                npc: EnemyId(0),
                                                text: "Locked Gold Chest: Requires a Golden Key."
                                                    .to_string(),
                                            },
                                        );
                                    }
                                }
                            } else {
                                c.open_chest = Some(pos);
                                let slots = chests.entry(pos).or_default().slots.to_vec();
                                send(c, ServerMsg::ChestContents { pos, slots });
                            }
                        } else {
                            send(c, ServerMsg::ChestClosed);
                        }
                    }
                    ClientMsg::CloseChest => {
                        if c.open_chest.take().is_some() {
                            send(c, ServerMsg::ChestClosed);
                        }
                    }
                    ClientMsg::ChestMove { .. } | ClientMsg::ChestQuickStack => {
                        let Some(pos) = c.open_chest.filter(|&p| reach(c, p)) else {
                            continue; // validity sweep below closes it
                        };
                        let chest = chests.entry(pos).or_default();
                        let changed = match r {
                            ClientMsg::ChestMove {
                                to_chest: true,
                                slot,
                            } => chest.move_in(&mut c.inv, slot as usize),
                            ClientMsg::ChestMove {
                                to_chest: false,
                                slot,
                            } => chest.move_out(slot as usize, &mut c.inv),
                            _ => chest.quick_stack(&mut c.inv),
                        };
                        if changed {
                            let slots = chest.slots.to_vec();
                            send(c, ServerMsg::ChestContents { pos, slots });
                        }
                    }
                    _ => {}
                }
            }
            if c.open_chest.is_some_and(|p| !reach(c, p)) {
                c.open_chest = None;
                send(c, ServerMsg::ChestClosed);
            }
            c.dead |= !ok;
        }
    }

    /// Slot clicks and inventory close. A chest slot only works while that
    /// chest is open for this player and in reach.
    fn process_slot_reqs(&mut self) {
        let mut leftovers = Vec::new();
        let mut thrown: Vec<(terravibe_sim::inventory::ItemStack, [f32; 3], [f32; 3])> = Vec::new();
        for c in &mut self.conns {
            let reqs = std::mem::take(&mut c.slot_reqs);
            if c.player.is_none() {
                continue;
            }
            for r in reqs {
                match r {
                    ClientMsg::ClickSlot { at, kind } => {
                        if at == slots::SlotRef::Trash {
                            if slots::click_trash(&mut c.trash, &mut c.held, kind) {
                                c.dead |= c.io.send(ServerMsg::Trash(c.trash)).is_err();
                                c.dead |= c.io.send(ServerMsg::Held(c.held.0)).is_err();
                            }
                            continue;
                        }
                        let p = c.player.as_mut().unwrap();
                        let chest_pos = c.open_chest.filter(|&pos| {
                            !p.dead
                                && WorldQ(&self.world).block(pos[0], pos[1], pos[2]) == CHEST
                                && terravibe_sim::interact::in_reach(&p.body, pos)
                        });
                        let chest = chest_pos.map(|pos| self.chests.entry(pos).or_default());
                        let before = chest.as_ref().map(|ch| ch.slots.to_vec());
                        let before_guide = c.guide_slot;
                        let changed = slots::click_with_guide(
                            &mut c.inv,
                            &mut p.armor,
                            &mut p.accessories,
                            &mut p.accessory_prefixes,
                            &mut p.vanity_armor,
                            &mut p.vanity_accessories,
                            &mut p.vanity_accessory_prefixes,
                            chest,
                            Some(&mut c.guide_slot),
                            &mut c.held,
                            at,
                            kind,
                        );
                        if !changed {
                            continue;
                        }
                        p.sync_accessories();
                        if let (Some(pos), Some(before)) = (chest_pos, before) {
                            let slots = self.chests[&pos].slots.to_vec();
                            if slots != before {
                                c.dead |=
                                    c.io.send(ServerMsg::ChestContents { pos, slots }).is_err();
                            }
                        }
                        if c.guide_slot != before_guide {
                            c.dead |= c.io.send(ServerMsg::GuideSlot(c.guide_slot)).is_err();
                        }
                        c.dead |= c.io.send(ServerMsg::Held(c.held.0)).is_err();
                    }
                    ClientMsg::CloseInventory => {
                        if c.held.0.is_some() {
                            leftovers.extend(return_held(c));
                            c.dead |= c.io.send(ServerMsg::Held(None)).is_err();
                        }
                        if c.guide_slot.is_some() {
                            leftovers.extend(return_guide_slot(c));
                            c.dead |= c.io.send(ServerMsg::GuideSlot(None)).is_err();
                        }
                    }
                    ClientMsg::CloseGuideCrafting => {
                        if c.guide_slot.is_some() {
                            leftovers.extend(return_guide_slot(c));
                            c.dead |= c.io.send(ServerMsg::GuideSlot(None)).is_err();
                        }
                    }
                    ClientMsg::GuideSlotShift { slot } => {
                        let idx = slot as usize;
                        if idx < terravibe_sim::inventory::BAG
                            && let Some(stack) = c.inv.slots[idx]
                            && terravibe_sim::crafting::is_material(stack.id)
                        {
                            let prev = c.guide_slot.replace(stack);
                            c.inv.slots[idx] = prev;
                            c.dead |= c.io.send(ServerMsg::GuideSlot(c.guide_slot)).is_err();
                        }
                    }
                    ClientMsg::TrashShift { slot } => {
                        let idx = slot as usize;
                        if idx < terravibe_sim::inventory::BAG
                            && let Some(stack) = c.inv.slots[idx].take()
                        {
                            c.trash = Some(stack);
                            c.dead |= c.io.send(ServerMsg::Trash(c.trash)).is_err();
                        }
                    }
                    ClientMsg::SortInventory { mode } => {
                        c.inv.sort(mode);
                    }
                    ClientMsg::DropItem { dir, from_cursor } => {
                        let p = c.player.as_ref().unwrap();
                        if p.dead {
                            continue;
                        }
                        let stack = if from_cursor {
                            let taken = c.held.0.take();
                            if taken.is_some() {
                                c.dead |= c.io.send(ServerMsg::Held(None)).is_err();
                            }
                            taken
                        } else {
                            let sel = c.inv.selected;
                            c.inv.slots[sel].take()
                        };
                        if let Some(st) = stack {
                            let l = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
                            let n = if l > 1e-4 {
                                [dir[0] / l, dir[1] / l, dir[2] / l]
                            } else {
                                [0.0, 0.0, -1.0]
                            };
                            let pos = [
                                p.body.pos[0] + n[0] * 0.8,
                                p.body.pos[1] + 1.2,
                                p.body.pos[2] + n[2] * 0.8,
                            ];
                            let vel = [n[0] * 0.35, n[1] * 0.35 + 0.12, n[2] * 0.35];
                            thrown.push((st, pos, vel));
                        }
                    }
                    ClientMsg::QuickHeal | ClientMsg::QuickMana | ClientMsg::QuickBuff => {
                        let p = c.player.as_mut().unwrap();
                        if p.dead {
                            continue;
                        }
                        let changed = match r {
                            ClientMsg::QuickHeal => {
                                terravibe_sim::interact::quick_heal(&mut p.vitals, &mut c.inv)
                            }
                            ClientMsg::QuickMana => {
                                let eff_mana = p.effective_mana_max();
                                terravibe_sim::interact::quick_mana_with_max(
                                    &mut p.vitals,
                                    &mut c.inv,
                                    eff_mana,
                                )
                            }
                            ClientMsg::QuickBuff => {
                                terravibe_sim::interact::quick_buff(&mut p.vitals, &mut c.inv)
                            }
                            _ => false,
                        };
                        if changed {
                            c.dead |=
                                c.io.send(ServerMsg::PlayerState {
                                    ack: c.last_ack,
                                    body: p.body.clone(),
                                    vitals: p.vitals.clone(),
                                    dead: p.dead,
                                    respawn_ticks: p.respawn_ticks,
                                })
                                .is_err();
                            c.sent_hp = Some((
                                p.vitals.life,
                                p.vitals.life_max,
                                p.vitals.breath,
                                p.dead,
                                p.vitals.mana,
                                p.vitals.mana_max,
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
        for (stack, pos) in leftovers {
            self.add_drop(stack, pos);
        }
        for (stack, pos, vel) in thrown {
            let id = DropId(self.next_drop_id);
            self.next_drop_id += 1;
            let mut drop = Drop::new(id, stack, pos);
            drop.vel = vel;
            drop.pickup_delay = 45;
            self.drops.push(drop);
        }
    }

    /// Puts every cursor stack and Guide lookup stack back (before saves and disconnects);
    /// what does not fit becomes a world drop at the player.
    fn return_all_held(&mut self, dead_only: bool) {
        let mut leftovers = Vec::new();
        for c in self.conns.iter_mut().filter(|c| c.dead || !dead_only) {
            if c.held.0.is_some() {
                leftovers.extend(return_held(c));
                c.dead |= c.io.send(ServerMsg::Held(None)).is_err();
            }
            if c.guide_slot.is_some() {
                leftovers.extend(return_guide_slot(c));
                c.dead |= c.io.send(ServerMsg::GuideSlot(None)).is_err();
            }
        }
        for (stack, pos) in leftovers {
            self.add_drop(stack, pos);
        }
    }

    fn broadcast_to_joined(&mut self, msg: ServerMsg) {
        for c in &mut self.conns {
            if !c.dead && c.player.is_some() {
                c.dead |= c.io.send(msg.clone()).is_err();
            }
        }
    }

    fn broadcast_player_list(&mut self) {
        let players: Vec<(PlayerId, String)> = self
            .conns
            .iter()
            .filter(|c| !c.dead && c.player.is_some())
            .map(|c| (c.id, c.name.clone()))
            .collect();
        self.broadcast_to_joined(ServerMsg::PlayerList { players });
    }

    fn announce_player_joined(&mut self, joined_idx: usize) {
        let Some(c) = self
            .conns
            .get(joined_idx)
            .filter(|c| !c.dead && c.player.is_some())
        else {
            return;
        };
        let p = c.player.as_ref().unwrap();
        let j_id = c.id;
        let j_name = c.name.clone();
        let j_app = c
            .character
            .as_ref()
            .map(|ch| ch.appearance)
            .unwrap_or_default();
        let j_pos = p.body.pos;
        let j_yaw = c.yaw;
        let j_armor = p.armor;
        let j_vanity = p.vanity_armor;
        let j_held = c.inv.selected_stack().map(|s| s.id);

        self.conns[joined_idx].sent_remote_equip = Some((j_armor, j_vanity, j_held));

        let mut existing: Vec<(
            PlayerId,
            String,
            terravibe_sim::character::Appearance,
            [f32; 3],
            f32,
            [Option<ItemId>; 3],
            [Option<ItemId>; 3],
            Option<ItemId>,
        )> = Vec::new();

        for (i, other) in self.conns.iter_mut().enumerate() {
            if i == joined_idx || other.dead {
                continue;
            }
            let Some(op) = other.player.as_ref() else {
                continue;
            };
            existing.push((
                other.id,
                other.name.clone(),
                other
                    .character
                    .as_ref()
                    .map(|ch| ch.appearance)
                    .unwrap_or_default(),
                op.body.pos,
                other.yaw,
                op.armor,
                op.vanity_armor,
                other.inv.selected_stack().map(|s| s.id),
            ));
            if other.known_remotes.insert(j_id) {
                other.dead |= other
                    .io
                    .send(ServerMsg::RemotePlayerSpawn {
                        id: j_id,
                        name: j_name.clone(),
                        appearance: j_app,
                        pos: j_pos,
                        yaw: j_yaw,
                    })
                    .is_err();
                other.dead |= other
                    .io
                    .send(ServerMsg::RemotePlayerEquipment {
                        id: j_id,
                        armor: j_armor,
                        vanity_armor: j_vanity,
                        held: j_held,
                    })
                    .is_err();
            }
        }

        let joined = &mut self.conns[joined_idx];
        for (oid, oname, oapp, opos, oyaw, oarmor, ovanity, oheld) in existing {
            if joined.known_remotes.insert(oid) {
                joined.dead |= joined
                    .io
                    .send(ServerMsg::RemotePlayerSpawn {
                        id: oid,
                        name: oname,
                        appearance: oapp,
                        pos: opos,
                        yaw: oyaw,
                    })
                    .is_err();
                joined.dead |= joined
                    .io
                    .send(ServerMsg::RemotePlayerEquipment {
                        id: oid,
                        armor: oarmor,
                        vanity_armor: ovanity,
                        held: oheld,
                    })
                    .is_err();
            }
        }
    }

    fn broadcast_remote_players(&mut self, tick: u32) {
        // Broadcast equipment changes to all other joined players.
        let mut equip_updates: Vec<(
            PlayerId,
            [Option<ItemId>; 3],
            [Option<ItemId>; 3],
            Option<ItemId>,
        )> = Vec::new();
        for c in &mut self.conns {
            if c.dead {
                continue;
            }
            let Some(p) = c.player.as_ref() else {
                continue;
            };
            let equip = (
                p.armor,
                p.vanity_armor,
                c.inv.selected_stack().map(|s| s.id),
            );
            if c.sent_remote_equip != Some(equip) {
                c.sent_remote_equip = Some(equip);
                equip_updates.push((c.id, equip.0, equip.1, equip.2));
            }
        }
        for (src_id, armor, vanity_armor, held) in equip_updates {
            for dst in &mut self.conns {
                if !dst.dead && dst.player.is_some() && dst.id != src_id {
                    dst.dead |= dst
                        .io
                        .send(ServerMsg::RemotePlayerEquipment {
                            id: src_id,
                            armor,
                            vanity_armor,
                            held,
                        })
                        .is_err();
                }
            }
        }

        // 20 Hz (every 3 ticks) RemotePlayerState scoped to VIEW_RADIUS_BLOCKS.
        if tick % 3 == 0 {
            let states: Vec<(PlayerId, [f32; 3], [f32; 3], f32, f32, bool, bool)> = self
                .conns
                .iter()
                .filter(|c| !c.dead)
                .filter_map(|c| {
                    let p = c.player.as_ref()?;
                    Some((
                        c.id,
                        p.body.pos,
                        p.body.vel,
                        c.yaw,
                        c.pitch,
                        c.use_state.anim > 0,
                        p.dead,
                    ))
                })
                .collect();
            let max_dist2 = VIEW_RADIUS_BLOCKS * VIEW_RADIUS_BLOCKS;
            for (src_id, pos, vel, yaw, pitch, swinging, dead) in states {
                for dst in &mut self.conns {
                    if dst.dead || dst.id == src_id {
                        continue;
                    }
                    let Some(dst_p) = dst.player.as_ref() else {
                        continue;
                    };
                    let dx = dst_p.body.pos[0] - pos[0];
                    let dz = dst_p.body.pos[2] - pos[2];
                    if dx * dx + dz * dz <= max_dist2 {
                        dst.dead |= dst
                            .io
                            .send(ServerMsg::RemotePlayerState {
                                id: src_id,
                                pos,
                                vel,
                                yaw,
                                pitch,
                                swinging,
                                dead,
                            })
                            .is_err();
                    }
                }
            }
        }
    }

    /// Gracefully shuts down a hosted session: flushes `CharacterState` to all remaining
    /// clients, broadcasts `RejectReason::HostClosing` to guests, and snapshots players.
    pub fn shutdown_host(&mut self) {
        self.return_all_held(false);
        for c in &mut self.conns {
            flush_character_state(c);
            if !c.is_host {
                let _ = c.io.send(ServerMsg::Rejected {
                    reason: RejectReason::HostClosing,
                });
            }
            if let Some(ps) = player_save(c) {
                self.saved_players.insert(c.name.clone(), ps);
            }
        }
        self.conns.clear();
    }

    /// Drops dead connections, persisting their players.
    fn reap(&mut self) {
        self.return_all_held(true);
        let mut gone = Vec::new();
        let mut left_players: Vec<(PlayerId, String)> = Vec::new();
        for c in self.conns.iter_mut().filter(|c| c.dead) {
            flush_character_state(c);
            if c.player.is_some() {
                left_players.push((c.id, c.name.clone()));
            }
            if let Some(ps) = player_save(c) {
                gone.push((c.name.clone(), ps));
            }
        }
        self.conns.retain(|c| !c.dead);
        if !left_players.is_empty() {
            for (left_id, left_name) in left_players {
                for c in &mut self.conns {
                    if !c.dead && c.player.is_some() {
                        c.known_remotes.remove(&left_id);
                        c.dead |=
                            c.io.send(ServerMsg::RemotePlayerDespawn { id: left_id })
                                .is_err();
                        c.dead |=
                            c.io.send(ServerMsg::Chat {
                                sender: None,
                                text: format!("{left_name} left the game."),
                            })
                            .is_err();
                    }
                }
            }
            self.broadcast_player_list();
        }
        for (name, ps) in gone {
            if let Some(a) = &self.saver {
                // One small file; written off the tick thread.
                let f = a.dir().path().join(format!("players/{name}.bin"));
                let bytes = ps.to_bytes();
                std::thread::spawn(move || {
                    let _ = save::atomic_write(&f, &bytes);
                });
            }
            self.saved_players.insert(name, ps);
        }
    }

    fn snapshot(&self) -> WorldSnapshot {
        let mut players = self.saved_players.clone();
        for c in &self.conns {
            if let Some(ps) = player_save(c) {
                players.insert(c.name.clone(), ps);
            }
        }
        let mut locked_chests: Vec<[i32; 3]> = self.locked_chests.iter().copied().collect();
        locked_chests.sort_unstable();
        let mut shadow_chests: Vec<[i32; 3]> = self.shadow_chests.iter().copied().collect();
        shadow_chests.sort_unstable();
        WorldSnapshot {
            seed: self.seed,
            clock_tick: self.clock.tick,
            size: self.cfg.size,
            chunks: self
                .dirty
                .iter()
                .filter_map(|p| Some((*p, Arc::clone(self.world.get(p)?))))
                .collect(),
            players: players.into_iter().collect(),
            chests: self.chests.iter().map(|(p, c)| (*p, c.clone())).collect(),
            towns: TownSave {
                guide_spawned: self.guide_spawned,
                npcs: self
                    .houses
                    .iter()
                    .filter_map(|h| Some((h.npc?, h.home, h.origin)))
                    .collect(),
            },
            facts: self.facts.clone(),
            weather: self.weather,
            locked_chests,
            shadow_chests,
        }
    }

    /// Snapshot on this thread, write on the autosave worker; skipped when the
    /// previous write is still running.
    fn autosave(&mut self) {
        self.save_ticks = 0;
        if self.saver.is_none() {
            return;
        }
        self.return_all_held(false);
        for c in &mut self.conns {
            if let (Some(char_data), Some(pl)) = (&mut c.character, &c.player) {
                char_data.inventory = c.inv.clone();
                char_data.armor = pl.armor;
                char_data.accessories = pl.accessories;
                char_data.accessory_prefixes = pl.accessory_prefixes;
                char_data.vanity_armor = pl.vanity_armor;
                char_data.vanity_accessories = pl.vanity_accessories;
                char_data.vanity_accessory_prefixes = pl.vanity_accessory_prefixes;
                char_data.life_max = pl.vitals.life_max;
                char_data.life = pl.vitals.life;
                char_data.mana_max = pl.vitals.mana_max;
                char_data.mana = pl.vitals.mana;
                char_data.potion_delay = pl.vitals.potion_delay_ticks as i32;
                char_data.obsidian_skin_ticks = pl.vitals.obsidian_skin_ticks;
                char_data.facts = c.character_facts;
                let bytes = char_data.to_bytes();
                let _ = c.io.send(ServerMsg::CharacterState(bytes));
            }
        }
        let snap = self.snapshot();
        if self.saver.as_mut().is_some_and(|a| a.try_save(snap)) {
            self.dirty.clear();
        }
    }

    /// Writes everything now and waits for it (graceful exit, tests).
    pub fn save_now(&mut self) -> std::io::Result<()> {
        let snap = self.snapshot();
        let Some(a) = self.saver.as_mut() else {
            return Ok(());
        };
        a.flush(&snap)?;
        self.dirty.clear();
        Ok(())
    }

    fn add_enemy(&mut self, kind: EnemyKind, pos: [f32; 3]) {
        let id = EnemyId(self.next_enemy_id);
        self.next_enemy_id += 1;
        self.enemies.push(Enemy::new(id, kind, pos));
    }

    /// Spawns the Eye at `pos` (feet centre) unless one is alive.
    fn spawn_boss(&mut self, pos: [f32; 3]) -> bool {
        if self.boss.is_some() {
            return false;
        }
        let id = EnemyId(self.next_enemy_id);
        self.next_enemy_id += 1;
        self.enemies.push(boss::new_enemy(id, pos));
        self.boss = Some((id, EyeBoss::new()));
        true
    }

    pub fn spawn_skeletron(&mut self, pos: [f32; 3]) {
        let start_id = self.next_enemy_id;
        self.next_enemy_id += 3;
        let parts = terravibe_sim::enemy::spawn_skeletron(start_id, pos);
        self.enemies.extend(parts);
    }

    /// Rolls the nightly natural spawn, steps the Eye, and keeps clients informed.
    fn tick_boss(&mut self) {
        let day = self.clock.is_day();
        let town_npcs = self
            .enemies
            .iter()
            .filter(|e| {
                e.hp > 0
                    && matches!(
                        e.kind,
                        EnemyKind::Guide
                            | EnemyKind::Merchant
                            | EnemyKind::ArmsDealer
                            | EnemyKind::Nurse
                            | EnemyKind::Demolitionist
                            | EnemyKind::Dryad
                            | EnemyKind::GoblinTinkerer
                    )
            })
            .count();
        let players: Vec<([f32; 3], i32, i32)> = self
            .conns
            .iter()
            .filter_map(|c| c.player.as_ref().filter(|p| !p.dead))
            .map(|p| (p.body.pos, p.vitals.life_max, p.defense()))
            .collect();
        // Vanilla sets spawnEye once per day at dusk (gameplay spec) and spawns it after
        // night tick 4860 (gameplay spec); here the roll is this game's 1-in-3 (boss.rs).
        if self.cfg.natural_spawns {
            if self.was_day && !day {
                self.star_boost = night_setup(&mut self.rng);
                self.blood_moon =
                    players.iter().any(|&(_, lm, _)| lm > 100) && self.rng.below(9) == 0;
                self.eye_pending = !self.facts.downed_eye
                    && !self.any_boss_alive()
                    && players.iter().any(|&(_, lm, def)| {
                        boss::natural_spawn_roll_full(lm, def, town_npcs, &mut self.rng)
                    });
            }
            if !self.was_day && day {
                self.blood_moon = false;
            }
            let night_tick = self
                .clock
                .tick
                .saturating_sub(terravibe_sim::time::DAY_TICKS);
            if self.eye_pending && !day && night_tick > boss::SPAWN_AFTER_NIGHT_TICK {
                // gameplay spec: only a player above the world surface is targeted.
                let w = WorldQ(&self.world);
                let above: Vec<[f32; 3]> = players
                    .iter()
                    .filter(|&&(_, lm, _)| lm >= boss::MIN_LIFE_MAX)
                    .filter(|&&(p, _, _)| {
                        w.surface_y(p[0].floor() as i32, p[2].floor() as i32)
                            .is_some_and(|sy| p[1] >= sy as f32 - 1.0)
                    })
                    .map(|&(p, _, _)| p)
                    .collect();
                if !above.is_empty() && !self.any_boss_alive() {
                    let at = above[self.rng.below(above.len() as u32) as usize];
                    self.spawn_boss(boss::spawn_pos(at));
                    self.eye_pending = false;
                }
            }
        }
        if !self.was_day && day {
            for c in &mut self.conns {
                c.character_facts.first_night_survived = true;
                if let Some(ch) = c.character.as_mut() {
                    ch.facts.first_night_survived = true;
                }
            }
        }
        self.was_day = day;

        if self.cfg.demo_boss && self.boss.is_none() && !self.demo_done && !players.is_empty() {
            let [sx, sy, sz] = self.spawn;
            self.spawn_boss([sx, sy + 9.0, sz - 24.0]);
            self.demo_done = true;
        }
        let Some((id, mut ai)) = self.boss.take() else {
            return;
        };
        let servants_alive = self
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::ServantOfCthulhu && e.hp > 0)
            .count();
        let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) else {
            return; // killed this tick; tick_enemies clears the boss after the loot
        };
        if e.hp <= 0 {
            self.boss = Some((id, ai));
            return;
        }
        let nearest = players
            .iter()
            .map(|&(p, _, _)| p)
            .min_by(|a, b| dist2(*a, e.pos).total_cmp(&dist2(*b, e.pos)));
        let ev = match nearest {
            _ if self.cfg.demo_boss => BossEvent::None, // frozen for the screenshot
            // Aim at the body centre.
            Some(t) => ai.step(e, [t[0], t[1] + HEIGHT * 0.5, t[2]], day, servants_alive),
            None => BossEvent::Despawn,
        };
        if ev == BossEvent::Despawn {
            self.enemies.retain(|e| e.id != id);
            boss::clear_servants(&mut self.enemies);
            self.send_all_players(ServerMsg::BossGone);
            return;
        }
        let msg = ServerMsg::Boss {
            pos: e.pos,
            vel: e.vel,
            life: e.hp.max(0),
            life_max: EnemyKind::EyeOfCthulhu.stats().hp,
            phase: ai.phase(),
            state: ai.state_code(day),
            spin: ai.rotation,
        };
        self.boss = Some((id, ai));
        if let BossEvent::SpawnServant { pos, vel } = ev {
            let s_id = EnemyId(self.next_enemy_id);
            self.next_enemy_id += 1;
            let mut servant = Enemy::new(s_id, EnemyKind::ServantOfCthulhu, pos);
            servant.vel = vel;
            self.enemies.push(servant);
        }
        self.send_all_players(msg);
    }

    /// Spawns the Wall of Flesh relative to `player_pos` if none is alive.
    pub fn spawn_wof(&mut self, player_pos: [f32; 3]) -> bool {
        if self.wof.is_some() {
            return false;
        }
        let (wof, parts, next_id) =
            terravibe_sim::wof::spawn_wof(self.next_enemy_id, player_pos, &self.bounds);
        self.next_enemy_id = next_id;
        self.enemies.extend(parts);
        let msg = ServerMsg::WofState {
            wall_x: wof.wall_x,
            dir: wof.dir,
            life: wof.hp,
            life_max: terravibe_sim::wof::WOF_MAX_HP,
        };
        self.wof = Some(wof);
        self.send_all_players(msg);
        true
    }

    fn tick_wof(&mut self) {
        use terravibe_sim::wof::{self, WofEvent};
        let Some(mut wof) = self.wof.take() else {
            return;
        };
        let players: Vec<(usize, [f32; 3])> = self
            .conns
            .iter()
            .enumerate()
            .filter_map(|(cid, c)| {
                c.player
                    .as_ref()
                    .filter(|p| !p.dead)
                    .map(|p| (cid, p.body.pos))
            })
            .collect();
        if players.is_empty() {
            wof::clear_wof(&mut self.enemies);
            self.projectiles.retain(|p| p.kind != ProjKind::EyeLaser);
            self.send_all_players(ServerMsg::WofGone);
            return;
        }
        let nearest = players
            .iter()
            .filter(|(_, p)| p[1] <= wof::WOF_TONGUE_MAX_Y)
            .min_by(|a, b| {
                (a.1[0] - wof.wall_x)
                    .abs()
                    .total_cmp(&(b.1[0] - wof.wall_x).abs())
            })
            .or_else(|| {
                players.iter().min_by(|a, b| {
                    (a.1[0] - wof.wall_x)
                        .abs()
                        .total_cmp(&(b.1[0] - wof.wall_x).abs())
                })
            })
            .unwrap()
            .1;
        let evs = wof.step(&mut self.enemies, nearest, &self.bounds);
        if wof.hp <= 0 {
            self.wof = Some(wof);
            return;
        }
        if evs.contains(&WofEvent::FarEdgeCrush) {
            for (cid, _) in players {
                self.kill_player(cid, DeathCause::Contact(EnemyKind::WallOfFleshMouth));
            }
            wof::clear_wof(&mut self.enemies);
            self.projectiles.retain(|p| p.kind != ProjKind::EyeLaser);
            self.send_all_players(ServerMsg::WofGone);
            return;
        }
        for ev in evs {
            match ev {
                WofEvent::EyeLaser {
                    origin,
                    target,
                    damage,
                } => {
                    if self.projectiles.len() < MAX_PROJECTILES {
                        let v = [
                            target[0] - origin[0],
                            target[1] - origin[1],
                            target[2] - origin[2],
                        ];
                        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                        if l > 1e-4 {
                            let vel = [
                                v[0] / l * wof::WOF_EYE_LASER_SPEED,
                                v[1] / l * wof::WOF_EYE_LASER_SPEED,
                                v[2] / l * wof::WOF_EYE_LASER_SPEED,
                            ];
                            let id = self.next_proj_id;
                            self.next_proj_id = self.next_proj_id.wrapping_add(1);
                            self.projectiles.push(Projectile {
                                id,
                                pos: origin,
                                vel,
                                kind: ProjKind::EyeLaser,
                                owner: u32::MAX,
                                ttl: 180,
                                damage,
                                knock_back: 1.0,
                                crit: 0,
                                ai0: 0,
                                pierced: 0,
                                origin,
                                armor_penetration: 0,
                            });
                        }
                    }
                }
                WofEvent::SpawnLeech { pos } => {
                    self.add_enemy(EnemyKind::Leech, pos);
                }
                WofEvent::FarEdgeCrush => {}
            }
        }

        // The Tongue pull & Horrified damage when behind the wall or escaping Underworld.
        wof.tongue_tick += 1;
        let deal_tongue_dmg = wof.tongue_tick % wof::WOF_TONGUE_DAMAGE_INTERVAL == 0;
        let mut tongue_deaths = Vec::new();
        for (cid, c) in self.conns.iter_mut().enumerate() {
            let Some(p) = c.player.as_mut().filter(|p| !p.dead) else {
                continue;
            };
            if wof::should_tongue_pull(p.body.pos, wof.wall_x, wof.dir) {
                wof::step_tongue_pull(
                    &mut p.body.pos,
                    &mut p.body.vel,
                    wof.wall_x,
                    wof.wall_z,
                    wof.dir,
                );
                if deal_tongue_dmg {
                    let mut pev = Vec::new();
                    if hurt_player_with_cause(
                        p,
                        wof::WOF_TONGUE_DAMAGE,
                        DeathCause::Contact(EnemyKind::WallOfFleshMouth),
                        &mut pev,
                    ) {
                        let amount = damage_taken(wof::WOF_TONGUE_DAMAGE, p.defense());
                        c.dead |= c.io.send(ServerMsg::PlayerHit { amount }).is_err();
                        if p.dead {
                            tongue_deaths
                                .push((cid, DeathCause::Contact(EnemyKind::WallOfFleshMouth)));
                        }
                    }
                }
            }
        }
        for (cid, cause) in tongue_deaths {
            self.on_player_killed(cid, cause);
        }

        let msg = ServerMsg::WofState {
            wall_x: wof.wall_x,
            dir: wof.dir,
            life: wof.hp,
            life_max: wof::WOF_MAX_HP,
        };
        self.wof = Some(wof);
        self.send_all_players(msg);
    }

    fn send_all_players(&mut self, m: ServerMsg) {
        for c in &mut self.conns {
            if c.player.is_some() {
                c.dead |= c.io.send(m.clone()).is_err();
            }
        }
    }

    fn add_drop(&mut self, stack: terravibe_sim::inventory::ItemStack, pos: [f32; 3]) {
        let id = DropId(self.next_drop_id);
        self.next_drop_id += 1;
        self.drops.push(Drop::new(id, stack, pos));
    }

    fn on_player_killed(&mut self, cid: usize, cause: DeathCause) {
        let tick = self.clock.tick;
        let Some(c) = self.conns.get_mut(cid) else {
            return;
        };
        let Some(p) = c.player.as_ref() else {
            return;
        };
        let death_pos = p.body.pos;
        let survived_ticks = tick.saturating_sub(c.spawn_tick);

        let mut coins_lost: u32 = 0;
        let mut dropped_coins = Vec::new();
        if let Some(held) = c.held.0.as_mut() {
            if terravibe_sim::item::is_coin(held.id) && held.count > 0 {
                let drop_count = (held.count + 1) / 2;
                let remaining = held.count - drop_count;
                coins_lost = coins_lost
                    .saturating_add(drop_count * terravibe_sim::coin::coin_unit_value(held.id));
                dropped_coins.push(terravibe_sim::inventory::ItemStack {
                    id: held.id,
                    count: drop_count,
                    prefix: held.prefix,
                });
                if remaining > 0 {
                    held.count = remaining;
                } else {
                    c.held = terravibe_sim::slots::Held(None);
                }
            }
        }
        for slot in &mut c.inv.slots {
            if let Some(stack) = slot {
                if terravibe_sim::item::is_coin(stack.id) {
                    let drop_count = (stack.count + 1) / 2;
                    let remaining = stack.count - drop_count;
                    coins_lost = coins_lost.saturating_add(
                        drop_count * terravibe_sim::coin::coin_unit_value(stack.id),
                    );
                    dropped_coins.push(terravibe_sim::inventory::ItemStack {
                        id: stack.id,
                        count: drop_count,
                        prefix: stack.prefix,
                    });
                    if remaining > 0 {
                        stack.count = remaining;
                    } else {
                        *slot = None;
                    }
                }
            }
        }

        for st in dropped_coins {
            self.add_drop(st, death_pos);
        }

        if let Some(c) = self.conns.get_mut(cid) {
            c.dead |=
                c.io.send(ServerMsg::PlayerDied {
                    cause,
                    coins_lost,
                    survived_ticks,
                })
                .is_err();
            c.dead |= c.io.send(ServerMsg::Inventory(c.inv.clone())).is_err();
            c.dead |= c.io.send(ServerMsg::Held(c.held.0)).is_err();
        }
    }

    pub fn kill_player(&mut self, cid: usize, cause: DeathCause) {
        if cid >= self.conns.len() {
            return;
        }
        let Some(p) = self.conns[cid].player.as_mut() else {
            return;
        };
        if p.dead {
            return;
        }
        let mut ev = Vec::new();
        p.kill(cause, &mut ev);
        for e in ev {
            self.conns[cid].dead |= self.conns[cid].io.send(ServerMsg::Event(e)).is_err();
        }
        self.on_player_killed(cid, cause);
    }

    fn inventory_has_ore(inv: &Inventory, held: &Held) -> bool {
        let is_ore = |id: terravibe_sim::item::ItemId| {
            matches!(
                id,
                terravibe_sim::item::COPPER_ORE
                    | terravibe_sim::item::IRON_ORE
                    | terravibe_sim::item::SILVER_ORE
                    | terravibe_sim::item::GOLD_ORE
                    | terravibe_sim::item::DEMONITE_ORE_ITEM
            )
        };
        inv.slots.iter().flatten().any(|stack| is_ore(stack.id))
            || held.0.as_ref().map_or(false, |stack| is_ore(stack.id))
    }

    pub fn handle_client_msg(&mut self, cid: usize, msg: ClientMsg) {
        match msg {
            ClientMsg::Respawn => {
                let tick = self.clock.tick;
                if let Some(c) = self.conns.get_mut(cid) {
                    if let Some(p) = c.player.as_mut() {
                        if p.dead {
                            let mut ev = Vec::new();
                            p.respawn(&mut ev);
                            c.spawn_tick = tick;
                            c.sent_hp = Some((
                                p.vitals.life,
                                p.vitals.life_max,
                                p.vitals.breath,
                                p.dead,
                                p.vitals.mana,
                                p.vitals.mana_max,
                            ));
                            c.dead |=
                                c.io.send(ServerMsg::PlayerState {
                                    ack: c.last_ack,
                                    body: p.body.clone(),
                                    vitals: p.vitals.clone(),
                                    dead: p.dead,
                                    respawn_ticks: p.respawn_ticks,
                                })
                                .is_err();
                            for e in ev {
                                c.dead |= c.io.send(ServerMsg::Event(e)).is_err();
                            }
                        }
                    }
                }
            }
            ClientMsg::QaKill => {
                if self.cfg.qa_kill {
                    self.kill_player(cid, DeathCause::Unknown);
                }
            }
            ClientMsg::TalkNpc { .. }
            | ClientMsg::ShopBuy { .. }
            | ClientMsg::ShopSell { .. }
            | ClientMsg::GuideAdvice
            | ClientMsg::AssignHousing { .. } => {
                if let Some(c) = self.conns.get_mut(cid) {
                    c.town_reqs.push(msg);
                }
                self.process_town_reqs();
            }
            ClientMsg::CreativeGive { item, count } => {
                if !item_exists(item) {
                    return;
                }
                let max_stack = item_data(item).max_stack.max(1);
                let n = count.clamp(1, max_stack);
                if let Some(c) = self.conns.get_mut(cid) {
                    let prefix = roll_prefix(item, &mut || c.rng.next_u32());
                    c.inv.add(ItemStack {
                        id: item,
                        count: n,
                        prefix,
                    });
                    c.dead |= c.io.send(ServerMsg::Inventory(c.inv.clone())).is_err();
                    c.sent_inv = Some(c.inv.clone());
                }
            }
            ClientMsg::CreativeAction(cmd) => match cmd {
                CreativeCmd::FullHeal => {
                    if let Some(c) = self.conns.get_mut(cid) {
                        if let Some(p) = c.player.as_mut() {
                            p.vitals.life = p.vitals.life_max;
                            p.vitals.mana = p.effective_mana_max();
                            p.vitals.potion_delay_ticks = 0;
                            p.vitals.burn_ticks = 0;
                            p.vitals.burning_ticks = 0;
                            p.vitals.breath = terravibe_sim::player::consts::BREATH_MAX;
                            c.sent_hp = Some((
                                p.vitals.life,
                                p.vitals.life_max,
                                p.vitals.breath,
                                p.dead,
                                p.vitals.mana,
                                p.vitals.mana_max,
                            ));
                            c.dead |=
                                c.io.send(ServerMsg::PlayerState {
                                    ack: c.last_ack,
                                    body: p.body.clone(),
                                    vitals: p.vitals.clone(),
                                    dead: p.dead,
                                    respawn_ticks: p.respawn_ticks,
                                })
                                .is_err();
                        }
                    }
                }
                CreativeCmd::ToggleGodMode => {
                    if let Some(c) = self.conns.get_mut(cid) {
                        if let Some(p) = c.player.as_mut() {
                            p.god_mode = !p.god_mode;
                            let god = p.god_mode;
                            c.dead |= c.io.send(ServerMsg::GodMode(god)).is_err();
                        }
                    }
                }
                CreativeCmd::SetTime(t) => {
                    let tick = t % terravibe_sim::time::CYCLE_TICKS;
                    self.clock.tick = tick;
                    self.was_day = self.clock.is_day();
                    for c in &mut self.conns {
                        c.dead |= c.io.send(ServerMsg::Time { tick }).is_err();
                    }
                }
            },
            _ => {}
        }
    }

    /// Physics, pickup and per-connection streaming of world items.
    fn tick_drops(&mut self) {
        let bounds = self.bounds;
        let guide_alive = self.town_alive(TownKind::Guide);
        let wof_active = self.wof.is_some();
        let mut wof_summon_pos: Option<[f32; 3]> = None;
        for d in &mut self.drops {
            tick_drop(&WorldQ(&self.world), d);
            bounds.clamp_body(&mut d.pos, &mut d.vel, 0.25);
            if d.stack.id == terravibe_sim::item::GUIDE_VOODOO_DOLL
                && d.stack.count > 0
                && d.pos[1] <= terravibe_sim::worldgen::underworld::UNDERWORLD_TOP_Y as f32 + 1.0
            {
                let liq = WorldQ(&self.world).liquid(
                    d.pos[0].floor() as i32,
                    d.pos[1].floor() as i32,
                    d.pos[2].floor() as i32,
                );
                if liq.is_some_and(|l| l.kind == terravibe_sim::LiquidKind::Lava) {
                    d.stack.count = 0;
                    if guide_alive && !wof_active && wof_summon_pos.is_none() {
                        wof_summon_pos = Some(d.pos);
                    }
                }
            }
        }
        if let Some(drop_pos) = wof_summon_pos {
            // Guide Voodoo Doll burns in Underworld lava: kill Guide and summon Wall of Flesh.
            self.enemies.retain(|e| e.kind != EnemyKind::Guide);
            self.dead_town_npcs.insert(TownKind::Guide);
            let player_pos = self
                .conns
                .iter()
                .filter_map(|c| c.player.as_ref().filter(|p| !p.dead).map(|p| p.body.pos))
                .min_by(|a, b| dist2(*a, drop_pos).total_cmp(&dist2(*b, drop_pos)))
                .unwrap_or(drop_pos);
            self.spawn_wof(player_pos);
        }
        for c in &mut self.conns {
            let Some(p) = c.player.as_mut().filter(|p| !p.dead) else {
                continue;
            };
            let centre = [p.body.pos[0], p.body.pos[1] + HEIGHT / 2.0, p.body.pos[2]];
            let mut picked = false;
            for d in &mut self.drops {
                // Hearts heal on contact and are never stored in the inventory.
                if d.stack.id == HEART {
                    let count = d.stack.count;
                    if try_pickup_heart(d, &mut p.vitals, centre) {
                        c.known_drops.remove(&d.id);
                        c.dead |=
                            c.io.send(ServerMsg::DropPickup {
                                id: d.id,
                                item: HEART,
                                count,
                            })
                            .is_err();
                    }
                    continue;
                }
                let before_count = d.stack.count;
                if try_pickup(d, &mut c.inv, centre) {
                    picked = true;
                    if d.stack.count == 0 {
                        c.known_drops.remove(&d.id);
                        c.dead |=
                            c.io.send(ServerMsg::DropPickup {
                                id: d.id,
                                item: d.stack.id,
                                count: before_count,
                            })
                            .is_err();
                    }
                }
            }
            if picked && c.sent_inv.as_ref() != Some(&c.inv) {
                c.dead |= c.io.send(ServerMsg::Inventory(c.inv.clone())).is_err();
                c.sent_inv = Some(c.inv.clone());
            }
        }
        self.drops.retain(|d| d.stack.count > 0);
        for c in &mut self.conns {
            let view = match c.player.as_ref().filter(|p| !p.dead) {
                Some(p) => Some(p.body.pos),
                None if self.cfg.demo_enemies => c.interest.centre,
                None => None,
            };
            let near: Vec<&Drop> = match view {
                Some(v) => self
                    .drops
                    .iter()
                    .filter(|d| dist2(v, d.pos) <= DROP_RADIUS.powi(2))
                    .collect(),
                None => Vec::new(),
            };
            let gone: Vec<DropId> = c
                .known_drops
                .iter()
                .copied()
                .filter(|id| !near.iter().any(|d| d.id == *id))
                .collect();
            for id in gone {
                c.known_drops.remove(&id);
                c.dead |= c.io.send(ServerMsg::DropDespawn { id }).is_err();
            }
            for d in near {
                if c.known_drops.insert(d.id) {
                    c.dead |=
                        c.io.send(ServerMsg::DropSpawn {
                            id: d.id,
                            stack: d.stack,
                            pos: d.pos,
                        })
                        .is_err();
                }
            }
        }
    }

    /// Hits land outside input handling, so an idle client would otherwise see
    /// its HP late: send `PlayerState` whenever life, death, or mana changed.
    fn sync_player_state(&mut self) {
        for c in &mut self.conns {
            let Some(p) = c.player.as_ref() else { continue };
            let hp = (
                p.vitals.life,
                p.vitals.life_max,
                p.vitals.breath,
                p.dead,
                p.vitals.mana,
                p.vitals.mana_max,
            );
            if c.sent_hp.is_some_and(|s| s != hp) {
                c.dead |=
                    c.io.send(ServerMsg::PlayerState {
                        ack: c.last_ack,
                        body: p.body.clone(),
                        vitals: p.vitals.clone(),
                        dead: p.dead,
                        respawn_ticks: p.respawn_ticks,
                    })
                    .is_err();
            }
            c.sent_hp = Some(hp);
        }
    }

    /// Steps arrows and projectiles, applies hits (damage path of melee: `apply_hit`)
    /// and streams the survivors. Nothing is sent while none exist.
    fn tick_projectiles(&mut self) {
        let mut grapple_pulls: Vec<(u32, [f32; 3])> = Vec::new();
        for p in &self.projectiles {
            if p.kind == ProjKind::GrappleHook && p.ai0 == 1 {
                grapple_pulls.push((p.owner, p.pos));
            }
        }
        let mut detached_owners: Vec<u32> = Vec::new();
        for (owner, hook_pos) in grapple_pulls {
            if let Some(c) = self.conns.iter_mut().find(|c| c.id == owner)
                && let Some(pl) = c.player.as_mut().filter(|pl| !pl.dead)
            {
                if pl.body.jump_held_prev {
                    pl.body.vel[1] = terravibe_sim::player::consts::JUMP_SPEED;
                    detached_owners.push(owner);
                } else {
                    let target = [hook_pos[0], hook_pos[1] - 0.9, hook_pos[2]];
                    let dx = target[0] - pl.body.pos[0];
                    let dy = target[1] - pl.body.pos[1];
                    let dz = target[2] - pl.body.pos[2];
                    let d = (dx * dx + dy * dy + dz * dz).sqrt();
                    if d > 0.35 {
                        let step = 0.72f32.min(d);
                        let wq = WorldQ(&self.world);
                        for (axis, delta) in
                            [(0, dx / d * step), (1, dy / d * step), (2, dz / d * step)]
                        {
                            terravibe_sim::player::collide::move_axis(
                                &wq,
                                &mut pl.body.pos,
                                axis,
                                delta,
                            );
                        }
                    }
                    pl.body.vel = [0.0; 3];
                    pl.body.fall_peak_y = pl.body.pos[1];
                }
            }
        }
        if !detached_owners.is_empty() {
            self.projectiles.retain(|p| {
                !(p.kind == ProjKind::GrappleHook && detached_owners.contains(&p.owner))
            });
        }

        let world = &self.world;
        let bounds = self.bounds;
        let conns = &mut self.conns;
        let (enemies, rng) = (&mut self.enemies, &mut self.rng);
        let mut proj_deaths = Vec::new();
        let mut combat_hits = Vec::new();
        let mut explosions: Vec<(ProjKind, [f32; 3], i32, f32, i32)> = Vec::new();
        self.projectiles.retain_mut(|p| {
            if p.owner == u32::MAX {
                // Enemy-owned projectile: skip enemy hit checks, damage overlapping live players
                let mut hit_player = false;
                for (cid, c) in conns.iter_mut().enumerate() {
                    let Some(pl) = c.player.as_mut().filter(|pl| !pl.dead) else {
                        continue;
                    };
                    let pp = pl.body.pos;
                    let hit = (p.pos[0] - pp[0]).abs()
                        < HALF_WIDTH + 5.0 * terravibe_sim::player::consts::PX_TO_BLOCKS
                        && (p.pos[2] - pp[2]).abs()
                            < HALF_WIDTH + 5.0 * terravibe_sim::player::consts::PX_TO_BLOCKS
                        && p.pos[1] + 5.0 * terravibe_sim::player::consts::PX_TO_BLOCKS > pp[1]
                        && p.pos[1] - 5.0 * terravibe_sim::player::consts::PX_TO_BLOCKS
                            < pp[1] + HEIGHT;
                    if hit {
                        let mut ev = Vec::new();
                        if hurt_player_with_cause(
                            pl,
                            p.damage,
                            DeathCause::Projectile(p.kind),
                            &mut ev,
                        ) {
                            let amount = damage_taken(p.damage, pl.defense());
                            let dx = pp[0] - p.pos[0];
                            let dz = pp[2] - p.pos[2];
                            let h = dx.hypot(dz);
                            let dir = if h > 1e-4 {
                                [dx / h, 0.55, dz / h]
                            } else {
                                [0.0, 0.55, 0.0]
                            };
                            pl.apply_knockback(0.32, dir);
                            c.dead |= c.io.send(ServerMsg::PlayerHit { amount }).is_err();
                            combat_hits.push(ServerMsg::CombatHit {
                                pos: [pp[0], pp[1] + HEIGHT * 0.5, pp[2]],
                                attacker_pos: Some(p.origin),
                                amount,
                                crit: false,
                                damage_kind: DamageKind::Projectile,
                                target_kind: HitTargetKind::Player,
                                killed: pl.dead,
                            });
                            if pl.dead {
                                proj_deaths.push((cid, DeathCause::Projectile(p.kind)));
                            }
                        }
                        hit_player = true;
                        break;
                    }
                }
                if hit_player {
                    return false;
                }
            } else {
                let hits = hit_enemies_detailed(p, enemies, rng);
                let damage_kind = match p.kind {
                    ProjKind::Arrow
                    | ProjKind::FlamingArrow
                    | ProjKind::Bullet
                    | ProjKind::FallenStarShot => DamageKind::Ranged,
                    ProjKind::Spark
                    | ProjKind::Vilethorn
                    | ProjKind::MagicMissile
                    | ProjKind::Flamelash
                    | ProjKind::WaterStream
                    | ProjKind::WaterBolt
                    | ProjKind::Laser
                    | ProjKind::SpaceLaser
                    | ProjKind::FlowerOfFire
                    | ProjKind::BeeShot => DamageKind::Magic,
                    ProjKind::Boomerang
                    | ProjKind::Flamarang
                    | ProjKind::ThornChakram
                    | ProjKind::Spear
                    | ProjKind::Flail
                    | ProjKind::Yoyo
                    | ProjKind::StarfuryStar => DamageKind::Melee,
                    ProjKind::Fireball
                    | ProjKind::DemonScythe
                    | ProjKind::EyeLaser
                    | ProjKind::Grenade
                    | ProjKind::Bomb
                    | ProjKind::Dynamite
                    | ProjKind::GrappleHook => DamageKind::Projectile,
                };
                for h in &hits {
                    if let Some(e) = enemies.iter().find(|e| e.id == h.id) {
                        let hit_pos = [e.pos[0], e.pos[1] + e.kind.stats().half[1], e.pos[2]];
                        combat_hits.push(ServerMsg::CombatHit {
                            pos: hit_pos,
                            attacker_pos: Some(p.origin),
                            amount: h.hit.damage,
                            crit: h.hit.crit,
                            damage_kind,
                            target_kind: if e.kind.is_boss() {
                                HitTargetKind::Boss
                            } else {
                                HitTargetKind::Enemy
                            },
                            killed: h.hit.killed,
                        });
                    }
                }
                if let Some(cap) = p.kind.pierce_cap() {
                    // Single-hit kinds and multi-hit ones alike are spent once
                    // they have damaged `cap` enemies.
                    if p.pierced >= cap {
                        if p.kind == ProjKind::Grenade {
                            explosions.push((p.kind, p.pos, p.damage, p.knock_back, p.crit));
                        }
                        return false;
                    }
                }
            }
            if matches!(p.kind, ProjKind::MagicMissile | ProjKind::Flamelash)
                && let Some((eye, dir, true)) = conns
                    .iter()
                    .find(|c| c.id == p.owner)
                    .and_then(|c| c.last_aim)
            {
                let dist = ((p.pos[0] - eye[0]).powi(2)
                    + (p.pos[1] - eye[1]).powi(2)
                    + (p.pos[2] - eye[2]).powi(2))
                .sqrt()
                .clamp(4.0, 24.0);
                let aim = [
                    eye[0] + dir[0] * dist,
                    eye[1] + dir[1] * dist,
                    eye[2] + dir[2] * dist,
                ];
                terravibe_sim::projectile::steer_magic_missile(p, aim);
            }
            if p.kind == ProjKind::BeeShot
                && let Some(target_pos) = enemies
                    .iter()
                    .filter(|e| e.hp > 0 && !e.kind.is_friendly())
                    .min_by(|a, b| dist2(a.pos, p.pos).total_cmp(&dist2(b.pos, p.pos)))
                    .map(|e| [e.pos[0], e.pos[1] + e.kind.stats().half[1], e.pos[2]])
            {
                let dx = target_pos[0] - p.pos[0];
                let dy = target_pos[1] - p.pos[1];
                let dz = target_pos[2] - p.pos[2];
                let d = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-4);
                let spd = (p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1] + p.vel[2] * p.vel[2])
                    .sqrt()
                    .max(0.25);
                p.vel = [
                    p.vel[0] + (dx / d * spd - p.vel[0]) * 0.25,
                    p.vel[1] + (dy / d * spd - p.vel[1]) * 0.25,
                    p.vel[2] + (dz / d * spd - p.vel[2]) * 0.25,
                ];
            }
            let owner_pos = conns
                .iter()
                .find(|c| c.id == p.owner)
                .and_then(|c| c.player.as_ref())
                .map(|pl| [pl.body.pos[0], pl.body.pos[1] + EYE_HEIGHT, pl.body.pos[2]]);
            let target = owner_pos.unwrap_or(p.origin);
            let step_res = step_with_target(&WorldQ(world), p, target);
            if step_res != Step::Flying
                && matches!(
                    p.kind,
                    ProjKind::Grenade | ProjKind::Bomb | ProjKind::Dynamite
                )
            {
                explosions.push((p.kind, p.pos, p.damage, p.knock_back, p.crit));
            }
            step_res == Step::Flying
                && bounds.contains_column(p.pos[0].floor() as i32, p.pos[2].floor() as i32)
        });
        for (kind, pos, dmg, kb, crit) in explosions {
            self.explode_projectile(kind, pos, dmg, kb, crit);
        }
        for msg in combat_hits {
            self.send_all_players(msg);
        }
        for (cid, cause) in proj_deaths {
            self.on_player_killed(cid, cause);
        }
        for c in &mut self.conns {
            let Some(pos) = c.player.as_ref().map(|p| p.body.pos) else {
                continue;
            };
            let list: Vec<Projectile> = self
                .projectiles
                .iter()
                .filter(|p| dist2(p.pos, pos) <= DROP_RADIUS * DROP_RADIUS)
                .cloned()
                .collect();
            if list.is_empty() && !c.sent_proj {
                continue;
            }
            c.sent_proj = !list.is_empty();
            c.dead |= c.io.send(ServerMsg::Projectiles { list }).is_err();
        }
    }

    fn explode_projectile(
        &mut self,
        kind: ProjKind,
        pos: [f32; 3],
        damage: i32,
        knock_back: f32,
        crit: i32,
    ) {
        let radius = match kind {
            ProjKind::Dynamite => 5.5f32,
            _ => 3.5f32,
        };
        let r2 = radius * radius;
        for e in &mut self.enemies {
            if e.hp <= 0 || e.kind.is_friendly() {
                continue;
            }
            if dist2(e.pos, pos) <= r2 {
                let _ = apply_hit(e, pos, damage, knock_back, crit, &mut self.rng);
            }
        }
        let mut player_deaths = Vec::new();
        for (cid, c) in self.conns.iter_mut().enumerate() {
            let Some(pl) = c.player.as_mut().filter(|pl| !pl.dead && !pl.god_mode) else {
                continue;
            };
            if dist2(pl.body.pos, pos) <= r2 {
                let mut ev = Vec::new();
                let self_dmg = damage / 2;
                if hurt_player_with_cause(pl, self_dmg, DeathCause::Projectile(kind), &mut ev) {
                    let amount = damage_taken(self_dmg, pl.defense());
                    c.dead |= c.io.send(ServerMsg::PlayerHit { amount }).is_err();
                    if pl.dead {
                        player_deaths.push((cid, DeathCause::Projectile(kind)));
                    }
                }
            }
        }
        for (cid, cause) in player_deaths {
            self.on_player_killed(cid, cause);
        }
        if matches!(kind, ProjKind::Bomb | ProjKind::Dynamite) {
            let r_i = radius.ceil() as i32;
            let cx = pos[0].floor() as i32;
            let cy = pos[1].floor() as i32;
            let cz = pos[2].floor() as i32;
            let mut broken_orbs = Vec::new();
            for dy in -r_i..=r_i {
                let wy = cy + dy;
                if wy <= 0 || wy >= terravibe_sim::WORLD_HEIGHT as i32 {
                    continue;
                }
                for dz in -r_i..=r_i {
                    let wz = cz + dz;
                    for dx in -r_i..=r_i {
                        let wx = cx + dx;
                        if (dx * dx + dy * dy + dz * dz) as f32 > r2 {
                            continue;
                        }
                        let cp = ChunkPos::of_block(wx, wy, wz);
                        let Some(chunk) = self.world.get_mut(&cp) else {
                            continue;
                        };
                        let lx = wx.rem_euclid(CHUNK_SIZE) as usize;
                        let ly = wy.rem_euclid(CHUNK_SIZE) as usize;
                        let lz = wz.rem_euclid(CHUNK_SIZE) as usize;
                        let b = chunk.get(lx, ly, lz);
                        if b == AIR
                            || matches!(
                                b,
                                CHEST
                                    | terravibe_sim::block::DEMON_ALTAR
                                    | terravibe_sim::block::HELLSTONE
                                    | terravibe_sim::block::HELLSTONE_BRICK
                                    | terravibe_sim::block::OBSIDIAN
                                    | terravibe_sim::block::BLUE_DUNGEON_BRICK
                                    | terravibe_sim::block::GREEN_DUNGEON_BRICK
                                    | terravibe_sim::block::PINK_DUNGEON_BRICK
                                    | terravibe_sim::block::OBSIDIAN_BRICK
                            )
                        {
                            continue;
                        }
                        Arc::make_mut(chunk).set(lx, ly, lz, AIR);
                        let bpos = [wx, wy, wz];
                        self.edited.insert(cp);
                        self.dirty.insert(cp);
                        self.liquid_sim.wake(bpos);
                        for c in &mut self.conns {
                            if c.interest.sent.contains(&cp) {
                                c.dead |=
                                    c.io.send(ServerMsg::BlockChanged {
                                        pos: bpos,
                                        block: AIR,
                                    })
                                    .is_err();
                            }
                        }
                        if b == terravibe_sim::block::SHADOW_ORB {
                            broken_orbs.push(bpos);
                        } else if self.rng.below(2) == 0
                            && let Some(item) = b.drop_item()
                        {
                            self.add_drop(
                                ItemStack {
                                    id: item,
                                    count: 1,
                                    prefix: Prefix(0),
                                },
                                [wx as f32 + 0.5, wy as f32 + 0.5, wz as f32 + 0.5],
                            );
                        }
                    }
                }
            }
            for orb_pos in broken_orbs {
                self.handle_shadow_orb_broken(orb_pos);
            }
        }
    }

    /// Falling stars: spawn at night, step downward, land into drops, despawn at dawn.
    fn tick_stars(&mut self) {
        if self.clock.is_day() {
            self.stars.clear();
            self.drops.retain(|d| d.stack.id != FALLEN_STAR);
        } else {
            if self.cfg.natural_spawns && !self.conns.is_empty() {
                if let Some(player) = self.conns.iter().filter_map(|c| c.player.as_ref()).next() {
                    let ground_y = player.body.pos[1];
                    if let Some(mut star) = spawn_star(
                        &mut self.rng,
                        &self.bounds,
                        ground_y,
                        self.star_boost,
                        false,
                    ) {
                        star.id = self.next_star_id;
                        self.next_star_id = self.next_star_id.wrapping_add(1);
                        self.stars.push(star);
                    }
                }
            }
            let mut landed = Vec::new();
            let world = &self.world;
            let bounds = &self.bounds;
            self.stars
                .retain_mut(|s| match step_star(&WorldQ(world), s) {
                    StarStep::Landed => {
                        landed.push(s.pos);
                        false
                    }
                    StarStep::Falling => {
                        bounds.contains_column(s.pos[0].floor() as i32, s.pos[2].floor() as i32)
                    }
                });
            for pos in landed {
                self.add_drop(
                    terravibe_sim::inventory::ItemStack {
                        id: FALLEN_STAR,
                        count: 1,
                        prefix: terravibe_sim::item::Prefix(0),
                    },
                    pos,
                );
            }
        }

        for c in &mut self.conns {
            let Some(pos) = c.player.as_ref().map(|p| p.body.pos) else {
                continue;
            };
            let list: Vec<Star> = self
                .stars
                .iter()
                .filter(|s| dist2(s.pos, pos) <= DROP_RADIUS * DROP_RADIUS * 4.0)
                .cloned()
                .collect();
            if list.is_empty() && !c.sent_stars {
                continue;
            }
            c.sent_stars = !list.is_empty();
            c.dead |= c.io.send(ServerMsg::Stars { list }).is_err();
        }
    }

    fn check_dungeon_defense(&mut self, players: &[[f32; 3]]) {
        let entrance = terravibe_sim::worldgen::dungeon_entrance_pos(self.seed, self.bounds.size());
        let surf_y = entrance[1] as i32 - 1;
        for &p in players {
            let (px, py, pz) = (
                p[0].floor() as i32,
                p[1].floor() as i32,
                p[2].floor() as i32,
            );
            if py < surf_y
                && terravibe_sim::worldgen::is_dungeon_point(
                    self.seed,
                    px,
                    py,
                    pz,
                    self.bounds.size(),
                )
            {
                if !self.downed_skeletron {
                    if self
                        .enemies
                        .iter()
                        .any(|e| e.kind == EnemyKind::DungeonGuardian && e.hp > 0)
                    {
                        return;
                    }
                    let spawn_pos = [p[0], p[1] + 10.0, p[2]];
                    self.add_enemy(EnemyKind::DungeonGuardian, spawn_pos);
                    break;
                } else {
                    self.dungeon_spawn_ticks = self.dungeon_spawn_ticks.saturating_add(1);
                    if self.dungeon_spawn_ticks < 180 {
                        return;
                    }
                    let dungeon_enemy_count = self
                        .enemies
                        .iter()
                        .filter(|e| {
                            matches!(
                                e.kind,
                                EnemyKind::AngryBones
                                    | EnemyKind::DarkCaster
                                    | EnemyKind::CursedSkull
                            ) && e.hp > 0
                        })
                        .count();
                    if dungeon_enemy_count < 5 {
                        let kinds = [
                            EnemyKind::AngryBones,
                            EnemyKind::DarkCaster,
                            EnemyKind::CursedSkull,
                        ];
                        let kind = kinds[(self.rng.next_u32() % 3) as usize];
                        let mut chosen = None;
                        let player_loaded;
                        {
                            let wq = WorldQ(&self.world);
                            player_loaded = wq.is_loaded(px, py, pz);
                            for _ in 0..8 {
                                let ang = ((self.rng.next_u32() >> 8) as f32 / (1u32 << 24) as f32)
                                    * std::f32::consts::TAU;
                                let r = 12.0
                                    + ((self.rng.next_u32() >> 8) as f32 / (1u32 << 24) as f32)
                                        * 8.0;
                                let sx = (p[0] + ang.cos() * r).floor() as i32;
                                let sz = (p[2] + ang.sin() * r).floor() as i32;
                                let sy = (-6..=6)
                                    .map(|dy| py + dy)
                                    .find(|&cy| cy >= 2 && wq.has_clearance(sx, cy, sz));
                                if let Some(sy) = sy {
                                    let spawn_pos = [sx as f32 + 0.5, sy as f32, sz as f32 + 0.5];
                                    let too_close = players.iter().any(|q| {
                                        (q[0] - spawn_pos[0]).hypot(q[2] - spawn_pos[2]) < 8.0
                                    });
                                    if !too_close {
                                        chosen = Some(spawn_pos);
                                        break;
                                    }
                                }
                            }
                        }
                        if let Some(spawn_pos) = chosen {
                            self.add_enemy(kind, spawn_pos);
                            self.dungeon_spawn_ticks = 0;
                        } else if !player_loaded {
                            self.add_enemy(kind, [p[0] + 10.0, p[1], p[2]]);
                            self.dungeon_spawn_ticks = 0;
                        }
                        break;
                    }
                }
            }
        }
    }

    /// Spawn, steer, stream and cull enemies.
    fn tick_enemies(&mut self) {
        terravibe_sim::enemy::handle_eater_of_worlds_splitting(&mut self.enemies);
        for e in &mut self.enemies {
            e.tick_on_fire();
        }
        let had_eow = self
            .enemies
            .iter()
            .any(|e| terravibe_sim::enemy::is_eater_of_worlds(e.kind));
        let mut last_eow_pos: Option<[f32; 3]> = None;
        let dead: Vec<(EnemyKind, [f32; 3])> = self
            .enemies
            .iter()
            .filter(|e| e.hp <= 0)
            .map(|e| (e.kind, e.pos))
            .collect();
        for (kind, pos) in dead {
            for stack in loot_for(kind, &mut self.rng) {
                self.add_drop(stack, pos);
            }
            if terravibe_sim::enemy::is_eater_of_worlds(kind) {
                last_eow_pos = Some(pos);
            }
            if let Some(tk) = TownKind::from_enemy(kind) {
                self.dead_town_npcs.insert(tk);
            }
            if kind == EnemyKind::EyeOfCthulhu {
                self.facts.downed_eye = true;
                // Bosses drop potions, hearts, and 30..=90 Demonite Ore.
                let demonite = boss::eye_demonite_loot(&mut self.rng);
                self.add_drop(demonite, pos);
                for stack in boss::boss_extra_loot(&mut self.rng) {
                    self.add_drop(stack, pos);
                }
                self.boss = None;
                boss::clear_servants(&mut self.enemies);
                self.send_all_players(ServerMsg::BossGone);
                continue;
            }
            if kind == EnemyKind::KingSlime {
                self.facts.downed_king_slime = true;
                for stack in terravibe_sim::drop::roll_king_slime_loot(&mut self.rng) {
                    self.add_drop(stack, pos);
                }
                for stack in boss::boss_extra_loot(&mut self.rng) {
                    self.add_drop(stack, pos);
                }
                self.send_all_players(ServerMsg::BossGone);
                continue;
            }
            if kind == EnemyKind::QueenBee {
                self.facts.downed_queen_bee = true;
                for stack in terravibe_sim::drop::roll_queen_bee_loot(&mut self.rng) {
                    self.add_drop(stack, pos);
                }
                for stack in boss::boss_extra_loot(&mut self.rng) {
                    self.add_drop(stack, pos);
                }
                self.send_all_players(ServerMsg::BossGone);
                continue;
            }
            if kind == EnemyKind::SkeletronHead {
                self.downed_skeletron = true;
                self.facts.downed_skeletron = true;
                for stack in boss::boss_extra_loot(&mut self.rng) {
                    self.add_drop(stack, pos);
                }
                self.enemies.retain(|e| e.kind != EnemyKind::SkeletronHand);
                self.send_all_players(ServerMsg::BossGone);
                continue;
            }
            if kind == EnemyKind::KingSlime {
                self.facts.downed_king_slime = true;
                for stack in terravibe_sim::drop::roll_king_slime_loot(&mut self.rng) {
                    self.add_drop(stack, pos);
                }
                for stack in boss::boss_extra_loot(&mut self.rng) {
                    self.add_drop(stack, pos);
                }
                self.send_all_players(ServerMsg::BossGone);
                continue;
            }
            if kind == EnemyKind::QueenBee {
                self.facts.downed_queen_bee = true;
                for stack in terravibe_sim::drop::roll_queen_bee_loot(&mut self.rng) {
                    self.add_drop(stack, pos);
                }
                for stack in boss::boss_extra_loot(&mut self.rng) {
                    self.add_drop(stack, pos);
                }
                self.send_all_players(ServerMsg::BossGone);
                continue;
            }
            if kind == EnemyKind::WallOfFleshMouth {
                self.facts.downed_wall_of_flesh = true;
                let loot_pos = [pos[0], pos[1].max(4.0), pos[2]];
                for stack in terravibe_sim::drop::roll_wof_loot(&mut self.rng) {
                    self.add_drop(stack, loot_pos);
                }
                for stack in boss::boss_extra_loot(&mut self.rng) {
                    self.add_drop(stack, loot_pos);
                }
                self.wof = None;
                terravibe_sim::wof::clear_wof(&mut self.enemies);
                self.projectiles.retain(|p| p.kind != ProjKind::EyeLaser);
                self.send_all_players(ServerMsg::WofGone);
                self.send_all_players(ServerMsg::WallOfFleshDefeated);
                continue;
            }
            // Hearts depend on the nearest live player being hurt (gameplay spec).
            // Ambient critters never drop hearts.
            if !kind.is_critter() {
                let hurt = self
                    .conns
                    .iter()
                    .filter_map(|c| c.player.as_ref().filter(|p| !p.dead))
                    .min_by(|a, b| dist2(a.body.pos, pos).total_cmp(&dist2(b.body.pos, pos)))
                    .is_some_and(|p| p.vitals.life < p.vitals.life_max);
                if let Some(stack) = heart_drop(&mut self.rng, hurt) {
                    self.add_drop(stack, pos);
                }
            }
            // Wave 5 event & biome drops
            match kind {
                EnemyKind::Hornet if self.rng.below(2) == 0 => {
                    self.add_drop(
                        ItemStack {
                            id: terravibe_sim::item::STINGER,
                            count: 1,
                            prefix: Prefix(0),
                        },
                        pos,
                    );
                }
                EnemyKind::ManEater if self.rng.below(2) == 0 => {
                    self.add_drop(
                        ItemStack {
                            id: terravibe_sim::item::VINE,
                            count: 1,
                            prefix: Prefix(0),
                        },
                        pos,
                    );
                }
                EnemyKind::BloodZombie | EnemyKind::Drippler => {
                    if self.rng.below(20) == 0 {
                        self.add_drop(
                            ItemStack {
                                id: terravibe_sim::item::MONEY_TROUGH,
                                count: 1,
                                prefix: Prefix(0),
                            },
                            pos,
                        );
                    } else if self.rng.below(20) == 0 {
                        self.add_drop(
                            ItemStack {
                                id: terravibe_sim::item::SHARK_TOOTH_NECKLACE,
                                count: 1,
                                prefix: Prefix(0),
                            },
                            pos,
                        );
                    }
                }
                EnemyKind::Shark if self.rng.below(10) == 0 => {
                    self.add_drop(
                        ItemStack {
                            id: terravibe_sim::item::SHARK_TOOTH_NECKLACE,
                            count: 1,
                            prefix: Prefix(0),
                        },
                        pos,
                    );
                }
                EnemyKind::GoblinPeon
                | EnemyKind::GoblinThief
                | EnemyKind::GoblinWarrior
                | EnemyKind::GoblinSorcerer
                | EnemyKind::GoblinArcher => {
                    self.goblin_army_remaining = self.goblin_army_remaining.saturating_sub(1);
                    if self.rng.below(3) == 0 {
                        let count = 1 + self.rng.below(5);
                        self.add_drop(
                            ItemStack {
                                id: terravibe_sim::item::SPIKY_BALL,
                                count,
                                prefix: Prefix(0),
                            },
                            pos,
                        );
                    }
                    if self.rng.below(20) == 0 {
                        self.add_drop(
                            ItemStack {
                                id: terravibe_sim::item::HARPOON,
                                count: 1,
                                prefix: Prefix(0),
                            },
                            pos,
                        );
                    }
                }
                _ => {}
            }
        }
        self.enemies.retain(|e| e.hp > 0);
        if had_eow && !self.has_eater_of_worlds() {
            self.facts.downed_eow = true;
            if let Some(p) = last_eow_pos {
                for stack in boss::boss_extra_loot(&mut self.rng) {
                    self.add_drop(stack, p);
                }
            }
            self.send_all_players(ServerMsg::BossGone);
        }
        let players: Vec<[f32; 3]> = self
            .conns
            .iter()
            .filter_map(|c| c.player.as_ref().filter(|p| !p.dead))
            .map(|p| p.body.pos)
            .collect();
        if !self.facts.underworld_reached && players.iter().any(|p| p[1] <= 24.0) {
            self.facts.underworld_reached = true;
        }
        let dungeon_entrance =
            terravibe_sim::worldgen::dungeon_entrance_pos(self.seed, self.bounds.size());
        if !self.facts.dungeon_reached
            && players
                .iter()
                .any(|p| dist2(*p, dungeon_entrance) <= 64.0 * 64.0)
        {
            self.facts.dungeon_reached = true;
        }
        self.check_dungeon_defense(&players);
        let watched = !players.is_empty() || self.conns.iter().any(|c| c.interest.centre.is_some());
        if self.cfg.demo_enemies && !self.demo_done && watched {
            let [sx, sy, sz] = self.spawn;
            let w = WorldQ(&self.world);
            let at = [
                (-3, EnemyKind::Zombie),
                (0, EnemyKind::DemonEye),
                (3, EnemyKind::Slime),
            ]
            .map(|(dx, k)| {
                (
                    k,
                    (sx + dx as f32).floor() as i32,
                    (sz - 6.0).floor() as i32,
                )
            });
            let ys: Vec<Option<i32>> = at.iter().map(|&(_, x, z)| w.surface_y(x, z)).collect();
            if ys.iter().all(Option::is_some) {
                for (&(k, x, z), y) in at.iter().zip(ys) {
                    let lift = if k == EnemyKind::DemonEye { 2.0 } else { 0.0 };
                    let y = y.unwrap() as f32 + 1.0 + lift;
                    self.add_enemy(k, [x as f32 + 0.5, y.max(sy), z as f32 + 0.5]);
                    // A drop beside each enemy so the same shot shows items.
                    let item = if k == EnemyKind::Slime {
                        terravibe_sim::item::GEL
                    } else {
                        terravibe_sim::item::WOOD
                    };
                    let stack = terravibe_sim::inventory::ItemStack {
                        id: item,
                        count: 3,
                        prefix: terravibe_sim::item::Prefix(0),
                    };
                    self.add_drop(stack, [x as f32 + 1.5, y.max(sy), z as f32 + 0.5]);
                }
                self.demo_done = true;
            }
        }
        if self.cfg.natural_spawns {
            let spawned = tick_spawner(
                &SpawnQ {
                    world: WorldQ(&self.world),
                    seed: self.seed,
                    size: self.cfg.size,
                },
                &self.clock,
                &players,
                self.enemies
                    .iter()
                    .filter(|e| !e.kind.is_friendly() && !e.kind.is_critter())
                    .count(),
                self.enemies
                    .iter()
                    .filter(|e| e.kind == EnemyKind::EaterOfSouls)
                    .count(),
                &mut self.spawner,
                &mut self.rng,
            );
            if let Some((mut kind, mut pos)) = spawned {
                let nearest_p = players
                    .iter()
                    .min_by(|a, b| dist2(**a, pos).total_cmp(&dist2(**b, pos)));
                let p_y = nearest_p.map(|p| p[1] as i32).unwrap_or(pos[1] as i32);
                if p_y > terravibe_sim::worldgen::underworld::UNDERWORLD_TOP_Y {
                    let wq = WorldQ(&self.world);
                    let sq = SpawnQ {
                        world: wq,
                        seed: self.seed,
                        size: self.bounds.size(),
                    };
                    let (px, pz) = (pos[0].floor() as i32, pos[2].floor() as i32);
                    if let Some(sy) = sq.surface_y(px, pz) {
                        let is_cave = p_y <= sy - 30;
                        let is_corrupt = sq.corrupt_at(px, pz);
                        if is_cave {
                            let base_y = p_y.min(sy - 30);
                            if let Some(cy) = (-8..=8)
                                .map(|dy| base_y + dy)
                                .find(|&y| y >= 2 && wq.has_clearance(px, y, pz))
                            {
                                pos[1] = cy as f32;
                            }
                            if !(is_corrupt && kind == EnemyKind::EaterOfSouls)
                                && !matches!(
                                    kind,
                                    EnemyKind::Skeleton | EnemyKind::CaveBat | EnemyKind::GiantWorm
                                )
                            {
                                kind = match self.rng.below(3) {
                                    0 => EnemyKind::Skeleton,
                                    1 => EnemyKind::CaveBat,
                                    _ => EnemyKind::GiantWorm,
                                };
                            }
                        } else if matches!(
                            kind,
                            EnemyKind::Skeleton | EnemyKind::CaveBat | EnemyKind::GiantWorm
                        ) {
                            kind = if self.clock.is_day() {
                                EnemyKind::Slime
                            } else if self.rng.below(2) == 0 {
                                EnemyKind::Zombie
                            } else {
                                EnemyKind::DemonEye
                            };
                        }
                        if self.goblin_army_remaining > 0 && !is_cave {
                            kind = match self.rng.below(5) {
                                0 => EnemyKind::GoblinPeon,
                                1 => EnemyKind::GoblinThief,
                                2 => EnemyKind::GoblinWarrior,
                                3 => EnemyKind::GoblinSorcerer,
                                _ => EnemyKind::GoblinArcher,
                            };
                        } else if self.blood_moon
                            && !self.clock.is_day()
                            && !is_cave
                            && self.rng.below(2) == 0
                        {
                            kind = if self.rng.below(2) == 0 {
                                EnemyKind::BloodZombie
                            } else {
                                EnemyKind::Drippler
                            };
                        } else if p_y >= 310 {
                            kind = EnemyKind::Harpy;
                        } else if self.bounds.edge_distance(px, pz)
                            < terravibe_sim::world_bounds::OCEAN_WIDTH
                            && !is_cave
                        {
                            kind = if self.rng.below(2) == 0 {
                                EnemyKind::Shark
                            } else {
                                EnemyKind::Crab
                            };
                        } else if !is_corrupt {
                            let biome = terravibe_sim::worldgen::biome::dominant(
                                &terravibe_sim::worldgen::biome::biome_weights_for_size(
                                    self.seed,
                                    px,
                                    pz,
                                    self.bounds.size(),
                                ),
                            );
                            match biome {
                                terravibe_sim::worldgen::biome::Biome::Desert if !is_cave => {
                                    kind = if self.rng.below(2) == 0 {
                                        EnemyKind::Antlion
                                    } else {
                                        EnemyKind::Vulture
                                    };
                                }
                                terravibe_sim::worldgen::biome::Biome::Snow => {
                                    if is_cave && self.rng.below(2) == 0 {
                                        kind = EnemyKind::SnowFlinx;
                                    } else if !is_cave && self.clock.is_day() {
                                        kind = EnemyKind::IceSlime;
                                    }
                                }
                                terravibe_sim::worldgen::biome::Biome::Jungle => {
                                    if is_cave {
                                        kind = match self.rng.below(3) {
                                            0 => EnemyKind::Hornet,
                                            1 => EnemyKind::ManEater,
                                            _ => EnemyKind::Piranha,
                                        };
                                    } else if self.clock.is_day() {
                                        kind = EnemyKind::JungleSlime;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                if kind == EnemyKind::BoneSerpentHead {
                    self.spawn_bone_serpent(pos);
                } else {
                    self.add_enemy(kind, pos);
                }
            }
            self.tick_critter_spawner(&players);
        }
        self.tick_towns(&players);
        let bounds = self.bounds;
        let is_day = self.clock.is_day();
        let mut dawn_gone: Vec<EnemyId> = Vec::new();
        let mut telegraphs: Vec<(EnemyId, EnemyKind)> = Vec::new();
        let mut ranged_casts: Vec<(EnemyId, [f32; 3], [f32; 3], ProjKind, i32, i32)> = Vec::new();
        for e in self.enemies.iter_mut().filter(|e| {
            !e.kind.is_friendly()
                && !terravibe_sim::enemy::is_eater_of_worlds(e.kind)
                && !terravibe_sim::enemy::is_bone_serpent(e.kind)
                && e.kind != EnemyKind::SkeletronHead
                && e.kind != EnemyKind::SkeletronHand
                && e.kind != EnemyKind::WallOfFleshMouth
                && e.kind != EnemyKind::WallOfFleshEye
                && e.kind != EnemyKind::TheHungry
        }) {
            let nearest = players
                .iter()
                .min_by(|a, b| dist2(**a, e.pos).total_cmp(&dist2(**b, e.pos)));
            if let Some(&t) = nearest {
                if !self.cfg.demo_enemies && terravibe_sim::enemy::tick_dawn_retreat(e, t, is_day) {
                    dawn_gone.push(e.id);
                    continue;
                }
                if e.kind == EnemyKind::Slime {
                    let wq = WorldQ(&self.world);
                    let is_surface = wq
                        .surface_y(e.pos[0].floor() as i32, e.pos[2].floor() as i32)
                        .is_some_and(|sy| e.pos[1] >= (sy - 20) as f32);
                    let provoked = e.hp < e.kind.stats().hp || e.hurt_ticks > 0;
                    e.ai[2] = i32::from(is_day && is_surface && !provoked);
                }
                if !(is_day
                    && !self.cfg.demo_enemies
                    && matches!(e.kind, EnemyKind::DemonEye | EnemyKind::Zombie))
                {
                    tick_enemy(&WorldQ(&self.world), e, t);
                }
                if e.ai[3] == 1 {
                    e.ai[3] = 0;
                    telegraphs.push((e.id, e.kind));
                }
                if e.ai[1] == 1 {
                    match e.kind {
                        EnemyKind::DarkCaster => {
                            e.ai[1] = 0;
                            ranged_casts.push((e.id, e.pos, t, ProjKind::Spark, 20, 120));
                        }
                        EnemyKind::FireImp => {
                            ranged_casts.push((e.id, e.pos, t, ProjKind::Fireball, 30, 180));
                        }
                        EnemyKind::Demon | EnemyKind::VoodooDemon => {
                            ranged_casts.push((e.id, e.pos, t, ProjKind::DemonScythe, 22, 180));
                        }
                        _ => {}
                    }
                }
            }
            if e.kind != EnemyKind::EyeOfCthulhu {
                bounds.clamp_body(&mut e.pos, &mut e.vel, e.kind.stats().half[0]);
            }
        }
        if !dawn_gone.is_empty() {
            self.enemies.retain(|e| !dawn_gone.contains(&e.id));
        }
        for (id, kind) in telegraphs {
            self.send_all_players(ServerMsg::EnemyTelegraph { id, kind });
        }
        for (id, pos, t, kind, damage, ttl) in ranged_casts {
            if self.projectiles.len() < MAX_PROJECTILES {
                let v = [t[0] - pos[0], t[1] + 0.8 - pos[1], t[2] - pos[2]];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if l > 1e-4 {
                    let speed = if kind == ProjKind::DemonScythe {
                        0.06
                    } else {
                        0.25
                    };
                    let vel = [v[0] / l * speed, v[1] / l * speed, v[2] / l * speed];
                    let next_id = self.next_proj_id;
                    self.next_proj_id += 1;
                    self.projectiles.push(Projectile {
                        id: next_id,
                        pos: [pos[0], pos[1] + 1.0, pos[2]],
                        vel,
                        kind,
                        owner: u32::MAX,
                        ttl,
                        damage,
                        knock_back: 1.0,
                        crit: 0,
                        ai0: 0,
                        pierced: 0,
                        origin: [pos[0], pos[1] + 1.0, pos[2]],
                        armor_penetration: 0,
                    });
                    if let Some(e) = self.enemies.iter_mut().find(|e| e.id == id) {
                        e.ai[1] = 0;
                    }
                }
            }
        }
        if !players.is_empty() {
            let nearest_to = |pos: [f32; 3]| -> [f32; 3] {
                *players
                    .iter()
                    .min_by(|a, b| dist2(**a, pos).total_cmp(&dist2(**b, pos)))
                    .unwrap()
            };
            let eow_head = self
                .enemies
                .iter()
                .find(|e| e.kind == EnemyKind::EaterOfWorldsHead)
                .or_else(|| {
                    self.enemies
                        .iter()
                        .find(|e| terravibe_sim::enemy::is_eater_of_worlds(e.kind))
                })
                .map(|e| e.pos);
            if let Some(head_pos) = eow_head {
                let target = nearest_to(head_pos);
                let any_corrupt = players
                    .iter()
                    .any(|p| is_pos_in_corruption(&self.world, self.seed, self.cfg.size, *p));
                if any_corrupt {
                    self.eow_out_of_biome_ticks = 0;
                    terravibe_sim::enemy::tick_eater_of_worlds(&mut self.enemies, target);
                } else {
                    self.eow_out_of_biome_ticks += 1;
                    if self.eow_out_of_biome_ticks >= 300 {
                        self.eow_out_of_biome_ticks = 0;
                        self.enemies
                            .retain(|e| !terravibe_sim::enemy::is_eater_of_worlds(e.kind));
                        self.send_all_players(ServerMsg::BossGone);
                    } else {
                        terravibe_sim::enemy::tick_eater_of_worlds(&mut self.enemies, target);
                    }
                }
            } else {
                self.eow_out_of_biome_ticks = 0;
            }
            if let Some(bs_pos) = self
                .enemies
                .iter()
                .find(|e| e.kind == EnemyKind::BoneSerpentHead)
                .or_else(|| {
                    self.enemies
                        .iter()
                        .find(|e| terravibe_sim::enemy::is_bone_serpent(e.kind))
                })
                .map(|e| e.pos)
            {
                terravibe_sim::enemy::tick_bone_serpent(&mut self.enemies, nearest_to(bs_pos));
            }
            if let Some(sk_pos) = self
                .enemies
                .iter()
                .find(|e| e.kind == EnemyKind::SkeletronHead)
                .map(|e| e.pos)
            {
                terravibe_sim::enemy::tick_skeletron_with_day(
                    &mut self.enemies,
                    nearest_to(sk_pos),
                    is_day,
                );
            }
        } else {
            let had_skeletron = self
                .enemies
                .iter()
                .any(|e| e.kind == EnemyKind::SkeletronHead || e.kind == EnemyKind::SkeletronHand);
            let had_eow = self
                .enemies
                .iter()
                .any(|e| terravibe_sim::enemy::is_eater_of_worlds(e.kind));
            if had_skeletron || had_eow {
                self.enemies.retain(|e| {
                    e.kind != EnemyKind::SkeletronHead
                        && e.kind != EnemyKind::SkeletronHand
                        && !terravibe_sim::enemy::is_eater_of_worlds(e.kind)
                });
                self.send_all_players(ServerMsg::BossGone);
            }
            if had_skeletron
                && !self.downed_skeletron
                && !self.facts.downed_skeletron
                && !self
                    .enemies
                    .iter()
                    .any(|e| e.kind == EnemyKind::OldMan && e.hp > 0)
            {
                let pos =
                    terravibe_sim::worldgen::dungeon_entrance_pos(self.seed, self.bounds.size());
                self.add_enemy(EnemyKind::OldMan, pos);
            }
        }
        if !players.is_empty() {
            use terravibe_sim::enemy::CRITTER_DESPAWN_DIST;
            let gone: Vec<EnemyId> = self
                .enemies
                .iter()
                .filter(|e| {
                    if e.kind.is_critter() {
                        return players
                            .iter()
                            .all(|&p| dist2(p, e.pos) > CRITTER_DESPAWN_DIST.powi(2));
                    }
                    e.kind != EnemyKind::EyeOfCthulhu
                        && e.kind != EnemyKind::SkeletronHead
                        && e.kind != EnemyKind::SkeletronHand
                        && e.kind != EnemyKind::WallOfFleshMouth
                        && e.kind != EnemyKind::WallOfFleshEye
                        && e.kind != EnemyKind::TheHungry
                        && e.kind != EnemyKind::Leech
                        && e.kind != EnemyKind::DungeonGuardian
                        && !terravibe_sim::enemy::is_eater_of_worlds(e.kind)
                        && !terravibe_sim::enemy::is_bone_serpent(e.kind)
                        && !e.kind.is_friendly()
                        && players
                            .iter()
                            .all(|&p| dist2(p, e.pos) > ENEMY_RADIUS.powi(2))
                })
                .map(|e| e.id)
                .collect();
            self.enemies.retain(|e| !gone.contains(&e.id));
        }
        self.contact_damage();
        self.stream_enemies();
    }

    /// Town NPC logic: steering every tick; house scan, move-in, residents and
    /// Merchant arrival on timers (2 s scan, 7200-tick arrival check).
    fn tick_towns(&mut self, players: &[[f32; 3]]) {
        let tick = self.clock.tick;
        if self.cfg.demo_npc {
            if !self.demo_done && !players.is_empty() {
                let [sx, _, sz] = self.spawn;
                let (x, z) = (sx.floor() as i32, sz.floor() as i32 - 4);
                if let Some(y) = WorldQ(&self.world).surface_y(x, z) {
                    self.add_enemy(
                        EnemyKind::Guide,
                        [x as f32 + 0.5, y as f32 + 1.0, z as f32 + 0.5],
                    );
                    self.demo_done = true;
                }
            }
            return; // frozen for the screenshot
        }
        if !players.is_empty() && tick % 120 == 0 {
            self.scan_houses();
            // Demo and test servers without natural spawns stay NPC-free.
            if self.cfg.natural_spawns {
                self.ensure_residents();
            }
            self.move_in();
            if self.cfg.natural_spawns && tick % town_npc::SPAWN_CHECK_TICKS == 0 {
                self.respawn_dead_town_npcs();
                self.maybe_spawn_merchant();
                self.maybe_spawn_arms_dealer();
            }
        }
        let day = self.clock.is_day();
        let (spawn, houses, bounds) = (self.spawn, &self.houses, self.bounds);
        let world = WorldQ(&self.world);
        for e in self.enemies.iter_mut().filter(|e| e.kind.is_friendly()) {
            let (anchor, has_home) = if e.kind == EnemyKind::OldMan {
                (
                    terravibe_sim::worldgen::dungeon_entrance_pos(self.seed, self.bounds.size()),
                    true,
                )
            } else {
                let home = TownKind::from_enemy(e.kind)
                    .and_then(|k| houses.iter().find(|h| h.npc == Some(k)))
                    .map(|h| h.home);
                (
                    home.map_or(spawn, |h| {
                        [h[0] as f32 + 0.5, h[1] as f32, h[2] as f32 + 0.5]
                    }),
                    home.is_some(),
                )
            };
            let st = self.town_ai.entry(e.id).or_default();
            town_npc::tick_town(&world, e, st, &mut self.rng, anchor, has_home, day);
            bounds.clamp_body(&mut e.pos, &mut e.vel, e.kind.stats().half[0]);
        }
    }

    /// Talk and shop requests. Everything is validated here: the NPC must be alive
    /// and in reach (`interact::in_reach`, gameplay spec range), the player alive.
    fn process_town_reqs(&mut self) {
        for i in 0..self.conns.len() {
            let reqs = std::mem::take(&mut self.conns[i].town_reqs);
            if reqs.is_empty() {
                continue;
            }
            let npcs: Vec<(EnemyId, EnemyKind, [f32; 3])> = self
                .enemies
                .iter()
                .filter(|e| e.hp > 0 && e.kind.is_friendly())
                .map(|e| (e.id, e.kind, e.pos))
                .collect();
            let near = |c: &Conn, id: EnemyId| {
                let (_, k, pos) = npcs.iter().find(|n| n.0 == id)?;
                let p = c.player.as_ref().filter(|p| !p.dead)?;
                let cell = [
                    pos[0].floor() as i32,
                    pos[1] as i32 + 1,
                    pos[2].floor() as i32,
                ];
                terravibe_sim::interact::in_reach(&p.body, cell).then_some(*k)
            };
            let mut summon_skeletron = false;
            let mut despawn_old_man: Option<EnemyId> = None;
            let mut assign_reqs: Vec<(TownKind, [i32; 3])> = Vec::new();
            let mut guide_leftovers: Vec<(terravibe_sim::inventory::ItemStack, [f32; 3])> =
                Vec::new();
            for m in reqs {
                let c = &mut self.conns[i];
                let msgs: Vec<ServerMsg> = match m {
                    ClientMsg::TalkNpc { id } => {
                        let talked_kind = near(c, id);
                        if talked_kind != Some(EnemyKind::Guide) && c.guide_slot.is_some() {
                            guide_leftovers.extend(return_guide_slot(c));
                            c.dead |= c.io.send(ServerMsg::GuideSlot(None)).is_err();
                        }
                        match talked_kind {
                            Some(EnemyKind::OldMan) => {
                                c.shop_npc = None;
                                if self.clock.is_day() {
                                    vec![ServerMsg::Dialogue {
                                    npc: id,
                                    text: "Old Man: My master cannot be summoned by the light of day... Return at night if you wish to enter without a curse.".to_string(),
                                }]
                                } else {
                                    despawn_old_man = Some(id);
                                    summon_skeletron = true;
                                    Vec::new()
                                }
                            }
                            Some(k) => match TownKind::from_enemy(k) {
                                Some(tk) => {
                                    let npc_pos = npcs
                                        .iter()
                                        .find(|n| n.0 == id)
                                        .map(|n| n.2)
                                        .unwrap_or(self.spawn);
                                    let biome = terravibe_sim::worldgen::biome::dominant(
                                        &terravibe_sim::worldgen::biome::biome_weights_for_size(
                                            self.seed,
                                            npc_pos[0].floor() as i32,
                                            npc_pos[2].floor() as i32,
                                            self.bounds.size(),
                                        ),
                                    );
                                    let dctx = terravibe_sim::shop::DialogueContext {
                                        weather: self.weather.kind,
                                        is_day: self.clock.is_day(),
                                        biome,
                                        facts: self.facts,
                                    };
                                    let text = if tk == TownKind::Dryad {
                                        let mut corrupt = 0u32;
                                        let mut total = 0u32;
                                        let half = self.bounds.half;
                                        let step = (half * 2 / 32).max(8);
                                        let mut x = -half;
                                        while x <= half {
                                            let mut z = -half;
                                            while z <= half {
                                                total += 1;
                                                if terravibe_sim::worldgen::corruption::corruption_at_for_size(
                                                    self.seed,
                                                    x,
                                                    z,
                                                    self.cfg.size,
                                                ) > 0.0
                                                {
                                                    corrupt += 1;
                                                }
                                                z += step;
                                            }
                                            x += step;
                                        }
                                        let pct = (corrupt * 100) / total.max(1);
                                        format!(
                                            "{}: {} Your world is {}% corrupt.",
                                            tk.name(),
                                            terravibe_sim::shop::town_dialogue(
                                                tk, &dctx, &mut c.rng
                                            ),
                                            pct
                                        )
                                    } else {
                                        format!(
                                            "{}: {}",
                                            tk.name(),
                                            terravibe_sim::shop::town_dialogue(
                                                tk, &dctx, &mut c.rng
                                            )
                                        )
                                    };
                                    let mut v = vec![ServerMsg::Dialogue { npc: id, text }];
                                    c.shop_npc = None;
                                    if !terravibe_sim::shop::shop_stock(tk).is_empty() {
                                        c.shop_npc = Some(id);
                                        v.push(ServerMsg::ShopContents {
                                            items: terravibe_sim::shop::shop_stock(tk)
                                                .iter()
                                                .map(|s| (s.item, s.price_copper))
                                                .collect(),
                                        });
                                    }
                                    v
                                }
                                None => Vec::new(),
                            },
                            None => Vec::new(),
                        }
                    }
                    ClientMsg::HealAtNurse { npc } => {
                        if near(c, npc) == Some(EnemyKind::Nurse)
                            && let Some(p) = c.player.as_mut()
                        {
                            let missing = (p.vitals.life_max - p.vitals.life).max(0);
                            let debuffs = u32::from(p.vitals.burn_ticks > 0);
                            let cost = terravibe_sim::shop::nurse_heal_cost(missing, debuffs);
                            if cost > 0 && terravibe_sim::coin::pay(&mut c.inv, cost) {
                                p.vitals.life = p.vitals.life_max;
                                p.vitals.extinguish();
                                vec![
                                    ServerMsg::NurseHealed {
                                        ok: true,
                                        healed: missing,
                                        cost_copper: cost,
                                    },
                                    ServerMsg::Inventory(c.inv.clone()),
                                    ServerMsg::PlayerState {
                                        ack: 0,
                                        body: p.body.clone(),
                                        vitals: p.vitals.clone(),
                                        dead: p.dead,
                                        respawn_ticks: p.respawn_ticks,
                                    },
                                ]
                            } else {
                                vec![ServerMsg::NurseHealed {
                                    ok: false,
                                    healed: 0,
                                    cost_copper: cost,
                                }]
                            }
                        } else {
                            vec![ServerMsg::NurseHealed {
                                ok: false,
                                healed: 0,
                                cost_copper: 0,
                            }]
                        }
                    }
                    ClientMsg::ReforgeItem { npc, slot } => {
                        if near(c, npc) == Some(EnemyKind::GoblinTinkerer) {
                            if let Some((new_prefix, cost_copper)) =
                                terravibe_sim::shop::reforge_slot(
                                    &mut c.inv,
                                    slot as usize,
                                    &mut c.rng,
                                )
                            {
                                vec![
                                    ServerMsg::ReforgeResult {
                                        ok: true,
                                        slot,
                                        new_prefix,
                                        cost_copper,
                                    },
                                    ServerMsg::Inventory(c.inv.clone()),
                                ]
                            } else {
                                vec![ServerMsg::ReforgeResult {
                                    ok: false,
                                    slot,
                                    new_prefix: Prefix(0),
                                    cost_copper: 0,
                                }]
                            }
                        } else {
                            vec![ServerMsg::ReforgeResult {
                                ok: false,
                                slot,
                                new_prefix: Prefix(0),
                                cost_copper: 0,
                            }]
                        }
                    }
                    ClientMsg::ShopBuy { npc, index, count } => {
                        let (ok, cannot_afford) = match near(c, npc).and_then(TownKind::from_enemy)
                        {
                            Some(k) => {
                                let bought = terravibe_sim::shop::buy_many(
                                    &mut c.inv,
                                    k,
                                    index as usize,
                                    count,
                                );
                                let cannot_afford = bought == 0
                                    && terravibe_sim::shop::shop_stock(k)
                                        .get(index as usize)
                                        .is_some_and(|s| {
                                            terravibe_sim::coin::total_copper(&c.inv)
                                                < s.price_copper
                                        });
                                (bought > 0, cannot_afford)
                            }
                            None => (false, false),
                        };
                        vec![ServerMsg::ShopResult { ok, cannot_afford }]
                    }
                    ClientMsg::ShopSell { slot } => {
                        let ok = (slot as usize) < terravibe_sim::inventory::BAG
                            && c.shop_npc
                                .and_then(|id| near(c, id))
                                .and_then(TownKind::from_enemy)
                                .is_some()
                            && terravibe_sim::shop::sell(&mut c.inv, slot as usize);
                        vec![ServerMsg::ShopResult {
                            ok,
                            cannot_afford: false,
                        }]
                    }
                    ClientMsg::GuideAdvice => {
                        let guide_id = self
                            .enemies
                            .iter()
                            .find(|e| e.hp > 0 && e.kind == EnemyKind::Guide)
                            .map(|e| e.id)
                            .unwrap_or(EnemyId(0));
                        let char_facts = c.character_facts;
                        let advice =
                            terravibe_sim::facts::current_guide_advice(&self.facts, &char_facts);
                        vec![ServerMsg::Dialogue {
                            npc: guide_id,
                            text: format!("Guide: {advice}"),
                        }]
                    }
                    ClientMsg::AssignHousing { npc, pos } => {
                        assign_reqs.push((npc, pos));
                        Vec::new()
                    }
                    _ => Vec::new(),
                };
                for m in msgs {
                    c.dead |= c.io.send(m).is_err();
                }
            }
            for (npc, pos) in assign_reqs {
                self.assign_housing_at(i, npc, pos);
            }
            for (stack, pos) in guide_leftovers {
                self.add_drop(stack, pos);
            }
            if let Some(id) = despawn_old_man {
                self.enemies.retain(|e| e.id != id);
            }
            if summon_skeletron {
                let entrance =
                    terravibe_sim::worldgen::dungeon_entrance_pos(self.seed, self.bounds.size());
                self.spawn_skeletron(entrance);
            }
        }
    }

    /// Validates a room at `pos` and assigns `npc` to it, teleporting the NPC
    /// and emitting `HouseStatus`, `TownNpcInfo`, and `NpcSpeech` on success.
    fn assign_housing_at(&mut self, conn_idx: usize, npc: TownKind, pos: [i32; 3]) {
        use terravibe_sim::housing::{check_room, status};
        self.room_checks += 1;
        let r = check_room(&WorldQ(&self.world), pos);
        let (ok, reason) = status(&r);
        let mut out = vec![ServerMsg::HouseStatus { ok, reason }];
        if let Ok(room) = r
            && ok
        {
            let mut evicted: Option<TownKind> = None;
            for h in &mut self.houses {
                if h.npc == Some(npc) {
                    h.npc = None;
                }
            }
            let home_cell = if let Some(h) = self.houses.iter_mut().find(|h| h.origin == room.lo) {
                if h.npc != Some(npc) {
                    evicted = h.npc;
                }
                h.npc = Some(npc);
                h.home
            } else {
                let mut chair = pos;
                let w = WorldQ(&self.world);
                'find_chair: for x in room.lo[0]..=room.lo[0] + room.bbox[0] {
                    for y in room.lo[1]..=room.lo[1] + room.bbox[1] {
                        for z in room.lo[2]..=room.lo[2] + room.bbox[2] {
                            if w.block(x, y, z) == terravibe_sim::CHAIR {
                                chair = [x, y, z];
                                break 'find_chair;
                            }
                        }
                    }
                }
                self.houses.push(House {
                    origin: room.lo,
                    home: chair,
                    npc: Some(npc),
                });
                chair
            };
            if npc == TownKind::Merchant {
                self.facts.merchant_housed = true;
            }
            if let Some(ev) = evicted
                && !self.houses.iter().any(|h| h.npc == Some(ev))
                && let Some(e) = self.enemies.iter().find(|e| e.kind == ev.enemy_kind())
            {
                out.push(ServerMsg::TownNpcInfo {
                    id: e.id,
                    homeless: true,
                });
            }
            if let Some(e) = self.enemies.iter_mut().find(|e| e.kind == npc.enemy_kind()) {
                e.pos = [
                    home_cell[0] as f32 + 0.5,
                    home_cell[1] as f32,
                    home_cell[2] as f32 + 0.5,
                ];
                e.vel = [0.0; 3];
                out.push(ServerMsg::TownNpcInfo {
                    id: e.id,
                    homeless: false,
                });
                out.push(ServerMsg::NpcSpeech {
                    id: e.id,
                    text: format!("{}: This will do nicely!", npc.name()),
                });
            }
        }
        let c = &mut self.conns[conn_idx];
        for m in out {
            c.dead |= c.io.send(m).is_err();
        }
    }

    fn town_alive(&self, k: TownKind) -> bool {
        self.enemies
            .iter()
            .any(|e| e.kind == k.enemy_kind() && e.hp > 0)
    }

    /// Finds valid rooms (a chair in a walled chunk whose room passes
    /// `room_has_needs`) and drops houses that stopped being valid. A house whose
    /// surroundings are unloaded is kept.
    fn scan_houses(&mut self) {
        use terravibe_sim::housing::{RoomFail, check_room, room_has_needs};
        let w = WorldQ(&self.world);
        self.houses.retain(|h| {
            self.room_checks += 1;
            match check_room(&w, h.home) {
                Ok(r) => room_has_needs(&r) && r.lo == h.origin,
                Err(RoomFail::Unloaded) => true,
                Err(_) => false,
            }
        });
        let mut found = Vec::new();
        for cp in &self.edited {
            let Some(ch) = self.world.get(cp).filter(|c| c.has_walls()) else {
                continue;
            };
            for lx in 0..CHUNK_SIZE as usize {
                for ly in 0..CHUNK_SIZE as usize {
                    for lz in 0..CHUNK_SIZE as usize {
                        if ch.get(lx, ly, lz) != terravibe_sim::CHAIR {
                            continue;
                        }
                        let cell = [
                            cp.x * CHUNK_SIZE + lx as i32,
                            cp.y * CHUNK_SIZE + ly as i32,
                            cp.z * CHUNK_SIZE + lz as i32,
                        ];
                        if self.houses.iter().any(|h| h.home == cell)
                            || found.iter().any(|h: &House| h.home == cell)
                        {
                            continue;
                        }
                        self.room_checks += 1;
                        if let Ok(r) = check_room(&w, cell)
                            && room_has_needs(&r)
                            && !self.houses.iter().any(|h| h.origin == r.lo)
                            && !found.iter().any(|h: &House| h.origin == r.lo)
                        {
                            found.push(House {
                                origin: r.lo,
                                home: cell,
                                npc: None,
                            });
                        }
                    }
                }
            }
        }
        self.houses.extend(found);
    }

    fn spawn_town(&mut self, k: TownKind, pos: [f32; 3]) {
        self.add_enemy(k.enemy_kind(), pos);
        self.dead_town_npcs.remove(&k);
        if k == TownKind::Guide {
            self.guide_spawned = true;
        }
    }

    /// The Guide exists from world start (spawned at the world spawn, or at his
    /// house); residents saved in `towns.bin` are hydrated at their homes when their
    /// chunks load. Dead town NPCs return during daytime, boss-free spawn checks.
    fn ensure_residents(&mut self) {
        let wq = WorldQ(&self.world);
        let mut todo: Vec<(TownKind, [f32; 3])> = Vec::new();
        for h in &self.houses {
            if let Some(k) = h.npc
                && !self.town_alive(k)
                && !self.dead_town_npcs.contains(&k)
                && wq.is_loaded(h.home[0], h.home[1], h.home[2])
            {
                todo.push((
                    k,
                    [
                        h.home[0] as f32 + 0.5,
                        h.home[1] as f32,
                        h.home[2] as f32 + 0.5,
                    ],
                ));
            }
        }
        let guide_housed = self.houses.iter().any(|h| h.npc == Some(TownKind::Guide));
        if !self.town_alive(TownKind::Guide)
            && !todo.iter().any(|t| t.0 == TownKind::Guide)
            && !self.dead_town_npcs.contains(&TownKind::Guide)
            && !guide_housed
            && !self.guide_spawned
        {
            let [sx, _, sz] = self.spawn;
            if let Some(y) = wq.surface_y(sx.floor() as i32, sz.floor() as i32) {
                todo.push((TownKind::Guide, [sx, y as f32 + 1.0, sz]));
            }
        }
        for (k, pos) in todo {
            self.spawn_town(k, pos);
        }
        let skeletron_alive = self
            .enemies
            .iter()
            .any(|e| e.kind == EnemyKind::SkeletronHead && e.hp > 0);
        if !self.downed_skeletron
            && !skeletron_alive
            && self.clock.is_day()
            && !self
                .enemies
                .iter()
                .any(|e| e.kind == EnemyKind::OldMan && e.hp > 0)
        {
            let pos = terravibe_sim::worldgen::dungeon_entrance_pos(self.seed, self.bounds.size());
            self.add_enemy(EnemyKind::OldMan, pos);
        }
    }

    fn respawn_dead_town_npcs(&mut self) {
        if !self.clock.is_day() || self.any_boss_alive() || self.dead_town_npcs.is_empty() {
            return;
        }
        let wq = WorldQ(&self.world);
        let mut todo: Vec<(TownKind, [f32; 3])> = Vec::new();
        let pending: Vec<TownKind> = self.dead_town_npcs.iter().copied().collect();
        for k in pending {
            if self.town_alive(k) {
                self.dead_town_npcs.remove(&k);
                continue;
            }
            if let Some(h) = self
                .houses
                .iter()
                .find(|h| h.npc == Some(k) && wq.is_loaded(h.home[0], h.home[1], h.home[2]))
            {
                todo.push((
                    k,
                    [
                        h.home[0] as f32 + 0.5,
                        h.home[1] as f32,
                        h.home[2] as f32 + 0.5,
                    ],
                ));
            } else if k == TownKind::Guide
                && !self.houses.iter().any(|h| h.npc == Some(TownKind::Guide))
            {
                let [sx, _, sz] = self.spawn;
                if let Some(y) = wq.surface_y(sx.floor() as i32, sz.floor() as i32) {
                    todo.push((TownKind::Guide, [sx, y as f32 + 1.0, sz]));
                }
            }
        }
        for (k, pos) in todo {
            self.spawn_town(k, pos);
        }
    }

    /// Homeless NPCs take the nearest vacant valid house and walk (teleport, as
    /// `AI_007_TownEntities_TeleportToHome`, gameplay spec) to it.
    fn move_in(&mut self) {
        let homeless: Vec<(TownKind, [f32; 3])> = self
            .enemies
            .iter()
            .filter_map(|e| Some((TownKind::from_enemy(e.kind)?, e.pos)))
            .collect();
        for (k, i) in town_npc::assign_homes(&mut self.houses, &homeless) {
            let h = self.houses[i].home;
            if k == TownKind::Merchant {
                self.facts.merchant_housed = true;
            }
            if let Some(e) = self.enemies.iter_mut().find(|e| e.kind == k.enemy_kind()) {
                e.pos = [h[0] as f32 + 0.5, h[1] as f32, h[2] as f32 + 0.5];
                e.vel = [0.0; 3];
            }
        }
    }

    /// Merchant arrival (`Main.town NPC spawn check`, gameplay spec).
    fn maybe_spawn_merchant(&mut self) {
        let copper = self
            .conns
            .iter()
            .filter(|c| c.player.is_some())
            .map(|c| {
                let held = c.held.0.map_or(0u64, |st| {
                    terravibe_sim::coin::coin_unit_value(st.id) as u64 * st.count as u64
                });
                (terravibe_sim::coin::total_copper(&c.inv) as u64).saturating_add(held)
            })
            .sum::<u64>()
            .min(u32::MAX as u64) as u32;
        let vacant = self.houses.iter().find(|h| h.npc.is_none()).copied();
        if town_npc::merchant_may_spawn(
            copper,
            vacant.is_some(),
            self.town_alive(TownKind::Merchant),
            self.clock.is_day(),
            self.any_boss_alive(),
        ) && let Some(h) = vacant
        {
            let pos = [
                h.home[0] as f32 + 0.5,
                h.home[1] as f32,
                h.home[2] as f32 + 0.5,
            ];
            self.spawn_town(TownKind::Merchant, pos);
            self.facts.merchant_housed = true;
        }
    }

    fn maybe_spawn_arms_dealer(&mut self) {
        use terravibe_sim::item::{
            CLOCKWORK_ASSAULT_RIFLE, FLINTLOCK_PISTOL, HANDGUN, ItemId, MINISHARK, MUSKET,
            MUSKET_BALL, PHOENIX_BLASTER,
        };
        let is_gun_or_bullet = |id: ItemId| {
            matches!(
                id,
                MUSKET
                    | HANDGUN
                    | PHOENIX_BLASTER
                    | FLINTLOCK_PISTOL
                    | MINISHARK
                    | CLOCKWORK_ASSAULT_RIFLE
                    | MUSKET_BALL
            )
        };
        if !self.clock.is_day() || self.any_boss_alive() {
            return;
        }
        let has_gun_or_bullet = self.conns.iter().filter(|c| c.player.is_some()).any(|c| {
            c.inv
                .slots
                .iter()
                .flatten()
                .any(|st| is_gun_or_bullet(st.id))
                || c.held.0.is_some_and(|st| is_gun_or_bullet(st.id))
        });
        let vacant = self.houses.iter().find(|h| h.npc.is_none()).copied();
        if town_npc::arms_dealer_may_spawn(
            has_gun_or_bullet,
            self.town_alive(TownKind::ArmsDealer),
            vacant.is_some(),
        ) && let Some(h) = vacant
        {
            let pos = [
                h.home[0] as f32 + 0.5,
                h.home[1] as f32,
                h.home[2] as f32 + 0.5,
            ];
            self.spawn_town(TownKind::ArmsDealer, pos);
            return;
        }
        let merchant_alive = self.town_alive(TownKind::Merchant);
        let any_life_gt_100 = self
            .conns
            .iter()
            .filter_map(|c| c.player.as_ref())
            .any(|p| p.vitals.life_max > 100);
        if town_npc::nurse_may_spawn(
            any_life_gt_100,
            merchant_alive,
            self.town_alive(TownKind::Nurse),
            vacant.is_some(),
        ) && let Some(h) = vacant
        {
            let pos = [
                h.home[0] as f32 + 0.5,
                h.home[1] as f32,
                h.home[2] as f32 + 0.5,
            ];
            self.spawn_town(TownKind::Nurse, pos);
            return;
        }
        let is_explosive = |id: ItemId| {
            matches!(
                id,
                terravibe_sim::item::BOMB
                    | terravibe_sim::item::DYNAMITE
                    | terravibe_sim::item::GRENADE
            )
        };
        let has_explosive = self.conns.iter().any(|c| {
            c.inv.slots.iter().flatten().any(|st| is_explosive(st.id))
                || c.held.0.is_some_and(|st| is_explosive(st.id))
        });
        if town_npc::demolitionist_may_spawn(
            has_explosive,
            merchant_alive,
            self.town_alive(TownKind::Demolitionist),
            vacant.is_some(),
        ) && let Some(h) = vacant
        {
            let pos = [
                h.home[0] as f32 + 0.5,
                h.home[1] as f32,
                h.home[2] as f32 + 0.5,
            ];
            self.spawn_town(TownKind::Demolitionist, pos);
            return;
        }
        let any_boss_downed =
            self.facts.downed_eye || self.facts.downed_eow || self.facts.downed_skeletron;
        if town_npc::dryad_may_spawn(
            any_boss_downed,
            self.town_alive(TownKind::Dryad),
            vacant.is_some(),
        ) && let Some(h) = vacant
        {
            let pos = [
                h.home[0] as f32 + 0.5,
                h.home[1] as f32,
                h.home[2] as f32 + 0.5,
            ];
            self.spawn_town(TownKind::Dryad, pos);
            return;
        }
        let goblin_unlocked =
            self.facts.shadow_orbs_broken > 0 || self.facts.downed_eye || self.facts.downed_eow;
        if town_npc::goblin_tinkerer_may_spawn(
            goblin_unlocked,
            self.town_alive(TownKind::GoblinTinkerer),
            vacant.is_some(),
        ) && let Some(h) = vacant
        {
            let pos = [
                h.home[0] as f32 + 0.5,
                h.home[1] as f32,
                h.home[2] as f32 + 0.5,
            ];
            self.spawn_town(TownKind::GoblinTinkerer, pos);
        }
    }

    /// Enemy bodies hurt overlapping live players (`hurt_player` honours i-frames).
    fn tick_critter_spawner(&mut self, players: &[[f32; 3]]) {
        use terravibe_sim::enemy::MAX_CRITTERS;
        const CRITTER_SPAWN_INTERVAL: u32 = 120;
        const CRITTER_RING: (f32, f32) = (14.0, 42.0);
        const CANDIDATES: u32 = 12;

        if players.is_empty() {
            return;
        }
        self.critter_ticks += 1;
        if self.critter_ticks < CRITTER_SPAWN_INTERVAL {
            return;
        }
        self.critter_ticks = 0;

        let existing = self.enemies.iter().filter(|e| e.kind.is_critter()).count();
        if existing >= MAX_CRITTERS {
            return;
        }

        let wq = WorldQ(&self.world);
        for _ in 0..CANDIDATES {
            let p = players[self.critter_rng.below(players.len() as u32) as usize];
            let ang = ((self.critter_rng.next_u32() >> 8) as f32 / (1u32 << 24) as f32)
                * std::f32::consts::TAU;
            let r = CRITTER_RING.0
                + ((self.critter_rng.next_u32() >> 8) as f32 / (1u32 << 24) as f32)
                    * (CRITTER_RING.1 - CRITTER_RING.0);
            let x = (p[0] + ang.cos() * r).floor() as i32;
            let z = (p[2] + ang.sin() * r).floor() as i32;
            let Some(sy) = wq.surface_y(x, z) else {
                continue;
            };
            let p_y = p[1].floor() as i32;
            if p_y <= sy - 15 {
                for dy in -4..=4 {
                    let cy = p_y + dy;
                    if wq.block(x, cy - 1, z).is_solid()
                        && !wq.block(x, cy, z).is_solid()
                        && !wq.block(x, cy + 1, z).is_solid()
                    {
                        self.add_enemy(
                            EnemyKind::CaveCritter,
                            [x as f32 + 0.5, cy as f32, z as f32 + 0.5],
                        );
                        return;
                    }
                }
            } else if self.clock.is_day()
                && !wq.block(x, sy + 1, z).is_solid()
                && !wq.block(x, sy + 2, z).is_solid()
            {
                let kind = match self.critter_rng.below(3) {
                    0 => EnemyKind::Bunny,
                    1 => EnemyKind::Bird,
                    _ => EnemyKind::Squirrel,
                };
                let y = if kind == EnemyKind::Bird {
                    sy as f32 + 3.0
                } else {
                    sy as f32 + 1.0
                };
                self.add_enemy(kind, [x as f32 + 0.5, y, z as f32 + 0.5]);
                return;
            }
        }
    }

    fn contact_damage(&mut self) {
        let mut deaths = Vec::new();
        let mut combat_hits = Vec::new();
        for (cid, c) in self.conns.iter_mut().enumerate() {
            let Some(p) = c.player.as_mut().filter(|p| !p.dead) else {
                continue;
            };
            let (pp, mut ev) = (p.body.pos, Vec::new());
            for e in self
                .enemies
                .iter()
                .filter(|e| !e.kind.is_friendly() && !e.kind.is_critter())
            {
                let st = e.kind.stats();
                let hit = (e.pos[0] - pp[0]).abs() < st.half[0] + HALF_WIDTH
                    && (e.pos[2] - pp[2]).abs() < st.half[0] + HALF_WIDTH
                    && e.pos[1] < pp[1] + HEIGHT
                    && pp[1] < e.pos[1] + 2.0 * st.half[1];
                if hit
                    && hurt_player_with_cause(p, e.damage(), DeathCause::Contact(e.kind), &mut ev)
                {
                    let amount = damage_taken(e.damage(), p.defense());
                    let dx = pp[0] - e.pos[0];
                    let dz = pp[2] - e.pos[2];
                    let h = dx.hypot(dz);
                    let dir = if h > 1e-4 {
                        [dx / h, 0.55, dz / h]
                    } else {
                        [1.0, 0.55, 0.0]
                    };
                    p.apply_knockback(0.32, dir);
                    c.dead |= c.io.send(ServerMsg::PlayerHit { amount }).is_err();
                    combat_hits.push(ServerMsg::CombatHit {
                        pos: [pp[0], pp[1] + HEIGHT * 0.5, pp[2]],
                        attacker_pos: Some(e.pos),
                        amount,
                        crit: false,
                        damage_kind: DamageKind::Contact,
                        target_kind: HitTargetKind::Player,
                        killed: p.dead,
                    });
                    if p.dead {
                        deaths.push((cid, DeathCause::Contact(e.kind)));
                        break;
                    }
                }
            }
        }
        for msg in combat_hits {
            self.send_all_players(msg);
        }
        for (cid, cause) in deaths {
            self.on_player_killed(cid, cause);
        }
    }

    /// Per-connection spawn/despawn on the radius edge, plus 10 Hz state.
    fn stream_enemies(&mut self) {
        let state_tick = self.clock.tick % ENEMY_STATE_TICKS == 0;
        for c in &mut self.conns {
            // The enemies demo is also viewed by fly-camera spectators.
            let view = match c.player.as_ref().filter(|p| !p.dead) {
                Some(p) => Some(p.body.pos),
                None if self.cfg.demo_enemies => c.interest.centre,
                None => None,
            };
            let near: Vec<&Enemy> = match view {
                Some(v) => self
                    .enemies
                    .iter()
                    .filter(|e| {
                        e.kind != EnemyKind::EyeOfCthulhu
                            && (matches!(
                                e.kind,
                                EnemyKind::WallOfFleshMouth | EnemyKind::WallOfFleshEye
                            ) || dist2(v, e.pos) <= ENEMY_RADIUS.powi(2))
                    })
                    .collect(),
                None => Vec::new(),
            };
            let gone: Vec<EnemyId> = c
                .known_enemies
                .iter()
                .copied()
                .filter(|id| !near.iter().any(|e| e.id == *id))
                .collect();
            for id in gone {
                c.known_enemies.remove(&id);
                c.dead |= c.io.send(ServerMsg::EnemyDespawn { id }).is_err();
            }
            for e in near {
                let msg = if c.known_enemies.insert(e.id) {
                    if e.kind.is_friendly() {
                        let homeless = !self
                            .houses
                            .iter()
                            .any(|h| h.npc == TownKind::from_enemy(e.kind));
                        c.dead |=
                            c.io.send(ServerMsg::EnemySpawn {
                                id: e.id,
                                kind: e.kind,
                                pos: e.pos,
                            })
                            .is_err();
                        ServerMsg::TownNpcInfo { id: e.id, homeless }
                    } else {
                        ServerMsg::EnemySpawn {
                            id: e.id,
                            kind: e.kind,
                            pos: e.pos,
                        }
                    }
                } else if state_tick {
                    ServerMsg::EnemyState {
                        id: e.id,
                        pos: e.pos,
                        vel: e.vel,
                        hp: e.hp,
                        on_fire: e.is_on_fire(),
                    }
                } else {
                    continue;
                };
                c.dead |= c.io.send(msg).is_err();
            }
        }
    }

    /// Makes a freshly generated chunk resident and seeds its generated chests'
    /// loot, unless the position already has an entry (looted, saved or reloaded).
    pub fn install_chunk(&mut self, pos: ChunkPos, chunk: Arc<Chunk>) {
        if !self.bounds.chunk_inside(pos) {
            return;
        }
        if self.edited.contains(&pos) && self.world.contains_key(&pos) {
            return;
        }
        for (p, loot) in terravibe_sim::worldgen::chest_gen::chests_in_for_size(
            self.seed,
            pos,
            self.bounds.size(),
        ) {
            let fresh = !self.chests.contains_key(&p);
            self.chests
                .entry(p)
                .or_insert_with(|| ChestInv::from_stacks(&loot));
            if fresh {
                if terravibe_sim::worldgen::chest_gen::is_dungeon_chest(
                    self.seed,
                    p,
                    self.bounds.size(),
                ) {
                    self.locked_chests.insert(p);
                }
                if terravibe_sim::worldgen::chest_gen::is_underworld_chest(
                    self.seed,
                    p,
                    self.bounds.size(),
                ) {
                    self.shadow_chests.insert(p);
                }
            }
        }
        self.world.insert(pos, chunk);
        self.liquid_sim.resume_chunk(pos);
        self.wake_unsettled_liquid_around(pos);
    }

    /// Wakes only unsettled liquid cells in `pos` (those able to fall or spread into
    /// adjacent loaded non-solid space) and border liquid cells of loaded neighbours
    /// facing open non-solid cells in `pos`.
    fn wake_unsettled_liquid_around(&mut self, pos: ChunkPos) {
        let mut to_wake: Vec<[i32; 3]> = Vec::new();
        {
            let adapter = ServerLiquidWorld::new(&mut self.world);
            adapter.for_each_liquid_in_chunk(pos, |[x, y, z]| {
                let Some(cur) = adapter.liquid(x, y, z) else {
                    return;
                };
                let (b_loaded, b_block, b_liq) = adapter.cell_info(x, y - 1, z);
                let can_fall = b_loaded
                    && !b_block.is_solid()
                    && b_liq.is_none_or(|bl| {
                        bl.kind != cur.kind || bl.amount < terravibe_sim::liquid::AMOUNT_FULL
                    });
                if can_fall {
                    to_wake.push([x, y, z]);
                    return;
                }
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (n_loaded, n_block, n_liq) = adapter.cell_info(x + dx, y, z + dz);
                    if n_loaded && !n_block.is_solid() && n_liq.is_none_or(|nl| nl.kind != cur.kind)
                    {
                        to_wake.push([x, y, z]);
                        return;
                    }
                }
            });

            // Also check border faces of the 6 loaded neighbours touching `pos`.
            let bx = pos.x * CHUNK_SIZE;
            let by = pos.y * CHUNK_SIZE;
            let bz = pos.z * CHUNK_SIZE;
            // Neighbour at -Y/+Y
            for lz in 0..CHUNK_SIZE {
                for lx in 0..CHUNK_SIZE {
                    let wx = bx + lx;
                    let wz = bz + lz;
                    // Cell in pos+Y just above pos top face
                    let ty = by + CHUNK_SIZE;
                    if let (true, _, Some(_)) = adapter.cell_info(wx, ty, wz) {
                        let (_, b_block, b_liq) = adapter.cell_info(wx, ty - 1, wz);
                        if !b_block.is_solid()
                            && b_liq.is_none_or(|bl| bl.amount < terravibe_sim::liquid::AMOUNT_FULL)
                        {
                            to_wake.push([wx, ty, wz]);
                        }
                    }
                }
            }
            // Neighbours at -X, +X, -Z, +Z
            for ly in 0..CHUNK_SIZE {
                let wy = by + ly;
                for i in 0..CHUNK_SIZE {
                    let candidates = [
                        ([bx - 1, wy, bz + i], [bx, wy, bz + i]),
                        (
                            [bx + CHUNK_SIZE, wy, bz + i],
                            [bx + CHUNK_SIZE - 1, wy, bz + i],
                        ),
                        ([bx + i, wy, bz - 1], [bx + i, wy, bz]),
                        (
                            [bx + i, wy, bz + CHUNK_SIZE],
                            [bx + i, wy, bz + CHUNK_SIZE - 1],
                        ),
                    ];
                    for (nb, inner) in candidates {
                        if let (true, _, Some(nl)) = adapter.cell_info(nb[0], nb[1], nb[2]) {
                            let (_, ib, il) = adapter.cell_info(inner[0], inner[1], inner[2]);
                            if !ib.is_solid() && il.is_none_or(|l| l.kind != nl.kind) {
                                to_wake.push(nb);
                            }
                        }
                    }
                }
            }
        }
        for p in to_wake {
            self.liquid_sim.wake(p);
        }
    }

    fn stream_chunks(&mut self) {
        for (pos, chunk) in self.generator.drain() {
            self.install_chunk(pos, chunk);
        }
        let bounds = self.bounds;
        for c in &mut self.conns {
            for pos in c.interest.wanted() {
                if !bounds.chunk_inside(pos) || c.interest.sent.contains(&pos) {
                    continue;
                }
                if let Some(chunk) = self.world.get(&pos) {
                    let msg = ServerMsg::Chunk {
                        pos,
                        chunk: Arc::clone(chunk),
                    };
                    if c.io.send(msg).is_err() {
                        c.dead = true;
                        break;
                    }
                    c.interest.sent.insert(pos);
                } else if !self.generator.is_in_flight(pos) && !self.generator.request(pos) {
                    break; // at capacity; nearer chunks were queued first
                }
            }
            let far: Vec<ChunkPos> = c
                .interest
                .sent
                .iter()
                .copied()
                .filter(|&p| c.interest.too_far(p))
                .collect();
            for pos in far {
                c.interest.sent.remove(&pos);
                c.dead |= c.io.send(ServerMsg::Unload { pos }).is_err();
            }
        }
        self.reap();
        // Forget chunks nobody is near any more, except chunks with edits:
        // those persist for the session so the world never reverts.
        let conns = &self.conns;
        let edited = &self.edited;
        self.world
            .retain(|&p, _| edited.contains(&p) || conns.iter().any(|c| !c.interest.too_far(p)));
    }
}

/// Puts the cursor stack back in the bag; the leftover (bag full) with the
/// player's position, to be dropped in the world.
fn return_held(c: &mut Conn) -> Option<(terravibe_sim::inventory::ItemStack, [f32; 3])> {
    let left = slots::return_held(&mut c.inv, &mut c.held)?;
    Some((left, c.player.as_ref()?.body.pos))
}

/// Puts the Guide crafting lookup stack back in the bag; the leftover (bag full) with the
/// player's position, to be dropped in the world.
fn return_guide_slot(c: &mut Conn) -> Option<(terravibe_sim::inventory::ItemStack, [f32; 3])> {
    let stack = c.guide_slot.take()?;
    let left = c.inv.add(stack)?;
    Some((left, c.player.as_ref()?.body.pos))
}

fn player_save(c: &Conn) -> Option<PlayerSave> {
    let p = c.player.as_ref()?;
    let mut inv = c.inv.clone();
    if let Some(held) = c.held.0 {
        inv.add(held);
    }
    Some(PlayerSave {
        inventory: inv,
        armor: p.armor,
        life_max: p.vitals.life_max,
        life: p.vitals.life,
        pos: p.body.pos,
        potion_delay: p.vitals.potion_delay_ticks,
    })
}

fn flush_character_state(c: &mut Conn) {
    if let (Some(char_data), Some(pl)) = (&mut c.character, &c.player) {
        char_data.inventory = c.inv.clone();
        char_data.armor = pl.armor;
        char_data.accessories = pl.accessories;
        char_data.accessory_prefixes = pl.accessory_prefixes;
        char_data.vanity_armor = pl.vanity_armor;
        char_data.vanity_accessories = pl.vanity_accessories;
        char_data.vanity_accessory_prefixes = pl.vanity_accessory_prefixes;
        char_data.life_max = pl.vitals.life_max;
        char_data.life = pl.vitals.life;
        char_data.mana_max = pl.vitals.mana_max;
        char_data.mana = pl.vitals.mana;
        char_data.potion_delay = pl.vitals.potion_delay_ticks as i32;
        char_data.obsidian_skin_ticks = pl.vitals.obsidian_skin_ticks;
        char_data.facts = c.character_facts;
        let bytes = char_data.to_bytes();
        let _ = c.io.send(ServerMsg::CharacterState(bytes));
    }
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    let len = a.len().max(b.len());
    for i in 0..len {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= (x ^ y) as usize;
    }
    diff == 0
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}

/// Runs a `Server` at 60 Hz on its own thread; returns the connected client.
pub fn spawn_local(seed: u32) -> (InProcClient, std::thread::JoinHandle<()>) {
    spawn_local_cfg(seed, ServerCfg::default())
}

pub fn spawn_local_cfg(seed: u32, cfg: ServerCfg) -> (InProcClient, std::thread::JoinHandle<()>) {
    let (client, conn) = in_process_pair();
    let handle = std::thread::spawn(move || {
        let mut server = Server::with_cfg(seed, cfg);
        server.connect(Box::new(conn));
        let step = Duration::from_secs_f64(1.0 / 60.0);
        let mut next = Instant::now();
        while server.connection_count() > 0 {
            server.tick();
            next += step;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            } else if now - next > step * 10 {
                next = now; // fell far behind; don't spiral
            }
        }
        if let Err(e) = server.save_now() {
            eprintln!("terravibe: final save failed: {e}");
        }
    });
    (client, handle)
}

/// Runs a multi-player `Server` at 60 Hz on its own thread with a local host client
/// (trusted/exempt from password & `max_players`) and accepts remote connections from `incoming_conns`.
/// When the host disconnects or `shutdown` is set, broadcasts `RejectReason::HostClosing`,
/// flushes `CharacterState` to remaining guests, saves the world, and exits cleanly.
pub fn spawn_host_cfg(
    seed: u32,
    cfg: ServerCfg,
    incoming_conns: std::sync::mpsc::Receiver<Box<dyn ServerConn>>,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> (InProcClient, std::thread::JoinHandle<()>) {
    let (client, conn) = in_process_pair();
    let handle = std::thread::spawn(move || {
        let mut server = Server::with_cfg(seed, cfg);
        let host_id = server.connect_host(Box::new(conn));
        let step = Duration::from_secs_f64(1.0 / 60.0);
        let mut next = Instant::now();
        loop {
            while let Ok(incoming) = incoming_conns.try_recv() {
                server.connect(incoming);
            }
            if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                server.shutdown_host();
                break;
            }
            server.tick();
            if !server.has_conn(host_id) || shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                server.shutdown_host();
                break;
            }
            next += step;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            } else if now - next > step * 10 {
                next = now; // fell far behind; don't spiral
            }
        }
        if let Err(e) = server.save_now() {
            eprintln!("terravibe: final save failed: {e}");
        }
    });
    (client, handle)
}

#[cfg(test)]
mod tests {
    #[test]
    fn eater_spawn_world_uses_seed_and_candidate_coordinates() {
        use terravibe_sim::worldgen::corruption::corruption_at_for_size;
        let world = HashMap::new();
        let mut seed_changes_biome = false;
        for x in (-4000..=4000).step_by(200) {
            for z in (-4000..=4000).step_by(200) {
                let a = SpawnQ {
                    world: WorldQ(&world),
                    seed: 1,
                    size: WorldSize::Small,
                };
                let b = SpawnQ {
                    world: WorldQ(&world),
                    seed: 7,
                    size: WorldSize::Small,
                };
                assert_eq!(
                    a.corrupt_at(x, z),
                    corruption_at_for_size(1, x, z, WorldSize::Small) > 0.0
                );
                assert_eq!(
                    b.corrupt_at(x, z),
                    corruption_at_for_size(7, x, z, WorldSize::Small) > 0.0
                );
                seed_changes_biome |= a.corrupt_at(x, z) != b.corrupt_at(x, z);
                assert_eq!(a.surface_y(x, z), None, "unloaded column stays excluded");
            }
        }
        assert!(seed_changes_biome, "fixture detects a hardcoded seed");
    }

    #[test]
    fn spawn_is_not_in_a_dimple() {
        for seed in [1, 2, 3, 7] {
            let sp = spawn_point(seed);
            let (x, z) = (sp[0].floor() as i32, sp[2].floor() as i32);
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = surface_y(seed, x + dx, z + dz).unwrap_or(0);
                assert!(
                    n + 1 <= sp[1] as i32 + 0,
                    "seed {seed}: neighbour higher than spawn"
                );
            }
            assert_eq!(surface_y(seed, x, z).unwrap_or(0) + 1, sp[1] as i32);
        }
    }

    use super::*;
    use terravibe_sim::drop::PICKUP_DELAY;
    use terravibe_sim::net::ClientTransport;
    use terravibe_sim::slots::{ClickKind, SlotRef};

    fn setup() -> (Server, InProcClient) {
        let (client, conn) = in_process_pair();
        let mut s = Server::new(1);
        s.connect(Box::new(conn));
        (s, client)
    }

    /// Tick for up to 2 s, collecting messages, until `done` says stop.
    fn pump(
        s: &mut Server,
        c: &InProcClient,
        got: &mut Vec<ServerMsg>,
        done: impl Fn(&[ServerMsg]) -> bool,
    ) -> bool {
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(2) {
            s.tick();
            while let Ok(Some(m)) = c.try_recv() {
                got.push(m);
            }
            if done(got) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        false
    }

    fn has_chunk(m: &[ServerMsg], p: ChunkPos) -> bool {
        m.iter()
            .any(|m| matches!(m, ServerMsg::Chunk { pos, .. } if *pos == p))
    }

    #[test]
    fn hello_gets_welcome_at_spawn() {
        let (mut s, c) = setup();
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();
        match c.try_recv().unwrap() {
            Some(ServerMsg::Welcome { spawn, .. }) => assert_eq!(spawn, spawn_point(1)),
            other => panic!("expected Welcome, got {other:?}"),
        }
    }

    #[test]
    fn server_broadcasts_time_on_join_and_every_60_ticks() {
        let (mut s, c) = setup();
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();
        let mut got = Vec::new();
        while let Ok(Some(m)) = c.try_recv() {
            got.push(m);
        }
        assert!(matches!(got[0], ServerMsg::Welcome { .. }));
        assert!(matches!(got[1], ServerMsg::Time { .. }));
        for _ in 0..60 {
            s.tick();
        }
        let mut times = 0;
        while let Ok(Some(m)) = c.try_recv() {
            times += matches!(m, ServerMsg::Time { .. }) as u32;
        }
        assert_eq!(times, 1);
    }

    #[test]
    fn streams_spawn_chunks() {
        let (mut s, c) = setup();
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        let sp = spawn_point(1);
        let want = ChunkPos::of_block(0, sp[1] as i32 - 1, 0);
        let mut got = Vec::new();
        assert!(pump(&mut s, &c, &mut got, |g| has_chunk(g, want)));
    }

    #[test]
    fn inputs_processed_in_order_one_tick_each() {
        let (mut s, c) = setup();
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        let sp = spawn_point(1);
        let want = ChunkPos::of_block(0, sp[1] as i32 - 1, 0);
        let mut got = Vec::new();
        assert!(pump(&mut s, &c, &mut got, |g| has_chunk(g, want)));
        got.clear();

        let input = PlayerInput {
            forward: 1.0,
            ..Default::default()
        };
        for seq in 1..=30 {
            c.send(ClientMsg::Input { seq, input }).unwrap();
        }
        // Inputs run before newly generated chunks are merged, so this
        // snapshot is exactly the world the server steps against.
        let snapshot = s.world.clone();
        s.tick();
        while let Ok(Some(m)) = c.try_recv() {
            got.push(m);
        }
        let (ack, body) = got
            .iter()
            .rev()
            .find_map(|m| match m {
                ServerMsg::PlayerState { ack, body, .. } => Some((*ack, body.clone())),
                _ => None,
            })
            .expect("PlayerState");
        assert_eq!(ack, 30);

        let q = WorldQ(&snapshot);
        let mut p = Player::new(sp);
        for _ in 0..30 {
            terravibe_sim::player::tick_player(&q, &mut p, &input);
        }
        assert_eq!(body, p.body);
    }

    #[test]
    fn server_mines_block_and_sends_change() {
        let (mut s, c) = setup();
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        let sp = spawn_point(1);
        let want = ChunkPos::of_block(0, sp[1] as i32 - 1, 0);
        let mut got = Vec::new();
        assert!(pump(&mut s, &c, &mut got, |g| has_chunk(g, want)));
        let input = PlayerInput {
            pitch: -std::f32::consts::FRAC_PI_2,
            use_held: true,
            slot: 1,
            ..Default::default()
        };
        for seq in 1..=120 {
            c.send(ClientMsg::Input { seq, input }).unwrap();
        }
        assert!(pump(&mut s, &c, &mut got, |g| {
            g.iter()
                .any(|m| matches!(m, ServerMsg::BlockChanged { block, .. } if *block == AIR))
                && g.iter().any(|m| {
                    matches!(m, ServerMsg::Inventory(i)
                    if i.slots.iter().flatten().any(|s| s.id == terravibe_sim::item::DIRT_BLOCK))
                })
        }));
    }

    #[test]
    fn edited_chunk_survives_interest_loss() {
        let (mut s, c) = setup();
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        let sp = spawn_point(1);
        let spawn_chunk = ChunkPos::of_block(0, sp[1] as i32 - 1, 0);
        let mut got = Vec::new();
        assert!(pump(&mut s, &c, &mut got, |g| has_chunk(g, spawn_chunk)));

        // Mine the block under the player.
        let input = PlayerInput {
            pitch: -std::f32::consts::FRAC_PI_2,
            use_held: true,
            slot: 1,
            ..Default::default()
        };
        for seq in 1..=120 {
            c.send(ClientMsg::Input { seq, input }).unwrap();
        }
        assert!(pump(&mut s, &c, &mut got, |g| g.iter().any(
            |m| matches!(m, ServerMsg::BlockChanged { block, .. } if *block == AIR)
        )));
        let pos = got
            .iter()
            .find_map(|m| match m {
                ServerMsg::BlockChanged { pos, block } if *block == AIR => Some(*pos),
                _ => None,
            })
            .expect("mined block");
        let cp = ChunkPos::of_block(pos[0], pos[1], pos[2]);
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        assert_eq!(
            s.world
                .get(&cp)
                .unwrap()
                .get(l(pos[0]), l(pos[1]), l(pos[2])),
            AIR
        );
        assert!(s.edited.contains(&cp));

        // Walk the interest far away: the spawn chunk must unload from the
        // client, but the edited copy stays in the server's world.
        got.clear();
        c.send(ClientMsg::Spectate {
            pos: [1000.0, 100.0, 0.0],
        })
        .unwrap();
        assert!(pump(&mut s, &c, &mut got, |g| g
            .iter()
            .any(|m| matches!(m, ServerMsg::Unload { pos } if *pos == cp))));
        for _ in 0..5 {
            s.tick();
        }
        while let Ok(Some(_)) = c.try_recv() {}
        let chunk = s.world.get(&cp).expect("edited chunk retained");
        assert_eq!(chunk.get(l(pos[0]), l(pos[1]), l(pos[2])), AIR);

        // Coming back resends the retained chunk rather than regenerating it.
        got.clear();
        c.send(ClientMsg::Spectate { pos: sp }).unwrap();
        assert!(pump(&mut s, &c, &mut got, |g| has_chunk(g, cp)));
        let resent = got
            .iter()
            .find_map(|m| match m {
                ServerMsg::Chunk { pos, chunk } if *pos == cp => Some(Arc::clone(chunk)),
                _ => None,
            })
            .expect("chunk resent");
        assert_eq!(
            resent.get(l(pos[0]), l(pos[1]), l(pos[2])),
            AIR,
            "walking away and back must not regenerate pristine terrain"
        );
    }

    #[test]
    fn spectator_interest_streams_chunks() {
        let (mut s, c) = setup();
        c.send(ClientMsg::Spectate {
            pos: [500.0, 100.0, 0.0],
        })
        .unwrap();
        let mut got = Vec::new();
        assert!(pump(&mut s, &c, &mut got, |g| g.iter().any(
            |m| matches!(m, ServerMsg::Chunk { pos, .. } if pos.x == 15)
        )));
    }

    #[test]
    fn far_chunks_unload() {
        let (mut s, c) = setup();
        c.send(ClientMsg::Spectate {
            pos: [0.0, 100.0, 0.0],
        })
        .unwrap();
        let mut got = Vec::new();
        assert!(pump(&mut s, &c, &mut got, |g| has_chunk(
            g,
            ChunkPos { x: 0, y: 0, z: 0 }
        )));
        got.clear();
        c.send(ClientMsg::Spectate {
            pos: [1000.0, 100.0, 0.0],
        })
        .unwrap();
        assert!(pump(&mut s, &c, &mut got, |g| g.iter().any(
            |m| matches!(m, ServerMsg::Unload { pos } if pos.x.abs() < 6)
        )));
    }

    #[test]
    fn server_chop_wood_is_picked_up_after_delay() {
        use terravibe_sim::block::TRUNK;
        use terravibe_sim::item::WOOD;
        let (mut s, c) = setup();
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        let sp = spawn_point(1);
        let y0 = sp[1].floor() as i32;
        let trunk: Vec<[i32; 3]> = (0..5).map(|dy| [2, y0 + dy, 0]).collect();
        let mut got = Vec::new();
        assert!(pump(&mut s, &c, &mut got, |g| trunk
            .iter()
            .all(|p| has_chunk(g, ChunkPos::of_block(p[0], p[1], p[2])))));
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        for p in &trunk {
            let cp = ChunkPos::of_block(p[0], p[1], p[2]);
            Arc::make_mut(s.world.get_mut(&cp).unwrap()).set(l(p[0]), l(p[1]), l(p[2]), TRUNK);
        }

        // Copper axe (slot 2), facing +x, looking down at trunk base.
        let input = PlayerInput {
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: -0.9,
            use_held: true,
            slot: 2,
            ..Default::default()
        };
        for seq in 1..=500 {
            c.send(ClientMsg::Input { seq, input }).unwrap();
        }
        let gone = |g: &[ServerMsg]| {
            trunk.iter().all(|t| {
                g.iter().any(|m| {
                    matches!(m, ServerMsg::BlockChanged { pos, block } if pos == t && *block == AIR)
                })
            })
        };
        assert!(pump(&mut s, &c, &mut got, gone));
        // Drops spawned (not forwarded as Use events); stand on them after the delay.
        assert!(
            !got.iter()
                .any(|m| matches!(m, ServerMsg::Use(UseEvent::ItemDrop { .. })))
        );
        assert!(s.drops.len() >= 5);
        s.conns[0].player.as_mut().unwrap().body.pos = [2.5, y0 as f32, 0.5];
        for _ in 0..(PICKUP_DELAY + 90) {
            s.tick();
        }
        drain(&c, &mut got);
        let wood: u32 = got
            .iter()
            .rev()
            .find_map(|m| match m {
                ServerMsg::Inventory(inv) => Some(inv),
                _ => None,
            })
            .expect("inventory sent")
            .slots
            .iter()
            .flatten()
            .filter(|s| s.id == WOOD)
            .map(|s| s.count)
            .sum();
        assert!((5..=10).contains(&wood), "{wood}");
    }

    fn hello_and_load(s: &mut Server, c: &InProcClient, got: &mut Vec<ServerMsg>) {
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        let sp = spawn_point(1);
        let want0 = ChunkPos::of_block(0, sp[1] as i32 - 1, 0);
        let want1 = ChunkPos::of_block(0, sp[1] as i32, 0);
        let want2 = ChunkPos::of_block(0, sp[1] as i32, -2);
        assert!(pump(s, c, got, |g| {
            has_chunk(g, want0) && has_chunk(g, want1) && has_chunk(g, want2)
        }));
    }

    fn drain(c: &InProcClient, got: &mut Vec<ServerMsg>) {
        while let Ok(Some(m)) = c.try_recv() {
            got.push(m);
        }
    }

    /// Waits for the column at spawn + dx to load, then adds an enemy on its surface.
    fn add_on_surface(
        s: &mut Server,
        c: &InProcClient,
        got: &mut Vec<ServerMsg>,
        k: EnemyKind,
        dx: i32,
    ) {
        let (x, z) = (s.spawn[0].floor() as i32 + dx, s.spawn[2].floor() as i32);
        let t0 = Instant::now();
        while WorldQ(&s.world).surface_y(x, z).is_none() {
            assert!(t0.elapsed() < Duration::from_secs(5), "column never loaded");
            s.tick();
            drain(c, got);
            std::thread::sleep(Duration::from_millis(1));
        }
        let y = WorldQ(&s.world).surface_y(x, z).unwrap();
        s.add_enemy(k, [x as f32 + 0.5, (y + 1) as f32, z as f32 + 0.5]);
    }

    #[test]
    fn server_streams_enemy_spawn_and_state() {
        let (mut s, c) = setup();
        // This test streams one explicitly added enemy, not the natural spawner.
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        add_on_surface(&mut s, &c, &mut got, EnemyKind::Slime, 10);
        got.clear();
        for _ in 0..13 {
            s.tick();
        }
        drain(&c, &mut got);
        let spawns = got
            .iter()
            .filter(|m| matches!(m, ServerMsg::EnemySpawn { .. }))
            .count();
        let states = got
            .iter()
            .filter(|m| matches!(m, ServerMsg::EnemyState { .. }))
            .count();
        assert_eq!(spawns, 1);
        assert!((1..=3).contains(&states), "{states}");
    }

    #[test]
    fn enemy_despawns_after_leaving_radius() {
        let (mut s, c) = setup();
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        add_on_surface(&mut s, &c, &mut got, EnemyKind::Slime, 10);
        s.tick();
        // Move the enemy out of the player's radius.
        s.enemies[0].pos[0] += 200.0;
        got.clear();
        s.tick();
        drain(&c, &mut got);
        assert!(
            got.iter()
                .any(|m| matches!(m, ServerMsg::EnemyDespawn { .. }))
        );
        assert!(s.enemies.is_empty());
    }

    #[test]
    fn server_ticks_enemy_ai() {
        let (mut s, c) = setup();
        s.clock.tick = 60_000;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        add_on_surface(&mut s, &c, &mut got, EnemyKind::Slime, 12);
        let slime = s
            .enemies
            .iter()
            .find(|e| e.kind == EnemyKind::Slime)
            .expect("slime exists");
        let before = dist2(slime.pos, s.conns[0].player.as_ref().unwrap().body.pos);
        for _ in 0..300 {
            s.tick();
        }
        let slime = s
            .enemies
            .iter()
            .find(|e| e.kind == EnemyKind::Slime)
            .expect("slime exists");
        let after = dist2(slime.pos, s.conns[0].player.as_ref().unwrap().body.pos);
        assert!(after < before, "{before} -> {after}");
    }

    #[test]
    fn no_player_no_enemies() {
        let (mut s, c) = setup();
        c.send(ClientMsg::Spectate { pos: s.spawn }).unwrap();
        let mut got = Vec::new();
        for _ in 0..1300 {
            s.tick();
        }
        drain(&c, &mut got);
        assert!(s.enemies.is_empty());
        assert!(
            !got.iter()
                .any(|m| matches!(m, ServerMsg::EnemySpawn { .. }))
        );
    }

    #[test]
    fn cfg_without_natural_spawns_spawns_nothing() {
        let (client, conn) = in_process_pair();
        let mut s = Server::with_cfg(
            1,
            ServerCfg {
                natural_spawns: false,
                ..Default::default()
            },
        );
        s.connect(Box::new(conn));
        let mut got = Vec::new();
        hello_and_load(&mut s, &client, &mut got);
        for _ in 0..1300 {
            s.tick();
        }
        assert!(s.enemies.is_empty());
    }

    /// A sealed, walled 5x3x5 room with every furnishing in an otherwise empty chunk.
    fn build_room(s: &mut Server, cp: ChunkPos) -> [i32; 3] {
        use terravibe_sim::{DOOR_CLOSED, STONE, TABLE, TORCH};
        let mut ch = Chunk::filled(AIR);
        for x in 4..11usize {
            for y in 4..9usize {
                for z in 4..11usize {
                    let inner = (5..10).contains(&x) && (5..8).contains(&y) && (5..10).contains(&z);
                    if inner {
                        ch.set_wall(x, y, z, true);
                    } else {
                        ch.set(x, y, z, STONE);
                    }
                }
            }
        }
        ch.set(4, 5, 7, DOOR_CLOSED);
        ch.set(4, 6, 7, DOOR_CLOSED);
        ch.set(6, 5, 6, terravibe_sim::CHAIR);
        ch.set(8, 5, 6, TABLE);
        ch.set(7, 7, 8, TORCH);
        s.world.insert(cp, Arc::new(ch));
        s.edited.insert(cp);
        [
            cp.x * CHUNK_SIZE + 6,
            cp.y * CHUNK_SIZE + 5,
            cp.z * CHUNK_SIZE + 6,
        ]
    }

    #[test]
    fn npc_moves_into_valid_house_on_server() {
        let mut s = Server::new(1);
        let home = build_room(&mut s, ChunkPos { x: 50, y: 1, z: 50 });
        s.scan_houses();
        assert_eq!(s.houses.len(), 1);
        assert_eq!(s.houses[0].home, home);
        s.scan_houses();
        assert_eq!(s.houses.len(), 1, "same room is not found twice");
        s.add_enemy(EnemyKind::Guide, [0.0, 80.0, 0.0]);
        s.move_in();
        assert_eq!(s.houses[0].npc, Some(TownKind::Guide));
        let g = &s.enemies[0];
        assert_eq!(
            g.pos,
            [home[0] as f32 + 0.5, home[1] as f32, home[2] as f32 + 0.5]
        );
        // Breaking the wall of the room (open air) evicts the house.
        let mut ch = (**s.world.get(&ChunkPos { x: 50, y: 1, z: 50 }).unwrap()).clone();
        ch.set_wall(7, 6, 7, false);
        s.world
            .insert(ChunkPos { x: 50, y: 1, z: 50 }, Arc::new(ch));
        s.scan_houses();
        assert!(s.houses.is_empty());
    }

    /// Connected player standing next to a freshly added NPC of `kind`.
    fn talk_setup(kind: EnemyKind, dx: f32) -> (Server, InProcClient, EnemyId) {
        let (client, conn) = in_process_pair();
        let mut s = Server::with_cfg(
            1,
            ServerCfg {
                natural_spawns: false,
                ..ServerCfg::default()
            },
        );
        s.connect(Box::new(conn));
        let mut got = Vec::new();
        hello_and_load(&mut s, &client, &mut got);
        let p = s.conns[0].player.as_ref().unwrap().body.pos;
        s.add_enemy(kind, [p[0] + dx, p[1], p[2]]);
        let id = s.enemies.last().unwrap().id;
        (s, client, id)
    }

    fn replies(s: &mut Server, c: &InProcClient) -> Vec<ServerMsg> {
        s.tick();
        let mut got = Vec::new();
        drain(c, &mut got);
        got
    }

    #[test]
    fn talk_to_npc_returns_dialogue() {
        let (mut s, c, id) = talk_setup(EnemyKind::Merchant, 2.0);
        c.send(ClientMsg::TalkNpc { id }).unwrap();
        let got = replies(&mut s, &c);
        assert!(got.iter().any(
            |m| matches!(m, ServerMsg::Dialogue { npc, text } if *npc == id && !text.is_empty())
        ));
        assert!(
            got.iter()
                .any(|m| matches!(m, ServerMsg::ShopContents { items } if !items.is_empty()))
        );
    }

    #[test]
    fn old_man_warns_player_during_day() {
        let (mut s, c, id) = talk_setup(EnemyKind::OldMan, 2.0);
        s.clock.tick = 13_500; // daytime
        assert!(s.clock.is_day());
        c.send(ClientMsg::TalkNpc { id }).unwrap();
        let got = replies(&mut s, &c);
        let warning = got.iter().find_map(|m| match m {
            ServerMsg::Dialogue { npc, text } if *npc == id => Some(text),
            _ => None,
        });
        assert!(warning.is_some(), "expected dialogue from Old Man");
        assert!(
            warning
                .unwrap()
                .contains("My master cannot be summoned by the light of day"),
            "expected daytime curse warning, got: {}",
            warning.unwrap()
        );
        // Old man is still alive
        assert!(s.enemies.iter().any(|e| e.id == id && e.hp > 0));
        // Skeletron not spawned
        assert!(!s.enemies.iter().any(|e| e.kind == EnemyKind::SkeletronHead));
    }

    #[test]
    fn old_man_summons_skeletron_at_night() {
        let (mut s, c, id) = talk_setup(EnemyKind::OldMan, 2.0);
        s.clock.tick = 60_000; // night time
        assert!(!s.clock.is_day());
        c.send(ClientMsg::TalkNpc { id }).unwrap();
        let _ = replies(&mut s, &c);
        // Old man should be killed/despawned
        assert!(
            !s.enemies.iter().any(|e| e.id == id && e.hp > 0),
            "Old Man should be despawned or killed on night summon"
        );
        // SkeletronHead should be spawned at dungeon entrance
        assert!(
            s.enemies
                .iter()
                .any(|e| e.kind == EnemyKind::SkeletronHead && e.hp > 0),
            "SkeletronHead must be spawned at dungeon entrance"
        );
    }

    #[test]
    fn dungeon_guardian_spawns_when_entering_pre_skeletron() {
        let (client, conn) = in_process_pair();
        let mut s = Server::with_cfg(
            1,
            ServerCfg {
                natural_spawns: false,
                ..ServerCfg::default()
            },
        );
        s.connect(Box::new(conn));
        let mut got = Vec::new();
        hello_and_load(&mut s, &client, &mut got);
        assert!(!s.downed_skeletron);

        let entrance = terravibe_sim::worldgen::dungeon_entrance_pos(s.seed, s.bounds.size());
        let ex = entrance[0] as i32;
        let ey = entrance[1] as i32 - 1;
        let ez = entrance[2] as i32;

        let side = terravibe_sim::worldgen::dungeon::dungeon_side(s.seed, s.bounds.size());
        // Put player inside subterranean dungeon staircase
        let dungeon_subterranean = [(ex - side * 5) as f32, (ey - 5) as f32, ez as f32];
        s.conns[0].player.as_mut().unwrap().body.pos = dungeon_subterranean;

        s.tick();

        assert!(
            s.enemies
                .iter()
                .any(|e| e.kind == EnemyKind::DungeonGuardian && e.hp > 0),
            "Dungeon Guardian must spawn when player enters subterranean dungeon pre-skeletron"
        );
    }

    #[test]
    fn dungeon_enemies_spawn_after_skeletron_defeated() {
        let (client, conn) = in_process_pair();
        let mut s = Server::with_cfg(
            1,
            ServerCfg {
                natural_spawns: false,
                ..ServerCfg::default()
            },
        );
        s.connect(Box::new(conn));
        let mut got = Vec::new();
        hello_and_load(&mut s, &client, &mut got);
        s.downed_skeletron = true;

        let entrance = terravibe_sim::worldgen::dungeon_entrance_pos(s.seed, s.bounds.size());
        let ex = entrance[0] as i32;
        let ey = entrance[1] as i32 - 1;
        let ez = entrance[2] as i32;

        let side = terravibe_sim::worldgen::dungeon::dungeon_side(s.seed, s.bounds.size());
        let dungeon_subterranean = [(ex - side * 5) as f32, (ey - 5) as f32, ez as f32];
        s.conns[0].player.as_mut().unwrap().body.pos = dungeon_subterranean;

        s.tick();

        assert!(
            s.enemies.iter().any(|e| {
                matches!(
                    e.kind,
                    EnemyKind::AngryBones | EnemyKind::DarkCaster | EnemyKind::CursedSkull
                ) && e.hp > 0
            }),
            "Dungeon enemies must spawn when player enters subterranean dungeon post-skeletron"
        );
        assert!(
            !s.enemies
                .iter()
                .any(|e| e.kind == EnemyKind::DungeonGuardian && e.hp > 0),
            "Dungeon Guardian must not spawn post-skeletron"
        );
    }

    #[test]
    fn defeating_skeletron_head_flips_downed_flag_and_despawns_hands() {
        let (client, conn) = in_process_pair();
        let mut s = Server::with_cfg(
            1,
            ServerCfg {
                natural_spawns: false,
                ..ServerCfg::default()
            },
        );
        s.connect(Box::new(conn));
        let mut got = Vec::new();
        hello_and_load(&mut s, &client, &mut got);

        assert!(!s.downed_skeletron);
        s.spawn_skeletron([0.0, 50.0, 0.0]);
        assert_eq!(
            s.enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::SkeletronHand)
                .count(),
            2
        );
        let head_idx = s
            .enemies
            .iter()
            .position(|e| e.kind == EnemyKind::SkeletronHead)
            .expect("head must exist");
        s.enemies[head_idx].hp = 0;

        s.tick_enemies();

        assert!(s.downed_skeletron, "downed_skeletron must be true");
        assert_eq!(
            s.enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::SkeletronHand)
                .count(),
            0,
            "all hands must be despawned when head dies"
        );
        assert!(
            !s.drops.is_empty(),
            "boss drops should be awarded on head defeat"
        );
    }

    #[test]
    fn shop_out_of_reach_rejected() {
        let (mut s, c, id) = talk_setup(EnemyKind::Merchant, 30.0);
        let coins_before = terravibe_sim::coin::total_copper(&s.conns[0].inv);
        terravibe_sim::coin::give_copper(&mut s.conns[0].inv, 10_000);
        c.send(ClientMsg::TalkNpc { id }).unwrap();
        c.send(ClientMsg::ShopBuy {
            npc: id,
            index: 3,
            count: 1,
        })
        .unwrap();
        let got = replies(&mut s, &c);
        assert!(!got.iter().any(|m| matches!(m, ServerMsg::Dialogue { .. })));
        assert!(
            got.iter()
                .any(|m| matches!(m, ServerMsg::ShopResult { ok: false, .. }))
        );
        assert_eq!(
            terravibe_sim::coin::total_copper(&s.conns[0].inv),
            coins_before + 10_000
        );
    }

    #[test]
    fn shop_buy_and_sell_in_reach() {
        let (mut s, c, id) = talk_setup(EnemyKind::Merchant, 2.0);
        terravibe_sim::coin::give_copper(&mut s.conns[0].inv, 100);
        let t0 = s.conns[0].inv.count_of(terravibe_sim::item::TORCH);
        c.send(ClientMsg::TalkNpc { id }).unwrap();
        c.send(ClientMsg::ShopBuy {
            npc: id,
            index: 3,
            count: 1,
        })
        .unwrap();
        let got = replies(&mut s, &c);
        assert!(
            got.iter()
                .any(|m| matches!(m, ServerMsg::ShopResult { ok: true, .. }))
        );
        assert_eq!(s.conns[0].inv.count_of(terravibe_sim::item::TORCH), t0 + 1);
        let slot = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|x| x.id == terravibe_sim::item::TORCH))
            .unwrap();
        let before = terravibe_sim::coin::total_copper(&s.conns[0].inv);
        let n = s.conns[0].inv.slots[slot].unwrap().count;
        c.send(ClientMsg::ShopSell { slot: slot as u8 }).unwrap();
        replies(&mut s, &c);
        assert_eq!(
            terravibe_sim::coin::total_copper(&s.conns[0].inv),
            before + 10 * n
        );
    }

    #[test]
    fn guide_spawns_once_on_new_world() {
        let (client, conn) = in_process_pair();
        let mut s = Server::with_cfg(
            1,
            ServerCfg {
                natural_spawns: true,
                clock_start: 0,
                ..ServerCfg::default()
            },
        );
        s.connect(Box::new(conn));
        let mut got = Vec::new();
        hello_and_load(&mut s, &client, &mut got);
        for _ in 0..1200 {
            s.tick();
            std::thread::sleep(Duration::from_micros(500));
        }
        let guides = |s: &Server| {
            s.enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::Guide)
                .count()
        };
        assert_eq!(guides(&s), 1);
        assert!(s.guide_spawned);
        assert!(
            got.iter()
                .any(|m| matches!(m, ServerMsg::TownNpcInfo { .. }))
                || {
                    drain(&client, &mut got);
                    got.iter()
                        .any(|m| matches!(m, ServerMsg::TownNpcInfo { .. }))
                }
        );
    }

    #[test]
    fn demo_enemies_spawn_three_fixed() {
        let (client, conn) = in_process_pair();
        let mut s = Server::with_cfg(
            1,
            ServerCfg {
                natural_spawns: false,
                demo_enemies: true,
                demo_boss: false,
                demo_npc: false,
                liquid_stress: false,
                qa_liquids: false,
                qa_kill: false,
                clock_start: terravibe_sim::time::DAY_TICKS,
                size: WorldSize::Small,
                save: None,
                ..ServerCfg::default()
            },
        );
        s.connect(Box::new(conn));
        let mut got = Vec::new();
        hello_and_load(&mut s, &client, &mut got);
        for _ in 0..600 {
            s.tick();
            std::thread::sleep(Duration::from_millis(1));
            if s.enemies.len() == 3 {
                break;
            }
        }
        assert_eq!(s.enemies.len(), 3);
        let kinds: HashSet<_> = s.enemies.iter().map(|e| e.kind).collect();
        assert_eq!(kinds.len(), 3);
    }

    #[test]
    fn demo_enemies_stream_to_spectator() {
        let (client, conn) = in_process_pair();
        let mut s = Server::with_cfg(
            1,
            ServerCfg {
                natural_spawns: false,
                demo_enemies: true,
                demo_boss: false,
                demo_npc: false,
                liquid_stress: false,
                qa_liquids: false,
                qa_kill: false,
                clock_start: terravibe_sim::time::DAY_TICKS,
                size: WorldSize::Small,
                save: None,
                ..ServerCfg::default()
            },
        );
        s.connect(Box::new(conn));
        let sp = spawn_point(1);
        client.send(ClientMsg::Spectate { pos: sp }).unwrap();
        let mut got = Vec::new();
        let ok = pump(&mut s, &client, &mut got, |g| {
            g.iter()
                .filter(|m| matches!(m, ServerMsg::EnemySpawn { .. }))
                .count()
                == 3
                && g.iter().any(|m| matches!(m, ServerMsg::DropSpawn { .. }))
        });
        assert!(ok);
    }

    #[test]
    fn server_sword_kills_slime() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        add_on_surface(&mut s, &c, &mut got, EnemyKind::Slime, 2);
        let home = s
            .enemies
            .iter()
            .find(|e| e.kind == EnemyKind::Slime)
            .unwrap()
            .pos;
        got.clear();
        // Sword in slot 0, facing +x; Copper Broadsword is not auto_reuse, so
        // pulse clicks (`seq % 2 == 1`). The slime is pinned so knockback stays in cone.
        for seq in 1..=160 {
            let input = PlayerInput {
                yaw: -std::f32::consts::FRAC_PI_2,
                use_held: seq % 2 == 1,
                ..Default::default()
            };
            c.send(ClientMsg::Input { seq, input }).unwrap();
            if let Some(e) = s.enemies.iter_mut().find(|e| e.kind == EnemyKind::Slime) {
                e.pos = home;
                e.vel = [0.0; 3];
            }
            s.tick();
            drain(&c, &mut got);
        }
        assert!(
            s.enemies
                .iter()
                .all(|e| e.kind.is_friendly() || e.kind.is_critter())
        );
        assert!(
            got.iter()
                .any(|m| matches!(m, ServerMsg::EnemyDespawn { .. }))
        );
    }

    /// Ticks with one idle input per tick (so i-frames count down); returns PlayerHit amounts.
    fn tick_idle(s: &mut Server, c: &InProcClient, seq: &mut u32, n: u32) -> Vec<ServerMsg> {
        let mut got = Vec::new();
        for _ in 0..n {
            *seq += 1;
            c.send(ClientMsg::Input {
                seq: *seq,
                input: PlayerInput::default(),
            })
            .unwrap();
            s.tick();
            drain(c, &mut got);
        }
        got
    }

    fn hits(m: &[ServerMsg]) -> Vec<i32> {
        m.iter()
            .filter_map(|m| match m {
                ServerMsg::PlayerHit { amount } => Some(*amount),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn contact_damage_respects_player_iframes() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.enemies.clear();
        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        s.add_enemy(EnemyKind::Zombie, pos);
        let mut seq = 0;
        let mut overlap_ticks = |n: u32| {
            let mut out = Vec::new();
            for _ in 0..n {
                s.enemies[0].pos = s.conns[0].player.as_ref().unwrap().body.pos;
                s.enemies[0].vel = [0.0; 3];
                out.extend(tick_idle(&mut s, &c, &mut seq, 1));
            }
            out
        };
        let first = overlap_ticks(30);
        assert_eq!(hits(&first), [14]);
        assert!(!first.iter().any(|m| matches!(m, ServerMsg::Event(_))));
        let later = overlap_ticks(30);
        assert_eq!(hits(&later), [14]);
    }

    #[test]
    fn contact_kill_sends_player_died() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        let p = s.conns[0].player.as_mut().unwrap();
        p.vitals.life = 5;
        let pos = p.body.pos;
        s.add_enemy(EnemyKind::Zombie, pos);
        let mut seq = 0;
        let got = tick_idle(&mut s, &c, &mut seq, 3);
        assert_eq!(hits(&got), [14]);
        assert!(got.iter().any(|m| matches!(
            m,
            ServerMsg::PlayerDied {
                cause: DeathCause::Contact(EnemyKind::Zombie),
                ..
            }
        )));
    }

    #[test]
    fn server_kill_spawns_and_streams_drop() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        s.add_enemy(EnemyKind::Slime, [pos[0] + 20.0, pos[1], pos[2]]);
        s.enemies.last_mut().unwrap().hp = 0;
        got.clear();
        s.tick();
        drain(&c, &mut got);
        assert!(
            s.drops
                .iter()
                .any(|d| d.stack.id == terravibe_sim::item::GEL)
        );
        assert!(got.iter().any(|m| matches!(m,
            ServerMsg::DropSpawn { stack, .. } if stack.id == terravibe_sim::item::GEL)));
    }

    #[test]
    fn server_pickup_sends_inventory_and_pickup() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        let gel = terravibe_sim::inventory::ItemStack {
            id: terravibe_sim::item::GEL,
            count: 2,
            prefix: terravibe_sim::item::Prefix(0),
        };
        s.add_drop(gel, pos);
        got.clear();
        for _ in 0..(PICKUP_DELAY + 5) {
            s.tick();
        }
        drain(&c, &mut got);
        let id = DropId(1);
        assert!(
            got.iter()
                .any(|m| matches!(m, ServerMsg::DropSpawn { id: i, .. } if *i == id))
        );
        assert!(got.iter().any(|m| matches!(
            m,
            ServerMsg::DropPickup {
                id: i,
                item,
                count,
            } if *i == id && *item == gel.id && *count == 2
        )));
        assert!(got.iter().any(|m| matches!(m, ServerMsg::Inventory(i)
            if i.slots.iter().flatten().any(|s| s.id == gel.id && s.count == 2))));
        assert!(s.drops.is_empty());
    }

    fn give(s: &mut Server, item: ItemId, count: u32) {
        let st = terravibe_sim::inventory::ItemStack {
            id: item,
            count,
            prefix: terravibe_sim::item::Prefix(0),
        };
        assert!(s.conns[0].inv.add(st).is_none());
    }

    fn craft_req(s: &mut Server, c: &InProcClient, recipe: RecipeId, count: u16) -> Vec<ServerMsg> {
        c.send(ClientMsg::Craft { recipe, count }).unwrap();
        let mut got = Vec::new();
        s.tick();
        drain(c, &mut got);
        got
    }

    fn result_of(got: &[ServerMsg]) -> Option<(u32, Option<CraftError>)> {
        got.iter().find_map(|m| match m {
            ServerMsg::CraftResult { crafted, error, .. } => Some((*crafted, *error)),
            _ => None,
        })
    }

    fn has_inv(got: &[ServerMsg]) -> bool {
        got.iter().any(|m| matches!(m, ServerMsg::Inventory(_)))
    }

    #[test]
    fn server_crafts_torches() {
        use terravibe_sim::crafting::TORCH_RECIPE;
        use terravibe_sim::item::{GEL, TORCH, WOOD};
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        give(&mut s, GEL, 1);
        give(&mut s, WOOD, 1);
        let got = craft_req(&mut s, &c, TORCH_RECIPE, 1);
        assert_eq!(result_of(&got), Some((3, None)));
        assert!(got.iter().any(|m| matches!(m, ServerMsg::Inventory(i)
            if i.count_of(TORCH) == 3 && i.count_of(GEL) == 0 && i.count_of(WOOD) == 0)));
    }

    #[test]
    fn server_craft_requires_nearby_workbench() {
        use terravibe_sim::crafting::WOODEN_SWORD_RECIPE;
        use terravibe_sim::item::{WOOD, WOODEN_SWORD};
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        give(&mut s, WOOD, 7);
        s.tick(); // streams the granted inventory
        drain(&c, &mut got);
        let got = craft_req(&mut s, &c, WOODEN_SWORD_RECIPE, 1);
        assert_eq!(result_of(&got), Some((0, Some(CraftError::MissingStation))));
        assert!(!has_inv(&got));
        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        let at = [
            pos[0].floor() as i32 + 2,
            pos[1].floor() as i32,
            pos[2].floor() as i32,
        ];
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        let cp = ChunkPos::of_block(at[0], at[1], at[2]);
        Arc::make_mut(s.world.get_mut(&cp).unwrap()).set(
            l(at[0]),
            l(at[1]),
            l(at[2]),
            terravibe_sim::WORKBENCH,
        );
        let got = craft_req(&mut s, &c, WOODEN_SWORD_RECIPE, 1);
        assert_eq!(result_of(&got), Some((1, None)));
        assert!(got.iter().any(|m| matches!(m, ServerMsg::Inventory(i)
            if i.count_of(WOODEN_SWORD) == 1 && i.count_of(WOOD) == 0)));
    }

    #[test]
    fn server_failed_craft_consumes_nothing() {
        use terravibe_sim::crafting::TORCH_RECIPE;
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.tick();
        let before = s.conns[0].inv.clone();
        // Missing ingredients, unknown recipe, zero count.
        for (id, n, e) in [
            (TORCH_RECIPE, 1, CraftError::MissingIngredients),
            (RecipeId(999), 1, CraftError::UnknownRecipe),
            (TORCH_RECIPE, 0, CraftError::InvalidCount),
        ] {
            drain(&c, &mut got);
            let got = craft_req(&mut s, &c, id, n);
            assert_eq!(result_of(&got), Some((0, Some(e))));
            assert!(!has_inv(&got));
            assert_eq!(s.conns[0].inv, before);
        }
    }

    #[test]
    fn server_craft_before_hello_is_ignored() {
        use terravibe_sim::crafting::TORCH_RECIPE;
        let (mut s, c) = setup();
        let got = craft_req(&mut s, &c, TORCH_RECIPE, 1);
        assert!(result_of(&got).is_none());
    }

    #[test]
    fn server_craft_without_input_still_streams_inventory() {
        use terravibe_sim::crafting::TORCH_RECIPE;
        use terravibe_sim::item::{GEL, WOOD};
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        give(&mut s, GEL, 1);
        give(&mut s, WOOD, 1);
        // No Input is ever sent.
        let got = craft_req(&mut s, &c, TORCH_RECIPE, 1);
        assert_eq!(result_of(&got), Some((3, None)));
        assert!(has_inv(&got));
    }

    #[test]
    fn equip_is_server_authoritative_and_reduces_contact_damage() {
        use terravibe_sim::item::{COPPER_SHORTSWORD, GOLD_CHAINMAIL};
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        give(&mut s, GOLD_CHAINMAIL, 1);
        let slot = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|x| x.id == GOLD_CHAINMAIL))
            .unwrap();
        // Non-armor and empty slots are refused.
        c.send(ClientMsg::Equip { from: 0 }).unwrap();
        c.send(ClientMsg::Equip { from: 49 }).unwrap();
        s.tick();
        assert_eq!(s.conns[0].player.as_ref().unwrap().armor, [None; 3]);
        assert!(s.conns[0].inv.count_of(COPPER_SHORTSWORD) > 0);
        got.clear();
        c.send(ClientMsg::Equip { from: slot as u8 }).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(s.conns[0].player.as_ref().unwrap().defense(), 5);
        assert!(got.iter().any(|m| matches!(m,
            ServerMsg::Armor { pieces } if *pieces == [None, Some(GOLD_CHAINMAIL), None])));
        assert_eq!(s.conns[0].inv.count_of(GOLD_CHAINMAIL), 0);
        // A zombie (14 damage) now hurts for 14 - (5 + 1) / 2 = 11.
        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        s.add_enemy(EnemyKind::Zombie, pos);
        got.clear();
        s.tick();
        drain(&c, &mut got);
        assert!(
            got.iter()
                .any(|m| matches!(m, ServerMsg::PlayerHit { amount: 11 }))
        );
        // Unequip returns it.
        c.send(ClientMsg::Unequip {
            slot: terravibe_sim::item::ArmorSlot::Body,
        })
        .unwrap();
        s.tick();
        assert_eq!(s.conns[0].inv.count_of(GOLD_CHAINMAIL), 1);
    }

    #[test]
    fn shadow_armor_set_bonus_server_authoritative() {
        use terravibe_sim::item::{ArmorSlot, SHADOW_GREAVES, SHADOW_HELMET, SHADOW_SCALEMAIL};
        use terravibe_sim::player::consts::MAX_RUN_SPEED;
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        give(&mut s, SHADOW_HELMET, 1);
        give(&mut s, SHADOW_SCALEMAIL, 1);
        give(&mut s, SHADOW_GREAVES, 1);

        let find_slot = |s: &Server, id| {
            s.conns[0]
                .inv
                .slots
                .iter()
                .position(|x| x.is_some_and(|x| x.id == id))
                .unwrap() as u8
        };

        let slot_h = find_slot(&s, SHADOW_HELMET);
        c.send(ClientMsg::Equip { from: slot_h }).unwrap();
        s.tick();

        let slot_b = find_slot(&s, SHADOW_SCALEMAIL);
        c.send(ClientMsg::Equip { from: slot_b }).unwrap();
        s.tick();

        let slot_l = find_slot(&s, SHADOW_GREAVES);
        c.send(ClientMsg::Equip { from: slot_l }).unwrap();
        s.tick();

        let p = s.conns[0].player.as_ref().unwrap();
        assert!(p.has_shadow_set());
        assert_eq!(p.defense(), 19);
        assert!(p.body.has_shadow_set);

        // Move player forward
        let input = terravibe_sim::player::PlayerInput {
            forward: 1.0,
            yaw: 0.0,
            ..Default::default()
        };
        for seq in 1..=15 {
            c.send(ClientMsg::Input {
                seq,
                input: input.clone(),
            })
            .unwrap();
            s.tick();
        }

        let p_moving = s.conns[0].player.as_ref().unwrap();
        let speed = p_moving.body.vel[0].hypot(p_moving.body.vel[2]);
        let expected_speed = MAX_RUN_SPEED * 1.15;
        assert!(
            (speed - expected_speed).abs() < 1e-3,
            "speed ({speed}) should equal boosted speed ({expected_speed})"
        );

        // Unequip legs
        c.send(ClientMsg::Unequip {
            slot: ArmorSlot::Legs,
        })
        .unwrap();
        s.tick();

        let p_after = s.conns[0].player.as_ref().unwrap();
        assert!(!p_after.has_shadow_set());
        assert!(!p_after.body.has_shadow_set);
    }

    #[test]
    fn molten_armor_set_bonus_server_authoritative() {
        use terravibe_sim::item::{ArmorSlot, MOLTEN_BREASTPLATE, MOLTEN_GREAVES, MOLTEN_HELMET};
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        give(&mut s, MOLTEN_HELMET, 1);
        give(&mut s, MOLTEN_BREASTPLATE, 1);
        give(&mut s, MOLTEN_GREAVES, 1);

        let find_slot = |s: &Server, id| {
            s.conns[0]
                .inv
                .slots
                .iter()
                .position(|x| x.is_some_and(|x| x.id == id))
                .unwrap() as u8
        };

        let slot_h = find_slot(&s, MOLTEN_HELMET);
        c.send(ClientMsg::Equip { from: slot_h }).unwrap();
        s.tick();

        let slot_b = find_slot(&s, MOLTEN_BREASTPLATE);
        c.send(ClientMsg::Equip { from: slot_b }).unwrap();
        s.tick();

        let slot_l = find_slot(&s, MOLTEN_GREAVES);
        c.send(ClientMsg::Equip { from: slot_l }).unwrap();
        s.tick();

        let p = s.conns[0].player.as_ref().unwrap();
        assert!(p.has_molten_set());
        assert_eq!(p.defense(), 25);
        assert!(p.body.has_molten_set);
        assert_eq!(p.melee_crit_bonus(), 7);
        // The starting Copper Shortsword (slot 0) hits a Zombie (6 defense) with ±15% damage
        // variance before (defense + 1) / 2 subtraction, doubled on crit.
        let held_id = s.conns[0].inv.slots[0].unwrap().id;
        let base =
            (item_data(held_id).damage as f32 * p.melee_damage_mult()).round_ties_even() as i32;
        let def_sub = (EnemyKind::Zombie.stats().defense + 1) / 2;
        let lo = ((base as f32 * 0.85).round_ties_even() as i32 - def_sub).max(1);
        let hi = ((base as f32 * 1.15).round_ties_even() as i32 - def_sub).max(1);
        let pos = p.body.pos;

        s.add_enemy(EnemyKind::Zombie, [pos[0], pos[1], pos[2] - 1.5]);
        got.clear();
        let input = PlayerInput {
            use_held: true,
            ..Default::default()
        };
        c.send(ClientMsg::Input { seq: 1, input }).unwrap();
        s.tick();
        drain(&c, &mut got);

        let (amount, crit) = got
            .iter()
            .find_map(|m| match m {
                ServerMsg::CombatHit {
                    amount,
                    crit,
                    damage_kind: DamageKind::Melee,
                    ..
                } => Some((*amount, *crit)),
                _ => None,
            })
            .expect("expected a melee CombatHit on the zombie");
        let want_range = if crit { lo * 2..=hi * 2 } else { lo..=hi };
        assert!(
            want_range.contains(&amount),
            "molten set damage should be in {want_range:?} (crit {crit}), got {amount}"
        );

        // Without the set the same piece of armour only loses its legs.
        c.send(ClientMsg::Unequip {
            slot: ArmorSlot::Legs,
        })
        .unwrap();
        s.tick();
        let p_after = s.conns[0].player.as_ref().unwrap();
        assert!(!p_after.has_molten_set());
        assert!(!p_after.body.has_molten_set);
    }

    #[test]
    fn molten_fury_shot_spawns_flaming_arrow() {
        use terravibe_sim::inventory::{AMMO0, ItemStack};
        use terravibe_sim::item::{MOLTEN_FURY, Prefix, WOODEN_ARROW};
        use terravibe_sim::projectile::ProjKind;

        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        s.conns[0].inv.slots[0] = Some(ItemStack {
            id: MOLTEN_FURY,
            count: 1,
            prefix: Prefix(0),
        });
        s.conns[0].inv.slots[AMMO0] = Some(ItemStack {
            id: WOODEN_ARROW,
            count: 10,
            prefix: Prefix(0),
        });
        s.conns[0].inv.selected = 0;
        s.conns[0].use_state.time = 0;
        s.conns[0].use_state.anim = 0;
        s.conns[0].use_state.last_use_held = false;

        c.send(ClientMsg::Input {
            seq: 1,
            input: PlayerInput {
                use_held: true,
                yaw: 0.0,
                pitch: 0.0,
                ..Default::default()
            },
        })
        .unwrap();
        s.tick();

        assert_eq!(
            s.conns[0].inv.slots[AMMO0].as_ref().unwrap().count,
            9,
            "the arrow is consumed once, no extra ammo cost"
        );
        let p = s
            .projectiles
            .iter()
            .find(|p| p.kind == ProjKind::FlamingArrow)
            .expect("Molten Fury must spawn a Flaming Arrow for wooden arrows");
        assert_eq!(p.damage, 31 + item_data(WOODEN_ARROW).damage);
    }

    #[test]
    fn fiery_greatsword_explosion_hits_two_extra_enemies() {
        use terravibe_sim::enemy::EnemyKind;
        use terravibe_sim::inventory::ItemStack;
        use terravibe_sim::item::{FIERY_GREATSWORD, Prefix};

        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.enemies.clear();

        s.conns[0].inv.slots[0] = Some(ItemStack {
            id: FIERY_GREATSWORD,
            count: 1,
            prefix: Prefix(0),
        });
        s.conns[0].inv.selected = 0;
        s.conns[0].use_state.time = 0;
        s.conns[0].use_state.anim = 0;
        s.conns[0].use_state.last_use_held = false;

        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        // Primary within sword reach; two extras beyond the cone but within
        // 5 blocks of the primary; a fourth enemy far outside the blast.
        s.add_enemy(EnemyKind::Zombie, [pos[0], pos[1], pos[2] - 1.5]);
        s.add_enemy(EnemyKind::Zombie, [pos[0], pos[1], pos[2] - 5.5]);
        s.add_enemy(EnemyKind::Zombie, [pos[0], pos[1], pos[2] - 6.0]);
        s.add_enemy(EnemyKind::Zombie, [pos[0], pos[1], pos[2] - 12.0]);
        let before: Vec<i32> = s.enemies.iter().map(|e| e.hp).collect();

        c.send(ClientMsg::Input {
            seq: 1,
            input: PlayerInput {
                use_held: true,
                ..Default::default()
            },
        })
        .unwrap();
        s.tick();
        drain(&c, &mut got);

        let hits: Vec<(i32, bool)> = got
            .iter()
            .filter_map(|m| match m {
                ServerMsg::CombatHit {
                    amount,
                    crit,
                    damage_kind: DamageKind::Melee,
                    ..
                } => Some((*amount, *crit)),
                _ => None,
            })
            .collect();
        assert_eq!(hits.len(), 3, "primary plus two explosion hits");
        let amounts: Vec<i32> = hits.iter().map(|(a, _)| *a).collect();
        let def = EnemyKind::Zombie.stats().defense / 2;
        let base = item_data(FIERY_GREATSWORD).damage - def;
        let want = if amounts[0] == base {
            (base as f32 * 0.75) as i32 - def
        } else if amounts[0] == base * 2 {
            ((base * 2) as f32 * 0.75) as i32 - def
        } else {
            panic!("unexpected primary damage {}", amounts[0]);
        };
        assert_eq!(
            amounts[1], want,
            "explosion damage is 75% of the primary hit, defence applies"
        );
        assert_eq!(amounts[2], want, "both explosion targets take the same");
        assert!(
            !hits[1].1 && !hits[2].1,
            "explosion hits never crit, got {:?}",
            &hits[1..]
        );

        assert!(s.enemies[0].hp < before[0], "primary zombie damaged");
        assert!(
            s.enemies[1].hp < before[1],
            "first explosion target damaged"
        );
        assert!(s.enemies[1].is_on_fire(), "explosion targets are on fire");
        assert!(
            s.enemies[2].hp < before[2],
            "second explosion target damaged"
        );
        assert!(s.enemies[2].is_on_fire(), "explosion targets are on fire");
        assert_eq!(s.enemies[3].hp, before[3], "distant enemy untouched");
        assert!(!s.enemies[3].is_on_fire(), "distant enemy not on fire");
    }

    #[test]
    fn fiery_greatsword_splash_skips_other_cone_hits() {
        use terravibe_sim::enemy::EnemyKind;
        use terravibe_sim::inventory::ItemStack;
        use terravibe_sim::item::{FIERY_GREATSWORD, Prefix};

        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.enemies.clear();

        s.conns[0].inv.slots[0] = Some(ItemStack {
            id: FIERY_GREATSWORD,
            count: 1,
            prefix: Prefix(0),
        });
        s.conns[0].inv.selected = 0;
        s.conns[0].use_state.time = 0;
        s.conns[0].use_state.anim = 0;
        s.conns[0].use_state.last_use_held = false;

        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        let half_y = EnemyKind::Zombie.stats().half[1];
        let centre = |p: [f32; 3]| [p[0], p[1] + half_y, p[2]];
        // Primary (nearest cone hit); a second cone hit 1.5 blocks behind it,
        // still within blast radius; an enemy outside the cone but in range.
        let primary_pos = [pos[0], pos[1], pos[2] - 1.5];
        let cone_pos = [pos[0], pos[1], pos[2] - 3.0];
        let splash_pos = [pos[0] + 3.0, pos[1], pos[2] - 1.5];
        s.add_enemy(EnemyKind::Zombie, primary_pos);
        s.add_enemy(EnemyKind::Zombie, cone_pos);
        s.add_enemy(EnemyKind::Zombie, splash_pos);
        let before: Vec<i32> = s.enemies.iter().map(|e| e.hp).collect();

        c.send(ClientMsg::Input {
            seq: 1,
            input: PlayerInput {
                use_held: true,
                ..Default::default()
            },
        })
        .unwrap();
        s.tick();
        drain(&c, &mut got);

        let hits: Vec<(i32, bool, [f32; 3])> = got
            .iter()
            .filter_map(|m| match m {
                ServerMsg::CombatHit {
                    pos,
                    amount,
                    crit,
                    damage_kind: DamageKind::Melee,
                    ..
                } => Some((*amount, *crit, *pos)),
                _ => None,
            })
            .collect();
        let at = |want: [f32; 3]| -> Vec<(i32, bool)> {
            hits.iter()
                .filter(|(_, _, p)| {
                    (p[0] - want[0]).abs() < 1e-3
                        && (p[1] - want[1]).abs() < 1e-3
                        && (p[2] - want[2]).abs() < 1e-3
                })
                .map(|(a, c, _)| (*a, *c))
                .collect()
        };
        let primary_hits = at(centre(primary_pos));
        let cone_hits = at(centre(cone_pos));
        let splash_hits = at(centre(splash_pos));
        assert_eq!(
            primary_hits.len() + cone_hits.len(),
            2,
            "both cone enemies get exactly one melee hit each: {hits:?}"
        );
        assert_eq!(primary_hits.len(), 1, "primary hit once: {hits:?}");
        assert_eq!(
            cone_hits.len(),
            1,
            "the other cone hit must not be splashed: {hits:?}"
        );
        assert_eq!(
            splash_hits.len(),
            1,
            "the outside-cone enemy takes exactly the splash: {hits:?}"
        );

        let def = EnemyKind::Zombie.stats().defense / 2;
        let base = item_data(FIERY_GREATSWORD).damage - def;
        let full = |(amount, _): (i32, bool)| amount == base || amount == base * 2;
        assert!(
            full(primary_hits[0]),
            "the nearest cone hit (primary) takes the full sword hit"
        );
        assert!(
            full(cone_hits[0]),
            "the other cone enemy takes a normal melee hit"
        );
        assert_eq!(
            splash_hits[0].0,
            ((primary_hits[0].0 as f32 * 0.75) as i32 - def).max(1),
            "splash is 75% of the primary hit"
        );
        assert!(!splash_hits[0].1, "splash never crits");
        assert!(
            s.enemies[2].is_on_fire(),
            "the outside-cone splash target burns"
        );
        assert_eq!(s.enemies[0].hp, before[0] - primary_hits[0].0);
        assert_eq!(s.enemies[1].hp, before[1] - cone_hits[0].0);
        assert_eq!(s.enemies[2].hp, before[2] - splash_hits[0].0);
    }

    #[test]
    fn fiery_greatsword_direct_hit_has_50_percent_on_fire() {
        use terravibe_sim::enemy::EnemyKind;
        use terravibe_sim::inventory::ItemStack;
        use terravibe_sim::item::{FIERY_GREATSWORD, Prefix};

        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.rng = Rng::new(0xDEAD_BEEF);

        s.conns[0].inv.slots[0] = Some(ItemStack {
            id: FIERY_GREATSWORD,
            count: 1,
            prefix: Prefix(0),
        });
        s.conns[0].inv.selected = 0;

        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        let zi = s.enemies.len();
        s.add_enemy(EnemyKind::Zombie, [pos[0], pos[1], pos[2] - 1.5]);

        let mut burned = 0;
        let trials = 20;
        for i in 0..trials {
            s.conns[0].use_state.time = 0;
            s.conns[0].use_state.anim = 0;
            s.conns[0].use_state.last_use_held = false;
            s.enemies[zi].hp = 500;
            s.enemies[zi].hurt_ticks = 0;
            s.enemies[zi].vel = [0.0; 3];
            s.enemies[zi].on_fire_ticks = 0;
            s.enemies[zi].pos = [pos[0], pos[1], pos[2] - 1.5];

            c.send(ClientMsg::Input {
                seq: i as u32 + 1,
                input: PlayerInput {
                    use_held: true,
                    ..Default::default()
                },
            })
            .unwrap();
            s.tick();

            if s.enemies[zi].is_on_fire() {
                burned += 1;
            }
        }
        assert!(
            burned > 0 && burned < trials,
            "expected ~50% On Fire on direct Fiery Greatsword hits, got {burned}/{trials}"
        );
    }

    #[test]
    fn phoenix_blaster_consumes_bullets_and_fires_fast() {
        use terravibe_sim::inventory::{AMMO0, ItemStack};
        use terravibe_sim::item::{MUSKET_BALL, PHOENIX_BLASTER, Prefix};
        use terravibe_sim::projectile::ProjKind;

        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        s.conns[0].inv.slots[0] = Some(ItemStack {
            id: PHOENIX_BLASTER,
            count: 1,
            prefix: Prefix(0),
        });
        s.conns[0].inv.slots[AMMO0] = Some(ItemStack {
            id: MUSKET_BALL,
            count: 10,
            prefix: Prefix(0),
        });
        s.conns[0].inv.selected = 0;
        s.conns[0].use_state.time = 0;
        s.conns[0].use_state.anim = 0;
        s.conns[0].use_state.last_use_held = false;

        c.send(ClientMsg::Input {
            seq: 1,
            input: PlayerInput {
                use_held: true,
                yaw: 0.0,
                pitch: 0.0,
                ..Default::default()
            },
        })
        .unwrap();
        s.tick();

        assert_eq!(
            s.conns[0].inv.slots[AMMO0].as_ref().unwrap().count,
            9,
            "firing the Phoenix Blaster must consume 1 bullet"
        );
        assert!(
            s.projectiles.iter().any(|p| p.kind == ProjKind::Bullet),
            "Phoenix Blaster must spawn a Bullet projectile"
        );
        let d = item_data(PHOENIX_BLASTER);
        assert_eq!(d.use_time, 11, "Phoenix Blaster fires fast");
        assert!(!d.auto_reuse, "Phoenix Blaster is semi-automatic");
    }

    #[test]
    fn flamarang_pierces_two_enemies() {
        use terravibe_sim::enemy::{EnemyId, EnemyKind};
        use terravibe_sim::inventory::ItemStack;
        use terravibe_sim::item::{FLAMARANG, Prefix};
        use terravibe_sim::projectile::ProjKind;

        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        s.conns[0].inv.slots[0] = Some(ItemStack {
            id: FLAMARANG,
            count: 1,
            prefix: Prefix(0),
        });
        s.conns[0].inv.selected = 0;
        s.conns[0].use_state.time = 0;
        s.conns[0].use_state.anim = 0;
        s.conns[0].use_state.last_use_held = false;

        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        // Two zombies aligned with the throw, one behind the other.
        s.add_enemy(EnemyKind::Zombie, [pos[0], pos[1], pos[2] - 2.0]);
        s.add_enemy(EnemyKind::Zombie, [pos[0], pos[1], pos[2] - 4.0]);
        let ids: Vec<EnemyId> = s
            .enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::Zombie && (e.pos[2] - pos[2]).abs() < 5.0)
            .map(|e| e.id)
            .collect();
        assert_eq!(ids.len(), 2);
        let before: Vec<i32> = ids
            .iter()
            .map(|id| s.enemies.iter().find(|e| e.id == *id).unwrap().hp)
            .collect();

        c.send(ClientMsg::Input {
            seq: 1,
            input: PlayerInput {
                use_held: true,
                yaw: 0.0,
                pitch: 0.0,
                ..Default::default()
            },
        })
        .unwrap();
        s.tick();
        for _ in 0..25 {
            s.tick();
        }

        assert!(
            s.enemies
                .iter()
                .find(|e| e.id == ids[0])
                .is_none_or(|e| e.hp < before[0]),
            "first zombie must take damage (or die) on outward hit"
        );
        // After bouncing off zombie 1 and returning, or continuing, at least zombie 1 was hit.
        // Verify bounce toggled ai0 or returned safely.
        assert!(
            s.projectiles
                .iter()
                .find(|p| p.kind == ProjKind::Flamarang)
                .is_none_or(|p| p.ai0 == 1),
            "Flamarang switches to return trip after outward hit"
        );
    }

    #[test]
    fn flamarang_scales_with_molten_melee_damage_bonus() {
        use terravibe_sim::inventory::ItemStack;
        use terravibe_sim::item::{
            FLAMARANG, MOLTEN_BREASTPLATE, MOLTEN_GREAVES, MOLTEN_HELMET, Prefix,
        };
        use terravibe_sim::projectile::ProjKind;

        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        let p = s.conns[0].player.as_mut().unwrap();
        p.armor = [
            Some(MOLTEN_HELMET),
            Some(MOLTEN_BREASTPLATE),
            Some(MOLTEN_GREAVES),
        ];
        p.sync_accessories();
        s.conns[0].inv.slots[0] = Some(ItemStack {
            id: FLAMARANG,
            count: 1,
            prefix: Prefix(0),
        });
        s.conns[0].inv.selected = 0;
        s.conns[0].use_state.time = 0;
        s.conns[0].use_state.anim = 0;
        s.conns[0].use_state.last_use_held = false;

        c.send(ClientMsg::Input {
            seq: 1,
            input: PlayerInput {
                use_held: true,
                yaw: 0.0,
                pitch: 0.0,
                ..Default::default()
            },
        })
        .unwrap();
        s.tick();

        let proj = s
            .projectiles
            .iter()
            .find(|pr| pr.kind == ProjKind::Flamarang)
            .expect("Flamarang projectile must spawn");
        let expected = (item_data(FLAMARANG).damage as f32 * 1.17).round_ties_even() as i32;
        assert_eq!(proj.damage, expected);
    }

    #[test]
    fn potion_server_heals_once_then_sickness_blocks() {
        use terravibe_sim::item::LESSER_HEALING_POTION;
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        give(&mut s, LESSER_HEALING_POTION, 3);
        let slot = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|x| x.id == LESSER_HEALING_POTION))
            .unwrap();
        assert!(slot < 10);
        s.conns[0].player.as_mut().unwrap().vitals.life = 20;
        let input = PlayerInput {
            use_held: true,
            slot: slot as u8,
            ..Default::default()
        };
        for seq in 1..=60 {
            c.send(ClientMsg::Input { seq, input }).unwrap();
            s.tick();
        }
        let p = s.conns[0].player.as_ref().unwrap();
        // One potion drunk (70 life, <= regen); sickness refuses the held repeats.
        assert!(p.vitals.life >= 70 && p.vitals.life < 100);
        assert!(p.vitals.potion_delay_ticks > 3000);
        assert_eq!(s.conns[0].inv.count_of(LESSER_HEALING_POTION), 2);
    }

    #[test]
    fn life_max_syncs_to_client() {
        use terravibe_sim::item::LIFE_CRYSTAL_ITEM;
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        give(&mut s, LIFE_CRYSTAL_ITEM, 1);
        let slot = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|x| x.id == LIFE_CRYSTAL_ITEM))
            .unwrap();
        got.clear();
        // Hold use on the crystal's slot (hotbar).
        assert!(slot < 10);
        let input = PlayerInput {
            use_held: true,
            slot: slot as u8,
            ..Default::default()
        };
        for seq in 1..=40 {
            c.send(ClientMsg::Input { seq, input }).unwrap();
            s.tick();
        }
        drain(&c, &mut got);
        let p = s.conns[0].player.as_ref().unwrap();
        assert_eq!(p.vitals.life_max, 120);
        assert_eq!(s.conns[0].inv.count_of(LIFE_CRYSTAL_ITEM), 0);
        assert!(got.iter().any(|m| matches!(m,
            ServerMsg::PlayerState { vitals, .. } if vitals.life_max == 120)));
    }

    #[test]
    fn heart_drop_heals_player_and_is_not_stored() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.conns[0].player.as_mut().unwrap().vitals.life = 50;
        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        s.add_drop(
            terravibe_sim::inventory::ItemStack {
                id: HEART,
                count: 1,
                prefix: terravibe_sim::item::Prefix(0),
            },
            pos,
        );
        for _ in 0..(PICKUP_DELAY + 5) {
            s.tick();
        }
        assert_eq!(s.conns[0].player.as_ref().unwrap().vitals.life, 70);
        assert!(s.drops.is_empty());
        assert_eq!(s.conns[0].inv.count_of(HEART), 0);
    }

    #[test]
    fn critter_death_never_drops_hearts() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.conns[0].player.as_mut().unwrap().vitals.life = 10;
        let p_pos = s.conns[0].player.as_ref().unwrap().body.pos;
        for _ in 0..50 {
            s.add_enemy(EnemyKind::Bunny, p_pos);
            if let Some(e) = s.enemies.last_mut() {
                e.hp = 0;
            }
            s.tick();
        }
        assert!(
            !s.drops.iter().any(|d| d.stack.id == HEART),
            "critters should never drop hearts on death"
        );
    }

    #[test]
    fn idle_client_gets_player_state_when_hit() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        let full = s.conns[0].player.as_ref().unwrap().vitals.life;
        s.add_enemy(EnemyKind::Zombie, pos);
        got.clear();
        // No inputs are sent at all.
        for _ in 0..3 {
            s.tick();
        }
        drain(&c, &mut got);
        assert!(got.iter().any(|m| matches!(m,
            ServerMsg::PlayerState { vitals, .. } if vitals.life < full)));
    }

    #[test]
    fn idle_client_gets_player_state_when_mana_changes() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        // Mana can change outside input handling (as HP does when hit); an idle
        // client must still be told.
        s.conns[0].player.as_mut().unwrap().vitals.mana = 7;
        got.clear();
        for _ in 0..2 {
            s.tick();
        }
        drain(&c, &mut got);
        assert!(got.iter().any(|m| matches!(m,
            ServerMsg::PlayerState { vitals, .. } if vitals.mana == 7)));
    }

    /// Latest streamed inventory in `got`, falling back to the server's copy.
    fn inv_of(s: &Server, got: &[ServerMsg]) -> Inventory {
        got.iter()
            .rev()
            .find_map(|m| match m {
                ServerMsg::Inventory(i) => Some(i.clone()),
                _ => None,
            })
            .unwrap_or_else(|| s.conns[0].inv.clone())
    }

    fn set_block(s: &mut Server, p: [i32; 3], b: BlockId) {
        s.set_block(p, b);
    }

    /// Stands on every drop for long enough to pick it up.
    fn collect_drops(s: &mut Server, c: &InProcClient, got: &mut Vec<ServerMsg>) {
        for _ in 0..(PICKUP_DELAY + 5) {
            s.tick();
        }
        let spots: Vec<[f32; 3]> = s.drops.iter().map(|d| d.pos).collect();
        for p in spots {
            s.conns[0].player.as_mut().unwrap().body.pos = p;
            for _ in 0..5 {
                s.tick();
            }
        }
        drain(c, got);
    }

    #[test]
    fn progression_chain_copper_to_gold_gear() {
        use terravibe_sim::crafting::{
            ANVIL_RECIPE, COPPER_BAR_RECIPE, COPPER_CHAINMAIL_RECIPE, COPPER_HELMET_RECIPE,
            COPPER_PICKAXE_RECIPE, FURNACE_RECIPE, TORCH_RECIPE, WORKBENCH_RECIPE,
        };
        use terravibe_sim::item::{
            COPPER_BAR, COPPER_CHAINMAIL, COPPER_HELMET, COPPER_ORE, COPPER_PICKAXE, GEL, IRON_BAR,
            STONE_BLOCK, WOOD,
        };
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        for (item, n) in [
            (COPPER_ORE, 120),
            (WOOD, 25),
            (STONE_BLOCK, 20),
            (GEL, 1),
            (IRON_BAR, 5),
        ] {
            give(&mut s, item, n);
        }
        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        let (fx, fy, fz) = (
            pos[0].floor() as i32,
            pos[1].floor() as i32,
            pos[2].floor() as i32,
        );
        // Stations are placed straight into the server world (placement itself is covered by
        // the interact tests); everything else goes through messages.
        let crafted = |s: &mut Server, id, n| result_of(&craft_req(s, &c, id, n));
        // A furnace recipe fails without a bench nearby, then works after one is built.
        assert_eq!(
            crafted(&mut s, COPPER_BAR_RECIPE, 1),
            Some((0, Some(CraftError::MissingStation)))
        );
        assert_eq!(crafted(&mut s, WORKBENCH_RECIPE, 1), Some((1, None)));
        set_block(&mut s, [fx + 2, fy, fz], terravibe_sim::WORKBENCH);
        assert_eq!(crafted(&mut s, TORCH_RECIPE, 1), Some((3, None)));
        assert_eq!(crafted(&mut s, FURNACE_RECIPE, 1), Some((1, None)));
        set_block(&mut s, [fx + 3, fy, fz], terravibe_sim::FURNACE);
        assert_eq!(crafted(&mut s, COPPER_BAR_RECIPE, 40), Some((40, None)));
        assert_eq!(s.conns[0].inv.count_of(COPPER_BAR), 40);
        assert_eq!(crafted(&mut s, ANVIL_RECIPE, 1), Some((1, None)));
        // No anvil placed yet.
        assert_eq!(
            crafted(&mut s, COPPER_PICKAXE_RECIPE, 1),
            Some((0, Some(CraftError::MissingStation)))
        );
        set_block(&mut s, [fx + 4, fy, fz], terravibe_sim::ANVIL);
        assert_eq!(crafted(&mut s, COPPER_PICKAXE_RECIPE, 1), Some((1, None)));
        assert_eq!(crafted(&mut s, COPPER_HELMET_RECIPE, 1), Some((1, None)));
        assert_eq!(crafted(&mut s, COPPER_CHAINMAIL_RECIPE, 1), Some((1, None)));
        assert!(s.conns[0].inv.count_of(COPPER_PICKAXE) > 0);
        // Equip both pieces by message.
        for item in [COPPER_HELMET, COPPER_CHAINMAIL] {
            let slot = s.conns[0]
                .inv
                .slots
                .iter()
                .position(|x| x.is_some_and(|x| x.id == item))
                .unwrap();
            c.send(ClientMsg::Equip { from: slot as u8 }).unwrap();
            s.tick();
        }
        assert_eq!(s.conns[0].player.as_ref().unwrap().defense(), 3);
        // A zombie hit (14) is reduced by (defense + 1) / 2 = 2.
        s.add_enemy(
            EnemyKind::Zombie,
            s.conns[0].player.as_ref().unwrap().body.pos,
        );
        got.clear();
        s.tick();
        drain(&c, &mut got);
        assert!(
            got.iter()
                .any(|m| matches!(m, ServerMsg::PlayerHit { amount: 12 })),
            "expected a reduced hit of 12"
        );
    }

    #[test]
    fn progression_chain_molten_gear() {
        use terravibe_sim::crafting::{
            HELLSTONE_BAR_RECIPE, MOLTEN_BREASTPLATE_RECIPE, MOLTEN_GREAVES_RECIPE,
            MOLTEN_HELMET_RECIPE, MOLTEN_PICKAXE_RECIPE,
        };
        use terravibe_sim::item::{
            HELLSTONE_BAR, HELLSTONE_ITEM, MOLTEN_BREASTPLATE, MOLTEN_GREAVES, MOLTEN_HELMET,
            MOLTEN_PICKAXE, OBSIDIAN_ITEM,
        };
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        for (item, n) in [(HELLSTONE_ITEM, 30), (OBSIDIAN_ITEM, 10)] {
            give(&mut s, item, n);
        }
        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        let (fx, fy, fz) = (
            pos[0].floor() as i32,
            pos[1].floor() as i32,
            pos[2].floor() as i32,
        );
        let crafted = |s: &mut Server, id, n| result_of(&craft_req(s, &c, id, n));
        // The bar needs a Hellforge: neither bare ground nor a Furnace will do.
        assert_eq!(
            crafted(&mut s, HELLSTONE_BAR_RECIPE, 1),
            Some((0, Some(CraftError::MissingStation)))
        );
        set_block(&mut s, [fx + 2, fy, fz], terravibe_sim::FURNACE);
        assert_eq!(
            crafted(&mut s, HELLSTONE_BAR_RECIPE, 1),
            Some((0, Some(CraftError::MissingStation)))
        );
        set_block(&mut s, [fx + 3, fy, fz], terravibe_sim::HELLFORGE);
        assert_eq!(crafted(&mut s, HELLSTONE_BAR_RECIPE, 10), Some((10, None)));
        assert_eq!(s.conns[0].inv.count_of(HELLSTONE_BAR), 10);
        // The whole set costs 65 bars; smelt the rest from more ore.
        give(&mut s, HELLSTONE_ITEM, 165);
        give(&mut s, OBSIDIAN_ITEM, 55);
        assert_eq!(crafted(&mut s, HELLSTONE_BAR_RECIPE, 55), Some((55, None)));
        assert_eq!(s.conns[0].inv.count_of(HELLSTONE_BAR), 65);
        // No anvil placed yet.
        assert_eq!(
            crafted(&mut s, MOLTEN_HELMET_RECIPE, 1),
            Some((0, Some(CraftError::MissingStation)))
        );
        set_block(&mut s, [fx + 4, fy, fz], terravibe_sim::ANVIL);
        assert_eq!(crafted(&mut s, MOLTEN_PICKAXE_RECIPE, 1), Some((1, None)));
        assert_eq!(crafted(&mut s, MOLTEN_HELMET_RECIPE, 1), Some((1, None)));
        assert_eq!(
            crafted(&mut s, MOLTEN_BREASTPLATE_RECIPE, 1),
            Some((1, None))
        );
        assert_eq!(crafted(&mut s, MOLTEN_GREAVES_RECIPE, 1), Some((1, None)));
        // Equip all three pieces by message.
        for item in [MOLTEN_HELMET, MOLTEN_BREASTPLATE, MOLTEN_GREAVES] {
            let slot = s.conns[0]
                .inv
                .slots
                .iter()
                .position(|x| x.is_some_and(|x| x.id == item))
                .unwrap();
            c.send(ClientMsg::Equip { from: slot as u8 }).unwrap();
            s.tick();
        }
        let p = s.conns[0].player.as_ref().unwrap();
        assert_eq!(p.defense(), 25);
        assert!(p.has_molten_set());
        assert!(p.fire_walk());
        assert!(p.body.has_molten_set);
        assert!(
            (p.melee_damage_mult() - 1.17).abs() < 1e-5,
            "got {}",
            p.melee_damage_mult()
        );
        assert_eq!(p.melee_crit_bonus(), 7);
        assert_eq!(p.melee_speed_mult(), 0.93);
        assert!(s.conns[0].inv.count_of(MOLTEN_PICKAXE) > 0);
        // Full Molten set also refuses On Fire! ignition.
        let p_mut = s.conns[0].player.as_mut().unwrap();
        p_mut.ignite();
        assert_eq!(p_mut.vitals.burn_ticks, 0);
    }

    #[test]
    fn progression_chain_end_of_era() {
        use terravibe_sim::ChunkPos;
        use terravibe_sim::enemy::EnemyKind;
        use terravibe_sim::item::{
            BREAKER_BLADE, CLOCKWORK_ASSAULT_RIFLE, FIERY_GREATSWORD, FIRECRACKER,
            GUIDE_VOODOO_DOLL, LASER_RIFLE, MOLTEN_BREASTPLATE, MOLTEN_GREAVES, MOLTEN_HELMET,
            PHOENIX_BLASTER, PWNHAMMER, RANGER_EMBLEM, SORCERER_EMBLEM, WARRIOR_EMBLEM,
        };
        use terravibe_sim::liquid_cell::Liquid;
        use terravibe_sim::net::{ClientMsg, ServerMsg};
        use terravibe_sim::projectile::ProjKind;

        let (mut s, c) = setup();
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        // 1. Molten-equipped player
        give(&mut s, MOLTEN_HELMET, 1);
        give(&mut s, MOLTEN_BREASTPLATE, 1);
        give(&mut s, MOLTEN_GREAVES, 1);
        give(&mut s, FIERY_GREATSWORD, 1);
        give(&mut s, PHOENIX_BLASTER, 1);

        for item in [MOLTEN_HELMET, MOLTEN_BREASTPLATE, MOLTEN_GREAVES] {
            let slot = s.conns[0]
                .inv
                .slots
                .iter()
                .position(|x| x.is_some_and(|st| st.id == item))
                .unwrap();
            c.send(ClientMsg::Equip { from: slot as u8 }).unwrap();
            s.tick();
        }
        let p = s.conns[0].player.as_ref().unwrap();
        assert_eq!(p.defense(), 25);
        assert!(p.has_molten_set());
        assert!(p.fire_walk());
        assert!(s.conns[0].inv.count_of(FIERY_GREATSWORD) >= 1);
        assert!(s.conns[0].inv.count_of(PHOENIX_BLASTER) >= 1);

        // 2. Live Guide NPC
        s.add_enemy(EnemyKind::Guide, [0.0, 40.0, 0.0]);
        assert!(s.town_alive(TownKind::Guide));

        // 3. Underworld lava pool at y = 10..=11
        let pos = [10.0, 12.0, 10.0];
        s.conns[0].player.as_mut().unwrap().body.pos = pos;
        let cp = ChunkPos::of_block(10, 10, 10);
        s.world
            .entry(cp)
            .or_insert_with(|| Arc::new(Chunk::filled(terravibe_sim::AIR)));
        for dx in 0..=4 {
            for dz in 0..=4 {
                set_block(&mut s, [10 + dx, 9, 10 + dz], terravibe_sim::STONE);
                for dy in 10..=14 {
                    set_block(&mut s, [10 + dx, dy, 10 + dz], terravibe_sim::AIR);
                }
            }
        }
        {
            let mut adapter = ServerLiquidWorld::new(&mut s.world);
            for dx in 0..=4 {
                for dz in 0..=4 {
                    adapter.set_liquid(10 + dx, 10, 10 + dz, Some(Liquid::lava(255)));
                    adapter.set_liquid(10 + dx, 11, 10 + dz, Some(Liquid::lava(255)));
                }
            }
        }

        // 4. Drop Guide Voodoo Doll into Underworld lava
        give(&mut s, GUIDE_VOODOO_DOLL, 1);
        let doll_slot = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|st| st.id == GUIDE_VOODOO_DOLL))
            .unwrap();
        s.conns[0].inv.selected = doll_slot;
        c.send(ClientMsg::DropItem {
            dir: [1.0, -0.5, 0.0],
            from_cursor: false,
        })
        .unwrap();

        for _ in 0..25 {
            s.tick();
            if s.wof.is_some() {
                break;
            }
        }

        // 5. Guide dies, WallOfFleshMouth + 2 Eyes + TheHungry spawn
        assert!(
            s.wof.is_some(),
            "Wall of Flesh must spawn when doll touches Underworld lava"
        );
        assert!(!s.town_alive(TownKind::Guide), "Guide must die on summon");
        assert!(
            s.enemies
                .iter()
                .any(|e| e.kind == EnemyKind::WallOfFleshMouth),
            "WallOfFleshMouth must spawn"
        );
        assert_eq!(
            s.enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::WallOfFleshEye)
                .count(),
            2,
            "Two WallOfFleshEye parts must spawn"
        );
        assert!(
            s.enemies.iter().any(|e| e.kind == EnemyKind::TheHungry),
            "The Hungry must spawn attached to wall"
        );

        // 6. ServerMsg::WofState streams
        drain(&c, &mut got);
        assert!(
            has(&got, |m| matches!(m, ServerMsg::WofState { .. })),
            "ServerMsg::WofState must be sent to client"
        );

        // 7. Eyes fire ProjKind::EyeLaser
        for _ in 0..130 {
            s.tick();
            if s.projectiles.iter().any(|p| p.kind == ProjKind::EyeLaser) {
                break;
            }
        }
        assert!(
            s.projectiles.iter().any(|p| p.kind == ProjKind::EyeLaser),
            "Eyes must fire ProjKind::EyeLaser"
        );

        // 8. Damage dealt to an Eye reduces the shared 8000 HP pool
        let eye_id = s.wof.as_ref().unwrap().eye_ids[0];
        s.enemies.iter_mut().find(|e| e.id == eye_id).unwrap().hp -= 500;
        s.tick();
        assert_eq!(
            s.wof.as_ref().unwrap().hp,
            7500,
            "Damage to Eye must reduce shared WoF HP pool"
        );
        let mouth_id = s.wof.as_ref().unwrap().mouth_id;
        assert_eq!(
            s.enemies.iter().find(|e| e.id == mouth_id).unwrap().hp,
            7500,
            "Damage must sync to Mouth HP"
        );

        // 9. Defeat Wall of Flesh
        s.enemies.iter_mut().find(|e| e.id == eye_id).unwrap().hp = 0;
        s.tick();
        drain(&c, &mut got);

        // 10. Drops, facts flag, messages, pre-Hardmode preserved
        assert!(s.wof.is_none(), "WoF state cleared on defeat");
        assert!(
            s.facts.downed_wall_of_flesh,
            "facts.downed_wall_of_flesh flipped to true"
        );
        assert!(
            has(&got, |m| matches!(m, ServerMsg::WallOfFleshDefeated)),
            "ServerMsg::WallOfFleshDefeated emitted"
        );
        assert!(
            has(&got, |m| matches!(m, ServerMsg::WofGone)),
            "ServerMsg::WofGone emitted"
        );
        assert!(
            s.drops.iter().any(|d| d.stack.id == PWNHAMMER),
            "Must drop PWNHAMMER"
        );
        assert!(
            s.drops
                .iter()
                .any(|d| matches!(d.stack.id, WARRIOR_EMBLEM | RANGER_EMBLEM | SORCERER_EMBLEM)),
            "Must drop exactly one class emblem"
        );
        assert!(
            s.drops.iter().any(|d| matches!(
                d.stack.id,
                BREAKER_BLADE | CLOCKWORK_ASSAULT_RIFLE | LASER_RIFLE | FIRECRACKER
            )),
            "Must drop exactly one WoF weapon"
        );
        // Pre-Hardmode world preserved: world state remains valid and pre-Hardmode
        assert!(!s.downed_skeletron);
    }

    #[test]
    fn playable_loop_chop_fight_craft() {
        use terravibe_sim::block::TRUNK;
        use terravibe_sim::crafting::{TORCH_RECIPE, WOODEN_SWORD_RECIPE, WORKBENCH_RECIPE};
        use terravibe_sim::item::{GEL, TORCH, WOOD, WOODEN_SWORD};
        let (client, conn) = in_process_pair();
        let cfg = ServerCfg {
            natural_spawns: false,
            ..Default::default()
        };
        let mut s = Server::with_cfg(1, cfg);
        s.connect(Box::new(conn));
        let c = client;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        let home = s.conns[0].player.as_ref().unwrap().body.pos;
        let y0 = home[1].floor() as i32;
        let (hx, hz) = (home[0].floor() as i32, home[2].floor() as i32);
        let face_x = -std::f32::consts::FRAC_PI_2;
        // hello_and_load only waits for the spawn chunk; the trunk column may span
        // neighbouring chunks that stream in asynchronously. Wait for all of them.
        let t0 = Instant::now();
        while !(0..5).all(|dy| {
            s.world
                .contains_key(&ChunkPos::of_block(hx + 2, y0 + dy, hz))
        }) {
            assert!(
                t0.elapsed() < Duration::from_secs(5),
                "trunk chunks never loaded"
            );
            s.tick();
            drain(&c, &mut got);
            std::thread::sleep(Duration::from_millis(1));
        }

        // 1. Chop trees (real use-held inputs, copper axe slot 2) until 18 wood.
        let mut seq = 0u32;
        let mut rounds = 0;
        while inv_of(&s, &got).count_of(WOOD) < 18 {
            rounds += 1;
            assert!(rounds <= 8, "never got enough wood");
            let trunk: Vec<[i32; 3]> = (0..5).map(|dy| [hx + 2, y0 + dy, hz]).collect();
            for p in &trunk {
                set_block(&mut s, *p, TRUNK);
            }
            s.conns[0].player.as_mut().unwrap().body.pos = home;
            let input = PlayerInput {
                yaw: face_x,
                pitch: -0.5,
                use_held: true,
                slot: 2,
                ..Default::default()
            };
            for _ in 0..500 {
                seq += 1;
                c.send(ClientMsg::Input { seq, input }).unwrap();
                s.tick();
                if trunk
                    .iter()
                    .all(|p| WorldQ(&s.world).block(p[0], p[1], p[2]) == AIR)
                {
                    break;
                }
            }
            drain(&c, &mut got);
            assert!(
                trunk
                    .iter()
                    .all(|p| WorldQ(&s.world).block(p[0], p[1], p[2]) == AIR),
                "trunk not felled"
            );
            collect_drops(&mut s, &c, &mut got);
            assert!(inv_of(&s, &got).count_of(WOOD) > 0);
        }
        assert!(s.drops.is_empty());

        // 2. Kill a slime with the broadsword (slot 0), pick up its gel.
        s.conns[0].player.as_mut().unwrap().body.pos = home;
        add_on_surface(&mut s, &c, &mut got, EnemyKind::Slime, 2);
        let slime_home = s.enemies[0].pos;
        for _ in 0..220 {
            seq += 1;
            let input = PlayerInput {
                yaw: face_x,
                use_held: seq % 2 == 1,
                slot: 0,
                ..Default::default()
            };
            c.send(ClientMsg::Input { seq, input }).unwrap();
            if let Some(e) = s.enemies.first_mut() {
                e.pos = slime_home;
                e.vel = [0.0; 3];
            }
            s.tick();
            if s.enemies.is_empty() {
                break;
            }
        }
        assert!(
            s.enemies.is_empty(),
            "slime survived the sword {:?} {:?} {:?}",
            s.enemies[0].hp,
            s.enemies[0].pos,
            s.conns[0].player.as_ref().unwrap().body.pos
        );
        s.conns[0].player.as_mut().unwrap().body.pos = home;
        collect_drops(&mut s, &c, &mut got);
        let inv = inv_of(&s, &got);
        assert!(inv.count_of(GEL) >= 1, "no gel");
        let (wood0, gel0) = (inv.count_of(WOOD), inv.count_of(GEL));

        // 3. Craft a workbench (no station needed).
        let got2 = craft_req(&mut s, &c, WORKBENCH_RECIPE, 1);
        assert_eq!(result_of(&got2), Some((1, None)));
        let inv = inv_of(&s, &got2);
        assert_eq!(inv.count_of(terravibe_sim::item::WORKBENCH_ITEM), 1);
        assert_eq!(inv.count_of(WOOD), wood0 - 10);

        // Sword needs the station: refused before placing.
        let got2 = craft_req(&mut s, &c, WOODEN_SWORD_RECIPE, 1);
        assert_eq!(
            result_of(&got2),
            Some((0, Some(CraftError::MissingStation)))
        );

        // 4. Place it: select its hotbar slot, face +x and slightly down.
        let slot = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|x| x.id == terravibe_sim::item::WORKBENCH_ITEM))
            .unwrap();
        assert!(slot < 10, "workbench not on hotbar");
        s.conns[0].player.as_mut().unwrap().body.pos = home;
        let input = PlayerInput {
            yaw: face_x,
            pitch: -0.4,
            use_held: true,
            slot: slot as u8,
            ..Default::default()
        };
        for _ in 0..60 {
            seq += 1;
            c.send(ClientMsg::Input { seq, input }).unwrap();
            s.tick();
            if s.conns[0].inv.count_of(terravibe_sim::item::WORKBENCH_ITEM) == 0 {
                break;
            }
        }
        assert_eq!(
            s.conns[0].inv.count_of(terravibe_sim::item::WORKBENCH_ITEM),
            0
        );
        drain(&c, &mut got);

        // 5. Torches (gel + wood) and the wooden sword at the bench.
        let got2 = craft_req(&mut s, &c, TORCH_RECIPE, 1);
        assert_eq!(result_of(&got2), Some((3, None)));
        let inv = inv_of(&s, &got2);
        assert_eq!(inv.count_of(TORCH), 3);
        assert_eq!(inv.count_of(GEL), gel0 - 1);
        let got2 = craft_req(&mut s, &c, WOODEN_SWORD_RECIPE, 1);
        assert_eq!(result_of(&got2), Some((1, None)), "sword at bench");
        let inv = inv_of(&s, &got2);
        assert_eq!(inv.count_of(WOODEN_SWORD), 1);
        assert_eq!(inv.count_of(WOOD), wood0 - 10 - 1 - 7);
    }

    fn saving_cfg(tag: &str) -> (ServerCfg, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("tv_srv_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cfg = ServerCfg {
            natural_spawns: false,
            save: Some(SaveCfg {
                root: root.clone(),
                world: "w".into(),
            }),
            ..ServerCfg::default()
        };
        (cfg, root)
    }

    #[test]
    fn server_restart_restores_edit_and_clock() {
        let (cfg, root) = saving_cfg("restart");
        let mut a = Server::with_cfg(7, cfg);
        let cp = ChunkPos { x: 0, y: 3, z: 0 };
        a.world.insert(cp, Arc::new(generate_chunk(7, cp)));
        a.edited.insert(cp);
        a.dirty.insert(cp);
        Arc::make_mut(a.world.get_mut(&cp).unwrap()).set(4, 5, 6, terravibe_sim::block::WORKBENCH);
        for _ in 0..10 {
            a.tick();
        }
        let tick = a.clock.tick;
        a.save_now().unwrap();
        drop(a);

        let (mut cfg2, _) = saving_cfg("restart_unused");
        cfg2.save = Some(SaveCfg {
            root: root.clone(),
            world: "w".into(),
        });
        // Different seed argument: the stored seed wins.
        let b = Server::with_cfg(99, cfg2);
        assert_eq!(b.seed, 7);
        assert_eq!(b.clock.tick, tick);
        assert_eq!(b.world[&cp].get(4, 5, 6), terravibe_sim::block::WORKBENCH);
        assert!(b.edited.contains(&cp));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn player_state_restores_by_name_after_disconnect() {
        let (cfg, root) = saving_cfg("player");
        let mut a = Server::with_cfg(1, cfg);
        let (c, conn) = in_process_pair();
        a.connect(Box::new(conn));
        c.send(ClientMsg::Hello {
            seed: 1,
            name: Some("ann".into()),
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        a.tick();
        a.conns[0].inv.slots[20] = Some(terravibe_sim::inventory::ItemStack {
            id: terravibe_sim::item::WOOD,
            count: 77,
            prefix: terravibe_sim::item::Prefix(0),
        });
        a.conns[0].player.as_mut().unwrap().vitals.life_max = 140;
        c.send(ClientMsg::Bye).unwrap();
        a.tick();
        assert_eq!(a.connection_count(), 0);
        a.save_now().unwrap();
        drop(a);

        let (mut cfg2, _) = saving_cfg("player_unused");
        cfg2.save = Some(SaveCfg {
            root: root.clone(),
            world: "w".into(),
        });
        let mut b = Server::with_cfg(1, cfg2);
        let (c2, conn2) = in_process_pair();
        b.connect(Box::new(conn2));
        c2.send(ClientMsg::Hello {
            seed: 1,
            name: Some("ann".into()),
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        b.tick();
        assert_eq!(b.conns[0].inv.count_of(terravibe_sim::item::WOOD), 77);
        assert_eq!(b.conns[0].player.as_ref().unwrap().vitals.life_max, 140);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn autosave_runs_off_tick_and_writes_dirty_chunks() {
        let (cfg, root) = saving_cfg("auto");
        let mut a = Server::with_cfg(1, cfg);
        let cp = ChunkPos { x: 2, y: 3, z: 0 };
        a.world.insert(cp, Arc::new(generate_chunk(1, cp)));
        a.edited.insert(cp);
        a.dirty.insert(cp);
        a.save_ticks = AUTOSAVE_TICKS - 1;
        a.tick();
        assert!(a.dirty.is_empty());
        drop(a); // joins the worker
        assert!(root.join("w/chunks/2_3_0.bin").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    /// Loads the spawn chunk and puts a chest block at (dx, feet level, 0).
    fn chest_setup(dx: i32) -> (Server, InProcClient, Vec<ServerMsg>, [i32; 3]) {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        let pos = [dx, spawn_point(1)[1] as i32, 0];
        // Make sure the chest's chunk is resident (spawn is at y=64, so cp.y=2 while hello_and_load checks y=63, cp.y=1).
        let cp = ChunkPos::of_block(pos[0], pos[1], pos[2]);
        assert!(pump(&mut s, &c, &mut got, |g| has_chunk(g, cp)));
        assert!(s.world.contains_key(&cp));
        set_block(&mut s, pos, CHEST);
        (s, c, got, pos)
    }

    fn has(got: &[ServerMsg], f: impl Fn(&ServerMsg) -> bool) -> bool {
        got.iter().any(f)
    }

    fn stack(id: ItemId, count: u32) -> terravibe_sim::inventory::ItemStack {
        terravibe_sim::inventory::ItemStack {
            id,
            count,
            prefix: terravibe_sim::item::Prefix(0),
        }
    }

    fn click(c: &InProcClient, at: SlotRef, kind: ClickKind) {
        c.send(ClientMsg::ClickSlot { at, kind }).unwrap();
    }

    fn last_held(got: &[ServerMsg]) -> Option<Option<terravibe_sim::inventory::ItemStack>> {
        got.iter().rev().find_map(|m| match m {
            ServerMsg::Held(h) => Some(*h),
            _ => None,
        })
    }

    #[test]
    fn click_slot_swaps_and_streams_inventory_and_held() {
        use terravibe_sim::item::{TORCH, WOOD};
        let (mut s, c, mut got, _) = chest_setup(1);
        s.conns[0].inv.slots[0] = Some(stack(WOOD, 10));
        s.conns[0].inv.slots[1] = Some(stack(TORCH, 5));
        got.clear();
        click(&c, SlotRef::Inv(0), ClickKind::Left);
        s.tick();
        drain(&c, &mut got);
        assert_eq!(last_held(&got), Some(Some(stack(WOOD, 10))));
        got.clear();
        click(&c, SlotRef::Inv(1), ClickKind::Left);
        s.tick();
        drain(&c, &mut got);
        assert_eq!(last_held(&got), Some(Some(stack(TORCH, 5))));
        let inv = inv_of(&s, &got);
        assert_eq!(inv.slots[0], None);
        assert_eq!(inv.slots[1], Some(stack(WOOD, 10)));
        assert_eq!(s.conns[0].held.0, Some(stack(TORCH, 5)));
    }

    #[test]
    fn click_accessory_slot_equips_and_syncs_accessories() {
        use terravibe_sim::item::HERMES_BOOTS;
        let (mut s, c, mut got, _) = chest_setup(1);
        s.conns[0].inv.slots[0] = Some(stack(HERMES_BOOTS, 1));
        got.clear();
        // Pick up boots from inventory slot 0
        click(&c, SlotRef::Inv(0), ClickKind::Left);
        s.tick();
        drain(&c, &mut got);
        assert_eq!(last_held(&got), Some(Some(stack(HERMES_BOOTS, 1))));
        got.clear();
        // Click accessory slot 0
        click(&c, SlotRef::Accessory(0), ClickKind::Left);
        s.tick();
        drain(&c, &mut got);
        assert_eq!(last_held(&got), Some(None));
        assert_eq!(
            s.conns[0].player.as_ref().unwrap().accessories[0],
            Some(HERMES_BOOTS)
        );
        assert!(has(&got, |m| matches!(
            m,
            ServerMsg::Accessories { pieces, .. } if pieces[0] == Some(HERMES_BOOTS)
        )));
    }

    #[test]
    fn close_inventory_returns_held_to_bag() {
        use terravibe_sim::item::WOOD;
        let (mut s, c, mut got, _) = chest_setup(1);
        s.conns[0].inv.slots[0] = Some(stack(WOOD, 10));
        click(&c, SlotRef::Inv(0), ClickKind::Left);
        s.tick();
        got.clear();
        c.send(ClientMsg::CloseInventory).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(last_held(&got), Some(None));
        assert_eq!(s.conns[0].held.0, None);
        assert_eq!(s.conns[0].inv.count_of(WOOD), 10);
        assert!(has(&got, |m| matches!(m, ServerMsg::Inventory(_))));
    }

    #[test]
    fn close_inventory_full_bag_drops_held_as_world_drop() {
        use terravibe_sim::item::{TORCH, WOOD};
        let (mut s, c, mut got, _) = chest_setup(1);
        s.conns[0].inv.slots[0] = Some(stack(TORCH, 5));
        click(&c, SlotRef::Inv(0), ClickKind::Left);
        s.tick();
        for i in 0..terravibe_sim::inventory::BAG {
            s.conns[0].inv.slots[i] = Some(stack(WOOD, 999));
        }
        let before = s.drops.len();
        got.clear();
        c.send(ClientMsg::CloseInventory).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(last_held(&got), Some(None));
        assert_eq!(s.drops.len(), before + 1);
        assert_eq!(s.drops.last().unwrap().stack, stack(TORCH, 5));
    }

    #[test]
    fn disconnect_returns_held_before_save() {
        use terravibe_sim::item::WOOD;
        let (mut s, c, _got, _) = chest_setup(1);
        s.conns[0].inv.slots[0] = Some(stack(WOOD, 10));
        click(&c, SlotRef::Inv(0), ClickKind::Left);
        s.tick();
        assert_eq!(s.conns[0].held.0, Some(stack(WOOD, 10)));
        c.send(ClientMsg::Bye).unwrap();
        s.tick();
        assert_eq!(s.connection_count(), 0);
        let saved = &s.saved_players["player"];
        assert_eq!(saved.inventory.count_of(WOOD), 10);
    }

    #[test]
    fn chest_slot_click_requires_open_chest() {
        use terravibe_sim::item::WOOD;
        let (mut s, c, mut got, pos) = chest_setup(1);
        s.chests.entry(pos).or_default().slots[0] = Some(stack(WOOD, 7));
        click(&c, SlotRef::Chest(0), ClickKind::Left);
        s.tick();
        assert_eq!(s.conns[0].held.0, None);
        assert_eq!(s.chests[&pos].slots[0], Some(stack(WOOD, 7)));
        c.send(ClientMsg::OpenChest { pos }).unwrap();
        click(&c, SlotRef::Chest(0), ClickKind::Left);
        got.clear();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(s.conns[0].held.0, Some(stack(WOOD, 7)));
        assert_eq!(s.chests[&pos].slots[0], None);
        assert_eq!(last_held(&got), Some(Some(stack(WOOD, 7))));
        assert!(has(
            &got,
            |m| matches!(m, ServerMsg::ChestContents { slots, .. } if slots[0].is_none())
        ));
    }

    #[test]
    fn door_toggle_is_server_validated_by_reach() {
        use terravibe_sim::{DOOR_CLOSED, DOOR_OPEN};
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        let y = spawn_point(1)[1] as i32;
        let (near, far) = ([2, y, 0], [30, y, 0]);
        set_block(&mut s, near, DOOR_CLOSED);
        set_block(&mut s, far, DOOR_CLOSED);
        got.clear();
        c.send(ClientMsg::UseTile { pos: far }).unwrap();
        s.tick();
        assert_eq!(WorldQ(&s.world).block(far[0], far[1], far[2]), DOOR_CLOSED);
        c.send(ClientMsg::UseTile { pos: near }).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(WorldQ(&s.world).block(near[0], near[1], near[2]), DOOR_OPEN);
        assert!(has(&got, |m| matches!(
            m,
            ServerMsg::BlockChanged { pos, block } if *pos == near && *block == DOOR_OPEN
        )));
        // Toggling back; the edit marks the chunk for saving like any block change.
        c.send(ClientMsg::UseTile { pos: near }).unwrap();
        s.tick();
        assert_eq!(
            WorldQ(&s.world).block(near[0], near[1], near[2]),
            DOOR_CLOSED
        );
        assert!(
            s.edited
                .contains(&ChunkPos::of_block(near[0], near[1], near[2]))
        );
    }

    /// Gives the player a bow and `n` arrows, and aims the input at +x.
    fn arm(s: &mut Server, ammo: ItemId, n: u32) {
        use terravibe_sim::inventory::ItemStack;
        use terravibe_sim::item::{Prefix, WOODEN_BOW};
        let inv = &mut s.conns[0].inv;
        inv.slots = [None; terravibe_sim::inventory::SLOTS];
        inv.slots[0] = Some(ItemStack {
            id: WOODEN_BOW,
            count: 1,
            prefix: Prefix(0),
        });
        inv.slots[1] = Some(ItemStack {
            id: ammo,
            count: n,
            prefix: Prefix(0),
        });
        inv.selected = 0;
    }

    /// Makes the shot deterministic. `pump` stops as soon as the spawn chunk arrives, so
    /// how many neighbouring chunks are loaded (an arrow dies on entering an unloaded
    /// one) and which mobs have spawned vary with wall-clock time. Wait for the lane's
    /// chunks, clear enemies and carve an air lane in front of the player.
    fn clear_arrow_lane(s: &mut Server) {
        let p = s.conns[0].player.as_ref().unwrap().body.pos;
        let (x0, y, z) = (
            p[0].floor() as i32,
            p[1].floor() as i32,
            p[2].floor() as i32,
        );
        let lane = |dx: i32, dy: i32| [x0 + dx, y + dy, z];
        // The arrow flies up to 40 blocks within its lifetime; cover the near field.
        let chunks: Vec<ChunkPos> = (-1..=CHUNK_SIZE * 2)
            .step_by(CHUNK_SIZE as usize / 2)
            .flat_map(|dx| (-1..=3).map(move |dy| (dx, dy)))
            .map(|(dx, dy)| {
                let [a, b, c] = lane(dx, dy);
                ChunkPos::of_block(a, b, c)
            })
            .collect();
        let t0 = Instant::now();
        while chunks.iter().any(|cp| !s.world.contains_key(cp))
            && t0.elapsed() < Duration::from_secs(2)
        {
            s.tick();
        }
        s.enemies.clear();
        for dx in -1..CHUNK_SIZE * 2 {
            for dy in 0..3 {
                let [a, b, c] = lane(dx, dy);
                if s.world.contains_key(&ChunkPos::of_block(a, b, c)) {
                    set_block(s, [a, b, c], AIR);
                }
            }
        }
    }

    fn fire_input() -> PlayerInput {
        PlayerInput {
            use_held: true,
            slot: 0,
            yaw: -std::f32::consts::FRAC_PI_2, // faces +x
            ..Default::default()
        }
    }

    #[test]
    fn arrow_shot_spawns_projectile_and_consumes_ammo() {
        use terravibe_sim::item::WOODEN_ARROW;
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        arm(&mut s, WOODEN_ARROW, 5);
        clear_arrow_lane(&mut s);
        got.clear();
        c.send(ClientMsg::Input {
            seq: 1,
            input: fire_input(),
        })
        .unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(s.projectiles.len(), 1);
        assert_eq!(s.conns[0].inv.slots[1].unwrap().count, 4);
        assert!(has(
            &got,
            |m| matches!(m, ServerMsg::Projectiles { list } if list.len() == 1)
        ));
    }

    #[test]
    fn arrow_message_only_sent_when_nonempty() {
        use terravibe_sim::item::WOODEN_ARROW;
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        got.clear();
        for _ in 0..5 {
            s.tick();
        }
        drain(&c, &mut got);
        assert!(!has(&got, |m| matches!(m, ServerMsg::Projectiles { .. })));
        // After the last arrow dies exactly one empty list clears the client.
        arm(&mut s, WOODEN_ARROW, 1);
        clear_arrow_lane(&mut s);
        c.send(ClientMsg::Input {
            seq: 1,
            input: fire_input(),
        })
        .unwrap();
        s.tick();
        s.projectiles[0].ttl = 1;
        drain(&c, &mut got);
        got.clear();
        s.tick();
        s.tick();
        drain(&c, &mut got);
        let lens: Vec<usize> = got
            .iter()
            .filter_map(|m| match m {
                ServerMsg::Projectiles { list } => Some(list.len()),
                _ => None,
            })
            .collect();
        assert_eq!(lens, vec![0]);
    }

    #[test]
    fn arrow_no_ammo_spawns_nothing_and_cap_holds() {
        use terravibe_sim::item::WOODEN_ARROW;
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        arm(&mut s, WOODEN_ARROW, 0);
        s.conns[0].inv.slots[1] = None;
        c.send(ClientMsg::Input {
            seq: 1,
            input: fire_input(),
        })
        .unwrap();
        s.tick();
        assert!(s.projectiles.is_empty());
        for _ in 0..MAX_PROJECTILES + 10 {
            let id = s.next_proj_id;
            s.next_proj_id += 1;
            if s.projectiles.len() < MAX_PROJECTILES {
                s.projectiles.push(
                    Projectile::fire(
                        id,
                        0,
                        terravibe_sim::item::WOODEN_BOW,
                        WOODEN_ARROW,
                        Prefix(0),
                        [0.0; 3],
                        [0.0, 1.0, 0.0],
                    )
                    .unwrap(),
                );
            }
        }
        assert_eq!(s.projectiles.len(), MAX_PROJECTILES);
    }

    #[test]
    fn wand_cast_spends_mana_and_spawns_spark() {
        use terravibe_sim::item::WAND_OF_SPARKING;
        use terravibe_sim::projectile::ProjKind;
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.cfg.natural_spawns = false;
        let inv = &mut s.conns[0].inv;
        inv.slots = [None; terravibe_sim::inventory::SLOTS];
        inv.slots[0] = Some(stack(WAND_OF_SPARKING, 1));
        inv.selected = 0;
        clear_arrow_lane(&mut s);
        got.clear();
        c.send(ClientMsg::Input {
            seq: 1,
            input: fire_input(),
        })
        .unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(s.projectiles.len(), 1);
        assert_eq!(s.projectiles[0].kind, ProjKind::Spark);
        assert_eq!(s.conns[0].player.as_ref().unwrap().vitals.mana, 18);
        assert!(has(&got, |m| matches!(
            m,
            ServerMsg::Projectiles { list } if list.len() == 1
        )));
    }

    #[test]
    fn wand_does_not_cast_without_mana() {
        use terravibe_sim::item::WAND_OF_SPARKING;
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.cfg.natural_spawns = false;
        let inv = &mut s.conns[0].inv;
        inv.slots = [None; terravibe_sim::inventory::SLOTS];
        inv.slots[0] = Some(stack(WAND_OF_SPARKING, 1));
        inv.selected = 0;
        s.conns[0].player.as_mut().unwrap().vitals.mana = 1;
        clear_arrow_lane(&mut s);
        got.clear();
        c.send(ClientMsg::Input {
            seq: 1,
            input: fire_input(),
        })
        .unwrap();
        s.tick();
        drain(&c, &mut got);
        assert!(s.projectiles.is_empty());
        assert_eq!(s.conns[0].player.as_ref().unwrap().vitals.mana, 1);
    }

    #[test]
    fn star_despawns_at_dawn() {
        use terravibe_sim::star::Star;
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.clock.tick = terravibe_sim::time::DAY_TICKS + 100;
        let p = s.conns[0].player.as_ref().unwrap().body.pos;
        s.stars.push(Star {
            id: 1,
            pos: [p[0], p[1] + 20.0, p[2]],
            vel: [1.0, -8.0, 0.0],
        });
        s.tick();
        assert!(!s.stars.is_empty());
        got.clear();
        s.clock.tick = 100;
        s.tick();
        drain(&c, &mut got);
        assert!(s.stars.is_empty());
        assert!(has(&got, |m| matches!(
            m,
            ServerMsg::Stars { list } if list.is_empty()
        )));
    }

    #[test]
    fn demon_eye_retreats_and_despawns_at_dawn() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.clock.tick = terravibe_sim::time::DAY_TICKS + 100;
        let p = s.conns[0].player.as_ref().unwrap().body.pos;
        s.add_enemy(EnemyKind::DemonEye, [p[0] + 6.0, p[1] + 2.0, p[2]]);
        s.tick();
        assert!(
            s.enemies.iter().any(|e| e.kind == EnemyKind::DemonEye),
            "DemonEye should stay alive at night"
        );
        // Transition to day (dawn)
        s.clock.tick = 100;
        for _ in 0..150 {
            s.tick();
        }
        assert!(
            !s.enemies.iter().any(|e| e.kind == EnemyKind::DemonEye),
            "DemonEye must retreat and despawn promptly at dawn"
        );
    }

    #[test]
    fn star_lands_and_becomes_fallen_star_drop() {
        use terravibe_sim::item::FALLEN_STAR;
        use terravibe_sim::star::Star;
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.clock.tick = terravibe_sim::time::DAY_TICKS + 100;
        let p = s.conns[0].player.as_ref().unwrap().body.pos;
        s.stars.push(Star {
            id: 1,
            pos: [p[0], p[1] + 2.0, p[2]],
            vel: [0.0, -8.0, 0.0],
        });
        s.tick();
        s.tick();
        assert!(
            s.drops.iter().any(|d| d.stack.id == FALLEN_STAR),
            "landed star drops item 75"
        );
    }

    #[test]
    fn open_out_of_reach_is_rejected() {
        let (mut s, c, mut got, pos) = chest_setup(25);
        got.clear();
        c.send(ClientMsg::OpenChest { pos }).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert!(has(&got, |m| matches!(m, ServerMsg::ChestClosed)));
        assert!(!has(&got, |m| matches!(m, ServerMsg::ChestContents { .. })));
        assert_eq!(s.conns[0].open_chest, None);
        // Not a chest either.
        c.send(ClientMsg::OpenChest {
            pos: [1, pos[1] - 1, 0],
        })
        .unwrap();
        s.tick();
        assert_eq!(s.conns[0].open_chest, None);
    }

    #[test]
    fn chest_open_move_and_close_roundtrip() {
        let (mut s, c, mut got, pos) = chest_setup(1);
        got.clear();
        s.conns[0].inv.slots[15] = Some(terravibe_sim::inventory::ItemStack {
            id: terravibe_sim::item::WOOD,
            count: 30,
            prefix: terravibe_sim::item::Prefix(0),
        });
        c.send(ClientMsg::OpenChest { pos }).unwrap();
        c.send(ClientMsg::ChestMove {
            to_chest: true,
            slot: 15,
        })
        .unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(s.conns[0].open_chest, Some(pos));
        assert_eq!(s.chests[&pos].slots[0].unwrap().count, 30);
        assert_eq!(s.conns[0].inv.slots[15], None);
        assert!(has(
            &got,
            |m| matches!(m, ServerMsg::ChestContents { slots, .. } if slots[0].is_some())
        ));
        c.send(ClientMsg::ChestMove {
            to_chest: false,
            slot: 0,
        })
        .unwrap();
        c.send(ClientMsg::CloseChest).unwrap();
        s.tick();
        assert!(s.chests[&pos].is_empty());
        assert_eq!(s.conns[0].inv.count_of(terravibe_sim::item::WOOD), 30);
        assert_eq!(s.conns[0].open_chest, None);
    }

    #[test]
    fn golden_key_unlocks_dungeon_chest() {
        let (mut s, c, mut got, pos) = chest_setup(1);
        got.clear();
        s.locked_chests.insert(pos);
        s.conns[0].inv.slots[10] = Some(terravibe_sim::inventory::ItemStack {
            id: terravibe_sim::item::GOLDEN_KEY,
            count: 1,
            prefix: terravibe_sim::item::Prefix(0),
        });

        c.send(ClientMsg::OpenChest { pos }).unwrap();
        s.tick();
        drain(&c, &mut got);

        assert_eq!(s.conns[0].open_chest, Some(pos));
        assert!(has(
            &got,
            |m| matches!(m, ServerMsg::ChestContents { pos: p, .. } if *p == pos)
        ));
        assert_eq!(s.conns[0].inv.count_of(terravibe_sim::item::GOLDEN_KEY), 0);
        assert!(!s.locked_chests.contains(&pos));
    }

    #[test]
    fn dungeon_chest_refuses_without_golden_key() {
        let (mut s, c, mut got, pos) = chest_setup(1);
        got.clear();
        s.locked_chests.insert(pos);
        assert_eq!(s.conns[0].inv.count_of(terravibe_sim::item::GOLDEN_KEY), 0);

        c.send(ClientMsg::OpenChest { pos }).unwrap();
        s.tick();
        drain(&c, &mut got);

        assert_eq!(s.conns[0].open_chest, None);
        assert!(has(&got, |m| matches!(m, ServerMsg::ChestClosed)));
        assert!(!has(&got, |m| matches!(m, ServerMsg::ChestContents { .. })));
        assert!(s.locked_chests.contains(&pos));
    }

    #[test]
    fn shadow_chest_requires_shadow_key_without_consuming_it() {
        let (mut s, c, mut got, _pos) = chest_setup(1);
        got.clear();

        // Find an underworld chunk with a house chest
        let seed = s.seed;
        let mut chest_pos = None;
        let mut chest_cp = None;
        for cx in 0..12 {
            for cz in 0..12 {
                let cp = ChunkPos { x: cx, y: 0, z: cz };
                let chests = terravibe_sim::worldgen::chest_gen::chests_in(seed, cp);
                for (p, _) in chests {
                    if terravibe_sim::worldgen::chest_gen::is_underworld_chest(
                        seed,
                        p,
                        s.bounds.size(),
                    ) {
                        chest_pos = Some(p);
                        chest_cp = Some(cp);
                        break;
                    }
                }
                if chest_pos.is_some() {
                    break;
                }
            }
            if chest_pos.is_some() {
                break;
            }
        }
        let chest_pos = chest_pos.expect("must find an underworld house chest");
        let chest_cp = chest_cp.unwrap();

        // Install the underworld chest chunk
        let chunk = terravibe_sim::worldgen::generate_chunk_sized(seed, chest_cp, s.bounds.size());
        s.install_chunk(chest_cp, chunk.into());

        assert!(
            s.shadow_chests.contains(&chest_pos),
            "shadow_chests must contain installed underworld house chest at {chest_pos:?}"
        );

        // Move player next to the chest so it is in reach
        if let Some(conn) = s.conns.first_mut() {
            if let Some(player) = conn.player.as_mut() {
                let p = [
                    chest_pos[0] as f32 + 0.5,
                    chest_pos[1] as f32,
                    chest_pos[2] as f32 + 0.5,
                ];
                player.body.pos = p;
                player.body.vel = [0.0; 3];
                player.body.fall_peak_y = p[1];
                player.body.on_ground = true;
                conn.interest.centre = Some(p);
            }
        }

        // 1. Try opening without Shadow Key (player has no shadow key)
        assert_eq!(s.conns[0].inv.count_of(terravibe_sim::item::SHADOW_KEY), 0);
        c.send(ClientMsg::OpenChest { pos: chest_pos }).unwrap();
        s.tick();
        drain(&c, &mut got);

        assert_eq!(s.conns[0].open_chest, None);
        assert!(has(&got, |m| matches!(m, ServerMsg::ChestClosed)));
        assert!(!has(&got, |m| matches!(m, ServerMsg::ChestContents { .. })));

        // 2. Try with Golden Key - must NOT open shadow chest and key must not be consumed
        s.conns[0].inv.slots[10] = Some(terravibe_sim::inventory::ItemStack {
            id: terravibe_sim::item::GOLDEN_KEY,
            count: 1,
            prefix: terravibe_sim::item::Prefix(0),
        });
        got.clear();
        c.send(ClientMsg::OpenChest { pos: chest_pos }).unwrap();
        s.tick();
        drain(&c, &mut got);

        assert_eq!(s.conns[0].open_chest, None);
        assert_eq!(s.conns[0].inv.count_of(terravibe_sim::item::GOLDEN_KEY), 1);
        assert!(has(&got, |m| matches!(m, ServerMsg::ChestClosed)));

        // 3. Give SHADOW_KEY to player
        s.conns[0].inv.slots[11] = Some(terravibe_sim::inventory::ItemStack {
            id: terravibe_sim::item::SHADOW_KEY,
            count: 1,
            prefix: terravibe_sim::item::Prefix(0),
        });
        got.clear();
        c.send(ClientMsg::OpenChest { pos: chest_pos }).unwrap();
        s.tick();
        drain(&c, &mut got);

        assert_eq!(s.conns[0].open_chest, Some(chest_pos));
        assert!(has(
            &got,
            |m| matches!(m, ServerMsg::ChestContents { pos: p, .. } if *p == chest_pos)
        ));
        assert_eq!(
            s.conns[0].inv.count_of(terravibe_sim::item::SHADOW_KEY),
            1,
            "shadow key count must remain unchanged"
        );
    }

    #[test]
    fn second_player_cannot_open_same_chest() {
        let (mut s, c, mut got, pos) = chest_setup(1);
        let (c2, conn2) = in_process_pair();
        s.connect(Box::new(conn2));
        c2.send(ClientMsg::Hello {
            seed: 1,
            name: Some("b".into()),
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();
        c.send(ClientMsg::OpenChest { pos }).unwrap();
        s.tick();
        c2.send(ClientMsg::OpenChest { pos }).unwrap();
        s.tick();
        assert_eq!(s.conns[0].open_chest, Some(pos));
        assert_eq!(s.conns[1].open_chest, None);
        let mut got2 = Vec::new();
        drain(&c2, &mut got2);
        assert!(has(&got2, |m| matches!(m, ServerMsg::ChestClosed)));
        // Once the first player closes, the second may open it.
        c.send(ClientMsg::CloseChest).unwrap();
        s.tick();
        c2.send(ClientMsg::OpenChest { pos }).unwrap();
        s.tick();
        assert_eq!(s.conns[1].open_chest, Some(pos));
        drain(&c, &mut got);
    }

    #[test]
    fn cannot_break_nonempty_chest() {
        let cp = ChunkPos { x: 0, y: 0, z: 0 };
        let mut world = HashMap::new();
        world.insert(cp, Arc::new(Chunk::filled(AIR)));
        let mut chests = HashMap::new();
        let mut log = Vec::new();
        let mut q = EditQ {
            world: &mut world,
            log: &mut log,
            chests: &mut chests,
            wall_log: &mut Vec::new(),
            liquid_log: &mut Vec::new(),
        };
        let p = [3, 4, 5];
        assert!(q.try_set_block(3, 4, 5, CHEST));
        assert!(
            q.chests.get(&p).unwrap().is_empty(),
            "placing creates an empty entry"
        );
        q.chests.get_mut(&p).unwrap().slots[2] = Some(terravibe_sim::inventory::ItemStack {
            id: terravibe_sim::item::WOOD,
            count: 1,
            prefix: terravibe_sim::item::Prefix(0),
        });
        assert!(!q.try_set_block(3, 4, 5, AIR), "non-empty chest refuses");
        assert_eq!(WorldQ(q.world).block(3, 4, 5), CHEST);
        q.chests.get_mut(&p).unwrap().slots[2] = None;
        assert!(q.try_set_block(3, 4, 5, AIR));
        assert!(q.chests.is_empty(), "breaking removes the entry");
    }

    #[test]
    fn chest_contents_survive_save_roundtrip() {
        let (cfg, root) = saving_cfg("chest");
        let mut a = Server::with_cfg(1, cfg);
        let mut inv = ChestInv::empty();
        inv.slots[7] = Some(terravibe_sim::inventory::ItemStack {
            id: terravibe_sim::item::WOOD,
            count: 42,
            prefix: terravibe_sim::item::Prefix(1),
        });
        a.chests.insert([5, -6, 7], inv.clone());
        a.save_now().unwrap();
        drop(a);
        let (mut cfg2, _) = saving_cfg("chest_unused");
        cfg2.save = Some(SaveCfg {
            root: root.clone(),
            world: "w".into(),
        });
        let b = Server::with_cfg(1, cfg2);
        assert_eq!(b.chests[&[5, -6, 7]], inv);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn generated_chest_seeds_server_once() {
        use terravibe_sim::worldgen::chest_gen::chests_in;
        let (mut s, _c) = setup();
        let (pos, (cpos, loot)) = (0..8)
            .flat_map(|x| (0..8).flat_map(move |z| (0..4).map(move |y| ChunkPos { x, y, z })))
            .find_map(|p| {
                chests_in(1, p)
                    .into_iter()
                    .find(|(_, l)| !l.is_empty())
                    .map(|c| (p, c))
            })
            .expect("a generated chest with loot");
        let chunk = Arc::new(generate_chunk(1, pos));
        s.install_chunk(pos, Arc::clone(&chunk));
        assert_eq!(s.chests[&cpos], ChestInv::from_stacks(&loot));
        // The player takes everything; regenerating the chunk must not refill it.
        s.chests.get_mut(&cpos).unwrap().slots = ChestInv::empty().slots;
        s.world.remove(&pos);
        s.install_chunk(pos, chunk);
        assert!(s.chests[&cpos].is_empty());
        assert_eq!(
            s.world[&pos].get(0, 0, 0),
            generate_chunk(1, pos).get(0, 0, 0)
        );
    }

    // --- Eye of Cthulhu -------------------------------------------------

    use terravibe_sim::time::DAY_TICKS;

    fn bosses(s: &Server) -> usize {
        s.enemies
            .iter()
            .filter(|e| e.kind == EnemyKind::EyeOfCthulhu)
            .count()
    }

    /// Selected slot 0 holds `n` Suspicious Looking Eyes and the input uses it.
    fn hold_eye(s: &mut Server, c: &InProcClient, n: u32) {
        use terravibe_sim::inventory::ItemStack;
        use terravibe_sim::item::Prefix;
        let inv = &mut s.conns[0].inv;
        inv.slots = [None; terravibe_sim::inventory::SLOTS];
        inv.slots[0] = Some(ItemStack {
            id: SUSPICIOUS_LOOKING_EYE,
            count: n,
            prefix: Prefix(0),
        });
        inv.selected = 0;
        c.send(ClientMsg::Input {
            seq: 1,
            input: PlayerInput {
                use_held: true,
                slot: 0,
                ..Default::default()
            },
        })
        .unwrap();
    }

    fn eye_count(s: &Server) -> u32 {
        s.conns[0].inv.slots[0].map_or(0, |x| x.count)
    }

    fn set_night(s: &mut Server) {
        s.clock.tick = DAY_TICKS + 100;
    }

    #[test]
    fn summon_refused_in_daytime() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.clock.tick = 100;
        hold_eye(&mut s, &c, 2);
        s.tick();
        assert_eq!(bosses(&s), 0);
        assert_eq!(eye_count(&s), 2, "the item is kept");
    }

    #[test]
    fn summon_consumes_eye_item_and_spawns_boss() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        set_night(&mut s);
        hold_eye(&mut s, &c, 2);
        got.clear();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(bosses(&s), 1);
        assert_eq!(eye_count(&s), 1);
        let p = s.conns[0].player.as_ref().unwrap().body.pos;
        let e = s
            .enemies
            .iter()
            .find(|e| e.kind == EnemyKind::EyeOfCthulhu)
            .unwrap();
        assert!(e.pos[1] > p[1] + 30.0, "spawns high above the player");
        assert!(has(&got, |m| matches!(
            m,
            ServerMsg::Boss {
                life_max: 2800,
                phase: 1,
                ..
            }
        )));
    }

    #[test]
    fn only_one_boss_at_a_time() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        set_night(&mut s);
        hold_eye(&mut s, &c, 3);
        for _ in 0..80 {
            s.tick(); // the use animation repeats while the button is held
        }
        assert_eq!(bosses(&s), 1);
        assert_eq!(eye_count(&s), 2, "only the first use was spent");
    }

    #[test]
    fn natural_spawn_requires_life_max_and_roll() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        let rolls = |s: &mut Server, life_max: i32| {
            s.conns[0].player.as_mut().unwrap().vitals.life_max = life_max;
            (0..60)
                .filter(|_| {
                    s.was_day = true;
                    s.clock.tick = DAY_TICKS + 1;
                    s.eye_pending = false;
                    s.tick_boss();
                    s.eye_pending
                })
                .count()
        };
        // Fresh world (Night 1: 100 HP, 0 defense, <=1 town NPC) cannot spawn.
        assert_eq!(rolls(&mut s, 100), 0, "fresh world cannot spawn eye");
        // 200 HP alone with <10 defense and <3 town NPCs cannot spawn.
        assert_eq!(rolls(&mut s, 200), 0, "needs defense >= 10 and 3 town NPCs");

        // Give >= 10 defense, still < 3 town NPCs -> cannot spawn.
        s.conns[0].player.as_mut().unwrap().armor = [
            Some(terravibe_sim::item::MOLTEN_HELMET),
            Some(terravibe_sim::item::MOLTEN_BREASTPLATE),
            None,
        ];
        assert!(s.conns[0].player.as_ref().unwrap().defense() >= boss::MIN_DEFENSE);
        assert_eq!(rolls(&mut s, 200), 0, "needs 3 living town NPCs");

        // Ensure at least 3 living town NPCs.
        s.enemies.retain(|e| {
            !matches!(
                e.kind,
                EnemyKind::Guide | EnemyKind::Merchant | EnemyKind::ArmsDealer
            )
        });
        s.add_enemy(EnemyKind::Guide, [0.0, 40.0, 0.0]);
        s.add_enemy(EnemyKind::Merchant, [2.0, 40.0, 0.0]);
        s.add_enemy(EnemyKind::ArmsDealer, [4.0, 40.0, 0.0]);
        assert_eq!(rolls(&mut s, 100), 0, "needs life_max >= 200");
        let n = rolls(&mut s, 200);
        assert!((5..40).contains(&n), "about 1 in 3, got {n}");

        // Drop defense below 10 -> cannot spawn again.
        s.conns[0].player.as_mut().unwrap().armor = [None, None, None];
        assert_eq!(rolls(&mut s, 200), 0, "needs defense >= 10");

        // With a roll pending the Eye comes once night passes tick 4860.
        s.eye_pending = true;
        s.was_day = false;
        s.clock.tick = DAY_TICKS + 4000;
        s.tick_boss();
        assert_eq!(bosses(&s), 0, "too early");
        let t0 = Instant::now();
        while bosses(&s) == 0 && t0.elapsed() < Duration::from_secs(2) {
            s.clock.tick = DAY_TICKS + 5000;
            s.tick();
        }
        assert_eq!(bosses(&s), 1);
        assert!(!s.eye_pending);
    }

    #[test]
    fn eye_contact_damages_player_through_defense() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        set_night(&mut s);
        let p = s.conns[0].player.as_ref().unwrap();
        let (pos, def) = (p.body.pos, p.defense());
        s.spawn_boss(pos);
        got.clear();
        s.tick();
        drain(&c, &mut got);
        let want = damage_taken(15, def);
        assert!(has(
            &got,
            |m| matches!(m, ServerMsg::PlayerHit { amount } if *amount == want)
        ));
    }

    #[test]
    fn boss_death_drops_loot_and_clears_state() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        set_night(&mut s);
        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        s.spawn_boss([pos[0] + 10.0, pos[1] + 10.0, pos[2]]);
        s.enemies.iter_mut().for_each(|e| e.hp = 0);
        got.clear();
        s.tick();
        drain(&c, &mut got);
        assert!(s.boss.is_none() && bosses(&s) == 0);
        assert!(
            s.drops
                .iter()
                .any(|d| d.stack.id == terravibe_sim::item::LESSER_HEALING_POTION)
        );
        assert!(s.drops.iter().filter(|d| d.stack.id == HEART).count() >= 5);
        assert!(
            s.drops
                .iter()
                .any(|d| d.stack.id == terravibe_sim::item::SILVER_COIN
                    || d.stack.id == terravibe_sim::item::GOLD_COIN)
        );
        assert!(has(&got, |m| matches!(m, ServerMsg::BossGone)));
    }

    #[test]
    fn boss_message_only_while_alive() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        set_night(&mut s);
        got.clear();
        s.clock.tick = DAY_TICKS + 100;
        s.tick();
        drain(&c, &mut got);
        assert!(!has(&got, |m| matches!(
            m,
            ServerMsg::Boss { .. } | ServerMsg::BossGone
        )));
        let pos = s.conns[0].player.as_ref().unwrap().body.pos;
        s.spawn_boss([pos[0] + 10.0, pos[1] + 20.0, pos[2]]);
        s.tick();
        drain(&c, &mut got);
        assert!(has(&got, |m| matches!(m, ServerMsg::Boss { .. })));
        s.enemies.iter_mut().for_each(|e| e.hp = 0);
        s.tick();
        drain(&c, &mut got);
        assert!(has(&got, |m| matches!(m, ServerMsg::BossGone)));
        got.clear();
        s.tick();
        s.tick();
        drain(&c, &mut got);
        assert!(!has(&got, |m| matches!(
            m,
            ServerMsg::Boss { .. } | ServerMsg::BossGone
        )));
    }

    /// A second server on the same save root, with a fresh client connected.
    fn restart(root: &std::path::Path) -> (Server, InProcClient) {
        let cfg = ServerCfg {
            natural_spawns: false,
            save: Some(SaveCfg {
                root: root.to_path_buf(),
                world: "w".into(),
            }),
            ..ServerCfg::default()
        };
        let (client, conn) = in_process_pair();
        let mut s = Server::with_cfg(1, cfg);
        s.connect(Box::new(conn));
        (s, client)
    }

    #[test]
    fn chest_survives_restart_end_to_end() {
        let (cfg, root) = saving_cfg("e2e_chest");
        let (client, conn) = in_process_pair();
        let mut a = Server::with_cfg(1, cfg);
        a.connect(Box::new(conn));
        let mut got = Vec::new();
        hello_and_load(&mut a, &client, &mut got);
        let pos = [1, spawn_point(1)[1] as i32, 0];
        set_block(&mut a, pos, CHEST);
        let cp = ChunkPos::of_block(pos[0], pos[1], pos[2]);
        a.edited.insert(cp);
        a.dirty.insert(cp);
        a.conns[0].inv.slots[15] = Some(terravibe_sim::inventory::ItemStack {
            id: terravibe_sim::item::WOOD,
            count: 12,
            prefix: terravibe_sim::item::Prefix(0),
        });
        client.send(ClientMsg::OpenChest { pos }).unwrap();
        client
            .send(ClientMsg::ChestMove {
                to_chest: true,
                slot: 15,
            })
            .unwrap();
        client.send(ClientMsg::CloseChest).unwrap();
        a.tick();
        a.save_now().unwrap();
        drop(client);
        drop(a);
        // Same world name: the chest block, its contents and the player come back.
        let (mut b, c2) = restart(&root);
        let mut got2 = Vec::new();
        hello_and_load(&mut b, &c2, &mut got2);
        assert_eq!(WorldQ(&b.world).block(pos[0], pos[1], pos[2]), CHEST);
        c2.send(ClientMsg::OpenChest { pos }).unwrap();
        b.tick();
        drain(&c2, &mut got2);
        assert_eq!(b.chests[&pos].slots[0].unwrap().count, 12);
        assert!(has(&got2, |m| matches!(
            m,
            ServerMsg::ChestContents { slots, .. } if slots[0].is_some_and(|s| s.count == 12)
        )));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn boss_is_not_persisted() {
        let (cfg, root) = saving_cfg("boss_np");
        let mut a = Server::with_cfg(1, cfg);
        assert!(a.spawn_boss([0.0, 120.0, 0.0]));
        a.save_now().unwrap();
        drop(a);
        let (b, _c) = restart(&root);
        assert!(b.boss.is_none());
        assert_eq!(bosses(&b), 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn autosave_fires_at_interval() {
        let (cfg, root) = saving_cfg("interval");
        let mut a = Server::with_cfg(1, cfg);
        a.chests.insert([3, 4, 5], ChestInv::empty());
        // Server time is the tick count: nothing is written before 60 s.
        for _ in 0..AUTOSAVE_TICKS - 1 {
            a.tick();
        }
        assert!(!root.join("w/world.bin").exists());
        a.tick();
        drop(a); // joins the worker
        assert!(root.join("w/world.bin").exists());
        assert!(root.join("w/chests.bin").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn projectile_and_boss_counts_are_capped() {
        use terravibe_sim::item::{WOODEN_ARROW, WOODEN_BOW};
        let (mut s, _c) = setup();
        assert!(s.spawn_boss([0.0, 120.0, 0.0]));
        assert!(!s.spawn_boss([5.0, 120.0, 0.0]), "one boss at a time");
        for _ in 0..MAX_PROJECTILES * 2 {
            let id = s.next_proj_id;
            s.next_proj_id += 1;
            if s.projectiles.len() < MAX_PROJECTILES {
                s.projectiles.push(
                    Projectile::fire(
                        id,
                        0,
                        WOODEN_BOW,
                        WOODEN_ARROW,
                        Prefix(0),
                        [0.0; 3],
                        [0.0, 1.0, 0.0],
                    )
                    .unwrap(),
                );
            }
        }
        for _ in 0..30 {
            s.tick();
        }
        assert!(s.projectiles.len() <= MAX_PROJECTILES);
        assert!(bosses(&s) <= 1);
    }
    fn set_wall_at(s: &mut Server, p: [i32; 3], on: bool) {
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        let cp = ChunkPos::of_block(p[0], p[1], p[2]);
        Arc::make_mut(s.world.get_mut(&cp).unwrap()).set_wall(l(p[0]), l(p[1]), l(p[2]), on);
    }

    #[test]
    fn x_breaks_wall_in_reach_and_drops_item() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        let y = spawn_point(1)[1] as i32;
        let (near, far) = ([2, y, 0], [30, y, 0]);
        set_wall_at(&mut s, near, true);
        set_wall_at(&mut s, far, true);
        got.clear();
        // Without a hammer selected, BreakWall is refused.
        c.send(ClientMsg::BreakWall { pos: near }).unwrap();
        s.tick();
        assert!(WorldQ(&s.world).has_wall(near[0], near[1], near[2]));

        give(&mut s, terravibe_sim::item::WOODEN_HAMMER, 1);
        let hammer_slot = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|st| st.is_some_and(|x| x.id == terravibe_sim::item::WOODEN_HAMMER))
            .unwrap();
        s.conns[0].inv.selected = hammer_slot;

        c.send(ClientMsg::BreakWall { pos: far }).unwrap();
        s.tick();
        assert!(WorldQ(&s.world).has_wall(far[0], far[1], far[2]));
        c.send(ClientMsg::BreakWall { pos: near }).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert!(!WorldQ(&s.world).has_wall(near[0], near[1], near[2]));
        assert!(has(&got, |m| matches!(
            m,
            ServerMsg::WallChanged { pos, on: false } if *pos == near
        )));
        assert_eq!(s.drops.len(), 1);
    }

    #[test]
    fn h_query_reports_housing_validity() {
        use terravibe_sim::{CHAIR, DOOR_CLOSED, STONE, TABLE, TORCH};
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        let o = [2, spawn_point(1)[1] as i32 + 1, 1];
        for x in -1..=5 {
            for y in -1..=4 {
                for z in -1..=4 {
                    let p = [o[0] + x, o[1] + y, o[2] + z];
                    let inside = (0..5).contains(&x) && (0..4).contains(&y) && (0..4).contains(&z);
                    if inside {
                        set_block(&mut s, p, AIR);
                        set_wall_at(&mut s, p, true);
                    } else {
                        set_block(&mut s, p, STONE);
                    }
                }
            }
        }
        set_block(&mut s, [o[0] - 1, o[1], o[2]], DOOR_CLOSED);
        let q = [o[0] + 1, o[1] + 1, o[2] + 1];
        // Unfurnished: valid shell but needs furniture (reason 8).
        got.clear();
        c.send(ClientMsg::QueryHouse { pos: q }).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert!(
            has(&got, |m| matches!(
                m,
                ServerMsg::HouseStatus {
                    ok: false,
                    reason: 8
                }
            )),
            "{:?}",
            got.iter()
                .filter(|m| matches!(m, ServerMsg::HouseStatus { .. }))
                .collect::<Vec<_>>()
        );
        set_block(&mut s, [o[0] + 1, o[1], o[2]], CHAIR);
        set_block(&mut s, [o[0] + 2, o[1], o[2]], TABLE);
        set_block(&mut s, [o[0] + 3, o[1], o[2]], TORCH);
        got.clear();
        c.send(ClientMsg::QueryHouse { pos: q }).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert!(has(&got, |m| matches!(
            m,
            ServerMsg::HouseStatus {
                ok: true,
                reason: 0
            }
        )));
        // Out of reach: no answer.
        got.clear();
        c.send(ClientMsg::QueryHouse { pos: [60, o[1], 0] })
            .unwrap();
        s.tick();
        drain(&c, &mut got);
        assert!(!has(&got, |m| matches!(m, ServerMsg::HouseStatus { .. })));
    }

    #[test]
    fn fresh_world_liquid_settles_to_zero_pending_within_600_ticks() {
        let (client, conn) = in_process_pair();
        let mut s = Server::with_cfg(
            7,
            ServerCfg {
                size: WorldSize::Small,
                natural_spawns: false,
                ..ServerCfg::default()
            },
        );
        s.connect(Box::new(conn));
        let mut got = Vec::new();
        client
            .send(ClientMsg::Hello {
                seed: 7,
                name: None,
                character: None,
                protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
                password: None,
            })
            .unwrap();
        let sp = spawn_point(7);
        let want = ChunkPos::of_block(0, sp[1] as i32 - 1, 0);
        assert!(pump(&mut s, &client, &mut got, |g| has_chunk(g, want)));
        // Stream a chunk column outside the 48-block spawn liquid guard so ponds/pools are present.
        let away = ChunkPos::of_block(96, sp[1] as i32 - 32, 96);
        let away_chunk = terravibe_sim::worldgen::generate_chunk_sized(7, away, s.bounds.size());
        s.install_chunk(away, Arc::new(away_chunk));
        // Also ensure an Underworld chunk with lava sea is present so liquid settlement covers it.
        let uw = ChunkPos { x: 0, y: 0, z: 0 };
        let uw_chunk = terravibe_sim::worldgen::generate_chunk_sized(7, uw, s.bounds.size());
        s.install_chunk(uw, Arc::new(uw_chunk));

        let mut peak_pending = 0usize;
        for _ in 0..600 {
            s.tick();
            peak_pending = peak_pending.max(s.liquid_pending());
            drain(&client, &mut got);
        }
        assert!(
            s.world.values().any(|c| c.has_liquid()),
            "expected generated liquid in the streamed chunks"
        );
        assert_eq!(
            s.liquid_pending(),
            0,
            "generated liquid must be settled by construction (peak pending {peak_pending})"
        );
    }

    #[test]
    fn server_tick_with_liquid_flood_p99_under_8ms() {
        let (client, conn) = in_process_pair();
        let mut s = Server::with_cfg(
            1,
            ServerCfg {
                natural_spawns: false,
                ..ServerCfg::default()
            },
        );
        s.connect(Box::new(conn));
        let mut got = Vec::new();
        hello_and_load(&mut s, &client, &mut got);

        // Wait for initial chunk generation around spawn to settle before measuring flood ticks
        for _ in 0..30 {
            s.tick();
            while let Ok(Some(_)) = client.try_recv() {}
        }

        let sp = spawn_point(1);
        let base = [
            sp[0].floor() as i32 + 4,
            sp[1].floor() as i32,
            sp[2].floor() as i32,
        ];
        s.seed_liquid_flood(base, 20_000);
        let initial_pending = s.liquid_pending();
        assert!(
            initial_pending >= 20_000,
            "expected >= 20,000 pending cells, got {initial_pending}"
        );

        let mut tick_times_ms = Vec::new();
        let mut pending_samples = vec![initial_pending];
        let mut seq = 0u32;

        // Run 120 ticks so p99 is computed over >= 100 samples
        for step_idx in 1..=120 {
            seq += 1;
            client
                .send(ClientMsg::Input {
                    seq,
                    input: PlayerInput::default(),
                })
                .unwrap();
            let t0 = Instant::now();
            s.tick();
            tick_times_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
            while let Ok(Some(_)) = client.try_recv() {}

            if step_idx <= 15 || step_idx % 20 == 0 || s.liquid_pending() == 0 {
                pending_samples.push(s.liquid_pending());
            }
        }

        let mean = tick_times_ms.iter().sum::<f64>() / tick_times_ms.len() as f64;
        let mut sorted = tick_times_ms.clone();
        sorted.sort_by(f64::total_cmp);
        let p50 = sorted[sorted.len() / 2];
        let p99_idx = ((0.99 * sorted.len() as f64).ceil() as usize).saturating_sub(1);
        let p99 = sorted[p99_idx];
        let max = *sorted.last().unwrap();

        println!(
            "PERF server_tick_with_liquid_flood_p99_under_8ms: ticks={} mean={mean:.3}ms p50={p50:.3}ms p99={p99:.3}ms max={max:.3}ms end_pending={} samples={pending_samples:?}",
            tick_times_ms.len(),
            s.liquid_pending()
        );

        assert_eq!(
            s.liquid_pending(),
            0,
            "pending queue should drain to 0 within 600 ticks (samples: {pending_samples:?})"
        );
        // Check that pending queue drains monotonically across the initial deferred flood windows
        for w in pending_samples.windows(2) {
            assert!(
                w[1] <= w[0],
                "pending queue did not drain monotonically across 20-tick windows: {pending_samples:?}"
            );
        }
        #[cfg(not(debug_assertions))]
        {
            assert!(
                p99 <= 8.0,
                "server tick p99 {p99:.3} ms exceeded 8.0 ms gate"
            );
            assert!(
                max <= 16.0,
                "server tick max {max:.3} ms exceeded 16.0 ms gate"
            );
        }
    }

    #[test]
    fn mining_plug_releases_pond_and_streams_delta() {
        let (mut s, c) = setup();
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();
        let sp = spawn_point(1);
        let cp = ChunkPos::of_block(sp[0] as i32, sp[1] as i32, sp[2] as i32);
        let mut got = Vec::new();
        assert!(pump(&mut s, &c, &mut got, |g| has_chunk(g, cp)));
        got.clear();

        let bx = cp.x * 32 + 5;
        let by = cp.y * 32 + 5;
        let bz = cp.z * 32 + 5;

        // Basin: floor at by, walls around by+1, water at [bx, by+1, bz], plug at [bx+1, by+1, bz]
        s.set_block([bx, by, bz], terravibe_sim::STONE);
        s.set_block([bx + 1, by, bz], terravibe_sim::STONE);
        s.set_block([bx + 2, by, bz], terravibe_sim::STONE);
        s.set_block([bx - 1, by + 1, bz], terravibe_sim::STONE);
        s.set_block([bx, by + 1, bz - 1], terravibe_sim::STONE);
        s.set_block([bx, by + 1, bz + 1], terravibe_sim::STONE);
        s.set_block([bx, by + 2, bz], terravibe_sim::STONE);
        s.set_block([bx + 1, by + 1, bz], terravibe_sim::STONE); // Plug!
        s.set_block([bx + 2, by + 1, bz], AIR); // Empty chamber outside plug

        {
            let mut adapter = ServerLiquidWorld::new(&mut s.world);
            adapter.set_liquid(bx, by + 1, bz, Some(Liquid::water(255)));
        }
        for _ in 0..10 {
            s.tick();
        }
        while let Ok(Some(_)) = c.try_recv() {}

        // Mine the plug
        s.set_block([bx + 1, by + 1, bz], AIR);

        assert!(pump(&mut s, &c, &mut got, |g| {
            g.iter()
                .any(|m| matches!(m, ServerMsg::LiquidDelta { pos, .. } if *pos == cp))
        }));
    }

    #[test]
    fn liquid_delta_reaches_only_interested_clients() {
        let (mut s, c1) = setup();
        let (c2, conn2) = in_process_pair();
        s.connect(Box::new(conn2));

        c1.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        c2.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();

        let sp = spawn_point(1);
        let cp = ChunkPos::of_block(sp[0] as i32, sp[1] as i32, sp[2] as i32);
        let mut got1 = Vec::new();
        let mut got2 = Vec::new();
        assert!(pump(&mut s, &c1, &mut got1, |g| has_chunk(g, cp)));
        assert!(pump(&mut s, &c2, &mut got2, |g| has_chunk(g, cp)));

        // Move client 2 far away so cp is unstreamed/unloaded for client 2
        let far_x = sp[0] + 1000.0;
        let far_pos = [far_x, sp[1], sp[2]];
        c2.send(ClientMsg::Spectate { pos: far_pos }).unwrap();
        for _ in 0..20 {
            s.tick();
        }
        while let Ok(Some(_)) = c1.try_recv() {}
        while let Ok(Some(_)) = c2.try_recv() {}
        assert!(!s.conns[1].interest.sent.contains(&cp));

        // Create liquid delta in cp (clear neighbours so water flows horizontally)
        let bx = cp.x * 32 + 10;
        let by = cp.y * 32 + 10;
        let bz = cp.z * 32 + 10;
        s.set_block([bx, by, bz], terravibe_sim::STONE);
        s.set_block([bx + 1, by, bz], terravibe_sim::STONE);
        s.set_block([bx, by + 1, bz], AIR);
        s.set_block([bx + 1, by + 1, bz], AIR);
        {
            let mut adapter = ServerLiquidWorld::new(&mut s.world);
            adapter.set_liquid(bx, by + 1, bz, Some(Liquid::water(255)));
        }
        s.liquid_sim.wake([bx, by + 1, bz]);
        s.tick();

        let mut msgs1 = Vec::new();
        while let Ok(Some(m)) = c1.try_recv() {
            msgs1.push(m);
        }
        let mut msgs2 = Vec::new();
        while let Ok(Some(m)) = c2.try_recv() {
            msgs2.push(m);
        }

        assert!(
            msgs1
                .iter()
                .any(|m| matches!(m, ServerMsg::LiquidDelta { pos, .. } if *pos == cp))
        );
        assert!(
            !msgs2
                .iter()
                .any(|m| matches!(m, ServerMsg::LiquidDelta { pos, .. } if *pos == cp))
        );
    }

    #[test]
    fn large_delta_falls_back_to_full_chunk() {
        let (mut s, c) = setup();
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();
        let sp = spawn_point(1);
        let cp = ChunkPos::of_block(sp[0] as i32, sp[1] as i32, sp[2] as i32);
        let mut got = Vec::new();
        assert!(pump(&mut s, &c, &mut got, |g| has_chunk(g, cp)));
        got.clear();

        // Fill > LIQUID_DELTA_MAX cells (520 cells) in chunk cp and wake them
        let bx = cp.x * 32;
        let by = cp.y * 32 + 5;
        let bz = cp.z * 32;
        {
            let mut adapter = ServerLiquidWorld::new(&mut s.world);
            let mut count = 0;
            for lx in 0..26 {
                for lz in 0..21 {
                    adapter.set_block(bx + lx, by, bz + lz, AIR);
                    adapter.set_liquid(bx + lx, by, bz + lz, Some(Liquid::water(255)));
                    s.liquid_sim.wake([bx + lx, by, bz + lz]);
                    count += 1;
                    if count >= 520 {
                        break;
                    }
                }
                if count >= 520 {
                    break;
                }
            }
        }
        s.tick();

        while let Ok(Some(m)) = c.try_recv() {
            got.push(m);
        }

        // Must receive full ServerMsg::Chunk for cp, not LiquidDelta
        assert!(
            got.iter()
                .any(|m| matches!(m, ServerMsg::Chunk { pos, .. } if *pos == cp))
        );
        assert!(
            !got.iter()
                .any(|m| matches!(m, ServerMsg::LiquidDelta { pos, .. } if *pos == cp))
        );
    }

    #[test]
    fn liquid_chunk_survives_save_and_reload_settled() {
        let (cfg, root) = saving_cfg("save_liquid");
        let mut s = Server::with_cfg(1, cfg);
        let (client, conn) = in_process_pair();
        s.connect(Box::new(conn));
        client
            .send(ClientMsg::Hello {
                seed: 1,
                name: None,
                character: None,
                protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
                password: None,
            })
            .unwrap();
        s.tick();

        let sp = spawn_point(1);
        let cp = ChunkPos::of_block(sp[0] as i32, sp[1] as i32, sp[2] as i32);
        let mut got = Vec::new();
        assert!(pump(&mut s, &client, &mut got, |g| has_chunk(g, cp)));

        let bx = cp.x * 32 + 2;
        let by = cp.y * 32 + 2;
        let bz = cp.z * 32 + 2;

        s.set_block([bx, by, bz], terravibe_sim::STONE);
        s.set_block([bx + 1, by + 1, bz], terravibe_sim::STONE);
        s.set_block([bx - 1, by + 1, bz], terravibe_sim::STONE);
        s.set_block([bx, by + 1, bz + 1], terravibe_sim::STONE);
        s.set_block([bx, by + 1, bz - 1], terravibe_sim::STONE);
        s.set_block([bx, by + 1, bz], AIR);
        {
            let mut adapter = ServerLiquidWorld::new(&mut s.world);
            adapter.set_liquid(bx, by + 1, bz, Some(Liquid::water(255)));
        }
        s.liquid_sim.wake([bx, by + 1, bz]);
        for _ in 0..20 {
            s.tick();
        }

        s.save_now().unwrap();
        drop(s);

        let (mut cfg2, _) = saving_cfg("save_liquid_reload");
        cfg2.save = Some(SaveCfg {
            root,
            world: "w".into(),
        });
        let s2 = Server::with_cfg(1, cfg2);
        let ch = s2.world.get(&cp).expect("chunk loaded from save");
        let l = ch.liquid(
            (bx.rem_euclid(32)) as usize,
            ((by + 1).rem_euclid(32)) as usize,
            (bz.rem_euclid(32)) as usize,
        );
        assert_eq!(l, Some(Liquid::water(255)));
    }

    #[test]
    fn reloaded_chunk_wakes_once_then_sleeps() {
        let (cfg, root) = saving_cfg("wake_sleep");
        let mut s = Server::with_cfg(1, cfg);
        let (client, conn) = in_process_pair();
        s.connect(Box::new(conn));
        client
            .send(ClientMsg::Hello {
                seed: 1,
                name: None,
                character: None,
                protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
                password: None,
            })
            .unwrap();
        s.tick();

        let sp = spawn_point(1);
        let cp = ChunkPos::of_block(sp[0] as i32, sp[1] as i32, sp[2] as i32);
        let mut got = Vec::new();
        assert!(pump(&mut s, &client, &mut got, |g| has_chunk(g, cp)));

        let bx = cp.x * 32 + 5;
        let by = cp.y * 32 + 5;
        let bz = cp.z * 32 + 5;
        s.set_block([bx, by, bz], terravibe_sim::STONE);
        s.set_block([bx + 1, by + 1, bz], terravibe_sim::STONE);
        s.set_block([bx - 1, by + 1, bz], terravibe_sim::STONE);
        s.set_block([bx, by + 1, bz + 1], terravibe_sim::STONE);
        s.set_block([bx, by + 1, bz - 1], terravibe_sim::STONE);
        s.set_block([bx, by + 1, bz], AIR);
        {
            let mut adapter = ServerLiquidWorld::new(&mut s.world);
            adapter.set_liquid(bx, by + 1, bz, Some(Liquid::water(255)));
        }
        s.liquid_sim.wake([bx, by + 1, bz]);
        while s.liquid_pending() > 0 {
            s.tick();
        }
        s.save_now().unwrap();
        drop(s);

        let (mut cfg2, _) = saving_cfg("wake_sleep_reload");
        cfg2.save = Some(SaveCfg {
            root,
            world: "w".into(),
        });
        let mut s2 = Server::with_cfg(1, cfg2);
        assert!(
            s2.liquid_pending() > 0,
            "chunk with liquid wakes upon reload"
        );
        s2.tick();
        assert_eq!(s2.liquid_pending(), 0, "settled chunk sleeps after 1 tick");
    }

    #[test]
    fn unloaded_neighbour_blocks_flow_until_streamed() {
        let mut s = Server::new(1);
        let (client, conn) = in_process_pair();
        s.connect(Box::new(conn));
        client
            .send(ClientMsg::Hello {
                seed: 1,
                name: None,
                character: None,
                protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
                password: None,
            })
            .unwrap();
        s.tick();

        let cp0 = ChunkPos { x: 10, y: 0, z: 10 };
        let cp1 = ChunkPos { x: 11, y: 0, z: 10 };

        s.world.insert(cp0, Arc::new(Chunk::filled(AIR)));
        s.edited.insert(cp0);
        s.world.remove(&cp1);

        let bx = cp0.x * 32;
        let by = cp0.y * 32 + 5;
        let bz = cp0.z * 32 + 5;

        // Channel: floor at by, walls at -x and ±z, open at +x (which borders cp1)
        s.set_block([bx + 31, by, bz], terravibe_sim::STONE);
        s.set_block([bx + 30, by, bz], terravibe_sim::STONE);
        s.set_block([bx + 30, by + 1, bz], terravibe_sim::STONE);
        s.set_block([bx + 31, by + 1, bz - 1], terravibe_sim::STONE);
        s.set_block([bx + 31, by + 1, bz + 1], terravibe_sim::STONE);
        s.set_block([bx + 31, by + 1, bz], AIR);
        {
            let mut adapter = ServerLiquidWorld::new(&mut s.world);
            adapter.set_liquid(bx + 31, by + 1, bz, Some(Liquid::water(255)));
        }
        s.liquid_sim.wake([bx + 31, by + 1, bz]);

        for _ in 0..10 {
            s.tick();
        }

        let ch0 = s.world.get(&cp0).unwrap();
        assert!(ch0.liquid(31, 6, 5).is_some());
        assert!(!s.world.contains_key(&cp1));

        let mut ch1 = Chunk::filled(AIR);
        ch1.set(0, 5, 5, terravibe_sim::STONE);
        s.edited.insert(cp1);
        s.install_chunk(cp1, Arc::new(ch1));

        for _ in 0..10 {
            s.tick();
        }

        let ch1 = s.world.get(&cp1).unwrap();
        assert!(
            ch1.liquid(0, 6, 5).is_some(),
            "water flowed into newly streamed chunk"
        );
    }

    #[test]
    fn obsidian_from_contact_is_saved_and_broadcast_as_blockchanged() {
        let (cfg, root) = saving_cfg("obsidian");
        let mut s = Server::with_cfg(1, cfg);
        let (c, conn) = in_process_pair();
        s.connect(Box::new(conn));
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();

        let sp = spawn_point(1);
        let cp = ChunkPos::of_block(sp[0] as i32, sp[1] as i32, sp[2] as i32);
        let mut got = Vec::new();
        assert!(pump(&mut s, &c, &mut got, |g| has_chunk(g, cp)));
        got.clear();

        let bx = cp.x * 32 + 5;
        let by = cp.y * 32 + 5;
        let bz = cp.z * 32 + 5;

        s.set_block([bx, by, bz], terravibe_sim::STONE);
        s.set_block([bx, by + 1, bz], AIR);
        s.set_block([bx, by + 2, bz], AIR);
        {
            let mut adapter = ServerLiquidWorld::new(&mut s.world);
            adapter.set_liquid(bx, by + 1, bz, Some(Liquid::lava(255)));
            adapter.set_liquid(bx, by + 2, bz, Some(Liquid::water(255)));
        }
        s.liquid_sim.wake([bx, by + 1, bz]);
        s.liquid_sim.wake([bx, by + 2, bz]);

        assert!(pump(&mut s, &c, &mut got, |g| {
            g.iter().any(|m| {
                matches!(
                    m,
                    ServerMsg::BlockChanged { pos, block }
                        if *pos == [bx, by + 1, bz] && *block == terravibe_sim::OBSIDIAN
                )
            })
        }));

        s.save_now().unwrap();
        drop(s);

        let (mut cfg2, _) = saving_cfg("obsidian_reload");
        cfg2.save = Some(SaveCfg {
            root,
            world: "w".into(),
        });
        let s2 = Server::with_cfg(1, cfg2);
        let ch = s2.world.get(&cp).unwrap();
        let b = ch.get(
            (bx.rem_euclid(32)) as usize,
            ((by + 1).rem_euclid(32)) as usize,
            (bz.rem_euclid(32)) as usize,
        );
        assert_eq!(
            b,
            terravibe_sim::OBSIDIAN,
            "obsidian was saved and reloaded"
        );
    }

    #[test]
    fn enemies_drops_projectiles_clamped_or_despawned_at_edge() {
        use terravibe_sim::item::{GEL, WOODEN_ARROW, WOODEN_BOW};
        let mut s = Server::with_cfg(
            1,
            ServerCfg {
                natural_spawns: false,
                ..ServerCfg::default()
            },
        );
        let half = s.bounds.half as f32;
        // Place a floor chunk at the +X edge so entities don't hit unloaded blocks
        let edge_cp = ChunkPos::of_block(s.bounds.half - 1, 50, 0);
        let mut ch = Chunk::filled(AIR);
        for lx in 0..32 {
            for lz in 0..32 {
                ch.set(lx, 0, lz, terravibe_sim::STONE);
            }
        }
        s.world.insert(edge_cp, Arc::new(ch));
        s.edited.insert(edge_cp);

        let y = (edge_cp.y * 32 + 2) as f32;
        s.add_enemy(EnemyKind::DemonEye, [half - 0.5, y, 0.5]);
        s.enemies[0].vel = [2.0, 0.0, 0.0];
        s.enemies[0].hurt_ticks = 10;

        s.add_drop(stack(GEL, 1), [half - 0.1, y, 0.5]);
        s.drops[0].vel = [2.0, 0.0, 0.0];

        s.projectiles.push(
            Projectile::fire(
                1,
                0,
                WOODEN_BOW,
                WOODEN_ARROW,
                Prefix(0),
                [half - 0.2, y, 0.5],
                [1.0, 0.0, 0.0],
            )
            .unwrap(),
        );

        let (c, conn) = in_process_pair();
        s.connect(Box::new(conn));
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();
        s.conns[0].player.as_mut().unwrap().body.pos = [half - 5.0, y, 0.5];

        for _ in 0..20 {
            s.tick();
        }

        for e in &s.enemies {
            let hw = e.kind.stats().half[0];
            assert!(
                e.pos[0] <= half - hw + 1e-4,
                "enemy crossed edge: {}",
                e.pos[0]
            );
        }
        for d in &s.drops {
            assert!(d.pos[0] <= half + 1e-4, "drop crossed edge: {}", d.pos[0]);
        }
        assert!(
            s.projectiles.iter().all(|p| s
                .bounds
                .contains_column(p.pos[0].floor() as i32, p.pos[2].floor() as i32)),
            "projectile outside bounds must be despawned"
        );
    }

    #[test]
    fn server_never_streams_chunks_outside_bounds() {
        let mut s = Server::new(1);
        let (c, conn) = in_process_pair();
        s.connect(Box::new(conn));
        let half = s.bounds.half as f32;
        c.send(ClientMsg::Spectate {
            pos: [half - 10.0, 64.0, half - 10.0],
        })
        .unwrap();

        let mut got = Vec::new();
        for _ in 0..60 {
            s.tick();
            std::thread::sleep(Duration::from_millis(2));
            drain(&c, &mut got);
        }

        let streamed: Vec<ChunkPos> = got
            .iter()
            .filter_map(|m| match m {
                ServerMsg::Chunk { pos, .. } => Some(*pos),
                _ => None,
            })
            .collect();
        assert!(
            !streamed.is_empty(),
            "expected chunks inside bounds near edge"
        );
        for pos in streamed {
            assert!(
                s.bounds.chunk_inside(pos),
                "server streamed chunk outside bounds: {pos:?}"
            );
        }
        for &pos in s.world.keys() {
            assert!(
                s.bounds.chunk_inside(pos),
                "server generated chunk outside bounds: {pos:?}"
            );
        }
    }

    #[test]
    fn hello_with_character_applies_inventory_and_stats() {
        use terravibe_sim::character::{Appearance, Character};
        use terravibe_sim::inventory::ItemStack;
        use terravibe_sim::item::{COPPER_BROADSWORD, GOLD_BROADSWORD, Prefix};

        let mut s = Server::new(1);
        let (c, conn) = in_process_pair();
        s.connect(Box::new(conn));

        let mut ch = Character::new("Hero", Appearance::default());
        ch.life = 75;
        ch.life_max = 140;
        ch.mana = 10;
        ch.mana_max = 40;
        ch.potion_delay = 300;
        ch.armor[0] = Some(GOLD_BROADSWORD);
        ch.inventory.slots[10] = Some(ItemStack {
            id: COPPER_BROADSWORD,
            count: 5,
            prefix: Prefix(0),
        });

        c.send(ClientMsg::Hello {
            seed: 1,
            name: Some("Hero".into()),
            character: Some(ch.to_bytes()),
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();

        let conn = &s.conns[0];
        let p = conn.player.as_ref().unwrap();
        assert_eq!(p.vitals.life, 75);
        assert_eq!(p.vitals.life_max, 140);
        assert_eq!(p.vitals.mana, 10);
        assert_eq!(p.vitals.mana_max, 40);
        assert_eq!(p.vitals.potion_delay_ticks, 300);
        assert_eq!(p.armor[0], Some(GOLD_BROADSWORD));
        assert_eq!(
            conn.inv.slots[10].as_ref().map(|s| (s.id, s.count)),
            Some((COPPER_BROADSWORD, 5))
        );
    }

    #[test]
    fn hello_character_wins_over_existing_world_player_inventory_keeps_position() {
        use terravibe_sim::character::{Appearance, Character};
        use terravibe_sim::item::{COPPER_BROADSWORD, GOLD_BROADSWORD, Prefix};

        let (cfg, root) = saving_cfg("char_wins");
        let mut s = Server::with_cfg(1, cfg);
        let (c, conn) = in_process_pair();
        s.connect(Box::new(conn));

        let mut world_inv = Inventory::empty();
        world_inv.slots[0] = Some(terravibe_sim::inventory::ItemStack {
            id: COPPER_BROADSWORD,
            count: 1,
            prefix: Prefix(0),
        });
        s.saved_players.insert(
            "Hero".into(),
            PlayerSave {
                inventory: world_inv,
                armor: [None; 3],
                life_max: 100,
                life: 100,
                pos: [12.0, 65.0, 14.0],
                potion_delay: 0,
            },
        );

        let mut ch = Character::new("Hero", Appearance::default());
        ch.armor[0] = Some(GOLD_BROADSWORD);
        ch.life = 88;

        c.send(ClientMsg::Hello {
            seed: 1,
            name: Some("Hero".into()),
            character: Some(ch.to_bytes()),
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();

        let conn = &s.conns[0];
        let p = conn.player.as_ref().unwrap();
        assert_eq!(p.armor[0], Some(GOLD_BROADSWORD));
        assert_eq!(p.vitals.life, 88);
        assert_eq!(p.body.pos, [12.0, 65.0, 14.0]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn hello_bad_character_bytes_falls_back() {
        let mut s = Server::new(1);
        let (c, conn) = in_process_pair();
        s.connect(Box::new(conn));

        c.send(ClientMsg::Hello {
            seed: 1,
            name: Some("Regular".into()),
            character: Some(vec![0xDE, 0xAD, 0xBE, 0xEF]),
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();

        let conn = &s.conns[0];
        let p = conn.player.as_ref().unwrap();
        assert_eq!(p.vitals.life, 100);
        assert_eq!(conn.name, "Regular");
    }

    #[test]
    fn character_state_sent_on_disconnect_with_held_returned() {
        use terravibe_sim::character::{Appearance, Character};
        use terravibe_sim::item::COPPER_SHORTSWORD;
        use terravibe_sim::slots::{ClickKind, SlotRef};

        let mut s = Server::new(1);
        let (c, conn) = in_process_pair();
        s.connect(Box::new(conn));

        let ch = Character::new("Hero", Appearance::default());
        c.send(ClientMsg::Hello {
            seed: 1,
            name: Some("Hero".into()),
            character: Some(ch.to_bytes()),
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();

        c.send(ClientMsg::ClickSlot {
            at: SlotRef::Inv(0),
            kind: ClickKind::Left,
        })
        .unwrap();
        s.tick();
        assert!(
            s.conns[0].held.0.is_some(),
            "cursor must hold the picked item"
        );

        c.send(ClientMsg::Bye).unwrap();
        s.tick();

        let mut got_char_state = None;
        while let Ok(Some(m)) = c.try_recv() {
            if let ServerMsg::CharacterState(bytes) = m {
                got_char_state = Some(bytes);
            }
        }

        let bytes = got_char_state.expect("must receive CharacterState on disconnect");
        let decoded = Character::from_bytes(&bytes).expect("must decode CharacterState");
        assert_eq!(decoded.name, "Hero");
        assert!(
            decoded
                .inventory
                .slots
                .iter()
                .flatten()
                .any(|s| s.id == COPPER_SHORTSWORD),
            "held item must be safely returned to bag in character state"
        );
    }

    #[test]
    fn character_state_persists_equipped_accessories_on_disconnect() {
        use terravibe_sim::character::{Appearance, Character};
        use terravibe_sim::item::HERMES_BOOTS;
        use terravibe_sim::slots::{ClickKind, SlotRef};

        let mut s = Server::new(1);
        let (c, conn) = in_process_pair();
        s.connect(Box::new(conn));

        let mut ch = Character::new("Runner", Appearance::default());
        ch.inventory.slots[0] = Some(terravibe_sim::inventory::ItemStack {
            id: HERMES_BOOTS,
            count: 1,
            prefix: terravibe_sim::item::Prefix(0),
        });
        c.send(ClientMsg::Hello {
            seed: 1,
            name: Some("Runner".into()),
            character: Some(ch.to_bytes()),
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();

        // Pick up boots from inv 0
        c.send(ClientMsg::ClickSlot {
            at: SlotRef::Inv(0),
            kind: ClickKind::Left,
        })
        .unwrap();
        s.tick();

        // Place into accessory slot 0
        c.send(ClientMsg::ClickSlot {
            at: SlotRef::Accessory(0),
            kind: ClickKind::Left,
        })
        .unwrap();
        s.tick();

        c.send(ClientMsg::Bye).unwrap();
        s.tick();

        let mut got_char_state = None;
        while let Ok(Some(m)) = c.try_recv() {
            if let ServerMsg::CharacterState(bytes) = m {
                got_char_state = Some(bytes);
            }
        }

        let bytes = got_char_state.expect("must receive CharacterState on disconnect");
        let decoded = Character::from_bytes(&bytes).expect("must decode CharacterState");
        assert_eq!(decoded.accessories[0], Some(HERMES_BOOTS));
    }

    #[test]
    fn no_character_hello_unchanged() {
        let mut s = Server::new(1);
        let (c, conn) = in_process_pair();
        s.connect(Box::new(conn));

        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();

        assert_eq!(s.conns[0].name, "player");
        assert!(s.conns[0].character.is_none());
    }

    mod stage6_tests {
        use super::*;

        fn build_house_via_edit_query(s: &mut Server, ox: i32, oy: i32, oz: i32) -> [i32; 3] {
            use terravibe_sim::{CHAIR, DOOR_CLOSED, STONE, TABLE, TORCH};
            let mut log = Vec::new();
            let mut wall_log = Vec::new();
            let mut liquid_log = Vec::new();
            {
                let mut q = EditQ {
                    world: &mut s.world,
                    log: &mut log,
                    chests: &mut s.chests,
                    wall_log: &mut wall_log,
                    liquid_log: &mut liquid_log,
                };
                for x in ox..ox + 7 {
                    for y in oy..oy + 5 {
                        for z in oz..oz + 7 {
                            let inner = (ox + 1..ox + 6).contains(&x)
                                && (oy + 1..oy + 4).contains(&y)
                                && (oz + 1..oz + 6).contains(&z);
                            if inner {
                                q.try_set_block(x, y, z, AIR);
                                q.try_set_wall(x, y, z, true);
                            } else {
                                q.try_set_block(x, y, z, STONE);
                            }
                        }
                    }
                }
                q.try_set_block(ox, oy + 1, oz + 3, DOOR_CLOSED);
                q.try_set_block(ox, oy + 2, oz + 3, DOOR_CLOSED);
                q.try_set_block(ox + 2, oy + 1, oz + 2, CHAIR);
                q.try_set_block(ox + 4, oy + 1, oz + 2, TABLE);
                q.try_set_block(ox + 3, oy + 3, oz + 4, TORCH);
            }
            for (pos, _) in log {
                let cp = ChunkPos::of_block(pos[0], pos[1], pos[2]);
                s.edited.insert(cp);
                s.dirty.insert(cp);
            }
            for (pos, _) in wall_log {
                let cp = ChunkPos::of_block(pos[0], pos[1], pos[2]);
                s.edited.insert(cp);
                s.dirty.insert(cp);
            }
            [ox + 2, oy + 1, oz + 2]
        }

        #[test]
        fn stage6_e2e_house_npc_shop_roundtrip() {
            use terravibe_sim::coin::give_copper;
            use terravibe_sim::item::TORCH;

            let (cfg, root) = saving_cfg("stage6_e2e_house");
            let mut s = Server::with_cfg(
                1,
                ServerCfg {
                    natural_spawns: true,
                    clock_start: 0,
                    ..cfg
                },
            );
            let (c, conn) = in_process_pair();
            s.connect(Box::new(conn));
            let mut got = Vec::new();
            hello_and_load(&mut s, &c, &mut got);

            // 1. Guide exists from world start
            let t0 = Instant::now();
            while !s.enemies.iter().any(|e| e.kind == EnemyKind::Guide)
                && t0.elapsed() < Duration::from_secs(3)
            {
                s.tick();
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(
                s.enemies.iter().any(|e| e.kind == EnemyKind::Guide),
                "Guide must exist on new world"
            );
            assert!(s.guide_spawned);

            // 2. Build test house through EditQuery -> Guide moves in
            let cp1 = ChunkPos { x: 2, y: 1, z: 2 };
            s.world
                .entry(cp1)
                .or_insert_with(|| Arc::new(Chunk::filled(AIR)));
            let home1 = build_house_via_edit_query(
                &mut s,
                cp1.x * CHUNK_SIZE + 4,
                cp1.y * CHUNK_SIZE + 4,
                cp1.z * CHUNK_SIZE + 4,
            );

            s.scan_houses();
            s.move_in();
            assert_eq!(s.houses.len(), 1);
            assert_eq!(s.houses[0].npc, Some(TownKind::Guide));
            let guide = s
                .enemies
                .iter()
                .find(|e| e.kind == EnemyKind::Guide)
                .unwrap();
            assert_eq!(
                guide.pos,
                [
                    home1[0] as f32 + 0.5,
                    home1[1] as f32,
                    home1[2] as f32 + 0.5
                ]
            );

            // 3. Build a second vacant house for Merchant
            let cp2 = ChunkPos { x: 3, y: 1, z: 2 };
            s.world
                .entry(cp2)
                .or_insert_with(|| Arc::new(Chunk::filled(AIR)));
            let _home2 = build_house_via_edit_query(
                &mut s,
                cp2.x * CHUNK_SIZE + 4,
                cp2.y * CHUNK_SIZE + 4,
                cp2.z * CHUNK_SIZE + 4,
            );
            s.scan_houses();
            assert_eq!(s.houses.len(), 2);
            assert!(s.houses.iter().any(|h| h.npc.is_none()));

            // Give player >= 5000 copper (5050 so they can buy the 50 copper torch)
            give_copper(&mut s.conns[0].inv, 5050);

            // Merchant spawns at daytime check (e.g. tick 7200)
            s.clock.tick = town_npc::SPAWN_CHECK_TICKS - 1;
            s.tick();
            let merchant = s.enemies.iter().find(|e| e.kind == EnemyKind::Merchant);
            assert!(
                merchant.is_some(),
                "Merchant must spawn with >= 5000 copper and vacant house"
            );
            let merchant_id = merchant.unwrap().id;

            // 4. Buy a Torch
            let merchant_pos = s.enemies.iter().find(|e| e.id == merchant_id).unwrap().pos;
            s.conns[0].player.as_mut().unwrap().body.pos = merchant_pos;
            let torch_count_before = s.conns[0].inv.count_of(TORCH);

            c.send(ClientMsg::TalkNpc { id: merchant_id }).unwrap();
            c.send(ClientMsg::ShopBuy {
                npc: merchant_id,
                index: 3,
                count: 1,
            })
            .unwrap();
            s.tick();

            let mut msgs = Vec::new();
            drain(&c, &mut msgs);
            assert!(
                msgs.iter()
                    .any(|m| matches!(m, ServerMsg::ShopResult { ok: true, .. })),
                "Shop buy must succeed"
            );
            assert_eq!(
                s.conns[0].inv.count_of(TORCH),
                torch_count_before + 1,
                "Player must receive 1 torch"
            );
            assert_eq!(
                terravibe_sim::coin::total_copper(&s.conns[0].inv),
                5000,
                "50 copper paid"
            );

            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn stage6_e2e_corruption_altar_and_mana_roundtrip() {
            use terravibe_sim::DEMON_ALTAR;
            use terravibe_sim::crafting::SUSPICIOUS_EYE_RECIPE;
            use terravibe_sim::item::{
                FLAMING_ARROW, LENS, MANA_CRYSTAL, SUSPICIOUS_LOOKING_EYE, WAND_OF_SPARKING,
            };
            use terravibe_sim::projectile::ProjKind;

            let (cfg, root) = saving_cfg("stage6_e2e_corrupt");
            let mut s = Server::with_cfg(
                1,
                ServerCfg {
                    natural_spawns: false,
                    ..cfg
                },
            );
            let (c, conn) = in_process_pair();
            s.connect(Box::new(conn));
            let mut got = Vec::new();
            let ch = terravibe_sim::character::Character::new(
                "Hero",
                terravibe_sim::character::Appearance::default(),
            );
            c.send(ClientMsg::Hello {
                seed: 1,
                name: Some("Hero".into()),
                character: Some(ch.to_bytes()),
                protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
                password: None,
            })
            .unwrap();
            let sp = spawn_point(1);
            let want = ChunkPos::of_block(0, sp[1] as i32 - 1, 0);
            assert!(pump(&mut s, &c, &mut got, |g| has_chunk(g, want)));

            // 1. Verify corruption zone generates in world
            let mut found_corrupt = false;
            for x in (350..=3000).step_by(50) {
                for z in (350..=3000).step_by(50) {
                    if terravibe_sim::worldgen::corruption::corruption_at(1, x, z) > 0.1 {
                        found_corrupt = true;
                        break;
                    }
                }
                if found_corrupt {
                    break;
                }
            }
            assert!(found_corrupt, "corruption zone must generate in world");

            // Verify altar is craftable-at
            let p_pos = s.conns[0].player.as_ref().unwrap().body.pos;
            let altar_pos = [p_pos[0] as i32, p_pos[1] as i32, p_pos[2] as i32 + 2];
            let cp = ChunkPos::of_block(altar_pos[0], altar_pos[1], altar_pos[2]);
            s.world
                .entry(cp)
                .or_insert_with(|| Arc::new(Chunk::filled(AIR)));
            set_block(&mut s, altar_pos, DEMON_ALTAR);
            s.edited.insert(cp);
            s.dirty.insert(cp);

            s.conns[0].inv.slots[1] = Some(stack(LENS, 6));
            let msgs = craft_req(&mut s, &c, SUSPICIOUS_EYE_RECIPE, 1);
            let res = result_of(&msgs);
            assert_eq!(
                res,
                Some((1, None)),
                "must craft Suspicious Looking Eye at Demon Altar"
            );
            assert_eq!(s.conns[0].inv.count_of(SUSPICIOUS_LOOKING_EYE), 1);

            // 2. Use a Mana Crystal
            assert_eq!(s.conns[0].player.as_ref().unwrap().vitals.mana_max, 20);
            assert_eq!(s.conns[0].player.as_ref().unwrap().vitals.mana, 20);
            s.conns[0].inv.slots[2] = Some(stack(MANA_CRYSTAL, 1));
            s.conns[0].inv.selected = 2;
            let mut v = s.conns[0].player.as_mut().unwrap().vitals.clone();
            assert!(terravibe_sim::interact::apply_consumable(
                MANA_CRYSTAL,
                &mut v,
                &mut s.conns[0].inv
            ));
            s.conns[0].player.as_mut().unwrap().vitals = v;
            assert_eq!(s.conns[0].player.as_ref().unwrap().vitals.mana_max, 40);
            assert_eq!(s.conns[0].player.as_ref().unwrap().vitals.mana, 40);

            // 3. Cast a Wand of Sparking
            s.conns[0].inv.slots[0] = Some(stack(WAND_OF_SPARKING, 1));
            s.conns[0].inv.selected = 0;
            clear_arrow_lane(&mut s);
            c.send(ClientMsg::Input {
                seq: 1,
                input: fire_input(),
            })
            .unwrap();
            s.tick();
            assert_eq!(s.projectiles.len(), 1);
            assert_eq!(s.projectiles[0].kind, ProjKind::Spark);
            assert_eq!(s.conns[0].player.as_ref().unwrap().vitals.mana, 38);

            // 4. Kill an Eater of Souls and a flaming-arrow-hit slime burns
            s.add_enemy(
                EnemyKind::EaterOfSouls,
                [p_pos[0] + 5.0, p_pos[1], p_pos[2]],
            );
            assert!(s.enemies.iter().any(|e| e.kind == EnemyKind::EaterOfSouls));
            s.enemies
                .iter_mut()
                .find(|e| e.kind == EnemyKind::EaterOfSouls)
                .unwrap()
                .hp = 0;
            s.tick();
            assert!(!s.enemies.iter().any(|e| e.kind == EnemyKind::EaterOfSouls));
            assert!(!s.drops.is_empty(), "Eater of Souls drops loot upon death");

            s.add_enemy(EnemyKind::Slime, [p_pos[0] + 10.0, p_pos[1], p_pos[2]]);
            let slime_id = s
                .enemies
                .iter()
                .find(|e| e.kind == EnemyKind::Slime)
                .unwrap()
                .id;
            let flaming = Projectile::fire_from_bow(
                s.next_proj_id,
                0,
                FLAMING_ARROW,
                [p_pos[0] + 9.5, p_pos[1] + 0.5, p_pos[2]],
                [1.0, 0.0, 0.0],
            )
            .unwrap();
            s.projectiles.push(flaming);
            s.tick();
            let slime = s.enemies.iter().find(|e| e.id == slime_id).unwrap();
            assert!(
                slime.is_on_fire(),
                "slime hit with flaming arrow must be on fire"
            );
            let hp_before = slime.hp;
            for _ in 0..15 {
                s.tick();
            }
            let slime_after = s.enemies.iter().find(|e| e.id == slime_id).unwrap();
            assert!(slime_after.hp < hp_before, "on-fire DoT must damage slime");

            // 5. Save, restart, all state restored
            s.save_now().unwrap();
            c.send(ClientMsg::Bye).unwrap();
            s.tick();
            let mut char_bytes = None;
            while let Ok(Some(m)) = c.try_recv() {
                if let ServerMsg::CharacterState(bytes) = m {
                    char_bytes = Some(bytes);
                }
            }
            let char_bytes = char_bytes.expect("disconnect yields CharacterState");
            drop(c);
            drop(s);

            let (mut s2, c2) = restart(&root);
            c2.send(ClientMsg::Hello {
                seed: 1,
                name: Some("Hero".into()),
                character: Some(char_bytes),
                protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
                password: None,
            })
            .unwrap();
            let mut got2 = Vec::new();
            assert!(pump(&mut s2, &c2, &mut got2, |g| has_chunk(g, cp)));
            let p2 = s2.conns[0].player.as_ref().unwrap();
            assert_eq!(p2.vitals.mana_max, 40, "mana_max restored");
            assert_eq!(p2.vitals.mana, 38, "mana restored");
            assert_eq!(
                WorldQ(&s2.world).block(altar_pos[0], altar_pos[1], altar_pos[2]),
                DEMON_ALTAR,
                "Demon Altar block restored"
            );

            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn room_check_rate_limited() {
            let (mut s, c) = setup();
            let mut got = Vec::new();
            hello_and_load(&mut s, &c, &mut got);

            let home = build_room(&mut s, ChunkPos { x: 50, y: 1, z: 50 });
            s.scan_houses();
            assert_eq!(s.houses.len(), 1);
            assert_eq!(s.houses[0].home, home);

            // Align clock to a 120 boundary so subsequent ticks test the exact 120-tick period
            s.clock.tick = 120;
            let initial_checks = s.room_checks;

            // Ticking for 119 ticks: scan_houses must NOT be called on tick % 120 != 0
            for _ in 0..119 {
                s.tick();
            }
            assert_eq!(
                s.room_checks, initial_checks,
                "room checks must stay off the per-tick path"
            );

            // The 120th tick triggers scan_houses exactly once (2.0s server time)
            s.tick();
            assert_eq!(
                s.room_checks - initial_checks,
                1,
                "check_room called at most once per 2s per house"
            );

            // Another 120 ticks triggers it exactly once more
            for _ in 0..119 {
                s.tick();
            }
            assert_eq!(s.room_checks - initial_checks, 1);
            s.tick();
            assert_eq!(s.room_checks - initial_checks, 2);
        }

        #[test]
        fn cave_enemies_spawn_only_in_deep_layers() {
            let (client, conn) = in_process_pair();
            let mut s = Server::with_cfg(
                1,
                ServerCfg {
                    natural_spawns: true,
                    ..Default::default()
                },
            );
            s.connect(Box::new(conn));
            let mut got = Vec::new();
            hello_and_load(&mut s, &client, &mut got);

            let (sx, sz) = (s.spawn[0].floor() as i32, s.spawn[2].floor() as i32);
            let t0 = Instant::now();
            while WorldQ(&s.world).surface_y(sx, sz).is_none() {
                assert!(t0.elapsed() < Duration::from_secs(5), "column never loaded");
                s.tick();
                drain(&client, &mut got);
                std::thread::sleep(Duration::from_millis(1));
            }
            let surface_y = WorldQ(&s.world).surface_y(sx, sz).unwrap();

            // Ensure columns in spawn ring around player are loaded in s.world
            for dx in -5..=5 {
                for dz in -5..=5 {
                    for y in 0..4 {
                        let cp = ChunkPos {
                            x: (sx / CHUNK_SIZE) + dx,
                            y,
                            z: (sz / CHUNK_SIZE) + dz,
                        };
                        if !s.world.contains_key(&cp) {
                            let ch = terravibe_sim::generate_chunk(s.seed, cp);
                            s.world.insert(cp, Arc::new(ch));
                        }
                    }
                }
            }

            // 1. Surface player test: player is at surface_y + 1
            s.enemies.clear();
            let mut surface_spawned = false;
            for _ in 0..30 {
                s.spawner.ticks = terravibe_sim::spawn::SPAWN_RATE - 1;
                s.tick();
                for e in s
                    .enemies
                    .iter()
                    .filter(|e| !e.kind.is_friendly() && !e.kind.is_critter())
                {
                    assert!(
                        !matches!(
                            e.kind,
                            EnemyKind::Skeleton | EnemyKind::CaveBat | EnemyKind::GiantWorm
                        ),
                        "cave enemy {:?} spawned at surface depth pos {:?}",
                        e.kind,
                        e.pos
                    );
                    surface_spawned = true;
                }
            }
            assert!(
                surface_spawned,
                "expected surface enemies to spawn for surface player"
            );

            // 2. Deep player test: move player below the lowest surface in the spawn
            // ring, but above the Underworld band, which has its own spawn pool.
            let mut ring_min_sy = i32::MAX;
            for dx in -90..=90 {
                for dz in -90..=90 {
                    let r = ((dx * dx + dz * dz) as f32).sqrt();
                    if !(60.0..=90.0).contains(&r) {
                        continue;
                    }
                    if let Some(sy) = WorldQ(&s.world).surface_y(sx + dx, sz + dz) {
                        ring_min_sy = ring_min_sy.min(sy);
                    }
                }
            }
            assert!(ring_min_sy < i32::MAX, "spawn ring columns never loaded");
            let deep_y = ring_min_sy - 35;
            assert!(
                deep_y > terravibe_sim::worldgen::underworld::UNDERWORLD_TOP_Y,
                "test probe must stay above the Underworld band (surface {surface_y}, ring min {ring_min_sy}, deep {deep_y})"
            );
            if let Some(conn) = s.conns.first_mut() {
                if let Some(player) = conn.player.as_mut() {
                    player.body.pos = [sx as f32 + 0.5, deep_y as f32, sz as f32 + 0.5];
                    conn.interest.centre = Some(player.body.pos);
                }
            }
            s.enemies.clear();
            let mut spawned_cave_enemy = false;
            for _ in 0..30 {
                s.spawner.ticks = terravibe_sim::spawn::SPAWN_RATE - 1;
                s.tick();
                for e in s
                    .enemies
                    .iter()
                    .filter(|e| !e.kind.is_friendly() && !e.kind.is_critter())
                {
                    assert!(
                        matches!(
                            e.kind,
                            EnemyKind::Skeleton | EnemyKind::CaveBat | EnemyKind::GiantWorm
                        ),
                        "expected cave enemy underground, got {:?}",
                        e.kind
                    );
                    assert!(
                        e.pos[1] <= (surface_y - 30) as f32,
                        "cave enemy spawned at pos {:?} which is above underground boundary {}",
                        e.pos,
                        surface_y - 30
                    );
                    spawned_cave_enemy = true;
                }
            }
            assert!(
                spawned_cave_enemy,
                "expected cave enemies to spawn for underground player"
            );
        }

        #[test]
        fn m6_e2e_cavern_exploration_and_accessories() {
            use terravibe_sim::block::{AIR, CHEST, WOOD_BLOCK};
            use terravibe_sim::enemy::EnemyKind;
            use terravibe_sim::item::{
                CLOUD_IN_A_BOTTLE, ENCHANTED_BOOMERANG, HERMES_BOOTS, SPEAR,
            };
            use terravibe_sim::player::PlayerInput;
            use terravibe_sim::projectile::ProjKind;
            use terravibe_sim::slots::{ClickKind, SlotRef};
            use terravibe_sim::worldgen::terrain::{DepthBand, depth_band};

            // 1. Creates test server world & connects client
            let (client, conn) = in_process_pair();
            let mut s = Server::with_cfg(
                1,
                ServerCfg {
                    natural_spawns: true,
                    ..Default::default()
                },
            );
            s.connect(Box::new(conn));
            let mut got = Vec::new();
            hello_and_load(&mut s, &client, &mut got);

            // 2. Spawns player, verifies surface vs underground depth bands
            let sx = s.spawn[0].floor() as i32;
            let sz = s.spawn[2].floor() as i32;
            let t0 = Instant::now();
            while WorldQ(&s.world).surface_y(sx, sz).is_none() {
                assert!(t0.elapsed() < Duration::from_secs(5), "column never loaded");
                s.tick();
                drain(&client, &mut got);
                std::thread::sleep(Duration::from_millis(1));
            }
            let surface_y = WorldQ(&s.world).surface_y(sx, sz).unwrap();
            let player_spawn_y = s.conns[0].player.as_ref().unwrap().body.pos[1].floor() as i32;

            assert_eq!(
                depth_band(surface_y, player_spawn_y),
                DepthBand::Surface,
                "spawn must be in Surface depth band"
            );
            assert_eq!(
                depth_band(surface_y, surface_y - 30),
                DepthBand::Underground,
                "surface_y - 30 must be Underground depth band"
            );

            // 3. Teleports / moves down to cavern depth
            let cavern_y = surface_y - 120;
            assert_eq!(
                depth_band(surface_y, cavern_y),
                DepthBand::Cavern,
                "cavern_y must be Cavern depth band"
            );

            let cavern_pos = [sx as f32 + 0.5, cavern_y as f32, sz as f32 + 0.5];
            if let Some(conn) = s.conns.first_mut() {
                if let Some(player) = conn.player.as_mut() {
                    player.body.pos = cavern_pos;
                    player.body.vel = [0.0, 0.0, 0.0];
                    player.body.on_ground = true;
                    conn.interest.centre = Some(cavern_pos);
                }
            }
            // Ensure chunks around cavern pos are loaded into s.world
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        let cp = ChunkPos {
                            x: (sx / CHUNK_SIZE) + dx,
                            y: (cavern_y / CHUNK_SIZE) + dy,
                            z: (sz / CHUNK_SIZE) + dz,
                        };
                        s.world
                            .entry(cp)
                            .or_insert_with(|| Arc::new(Chunk::filled(AIR)));
                    }
                }
            }

            // 4. Spawns a cabin with a chest containing cavern loot (Hermes Boots, Cloud in a Bottle)
            for dz in -20..=20 {
                for dx in -3..=3 {
                    for dy in -1..=4 {
                        let wx = sx + dx;
                        let wy = cavern_y + dy;
                        let wz = sz + dz;
                        let is_floor = dy == -1;
                        let is_ceiling = dy == 4;
                        let is_wall = (dx.abs() == 3 || dz.abs() == 20) && !is_floor && !is_ceiling;
                        let block = if is_floor || is_ceiling || is_wall {
                            WOOD_BLOCK
                        } else {
                            AIR
                        };
                        s.set_block([wx, wy, wz], block);
                    }
                }
            }

            let chest_pos = [sx + 1, cavern_y, sz];
            s.set_block(chest_pos, CHEST);
            s.chests.entry(chest_pos).or_default().slots[0] = Some(stack(HERMES_BOOTS, 1));
            s.chests.entry(chest_pos).or_default().slots[1] = Some(stack(CLOUD_IN_A_BOTTLE, 1));

            // 5. Equips Hermes Boots and Cloud in a Bottle into accessory slots via slot clicks
            client
                .send(ClientMsg::OpenChest { pos: chest_pos })
                .unwrap();
            s.tick();

            // Pick up Hermes Boots from chest slot 0 to held
            click(&client, SlotRef::Chest(0), ClickKind::Left);
            s.tick();
            // Place into accessory slot 0
            click(&client, SlotRef::Accessory(0), ClickKind::Left);
            s.tick();

            // Pick up Cloud in a Bottle from chest slot 1 to held
            click(&client, SlotRef::Chest(1), ClickKind::Left);
            s.tick();
            // Place into accessory slot 1
            click(&client, SlotRef::Accessory(1), ClickKind::Left);
            s.tick();

            client.send(ClientMsg::CloseInventory).unwrap();
            s.tick();

            assert_eq!(
                s.conns[0].player.as_ref().unwrap().accessories[0],
                Some(HERMES_BOOTS)
            );
            assert_eq!(
                s.conns[0].player.as_ref().unwrap().accessories[1],
                Some(CLOUD_IN_A_BOTTLE)
            );

            // 6. Verifies double jump and boosted run speed active
            // 6a. Sprint along negative Z corridor with Hermes Boots
            let run_input = PlayerInput {
                forward: 1.0,
                yaw: 0.0,
                pitch: 0.0,
                ..Default::default()
            };
            for seq in 1..=30 {
                client
                    .send(ClientMsg::Input {
                        seq,
                        input: run_input,
                    })
                    .unwrap();
                s.tick();
            }
            let speed = {
                let vel = s.conns[0].player.as_ref().unwrap().body.vel;
                vel[0].hypot(vel[2])
            };
            assert!(
                speed > terravibe_sim::player::consts::MAX_RUN_SPEED * 1.1,
                "speed ({speed}) must be boosted by Hermes Boots"
            );
            assert!(
                s.conns[0].player.as_ref().unwrap().body.run_ticks >= 20,
                "run_ticks must reach Hermes threshold"
            );

            // 6b. Cloud in a Bottle double jump mid-air
            if let Some(conn) = s.conns.first_mut() {
                if let Some(player) = conn.player.as_mut() {
                    player.body.pos = [sx as f32 + 0.5, cavern_y as f32 + 2.0, sz as f32 + 0.5];
                    player.body.vel = [0.0, -0.1, 0.0];
                    player.body.on_ground = false;
                    player.body.double_jump_ready = true;
                    player.body.jump_held_prev = false;
                }
            }
            client
                .send(ClientMsg::Input {
                    seq: 50,
                    input: PlayerInput::default(),
                })
                .unwrap();
            s.tick();
            assert!(
                s.conns[0].player.as_ref().unwrap().body.vel[1] < 0.0,
                "player must be falling"
            );
            assert!(
                s.conns[0].player.as_ref().unwrap().body.double_jump_ready,
                "double jump must still be ready"
            );

            // Press jump in mid-air
            client
                .send(ClientMsg::Input {
                    seq: 51,
                    input: PlayerInput {
                        jump: true,
                        ..Default::default()
                    },
                })
                .unwrap();
            s.tick();
            assert!(
                s.conns[0].player.as_ref().unwrap().body.vel[1] > 0.0,
                "double jump must grant positive vertical velocity"
            );
            assert!(
                !s.conns[0].player.as_ref().unwrap().body.double_jump_ready,
                "double jump must be consumed"
            );

            // 7. Engages CaveBat and Skeleton enemies with Enchanted Boomerang and Spear, verifying damage
            // Reset player position on ground
            if let Some(conn) = s.conns.first_mut() {
                if let Some(player) = conn.player.as_mut() {
                    player.body.pos = [sx as f32 + 0.5, cavern_y as f32, sz as f32 + 0.5];
                    player.body.vel = [0.0, 0.0, 0.0];
                    player.body.on_ground = true;
                }
            }

            // 7a. CaveBat with Enchanted Boomerang
            s.conns[0].inv.slots[0] = Some(stack(ENCHANTED_BOOMERANG, 1));
            s.conns[0].inv.selected = 0;
            s.conns[0].use_state.anim = 0;
            s.conns[0].use_state.time = 0;
            s.conns[0].use_state.last_use_held = false;

            let bat_pos = [sx as f32 + 0.5, cavern_y as f32 + 1.0, sz as f32 - 3.0];
            s.add_enemy(EnemyKind::CaveBat, bat_pos);
            let bat = s
                .enemies
                .iter()
                .find(|e| e.kind == EnemyKind::CaveBat)
                .expect("CaveBat present");
            let bat_id = bat.id;
            let bat_hp_before = bat.hp;
            assert_eq!(bat_hp_before, 16);

            client
                .send(ClientMsg::Input {
                    seq: 100,
                    input: PlayerInput {
                        use_held: true,
                        yaw: 0.0,
                        pitch: 0.0,
                        ..Default::default()
                    },
                })
                .unwrap();
            s.tick();

            assert!(
                s.projectiles
                    .iter()
                    .any(|pr| pr.kind == ProjKind::Boomerang),
                "Boomerang projectile must be spawned"
            );

            for _ in 0..15 {
                s.tick();
            }

            let bat_after = s.enemies.iter().find(|e| e.id == bat_id);
            let bat_damaged_or_killed = bat_after.map_or(true, |b| b.hp < bat_hp_before);
            assert!(
                bat_damaged_or_killed,
                "CaveBat must take damage or die from Enchanted Boomerang"
            );

            // 7b. Skeleton with Spear
            s.projectiles.clear();
            s.conns[0].inv.slots[1] = Some(stack(SPEAR, 1));
            s.conns[0].inv.selected = 1;
            s.conns[0].use_state.anim = 0;
            s.conns[0].use_state.time = 0;
            s.conns[0].use_state.last_use_held = false;

            let skel_pos = [sx as f32 + 0.5, cavern_y as f32, sz as f32 - 2.0];
            s.add_enemy(EnemyKind::Skeleton, skel_pos);
            let skel = s
                .enemies
                .iter()
                .find(|e| e.kind == EnemyKind::Skeleton)
                .expect("Skeleton present");
            let skel_id = skel.id;
            let skel_hp_before = skel.hp;
            assert_eq!(skel_hp_before, 60);

            client
                .send(ClientMsg::Input {
                    seq: 200,
                    input: PlayerInput {
                        use_held: true,
                        slot: 1,
                        yaw: 0.0,
                        pitch: 0.0,
                        ..Default::default()
                    },
                })
                .unwrap();
            s.tick();

            assert!(
                s.projectiles.iter().any(|pr| pr.kind == ProjKind::Spear),
                "Spear projectile must be spawned"
            );

            for _ in 0..10 {
                s.tick();
            }

            let skel_after = s
                .enemies
                .iter()
                .find(|e| e.id == skel_id)
                .expect("Skeleton survives spear hit due to 60 hp");
            assert!(
                skel_after.hp < skel_hp_before,
                "Skeleton hp ({}) must be reduced by Spear hit (was {})",
                skel_after.hp,
                skel_hp_before
            );
        }

        #[test]
        fn first_orb_always_drops_musket() {
            use terravibe_sim::item::{MUSKET, MUSKET_BALL};
            let mut s = Server::new(42);
            assert_eq!(s.shadow_orbs_broken, 0);
            let orb_pos = [10, 20, 30];
            s.handle_shadow_orb_broken(orb_pos);
            assert_eq!(s.shadow_orbs_broken, 1);
            let drops: Vec<_> = s
                .drops
                .iter()
                .map(|d| (d.stack.id, d.stack.count))
                .collect();
            assert!(drops.contains(&(MUSKET, 1)), "first orb must drop 1 Musket");
            assert!(
                drops.contains(&(MUSKET_BALL, 100)),
                "first orb must drop 100 Musket Balls"
            );

            s.handle_shadow_orb_broken(orb_pos);
            assert_eq!(s.shadow_orbs_broken, 2);
            assert!(s.drops.len() >= 3);
            assert!(
                !s.enemies
                    .iter()
                    .any(|e| terravibe_sim::enemy::is_eater_of_worlds(e.kind)),
                "2nd orb does not summon boss"
            );

            s.handle_shadow_orb_broken(orb_pos);
            assert_eq!(s.shadow_orbs_broken, 3);
            assert!(
                s.enemies
                    .iter()
                    .any(|e| e.kind == EnemyKind::EaterOfWorldsHead),
                "3rd orb break summons Eater of Worlds"
            );
        }

        #[test]
        fn musket_consumes_ammo_and_fires_bullet() {
            use terravibe_sim::inventory::{AMMO0, ItemStack};
            use terravibe_sim::item::{MUSKET, MUSKET_BALL, Prefix};
            use terravibe_sim::player::PlayerInput;
            use terravibe_sim::projectile::ProjKind;

            let (client, conn) = in_process_pair();
            let mut s = Server::new(1);
            s.connect(Box::new(conn));
            let mut got = Vec::new();
            hello_and_load(&mut s, &client, &mut got);

            let conn = &mut s.conns[0];
            conn.inv.slots[0] = Some(ItemStack {
                id: MUSKET,
                count: 1,
                prefix: Prefix(0),
            });
            conn.inv.slots[AMMO0] = Some(ItemStack {
                id: MUSKET_BALL,
                count: 10,
                prefix: Prefix(0),
            });
            conn.inv.selected = 0;
            conn.use_state.time = 0;
            conn.use_state.anim = 0;
            conn.use_state.last_use_held = false;

            client
                .send(ClientMsg::Input {
                    seq: 1,
                    input: PlayerInput {
                        use_held: true,
                        yaw: 0.0,
                        pitch: 0.0,
                        ..Default::default()
                    },
                })
                .unwrap();
            s.tick();

            assert_eq!(
                s.conns[0].inv.slots[AMMO0].as_ref().unwrap().count,
                9,
                "firing musket must consume 1 musket ball"
            );
            assert!(
                s.projectiles.iter().any(|p| p.kind == ProjKind::Bullet),
                "musket must spawn a Bullet projectile"
            );
        }

        #[test]
        fn band_of_starpower_increases_max_mana() {
            use terravibe_sim::inventory::{Inventory, ItemStack};
            use terravibe_sim::item::{BAND_OF_STARPOWER, Prefix};
            use terravibe_sim::player::Player;

            let mut p = Player::new([0.0, 0.0, 0.0]);
            assert_eq!(p.vitals.mana_max, 20);
            assert_eq!(p.effective_mana_max(), 20);

            let mut inv = Inventory::empty();
            inv.slots[0] = Some(ItemStack {
                id: BAND_OF_STARPOWER,
                count: 1,
                prefix: Prefix(0),
            });
            assert!(p.equip(&mut inv, 0));
            assert_eq!(p.vitals.mana_max, 20, "base mana_max stays unchanged");
            assert_eq!(
                p.effective_mana_max(),
                40,
                "wearing Band of Starpower increases effective max mana by 20"
            );

            assert!(p.unequip_accessory(&mut inv, 0));
            assert_eq!(
                p.effective_mana_max(),
                20,
                "unequipping Band of Starpower reverts effective max mana"
            );
        }

        #[test]
        fn worm_food_summons_eater_of_worlds_in_corruption() {
            use terravibe_sim::enemy::EnemyKind;
            use terravibe_sim::item::WORM_FOOD;

            let (mut s, c) = setup();
            let mut got = Vec::new();
            hello_and_load(&mut s, &c, &mut got);

            // Outside corruption: using Worm Food does nothing, item is kept
            let inv = &mut s.conns[0].inv;
            inv.slots = [None; terravibe_sim::inventory::SLOTS];
            inv.slots[0] = Some(terravibe_sim::inventory::ItemStack {
                id: WORM_FOOD,
                count: 2,
                prefix: terravibe_sim::item::Prefix(0),
            });
            inv.selected = 0;

            c.send(ClientMsg::Input {
                seq: 1,
                input: PlayerInput {
                    use_held: true,
                    slot: 0,
                    ..Default::default()
                },
            })
            .unwrap();
            s.tick();

            assert_eq!(
                s.conns[0].inv.slots[0].unwrap().count,
                2,
                "item kept outside corruption"
            );
            assert!(
                !s.enemies
                    .iter()
                    .any(|e| terravibe_sim::enemy::is_eater_of_worlds(e.kind)),
                "no boss spawned outside corruption"
            );

            // Now place corrupt blocks around player
            let p_pos = s.conns[0].player.as_ref().unwrap().body.pos;
            let (px, py, pz) = (
                p_pos[0].floor() as i32,
                p_pos[1].floor() as i32,
                p_pos[2].floor() as i32,
            );
            for dx in -3..=3 {
                for dz in -3..=3 {
                    s.set_block([px + dx, py - 1, pz + dz], terravibe_sim::block::EBONSTONE);
                }
            }

            s.conns[0].use_state.time = 0;
            s.conns[0].use_state.anim = 0;
            s.conns[0].use_state.last_use_held = false;

            c.send(ClientMsg::Input {
                seq: 2,
                input: PlayerInput {
                    use_held: true,
                    slot: 0,
                    ..Default::default()
                },
            })
            .unwrap();
            s.tick();

            assert_eq!(
                s.conns[0].inv.slots[0].unwrap().count,
                1,
                "item consumed in corruption"
            );
            assert!(
                s.enemies
                    .iter()
                    .any(|e| e.kind == EnemyKind::EaterOfWorldsHead),
                "Eater of Worlds head spawned"
            );
            assert!(
                s.enemies
                    .iter()
                    .any(|e| e.kind == EnemyKind::EaterOfWorldsBody),
                "Eater of Worlds body spawned"
            );
            assert!(
                s.enemies
                    .iter()
                    .any(|e| e.kind == EnemyKind::EaterOfWorldsTail),
                "Eater of Worlds tail spawned"
            );
        }

        #[test]
        fn m7_e2e_shadow_orbs_and_eater_of_worlds() {
            use terravibe_sim::block::{
                AIR, ANVIL, DIRT, FURNACE, HELLSTONE, SHADOW_ORB, WORKBENCH,
            };
            use terravibe_sim::crafting::{
                DEMONITE_BAR_RECIPE, NIGHTMARE_PICKAXE_RECIPE, SHADOW_GREAVES_RECIPE,
                SHADOW_HELMET_RECIPE, SHADOW_SCALEMAIL_RECIPE, WOODEN_HAMMER_RECIPE,
            };
            use terravibe_sim::enemy::EnemyKind;
            use terravibe_sim::interact::min_pick;
            use terravibe_sim::inventory::AMMO0;
            use terravibe_sim::item::{
                BALL_O_HURT, DEMONITE_BAR, DEMONITE_ORE_ITEM, MUSKET, MUSKET_BALL,
                NIGHTMARE_PICKAXE, SHADOW_GREAVES, SHADOW_HELMET, SHADOW_SCALE, SHADOW_SCALEMAIL,
                VILETHORN, WOOD, WOODEN_HAMMER, item_data,
            };
            use terravibe_sim::player::PlayerInput;
            use terravibe_sim::player::consts::MAX_RUN_SPEED;
            use terravibe_sim::projectile::ProjKind;

            // 1. Creates test server world & connects client
            let (mut s, c) = setup();
            let mut got = Vec::new();
            hello_and_load(&mut s, &c, &mut got);

            let p_pos = s.conns[0].player.as_ref().unwrap().body.pos;
            let (px, py, pz) = (
                p_pos[0].floor() as i32,
                p_pos[1].floor() as i32,
                p_pos[2].floor() as i32,
            );

            // Clear and pave flat corridor for clean testing
            for dz in -35..=10 {
                for dx in -3..=3 {
                    s.set_block([px + dx, py - 1, pz + dz], DIRT);
                    for dy in 0..=4 {
                        s.set_block([px + dx, py + dy, pz + dz], AIR);
                    }
                }
            }

            s.conns[0].player.as_mut().unwrap().body.pos =
                [px as f32 + 0.5, py as f32, pz as f32 + 0.5];
            s.conns[0].player.as_mut().unwrap().body.vel = [0.0, 0.0, 0.0];
            s.conns[0].player.as_mut().unwrap().body.on_ground = true;

            // 2. Crafts Wooden Hammer with wood at workbench
            give(&mut s, WOOD, 20);
            let bench_pos = [px + 1, py, pz];
            s.set_block(bench_pos, WORKBENCH);
            let _ = craft_req(&mut s, &c, WOODEN_HAMMER_RECIPE, 1);
            assert!(
                s.conns[0].inv.count_of(WOODEN_HAMMER) >= 1,
                "Wooden Hammer must be crafted at workbench"
            );

            // 3. Breaks 1st Shadow Orb with hammer -> verifies Musket and Musket Ball drop
            let find_slot = |s: &Server, id| {
                s.conns[0]
                    .inv
                    .slots
                    .iter()
                    .position(|x| x.is_some_and(|st| st.id == id))
                    .unwrap() as u8
            };
            let hammer_slot = find_slot(&s, WOODEN_HAMMER);
            let orb_pos = [px, py + 1, pz - 2];
            s.set_block(orb_pos, SHADOW_ORB);

            let mut seq = 100;
            let mut broken_1st = false;
            for _ in 0..250 {
                seq += 1;
                c.send(ClientMsg::Input {
                    seq,
                    input: PlayerInput {
                        use_held: true,
                        slot: hammer_slot,
                        yaw: 0.0,
                        pitch: 0.0,
                        ..Default::default()
                    },
                })
                .unwrap();
                s.tick();
                if WorldQ(&s.world).block(orb_pos[0], orb_pos[1], orb_pos[2]) == AIR {
                    broken_1st = true;
                    break;
                }
            }
            assert!(
                broken_1st,
                "1st Shadow Orb must be broken with Wooden Hammer"
            );
            assert_eq!(s.shadow_orbs_broken, 1);
            assert!(
                s.drops.iter().any(|d| d.stack.id == MUSKET),
                "1st orb must drop Musket"
            );
            assert!(
                s.drops
                    .iter()
                    .any(|d| d.stack.id == MUSKET_BALL && d.stack.count == 100),
                "1st orb must drop 100 Musket Balls"
            );

            // 4. Breaks 2nd and 3rd Shadow Orbs -> verifies 3rd orb triggers Eater of Worlds boss spawn
            s.set_block(orb_pos, SHADOW_ORB);
            let mut broken_2nd = false;
            for _ in 0..250 {
                seq += 1;
                c.send(ClientMsg::Input {
                    seq,
                    input: PlayerInput {
                        use_held: true,
                        slot: hammer_slot,
                        yaw: 0.0,
                        pitch: 0.0,
                        ..Default::default()
                    },
                })
                .unwrap();
                s.tick();
                if WorldQ(&s.world).block(orb_pos[0], orb_pos[1], orb_pos[2]) == AIR {
                    broken_2nd = true;
                    break;
                }
            }
            assert!(broken_2nd, "2nd Shadow Orb must be broken");
            assert_eq!(s.shadow_orbs_broken, 2);
            assert!(
                !s.enemies
                    .iter()
                    .any(|e| terravibe_sim::enemy::is_eater_of_worlds(e.kind)),
                "2nd orb break must not spawn Eater of Worlds"
            );

            s.set_block(orb_pos, SHADOW_ORB);
            let mut broken_3rd = false;
            for _ in 0..250 {
                seq += 1;
                c.send(ClientMsg::Input {
                    seq,
                    input: PlayerInput {
                        use_held: true,
                        slot: hammer_slot,
                        yaw: 0.0,
                        pitch: 0.0,
                        ..Default::default()
                    },
                })
                .unwrap();
                s.tick();
                if WorldQ(&s.world).block(orb_pos[0], orb_pos[1], orb_pos[2]) == AIR {
                    broken_3rd = true;
                    break;
                }
            }
            assert!(broken_3rd, "3rd Shadow Orb must be broken");
            assert_eq!(s.shadow_orbs_broken, 3);
            assert!(
                s.enemies
                    .iter()
                    .any(|e| e.kind == EnemyKind::EaterOfWorldsHead),
                "3rd orb break must summon Eater of Worlds Head"
            );
            assert!(
                s.enemies
                    .iter()
                    .any(|e| e.kind == EnemyKind::EaterOfWorldsBody),
                "3rd orb break must summon Eater of Worlds Body"
            );
            assert!(
                s.enemies
                    .iter()
                    .any(|e| e.kind == EnemyKind::EaterOfWorldsTail),
                "3rd orb break must summon Eater of Worlds Tail"
            );

            // 5. Fires Musket (consuming Musket Ball ammo) and strikes with Ball O' Hurt and Vilethorn at Eater of Worlds segments
            let head_seg = s
                .enemies
                .iter_mut()
                .find(|e| e.kind == EnemyKind::EaterOfWorldsHead)
                .expect("Eater of Worlds Head present");
            head_seg.pos = [px as f32 + 0.5, py as f32 + 1.0, pz as f32 - 2.0];
            head_seg.vel = [0.0, 0.0, 0.0];
            let head_id = head_seg.id;
            let initial_hp = head_seg.hp;
            assert_eq!(initial_hp, 150);

            // 5a. Musket fire
            give(&mut s, MUSKET, 1);
            s.conns[0].inv.slots[AMMO0] = Some(terravibe_sim::inventory::ItemStack {
                id: MUSKET_BALL,
                count: 100,
                prefix: terravibe_sim::item::Prefix(0),
            });
            let musket_slot = find_slot(&s, MUSKET);
            s.conns[0].inv.selected = musket_slot as usize;
            s.conns[0].use_state.time = 0;
            s.conns[0].use_state.anim = 0;
            s.conns[0].use_state.last_use_held = false;

            seq += 1;
            c.send(ClientMsg::Input {
                seq,
                input: PlayerInput {
                    use_held: true,
                    slot: musket_slot,
                    yaw: 0.0,
                    pitch: 0.0,
                    ..Default::default()
                },
            })
            .unwrap();
            for _ in 0..5 {
                s.tick();
                if s.enemies.iter().find(|e| e.id == head_id).unwrap().hp < initial_hp {
                    break;
                }
            }

            assert_eq!(
                s.conns[0].inv.slots[AMMO0].as_ref().unwrap().count,
                99,
                "firing musket must consume 1 musket ball"
            );
            let hp_after_musket = s
                .enemies
                .iter()
                .find(|e| e.id == head_id)
                .expect("head alive")
                .hp;
            assert!(
                hp_after_musket < initial_hp,
                "head hp ({hp_after_musket}) must decrease from musket bullet (was {initial_hp})"
            );

            // 5b. Ball O' Hurt strike
            give(&mut s, BALL_O_HURT, 1);
            let flail_slot = find_slot(&s, BALL_O_HURT);
            s.conns[0].inv.selected = flail_slot as usize;
            s.conns[0].use_state.time = 0;
            s.conns[0].use_state.anim = 0;
            s.conns[0].use_state.last_use_held = false;
            s.projectiles.clear();

            let head_seg = s
                .enemies
                .iter_mut()
                .find(|e| e.id == head_id)
                .expect("head alive");
            head_seg.pos = [px as f32 + 0.5, py as f32 + 1.0, pz as f32 - 2.0];
            head_seg.vel = [0.0, 0.0, 0.0];
            head_seg.hurt_ticks = 0;

            seq += 1;
            c.send(ClientMsg::Input {
                seq,
                input: PlayerInput {
                    use_held: true,
                    slot: flail_slot,
                    yaw: 0.0,
                    pitch: 0.0,
                    ..Default::default()
                },
            })
            .unwrap();
            s.tick();
            assert!(
                s.projectiles.iter().any(|p| p.kind == ProjKind::Flail),
                "Ball O' Hurt must launch flail projectile"
            );
            for _ in 0..8 {
                s.tick();
                if s.enemies.iter().find(|e| e.id == head_id).unwrap().hp < hp_after_musket {
                    break;
                }
            }
            let hp_after_flail = s
                .enemies
                .iter()
                .find(|e| e.id == head_id)
                .expect("head alive")
                .hp;
            assert!(
                hp_after_flail < hp_after_musket,
                "head hp ({hp_after_flail}) must decrease from Ball O' Hurt strike (was {hp_after_musket})"
            );

            // 5c. Vilethorn magic strike
            give(&mut s, VILETHORN, 1);
            let thorn_slot = find_slot(&s, VILETHORN);
            s.conns[0].player.as_mut().unwrap().vitals.mana = 20;
            s.conns[0].inv.selected = thorn_slot as usize;
            s.conns[0].use_state.time = 0;
            s.conns[0].use_state.anim = 0;
            s.conns[0].use_state.last_use_held = false;
            s.projectiles.clear();

            let head_seg = s
                .enemies
                .iter_mut()
                .find(|e| e.id == head_id)
                .expect("head alive");
            head_seg.pos = [px as f32 + 0.5, py as f32 + 1.0, pz as f32 - 2.0];
            head_seg.vel = [0.0, 0.0, 0.0];
            head_seg.hurt_ticks = 0;

            seq += 1;
            c.send(ClientMsg::Input {
                seq,
                input: PlayerInput {
                    use_held: true,
                    slot: thorn_slot,
                    yaw: 0.0,
                    pitch: 0.0,
                    ..Default::default()
                },
            })
            .unwrap();
            s.tick();
            assert!(
                s.projectiles.iter().any(|p| p.kind == ProjKind::Vilethorn),
                "Vilethorn must launch thorny staff projectile"
            );
            assert_eq!(
                s.conns[0].player.as_ref().unwrap().vitals.mana,
                10,
                "Vilethorn must consume 10 mana"
            );
            for _ in 0..8 {
                s.tick();
                if s.enemies.iter().find(|e| e.id == head_id).unwrap().hp < hp_after_flail {
                    break;
                }
            }
            let hp_after_vilethorn = s
                .enemies
                .iter()
                .find(|e| e.id == head_id)
                .expect("head alive")
                .hp;
            assert!(
                hp_after_vilethorn < hp_after_flail,
                "head hp ({hp_after_vilethorn}) must decrease from Vilethorn strike (was {hp_after_flail})"
            );

            // 6. Verifies body segment dies and worm splits into two active worms
            let initial_heads = s
                .enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::EaterOfWorldsHead && e.hp > 0)
                .count();
            assert_eq!(initial_heads, 1, "initially exactly 1 head segment");

            // Kill intermediate body segment (segment index 5)
            let mid_body = s
                .enemies
                .iter_mut()
                .find(|e| e.kind == EnemyKind::EaterOfWorldsBody && e.ai[0] != 0 && e.ai[1] != 0)
                .expect("intermediate body segment with both parent and child");
            mid_body.hp = 0;

            s.tick();

            let split_heads = s
                .enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::EaterOfWorldsHead && e.hp > 0)
                .count();
            assert_eq!(
                split_heads, 2,
                "killing intermediate body segment must split worm into two active worms (2 heads)"
            );

            // 7. Kills segments -> verifies Demonite Ore and Shadow Scale drops
            assert!(
                s.drops.iter().any(|d| d.stack.id == DEMONITE_ORE_ITEM),
                "dying segment must drop Demonite Ore"
            );
            assert!(
                s.drops.iter().any(|d| d.stack.id == SHADOW_SCALE),
                "dying segment must drop Shadow Scale"
            );

            // 8. Smelts Demonite Bars, crafts Nightmare Pickaxe and full Shadow Armor
            give(&mut s, DEMONITE_ORE_ITEM, 300);
            give(&mut s, SHADOW_SCALE, 60);

            s.set_block([px + 1, py, pz + 1], FURNACE);
            s.set_block([px - 1, py, pz], ANVIL);

            let _ = craft_req(&mut s, &c, DEMONITE_BAR_RECIPE, 80);
            assert!(
                s.conns[0].inv.count_of(DEMONITE_BAR) >= 80,
                "must smelt Demonite Bars at furnace"
            );

            let _ = craft_req(&mut s, &c, NIGHTMARE_PICKAXE_RECIPE, 1);
            assert_eq!(s.conns[0].inv.count_of(NIGHTMARE_PICKAXE), 1);

            let _ = craft_req(&mut s, &c, SHADOW_HELMET_RECIPE, 1);
            let _ = craft_req(&mut s, &c, SHADOW_SCALEMAIL_RECIPE, 1);
            let _ = craft_req(&mut s, &c, SHADOW_GREAVES_RECIPE, 1);
            assert_eq!(s.conns[0].inv.count_of(SHADOW_HELMET), 1);
            assert_eq!(s.conns[0].inv.count_of(SHADOW_SCALEMAIL), 1);
            assert_eq!(s.conns[0].inv.count_of(SHADOW_GREAVES), 1);

            // 9. Equips Shadow Armor -> verifies +15% run speed bonus active
            let helm_slot = find_slot(&s, SHADOW_HELMET);
            c.send(ClientMsg::Equip { from: helm_slot }).unwrap();
            s.tick();
            let chest_slot = find_slot(&s, SHADOW_SCALEMAIL);
            c.send(ClientMsg::Equip { from: chest_slot }).unwrap();
            s.tick();
            let legs_slot = find_slot(&s, SHADOW_GREAVES);
            c.send(ClientMsg::Equip { from: legs_slot }).unwrap();
            s.tick();

            let player = s.conns[0].player.as_ref().unwrap();
            assert!(
                player.has_shadow_set(),
                "player must have active shadow armor set bonus"
            );
            assert_eq!(player.defense(), 19);

            // Run along paved corridor
            let run_input = PlayerInput {
                forward: 1.0,
                yaw: 0.0,
                ..Default::default()
            };
            for _ in 0..20 {
                seq += 1;
                c.send(ClientMsg::Input {
                    seq,
                    input: run_input.clone(),
                })
                .unwrap();
                s.tick();
            }
            let speed = {
                let vel = s.conns[0].player.as_ref().unwrap().body.vel;
                vel[0].hypot(vel[2])
            };
            let expected_speed = MAX_RUN_SPEED * 1.15;
            assert!(
                (speed - expected_speed).abs() < 1e-3,
                "speed ({speed}) must equal boosted speed ({expected_speed}) with Shadow Armor set bonus"
            );

            // 10. Verifies Nightmare Pickaxe mines hellstone tier
            assert_eq!(min_pick(HELLSTONE), 65);
            let pick_data = item_data(NIGHTMARE_PICKAXE);
            assert_eq!(pick_data.pick, 65);
            assert!(pick_data.pick >= min_pick(HELLSTONE));

            let hellstone_pos = [px, py + 1, pz - 2];
            s.set_block(hellstone_pos, HELLSTONE);

            let pick_slot = find_slot(&s, NIGHTMARE_PICKAXE);
            s.conns[0].inv.selected = pick_slot as usize;
            s.conns[0].player.as_mut().unwrap().body.pos =
                [px as f32 + 0.5, py as f32, pz as f32 + 0.5];
            s.conns[0].player.as_mut().unwrap().body.vel = [0.0, 0.0, 0.0];
            s.conns[0].use_state.time = 0;
            s.conns[0].use_state.anim = 0;
            s.conns[0].use_state.last_use_held = false;

            let mut hellstone_broken = false;
            for _ in 0..250 {
                seq += 1;
                c.send(ClientMsg::Input {
                    seq,
                    input: PlayerInput {
                        use_held: true,
                        slot: pick_slot,
                        yaw: 0.0,
                        pitch: 0.0,
                        ..Default::default()
                    },
                })
                .unwrap();
                s.tick();
                if WorldQ(&s.world).block(hellstone_pos[0], hellstone_pos[1], hellstone_pos[2])
                    == AIR
                {
                    hellstone_broken = true;
                    break;
                }
            }
            assert!(
                hellstone_broken,
                "Nightmare Pickaxe must break Hellstone tier block"
            );
        }

        #[test]
        fn m8_e2e_dungeon_and_skeletron() {
            use terravibe_sim::block::{AIR, CHEST};
            use terravibe_sim::enemy::EnemyKind;
            use terravibe_sim::inventory::AMMO0;
            use terravibe_sim::item::{
                BALL_O_HURT, BONE, COBALT_SHIELD, GOLDEN_KEY, MURAMASA, MUSKET, MUSKET_BALL,
            };
            use terravibe_sim::player::PlayerInput;
            use terravibe_sim::projectile::ProjKind;

            // 1. Creates fixture world with dungeon generated and connects client
            let (client, conn) = in_process_pair();
            let mut s = Server::with_cfg(
                1,
                ServerCfg {
                    natural_spawns: false,
                    ..ServerCfg::default()
                },
            );
            s.connect(Box::new(conn));
            let mut got = Vec::new();
            hello_and_load(&mut s, &client, &mut got);

            let entrance = terravibe_sim::worldgen::dungeon_entrance_pos(s.seed, s.bounds.size());
            let ex = entrance[0] as i32;
            let ey = entrance[1] as i32 - 1;
            let ez = entrance[2] as i32;
            let side = terravibe_sim::worldgen::dungeon::dungeon_side(s.seed, s.bounds.size());

            // 2. Player approaches dungeon entrance, verifies Old Man is present
            // Ensure chunks around dungeon entrance are generated and installed
            let center_cp = ChunkPos::of_block(ex, ey, ez);
            for cdx in -2..=2 {
                for cdy in -2..=2 {
                    for cdz in -2..=2 {
                        let cp = ChunkPos {
                            x: center_cp.x + cdx,
                            y: center_cp.y + cdy,
                            z: center_cp.z + cdz,
                        };
                        if s.bounds.chunk_inside(cp) && !s.world.contains_key(&cp) {
                            let chunk =
                                terravibe_sim::generate_chunk_sized(s.seed, cp, s.bounds.size());
                            s.install_chunk(cp, std::sync::Arc::new(chunk));
                        }
                    }
                }
            }

            s.conns[0].player.as_mut().unwrap().body.pos = entrance;
            s.conns[0].player.as_mut().unwrap().body.vel = [0.0, 0.0, 0.0];
            s.conns[0].interest.centre = Some(entrance);
            s.ensure_residents();

            let old_man = s
                .enemies
                .iter()
                .find(|e| e.kind == EnemyKind::OldMan && e.hp > 0);
            assert!(
                old_man.is_some(),
                "Old Man NPC must be present at dungeon entrance"
            );
            let old_man_id = old_man.unwrap().id;

            // 3. Player enters subterranean dungeon before defeating Skeletron -> Dungeon Guardian spawns to defend
            assert!(!s.downed_skeletron);
            let dungeon_subterranean = [(ex - side * 5) as f32, (ey - 5) as f32, ez as f32];
            s.conns[0].player.as_mut().unwrap().body.pos = dungeon_subterranean;
            s.tick();
            assert!(
                s.enemies
                    .iter()
                    .any(|e| e.kind == EnemyKind::DungeonGuardian && e.hp > 0),
                "Dungeon Guardian must spawn to defend pre-skeletron"
            );

            // Clear Dungeon Guardian and return player to surface entrance
            s.enemies.retain(|e| e.kind != EnemyKind::DungeonGuardian);
            s.conns[0].player.as_mut().unwrap().body.pos = entrance;
            s.conns[0].player.as_mut().unwrap().body.vel = [0.0, 0.0, 0.0];

            // 4. Interacts with Old Man at night -> summons Skeletron (Head + 2 Hands)
            s.clock.tick = 60_000; // night time
            assert!(!s.clock.is_day(), "Must be night time to summon Skeletron");

            client.send(ClientMsg::TalkNpc { id: old_man_id }).unwrap();
            s.tick();

            assert!(
                !s.enemies.iter().any(|e| e.id == old_man_id && e.hp > 0),
                "Old Man should be despawned or sacrificed on Skeletron summon"
            );
            let head_enemy = s
                .enemies
                .iter()
                .find(|e| e.kind == EnemyKind::SkeletronHead && e.hp > 0);
            assert!(
                head_enemy.is_some(),
                "SkeletronHead must be summoned at dungeon entrance"
            );
            let head_id = head_enemy.unwrap().id;
            let hand_count = s
                .enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::SkeletronHand && e.hp > 0)
                .count();
            assert_eq!(hand_count, 2, "Skeletron must have 2 hands summoned");

            // 5. Attacks hands and head with weapons (e.g. Musket, Ball O' Hurt) -> defeats SkeletronHead
            let find_slot = |s: &Server, id| {
                s.conns[0]
                    .inv
                    .slots
                    .iter()
                    .position(|x| x.is_some_and(|st| st.id == id))
                    .unwrap() as u8
            };

            // 5a. Musket attack against a hand
            give(&mut s, MUSKET, 1);
            s.conns[0].inv.slots[AMMO0] = Some(terravibe_sim::inventory::ItemStack {
                id: MUSKET_BALL,
                count: 100,
                prefix: terravibe_sim::item::Prefix(0),
            });
            let musket_slot = find_slot(&s, MUSKET);
            s.conns[0].inv.selected = musket_slot as usize;
            s.conns[0].use_state.time = 0;
            s.conns[0].use_state.anim = 0;
            s.conns[0].use_state.last_use_held = false;

            let p_pos = s.conns[0].player.as_ref().unwrap().body.pos;
            // Clear air corridor so attacks have line of sight
            for dx in -3..=3 {
                for dy in 0..=4 {
                    for dz in -5..=2 {
                        s.set_block(
                            [
                                p_pos[0] as i32 + dx,
                                p_pos[1] as i32 + dy,
                                p_pos[2] as i32 + dz,
                            ],
                            AIR,
                        );
                    }
                }
            }

            let initial_head_hp = s.enemies.iter().find(|e| e.id == head_id).unwrap().hp;

            let mut seq = 500;
            seq += 1;
            client
                .send(ClientMsg::Input {
                    seq,
                    input: PlayerInput {
                        use_held: true,
                        slot: musket_slot,
                        yaw: 0.0,
                        pitch: 0.0,
                        ..Default::default()
                    },
                })
                .unwrap();

            for _ in 0..5 {
                s.tick();
                if s.enemies
                    .iter()
                    .find(|e| e.id == head_id)
                    .map_or(false, |e| e.hp < initial_head_hp)
                {
                    break;
                }
            }
            assert_eq!(
                s.conns[0].inv.slots[AMMO0].as_ref().unwrap().count,
                99,
                "Firing Musket must consume 1 Musket Ball"
            );
            let head_hp_after_shot = s.enemies.iter().find(|e| e.id == head_id).unwrap().hp;
            assert!(
                head_hp_after_shot < initial_head_hp,
                "SkeletronHead HP must decrease from Musket shot"
            );

            // 5b. Ball O' Hurt attack against SkeletronHand
            give(&mut s, BALL_O_HURT, 1);
            let flail_slot = find_slot(&s, BALL_O_HURT);
            s.conns[0].inv.selected = flail_slot as usize;
            s.conns[0].use_state.time = 0;
            s.conns[0].use_state.anim = 0;
            s.conns[0].use_state.last_use_held = false;
            s.projectiles.clear();

            // Position head away so flail targets hand
            let head = s.enemies.iter_mut().find(|e| e.id == head_id).unwrap();
            head.pos = [p_pos[0] + 15.0, p_pos[1] + 10.0, p_pos[2]];

            let hand = s
                .enemies
                .iter_mut()
                .find(|e| e.kind == EnemyKind::SkeletronHand)
                .unwrap();
            hand.pos = [p_pos[0], p_pos[1] + 1.0, p_pos[2] - 2.0];
            hand.vel = [0.0, 0.0, 0.0];
            hand.hurt_ticks = 0;
            let hand_id = hand.id;
            let initial_hand_hp = hand.hp;

            seq += 1;
            client
                .send(ClientMsg::Input {
                    seq,
                    input: PlayerInput {
                        use_held: true,
                        slot: flail_slot,
                        yaw: 0.0,
                        pitch: 0.0,
                        ..Default::default()
                    },
                })
                .unwrap();
            s.tick();
            assert!(
                s.projectiles.iter().any(|p| p.kind == ProjKind::Flail),
                "Ball O' Hurt must launch flail projectile"
            );
            for _ in 0..8 {
                s.tick();
                if s.enemies
                    .iter()
                    .find(|e| e.id == hand_id)
                    .map_or(false, |e| e.hp < initial_hand_hp)
                {
                    break;
                }
            }
            assert!(
                s.enemies.iter().find(|e| e.id == hand_id).unwrap().hp < initial_hand_hp,
                "SkeletronHand HP must decrease from Ball O' Hurt strike"
            );

            // 5c. Defeats SkeletronHead
            let head = s.enemies.iter_mut().find(|e| e.id == head_id).unwrap();
            head.hp = 0;
            s.tick_enemies();

            // 6. Verifies remaining hands despawn and `downed_skeletron` flag becomes `true`
            assert!(
                s.downed_skeletron,
                "downed_skeletron flag must become true upon head defeat"
            );
            assert_eq!(
                s.enemies
                    .iter()
                    .filter(|e| e.kind == EnemyKind::SkeletronHand)
                    .count(),
                0,
                "All Skeletron hands must despawn upon head defeat"
            );

            // 7. Player re-enters subterranean dungeon safely (no Dungeon Guardian)
            s.conns[0].player.as_mut().unwrap().body.pos = dungeon_subterranean;
            s.tick();
            assert!(
                !s.enemies
                    .iter()
                    .any(|e| e.kind == EnemyKind::DungeonGuardian && e.hp > 0),
                "Dungeon Guardian must not spawn after Skeletron is downed"
            );

            // 8. Encounters and defeats Angry Bones -> drops Golden Key and Bones
            let mut dropped_bone = false;
            let mut dropped_key = false;
            for _ in 0..300 {
                s.add_enemy(
                    EnemyKind::AngryBones,
                    [
                        dungeon_subterranean[0] + 1.0,
                        dungeon_subterranean[1],
                        dungeon_subterranean[2],
                    ],
                );
                let bones = s
                    .enemies
                    .iter_mut()
                    .find(|e| e.kind == EnemyKind::AngryBones && e.hp > 0)
                    .unwrap();
                bones.hp = 0;
                s.tick_enemies();
                if s.drops.iter().any(|d| d.stack.id == BONE) {
                    dropped_bone = true;
                }
                if s.drops.iter().any(|d| d.stack.id == GOLDEN_KEY) {
                    dropped_key = true;
                    break;
                }
            }
            assert!(dropped_bone, "Angry Bones must drop Bone");
            assert!(dropped_key, "Angry Bones must drop Golden Key");

            // 9. Unlocks locked golden dungeon chest using Golden Key (key consumed) -> loots Muramasa, Cobalt Shield
            let chest_pos = [ex, ey - 2, ez];
            let cp = ChunkPos::of_block(chest_pos[0], chest_pos[1], chest_pos[2]);
            s.world.entry(cp).or_insert_with(|| {
                std::sync::Arc::new(terravibe_sim::generate_chunk_sized(
                    s.seed,
                    cp,
                    s.bounds.size(),
                ))
            });
            s.set_block(chest_pos, CHEST);
            s.locked_chests.insert(chest_pos);
            s.chests.insert(
                chest_pos,
                terravibe_sim::chest::ChestInv::from_stacks(&[
                    terravibe_sim::inventory::ItemStack {
                        id: MURAMASA,
                        count: 1,
                        prefix: terravibe_sim::item::Prefix(0),
                    },
                    terravibe_sim::inventory::ItemStack {
                        id: COBALT_SHIELD,
                        count: 1,
                        prefix: terravibe_sim::item::Prefix(0),
                    },
                ]),
            );

            // Give player the Golden Key
            give(&mut s, GOLDEN_KEY, 1);
            assert_eq!(s.conns[0].inv.count_of(GOLDEN_KEY), 1);
            s.conns[0].player.as_mut().unwrap().body.pos = [
                chest_pos[0] as f32 + 0.5,
                chest_pos[1] as f32 + 1.0,
                chest_pos[2] as f32 + 0.5,
            ];

            client
                .send(ClientMsg::OpenChest { pos: chest_pos })
                .unwrap();
            s.tick();

            assert_eq!(
                s.conns[0].inv.count_of(GOLDEN_KEY),
                0,
                "Golden Key must be consumed to unlock locked dungeon chest"
            );
            assert!(
                !s.locked_chests.contains(&chest_pos),
                "Chest must be unlocked"
            );
            assert_eq!(s.conns[0].open_chest, Some(chest_pos));

            // Loot Muramasa (slot 0) and Cobalt Shield (slot 1) from chest into inventory
            client
                .send(ClientMsg::ChestMove {
                    to_chest: false,
                    slot: 0,
                })
                .unwrap();
            s.tick();
            client
                .send(ClientMsg::ChestMove {
                    to_chest: false,
                    slot: 1,
                })
                .unwrap();
            s.tick();

            assert_eq!(
                s.conns[0].inv.count_of(MURAMASA),
                1,
                "Player must have looted Muramasa"
            );
            assert_eq!(
                s.conns[0].inv.count_of(COBALT_SHIELD),
                1,
                "Player must have looted Cobalt Shield"
            );

            // 10. Equips Cobalt Shield -> verifies knockback immunity active
            let shield_slot = find_slot(&s, COBALT_SHIELD);
            client.send(ClientMsg::Equip { from: shield_slot }).unwrap();
            s.tick();

            let player = s.conns[0].player.as_ref().unwrap();
            assert!(
                player.no_knockback(),
                "Player must have knockback immunity active with Cobalt Shield"
            );
            assert!(player.body.no_knockback);
            assert_eq!(
                player.damage_knockback(10.0),
                0.0,
                "Knockback received from damage must be 0"
            );

            let player_mut = s.conns[0].player.as_mut().unwrap();
            player_mut.body.vel = [0.0, 0.0, 0.0];
            player_mut.apply_knockback(10.0, [1.0, 0.0, 0.0]);
            assert_eq!(
                player_mut.body.vel[0], 0.0,
                "Velocity knockback must remain 0 with Cobalt Shield equipped"
            );
        }

        #[test]
        fn fire_imp_fireball_hurts_player() {
            let (mut s, client) = setup();
            let mut got = Vec::new();
            hello_and_load(&mut s, &client, &mut got);
            s.cfg.natural_spawns = false;

            let p_pos = [10.0, 10.0, 10.0];
            s.conns[0].player.as_mut().unwrap().body.pos = p_pos;
            s.conns[0].player.as_mut().unwrap().vitals.life = 100;
            s.conns[0].player.as_mut().unwrap().i_frames = 0;

            // Spawn Fire Imp in range
            s.add_enemy(EnemyKind::FireImp, [15.0, 10.0, 10.0]);

            // Tick until a projectile is cast by Fire Imp
            let mut cast = false;
            for _ in 0..200 {
                s.tick();
                if s.projectiles
                    .iter()
                    .any(|p| p.kind == ProjKind::Fireball && p.owner == u32::MAX)
                {
                    cast = true;
                    break;
                }
            }
            assert!(cast, "Fire Imp must cast Fireball");

            // Place player directly on the fireball path / position
            let fb = s
                .projectiles
                .iter()
                .find(|p| p.kind == ProjKind::Fireball && p.owner == u32::MAX)
                .unwrap();
            let fb_pos = fb.pos;

            s.conns[0].player.as_mut().unwrap().body.pos = fb_pos;
            s.conns[0].player.as_mut().unwrap().i_frames = 0;
            let initial_life = s.conns[0].player.as_ref().unwrap().vitals.life;

            s.tick();

            let new_life = s.conns[0].player.as_ref().unwrap().vitals.life;
            assert!(new_life < initial_life, "Fireball must hurt player");
            assert!(
                !s.projectiles.iter().any(|p| p.kind == ProjKind::Fireball),
                "Fireball projectile must be removed on hit"
            );
        }

        #[test]
        fn fire_imp_cast_retries_when_projectile_cap_is_full() {
            let (mut s, client) = setup();
            let mut got = Vec::new();
            hello_and_load(&mut s, &client, &mut got);
            s.cfg.natural_spawns = false;

            let sp = s.spawn;
            s.conns[0].player.as_mut().unwrap().body.pos = sp;
            s.add_enemy(EnemyKind::FireImp, [sp[0] + 5.0, sp[1], sp[2]]);

            // Fill the projectile cap, then arm the imp's cast flag by hand.
            for _ in 0..MAX_PROJECTILES {
                let id = s.next_proj_id;
                s.next_proj_id += 1;
                s.projectiles.push(Projectile {
                    id,
                    pos: [sp[0], sp[1] + 20.0, sp[2]],
                    vel: [0.0; 3],
                    kind: ProjKind::Fireball,
                    owner: u32::MAX,
                    ttl: 1000,
                    damage: 30,
                    knock_back: 1.0,
                    crit: 0,
                    ai0: 0,
                    pierced: 0,
                    origin: [sp[0], sp[1] + 20.0, sp[2]],
                    armor_penetration: 0,
                });
            }
            s.enemies
                .iter_mut()
                .find(|e| e.kind == EnemyKind::FireImp)
                .unwrap()
                .ai[1] = 1;

            // A full cap must not consume the pending cast.
            s.tick();
            let imp = s
                .enemies
                .iter()
                .find(|e| e.kind == EnemyKind::FireImp)
                .unwrap();
            assert_eq!(imp.ai[1], 1, "cast must stay pending while the cap is full");

            // Freeing one slot lets the imp fire and clears the flag.
            s.projectiles.pop();
            s.tick();
            assert_eq!(s.projectiles.len(), MAX_PROJECTILES);
            let imp = s
                .enemies
                .iter()
                .find(|e| e.kind == EnemyKind::FireImp)
                .unwrap();
            assert_eq!(
                imp.ai[1], 0,
                "cast flag clears once the fireball is created"
            );
        }

        #[test]
        fn bone_serpent_spawn_expands_to_chain() {
            let (mut s, _client) = setup();
            s.cfg.natural_spawns = false;
            let spawn_pos = [20.0, 10.0, 20.0];

            s.spawn_bone_serpent(spawn_pos);

            assert_eq!(
                s.enemies.len(),
                6,
                "Bone serpent spawn helper must create 6 segments"
            );
            assert_eq!(s.enemies[0].kind, EnemyKind::BoneSerpentHead);
            assert_eq!(s.enemies[1].kind, EnemyKind::BoneSerpentBody);
            assert_eq!(s.enemies[2].kind, EnemyKind::BoneSerpentBody);
            assert_eq!(s.enemies[3].kind, EnemyKind::BoneSerpentBody);
            assert_eq!(s.enemies[4].kind, EnemyKind::BoneSerpentBody);
            assert_eq!(s.enemies[5].kind, EnemyKind::BoneSerpentTail);
        }

        #[test]
        fn m9_e2e_underworld() {
            use terravibe_sim::Liquid;
            use terravibe_sim::block::{AIR, ASH, HELLSTONE, HELLSTONE_BRICK, OBSIDIAN_BRICK};
            use terravibe_sim::chunk::BlockQuery;
            use terravibe_sim::drop::loot_for;
            use terravibe_sim::enemy::EnemyKind;
            use terravibe_sim::item::{
                GUIDE_VOODOO_DOLL, LAVA_CHARM, NIGHTMARE_PICKAXE, OBSIDIAN_SKIN_POTION,
                OBSIDIAN_SKULL, Rng, SHADOW_KEY,
            };
            use terravibe_sim::liquid_cell::LiquidKind;
            use terravibe_sim::player::consts::{
                HOT_BURN_TICKS, LAVA_CHARM_TICKS, LAVA_DAMAGE, OBSIDIAN_SKIN_TICKS,
            };
            use terravibe_sim::player::{Player, PlayerEvent, PlayerInput, tick_player};
            use terravibe_sim::spawn::{SPAWN_RATE, Spawner, UNDERWORLD_POOL, tick_spawner};
            use terravibe_sim::worldgen::underworld::UNDERWORLD_TOP_Y;

            // 1. Fixture player descends to y <= UNDERWORLD_TOP_Y (e.g. generate Underworld chunks y=0..=24).
            // Verify loaded chunks contain ASH, HELLSTONE, LAVA, and at least one ruined house (OBSIDIAN_BRICK or HELLSTONE_BRICK).
            let (client, conn) = in_process_pair();
            let mut s = Server::with_cfg(
                42,
                ServerCfg {
                    natural_spawns: false,
                    ..ServerCfg::default()
                },
            );
            s.connect(Box::new(conn));
            let mut got = Vec::new();
            hello_and_load(&mut s, &client, &mut got);

            let mut ash_count = 0;
            let mut hellstone_count = 0;
            let mut lava_count = 0;
            let mut brick_count = 0;

            for cx in 0..12 {
                for cy in 0..=1 {
                    for cz in 0..12 {
                        let cp = ChunkPos {
                            x: cx,
                            y: cy,
                            z: cz,
                        };
                        let chunk = terravibe_sim::worldgen::generate_chunk_sized(
                            s.seed,
                            cp,
                            s.bounds.size(),
                        );
                        for lx in 0..CHUNK_SIZE {
                            for ly in 0..CHUNK_SIZE {
                                for lz in 0..CHUNK_SIZE {
                                    let b = chunk.get(lx as usize, ly as usize, lz as usize);
                                    if b == ASH {
                                        ash_count += 1;
                                    } else if b == HELLSTONE {
                                        hellstone_count += 1;
                                    } else if b == OBSIDIAN_BRICK || b == HELLSTONE_BRICK {
                                        brick_count += 1;
                                    }
                                    if let Some(liq) =
                                        chunk.liquid(lx as usize, ly as usize, lz as usize)
                                    {
                                        if liq.kind == LiquidKind::Lava {
                                            lava_count += 1;
                                        }
                                    }
                                }
                            }
                        }
                        s.install_chunk(cp, std::sync::Arc::new(chunk));
                    }
                }
            }

            let underworld_player_pos = [16.0, 20.0, 16.0];
            {
                let pl = s.conns[0].player.as_mut().unwrap();
                pl.body.pos = underworld_player_pos;
                pl.body.vel = [0.0, 0.0, 0.0];
                pl.body.fall_peak_y = underworld_player_pos[1];
                pl.body.on_ground = true;
            }
            assert!(
                (s.conns[0].player.as_ref().unwrap().body.pos[1] as i32) <= UNDERWORLD_TOP_Y,
                "Fixture player must descend to y <= UNDERWORLD_TOP_Y"
            );

            assert!(ash_count > 0, "Underworld chunks must contain Ash blocks");
            assert!(
                hellstone_count > 0,
                "Underworld chunks must contain Hellstone blocks"
            );
            assert!(lava_count > 0, "Underworld chunks must contain Lava");
            assert!(
                brick_count > 0,
                "Underworld chunks must contain ruined house bricks (Obsidian Brick or Hellstone Brick)"
            );

            // 2. Natural Underworld spawner produces only Underworld kinds, and all 6 kinds can be spawned and ticked by AI.
            let mut spawn_rng = Rng::new(123);
            let mut seen_kinds = std::collections::HashSet::new();
            for _ in 0..300 {
                let mut sp = Spawner {
                    ticks: SPAWN_RATE - 1,
                };
                let Some((kind, pos)) = tick_spawner(
                    &SpawnQ {
                        world: WorldQ(&s.world),
                        seed: s.seed,
                        size: s.cfg.size,
                    },
                    &s.clock,
                    &[underworld_player_pos],
                    0,
                    0,
                    &mut sp,
                    &mut spawn_rng,
                ) else {
                    continue;
                };
                assert!(
                    UNDERWORLD_POOL.contains(&kind),
                    "Natural spawner at underworld depth must produce only underworld kinds, got {:?}",
                    kind
                );
                assert!(
                    pos[1] <= UNDERWORLD_TOP_Y as f32,
                    "Natural spawn y {} must be in Underworld band",
                    pos[1]
                );
                seen_kinds.insert(kind);
            }
            assert_eq!(
                seen_kinds.len(),
                6,
                "All 6 Underworld enemy kinds must appear in Underworld natural spawner: {:?}",
                seen_kinds
            );

            // Verify all 6 Underworld enemy kinds can be spawned and ticked by server AI
            let distinct_kinds = [
                EnemyKind::LavaSlime,
                EnemyKind::Hellbat,
                EnemyKind::FireImp,
                EnemyKind::Demon,
                EnemyKind::VoodooDemon,
                EnemyKind::BoneSerpentHead,
            ];
            s.enemies.clear();
            for &kind in &distinct_kinds {
                s.add_enemy(
                    kind,
                    [
                        underworld_player_pos[0] + 5.0,
                        underworld_player_pos[1],
                        underworld_player_pos[2],
                    ],
                );
            }
            assert_eq!(s.enemies.len(), 6);
            for _ in 0..10 {
                s.tick_enemies();
            }
            assert_eq!(
                s.enemies.len(),
                6,
                "All 6 Underworld enemies must remain active after AI ticks"
            );

            // 3. 100 Voodoo Demon kills produce 100 Guide Voodoo Dolls (loot_for(EnemyKind::VoodooDemon, ...))
            let mut loot_rng = Rng::new(777);
            let mut voodoo_doll_count = 0;
            for _ in 0..100 {
                let drops = loot_for(EnemyKind::VoodooDemon, &mut loot_rng);
                let dolls: u32 = drops
                    .iter()
                    .filter(|st| st.id == GUIDE_VOODOO_DOLL)
                    .map(|st| st.count)
                    .sum();
                assert_eq!(
                    dolls, 1,
                    "Each Voodoo Demon kill must drop exactly 1 Guide Voodoo Doll"
                );
                voodoo_doll_count += dolls;
            }
            assert_eq!(
                voodoo_doll_count, 100,
                "100 Voodoo Demon kills must produce 100 Guide Voodoo Dolls"
            );

            // 4. Bone Serpent spawns as 6-part chain (spawn_bone_serpent) and follows player across server ticks
            s.enemies.clear();
            let initial_serpent_pos = [
                underworld_player_pos[0] + 15.0,
                underworld_player_pos[1],
                underworld_player_pos[2],
            ];
            s.spawn_bone_serpent(initial_serpent_pos);
            assert_eq!(
                s.enemies.len(),
                6,
                "spawn_bone_serpent must create 6-part chain"
            );
            assert_eq!(s.enemies[0].kind, EnemyKind::BoneSerpentHead);
            assert_eq!(s.enemies[1].kind, EnemyKind::BoneSerpentBody);
            assert_eq!(s.enemies[2].kind, EnemyKind::BoneSerpentBody);
            assert_eq!(s.enemies[3].kind, EnemyKind::BoneSerpentBody);
            assert_eq!(s.enemies[4].kind, EnemyKind::BoneSerpentBody);
            assert_eq!(s.enemies[5].kind, EnemyKind::BoneSerpentTail);

            let initial_head_pos = s.enemies[0].pos;
            let dist_before = {
                let dx = initial_head_pos[0] - underworld_player_pos[0];
                let dy = initial_head_pos[1] - underworld_player_pos[1];
                let dz = initial_head_pos[2] - underworld_player_pos[2];
                (dx * dx + dy * dy + dz * dz).sqrt()
            };

            for _ in 0..20 {
                s.tick();
            }

            let head_after = s.enemies[0].pos;
            let dist_after = {
                let dx = head_after[0] - underworld_player_pos[0];
                let dy = head_after[1] - underworld_player_pos[1];
                let dz = head_after[2] - underworld_player_pos[2];
                (dx * dx + dy * dy + dz * dz).sqrt()
            };
            assert!(
                dist_after < dist_before,
                "Bone Serpent head must follow player across server ticks (dist before: {dist_before}, dist after: {dist_after})"
            );

            // 5. Lava Charm delays first lava hit by 420 ticks; Obsidian Skin Potion grants 21,600 ticks of immunity
            // 5a. Lava Charm delays first lava hit by 420 ticks
            struct LavaEnv;
            impl BlockQuery for LavaEnv {
                fn block(&self, _x: i32, y: i32, _z: i32) -> terravibe_sim::block::BlockId {
                    if y < 0 {
                        terravibe_sim::block::STONE
                    } else {
                        AIR
                    }
                }
                fn liquid(&self, _x: i32, y: i32, _z: i32) -> Option<Liquid> {
                    if (0..=5).contains(&y) {
                        Some(Liquid::lava(255))
                    } else {
                        None
                    }
                }
            }

            let mut test_player = Player::new([0.5, 0.0, 0.5]);
            test_player.vitals.life = 200;
            test_player.vitals.life_max = 200;
            test_player.accessories[0] = Some(LAVA_CHARM);
            test_player.vitals.lava_time = LAVA_CHARM_TICKS;

            let mut lava_hurts = 0;
            for _ in 1..=419 {
                let evs = tick_player(&LavaEnv, &mut test_player, &PlayerInput::default());
                for ev in evs {
                    if let PlayerEvent::Hurt { amount } = ev {
                        if amount == LAVA_DAMAGE {
                            lava_hurts += 1;
                        }
                    }
                }
            }
            assert_eq!(
                lava_hurts, 0,
                "Lava Charm must delay lava damage for 419 ticks"
            );
            let evs = tick_player(&LavaEnv, &mut test_player, &PlayerInput::default());
            let hit_on_420 = evs
                .iter()
                .any(|e| matches!(e, PlayerEvent::Hurt { amount } if *amount == LAVA_DAMAGE));
            assert!(hit_on_420, "First lava hit must occur on tick 420");

            // 5b. Obsidian Skin Potion grants 21,600 ticks of immunity
            let mut potion_player = Player::new([0.5, 0.0, 0.5]);
            let mut p_inv = terravibe_sim::inventory::Inventory::empty();
            p_inv.slots[0] = Some(terravibe_sim::inventory::ItemStack {
                id: OBSIDIAN_SKIN_POTION,
                count: 1,
                prefix: terravibe_sim::item::Prefix(0),
            });
            assert!(terravibe_sim::interact::apply_consumable(
                OBSIDIAN_SKIN_POTION,
                &mut potion_player.vitals,
                &mut p_inv
            ));
            assert_eq!(
                potion_player.vitals.obsidian_skin_ticks, OBSIDIAN_SKIN_TICKS,
                "Obsidian Skin Potion must grant 21,600 ticks of immunity"
            );
            assert_eq!(OBSIDIAN_SKIN_TICKS, 21_600);
            let evs = tick_player(&LavaEnv, &mut potion_player, &PlayerInput::default());
            assert!(
                !evs.iter().any(|e| matches!(e, PlayerEvent::Hurt { .. })),
                "Obsidian Skin Potion grants complete lava immunity"
            );

            // 6. Obsidian Skull prevents Hellstone burn (fire_walk)
            struct HotEnv;
            impl BlockQuery for HotEnv {
                fn block(&self, _x: i32, y: i32, _z: i32) -> terravibe_sim::block::BlockId {
                    if y <= 0 { HELLSTONE } else { AIR }
                }
            }

            let mut bare_player = Player::new([0.5, 1.0, 0.5]);
            assert!(!bare_player.fire_walk());
            tick_player(&HotEnv, &mut bare_player, &PlayerInput::default());
            assert_eq!(
                bare_player.vitals.burning_ticks,
                HOT_BURN_TICKS - 1,
                "Stepping on Hellstone without skull inflicts burn"
            );

            let mut skull_player = Player::new([0.5, 1.0, 0.5]);
            skull_player.accessories[0] = Some(OBSIDIAN_SKULL);
            skull_player.sync_accessories();
            assert!(skull_player.fire_walk(), "Obsidian Skull grants fire_walk");
            tick_player(&HotEnv, &mut skull_player, &PlayerInput::default());
            assert_eq!(
                skull_player.vitals.burning_ticks, 0,
                "Obsidian Skull prevents Hellstone burn"
            );

            // 7. Shadow Key opens an Underworld chest without being consumed
            let chest_pos = [20, 10, 20];
            s.install_chunk(ChunkPos { x: 0, y: 0, z: 0 }, Arc::new(Chunk::filled(AIR)));
            s.set_block(chest_pos, terravibe_sim::block::CHEST);
            s.shadow_chests.insert(chest_pos);
            s.chests.insert(
                chest_pos,
                terravibe_sim::chest::ChestInv::from_stacks(&[
                    terravibe_sim::inventory::ItemStack {
                        id: LAVA_CHARM,
                        count: 1,
                        prefix: terravibe_sim::item::Prefix(0),
                    },
                ]),
            );

            {
                let pl = s.conns[0].player.as_mut().unwrap();
                pl.body.pos = [
                    chest_pos[0] as f32 + 0.5,
                    chest_pos[1] as f32 + 1.0,
                    chest_pos[2] as f32 + 0.5,
                ];
                pl.body.vel = [0.0; 3];
                pl.body.fall_peak_y = pl.body.pos[1];
                pl.body.on_ground = true;
            }

            give(&mut s, SHADOW_KEY, 1);
            assert_eq!(s.conns[0].inv.count_of(SHADOW_KEY), 1);

            client
                .send(ClientMsg::OpenChest { pos: chest_pos })
                .unwrap();
            s.tick();

            assert_eq!(
                s.conns[0].open_chest,
                Some(chest_pos),
                "Shadow Key must open underworld chest"
            );
            assert_eq!(
                s.conns[0].inv.count_of(SHADOW_KEY),
                1,
                "Shadow Key must not be consumed when opening underworld chest"
            );

            // 8. Nightmare Pickaxe mines Hellstone in the fixture (interact::tick_use breaks HELLSTONE)
            s.enemies.clear();
            {
                let pl = s.conns[0].player.as_mut().unwrap();
                pl.dead = false;
                pl.respawn_ticks = 0;
                pl.vitals.life = pl.vitals.life_max;
                pl.vitals.burning_ticks = 0;
                pl.vitals.burn_ticks = 0;
                pl.body.pos = [
                    chest_pos[0] as f32 + 0.5,
                    chest_pos[1] as f32 + 1.0,
                    chest_pos[2] as f32 + 0.5,
                ];
                pl.body.vel = [0.0; 3];
                pl.body.fall_peak_y = pl.body.pos[1];
                pl.body.on_ground = true;
                pl.i_frames = 0;
            }
            let px = s.conns[0].player.as_ref().unwrap().body.pos[0].floor() as i32;
            let py = s.conns[0].player.as_ref().unwrap().body.pos[1].floor() as i32;
            let pz = s.conns[0].player.as_ref().unwrap().body.pos[2].floor() as i32;
            s.set_block([px, py - 1, pz], OBSIDIAN_BRICK);
            for pos in [
                [px, py, pz],
                [px, py + 1, pz],
                [px, py, pz - 1],
                [px, py + 1, pz - 1],
            ] {
                s.set_block(pos, AIR);
                ServerLiquidWorld::new(&mut s.world).set_liquid(pos[0], pos[1], pos[2], None);
            }
            let hellstone_pos = [px, py + 1, pz - 2];
            s.set_block(hellstone_pos, HELLSTONE);
            assert_eq!(
                WorldQ(&s.world).block(hellstone_pos[0], hellstone_pos[1], hellstone_pos[2]),
                HELLSTONE
            );

            give(&mut s, NIGHTMARE_PICKAXE, 1);
            let find_slot = |s: &Server, id| {
                s.conns[0]
                    .inv
                    .slots
                    .iter()
                    .position(|x| x.is_some_and(|st| st.id == id))
                    .unwrap() as u8
            };
            let pick_slot = find_slot(&s, NIGHTMARE_PICKAXE);
            s.conns[0].inv.selected = pick_slot as usize;
            s.conns[0].use_state = Default::default();

            let mut hellstone_broken = false;
            let mut seq = 1000;
            for _ in 0..250 {
                seq += 1;
                client
                    .send(ClientMsg::Input {
                        seq,
                        input: PlayerInput {
                            use_held: true,
                            slot: pick_slot,
                            yaw: 0.0,
                            pitch: 0.0,
                            ..Default::default()
                        },
                    })
                    .unwrap();
                s.tick();
                if WorldQ(&s.world).block(hellstone_pos[0], hellstone_pos[1], hellstone_pos[2])
                    == AIR
                {
                    hellstone_broken = true;
                    break;
                }
            }
            assert!(
                hellstone_broken,
                "Nightmare Pickaxe must break Hellstone block in the fixture"
            );
        }

        fn test_server() -> Server {
            Server::new(1)
        }

        fn connect_player(s: &mut Server) -> (usize, InProcClient) {
            let (client, conn) = in_process_pair();
            s.connect(Box::new(conn));
            let cid = s.conns.len() - 1;
            client
                .send(ClientMsg::Hello {
                    seed: 1,
                    name: None,
                    character: None,
                    protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
                    password: None,
                })
                .unwrap();
            s.tick();
            (cid, client)
        }

        #[test]
        fn death_drops_ceil_half_carried_coins() {
            use terravibe_sim::inventory::ItemStack;
            use terravibe_sim::item::{COPPER_COIN, DIRT_BLOCK, GOLD_COIN, Prefix};
            use terravibe_sim::player::DeathCause;
            let mut s = test_server();
            let (cid, _client) = connect_player(&mut s);
            let c = &mut s.conns[cid];
            // Give player 5 Gold Coins and 1 Copper Coin in inventory
            c.inv.slots[0] = Some(ItemStack {
                id: GOLD_COIN,
                count: 5,
                prefix: Prefix(0),
            });
            c.inv.slots[1] = Some(ItemStack {
                id: COPPER_COIN,
                count: 1,
                prefix: Prefix(0),
            });
            c.inv.slots[2] = Some(ItemStack {
                id: DIRT_BLOCK,
                count: 99,
                prefix: Prefix(0),
            });
            c.held = terravibe_sim::slots::Held(Some(ItemStack {
                id: GOLD_COIN,
                count: 3,
                prefix: Prefix(0),
            }));

            // Kill player
            s.kill_player(cid, DeathCause::Fall);

            let c = &s.conns[cid];
            // Ceil half: 5 -> 3 dropped, 2 left; 1 -> 1 dropped, 0 left (None)
            assert_eq!(c.inv.slots[0].unwrap().count, 2);
            assert!(c.inv.slots[1].is_none());
            assert_eq!(c.inv.slots[2].unwrap().count, 99); // Dirt untouched
            // Held ceil half: 3 -> 2 dropped, 1 left
            assert_eq!(c.held.0.as_ref().unwrap().count, 1);

            // Check drops created at death position
            let drops = &s.drops;
            assert_eq!(
                drops
                    .iter()
                    .filter(|d| d.stack.id == GOLD_COIN)
                    .map(|d| d.stack.count)
                    .sum::<u32>(),
                5 // 3 from inv + 2 from held
            );
            assert!(
                drops
                    .iter()
                    .any(|d| d.stack.id == COPPER_COIN && d.stack.count == 1)
            );
        }

        #[test]
        fn client_msg_respawn_respawns_dead_player() {
            use terravibe_sim::player::DeathCause;
            let mut s = test_server();
            let (cid, _client) = connect_player(&mut s);
            s.kill_player(cid, DeathCause::Lava);
            assert!(s.conns[cid].player.as_ref().unwrap().dead);

            s.handle_client_msg(cid, ClientMsg::Respawn);
            assert!(!s.conns[cid].player.as_ref().unwrap().dead);
        }

        #[test]
        fn qa_kill_is_ignored_unless_enabled() {
            let mut s = test_server();
            let (cid, _client) = connect_player(&mut s);
            assert!(!s.cfg.qa_kill);

            s.handle_client_msg(cid, ClientMsg::QaKill);
            assert!(
                !s.conns[cid].player.as_ref().unwrap().dead,
                "QaKill must be ignored when qa_kill is disabled"
            );
        }

        #[test]
        fn qa_kill_kills_player_when_enabled() {
            let mut s = test_server();
            let (cid, _client) = connect_player(&mut s);
            s.cfg.qa_kill = true;

            s.handle_client_msg(cid, ClientMsg::QaKill);
            assert!(
                s.conns[cid].player.as_ref().unwrap().dead,
                "QaKill must kill the player when qa_kill is enabled"
            );
        }

        #[test]
        fn bomb_breaks_blocks_preserves_chest_and_grenade_preserves_terrain() {
            let mut s = test_server();
            let stone_pos = [10, 60, 10];
            let ebon_pos = [11, 60, 10];
            let chest_pos = [12, 60, 10];
            let hell_pos = [10, 61, 10];
            let dung_pos = [10, 62, 10];
            for (p, b) in [
                (stone_pos, terravibe_sim::block::STONE),
                (ebon_pos, terravibe_sim::block::EBONSTONE),
                (chest_pos, CHEST),
                (hell_pos, terravibe_sim::block::HELLSTONE),
                (dung_pos, terravibe_sim::block::BLUE_DUNGEON_BRICK),
            ] {
                let cp = ChunkPos::of_block(p[0], p[1], p[2]);
                s.world
                    .entry(cp)
                    .or_insert_with(|| Arc::new(Chunk::filled(AIR)));
                let ch = Arc::make_mut(s.world.get_mut(&cp).unwrap());
                ch.set(
                    p[0].rem_euclid(CHUNK_SIZE) as usize,
                    p[1].rem_euclid(CHUNK_SIZE) as usize,
                    p[2].rem_euclid(CHUNK_SIZE) as usize,
                    b,
                );
            }
            let block_at = |s: &Server, p: [i32; 3]| {
                s.world
                    .get(&ChunkPos::of_block(p[0], p[1], p[2]))
                    .unwrap()
                    .get(
                        p[0].rem_euclid(CHUNK_SIZE) as usize,
                        p[1].rem_euclid(CHUNK_SIZE) as usize,
                        p[2].rem_euclid(CHUNK_SIZE) as usize,
                    )
            };

            // Grenade damages enemy in radius without breaking STONE.
            s.add_enemy(EnemyKind::Slime, [10.5, 60.0, 10.5]);
            let before_hp = s.enemies.last().unwrap().hp;
            let eid = s.enemies.last().unwrap().id;
            s.explode_projectile(ProjKind::Grenade, [10.5, 60.5, 10.5], 60, 8.0, 4);
            assert!(
                s.enemies
                    .iter()
                    .find(|e| e.id == eid)
                    .is_none_or(|e| e.hp < before_hp),
                "grenade should damage nearby slime"
            );
            assert_eq!(block_at(&s, stone_pos), terravibe_sim::block::STONE);

            // Bomb breaks STONE and EBONSTONE, preserves CHEST/HELLSTONE/DUNGEON.
            s.explode_projectile(ProjKind::Bomb, [10.5, 60.5, 10.5], 100, 0.0, 0);
            assert_eq!(block_at(&s, stone_pos), AIR);
            assert_eq!(block_at(&s, ebon_pos), AIR);
            assert_eq!(block_at(&s, chest_pos), CHEST);
            assert_eq!(block_at(&s, hell_pos), terravibe_sim::block::HELLSTONE);
            assert_eq!(
                block_at(&s, dung_pos),
                terravibe_sim::block::BLUE_DUNGEON_BRICK
            );
        }

        #[test]
        fn server_tracks_world_facts_progression() {
            let mut s = test_server();
            let (cid, _client) = connect_player(&mut s);

            // 1. Shadow orbs broken syncs to facts
            assert_eq!(s.facts.shadow_orbs_broken, 0);
            s.handle_shadow_orb_broken([0, 0, 0]);
            assert_eq!(s.facts.shadow_orbs_broken, 1);
            assert_eq!(s.shadow_orbs_broken, 1);

            // 2. Underworld reached when player y <= 24
            assert!(!s.facts.underworld_reached);
            s.conns[cid].player.as_mut().unwrap().body.pos = [0.0, 20.0, 0.0];
            s.tick();
            assert!(s.facts.underworld_reached);

            // 3. Dungeon reached when player near entrance
            assert!(!s.facts.dungeon_reached);
            let entrance = terravibe_sim::worldgen::dungeon_entrance_pos(s.seed, s.bounds.size());
            s.conns[cid].player.as_mut().unwrap().body.pos = entrance;
            s.tick();
            assert!(s.facts.dungeon_reached);

            // 4. Downed Skeletron
            assert!(!s.facts.downed_skeletron);
            s.add_enemy(EnemyKind::SkeletronHead, [0.0, 50.0, 0.0]);
            s.enemies
                .iter_mut()
                .find(|e| e.kind == EnemyKind::SkeletronHead)
                .unwrap()
                .hp = 0;
            s.tick();
            assert!(s.facts.downed_skeletron);
            assert!(s.downed_skeletron);

            // 5. Downed Eye of Cthulhu
            assert!(!s.facts.downed_eye);
            s.add_enemy(EnemyKind::EyeOfCthulhu, [0.0, 50.0, 0.0]);
            s.enemies
                .iter_mut()
                .find(|e| e.kind == EnemyKind::EyeOfCthulhu)
                .unwrap()
                .hp = 0;
            s.tick();
            assert!(s.facts.downed_eye);

            // 6. Merchant housed flag
            assert!(!s.facts.merchant_housed);
            s.houses.push(House {
                origin: [10, 10, 10],
                home: [12, 10, 12],
                npc: None,
            });
            s.add_enemy(EnemyKind::Merchant, [12.0, 10.0, 12.0]);
            s.move_in();
            assert!(s.facts.merchant_housed);
        }

        fn drain_client_msgs(client: &InProcClient) -> Vec<ServerMsg> {
            let mut msgs = Vec::new();
            while let Ok(Some(m)) = client.try_recv() {
                msgs.push(m);
            }
            msgs
        }

        #[test]
        fn guide_advice_client_message_returns_dialogue() {
            let mut s = test_server();
            let (cid, client) = connect_player(&mut s);
            let guide_id = s
                .enemies
                .iter()
                .find(|e| e.kind == EnemyKind::Guide)
                .map(|e| e.id)
                .unwrap_or_else(|| {
                    let p = s.conns[cid]
                        .player
                        .as_ref()
                        .map(|p| p.body.pos)
                        .unwrap_or(s.spawn);
                    s.add_enemy(EnemyKind::Guide, [p[0] + 1.0, p[1], p[2]]);
                    s.enemies.last().unwrap().id
                });

            s.handle_client_msg(cid, ClientMsg::TalkNpc { id: guide_id });
            s.handle_client_msg(cid, ClientMsg::GuideAdvice);

            let msgs = drain_client_msgs(&client);
            assert!(msgs.iter().any(|m| matches!(m, ServerMsg::Dialogue { npc, text } if *npc == guide_id && text.contains("Guide:"))));
        }

        #[test]
        fn server_syncs_facts_on_join_and_milestones() {
            let mut s = test_server();
            let (cid, client) = connect_player(&mut s);
            let msgs = drain_client_msgs(&client);
            assert!(
                msgs.iter()
                    .any(|m| matches!(m, ServerMsg::SyncFacts { .. }))
            );

            // Update world fact
            s.handle_shadow_orb_broken([0, 0, 0]);
            s.tick();
            let msgs2 = drain_client_msgs(&client);
            assert!(msgs2.iter().any(
                |m| matches!(m, ServerMsg::SyncFacts { world, .. } if world.shadow_orbs_broken == 1)
            ));

            // Update character fact: picking up ore
            let c = &mut s.conns[cid];
            c.inv.slots[0] = Some(terravibe_sim::inventory::ItemStack {
                id: terravibe_sim::item::COPPER_ORE,
                count: 5,
                prefix: terravibe_sim::item::Prefix(0),
            });
            s.tick();
            let msgs3 = drain_client_msgs(&client);
            assert!(
                msgs3.iter().any(
                    |m| matches!(m, ServerMsg::SyncFacts { character, .. } if character.has_ore)
                )
            );
        }

        #[test]
        fn melee_and_projectile_hits_emit_combat_hit_events() {
            use terravibe_sim::net::{DamageKind, HitTargetKind};
            let mut s = test_server();
            s.cfg.natural_spawns = false;
            let (cid, client) = connect_player(&mut s);
            let _ = drain_client_msgs(&client);

            let p_pos = s.conns[cid].player.as_ref().unwrap().body.pos;
            let owner_id = s.conns[cid].id;

            // 1. Melee hit on regular enemy
            // Default yaw=0, pitch=0 looks along -Z. Place slime 1.5 blocks in front at -Z.
            let slime_pos = [p_pos[0], p_pos[1], p_pos[2] - 1.5];
            s.add_enemy(EnemyKind::Slime, slime_pos);
            let mut inp = PlayerInput::default();
            inp.use_held = true;
            inp.slot = 0; // Copper Shortsword
            client
                .send(ClientMsg::Input { seq: 1, input: inp })
                .unwrap();
            s.tick();
            let msgs = drain_client_msgs(&client);
            assert!(
                msgs.iter().any(|m| matches!(
                    m,
                    ServerMsg::CombatHit {
                        damage_kind: DamageKind::Melee,
                        target_kind: HitTargetKind::Enemy,
                        amount,
                        ..
                    } if *amount > 0
                )),
                "expected Melee CombatHit on Enemy"
            );

            // Clear enemies
            s.enemies.clear();

            // 2. Player projectile hit on enemy
            let target_pos = [p_pos[0], p_pos[1], p_pos[2] - 6.0];
            s.add_enemy(EnemyKind::Slime, target_pos);
            let pid = s.next_proj_id;
            s.next_proj_id += 1;
            let mut arrow = Projectile::fire(
                pid,
                owner_id,
                terravibe_sim::item::WOODEN_BOW,
                terravibe_sim::item::WOODEN_ARROW,
                Prefix(0),
                [target_pos[0], target_pos[1] + 0.3, target_pos[2] + 0.5],
                [0.0, 0.0, -1.0],
            )
            .unwrap();
            arrow.ttl = 60;
            s.projectiles.push(arrow);
            s.tick();
            let msgs = drain_client_msgs(&client);
            assert!(
                msgs.iter().any(|m| matches!(
                    m,
                    ServerMsg::CombatHit {
                        damage_kind: DamageKind::Ranged,
                        target_kind: HitTargetKind::Enemy,
                        amount,
                        ..
                    } if *amount > 0
                )),
                "expected Ranged CombatHit on Enemy"
            );

            // 3. Boss hit (melee)
            s.enemies.clear();
            s.projectiles.clear();
            // Let melee swing cooldown expire.
            for seq in 2..25 {
                client
                    .send(ClientMsg::Input {
                        seq,
                        input: PlayerInput::default(),
                    })
                    .unwrap();
                s.tick();
            }
            let _ = drain_client_msgs(&client);
            let boss_pos = [p_pos[0], p_pos[1] - 2.0, p_pos[2] - 2.0];
            s.spawn_boss(boss_pos);
            let mut inp2 = PlayerInput::default();
            inp2.use_held = true;
            inp2.slot = 0;
            client
                .send(ClientMsg::Input {
                    seq: 25,
                    input: inp2,
                })
                .unwrap();
            s.tick();
            let msgs = drain_client_msgs(&client);
            assert!(
                msgs.iter().any(|m| matches!(
                    m,
                    ServerMsg::CombatHit {
                        damage_kind: DamageKind::Melee,
                        target_kind: HitTargetKind::Boss,
                        amount,
                        ..
                    } if *amount > 0
                )),
                "expected Melee CombatHit on Boss"
            );

            // 4. Contact damage on player
            s.enemies.clear();
            s.boss = None;
            s.conns[cid].player.as_mut().unwrap().i_frames = 0;
            s.add_enemy(EnemyKind::Slime, p_pos);
            s.tick();
            let msgs = drain_client_msgs(&client);
            assert!(
                msgs.iter().any(|m| matches!(
                    m,
                    ServerMsg::CombatHit {
                        damage_kind: DamageKind::Contact,
                        target_kind: HitTargetKind::Player,
                        amount,
                        ..
                    } if *amount > 0
                )),
                "expected Contact CombatHit on Player"
            );

            // 5. Enemy telegraph + enemy projectile hit on player
            s.enemies.clear();
            s.projectiles.clear();
            s.conns[cid].player.as_mut().unwrap().i_frames = 0;
            s.add_enemy(EnemyKind::FireImp, [p_pos[0], p_pos[1], p_pos[2] - 5.0]);
            let mut saw_telegraph = false;
            let mut saw_proj_hit = false;
            for _ in 0..220 {
                for dz in -6..=1 {
                    for dy in 0..=3 {
                        set_block(
                            &mut s,
                            [
                                p_pos[0].floor() as i32,
                                p_pos[1].floor() as i32 + dy,
                                p_pos[2].floor() as i32 + dz,
                            ],
                            AIR,
                        );
                    }
                }
                if let Some(pl) = s.conns[cid].player.as_mut() {
                    pl.body.pos = p_pos;
                    pl.body.vel = [0.0; 3];
                    pl.i_frames = 0;
                    pl.vitals.life = pl.vitals.life_max;
                }
                s.tick();
                for m in drain_client_msgs(&client) {
                    if matches!(
                        m,
                        ServerMsg::EnemyTelegraph {
                            kind: EnemyKind::FireImp,
                            ..
                        }
                    ) {
                        assert!(
                            !s.projectiles.iter().any(|pr| pr.owner == u32::MAX),
                            "telegraph must arrive before enemy projectile spawns"
                        );
                        saw_telegraph = true;
                    }
                    if matches!(
                        m,
                        ServerMsg::CombatHit {
                            damage_kind: DamageKind::Projectile,
                            target_kind: HitTargetKind::Player,
                            ..
                        }
                    ) {
                        saw_proj_hit = true;
                    }
                }
                if saw_telegraph && saw_proj_hit {
                    break;
                }
            }
            assert!(saw_telegraph, "expected EnemyTelegraph for FireImp");
            assert!(saw_proj_hit, "expected Projectile CombatHit on Player");
        }
    }

    #[test]
    fn pot_break_drops_coins_and_potions() {
        let mut s = Server::new(42);
        let mut saw_coin = false;
        let mut saw_potion = false;
        for i in 0..20 {
            s.handle_pot_broken([i, 64, 0]);
        }
        for d in &s.drops {
            if d.stack.id == terravibe_sim::item::COPPER_COIN
                || d.stack.id == terravibe_sim::item::SILVER_COIN
            {
                saw_coin = true;
            }
            if d.stack.id == terravibe_sim::item::LESSER_HEALING_POTION {
                saw_potion = true;
            }
        }
        assert!(saw_coin, "breaking pots must drop coins");
        assert!(
            saw_potion,
            "breaking pots must sometimes drop healing potions"
        );
    }

    #[test]
    fn critters_flee_nearby_player_and_respect_cap_and_despawn_dist() {
        use terravibe_sim::enemy::{CRITTER_DESPAWN_DIST, MAX_CRITTERS};

        let (client, conn) = in_process_pair();
        let mut s = Server::with_cfg(
            42,
            ServerCfg {
                natural_spawns: true,
                ..Default::default()
            },
        );
        s.connect(Box::new(conn));
        let mut got = Vec::new();
        hello_and_load(&mut s, &client, &mut got);

        // Run enough ticks for ambient critter spawner to populate up to cap.
        let mut max_seen = 0usize;
        for _ in 0..4000 {
            s.tick();
            let count = s.enemies.iter().filter(|e| e.kind.is_critter()).count();
            assert!(
                count <= MAX_CRITTERS,
                "critter count {count} exceeded MAX_CRITTERS {MAX_CRITTERS}"
            );
            max_seen = max_seen.max(count);
        }
        assert!(
            max_seen > 0,
            "ambient spawner should spawn at least one critter"
        );

        // Add a critter beyond CRITTER_DESPAWN_DIST and verify it despawns on tick.
        let p_pos = s.conns[0].player.as_ref().unwrap().body.pos;
        s.add_enemy(
            EnemyKind::Bunny,
            [p_pos[0] + CRITTER_DESPAWN_DIST + 5.0, p_pos[1], p_pos[2]],
        );
        let far_id = s.enemies.last().unwrap().id;
        s.tick();
        assert!(
            !s.enemies.iter().any(|e| e.id == far_id),
            "critter beyond CRITTER_DESPAWN_DIST must despawn"
        );

        // Add a critter right next to player: no contact damage and flees away.
        let near_pos = [p_pos[0] + 2.0, p_pos[1], p_pos[2]];
        s.add_enemy(EnemyKind::Bunny, near_pos);
        let near_id = s.enemies.last().unwrap().id;
        let hp_before = s.conns[0].player.as_ref().unwrap().vitals.life;
        for _ in 0..20 {
            s.tick();
        }
        let hp_after = s.conns[0].player.as_ref().unwrap().vitals.life;
        assert_eq!(hp_after, hp_before, "critter must not deal contact damage");
        if let Some(b) = s.enemies.iter().find(|e| e.id == near_id) {
            let d_after = (b.pos[0] - p_pos[0]).hypot(b.pos[2] - p_pos[2]);
            assert!(d_after > 2.0, "critter should flee away from nearby player");
        }
    }

    #[test]
    fn assign_housing_moves_npc_and_emits_npc_speech() {
        use terravibe_sim::{CHAIR, DOOR_CLOSED, STONE, TABLE, TORCH};
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.add_enemy(EnemyKind::Guide, spawn_point(1));
        s.tick();
        let guide_id = s
            .enemies
            .iter()
            .find(|e| e.kind == EnemyKind::Guide)
            .expect("guide spawned")
            .id;

        let o = [2, spawn_point(1)[1] as i32 + 1, 1];
        for x in -1..=5 {
            for y in -1..=4 {
                for z in -1..=4 {
                    let p = [o[0] + x, o[1] + y, o[2] + z];
                    let inside = (0..5).contains(&x) && (0..4).contains(&y) && (0..4).contains(&z);
                    if inside {
                        set_block(&mut s, p, AIR);
                        set_wall_at(&mut s, p, true);
                    } else {
                        set_block(&mut s, p, STONE);
                    }
                }
            }
        }
        set_block(&mut s, [o[0] - 1, o[1], o[2]], DOOR_CLOSED);
        set_block(&mut s, [o[0] + 1, o[1], o[2]], CHAIR);
        set_block(&mut s, [o[0] + 2, o[1], o[2]], TABLE);
        set_block(&mut s, [o[0] + 3, o[1], o[2]], TORCH);

        got.clear();
        c.send(ClientMsg::AssignHousing {
            npc: TownKind::Guide,
            pos: [o[0] + 1, o[1] + 1, o[2] + 1],
        })
        .unwrap();
        s.tick();
        drain(&c, &mut got);

        assert!(has(&got, |m| matches!(
            m,
            ServerMsg::HouseStatus {
                ok: true,
                reason: 0
            }
        )));
        assert!(s.houses.iter().any(|h| h.npc == Some(TownKind::Guide)));
        assert!(has(&got, |m| matches!(
            m,
            ServerMsg::NpcSpeech { id, text } if *id == guide_id && !text.is_empty()
        )));
    }

    #[test]
    fn m11_breaker_blade_deals_bonus_damage_to_high_hp_target() {
        use terravibe_sim::item::{BREAKER_BLADE, WARRIOR_EMBLEM};
        let (mut s, c) = setup();
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        give(&mut s, BREAKER_BLADE, 1);
        give(&mut s, WARRIOR_EMBLEM, 1);
        s.conns[0].inv.selected = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|st| st.id == BREAKER_BLADE))
            .unwrap();
        let emb_slot = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|st| st.id == WARRIOR_EMBLEM))
            .unwrap();
        c.send(ClientMsg::Equip {
            from: emb_slot as u8,
        })
        .unwrap();
        s.tick();

        let ppos = s.conns[0].player.as_ref().unwrap().body.pos;
        s.add_enemy(EnemyKind::SkeletronHead, [ppos[0], ppos[1], ppos[2] - 2.0]);
        let target_id = s.enemies.last().unwrap().id;
        let slot = s.conns[0].inv.selected as u8;
        let before_hp = s.enemies.last().unwrap().hp;
        c.send(ClientMsg::Input {
            seq: 1,
            input: PlayerInput {
                use_held: true,
                slot,
                yaw: 0.0,
                pitch: 0.0,
                ..Default::default()
            },
        })
        .unwrap();
        s.tick();
        let after_hp = s.enemies.iter().find(|e| e.id == target_id).unwrap().hp;
        // 70 base * 1.15 Warrior Emblem = 80.5 -> 80; * 2.5 Breaker Blade = 200 (minus def/2=5 -> 195)
        assert!(
            before_hp - after_hp >= 180,
            "expected Breaker Blade 2.5x hit on full-HP SkeletronHead, dealt {}",
            before_hp - after_hp
        );
    }

    #[test]
    fn m11_firecracker_ignites_and_splashes_nearby_enemies() {
        use terravibe_sim::item::FIRECRACKER;
        let (mut s, c) = setup();
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.rng = Rng::new(7);
        give(&mut s, FIRECRACKER, 1);
        s.conns[0].inv.selected = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|st| st.id == FIRECRACKER))
            .unwrap();
        let slot = s.conns[0].inv.selected as u8;
        let ppos = s.conns[0].player.as_ref().unwrap().body.pos;
        s.add_enemy(EnemyKind::Skeleton, [ppos[0], ppos[1], ppos[2] - 2.0]);
        let z1 = s.enemies.last().unwrap().id;
        s.add_enemy(EnemyKind::Skeleton, [ppos[0] + 2.5, ppos[1], ppos[2] - 2.0]);
        let z2 = s.enemies.last().unwrap().id;
        c.send(ClientMsg::Input {
            seq: 1,
            input: PlayerInput {
                use_held: true,
                slot,
                yaw: 0.0,
                pitch: 0.0,
                ..Default::default()
            },
        })
        .unwrap();
        s.tick();
        for id in [z1, z2] {
            let e = s.enemies.iter().find(|e| e.id == id).unwrap();
            assert!(e.hp < e.kind.stats().hp, "enemy {id:?} must take damage");
        }
        let e2 = s.enemies.iter().find(|e| e.id == z2).unwrap();
        assert!(
            e2.is_on_fire(),
            "splash enemy {z2:?} must always be set on fire"
        );
    }

    #[test]
    fn m11_clockwork_assault_rifle_burst_with_ranger_emblem() {
        use terravibe_sim::item::{CLOCKWORK_ASSAULT_RIFLE, MUSKET_BALL, RANGER_EMBLEM};
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        give(&mut s, CLOCKWORK_ASSAULT_RIFLE, 1);
        give(&mut s, MUSKET_BALL, 10);
        give(&mut s, RANGER_EMBLEM, 1);
        s.conns[0].inv.selected = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|st| st.id == CLOCKWORK_ASSAULT_RIFLE))
            .unwrap();
        let emb_slot = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|st| st.id == RANGER_EMBLEM))
            .unwrap();
        c.send(ClientMsg::Equip {
            from: emb_slot as u8,
        })
        .unwrap();
        s.tick();

        let slot = s.conns[0].inv.selected as u8;
        let mut max_projs = 0usize;
        for seq in 1..=12u32 {
            c.send(ClientMsg::Input {
                seq,
                input: PlayerInput {
                    use_held: seq == 1,
                    slot,
                    ..Default::default()
                },
            })
            .unwrap();
            s.tick();
            max_projs = max_projs.max(
                s.projectiles
                    .iter()
                    .filter(|p| p.kind == ProjKind::Bullet)
                    .count(),
            );
        }
        assert_eq!(max_projs, 3, "Clockwork Assault Rifle fires 3-round burst");
        assert_eq!(s.conns[0].inv.count_of(MUSKET_BALL), 9);
        // (17 + 7) * 1.15 = 27.6 -> 28
        assert!(
            s.projectiles.iter().any(|p| p.damage >= 27),
            "Ranger Emblem boosts bullet damage"
        );
    }

    #[test]
    fn m11_laser_rifle_fires_magic_laser_with_sorcerer_emblem() {
        use terravibe_sim::item::{LASER_RIFLE, SORCERER_EMBLEM};
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        give(&mut s, LASER_RIFLE, 1);
        give(&mut s, SORCERER_EMBLEM, 1);
        s.conns[0].inv.selected = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|st| st.id == LASER_RIFLE))
            .unwrap();
        let emb_slot = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|st| st.id == SORCERER_EMBLEM))
            .unwrap();
        c.send(ClientMsg::Equip {
            from: emb_slot as u8,
        })
        .unwrap();
        s.tick();

        let slot = s.conns[0].inv.selected as u8;
        c.send(ClientMsg::Input {
            seq: 1,
            input: PlayerInput {
                use_held: true,
                slot,
                ..Default::default()
            },
        })
        .unwrap();
        s.tick();
        let laser = s
            .projectiles
            .iter()
            .find(|p| p.kind == ProjKind::Laser)
            .expect("Laser Rifle must spawn ProjKind::Laser");
        // 29 * 1.15 = 33.35 -> 33
        assert_eq!(laser.damage, 33);
    }

    #[test]
    fn wof_doll_in_underworld_lava_kills_guide_and_summons_wof() {
        use terravibe_sim::item::GUIDE_VOODOO_DOLL;
        let (mut s, c) = setup();
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        // Place player in Underworld (y <= 24) and ensure Guide is alive.
        let pos = [10.0, 12.0, 10.0];
        s.conns[0].player.as_mut().unwrap().body.pos = pos;
        s.add_enemy(EnemyKind::Guide, [0.0, 40.0, 0.0]);
        assert!(s.town_alive(TownKind::Guide));

        // Create a lava pool at [10, 10, 10]..=[14, 11, 14] in a loaded chunk.
        let cp = ChunkPos::of_block(10, 10, 10);
        s.world
            .entry(cp)
            .or_insert_with(|| Arc::new(Chunk::filled(terravibe_sim::AIR)));
        for dx in 0..=4 {
            for dz in 0..=4 {
                set_block(&mut s, [10 + dx, 9, 10 + dz], terravibe_sim::STONE);
                for dy in 10..=14 {
                    set_block(&mut s, [10 + dx, dy, 10 + dz], terravibe_sim::AIR);
                }
            }
        }
        {
            let mut adapter = ServerLiquidWorld::new(&mut s.world);
            for dx in 0..=4 {
                for dz in 0..=4 {
                    adapter.set_liquid(10 + dx, 10, 10 + dz, Some(Liquid::lava(255)));
                    adapter.set_liquid(10 + dx, 11, 10 + dz, Some(Liquid::lava(255)));
                }
            }
        }

        give(&mut s, GUIDE_VOODOO_DOLL, 1);
        let doll_slot = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|x| x.is_some_and(|st| st.id == GUIDE_VOODOO_DOLL))
            .unwrap();
        s.conns[0].inv.selected = doll_slot;
        c.send(ClientMsg::DropItem {
            dir: [1.0, -0.5, 0.0],
            from_cursor: false,
        })
        .unwrap();

        for _ in 0..25 {
            s.tick();
            if s.wof.is_some() {
                break;
            }
        }
        assert!(
            s.wof.is_some(),
            "Guide Voodoo Doll in Underworld lava must summon Wall of Flesh"
        );
        assert!(
            !s.town_alive(TownKind::Guide),
            "Guide must be killed on WoF summon"
        );
        assert_eq!(s.conns[0].inv.count_of(GUIDE_VOODOO_DOLL), 0);
        assert!(
            s.enemies
                .iter()
                .any(|e| e.kind == EnemyKind::WallOfFleshMouth)
        );
        assert_eq!(
            s.enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::WallOfFleshEye)
                .count(),
            2
        );
    }

    #[test]
    fn wof_doll_in_lava_without_live_guide_does_not_summon() {
        use terravibe_sim::item::GUIDE_VOODOO_DOLL;
        let (mut s, c) = setup();
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.enemies.retain(|e| e.kind != EnemyKind::Guide);

        let cp = ChunkPos::of_block(10, 10, 10);
        s.world
            .entry(cp)
            .or_insert_with(|| Arc::new(Chunk::filled(terravibe_sim::AIR)));
        set_block(&mut s, [10, 9, 10], terravibe_sim::STONE);
        set_block(&mut s, [10, 10, 10], terravibe_sim::AIR);
        ServerLiquidWorld::new(&mut s.world).set_liquid(10, 10, 10, Some(Liquid::lava(255)));

        let stack = terravibe_sim::inventory::ItemStack {
            id: GUIDE_VOODOO_DOLL,
            count: 1,
            prefix: terravibe_sim::item::Prefix(0),
        };
        s.add_drop(stack, [10.5, 10.5, 10.5]);
        s.tick();
        assert!(s.wof.is_none(), "WoF must not summon when Guide is dead");
        assert!(s.drops.is_empty(), "doll is consumed by lava");
    }

    #[test]
    fn wof_defeat_drops_loot_flips_flag_and_keeps_pre_hardmode() {
        use terravibe_sim::item::{
            BREAKER_BLADE, CLOCKWORK_ASSAULT_RIFLE, FIRECRACKER, LASER_RIFLE, PWNHAMMER,
            RANGER_EMBLEM, SORCERER_EMBLEM, WARRIOR_EMBLEM,
        };
        let (mut s, c) = setup();
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        let pos = [-100.0, 12.0, 0.0];
        s.conns[0].player.as_mut().unwrap().body.pos = pos;
        assert!(s.spawn_wof(pos));

        // Damage an Eye to 0 HP; WofBoss::step must sync shared HP to 0 and tick_enemies handles defeat.
        let eye_id = s.wof.as_ref().unwrap().eye_ids[0];
        s.enemies.iter_mut().find(|e| e.id == eye_id).unwrap().hp = 0;
        s.tick();
        drain(&c, &mut got);

        assert!(s.wof.is_none(), "WoF state cleared on defeat");
        assert!(
            s.facts.downed_wall_of_flesh,
            "downed_wall_of_flesh flag flipped"
        );
        assert!(
            !s.enemies.iter().any(|e| matches!(
                e.kind,
                EnemyKind::WallOfFleshMouth
                    | EnemyKind::WallOfFleshEye
                    | EnemyKind::TheHungry
                    | EnemyKind::Leech
            )),
            "all WoF parts and minions must be cleared"
        );
        assert!(has(&got, |m| matches!(m, ServerMsg::WofGone)));
        assert!(
            !has(&got, |m| matches!(m, ServerMsg::WofState { life: 0, .. })),
            "kill tick must not emit WofState(life=0) before WofGone"
        );
        assert!(s.drops.iter().any(|d| d.stack.id == PWNHAMMER));
        assert!(
            s.drops
                .iter()
                .any(|d| matches!(d.stack.id, WARRIOR_EMBLEM | RANGER_EMBLEM | SORCERER_EMBLEM))
        );
        assert!(s.drops.iter().any(|d| matches!(
            d.stack.id,
            BREAKER_BLADE | CLOCKWORK_ASSAULT_RIFLE | LASER_RIFLE | FIRECRACKER
        )));
        // World remains pre-Hardmode (no world-wide Hardmode conversion).
        assert!(!s.downed_skeletron);
    }

    #[test]
    fn wof_eye_lasers_and_tongue_pull_and_far_edge_crush() {
        let (mut s, c) = setup();
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        let pos = [-100.0, 12.0, 0.0];
        s.conns[0].player.as_mut().unwrap().body.pos = pos;
        assert!(s.spawn_wof(pos));

        // Step until Eyes fire EyeLaser.
        for _ in 0..130 {
            s.tick();
            if s.projectiles.iter().any(|p| p.kind == ProjKind::EyeLaser) {
                break;
            }
        }
        assert!(
            s.projectiles.iter().any(|p| p.kind == ProjKind::EyeLaser),
            "Wall of Flesh Eyes must fire EyeLaser"
        );

        // Move player behind the wall -> The Tongue pulls them forward.
        let wall_x = s.wof.as_ref().unwrap().wall_x;
        s.conns[0].player.as_mut().unwrap().body.pos = [wall_x - 20.0, 12.0, 0.0];
        let before_x = s.conns[0].player.as_ref().unwrap().body.pos[0];
        s.tick();
        let after_x = s.conns[0].player.as_ref().unwrap().body.pos[0];
        assert!(
            after_x > before_x,
            "The Tongue must pull player toward front of wall"
        );

        // Advance wall to far edge -> crushes player and clears WoF.
        s.wof.as_mut().unwrap().wall_x = s.bounds.half as f32 - 2.0;
        s.tick();
        assert!(
            s.conns[0].player.as_ref().unwrap().dead,
            "far-edge crush must kill player"
        );
        assert!(s.wof.is_none());
    }

    #[test]
    fn skeletron_wipes_and_respawns_old_man_when_all_players_die() {
        let (mut s, c) = setup();
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        s.enemies.retain(|e| e.kind != EnemyKind::OldMan);
        s.spawn_skeletron([0.0, 50.0, 0.0]);
        assert!(
            s.enemies
                .iter()
                .any(|e| e.kind == EnemyKind::SkeletronHead && e.hp > 0)
        );
        assert_eq!(
            s.enemies
                .iter()
                .filter(|e| e.kind == EnemyKind::SkeletronHand && e.hp > 0)
                .count(),
            2
        );

        s.kill_player(0, DeathCause::Contact(EnemyKind::SkeletronHead));
        assert!(s.conns[0].player.as_ref().unwrap().dead);
        s.tick();

        assert!(
            !s.enemies.iter().any(|e| {
                e.kind == EnemyKind::SkeletronHead || e.kind == EnemyKind::SkeletronHand
            }),
            "Skeletron head and hands must despawn when all players die"
        );
        assert!(
            s.enemies
                .iter()
                .any(|e| e.kind == EnemyKind::OldMan && e.hp > 0),
            "Old Man must respawn at Dungeon entrance after Skeletron wipe"
        );
    }

    #[test]
    fn eater_of_worlds_wipes_when_all_players_die() {
        let (mut s, c) = setup();
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        s.spawn_eater_of_worlds([0.0, 40.0, 0.0]);
        assert!(
            s.enemies
                .iter()
                .any(|e| terravibe_sim::enemy::is_eater_of_worlds(e.kind))
        );

        s.kill_player(0, DeathCause::Contact(EnemyKind::EaterOfWorldsHead));
        s.tick();

        assert!(
            !s.enemies
                .iter()
                .any(|e| terravibe_sim::enemy::is_eater_of_worlds(e.kind)),
            "Eater of Worlds segments must despawn when all players die"
        );
    }

    #[test]
    fn wof_tongue_ignores_respawned_surface_player_in_multiplayer() {
        let (mut s, c1) = setup();
        s.cfg.natural_spawns = false;
        let mut got1 = Vec::new();
        hello_and_load(&mut s, &c1, &mut got1);

        let (c2, conn2) = in_process_pair();
        s.connect(Box::new(conn2));
        c2.send(ClientMsg::Hello {
            seed: 1,
            name: Some("p2".into()),
            character: None,
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();

        // Player 1 is on the Surface (respawned at spawn), Player 2 is in the Underworld fighting WoF.
        let surface_pos = [0.0, 300.0, 0.0];
        let underworld_pos = [-100.0, 12.0, 0.0];
        s.conns[0].player.as_mut().unwrap().body.pos = surface_pos;
        s.conns[1].player.as_mut().unwrap().body.pos = underworld_pos;
        assert!(s.spawn_wof(underworld_pos));

        s.tick_wof();
        let p1_after = s.conns[0].player.as_ref().unwrap().body.pos;
        assert_eq!(
            p1_after, surface_pos,
            "Surface player must not be dragged down by The Tongue"
        );
    }

    #[test]
    fn blue_moon_valor_magic_missile_aqua_scepter_and_laser_pierce_work_on_server() {
        use terravibe_sim::item::{AQUA_SCEPTER, BLUE_MOON, LASER_RIFLE, MAGIC_MISSILE, VALOR};
        let (mut s, c) = setup();
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        // 1. Blue Moon & Valor spawn ProjKind::Flail
        for flail_item in [BLUE_MOON, VALOR] {
            s.projectiles.clear();
            s.conns[0].use_state = UseState::default();
            s.conns[0].inv.slots[0] = Some(terravibe_sim::inventory::ItemStack {
                id: flail_item,
                count: 1,
                prefix: terravibe_sim::item::Prefix(0),
            });
            s.conns[0].inv.selected = 0;
            c.send(ClientMsg::Input {
                seq: 10 + flail_item.0 as u32,
                input: PlayerInput {
                    use_held: true,
                    slot: 0,
                    ..Default::default()
                },
            })
            .unwrap();
            s.tick();
            let want_kind = if flail_item == terravibe_sim::item::VALOR {
                ProjKind::Yoyo
            } else {
                ProjKind::Flail
            };
            assert!(
                s.projectiles.iter().any(|p| p.kind == want_kind),
                "{flail_item:?} must spawn {want_kind:?}"
            );
        }

        // 2. Magic Missile & Aqua Scepter spawn MagicMissile / WaterStream
        for (idx, wand) in [MAGIC_MISSILE, AQUA_SCEPTER].into_iter().enumerate() {
            s.projectiles.clear();
            s.conns[0].use_state = UseState::default();
            s.conns[0].player.as_mut().unwrap().vitals.mana = 100;
            s.conns[0].player.as_mut().unwrap().vitals.mana_max = 100;
            s.conns[0].inv.slots[0] = Some(terravibe_sim::inventory::ItemStack {
                id: wand,
                count: 1,
                prefix: terravibe_sim::item::Prefix(0),
            });
            s.conns[0].inv.selected = 0;
            c.send(ClientMsg::Input {
                seq: 50 + idx as u32,
                input: PlayerInput {
                    use_held: true,
                    slot: 0,
                    ..Default::default()
                },
            })
            .unwrap();
            s.tick();
            let want_kind = if wand == AQUA_SCEPTER {
                ProjKind::WaterStream
            } else {
                ProjKind::MagicMissile
            };
            let proj = s
                .projectiles
                .iter()
                .find(|p| p.kind == want_kind)
                .unwrap_or_else(|| panic!("{wand:?} must spawn {want_kind:?}"));
            if wand == AQUA_SCEPTER {
                assert_eq!(proj.ai0, -1, "Aqua Scepter uses piercing water stream mode");
            } else {
                assert_eq!(proj.ai0, 0, "Magic Missile uses direct missile mode");
            }
        }

        // 3. Laser Rifle pierces 3 enemies and despawns before hitting a 4th
        s.projectiles.clear();
        s.enemies.clear();
        s.conns[0].use_state = UseState::default();
        s.conns[0].player.as_mut().unwrap().vitals.mana = 100;
        let ppos = s.conns[0].player.as_ref().unwrap().body.pos;
        s.add_enemy(EnemyKind::Zombie, [ppos[0], ppos[1], ppos[2] - 1.5]);
        s.add_enemy(EnemyKind::Zombie, [ppos[0], ppos[1], ppos[2] - 1.6]);
        s.add_enemy(EnemyKind::Zombie, [ppos[0], ppos[1], ppos[2] - 1.7]);
        s.add_enemy(EnemyKind::Zombie, [ppos[0], ppos[1], ppos[2] - 1.8]);
        s.conns[0].inv.slots[0] = Some(terravibe_sim::inventory::ItemStack {
            id: LASER_RIFLE,
            count: 1,
            prefix: terravibe_sim::item::Prefix(0),
        });
        s.conns[0].inv.selected = 0;
        c.send(ClientMsg::Input {
            seq: 99,
            input: PlayerInput {
                use_held: true,
                slot: 0,
                yaw: 0.0,
                pitch: 0.0,
                ..Default::default()
            },
        })
        .unwrap();
        for seq in 100..105 {
            c.send(ClientMsg::Input {
                seq,
                input: PlayerInput {
                    use_held: false,
                    slot: 0,
                    ..Default::default()
                },
            })
            .unwrap();
            s.tick();
        }
        let damaged = s
            .enemies
            .iter()
            .filter(|e| e.hp < e.kind.stats().hp)
            .count();
        assert_eq!(damaged, 3, "Laser Rifle must pierce exactly 3 enemies");
    }

    #[test]
    fn tools_deal_melee_damage_and_magic_mirror_recalls_to_spawn() {
        use terravibe_sim::item::{MAGIC_MIRROR, MOLTEN_HAMAXE, MOLTEN_PICKAXE};
        let (mut s, c) = setup();
        s.cfg.natural_spawns = false;
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        let ppos = s.conns[0].player.as_ref().unwrap().body.pos;
        s.add_enemy(EnemyKind::Zombie, [ppos[0], ppos[1], ppos[2] - 1.5]);
        let zid = s.enemies.last().unwrap().id;

        for (idx, tool) in [MOLTEN_PICKAXE, MOLTEN_HAMAXE].into_iter().enumerate() {
            s.conns[0].use_state = UseState::default();
            s.conns[0].inv.slots[0] = Some(terravibe_sim::inventory::ItemStack {
                id: tool,
                count: 1,
                prefix: terravibe_sim::item::Prefix(0),
            });
            s.conns[0].inv.selected = 0;
            let z = s.enemies.iter_mut().find(|e| e.id == zid).unwrap();
            z.hp = 200;
            z.hurt_ticks = 0;
            z.pos = [ppos[0], ppos[1], ppos[2] - 1.5];

            c.send(ClientMsg::Input {
                seq: 10 + idx as u32,
                input: PlayerInput {
                    use_held: true,
                    slot: 0,
                    yaw: 0.0,
                    pitch: 0.0,
                    ..Default::default()
                },
            })
            .unwrap();
            s.tick();
            let z = s.enemies.iter().find(|e| e.id == zid).unwrap();
            assert!(z.hp < 200, "Tool {tool:?} must deal melee damage to enemy");
        }

        // Magic Mirror recalls player to spawn and resets velocity
        s.conns[0].use_state = UseState::default();
        s.conns[0].player.as_mut().unwrap().body.pos = [ppos[0] + 25.0, ppos[1] - 15.0, ppos[2]];
        s.conns[0].player.as_mut().unwrap().body.vel = [0.2, -0.4, 0.1];
        s.conns[0].inv.slots[0] = Some(terravibe_sim::inventory::ItemStack {
            id: MAGIC_MIRROR,
            count: 1,
            prefix: terravibe_sim::item::Prefix(0),
        });
        s.conns[0].inv.selected = 0;
        c.send(ClientMsg::Input {
            seq: 30,
            input: PlayerInput {
                use_held: true,
                slot: 0,
                ..Default::default()
            },
        })
        .unwrap();
        s.tick();
        let after_pos = s.conns[0].player.as_ref().unwrap().body.pos;
        assert!(
            (after_pos[0] - s.spawn[0]).abs() < 1.0 && (after_pos[2] - s.spawn[2]).abs() < 1.0,
            "Magic Mirror must recall player to world spawn (got {after_pos:?}, spawn {:?})",
            s.spawn
        );
    }

    #[test]
    fn locked_chests_send_feedback_dialogue_when_key_missing() {
        let (mut s, c, mut got, pos) = chest_setup(1);
        s.locked_chests.insert(pos);
        c.send(ClientMsg::OpenChest { pos }).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert!(has(&got, |m| matches!(m, ServerMsg::ChestClosed)));
        assert!(
            has(&got, |m| matches!(
                m,
                ServerMsg::Dialogue { text, .. } if text.contains("Golden Key")
            )),
            "Locked gold chest without key must send Golden Key feedback"
        );

        got.clear();
        s.locked_chests.remove(&pos);
        s.shadow_chests.insert(pos);
        c.send(ClientMsg::OpenChest { pos }).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert!(has(&got, |m| matches!(m, ServerMsg::ChestClosed)));
        assert!(
            has(&got, |m| matches!(
                m,
                ServerMsg::Dialogue { text, .. } if text.contains("Shadow Key")
            )),
            "Locked shadow chest without key must send Shadow Key feedback"
        );
    }

    #[test]
    fn old_man_does_not_respawn_while_skeletron_alive_or_at_night() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        // At night with Skeletron alive, OldMan must not respawn
        s.clock.tick = 60_000;
        s.enemies.retain(|e| e.kind != EnemyKind::OldMan);
        s.add_enemy(EnemyKind::SkeletronHead, [0.0, 50.0, 0.0]);
        s.ensure_residents();
        assert!(
            !s.enemies.iter().any(|e| e.kind == EnemyKind::OldMan),
            "OldMan must not respawn while Skeletron is alive"
        );

        // Even if Skeletron is gone, OldMan must not respawn at night
        s.enemies.retain(|e| e.kind != EnemyKind::SkeletronHead);
        s.ensure_residents();
        assert!(
            !s.enemies.iter().any(|e| e.kind == EnemyKind::OldMan),
            "OldMan must not respawn at night"
        );

        // During day without Skeletron and !downed_skeletron, OldMan respawns
        s.clock.tick = 13_500;
        s.ensure_residents();
        assert!(
            s.enemies.iter().any(|e| e.kind == EnemyKind::OldMan),
            "OldMan must respawn during day when Skeletron is not alive"
        );
    }

    #[test]
    fn surface_critters_do_not_spawn_at_night() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.cfg.natural_spawns = true;
        s.clock.tick = 60_000; // night
        s.enemies.retain(|e| !e.kind.is_critter());

        for _ in 0..400 {
            s.critter_ticks = 119;
            let pos = s.conns[0].player.as_ref().unwrap().body.pos;
            s.tick_critter_spawner(&[pos]);
        }
        assert!(
            !s.enemies.iter().any(|e| matches!(
                e.kind,
                EnemyKind::Bunny | EnemyKind::Bird | EnemyKind::Squirrel
            )),
            "Surface critters must not spawn at night"
        );
    }

    #[test]
    fn eater_of_worlds_despawns_after_leaving_corruption() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.cfg.natural_spawns = false;

        // Player is at spawn (Forest, not Corruption); spawn EoW
        s.spawn_eater_of_worlds([0.0, 50.0, 0.0]);
        assert!(
            s.enemies
                .iter()
                .any(|e| terravibe_sim::enemy::is_eater_of_worlds(e.kind))
        );

        for _ in 0..310 {
            s.tick();
        }
        assert!(
            !s.enemies
                .iter()
                .any(|e| terravibe_sim::enemy::is_eater_of_worlds(e.kind)),
            "Eater of Worlds must despawn after players leave Corruption"
        );
    }

    #[test]
    fn ws3_chest_locks_persist_and_unlock_does_not_relock_on_reload() {
        let root = std::env::temp_dir().join(format!("tv_ws3_locks_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mk_cfg = || ServerCfg {
            natural_spawns: false,
            save: Some(save::SaveCfg {
                root: root.clone(),
                world: "w".into(),
            }),
            ..Default::default()
        };
        let dungeon_pos =
            terravibe_sim::worldgen::dungeon::DungeonLayout::new(1, WorldSize::Small).chests[0];
        {
            let mut s = Server::with_cfg(1, mk_cfg());
            let cp = ChunkPos::of_block(dungeon_pos[0], dungeon_pos[1], dungeon_pos[2]);
            let ch = terravibe_sim::worldgen::generate_chunk_sized(1, cp, s.bounds.size());
            s.install_chunk(cp, std::sync::Arc::new(ch));
            assert!(s.locked_chests.contains(&dungeon_pos));
            // Simulate unlocking the chest with a Golden Key.
            s.locked_chests.remove(&dungeon_pos);
            s.save_now().unwrap();
        }
        // Reload from disk and verify re-installing the chunk does NOT re-lock the chest.
        let mut s2 = Server::with_cfg(1, mk_cfg());
        assert!(!s2.locked_chests.contains(&dungeon_pos));
        let cp = ChunkPos::of_block(dungeon_pos[0], dungeon_pos[1], dungeon_pos[2]);
        let ch = terravibe_sim::worldgen::generate_chunk_sized(1, cp, s2.bounds.size());
        s2.install_chunk(cp, std::sync::Arc::new(ch));
        assert!(
            !s2.locked_chests.contains(&dungeon_pos),
            "unlocked chest must stay unlocked after reload"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn ws3_eoc_drops_demonite_and_eow_defeat_sets_downed_eow_and_blocks_duplicate() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.cfg.natural_spawns = false;

        // 1. Eye of Cthulhu death drops Demonite Ore (30..=90) and sets downed_eye.
        s.spawn_boss([0.0, 80.0, 0.0]);
        for e in &mut s.enemies {
            if e.kind == EnemyKind::EyeOfCthulhu {
                e.hp = 0;
            }
        }
        s.tick();
        assert!(s.facts.downed_eye);
        let demonite_total: u32 = s
            .drops
            .iter()
            .filter(|d| d.stack.id == terravibe_sim::item::DEMONITE_ORE_ITEM)
            .map(|d| d.stack.count)
            .sum();
        assert!((30..=90).contains(&demonite_total));

        // 2. Duplicate EoW summon is ignored while EoW is alive; killing all segments sets downed_eow.
        s.spawn_eater_of_worlds([0.0, 50.0, 0.0]);
        let segs_before = s
            .enemies
            .iter()
            .filter(|e| terravibe_sim::enemy::is_eater_of_worlds(e.kind))
            .count();
        assert!(segs_before > 1);
        s.spawn_eater_of_worlds([0.0, 50.0, 0.0]);
        let segs_after = s
            .enemies
            .iter()
            .filter(|e| terravibe_sim::enemy::is_eater_of_worlds(e.kind))
            .count();
        assert_eq!(
            segs_before, segs_after,
            "duplicate EoW summon must be ignored"
        );

        s.drops.clear();
        drain(&c, &mut got);
        for e in &mut s.enemies {
            if terravibe_sim::enemy::is_eater_of_worlds(e.kind) {
                e.hp = 0;
            }
        }
        s.tick();
        assert!(s.facts.downed_eow);
        assert!(
            s.drops
                .iter()
                .any(|d| d.stack.id == terravibe_sim::item::LESSER_HEALING_POTION),
            "full EoW defeat must drop boss extra loot"
        );
        drain(&c, &mut got);
        assert!(got.iter().any(|m| matches!(m, ServerMsg::BossGone)));
    }

    #[test]
    fn ws3_arms_dealer_spawns_and_sells_guns() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        let sp = s.spawn;
        s.houses.push(town_npc::House {
            origin: [
                sp[0].floor() as i32,
                sp[1].floor() as i32,
                sp[2].floor() as i32,
            ],
            home: [
                sp[0].floor() as i32,
                sp[1].floor() as i32,
                sp[2].floor() as i32,
            ],
            npc: None,
        });
        s.conns[0].inv.slots[10] = Some(terravibe_sim::inventory::ItemStack {
            id: terravibe_sim::item::MUSKET_BALL,
            count: 20,
            prefix: terravibe_sim::item::Prefix(0),
        });
        s.clock.tick = town_npc::SPAWN_CHECK_TICKS;
        s.maybe_spawn_arms_dealer();
        let dealer_id = s
            .enemies
            .iter()
            .find(|e| e.kind == EnemyKind::ArmsDealer)
            .expect("Arms Dealer must spawn when carrying gun/bullets and house vacant")
            .id;

        let dealer_pos = s.enemies.iter().find(|e| e.id == dealer_id).unwrap().pos;
        s.conns[0].player.as_mut().unwrap().body.pos = dealer_pos;
        terravibe_sim::coin::give_copper(&mut s.conns[0].inv, 60_000);
        c.send(ClientMsg::TalkNpc { id: dealer_id }).unwrap();
        c.send(ClientMsg::ShopBuy {
            npc: dealer_id,
            index: 1, // Flintlock Pistol (50,000c)
            count: 1,
        })
        .unwrap();
        s.tick();
        drain(&c, &mut got);
        assert!(
            got.iter()
                .any(|m| matches!(m, ServerMsg::ShopContents { items } if items.len() == 3))
        );
        assert!(
            s.conns[0]
                .inv
                .count_of(terravibe_sim::item::FLINTLOCK_PISTOL)
                == 1
        );
    }

    #[test]
    fn ws3_dead_guide_respawns_only_on_day_boss_free_spawn_check() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        s.clock.tick = terravibe_sim::time::DAY_TICKS / 2;
        for _ in 0..1200 {
            s.ensure_residents();
            if s.town_alive(TownKind::Guide) {
                break;
            }
            s.tick();
            std::thread::sleep(std::time::Duration::from_micros(500));
        }
        assert!(s.town_alive(TownKind::Guide));

        // Kill Guide.
        for e in &mut s.enemies {
            if e.kind == EnemyKind::Guide {
                e.hp = 0;
            }
        }
        s.tick();
        assert!(!s.town_alive(TownKind::Guide));

        // Night check: does NOT respawn even on SPAWN_CHECK_TICKS.
        s.clock.tick = terravibe_sim::time::DAY_TICKS + 120;
        s.ensure_residents();
        s.respawn_dead_town_npcs();
        assert!(
            !s.town_alive(TownKind::Guide),
            "Guide must not respawn at night"
        );

        // Daytime check WITH boss alive: does NOT respawn.
        s.clock.tick = terravibe_sim::time::DAY_TICKS / 2;
        s.spawn_boss([0.0, 80.0, 0.0]);
        s.respawn_dead_town_npcs();
        assert!(
            !s.town_alive(TownKind::Guide),
            "Guide must not respawn while a boss is alive"
        );

        // Daytime + boss gone: respawns on respawn check.
        s.boss = None;
        s.enemies.clear();
        s.respawn_dead_town_npcs();
        assert!(
            s.town_alive(TownKind::Guide),
            "Guide must respawn on daytime boss-free check"
        );
    }

    #[test]
    fn ws3_merchant_counts_cursor_coins_across_players() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);
        let sp = s.spawn;
        s.houses.push(town_npc::House {
            origin: [
                sp[0].floor() as i32,
                sp[1].floor() as i32,
                sp[2].floor() as i32,
            ],
            home: [
                sp[0].floor() as i32,
                sp[1].floor() as i32,
                sp[2].floor() as i32,
            ],
            npc: None,
        });
        // Hold 1 Gold Coin (= 10,000 copper >= 5,000) in cursor `held` instead of bag.
        s.conns[0].held = terravibe_sim::slots::Held(Some(terravibe_sim::inventory::ItemStack {
            id: terravibe_sim::item::GOLD_COIN,
            count: 1,
            prefix: terravibe_sim::item::Prefix(0),
        }));
        s.clock.tick = town_npc::SPAWN_CHECK_TICKS;
        s.maybe_spawn_merchant();
        assert!(
            s.town_alive(TownKind::Merchant),
            "Merchant spawn check must include cursor held coins"
        );
    }

    #[test]
    fn bug_net_catches_critter_and_using_critter_item_releases_it() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        let p_pos = s.conns[0].player.as_ref().unwrap().body.pos;
        s.enemies.retain(|e| !e.kind.is_critter());
        s.add_enemy(EnemyKind::Bunny, [p_pos[0], p_pos[1], p_pos[2] - 2.0]);

        s.conns[0].inv.slots[0] = Some(ItemStack {
            id: BUG_NET,
            count: 1,
            prefix: Prefix(0),
        });
        s.conns[0].inv.selected = 0;

        c.send(ClientMsg::Input {
            seq: 1,
            input: PlayerInput {
                use_held: true,
                slot: 0,
                ..Default::default()
            },
        })
        .unwrap();
        s.tick();

        assert_eq!(
            s.conns[0].inv.count_of(BUNNY_ITEM),
            1,
            "Bug Net must catch nearby Bunny into inventory"
        );
        assert!(
            !s.enemies.iter().any(|e| e.kind == EnemyKind::Bunny),
            "Caught Bunny must be despawned from world"
        );

        // Select the caught Bunny item and release it.
        let bunny_slot = s.conns[0]
            .inv
            .slots
            .iter()
            .position(|sl| sl.is_some_and(|st| st.id == BUNNY_ITEM))
            .unwrap() as u8;
        for seq in 2..26 {
            c.send(ClientMsg::Input {
                seq,
                input: PlayerInput {
                    use_held: false,
                    slot: bunny_slot,
                    ..Default::default()
                },
            })
            .unwrap();
            s.tick();
        }
        c.send(ClientMsg::Input {
            seq: 26,
            input: PlayerInput {
                use_held: true,
                slot: bunny_slot,
                ..Default::default()
            },
        })
        .unwrap();
        s.tick();

        assert_eq!(
            s.conns[0].inv.count_of(BUNNY_ITEM),
            0,
            "Using caught Bunny item must consume 1"
        );
        assert!(
            s.enemies.iter().any(|e| e.kind == EnemyKind::Bunny),
            "Using caught Bunny item must spawn a live Bunny"
        );
    }

    #[test]
    fn creative_give_and_actions_work() {
        let (mut s, c) = setup();
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        // CreativeGive clamps count to max_stack and syncs Inventory.
        c.send(ClientMsg::CreativeGive {
            item: terravibe_sim::item::TORCH,
            count: 50_000,
        })
        .unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(
            s.conns[0].inv.count_of(terravibe_sim::item::TORCH),
            terravibe_sim::item::item_data(terravibe_sim::item::TORCH).max_stack
        );
        assert!(got.iter().any(|m| matches!(m, ServerMsg::Inventory(_))));

        // ToggleGodMode enables god_mode, sends ServerMsg::GodMode(true), and blocks damage.
        c.send(ClientMsg::CreativeAction(CreativeCmd::ToggleGodMode))
            .unwrap();
        s.tick();
        drain(&c, &mut got);
        assert!(s.conns[0].player.as_ref().unwrap().god_mode);
        assert!(got.iter().any(|m| matches!(m, ServerMsg::GodMode(true))));

        let p = s.conns[0].player.as_mut().unwrap();
        let mut ev = Vec::new();
        assert!(!hurt_player_with_cause(p, 50, DeathCause::Unknown, &mut ev));
        assert_eq!(p.vitals.life, p.vitals.life_max);

        // FullHeal restores life, mana, and clears debuffs.
        p.vitals.life = 10;
        p.vitals.mana = 0;
        p.vitals.potion_delay_ticks = 600;
        p.vitals.burn_ticks = 120;
        c.send(ClientMsg::CreativeAction(CreativeCmd::FullHeal))
            .unwrap();
        s.tick();
        let p = s.conns[0].player.as_ref().unwrap();
        assert_eq!(p.vitals.life, p.vitals.life_max);
        assert_eq!(p.vitals.mana, p.effective_mana_max());
        assert_eq!(p.vitals.potion_delay_ticks, 0);
        assert_eq!(p.vitals.burn_ticks, 0);

        // SetTime updates clock tick and syncs ServerMsg::Time.
        c.send(ClientMsg::CreativeAction(CreativeCmd::SetTime(27_000)))
            .unwrap();
        s.tick();
        drain(&c, &mut got);
        assert!(
            got.iter()
                .any(|m| matches!(m, ServerMsg::Time { tick } if *tick >= 27_000))
        );

        // SetTime(59_400) switches to night without triggering a natural dusk boss spawn roll.
        s.conns[0].player.as_mut().unwrap().vitals.life_max = 200;
        s.conns[0].player.as_mut().unwrap().armor = [
            Some(terravibe_sim::item::MOLTEN_HELMET),
            Some(terravibe_sim::item::MOLTEN_BREASTPLATE),
            None,
        ];
        s.add_enemy(EnemyKind::Guide, [0.0, 40.0, 0.0]);
        s.add_enemy(EnemyKind::Merchant, [2.0, 40.0, 0.0]);
        s.add_enemy(EnemyKind::ArmsDealer, [4.0, 40.0, 0.0]);
        s.eye_pending = false;
        c.send(ClientMsg::CreativeAction(CreativeCmd::SetTime(59_400)))
            .unwrap();
        s.tick();
        assert!(!s.clock.is_day());
        assert!(!s.eye_pending);
    }

    #[test]
    fn dismiss_onboarding_persists_across_sync_facts_and_character_state() {
        let (c, conn) = in_process_pair();
        let mut s = Server::new(1);
        s.connect(Box::new(conn));
        let ch =
            terravibe_sim::Character::new("OnboardingTester", terravibe_sim::Appearance::default());
        c.send(ClientMsg::Hello {
            seed: 1,
            name: None,
            character: Some(ch.to_bytes()),
            protocol_version: terravibe_sim::net::PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();
        let mut got = Vec::new();
        drain(&c, &mut got);

        c.send(ClientMsg::DismissOnboarding).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert!(
            got.iter().any(|m| matches!(
                m,
                ServerMsg::SyncFacts { character, .. } if character.onboarding_dismissed
            )),
            "DismissOnboarding must sync onboarding_dismissed=true"
        );

        // Trigger another SyncFacts update (e.g. picking up ore) and verify it stays dismissed.
        s.conns[0].inv.slots[0] = Some(ItemStack {
            id: terravibe_sim::item::COPPER_ORE,
            count: 1,
            prefix: Prefix(0),
        });
        s.tick();
        drain(&c, &mut got);
        assert!(
            got.iter().any(|m| matches!(
                m,
                ServerMsg::SyncFacts { character, .. } if character.onboarding_dismissed && character.has_ore
            )),
            "Subsequent SyncFacts must keep onboarding_dismissed=true"
        );

        // Disconnect and verify CharacterState persists onboarding_dismissed=true.
        c.send(ClientMsg::Bye).unwrap();
        s.tick();
        drain(&c, &mut got);
        let bytes = got
            .iter()
            .rev()
            .find_map(|m| match m {
                ServerMsg::CharacterState(b) => Some(b.clone()),
                _ => None,
            })
            .expect("must send CharacterState on disconnect");
        let saved = terravibe_sim::Character::from_bytes(&bytes).unwrap();
        assert!(saved.facts.onboarding_dismissed);
        assert!(saved.facts.has_ore);
    }

    #[test]
    fn guide_slot_click_shift_and_return_lifecycle() {
        use terravibe_sim::item::{IRON_BAR, WOOD, WOODEN_SWORD};
        use terravibe_sim::slots::{ClickKind, SlotRef};

        let (c, conn) = in_process_pair();
        let mut s = Server::new(1);
        s.connect(Box::new(conn));
        let mut got = Vec::new();
        hello_and_load(&mut s, &c, &mut got);

        // Seed inventory with WOOD (material) and WOODEN_SWORD (non-material) and IRON_BAR (material).
        s.conns[0].inv.slots[0] = Some(ItemStack {
            id: WOOD,
            count: 5,
            prefix: Prefix(0),
        });
        s.conns[0].inv.slots[1] = Some(ItemStack {
            id: WOODEN_SWORD,
            count: 1,
            prefix: Prefix(0),
        });
        s.conns[0].inv.slots[2] = Some(ItemStack {
            id: IRON_BAR,
            count: 3,
            prefix: Prefix(0),
        });

        // Pick up WOOD and click SlotRef::Guide.
        c.send(ClientMsg::ClickSlot {
            at: SlotRef::Inv(0),
            kind: ClickKind::Left,
        })
        .unwrap();
        c.send(ClientMsg::ClickSlot {
            at: SlotRef::Guide,
            kind: ClickKind::Left,
        })
        .unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(
            s.conns[0].guide_slot.map(|st| (st.id, st.count)),
            Some((WOOD, 5))
        );
        assert!(got.iter().any(|m| matches!(
            m,
            ServerMsg::GuideSlot(Some(st)) if st.id == WOOD && st.count == 5
        )));

        // Shift-place IRON_BAR swaps WOOD back into slot 2.
        c.send(ClientMsg::GuideSlotShift { slot: 2 }).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(
            s.conns[0].guide_slot.map(|st| (st.id, st.count)),
            Some((IRON_BAR, 3))
        );
        assert_eq!(
            s.conns[0].inv.slots[2].map(|st| (st.id, st.count)),
            Some((WOOD, 5))
        );

        // Shift-place non-material WOODEN_SWORD is ignored.
        c.send(ClientMsg::GuideSlotShift { slot: 1 }).unwrap();
        s.tick();
        assert_eq!(
            s.conns[0].guide_slot.map(|st| (st.id, st.count)),
            Some((IRON_BAR, 3))
        );

        // CloseGuideCrafting returns IRON_BAR to inventory and syncs GuideSlot(None).
        got.clear();
        c.send(ClientMsg::CloseGuideCrafting).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(s.conns[0].guide_slot, None);
        assert_eq!(s.conns[0].inv.count_of(IRON_BAR), 3);
        assert!(got.iter().any(|m| matches!(m, ServerMsg::GuideSlot(None))));
    }

    #[test]
    fn hello_rejects_bad_password_full_server_and_wrong_version() {
        use terravibe_sim::net::{PROTOCOL_VERSION, RejectReason};

        let cfg = ServerCfg {
            natural_spawns: false,
            max_players: 2,
            password: Some("s3cr3t".into()),
            ..ServerCfg::default()
        };
        let mut s = Server::with_cfg(1, cfg);

        // Host connects (exempt from password & max_players).
        let (host, host_conn) = in_process_pair();
        s.connect_host(Box::new(host_conn));
        host.send(ClientMsg::Hello {
            seed: 1,
            name: Some("Host".into()),
            character: None,
            protocol_version: PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();
        let mut host_msgs = Vec::new();
        drain(&host, &mut host_msgs);
        assert!(
            host_msgs
                .iter()
                .any(|m| matches!(m, ServerMsg::Welcome { .. })),
            "host must be welcomed even without password"
        );
        host_msgs.clear();

        // Wrong protocol version -> Rejected(ProtocolMismatch), no ghost, no RemotePlayerSpawn.
        let (bad_ver, bad_ver_conn) = in_process_pair();
        s.connect(Box::new(bad_ver_conn));
        bad_ver
            .send(ClientMsg::Hello {
                seed: 1,
                name: Some("BadVer".into()),
                character: None,
                protocol_version: PROTOCOL_VERSION + 1,
                password: Some("s3cr3t".into()),
            })
            .unwrap();
        s.tick();
        let mut bad_ver_msgs = Vec::new();
        drain(&bad_ver, &mut bad_ver_msgs);
        assert!(bad_ver_msgs.iter().any(|m| matches!(
            m,
            ServerMsg::Rejected {
                reason: RejectReason::ProtocolMismatch
            }
        )));
        assert_eq!(s.connection_count(), 1);

        // Wrong password -> Rejected(BadPassword), no ghost, no RemotePlayerSpawn.
        let (bad_pw, bad_pw_conn) = in_process_pair();
        s.connect(Box::new(bad_pw_conn));
        bad_pw
            .send(ClientMsg::Hello {
                seed: 1,
                name: Some("BadPw".into()),
                character: None,
                protocol_version: PROTOCOL_VERSION,
                password: Some("wrong".into()),
            })
            .unwrap();
        s.tick();
        let mut bad_pw_msgs = Vec::new();
        drain(&bad_pw, &mut bad_pw_msgs);
        assert!(bad_pw_msgs.iter().any(|m| matches!(
            m,
            ServerMsg::Rejected {
                reason: RejectReason::BadPassword
            }
        )));
        assert_eq!(s.connection_count(), 1);

        drain(&host, &mut host_msgs);
        assert!(
            !host_msgs
                .iter()
                .any(|m| matches!(m, ServerMsg::RemotePlayerSpawn { .. })),
            "rejected clients must not broadcast RemotePlayerSpawn"
        );

        // Valid guest fills the second slot (max_players = 2).
        let (g1, g1_conn) = in_process_pair();
        s.connect(Box::new(g1_conn));
        g1.send(ClientMsg::Hello {
            seed: 1,
            name: Some("Guest1".into()),
            character: None,
            protocol_version: PROTOCOL_VERSION,
            password: Some("s3cr3t".into()),
        })
        .unwrap();
        s.tick();
        let mut g1_msgs = Vec::new();
        drain(&g1, &mut g1_msgs);
        assert!(
            g1_msgs
                .iter()
                .any(|m| matches!(m, ServerMsg::Welcome { .. }))
        );
        assert_eq!(s.connection_count(), 2);
        // Drain host's messages from g1 joining before testing g2 rejection.
        host_msgs.clear();
        drain(&host, &mut host_msgs);
        assert!(
            host_msgs.iter().any(
                |m| matches!(m, ServerMsg::RemotePlayerSpawn { name, .. } if name == "Guest1")
            ),
            "host must receive RemotePlayerSpawn for accepted Guest1"
        );

        // Third player -> Rejected(ServerFull), no ghost, no RemotePlayerSpawn.
        host_msgs.clear();
        let (g2, g2_conn) = in_process_pair();
        s.connect(Box::new(g2_conn));
        g2.send(ClientMsg::Hello {
            seed: 1,
            name: Some("Guest2".into()),
            character: None,
            protocol_version: PROTOCOL_VERSION,
            password: Some("s3cr3t".into()),
        })
        .unwrap();
        s.tick();
        let mut g2_msgs = Vec::new();
        drain(&g2, &mut g2_msgs);
        assert!(g2_msgs.iter().any(|m| matches!(
            m,
            ServerMsg::Rejected {
                reason: RejectReason::ServerFull
            }
        )));
        assert_eq!(s.connection_count(), 2);
        drain(&host, &mut host_msgs);
        assert!(
            !host_msgs
                .iter()
                .any(|m| matches!(m, ServerMsg::RemotePlayerSpawn { .. })),
            "full-server rejection must not broadcast RemotePlayerSpawn"
        );
    }

    #[test]
    fn host_disconnect_broadcasts_host_closing_and_saves() {
        use std::sync::atomic::AtomicBool;
        use std::sync::mpsc;
        use terravibe_sim::Character;
        use terravibe_sim::net::{PROTOCOL_VERSION, RejectReason};

        let tmp = std::env::temp_dir().join(format!(
            "terravibe_mp_host_close_{}_{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        let save_cfg = SaveCfg {
            root: tmp.clone(),
            world: "mp_world".into(),
        };
        let cfg = ServerCfg {
            natural_spawns: false,
            save: Some(save_cfg),
            ..ServerCfg::default()
        };

        let (tx, rx) = mpsc::channel::<Box<dyn ServerConn>>();
        let shutdown = Arc::new(AtomicBool::new(false));
        let (host, handle) = spawn_host_cfg(7, cfg, rx, Arc::clone(&shutdown));

        // Host joins.
        host.send(ClientMsg::Hello {
            seed: 7,
            name: Some("Host".into()),
            character: None,
            protocol_version: PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        let mut host_got = Vec::new();
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(10) {
            while let Ok(Some(m)) = host.try_recv() {
                host_got.push(m);
            }
            if host_got
                .iter()
                .any(|m| matches!(m, ServerMsg::Welcome { .. }))
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            host_got
                .iter()
                .any(|m| matches!(m, ServerMsg::Welcome { .. }))
        );

        // Guest connects via incoming_conns with a Character.
        let (guest, guest_conn) = in_process_pair();
        tx.send(Box::new(guest_conn)).unwrap();
        let ch = Character::new("GuestHero", terravibe_sim::character::Appearance::default());
        guest
            .send(ClientMsg::Hello {
                seed: 7,
                name: Some("GuestHero".into()),
                character: Some(ch.to_bytes()),
                protocol_version: PROTOCOL_VERSION,
                password: None,
            })
            .unwrap();

        let mut guest_got = Vec::new();
        let t1 = Instant::now();
        while t1.elapsed() < Duration::from_secs(10) {
            while let Ok(Some(m)) = guest.try_recv() {
                guest_got.push(m);
            }
            if guest_got
                .iter()
                .any(|m| matches!(m, ServerMsg::Welcome { .. }))
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            guest_got
                .iter()
                .any(|m| matches!(m, ServerMsg::Welcome { .. }))
        );
        guest_got.clear();

        // Host disconnects with Bye -> server broadcasts HostClosing, flushes CharacterState, saves, and exits.
        host.send(ClientMsg::Bye).unwrap();
        handle.join().expect("server thread must exit cleanly");

        while let Ok(Some(m)) = guest.try_recv() {
            guest_got.push(m);
        }
        let char_idx = guest_got
            .iter()
            .position(|m| matches!(m, ServerMsg::CharacterState(_)));
        let close_idx = guest_got.iter().position(|m| {
            matches!(
                m,
                ServerMsg::Rejected {
                    reason: RejectReason::HostClosing
                }
            )
        });
        assert!(
            char_idx.is_some(),
            "guest must receive flushed CharacterState on host close"
        );
        assert!(
            close_idx.is_some(),
            "guest must receive Rejected(HostClosing) on host close"
        );
        assert!(
            char_idx.unwrap() < close_idx.unwrap(),
            "CharacterState should be flushed before HostClosing"
        );

        // Verify world was saved to disk.
        let dir = SaveDir::open(&tmp, "mp_world").expect("save dir should open");
        let loaded = dir.load_world().expect("world should load");
        assert!(loaded.is_some(), "world snapshot must be saved on exit");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn two_players_see_remote_spawn_state_equipment_and_chat() {
        use terravibe_sim::item::{COPPER_HELMET, WOODEN_SWORD};
        use terravibe_sim::net::PROTOCOL_VERSION;

        let mut s = Server::with_cfg(
            1,
            ServerCfg {
                natural_spawns: false,
                ..ServerCfg::default()
            },
        );
        let (p1, c1) = in_process_pair();
        let (p2, c2) = in_process_pair();
        s.connect(Box::new(c1));
        s.connect(Box::new(c2));

        p1.send(ClientMsg::Hello {
            seed: 1,
            name: Some("Alice".into()),
            character: None,
            protocol_version: PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();
        let mut m1 = Vec::new();
        drain(&p1, &mut m1);
        let p1_id = m1
            .iter()
            .find_map(|m| match m {
                ServerMsg::Welcome { id, .. } => Some(*id),
                _ => None,
            })
            .expect("p1 Welcome");
        m1.clear();

        p2.send(ClientMsg::Hello {
            seed: 1,
            name: Some("Bob".into()),
            character: None,
            protocol_version: PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();
        let mut m2 = Vec::new();
        drain(&p1, &mut m1);
        drain(&p2, &mut m2);
        let p2_id = m2
            .iter()
            .find_map(|m| match m {
                ServerMsg::Welcome { id, .. } => Some(*id),
                _ => None,
            })
            .expect("p2 Welcome");

        // Both directions receive RemotePlayerSpawn and PlayerList.
        assert!(m1.iter().any(|m| matches!(
            m,
            ServerMsg::RemotePlayerSpawn { id, name, .. } if *id == p2_id && name == "Bob"
        )));
        assert!(m2.iter().any(|m| matches!(
            m,
            ServerMsg::RemotePlayerSpawn { id, name, .. } if *id == p1_id && name == "Alice"
        )));
        assert!(m1.iter().any(|m| matches!(
            m,
            ServerMsg::PlayerList { players } if players.len() == 2
        )));
        assert!(m2.iter().any(|m| matches!(
            m,
            ServerMsg::PlayerList { players } if players.len() == 2
        )));

        // Equipment change broadcasts RemotePlayerEquipment.
        m1.clear();
        m2.clear();
        s.conns[0].inv.slots[0] = Some(ItemStack {
            id: WOODEN_SWORD,
            count: 1,
            prefix: Prefix(0),
        });
        s.conns[0].inv.selected = 0;
        s.conns[0].player.as_mut().unwrap().armor[0] = Some(COPPER_HELMET);
        s.tick();
        drain(&p2, &mut m2);
        assert!(m2.iter().any(|m| matches!(
            m,
            ServerMsg::RemotePlayerEquipment { id, armor, held, .. }
                if *id == p1_id && armor[0] == Some(COPPER_HELMET) && *held == Some(WOODEN_SWORD)
        )));

        // 20 Hz RemotePlayerState (within VIEW_RADIUS_BLOCKS).
        m2.clear();
        for seq in 1..=3 {
            let mut input = PlayerInput::default();
            input.yaw = 1.25;
            input.pitch = -0.2;
            p1.send(ClientMsg::Input { seq, input }).unwrap();
            s.tick();
        }
        drain(&p2, &mut m2);
        assert!(m2.iter().any(|m| matches!(
            m,
            ServerMsg::RemotePlayerState { id, yaw, pitch, .. }
                if *id == p1_id && (*yaw - 1.25).abs() < 1e-4 && (*pitch - (-0.2)).abs() < 1e-4
        )));

        // Chat: empty/whitespace ignored, >200 chars truncated to 200 chars.
        m1.clear();
        m2.clear();
        p1.send(ClientMsg::Chat {
            text: "   \t  ".into(),
        })
        .unwrap();
        let long_msg = format!("  {}  ", "a".repeat(250));
        p1.send(ClientMsg::Chat { text: long_msg }).unwrap();
        s.tick();
        drain(&p1, &mut m1);
        drain(&p2, &mut m2);
        let chats2: Vec<_> = m2
            .iter()
            .filter_map(|m| match m {
                ServerMsg::Chat { sender, text } => Some((sender.clone(), text.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(chats2.len(), 1, "empty chat must be ignored");
        assert_eq!(chats2[0].0.as_deref(), Some("Alice"));
        assert_eq!(chats2[0].1.chars().count(), 200);

        // Leave broadcasts RemotePlayerDespawn, system Chat, and updated PlayerList.
        m1.clear();
        p2.send(ClientMsg::Bye).unwrap();
        s.tick();
        drain(&p1, &mut m1);
        assert!(
            m1.iter()
                .any(|m| matches!(m, ServerMsg::RemotePlayerDespawn { id } if *id == p2_id))
        );
        assert!(m1.iter().any(|m| matches!(
            m,
            ServerMsg::Chat { sender: None, text } if text.contains("Bob")
        )));
        assert!(m1.iter().any(|m| matches!(
            m,
            ServerMsg::PlayerList { players } if players.len() == 1 && players[0].0 == p1_id
        )));
    }

    #[test]
    fn multiplayer_chest_lock_and_enemy_nearest_target() {
        use terravibe_sim::net::PROTOCOL_VERSION;

        let mut s = Server::with_cfg(
            1,
            ServerCfg {
                natural_spawns: false,
                clock_start: terravibe_sim::time::DAY_TICKS + 1000, // night so zombies chase
                ..ServerCfg::default()
            },
        );
        let (p1, c1) = in_process_pair();
        let (p2, c2) = in_process_pair();
        s.connect(Box::new(c1));
        s.connect(Box::new(c2));

        for (p, name) in [(&p1, "P1"), (&p2, "P2")] {
            p.send(ClientMsg::Hello {
                seed: 1,
                name: Some(name.into()),
                character: None,
                protocol_version: PROTOCOL_VERSION,
                password: None,
            })
            .unwrap();
        }
        let sp = spawn_point(1);
        let chest_pos = [
            sp[0].floor() as i32 + 1,
            sp[1].floor() as i32,
            sp[2].floor() as i32,
        ];
        let chest_cp = ChunkPos::of_block(chest_pos[0], chest_pos[1], chest_pos[2]);
        let mut got = Vec::new();
        assert!(pump(&mut s, &p1, &mut got, |m| has_chunk(m, chest_cp)));

        // Place a chest in reach of both players at spawn.
        s.set_block(chest_pos, CHEST);
        s.chests.entry(chest_pos).or_default();

        // P1 opens the chest; P2 cannot open it while P1 holds the lock.
        p1.send(ClientMsg::OpenChest { pos: chest_pos }).unwrap();
        p2.send(ClientMsg::OpenChest { pos: chest_pos }).unwrap();
        s.tick();
        assert_eq!(s.conns[0].open_chest, Some(chest_pos));
        assert_eq!(s.conns[1].open_chest, None);

        // Enemy targets nearest alive player: place P1 at x=sp[0]-20, P2 at x=sp[0]+20, enemy at x=sp[0]+14.
        s.conns[0].player.as_mut().unwrap().body.pos = [sp[0] - 20.0, sp[1], sp[2]];
        s.conns[1].player.as_mut().unwrap().body.pos = [sp[0] + 20.0, sp[1], sp[2]];
        let enemy_start = [sp[0] + 14.0, sp[1] + 2.0, sp[2]];
        s.add_enemy(EnemyKind::DemonEye, enemy_start);
        for _ in 0..10 {
            s.tick();
        }
        let eye = s
            .enemies
            .iter()
            .find(|e| e.kind == EnemyKind::DemonEye)
            .expect("demon eye alive");
        assert!(
            eye.pos[0] > enemy_start[0],
            "demon eye should move toward nearer P2 (+x), got x={} vs start={}",
            eye.pos[0],
            enemy_start[0]
        );

        // If P2 dies, enemy must retarget alive P1 (-x).
        s.conns[1].player.as_mut().unwrap().dead = true;
        let before_x = eye.pos[0];
        for _ in 0..25 {
            s.tick();
        }
        let eye2 = s
            .enemies
            .iter()
            .find(|e| e.kind == EnemyKind::DemonEye)
            .expect("demon eye still alive");
        assert!(
            eye2.pos[0] < before_x,
            "when nearer P2 is dead, demon eye must move toward alive P1 (-x), got x={} vs before={}",
            eye2.pos[0],
            before_x
        );
    }

    #[test]
    fn server_trash_slot_and_shift_trash() {
        use terravibe_sim::item::{IRON_BAR, WOOD};
        let mut s = Server::with_cfg(1, ServerCfg::default());
        let (c, c_srv) = in_process_pair();
        s.connect(Box::new(c_srv));
        c.send(ClientMsg::Hello {
            seed: 1,
            name: Some("Tester".into()),
            character: None,
            protocol_version: PROTOCOL_VERSION,
            password: None,
        })
        .unwrap();
        s.tick();
        let mut got = Vec::new();
        s.conns[0].inv.slots[0] = Some(ItemStack {
            id: WOOD,
            count: 5,
            prefix: Prefix(0),
        });
        s.conns[0].inv.slots[1] = Some(ItemStack {
            id: IRON_BAR,
            count: 3,
            prefix: Prefix(0),
        });

        // Pick up WOOD from slot 0 and click SlotRef::Trash.
        c.send(ClientMsg::ClickSlot {
            at: SlotRef::Inv(0),
            kind: ClickKind::Left,
        })
        .unwrap();
        c.send(ClientMsg::ClickSlot {
            at: SlotRef::Trash,
            kind: ClickKind::Left,
        })
        .unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(s.conns[0].held.0, None);
        assert_eq!(
            s.conns[0].trash.map(|st| (st.id, st.count)),
            Some((WOOD, 5))
        );
        assert!(got.iter().any(|m| matches!(
            m,
            ServerMsg::Trash(Some(st)) if st.id == WOOD && st.count == 5
        )));

        // Second click on SlotRef::Trash with empty cursor retrieves WOOD back to held, then place back in slot 0.
        c.send(ClientMsg::ClickSlot {
            at: SlotRef::Trash,
            kind: ClickKind::Left,
        })
        .unwrap();
        c.send(ClientMsg::ClickSlot {
            at: SlotRef::Inv(0),
            kind: ClickKind::Left,
        })
        .unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(s.conns[0].trash, None);
        assert_eq!(
            s.conns[0].inv.slots[0].map(|st| (st.id, st.count)),
            Some((WOOD, 5))
        );

        // Shift-trash slot 0 moves WOOD into trash and clears bag slot 0.
        c.send(ClientMsg::TrashShift { slot: 0 }).unwrap();
        s.tick();
        drain(&c, &mut got);
        assert_eq!(s.conns[0].inv.slots[0], None);
        assert_eq!(
            s.conns[0].trash.map(|st| (st.id, st.count)),
            Some((WOOD, 5))
        );
        assert!(got.iter().any(|m| matches!(
            m,
            ServerMsg::Trash(Some(st)) if st.id == WOOD && st.count == 5
        )));
    }
}
