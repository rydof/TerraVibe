//! World persistence: plain little-endian files, one directory per world.
//!
//! Layout: `<root>/<world>/world.bin` (header), `chunks/<cx>_<cy>_<cz>.bin`
//! (edited chunks only), `players/<name>.bin`. Every write goes to a `.tmp`
//! sibling and is renamed into place, so a crash never leaves a torn file.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use terravibe_sim::chest::ChestInv;
use terravibe_sim::codec::{DecodeError, Reader, Writer};
use terravibe_sim::facts::{WorldFacts, decode_world_facts, encode_world_facts};
use terravibe_sim::inventory::{BAG, Inventory, SLOTS};
use terravibe_sim::item::ItemId;
use terravibe_sim::town_npc::TownKind;
use terravibe_sim::weather::{WeatherKind, WeatherState};
use terravibe_sim::{Chunk, ChunkPos, WorldSize};

/// v2 = Stage 5: adds `chests.bin` and the trailing potion-delay field. Both are
/// optional on read, so v1 files (Task 1 saves) load unchanged.
/// v3 = Stage 6: chunk files may end with the wall bitset (absent = no walls), so
/// v1/v2 chunk files load unchanged; we always write v3. `towns.bin` (town NPC
/// homes) is a new optional file, so it needs no version bump: worlds without it
/// load with no residents and the Guide is spawned again.
/// v4 = 58-slot player inventory (50 bag + 4 coin + 4 ammo); chunk files may
/// end with tag 2 liquid layer (absent = no liquid); v1..=3 files load unchanged.
const VERSION: u8 = 4;
const OLDEST_VERSION: u8 = 1;
const WORLD_MAGIC: &[u8; 4] = b"TVWD";
const CHUNK_MAGIC: &[u8; 4] = b"TVCK";
const PLAYER_MAGIC: &[u8; 4] = b"TVPL";
const CHEST_MAGIC: &[u8; 4] = b"TVCH";
const TOWN_MAGIC: &[u8; 4] = b"TVTN";
const WEATHER_MAGIC: &[u8; 4] = b"TVWT";
const LOCK_MAGIC: &[u8; 4] = b"TVLK";
const LOCK_VERSION: u8 = 1;

#[derive(Debug, PartialEq, Eq)]
pub enum SaveError {
    BadMagic,
    UnknownVersion(u8),
    Corrupt,
}

impl From<DecodeError> for SaveError {
    fn from(_: DecodeError) -> Self {
        SaveError::Corrupt
    }
}

impl From<SaveError> for io::Error {
    fn from(e: SaveError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}"))
    }
}

/// Persistent part of a player. Keyed by the `Hello` name.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSave {
    pub inventory: Inventory,
    pub armor: [Option<ItemId>; 3],
    pub life_max: i32,
    pub life: i32,
    pub pos: [f32; 3],
    /// Remaining Potion Sickness ticks (optional trailing field; absent in older files = 0).
    pub potion_delay: i32,
}

#[derive(Clone)]
pub struct WorldSnapshot {
    pub seed: u32,
    pub clock_tick: u32,
    pub size: WorldSize,
    /// Chunks to (re)write. Chunks already on disk and not listed are kept.
    pub chunks: Vec<(ChunkPos, Arc<Chunk>)>,
    pub players: Vec<(String, PlayerSave)>,
    /// Every chest, always written in full to `chests.bin` (small).
    pub chests: Vec<([i32; 3], ChestInv)>,
    /// Town NPC state, always written in full to `towns.bin`.
    pub towns: TownSave,
    /// World progression facts, written in full to `facts.bin`.
    pub facts: WorldFacts,
    /// Authoritative weather state, written in full to `weather.bin`.
    pub weather: WeatherState,
    /// Locked Dungeon chest coordinates requiring a Golden Key (`chest_locks.bin`).
    pub locked_chests: Vec<[i32; 3]>,
    /// Locked Underworld Shadow chest coordinates requiring a Shadow Key (`chest_locks.bin`).
    pub shadow_chests: Vec<[i32; 3]>,
}

/// Persisted town state: which NPCs live where. Houses without a resident are
/// rediscovered by the server's room scan, so only occupied ones are stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TownSave {
    pub guide_spawned: bool,
    /// `(kind, home cell, house origin)`.
    pub npcs: Vec<(TownKind, [i32; 3], [i32; 3])>,
}

pub fn encode_towns(t: &TownSave) -> Vec<u8> {
    let mut w = header(TOWN_MAGIC);
    w.u8(t.guide_spawned as u8);
    w.u32(t.npcs.len() as u32);
    for (k, home, origin) in &t.npcs {
        w.u8(k.to_u8());
        for v in home.iter().chain(origin) {
            w.i32(*v);
        }
    }
    w.0
}

pub fn decode_towns(bytes: &[u8]) -> Result<TownSave, SaveError> {
    let mut r = check_header(TOWN_MAGIC, bytes)?;
    let guide_spawned = r.u8()? != 0;
    let n = r.u32()?;
    let mut npcs = Vec::new();
    for _ in 0..n {
        let k = TownKind::from_u8(r.u8()?).ok_or(SaveError::Corrupt)?;
        let home = [r.i32()?, r.i32()?, r.i32()?];
        let origin = [r.i32()?, r.i32()?, r.i32()?];
        npcs.push((k, home, origin));
    }
    if !r.is_empty() {
        return Err(SaveError::Corrupt);
    }
    Ok(TownSave {
        guide_spawned,
        npcs,
    })
}

pub fn encode_weather(ws: &WeatherState) -> Vec<u8> {
    let mut w = header(WEATHER_MAGIC);
    w.u8(ws.kind.to_u8());
    w.f32(ws.intensity);
    w.u32(ws.ticks_remaining);
    w.0
}

pub fn decode_weather(bytes: &[u8]) -> Result<WeatherState, SaveError> {
    let mut r = check_header(WEATHER_MAGIC, bytes)?;
    let kind = WeatherKind::from_u8(r.u8()?).ok_or(SaveError::Corrupt)?;
    let intensity = r.f32()?;
    if !intensity.is_finite() || !(0.0..=1.0).contains(&intensity) {
        return Err(SaveError::Corrupt);
    }
    let ticks_remaining = r.u32()?;
    if !r.is_empty() {
        return Err(SaveError::Corrupt);
    }
    Ok(WeatherState {
        kind,
        intensity,
        ticks_remaining,
    })
}

pub fn encode_chest_locks(locked: &[[i32; 3]], shadow: &[[i32; 3]]) -> Vec<u8> {
    let mut w = Writer::default();
    w.bytes(LOCK_MAGIC);
    w.u8(LOCK_VERSION);
    w.u32(locked.len() as u32);
    for p in locked {
        for v in *p {
            w.i32(v);
        }
    }
    w.u32(shadow.len() as u32);
    for p in shadow {
        for v in *p {
            w.i32(v);
        }
    }
    w.0
}

pub fn decode_chest_locks(bytes: &[u8]) -> Result<(Vec<[i32; 3]>, Vec<[i32; 3]>), SaveError> {
    let mut r = Reader(bytes);
    if r.take(4)? != LOCK_MAGIC {
        return Err(SaveError::BadMagic);
    }
    let ver = r.u8()?;
    if ver != LOCK_VERSION {
        return Err(SaveError::UnknownVersion(ver));
    }
    let read_list = |r: &mut Reader<'_>| -> Result<Vec<[i32; 3]>, SaveError> {
        let n = r.u32()? as usize;
        if n > 100_000 {
            return Err(SaveError::Corrupt);
        }
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push([r.i32()?, r.i32()?, r.i32()?]);
        }
        Ok(out)
    };
    let locked = read_list(&mut r)?;
    let shadow = read_list(&mut r)?;
    if !r.is_empty() {
        return Err(SaveError::Corrupt);
    }
    Ok((locked, shadow))
}

fn header(magic: &[u8; 4]) -> Writer {
    let mut w = Writer::default();
    w.bytes(magic);
    w.u8(VERSION);
    w
}

fn check_header<'a>(magic: &[u8; 4], bytes: &'a [u8]) -> Result<Reader<'a>, SaveError> {
    let mut r = Reader(bytes);
    if r.take(4)? != magic {
        return Err(SaveError::BadMagic);
    }
    match r.u8()? {
        OLDEST_VERSION..=VERSION => Ok(r),
        v => Err(SaveError::UnknownVersion(v)),
    }
}

pub fn encode_chunk(c: &Chunk) -> Vec<u8> {
    let mut w = header(CHUNK_MAGIC);
    w.bytes(&c.to_bytes());
    w.0
}

/// Rejects a wrong magic/version or a run list that does not fill 32^3 exactly.
pub fn decode_chunk(bytes: &[u8]) -> Result<Chunk, SaveError> {
    let r = check_header(CHUNK_MAGIC, bytes)?;
    Ok(Chunk::from_bytes(r.0)?)
}

impl PlayerSave {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = header(PLAYER_MAGIC);
        w.bytes(&self.inventory.to_bytes());
        for a in self.armor {
            w.u16(a.map_or(u16::MAX, |i| i.0));
        }
        w.i32(self.life_max);
        w.i32(self.life);
        for v in self.pos {
            w.f32(v);
        }
        w.i32(self.potion_delay);
        w.0
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<PlayerSave, SaveError> {
        let mut r = check_header(PLAYER_MAGIC, bytes)?;
        // check_header validated byte 4 as the version.
        let slots = if bytes[4] <= 3 { BAG } else { SLOTS };
        let inv = Inventory::read(&mut r, slots)?;
        let mut armor = [None; 3];
        for a in armor.iter_mut() {
            let v = r.u16()?;
            *a = (v != u16::MAX).then_some(ItemId(v));
        }
        let life_max = r.i32()?;
        let life = r.i32()?;
        let pos = [r.f32()?, r.f32()?, r.f32()?];
        let potion_delay = if r.is_empty() { 0 } else { r.i32()?.max(0) };
        if !r.is_empty() {
            return Err(SaveError::Corrupt);
        }
        Ok(PlayerSave {
            inventory: inv,
            armor,
            life_max,
            life,
            pos,
            potion_delay,
        })
    }
}

pub fn encode_chests(chests: &[([i32; 3], ChestInv)]) -> Vec<u8> {
    let mut w = header(CHEST_MAGIC);
    w.u32(chests.len() as u32);
    for (pos, c) in chests {
        for v in pos {
            w.i32(*v);
        }
        c.write(&mut w);
    }
    w.0
}

pub fn decode_chests(bytes: &[u8]) -> Result<Vec<([i32; 3], ChestInv)>, SaveError> {
    let mut r = check_header(CHEST_MAGIC, bytes)?;
    let n = r.u32()?;
    let mut out = Vec::new();
    for _ in 0..n {
        let pos = [r.i32()?, r.i32()?, r.i32()?];
        out.push((pos, ChestInv::read(&mut r)?));
    }
    if !r.is_empty() {
        return Err(SaveError::Corrupt);
    }
    Ok(out)
}

/// Keeps names to a safe alphabet: trimmed, ASCII letters/digits/space/`_`/`-`,
/// capped at 32; interior spaces survive (menu world names). Empty names fall
/// back to `default`.
pub fn sanitize(name: &str, default: &str) -> String {
    let filtered: String = name
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-'))
        .take(32)
        .collect();
    let s = filtered.trim();
    if s.is_empty() {
        default.into()
    } else {
        s.into()
    }
}

/// The `world.bin` header the catalogue and `SaveDir::save_world` both write,
/// byte-identical: magic, version, seed, clock, size.
pub(crate) fn encode_world_header(seed: u32, clock_tick: u32, size: WorldSize) -> Vec<u8> {
    let mut w = header(WORLD_MAGIC);
    w.u32(seed);
    w.u32(clock_tick);
    w.u8(size.to_u8());
    w.0
}

/// Reads a `world.bin` header: `(seed, clock_tick, size)`. Older headers without
/// a trailing size byte load as `WorldSize::Small`.
pub(crate) fn decode_world_header(bytes: &[u8]) -> Result<(u32, u32, WorldSize), SaveError> {
    let (_, seed, clock_tick, size) = decode_world_header_versioned(bytes)?;
    Ok((seed, clock_tick, size))
}

fn decode_world_header_versioned(bytes: &[u8]) -> Result<(u8, u32, u32, WorldSize), SaveError> {
    let mut r = check_header(WORLD_MAGIC, bytes)?;
    let ver = bytes[4];
    let seed = r.u32()?;
    let clock_tick = r.u32()?;
    let size = if r.is_empty() {
        WorldSize::Small
    } else {
        WorldSize::from_u8(r.u8()?).ok_or(SaveError::Corrupt)?
    };
    if !r.is_empty() {
        return Err(SaveError::Corrupt);
    }
    Ok((ver, seed, clock_tick, size))
}

/// Returns the `.bak` sibling path for `path` (e.g. `world.bin` -> `world.bin.bak`).
pub fn backup_path(path: &Path) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(".bak");
    PathBuf::from(os)
}

/// Copies an existing primary file to its `.bak` sibling before `atomic_write` replaces it.
/// No-op if `path` does not exist.
pub fn rotate_backup(path: &Path) -> io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    std::fs::copy(path, backup_path(path))?;
    Ok(())
}

/// Upgrades an older `WorldSnapshot` in-place to the current save format version.
/// Returns `true` if any migration was applied.
pub fn migrate_world_version(from_ver: u32, snap: &mut WorldSnapshot) -> bool {
    if from_ver == 0 || from_ver >= VERSION as u32 {
        return false;
    }
    if !snap.weather.intensity.is_finite() {
        snap.weather = WeatherState::default();
    } else {
        snap.weather.intensity = snap.weather.intensity.clamp(0.0, 1.0);
    }
    true
}

static ATOMIC_WRITE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Write `bytes` to `path` via a temp file and rename; a failed write leaves
/// the old file (if any) untouched and no partial file behind.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let id = ATOMIC_WRITE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("tmp.{id}"));
    let res = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

#[derive(Clone, Debug)]
pub struct SaveDir(PathBuf);

impl SaveDir {
    /// `<root>/<world_name>`, created if missing.
    pub fn open(root: &Path, world_name: &str) -> io::Result<Self> {
        let dir = root.join(sanitize(world_name, "world1"));
        std::fs::create_dir_all(dir.join("chunks"))?;
        std::fs::create_dir_all(dir.join("players"))?;
        Ok(SaveDir(dir))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn save_world(&self, snap: &WorldSnapshot) -> io::Result<()> {
        for (p, c) in &snap.chunks {
            let f = self.0.join(format!("chunks/{}_{}_{}.bin", p.x, p.y, p.z));
            atomic_write(&f, &encode_chunk(c))?;
        }
        for (name, p) in &snap.players {
            let f = self
                .0
                .join(format!("players/{}.bin", sanitize(name, "player")));
            rotate_backup(&f)?;
            atomic_write(&f, &p.to_bytes())?;
        }
        let chests_path = self.0.join("chests.bin");
        rotate_backup(&chests_path)?;
        atomic_write(&chests_path, &encode_chests(&snap.chests))?;

        let towns_path = self.0.join("towns.bin");
        rotate_backup(&towns_path)?;
        atomic_write(&towns_path, &encode_towns(&snap.towns))?;

        let facts_path = self.0.join("facts.bin");
        rotate_backup(&facts_path)?;
        atomic_write(&facts_path, &encode_world_facts(&snap.facts))?;

        let weather_path = self.0.join("weather.bin");
        rotate_backup(&weather_path)?;
        atomic_write(&weather_path, &encode_weather(&snap.weather))?;

        let locks_path = self.0.join("chest_locks.bin");
        rotate_backup(&locks_path)?;
        atomic_write(
            &locks_path,
            &encode_chest_locks(&snap.locked_chests, &snap.shadow_chests),
        )?;

        // Header last: a world.bin always describes a fully written save.
        let world_path = self.0.join("world.bin");
        rotate_backup(&world_path)?;
        atomic_write(
            &world_path,
            &encode_world_header(snap.seed, snap.clock_tick, snap.size),
        )
    }

    /// `Ok(None)` for a fresh directory; `Err` when the header is unusable.
    /// Corrupt chunk or player files are skipped (chunks regenerate).
    pub fn load_world(&self) -> io::Result<Option<WorldSnapshot>> {
        let bytes = match std::fs::read(self.0.join("world.bin")) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let (seed, clock_tick, size) = decode_world_header(&bytes).map_err(io::Error::from)?;
        let mut chunks = Vec::new();
        for e in std::fs::read_dir(self.0.join("chunks"))?.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(pos) = parse_chunk_name(&name) else {
                continue;
            };
            if let Ok(c) = std::fs::read(e.path())
                .map_err(|_| SaveError::Corrupt)
                .and_then(|b| decode_chunk(&b))
            {
                chunks.push((pos, Arc::new(c)));
            }
        }
        let mut players = Vec::new();
        for e in std::fs::read_dir(self.0.join("players"))?.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(".bin") else {
                continue;
            };
            if let Ok(p) = std::fs::read(e.path())
                .map_err(|_| SaveError::Corrupt)
                .and_then(|b| PlayerSave::from_bytes(&b))
            {
                players.push((stem.to_string(), p));
            }
        }
        // A corrupt chests file loses chest contents but never blocks loading.
        let chests = std::fs::read(self.0.join("chests.bin"))
            .ok()
            .and_then(|b| decode_chests(&b).ok())
            .unwrap_or_default();
        // Missing or corrupt: no residents, the Guide respawns.
        let towns = std::fs::read(self.0.join("towns.bin"))
            .ok()
            .and_then(|b| decode_towns(&b).ok())
            .unwrap_or_default();
        // Missing or corrupt: default world facts.
        let facts = std::fs::read(self.0.join("facts.bin"))
            .ok()
            .and_then(|b| decode_world_facts(&b).ok())
            .unwrap_or_default();
        // Missing or corrupt: default Clear weather.
        let weather = std::fs::read(self.0.join("weather.bin"))
            .ok()
            .and_then(|b| decode_weather(&b).ok())
            .unwrap_or_default();
        let (locked_chests, shadow_chests) = std::fs::read(self.0.join("chest_locks.bin"))
            .ok()
            .and_then(|b| decode_chest_locks(&b).ok())
            .unwrap_or_default();
        Ok(Some(WorldSnapshot {
            seed,
            clock_tick,
            size,
            chunks,
            players,
            chests,
            towns,
            facts,
            weather,
            locked_chests,
            shadow_chests,
        }))
    }
}

fn read_with_backup<T, E>(
    path: &Path,
    decode: impl Fn(&[u8]) -> Result<T, E>,
) -> (Option<T>, bool) {
    if let Ok(bytes) = std::fs::read(path) {
        if let Ok(val) = decode(&bytes) {
            return (Some(val), false);
        }
    }
    if let Ok(bytes) = std::fs::read(backup_path(path)) {
        if let Ok(val) = decode(&bytes) {
            return (Some(val), true);
        }
    }
    (None, false)
}

/// Returns `true` if a valid `.bak` backup of `world.bin` exists for `name` under `root`.
pub fn has_world_backup(root: &Path, name: &str) -> bool {
    let dir = root.join(sanitize(name, "world1"));
    let bak = backup_path(&dir.join("world.bin"));
    std::fs::read(bak)
        .ok()
        .is_some_and(|b| decode_world_header(&b).is_ok())
}

/// Loads `name` under `root`, falling back to `.bak` files when a primary file is
/// missing or corrupt. Returns `(snapshot, used_backup)`.
pub fn load_world_with_recovery(root: &Path, name: &str) -> io::Result<(WorldSnapshot, bool)> {
    let dir = root.join(sanitize(name, "world1"));
    let world_path = dir.join("world.bin");
    let (header_opt, mut used_backup) =
        read_with_backup(&world_path, decode_world_header_versioned);
    let (from_ver, seed, clock_tick, size) = match header_opt {
        Some(h) => h,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "primary world.bin and .bak backup are missing or corrupt",
            ));
        }
    };

    let mut chunks = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join("chunks")) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(pos) = parse_chunk_name(&name) else {
                continue;
            };
            if let Ok(c) = std::fs::read(e.path())
                .map_err(|_| SaveError::Corrupt)
                .and_then(|b| decode_chunk(&b))
            {
                chunks.push((pos, Arc::new(c)));
            }
        }
    }

    let mut players = Vec::new();
    let mut seen_players = std::collections::HashSet::new();
    if let Ok(entries) = std::fs::read_dir(dir.join("players")) {
        for e in entries.flatten() {
            let fname = e.file_name().to_string_lossy().into_owned();
            let stem = if let Some(s) = fname.strip_suffix(".bin") {
                s.to_string()
            } else if let Some(s) = fname.strip_suffix(".bin.bak") {
                s.to_string()
            } else {
                continue;
            };
            if !seen_players.insert(stem.clone()) {
                continue;
            }
            let primary = dir.join(format!("players/{stem}.bin"));
            let (p_opt, p_bak) = read_with_backup(&primary, PlayerSave::from_bytes);
            if let Some(p) = p_opt {
                used_backup |= p_bak;
                players.push((stem, p));
            }
        }
    }

    let (chests_opt, c_bak) = read_with_backup(&dir.join("chests.bin"), decode_chests);
    used_backup |= c_bak;
    let chests = chests_opt.unwrap_or_default();

    let (towns_opt, t_bak) = read_with_backup(&dir.join("towns.bin"), decode_towns);
    used_backup |= t_bak;
    let towns = towns_opt.unwrap_or_default();

    let (facts_opt, f_bak) = read_with_backup(&dir.join("facts.bin"), |b| {
        decode_world_facts(b).map_err(|_| SaveError::Corrupt)
    });
    used_backup |= f_bak;
    let facts = facts_opt.unwrap_or_default();

    let (weather_opt, w_bak) = read_with_backup(&dir.join("weather.bin"), decode_weather);
    used_backup |= w_bak;
    let weather = weather_opt.unwrap_or_default();

    let (locks_opt, l_bak) = read_with_backup(&dir.join("chest_locks.bin"), decode_chest_locks);
    used_backup |= l_bak;
    let (locked_chests, shadow_chests) = locks_opt.unwrap_or_default();

    let mut snap = WorldSnapshot {
        seed,
        clock_tick,
        size,
        chunks,
        players,
        chests,
        towns,
        facts,
        weather,
        locked_chests,
        shadow_chests,
    };
    migrate_world_version(from_ver as u32, &mut snap);
    Ok((snap, used_backup))
}

/// Recovers a world from its `.bak` backup and writes the recovered snapshot back
/// to the primary save files so subsequent loads succeed normally.
pub fn restore_world_backup(root: &Path, name: &str) -> io::Result<WorldSnapshot> {
    let (snap, _) = load_world_with_recovery(root, name)?;
    let dir = SaveDir::open(root, name)?;
    dir.save_world(&snap)?;
    Ok(snap)
}

fn parse_chunk_name(name: &str) -> Option<ChunkPos> {
    let mut it = name.strip_suffix(".bin")?.split('_');
    let p = ChunkPos {
        x: it.next()?.parse().ok()?,
        y: it.next()?.parse().ok()?,
        z: it.next()?.parse().ok()?,
    };
    it.next().is_none().then_some(p)
}

/// Where and under which name the server persists its world.
#[derive(Clone, Debug)]
pub struct SaveCfg {
    pub root: PathBuf,
    pub world: String,
}

impl SaveCfg {
    /// `--world <name>` / `TERRAVIBE_WORLD` (default `world1`), `--save-dir
    /// <dir>` / `TERRAVIBE_SAVES` (default: the user data dir, never the repo).
    pub fn from_args_env(args: &[String], env: impl Fn(&str) -> Option<String>) -> SaveCfg {
        let flag = |f: &str| {
            args.iter()
                .position(|a| a == f)
                .and_then(|i| args.get(i + 1).cloned())
        };
        let world = flag("--world")
            .or_else(|| env("TERRAVIBE_WORLD"))
            .unwrap_or_else(|| "world1".into());
        let root = flag("--save-dir")
            .or_else(|| env("TERRAVIBE_SAVES"))
            .map(PathBuf::from)
            .unwrap_or_else(|| default_save_root(&env));
        SaveCfg { root, world }
    }
}

/// `$XDG_DATA_HOME/terravibe/saves`, else `$HOME/.local/share/terravibe/saves`.
fn default_save_root(env: &impl Fn(&str) -> Option<String>) -> PathBuf {
    let base = env("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .or_else(|| env("APPDATA").map(PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    base.join("terravibe/saves")
}

/// Runs snapshot writes on a worker thread so the 60 Hz tick never blocks.
pub struct Autosaver {
    dir: SaveDir,
    busy: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Autosaver {
    pub fn new(dir: SaveDir) -> Self {
        Autosaver {
            dir,
            busy: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }

    pub fn dir(&self) -> &SaveDir {
        &self.dir
    }

    /// Hand the snapshot to the worker; false (snapshot dropped) when the
    /// previous write is still running.
    pub fn try_save(&mut self, snap: WorldSnapshot) -> bool {
        if self.busy.swap(true, Ordering::AcqRel) {
            return false;
        }
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
        let (dir, busy) = (self.dir.clone(), Arc::clone(&self.busy));
        self.worker = Some(std::thread::spawn(move || {
            if let Err(e) = dir.save_world(&snap) {
                eprintln!("terravibe: autosave failed: {e}");
            }
            busy.store(false, Ordering::Release);
        }));
        true
    }

    /// Wait for any running write, then write `snap` on this thread.
    pub fn flush(&mut self, snap: &WorldSnapshot) -> io::Result<()> {
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
        self.dir.save_world(snap)
    }
}

impl Drop for Autosaver {
    fn drop(&mut self) {
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }
}

/// Players by name, for restoring on join.
pub fn players_map(v: Vec<(String, PlayerSave)>) -> HashMap<String, PlayerSave> {
    v.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::Liquid;
    use terravibe_sim::block::{AIR, DIRT, STONE};
    use terravibe_sim::inventory::ItemStack;
    use terravibe_sim::item::{COPPER_PICKAXE, Prefix, Rng};

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tv_save_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn sample_chunk() -> Chunk {
        let mut c = Chunk::filled(STONE);
        c.set(1, 2, 3, AIR);
        c.set(31, 31, 31, DIRT);
        c.set(0, 0, 0, DIRT);
        c
    }

    #[test]
    fn walls_roundtrip_and_v2_chunk_loads() {
        let mut c = sample_chunk();
        c.set_wall(4, 5, 6, true);
        let d = decode_chunk(&encode_chunk(&c)).unwrap();
        assert!(d.wall(4, 5, 6));
        assert_eq!(d, c);
        // A v2 file: version byte 2, runs only.
        let mut old = encode_chunk(&sample_chunk());
        old[4] = 2;
        let o = decode_chunk(&old).unwrap();
        assert!(!o.has_walls());
        assert_eq!(o, sample_chunk());
    }

    #[test]
    fn chunk_roundtrip_is_exact() {
        let c = sample_chunk();
        assert_eq!(decode_chunk(&encode_chunk(&c)).unwrap(), c);
        // Uniform chunk compresses to a single run.
        assert!(encode_chunk(&Chunk::filled(AIR)).len() < 16);
        // Wrong size is rejected.
        let mut bad = encode_chunk(&c);
        bad.truncate(bad.len() - 6);
        assert_eq!(decode_chunk(&bad), Err(SaveError::Corrupt));
        assert_eq!(decode_chunk(b"TVCK\x01"), Err(SaveError::Corrupt));
    }

    #[test]
    fn v3_chunk_file_loads_without_liquid() {
        let mut c = sample_chunk();
        c.set_wall(2, 3, 4, true);
        // Hand-build v3 header (b"TVCK\x03") + old format bytes
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"TVCK\x03");
        bytes.extend_from_slice(&c.to_bytes());

        let loaded = decode_chunk(&bytes).unwrap();
        assert!(!loaded.has_liquid());
        assert!(loaded.wall(2, 3, 4));
        assert_eq!(loaded, c);
    }

    #[test]
    fn v4_chunk_with_liquid_roundtrips() {
        let mut c = sample_chunk();
        c.set_wall(1, 1, 1, true);
        c.set_liquid(2, 2, 2, Some(Liquid::water(255)));
        c.set_liquid(3, 3, 3, Some(Liquid::lava(128)));

        let encoded = encode_chunk(&c);
        assert_eq!(&encoded[..5], b"TVCK\x04");

        let loaded = decode_chunk(&encoded).unwrap();
        assert_eq!(loaded, c);
        assert!(loaded.has_liquid());
        assert_eq!(loaded.liquid(2, 2, 2), Some(Liquid::water(255)));
        assert_eq!(loaded.liquid(3, 3, 3), Some(Liquid::lava(128)));
        assert!(loaded.wall(1, 1, 1));
    }

    fn sample_player() -> PlayerSave {
        let mut inv = Inventory::starting(&mut Rng::new(5));
        inv.slots[7] = Some(ItemStack {
            id: COPPER_PICKAXE,
            count: 1,
            prefix: Prefix(3),
        });
        inv.selected = 4;
        PlayerSave {
            inventory: inv,
            armor: [Some(ItemId(81)), None, Some(ItemId(82))],
            life_max: 120,
            life: 77,
            pos: [1.5, -20.25, 300.0],
            potion_delay: 1234,
        }
    }

    #[test]
    fn player_roundtrip_keeps_inventory_armor_life_max_pos() {
        let p = sample_player();
        assert_eq!(PlayerSave::from_bytes(&p.to_bytes()).unwrap(), p);
    }

    #[test]
    fn v3_fifty_slot_player_save_loads() {
        let item = |id: u16, count: u32| {
            Some(ItemStack {
                id: ItemId(id),
                count,
                prefix: Prefix(0),
            })
        };
        let mut w = Writer::default();
        w.bytes(PLAYER_MAGIC);
        w.u8(3);
        for i in 0..50 {
            w.stack(&match i {
                0 => item(1, 1),
                7 => item(2, 40),
                49 => item(3, 9),
                _ => None,
            });
        }
        w.u8(4);
        for a in [81u16, u16::MAX, 82] {
            w.u16(a);
        }
        w.i32(120);
        w.i32(77);
        for v in [1.5f32, -20.25, 300.0] {
            w.f32(v);
        }
        let p = PlayerSave::from_bytes(&w.0).unwrap();
        assert_eq!(p.inventory.slots[0], item(1, 1));
        assert_eq!(p.inventory.slots[7], item(2, 40));
        assert_eq!(p.inventory.slots[49], item(3, 9));
        assert!(p.inventory.slots[50..].iter().all(|s| s.is_none()));
        assert_eq!(p.inventory.selected, 4);
        assert_eq!(p.armor, [Some(ItemId(81)), None, Some(ItemId(82))]);
        assert_eq!((p.life_max, p.life, p.potion_delay), (120, 77, 0));
    }

    #[test]
    fn sickness_persists_in_player_save() {
        let p = sample_player();
        assert_eq!(
            PlayerSave::from_bytes(&p.to_bytes()).unwrap().potion_delay,
            1234
        );
        // A file written before the field existed loads with no sickness.
        let mut b = p.to_bytes();
        b.truncate(b.len() - 4);
        assert_eq!(PlayerSave::from_bytes(&b).unwrap().potion_delay, 0);
    }

    #[test]
    fn world_header_roundtrip_keeps_seed_and_clock() {
        let root = tmp("hdr");
        let d = SaveDir::open(&root, "w").unwrap();
        assert!(d.load_world().unwrap().is_none());
        let snap = WorldSnapshot {
            seed: 0xDEAD_BEEF,
            clock_tick: 54_321,
            size: WorldSize::Small,
            chunks: vec![(ChunkPos { x: -1, y: 2, z: 3 }, Arc::new(sample_chunk()))],
            players: vec![("bob".into(), sample_player())],
            chests: vec![],
            towns: TownSave::default(),
            facts: WorldFacts::default(),
            weather: WeatherState::default(),
            locked_chests: vec![],
            shadow_chests: vec![],
        };
        d.save_world(&snap).unwrap();
        let l = d.load_world().unwrap().unwrap();
        assert_eq!((l.seed, l.clock_tick), (0xDEAD_BEEF, 54_321));
        assert_eq!(l.chunks.len(), 1);
        assert_eq!(l.chunks[0].0, ChunkPos { x: -1, y: 2, z: 3 });
        assert_eq!(*l.chunks[0].1, sample_chunk());
        assert_eq!(l.players, vec![("bob".to_string(), sample_player())]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn town_npc_state_roundtrips_save() {
        let t = TownSave {
            guide_spawned: true,
            npcs: vec![
                (TownKind::Guide, [1, 2, 3], [-4, 5, 6]),
                (TownKind::Merchant, [7, 8, -9], [0, 0, 0]),
            ],
        };
        assert_eq!(decode_towns(&encode_towns(&t)), Ok(t.clone()));
        let root = tmp("town");
        let d = SaveDir::open(&root, "w").unwrap();
        let snap = WorldSnapshot {
            seed: 1,
            clock_tick: 0,
            size: WorldSize::Small,
            chunks: vec![],
            players: vec![],
            chests: vec![],
            towns: t.clone(),
            facts: WorldFacts::default(),
            weather: WeatherState::default(),
            locked_chests: vec![],
            shadow_chests: vec![],
        };
        d.save_world(&snap).unwrap();
        assert_eq!(d.load_world().unwrap().unwrap().towns, t);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn v2_world_loads_and_respawns_guide() {
        // A world saved before towns existed has no towns.bin: it loads, with no
        // residents and `guide_spawned == false` (the server then spawns the Guide).
        let root = tmp("v2town");
        let d = SaveDir::open(&root, "w").unwrap();
        let snap = WorldSnapshot {
            seed: 9,
            clock_tick: 5,
            size: WorldSize::Small,
            chunks: vec![],
            players: vec![],
            chests: vec![],
            towns: TownSave::default(),
            facts: WorldFacts::default(),
            weather: WeatherState::default(),
            locked_chests: vec![],
            shadow_chests: vec![],
        };
        d.save_world(&snap).unwrap();
        std::fs::remove_file(d.path().join("towns.bin")).unwrap();
        let l = d.load_world().unwrap().unwrap();
        assert_eq!(l.towns, TownSave::default());
        assert!(!l.towns.guide_spawned);
        // A corrupt file also falls back without error.
        std::fs::write(d.path().join("towns.bin"), b"junk").unwrap();
        assert_eq!(d.load_world().unwrap().unwrap().towns, TownSave::default());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn world_facts_roundtrip_and_missing_fallback() {
        let root = tmp("facts");
        let d = SaveDir::open(&root, "w").unwrap();
        let snap = WorldSnapshot {
            seed: 42,
            clock_tick: 100,
            size: WorldSize::Small,
            chunks: vec![],
            players: vec![],
            chests: vec![],
            towns: TownSave::default(),
            facts: WorldFacts::default(),
            weather: WeatherState::default(),
            locked_chests: vec![],
            shadow_chests: vec![],
        };
        d.save_world(&snap).unwrap();
        // 1. Missing facts.bin returns defaults
        std::fs::remove_file(d.path().join("facts.bin")).unwrap();
        let l = d.load_world().unwrap().unwrap();
        assert_eq!(l.facts, WorldFacts::default());

        // 2. Save with custom facts and reload
        let mut snap2 = snap;
        snap2.facts.downed_eye = true;
        snap2.facts.underworld_reached = true;
        snap2.facts.shadow_orbs_broken = 5;
        d.save_world(&snap2).unwrap();

        let loaded = d.load_world().unwrap().unwrap();
        assert_eq!(loaded.facts.downed_eye, true);
        assert_eq!(loaded.facts.underworld_reached, true);
        assert_eq!(loaded.facts.shadow_orbs_broken, 5);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_chunk_file_is_ignored_and_regenerates() {
        let root = tmp("corrupt");
        let d = SaveDir::open(&root, "w").unwrap();
        let snap = WorldSnapshot {
            seed: 1,
            clock_tick: 0,
            size: WorldSize::Small,
            chunks: vec![
                (ChunkPos { x: 0, y: 0, z: 0 }, Arc::new(sample_chunk())),
                (ChunkPos { x: 1, y: 0, z: 0 }, Arc::new(sample_chunk())),
            ],
            players: vec![],
            chests: vec![],
            towns: TownSave::default(),
            facts: WorldFacts::default(),
            weather: WeatherState::default(),
            locked_chests: vec![],
            shadow_chests: vec![],
        };
        d.save_world(&snap).unwrap();
        std::fs::write(d.path().join("chunks/1_0_0.bin"), b"garbage").unwrap();
        let l = d.load_world().unwrap().unwrap();
        assert_eq!(l.chunks.len(), 1);
        assert_eq!(l.chunks[0].0, ChunkPos { x: 0, y: 0, z: 0 });
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unknown_version_is_rejected() {
        let root = tmp("ver");
        let d = SaveDir::open(&root, "w").unwrap();
        let mut w = header(WORLD_MAGIC);
        w.0[4] = 99;
        w.u32(1);
        w.u32(2);
        std::fs::write(d.path().join("world.bin"), &w.0).unwrap();
        assert!(d.load_world().is_err());
        assert_eq!(
            decode_chunk(b"TVCK\x63").unwrap_err(),
            SaveError::UnknownVersion(99)
        );
        assert_eq!(
            PlayerSave::from_bytes(b"TVPL\x63").unwrap_err(),
            SaveError::UnknownVersion(99)
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn atomic_write_leaves_no_partial_file() {
        use std::os::unix::fs::PermissionsExt;
        let root = tmp("atomic");
        std::fs::create_dir_all(&root).unwrap();
        let f = root.join("a.bin");
        atomic_write(&f, b"old").unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o555)).unwrap();
        let res = atomic_write(&f, b"new");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        if res.is_ok() {
            // Running as a user that ignores permissions (root): nothing to assert.
            let _ = std::fs::remove_dir_all(root);
            return;
        }
        assert_eq!(std::fs::read(&f).unwrap(), b"old");
        let names: Vec<_> = std::fs::read_dir(&root).unwrap().flatten().collect();
        assert_eq!(names.len(), 1, "no tmp file left behind");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sanitize_keeps_interior_spaces() {
        assert_eq!(sanitize("  My  World ", "w"), "My  World");
        assert_eq!(sanitize("\u{7}", "w"), "w");
    }

    #[test]
    fn world_name_selects_directory() {
        let root = tmp("name");
        let d = SaveDir::open(&root, "alpha").unwrap();
        assert_eq!(d.path(), root.join("alpha"));
        assert_eq!(
            SaveDir::open(&root, "../evil").unwrap().path(),
            root.join("evil")
        );
        let env = |k: &str| (k == "TERRAVIBE_WORLD").then(|| "beta".to_string());
        let cfg = SaveCfg::from_args_env(&[], env);
        assert_eq!(cfg.world, "beta");
        assert!(cfg.root.ends_with("terravibe/saves"));
        let args: Vec<String> = ["--world", "gamma", "--save-dir", "/x"]
            .map(String::from)
            .into();
        let cfg = SaveCfg::from_args_env(&args, env);
        assert_eq!(
            (cfg.world.as_str(), cfg.root),
            ("gamma", PathBuf::from("/x"))
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn v1_save_loads_into_v2() {
        let root = tmp("v1");
        let d = SaveDir::open(&root, "w").unwrap();
        let snap = WorldSnapshot {
            seed: 9,
            clock_tick: 77,
            size: WorldSize::Small,
            chunks: vec![(ChunkPos { x: 0, y: 1, z: 2 }, Arc::new(sample_chunk()))],
            players: vec![("bob".into(), sample_player())],
            chests: vec![],
            towns: TownSave::default(),
            facts: WorldFacts::default(),
            weather: WeatherState::default(),
            locked_chests: vec![],
            shadow_chests: vec![],
        };
        d.save_world(&snap).unwrap();
        // Rewrite every file as a Task 1 save: version byte 1, no chests.bin, no
        // trailing potion-delay field in the player file.
        let down = |p: std::path::PathBuf, trim: usize| {
            let mut b = std::fs::read(&p).unwrap();
            b[4] = 1;
            b.truncate(b.len() - trim);
            std::fs::write(p, b).unwrap();
        };
        down(d.path().join("world.bin"), 1);
        down(d.path().join("chunks/0_1_2.bin"), 0);
        down(d.path().join("players/bob.bin"), 4);
        // ...and the old 50-slot inventory: drop the 8 empty coin/ammo slots.
        let pp = d.path().join("players/bob.bin");
        let mut b = std::fs::read(&pp).unwrap();
        let mut w = Writer::default();
        for s in &sample_player().inventory.slots[..50] {
            w.stack(s);
        }
        let at = 5 + w.0.len();
        b.drain(at..at + 8);
        std::fs::write(pp, b).unwrap();
        std::fs::remove_file(d.path().join("chests.bin")).unwrap();
        let l = d.load_world().unwrap().unwrap();
        assert_eq!((l.seed, l.clock_tick), (9, 77));
        assert_eq!(*l.chunks[0].1, sample_chunk());
        let mut want = sample_player();
        want.potion_delay = 0;
        assert_eq!(l.players, vec![("bob".to_string(), want)]);
        assert!(l.chests.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn weather_state_survives_world_save_and_reload() {
        let root = tmp("weather_persist");
        let d = SaveDir::open(&root, "w").unwrap();
        let want = WeatherState {
            kind: WeatherKind::Storm,
            intensity: 0.85,
            ticks_remaining: 4_200,
        };
        let snap = WorldSnapshot {
            seed: 123,
            clock_tick: 500,
            size: WorldSize::Small,
            chunks: vec![],
            players: vec![],
            chests: vec![],
            towns: TownSave::default(),
            facts: WorldFacts::default(),
            weather: want,
            locked_chests: vec![],
            shadow_chests: vec![],
        };
        d.save_world(&snap).unwrap();
        let loaded = d.load_world().unwrap().unwrap();
        assert_eq!(loaded.weather, want);

        // Missing weather.bin (older saves) loads as default Clear weather.
        std::fs::remove_file(d.path().join("weather.bin")).unwrap();
        let loaded_old = d.load_world().unwrap().unwrap();
        assert_eq!(loaded_old.weather, WeatherState::default());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn save_rotates_bak_and_recovers_from_corrupt_primary() {
        let root = tmp("bak_recover");
        let d = SaveDir::open(&root, "w").unwrap();
        let snap1 = WorldSnapshot {
            seed: 42,
            clock_tick: 100,
            size: WorldSize::Small,
            chunks: vec![],
            players: vec![("alice".into(), sample_player())],
            chests: vec![],
            towns: TownSave::default(),
            facts: WorldFacts::default(),
            weather: WeatherState {
                kind: WeatherKind::Rain,
                intensity: 0.5,
                ticks_remaining: 1000,
            },
            locked_chests: vec![],
            shadow_chests: vec![],
        };
        d.save_world(&snap1).unwrap();

        let snap2 = WorldSnapshot {
            clock_tick: 200,
            ..snap1.clone()
        };
        d.save_world(&snap2).unwrap();

        // .bak files must exist after the second save
        assert!(backup_path(&d.path().join("world.bin")).exists());
        assert!(backup_path(&d.path().join("weather.bin")).exists());
        assert!(backup_path(&d.path().join("towns.bin")).exists());
        assert!(backup_path(&d.path().join("players/alice.bin")).exists());
        assert!(backup_path(&d.path().join("chests.bin")).exists());

        // Corrupt primary world.bin and players/alice.bin
        std::fs::write(d.path().join("world.bin"), b"corrupt!").unwrap();
        std::fs::write(d.path().join("players/alice.bin"), b"badplayer").unwrap();

        let (recovered, used_backup) = load_world_with_recovery(&root, "w").unwrap();
        assert!(used_backup);
        assert_eq!(recovered.seed, 42);
        assert_eq!(recovered.clock_tick, 100);
        assert_eq!(recovered.players.len(), 1);
        assert_eq!(recovered.players[0].0, "alice");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn save_version_migration_upgrades_older_snapshot() {
        let mut snap = WorldSnapshot {
            seed: 7,
            clock_tick: 55,
            size: WorldSize::Small,
            chunks: vec![],
            players: vec![],
            chests: vec![],
            towns: TownSave::default(),
            facts: WorldFacts::default(),
            weather: WeatherState {
                kind: WeatherKind::Clear,
                intensity: f32::NAN,
                ticks_remaining: 0,
            },
            locked_chests: vec![],
            shadow_chests: vec![],
        };

        assert!(migrate_world_version(1, &mut snap));
        assert!(snap.weather.intensity.is_finite());
        assert!(!migrate_world_version(VERSION as u32, &mut snap));
    }

    #[test]
    fn chest_locks_roundtrip_and_missing_fallback() {
        let root = tmp("chest_locks");
        let d = SaveDir::open(&root, "w").unwrap();
        let locked = vec![[10, 20, 30], [-5, 12, 99]];
        let shadow = vec![[1, -40, 2]];
        let snap = WorldSnapshot {
            seed: 42,
            clock_tick: 100,
            size: WorldSize::Small,
            chunks: vec![],
            players: vec![],
            chests: vec![],
            towns: TownSave::default(),
            facts: WorldFacts::default(),
            weather: WeatherState::default(),
            locked_chests: locked.clone(),
            shadow_chests: shadow.clone(),
        };
        d.save_world(&snap).unwrap();
        let loaded = d.load_world().unwrap().unwrap();
        assert_eq!(loaded.locked_chests, locked);
        assert_eq!(loaded.shadow_chests, shadow);

        std::fs::remove_file(d.path().join("chest_locks.bin")).unwrap();
        let old = d.load_world().unwrap().unwrap();
        assert!(old.locked_chests.is_empty());
        assert!(old.shadow_chests.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
}
