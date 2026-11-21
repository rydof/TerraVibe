//! Sparse corruption patches, world-cell chasms and ebonstone-hosted demonite.

use super::noise::{fbm2, hash};
use super::ore::tile_runner;
use super::terrain::{WORLD_HEIGHT, inland_surface_height as surface_height_raw};
use crate::block::{
    AIR, CORRUPT_GRASS, DEMON_ALTAR, DEMONITE_ORE, DIRT, EBONSTONE, GRASS, HARDENED_SAND, ICE,
    JUNGLE_GRASS, MUD, SAND, SANDSTONE, SHADOW_ORB, SNOW, STONE,
};
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, WorldBounds, WorldSize};

#[cfg(test)]
fn surface_height(seed: u32, x: i32, z: i32) -> i32 {
    surface_height_raw(seed, x, z, WorldSize::Small)
}

const CS: usize = CHUNK_SIZE as usize;
const PAD: usize = CS + 2;
const CHASM_CELL: i32 = 64;
const CHASM_REACH: i32 = 16;
const SALT_CHASM: u32 = 0xC4A5_7711;
const SALT_DEMONITE: u32 = 0xDE30_017E;
const SALT_ALTAR: u32 = 0xD41A_A1A7;
const SALT_ORB: u32 = 0x58A3_08B1;
const ORE_CELL: i32 = 32;
// Gold's existing mid/deep rates are 1.15e-4 / 6e-5 veins per voxel.
const DEMONITE_DENSITY: f64 = 1.0e-4;
// At most eight steps (axis velocity <= 1), plus radius <= 4 * 1.15.
const ORE_REACH: i32 = 16;
/// `spawn_point` picks the highest of the nine columns around the origin, so
/// the real spawn is at most one block from `(0,0)`. A 303-block origin radius
/// therefore keeps every feature at least 300 blocks from that spawn.
const SPAWN_CLEAR: f64 = 303.0;

/// Two broad noise octaves give kilometre-scale islands, separated by roughly
/// 2–3 km. The threshold is hard: consumers can test `> 0.0` directly.
pub fn corruption_at(seed: u32, wx: i32, wz: i32) -> f32 {
    let (x, z) = (f64::from(wx), f64::from(wz));
    if x * x + z * z <= SPAWN_CLEAR * SPAWN_CLEAR {
        return 0.0;
    }
    let noise = fbm2(seed ^ 0xC077_0A11, x, z, 1.0 / 1000.0, 2);
    ((noise - 0.35) / 0.65).max(0.0) as f32
}

/// [`corruption_at`] within a finite world of `size`: excludes the ocean and beach
/// ring and ensures at least one deterministic corruption region fits inside bounds
/// even when the broad noise misses the inner square.
pub fn corruption_at_for_size(seed: u32, wx: i32, wz: i32, size: WorldSize) -> f32 {
    let bounds = WorldBounds::new(size);
    if bounds.contains_column(wx, wz) && bounds.edge_distance(wx, wz) < OCEAN_WIDTH + BEACH_WIDTH {
        return 0.0;
    }
    let base = corruption_at(seed, wx, wz);
    if base > 0.0 {
        return base;
    }
    let (x, z) = (f64::from(wx), f64::from(wz));
    if x * x + z * z <= SPAWN_CLEAR * SPAWN_CLEAR {
        return 0.0;
    }
    // Guaranteed corrupt region anchored inside the world's inland band.
    let h = hash(seed ^ 0xC077_5EE0, 0, 0, 0);
    let oct = (h & 7) as usize;
    const DIRS: [(f64, f64); 8] = [
        (1.0, 0.0),
        (0.707_106_78, 0.707_106_78),
        (0.0, 1.0),
        (-0.707_106_78, 0.707_106_78),
        (-1.0, 0.0),
        (-0.707_106_78, -0.707_106_78),
        (0.0, -1.0),
        (0.707_106_78, -0.707_106_78),
    ];
    let (ux, uz) = DIRS[oct];
    let r = f64::from(size.half_extent()) * 0.55;
    let (cx, cz) = (ux * r, uz * r);
    let dist2 = (x - cx) * (x - cx) + (z - cz) * (z - cz);
    let patch_r = 160.0;
    if dist2 < patch_r * patch_r {
        let d = dist2.sqrt() / patch_r;
        ((1.0 - d) as f32).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[derive(Clone, Copy)]
struct Column {
    corrupt: bool,
    surface: i32,
}

struct Chasm {
    x: i32,
    z: i32,
    along_x: bool,
    length: i32,
    width: i32,
    bottom: i32,
    ledge_phase: i32,
}

impl Chasm {
    fn offsets(&self, x: i32, z: i32) -> (i32, i32) {
        if self.along_x {
            (x - self.x, z - self.z)
        } else {
            (z - self.z, x - self.x)
        }
    }

    fn contains_column(&self, x: i32, z: i32) -> bool {
        let (long, short) = self.offsets(x, z);
        (-self.length / 2..(self.length + 1) / 2).contains(&long)
            && (-self.width / 2..(self.width + 1) / 2).contains(&short)
    }

    /// Inclusive world-space footprint, `long` along the chasm axis.
    fn bounds(&self) -> (i32, i32, i32, i32) {
        let (lx, lz) = if self.along_x {
            (self.length, self.width)
        } else {
            (self.width, self.length)
        };
        let (x0, z0) = (self.x - lx / 2, self.z - lz / 2);
        (x0, x0 + lx - 1, z0, z0 + lz - 1)
    }

    fn carves(&self, x: i32, y: i32, z: i32, surface: i32) -> bool {
        // A shared floor keeps adjacent columns continuous. Clamp only on steep
        // slopes so each column still has a 20–60 block cut from its surface.
        let bottom = self.bottom.clamp(surface - 60, surface - 20);
        if !self.contains_column(x, z) || y <= bottom || y > surface {
            return false;
        }
        let (_, short) = self.offsets(x, z);
        let side = if y.div_euclid(11).rem_euclid(2) == 0 {
            -self.width / 2
        } else {
            (self.width - 1) / 2
        };
        // One-block ledges cling to alternating walls, never sealing the shaft.
        !(y.rem_euclid(11) == self.ledge_phase && short == side)
    }
}

/// One demon altar: the origin cell and the chasm axis. Vanilla builds the
/// altar as a 3x2 silhouette (`WorldGen.Place3x2`, gameplay spec, altar
/// pass :15849-15910); in this world that is 3 cells along the chasm's long
/// axis, 1 deep and 2 tall, every cell `DEMON_ALTAR`.
struct Altar {
    origin: [i32; 3],
    along_x: bool,
}

/// Altar sites for nearby chasms, from `(seed, chasm)` alone. Vanilla hangs an
/// altar on the bottom of each chasm pool; the plan's "2-6 per corruption
/// zone" is implemented as a distribution target: the noise here is not
/// flood-filled during chunk generation, so a dedicated hash selects one
/// chasm in eight and a 6-seed survey of the existing corruption noise puts
/// that at 3.2-6.1 altars per connected patch (median 2-3) counting the sites
/// that survive validation below.
fn altars_in(seed: u32, chasms: &[Chasm], size: WorldSize) -> Vec<Altar> {
    chasms
        .iter()
        .filter_map(|c| {
            let h = hash(seed ^ SALT_ALTAR, c.x, 0, c.z);
            if h % 8 != 0 {
                return None;
            }
            // A deterministic slot on the chasm floor: one column across the
            // width, three along the length (`contains_column` ranges).
            let s = hash(h, 1, 2, 3);
            let short_n = ((c.width + 1) / 2 + c.width / 2) as u32;
            let long_n = ((c.length + 1) / 2 + c.length / 2 - 2) as u32;
            let short = (s % short_n) as i32 - c.width / 2;
            let long0 = ((s >> 8) % long_n) as i32 - c.length / 2;
            let site = |i: i32| {
                if c.along_x {
                    (c.x + long0 + i, c.z + short)
                } else {
                    (c.x + short, c.z + long0 + i)
                }
            };
            // The bottom row rests on the carve floor: the first row carved at
            // every site column, so the support below is uncarved terrain.
            let bottom = ((0..3)
                .map(|i| {
                    let (x, z) = site(i);
                    let surface = surface_height_raw(seed, x, z, size);
                    c.bottom.clamp(surface - 60, surface - 20)
                })
                .max()
                .unwrap())
                + 1;
            Some(Altar {
                origin: {
                    let (x, z) = site(0);
                    [x, bottom, z]
                },
                along_x: c.along_x,
            })
        })
        .collect()
}

/// Writes the altars whose origin cell is in this chunk: all six cells and
/// the three floor cells must belong to it (the owning chunk alone writes the
/// whole group, so regeneration never duplicates one and a group is never
/// split); each of the six must be chasm-carved air, with the un-carved solid
/// floor under the bottom row — otherwise the site is skipped.
fn write_altars(seed: u32, lo: [i32; 3], chasms: &[Chasm], size: WorldSize, chunk: &mut Chunk) {
    let cs = CHUNK_SIZE as i32;
    let carved = |p: [i32; 3]| {
        let surface = surface_height_raw(seed, p[0], p[2], size);
        chasms.iter().any(|c| c.carves(p[0], p[1], p[2], surface))
    };
    for altar in altars_in(seed, chasms, size) {
        let [ox, oy, oz] = altar.origin;
        let (dx, dz) = if altar.along_x { (1, 0) } else { (0, 1) };
        let cell = |i: i32, j: i32| [ox + dx * i, oy + j, oz + dz * i];
        let cells: Vec<[i32; 3]> = (0..3)
            .flat_map(|i| (0..2).map(move |j| cell(i, j)))
            .collect();
        let floors: Vec<[i32; 3]> = (0..3).map(|i| cell(i, -1)).collect();
        let in_chunk = |p: [i32; 3]| (0..3).all(|a| (lo[a]..lo[a] + cs).contains(&p[a]));
        if !cells.iter().chain(floors.iter()).copied().all(in_chunk) {
            continue;
        }
        if !cells.iter().copied().all(carved) || !floors.iter().copied().all(|p| !carved(p)) {
            continue;
        }
        for (x, y, z) in cells.into_iter().map(|[x, y, z]| {
            (
                (x - lo[0]) as usize,
                (y - lo[1]) as usize,
                (z - lo[2]) as usize,
            )
        }) {
            chunk.set(x, y, z, DEMON_ALTAR);
        }
    }
}

struct ShadowOrbSite {
    pos: [i32; 3],
}

/// Shadow orb sites inside corruption chasms: places 2-3 shadow orbs in subterranean pockets along the chasm floor.
fn shadow_orbs_in(seed: u32, chasms: &[Chasm], size: WorldSize) -> Vec<ShadowOrbSite> {
    let mut out = Vec::new();
    for c in chasms {
        let h = hash(seed ^ SALT_ORB, c.x, 0, c.z);
        let count = 2 + (h & 1) as usize;
        let short_n = ((c.width + 1) / 2 + c.width / 2) as u32;
        let long_n = ((c.length + 1) / 2 + c.length / 2) as u32;
        if long_n < 4 {
            continue;
        }
        for i in 0..count {
            let s = hash(h, i as i32, 7, 13);
            let short = if short_n > 0 {
                (s % short_n) as i32 - c.width / 2
            } else {
                0
            };
            let step = (long_n - 2) / (count as u32 + 1);
            let long = (step * (i as u32 + 1)) as i32 - c.length / 2;
            let (x, z) = if c.along_x {
                (c.x + long, c.z + short)
            } else {
                (c.x + short, c.z + long)
            };
            let surface = surface_height_raw(seed, x, z, size);
            let bottom = c.bottom.clamp(surface - 60, surface - 20);
            let y = bottom + 1;
            out.push(ShadowOrbSite { pos: [x, y, z] });
        }
    }
    out
}

fn write_shadow_orbs(
    seed: u32,
    lo: [i32; 3],
    chasms: &[Chasm],
    size: WorldSize,
    chunk: &mut Chunk,
) {
    let cs = CHUNK_SIZE as i32;
    let carved = |p: [i32; 3]| {
        let surface = surface_height_raw(seed, p[0], p[2], size);
        chasms.iter().any(|c| c.carves(p[0], p[1], p[2], surface))
    };
    for orb in shadow_orbs_in(seed, chasms, size) {
        let [x, y, z] = orb.pos;
        let in_chunk = (0..3).all(|a| (lo[a]..lo[a] + cs).contains(&orb.pos[a]));
        if !in_chunk {
            continue;
        }
        if !carved(orb.pos) || carved([x, y - 1, z]) {
            continue;
        }
        let lx = (x - lo[0]) as usize;
        let ly = (y - lo[1]) as usize;
        let lz = (z - lo[2]) as usize;
        if chunk.get(lx, ly, lz) != DEMON_ALTAR {
            chunk.set(lx, ly, lz, SHADOW_ORB);
        }
    }
}

/// ChasmRunner inspiration: gameplay spec/14470. The 3-D adaptation uses
/// narrow fissures in hashed world cells, not chunk-local random shapes.
fn chasms_in(seed: u32, lo: [i32; 3], size: WorldSize) -> Vec<Chasm> {
    let mut out = Vec::new();
    let cells = |i: usize| {
        (lo[i] - CHASM_REACH - 1).div_euclid(CHASM_CELL)
            ..=(lo[i] + CHUNK_SIZE + CHASM_REACH).div_euclid(CHASM_CELL)
    };
    for cx in cells(0) {
        for cz in cells(2) {
            let h = hash(seed ^ SALT_CHASM, cx, 0, cz);
            if h % 4 != 0 {
                continue;
            }
            let x = cx * CHASM_CELL + ((h >> 2) % 64) as i32;
            let z = cz * CHASM_CELL + ((h >> 8) % 64) as i32;
            if x < lo[0] - CHASM_REACH - 1
                || x > lo[0] + CHUNK_SIZE + CHASM_REACH
                || z < lo[2] - CHASM_REACH - 1
                || z > lo[2] + CHUNK_SIZE + CHASM_REACH
                || corruption_at_for_size(seed, x, z, size) == 0.0
            {
                continue;
            }
            let shape = hash(seed ^ SALT_CHASM, cx, 1, cz);
            let chasm = Chasm {
                x,
                z,
                along_x: shape & 1 == 0,
                length: 16 + ((shape >> 1) % 17) as i32,
                width: 2 + ((shape >> 6) % 3) as i32,
                bottom: surface_height_raw(seed, x, z, size) - 20 - ((shape >> 8) % 41) as i32,
                ledge_phase: ((shape >> 14) % 11) as i32,
            };
            // Reject a feature that only partly overlaps the patch: clipping it
            // could leave a one-block slot and an unlined boundary wall. The
            // extra ring is the ebonstone lining halo.
            let (x0, x1, z0, z1) = chasm.bounds();
            let inside = (x0 - 1..=x1 + 1)
                .all(|x| (z0 - 1..=z1 + 1).all(|z| corruption_at_for_size(seed, x, z, size) > 0.0));
            if inside {
                out.push(chasm);
            }
        }
    }
    out
}

fn column_index(x: usize, z: usize) -> usize {
    x + z * PAD
}
fn mask_index(x: usize, y: usize, z: usize) -> usize {
    x + y * PAD + z * PAD * PAD
}

/// The whole pass: conversion, chasm carving/lining, then ebonstone-only ore.
/// Noise/surface queries are cached per column, including a one-voxel halo so
/// the wall of a chasm just outside this chunk is still lined correctly.
pub fn apply(seed: u32, pos: ChunkPos, chunk: &mut Chunk) {
    apply_for_size(seed, pos, WorldSize::Small, chunk);
}

pub fn apply_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let bounds = WorldBounds::new(size);
    let cx = pos.x * CHUNK_SIZE + CHUNK_SIZE / 2;
    let cz = pos.z * CHUNK_SIZE + CHUNK_SIZE / 2;
    if bounds.contains_column(cx, cz) && bounds.edge_distance(cx, cz) < OCEAN_WIDTH + BEACH_WIDTH {
        return;
    }
    let lo = [pos.x, pos.y, pos.z].map(|c| c * CHUNK_SIZE);
    if lo[1] + CHUNK_SIZE <= 0 || i64::from(lo[1]) >= WORLD_HEIGHT {
        return;
    }
    let chasms = chasms_in(seed, lo, size);
    let mut columns = [Column {
        corrupt: false,
        surface: 0,
    }; PAD * PAD];
    let mut any_corrupt = false;
    let mut carved = if chasms.is_empty() {
        Vec::new()
    } else {
        vec![false; PAD * PAD * PAD]
    };
    for z in 0..PAD {
        for x in 0..PAD {
            let (wx, wz) = (lo[0] + x as i32 - 1, lo[2] + z as i32 - 1);
            let corrupt = corruption_at_for_size(seed, wx, wz, size) > 0.0;
            // `chasms_in` keeps only footprints fully inside corruption; this
            // guard also keeps a future boundary bug from carving a tree/chest
            // column that the decoration passes still place.
            let in_chasm = corrupt && chasms.iter().any(|c| c.contains_column(wx, wz));
            let outside_spawn =
                f64::from(wx).powi(2) + f64::from(wz).powi(2) > SPAWN_CLEAR * SPAWN_CLEAR;
            let surface = if corrupt || in_chasm {
                surface_height_raw(seed, wx, wz, size)
            } else {
                0
            };
            columns[column_index(x, z)] = Column { corrupt, surface };
            any_corrupt |= corrupt;
            if in_chasm && outside_spawn {
                for y in 0..PAD {
                    let wy = lo[1] + y as i32 - 1;
                    carved[mask_index(x, y, z)] = (0..WORLD_HEIGHT).contains(&i64::from(wy))
                        && chasms.iter().any(|c| c.carves(wx, wy, wz, surface));
                }
            }
        }
    }
    if !any_corrupt && chasms.is_empty() {
        return;
    }
    for z in 0..CS {
        for x in 0..CS {
            let (wx, wz) = (f64::from(lo[0] + x as i32), f64::from(lo[2] + z as i32));
            // The halo can see a cut immediately outside the protected circle;
            // its lining must obey the same exclusion as the cut itself.
            if wx * wx + wz * wz <= SPAWN_CLEAR * SPAWN_CLEAR {
                continue;
            }
            let col = columns[column_index(x + 1, z + 1)];
            for y in 0..CS {
                let cur = chunk.get(x, y, z);
                let mut tile = match cur {
                    STONE if col.corrupt => EBONSTONE,
                    GRASS if col.corrupt => CORRUPT_GRASS,
                    _ => cur,
                };
                if !carved.is_empty() {
                    let i = mask_index(x + 1, y + 1, z + 1);
                    if carved[i] {
                        if matches!(
                            tile,
                            STONE
                                | DIRT
                                | GRASS
                                | CORRUPT_GRASS
                                | EBONSTONE
                                | SAND
                                | HARDENED_SAND
                                | SANDSTONE
                                | SNOW
                                | ICE
                                | MUD
                                | JUNGLE_GRASS
                        ) {
                            tile = AIR;
                        }
                    } else if col.corrupt
                        && matches!(
                            tile,
                            STONE
                                | DIRT
                                | GRASS
                                | CORRUPT_GRASS
                                | EBONSTONE
                                | SAND
                                | HARDENED_SAND
                                | SANDSTONE
                                | SNOW
                                | ICE
                                | MUD
                                | JUNGLE_GRASS
                        )
                        && [i - 1, i + 1, i - PAD, i + PAD, i - PAD * PAD, i + PAD * PAD]
                            .into_iter()
                            .any(|n| carved[n])
                    {
                        // Fill every carved terrain wall, including biome soil,
                        // so a chasm inside a patch is ebonstone-lined.
                        tile = EBONSTONE;
                    }
                }
                if cur != tile {
                    chunk.set(x, y, z, tile);
                }
            }
        }
    }
    // Altars are written once the carve/lining pass above has settled, so the
    // site test reads the real post-carve state. Demonite ore only replaces
    // ebonstone, so ordering against it is not load-bearing; this pass runs
    // first regardless, deterministically.
    if !carved.is_empty() {
        write_altars(seed, lo, &chasms, size, chunk);
        write_shadow_orbs(seed, lo, &chasms, size, chunk);
    }
    if any_corrupt {
        apply_demonite(seed, lo, &columns, size, chunk);
    }
}

/// Demonite uses TileRunner (gameplay spec). Vanilla's finite-world
/// count becomes gold-like 3-D density here; host checks are deliberately
/// stricter: only existing EBONSTONE, at least five blocks below the surface.
fn apply_demonite(
    seed: u32,
    lo: [i32; 3],
    columns: &[Column; PAD * PAD],
    size: WorldSize,
    chunk: &mut Chunk,
) {
    let cells = |i: usize| {
        (lo[i] - ORE_REACH).div_euclid(ORE_CELL)
            ..=(lo[i] + CHUNK_SIZE + ORE_REACH).div_euclid(ORE_CELL)
    };
    for cx in cells(0) {
        for cy in cells(1) {
            for cz in cells(2) {
                let h = hash(seed ^ SALT_DEMONITE, cx, cy, cz);
                let expected = DEMONITE_DENSITY * f64::from(ORE_CELL * ORE_CELL * ORE_CELL);
                let count =
                    expected as u32 + u32::from(f64::from(h) / 4_294_967_296.0 < expected.fract());
                for n in 0..count {
                    let r = hash(h, n as i32, 0, 0);
                    let origin = [
                        cx * ORE_CELL + (r & 31) as i32,
                        cy * ORE_CELL + ((r >> 5) & 31) as i32,
                        cz * ORE_CELL + ((r >> 10) & 31) as i32,
                    ];
                    if !(0..3).all(|i| {
                        origin[i] >= lo[i] - ORE_REACH && origin[i] < lo[i] + CHUNK_SIZE + ORE_REACH
                    }) || corruption_at_for_size(seed, origin[0], origin[2], size) == 0.0
                        || origin[1] >= surface_height_raw(seed, origin[0], origin[2], size) - 4
                    {
                        continue;
                    }
                    let strength = 4 + ((r >> 15) % 5) as i32;
                    let steps = 4 + ((r >> 18) % 5) as i32;
                    for (p, _) in
                        tile_runner(seed ^ SALT_DEMONITE, origin, strength, steps, DEMONITE_ORE)
                    {
                        if !(0..3).all(|i| p[i] >= lo[i] && p[i] < lo[i] + CHUNK_SIZE) {
                            continue;
                        }
                        let [x, y, z] = [0, 1, 2].map(|i| (p[i] - lo[i]) as usize);
                        let col = columns[column_index(x + 1, z + 1)];
                        if col.corrupt && p[1] < col.surface - 4 && chunk.get(x, y, z) == EBONSTONE
                        {
                            chunk.set(x, y, z, DEMONITE_ORE);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::*;
    use crate::chunk::CHUNK_SIZE;
    use std::collections::HashSet;

    const CS: usize = CHUNK_SIZE as usize;

    fn corrupt_chunk(seed: u32) -> ChunkPos {
        for x in (-6000..=6000).step_by(64) {
            for z in (-6000..=6000).step_by(64) {
                if corruption_at(seed, x, z) > 0.1 {
                    return ChunkPos::of_block(x, 0, z);
                }
            }
        }
        panic!("seed {seed} has no corruption within 6000 blocks");
    }

    #[test]
    fn corruption_not_near_spawn() {
        // 303 blocks from the origin covers the real spawn, which is within
        // one block of it (server::spawn_point picks among the nine columns).
        let r = SPAWN_CLEAR as i32;
        for seed in [0, 1, 7, 42, 12345, u32::MAX] {
            for x in (-r..=r).step_by(5) {
                for z in (-r..=r).step_by(5) {
                    if f64::from(x).powi(2) + f64::from(z).powi(2) <= SPAWN_CLEAR * SPAWN_CLEAR {
                        assert_eq!(corruption_at(seed, x, z), 0.0, "{seed}: {x},{z}");
                    }
                }
            }
            corrupt_chunk(seed);
        }
    }

    #[test]
    fn corruption_patch_density_is_sparse() {
        // Count connected patches on a 100-block survey grid. Ignore tiny
        // threshold slivers (<0.1 km²); area/count gives typical 2-D spacing.
        let mut spacing = 0.0;
        for seed in [0, 1, 7, 42, 12345, u32::MAX] {
            let mut grid = vec![false; 121 * 121];
            for z in 0..121 {
                for x in 0..121 {
                    grid[x + z * 121] =
                        corruption_at(seed, x as i32 * 100 - 6000, z as i32 * 100 - 6000) > 0.0;
                }
            }
            let mut patches = 0;
            for start in 0..grid.len() {
                if !grid[start] {
                    continue;
                }
                grid[start] = false;
                let mut todo = vec![start];
                let mut size = 0;
                while let Some(i) = todo.pop() {
                    size += 1;
                    let (x, z) = (i % 121, i / 121);
                    for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                        let (nx, nz) = (x as i32 + dx, z as i32 + dz);
                        if (0..121).contains(&nx) && (0..121).contains(&nz) {
                            let n = nx as usize + nz as usize * 121;
                            if grid[n] {
                                grid[n] = false;
                                todo.push(n);
                            }
                        }
                    }
                }
                if size >= 10 {
                    patches += 1;
                }
            }
            assert!(patches > 0);
            spacing += (144_000_000.0_f64 / f64::from(patches)).sqrt();
        }
        let average = spacing / 6.0;
        eprintln!("corruption survey: average patch spacing {average:.0} blocks");
        assert!(
            (2000.0..=3000.0).contains(&average),
            "average patch spacing {average:.0} blocks"
        );
    }

    #[test]
    fn chasm_carves_only_corruption() {
        // Even with fully-inside footprints, keep the per-column guard honest:
        // a carved column must always be corrupt so decoration placement and
        // the chasm pass cannot disagree.
        let seed = 7;
        let base = corrupt_chunk(seed);
        let mut carved = false;
        for cx in base.x - 3..=base.x + 3 {
            for cz in base.z - 3..=base.z + 3 {
                for cy in 1..=2 {
                    let mut chunk = Chunk::filled(DIRT);
                    apply(
                        seed,
                        ChunkPos {
                            x: cx,
                            y: cy,
                            z: cz,
                        },
                        &mut chunk,
                    );
                    for z in 0..CS {
                        for x in 0..CS {
                            let (wx, wz) = (cx * CHUNK_SIZE + x as i32, cz * CHUNK_SIZE + z as i32);
                            if (0..CS).any(|y| chunk.get(x, y, z) == AIR) {
                                carved = true;
                                assert!(
                                    corruption_at(seed, wx, wz) > 0.0,
                                    "chasm carved outside corruption at {wx},{wz}"
                                );
                            }
                        }
                    }
                }
            }
        }
        assert!(carved, "no chasm carve found near a corruption patch");
    }

    #[test]
    fn chasm_footprints_stay_inside_corruption() {
        // The full footprint plus the one-block lining halo must be corrupt so
        // clipping never narrows a 2-4 wide chasm or leaves an unlined wall.
        let seed = 7;
        let base = corrupt_chunk(seed);
        let mut found = 0;
        for cx in base.x - 4..=base.x + 4 {
            for cz in base.z - 4..=base.z + 4 {
                for chasm in chasms_in(
                    seed,
                    [cx * CHUNK_SIZE, 0, cz * CHUNK_SIZE],
                    WorldSize::Small,
                ) {
                    let (x0, x1, z0, z1) = chasm.bounds();
                    for x in x0 - 1..=x1 + 1 {
                        for z in z0 - 1..=z1 + 1 {
                            assert!(
                                corruption_at(seed, x, z) > 0.0,
                                "chasm footprint outside corruption at {x},{z}"
                            );
                        }
                    }
                    assert!((2..=4).contains(&chasm.width));
                    found += 1;
                }
            }
        }
        assert!(found > 0, "no chasms found to check");
    }

    #[test]
    fn corrupt_zone_converts_stone_to_ebonstone() {
        let seed = 7;
        let pos = corrupt_chunk(seed);
        let mut chunk = Chunk::filled(STONE);
        // Surface conversion and preservation are independent of natural terrain.
        let tiles = [GRASS, SAND, SNOW, JUNGLE_GRASS, MUD, GOLD, COPPER, AIR];
        for (x, tile) in tiles.into_iter().enumerate() {
            chunk.set(x, 31, 0, tile);
        }
        apply(seed, pos, &mut chunk);
        assert_eq!(chunk.get(0, 31, 0), CORRUPT_GRASS);
        for (x, tile) in tiles.into_iter().enumerate().skip(1) {
            assert_eq!(chunk.get(x, 31, 0), tile);
        }
        assert!((0..CS).any(|x| chunk.get(x, 0, 0) == EBONSTONE));
    }

    #[test]
    fn chasm_is_continuous_across_chunk_border() {
        let seed = 7;
        let base = corrupt_chunk(seed);
        let mut found = false;
        // Search real generated chunks, not a test-only chasm or mocked mask.
        'search: for cx in base.x - 4..=base.x + 4 {
            for cz in base.z - 4..=base.z + 4 {
                let edge = (cx + 1) * CHUNK_SIZE;
                let mut left = Vec::new();
                let mut right = Vec::new();
                for cy in 0..4 {
                    let mut a = Chunk::filled(DIRT);
                    let mut b = Chunk::filled(DIRT);
                    apply(
                        seed,
                        ChunkPos {
                            x: cx,
                            y: cy,
                            z: cz,
                        },
                        &mut a,
                    );
                    apply(
                        seed,
                        ChunkPos {
                            x: cx + 1,
                            y: cy,
                            z: cz,
                        },
                        &mut b,
                    );
                    left.push(a);
                    right.push(b);
                }
                for z in 1..CS - 1 {
                    let wz = cz * CHUNK_SIZE + z as i32;
                    let h = surface_height(seed, edge - 1, wz).min(surface_height(seed, edge, wz));
                    let column = |chunks: &[Chunk], x: usize| -> Vec<bool> {
                        (0..=h)
                            .map(|y| chunks[y as usize / CS].get(x, y as usize % CS, z) == AIR)
                            .collect()
                    };
                    let a = column(&left, CS - 1);
                    let b = column(&right, 0);
                    // An opening on both sides must continue with the same floor.
                    if a.iter().filter(|&&air| air).count() >= 20
                        && b.iter().filter(|&&air| air).count() >= 20
                    {
                        assert_eq!(a, b, "mismatched seam at {edge},{wz}");
                        // Dirt side walls prove lining is applied, even across the seam.
                        for y in 0..=h as usize {
                            if !a[y] {
                                continue;
                            }
                            for (chunks, x) in [(&left, CS - 1), (&right, 0)] {
                                for nz in [z - 1, z + 1] {
                                    let wall = chunks[y / CS].get(x, y % CS, nz);
                                    assert!(
                                        wall == AIR || wall == EBONSTONE || wall == DEMONITE_ORE,
                                        "unlined wall at {edge},{y},{wz}"
                                    );
                                }
                            }
                        }
                        found = true;
                        break 'search;
                    }
                }
            }
        }
        assert!(found, "no continuous chasm crossing found");
    }

    #[test]
    fn demonite_only_in_ebonstone() {
        let seed = 42;
        let mut spawn = Chunk::filled(STONE);
        apply(seed, ChunkPos { x: 0, y: 0, z: 0 }, &mut spawn);
        assert_eq!(spawn, Chunk::filled(STONE));

        let base = corrupt_chunk(seed);
        let lo = [base.x, base.y, base.z].map(|c| c * CHUNK_SIZE);
        let mut columns = [Column {
            corrupt: false,
            surface: 0,
        }; PAD * PAD];
        for z in 0..PAD {
            for x in 0..PAD {
                let (wx, wz) = (lo[0] + x as i32 - 1, lo[2] + z as i32 - 1);
                columns[column_index(x, z)] = Column {
                    corrupt: corruption_at(seed, wx, wz) > 0.0,
                    surface: surface_height(seed, wx, wz),
                };
            }
        }
        // Test the ore subpass directly too: STONE is a valid input to the
        // whole pass (it becomes EBONSTONE first), never to vein replacement.
        for host in [STONE, DIRT, AIR, SAND, SNOW, MUD, JUNGLE_GRASS, GOLD] {
            let mut chunk = Chunk::filled(host);
            apply_demonite(seed, lo, &columns, WorldSize::Small, &mut chunk);
            assert!(chunk == Chunk::filled(host), "demonite replaced {host:?}");
        }
        let mut count = 0;
        // Non-host stripes intersect real veins: loosening the replacement
        // predicate to air, dirt, mud or other ores must fail this test.
        let tiles = [EBONSTONE, AIR, SAND, SNOW, MUD, JUNGLE_GRASS, GOLD, COPPER];
        for cx in base.x..base.x + 3 {
            let pos = ChunkPos { x: cx, ..base };
            let mut chunk = Chunk::filled(AIR);
            for z in 0..CS {
                for y in 0..CS {
                    for x in 0..CS {
                        chunk.set(x, y, z, tiles[x % tiles.len()]);
                    }
                }
            }
            let before = chunk.clone();
            apply(seed, pos, &mut chunk);
            for z in 0..CS {
                for y in 0..CS {
                    for x in 0..CS {
                        if chunk.get(x, y, z) == DEMONITE_ORE {
                            count += 1;
                            assert_eq!(before.get(x, y, z), EBONSTONE);
                            assert!(
                                (y as i32)
                                    < surface_height(
                                        seed,
                                        cx * CHUNK_SIZE + x as i32,
                                        base.z * CHUNK_SIZE + z as i32
                                    ) - 4
                            );
                        }
                    }
                }
            }
        }
        assert!(count > 0, "no demonite placed; host check would be vacuous");
    }

    #[test]
    fn chasm_clipped_away_from_tree_outside_corruption() {
        // Seed 0 has a tree at [-4235, 68, -3605] just outside a patch; the
        // old unclipped chasm carved the column and left the tree floating.
        let (seed, x, z) = (0, -4235, -3605);
        assert_eq!(corruption_at(seed, x, z), 0.0);
        for cy in 0..4 {
            let pos = ChunkPos::of_block(x, cy * CHUNK_SIZE, z);
            let mut chunk = Chunk::filled(DIRT);
            apply(seed, pos, &mut chunk);
            let (lx, lz) = (
                (x - pos.x * CHUNK_SIZE) as usize,
                (z - pos.z * CHUNK_SIZE) as usize,
            );
            assert!(
                (0..CS).all(|y| chunk.get(lx, y, lz) != AIR),
                "non-corrupt tree column carved at chunk y {cy}"
            );
        }
    }

    #[test]
    fn corruption_gen_is_deterministic() {
        let seed = 7;
        let pos = corrupt_chunk(seed);
        let mut a = Chunk::filled(STONE);
        apply(seed, pos, &mut a);
        let mut neighbor = Chunk::filled(STONE);
        apply(
            seed,
            ChunkPos {
                x: pos.x + 1,
                ..pos
            },
            &mut neighbor,
        );
        let mut b = Chunk::filled(STONE);
        apply(seed, pos, &mut b);
        assert_eq!(a, b);
        assert!(a != Chunk::filled(STONE), "corruption pass made no changes");
    }

    /// World block for every `DEMON_ALTAR` cell in the `(2r+1)^2` chunks
    /// around `c0` (y chunks 0-2, where the chasm floors live).
    fn altar_cells(seed: u32, c0: ChunkPos, r: i32) -> HashSet<[i32; 3]> {
        let mut cells = HashSet::new();
        for cz in c0.z - r..=c0.z + r {
            for cx in c0.x - r..=c0.x + r {
                for cy in 0..=2 {
                    let mut chunk = Chunk::filled(DIRT);
                    apply(
                        seed,
                        ChunkPos {
                            x: cx,
                            y: cy,
                            z: cz,
                        },
                        &mut chunk,
                    );
                    for z in 0..CS {
                        for y in 0..CS {
                            for x in 0..CS {
                                if chunk.get(x, y, z) == DEMON_ALTAR {
                                    cells.insert([
                                        cx * CHUNK_SIZE + x as i32,
                                        cy * CHUNK_SIZE + y as i32,
                                        cz * CHUNK_SIZE + z as i32,
                                    ]);
                                }
                            }
                        }
                    }
                }
            }
        }
        cells
    }

    /// Validated site origins near the corruption patch, deduplicated: a
    /// whole `r`-chunk window sees each chasm within its 16-block reach, so
    /// one site appears once per visiting chunk. Applies are too costly to
    /// scan the world directly, so this reruns the `write_altars` checks
    /// (production `altars_in` plus the carve predicates).
    fn altar_sites(seed: u32, r: i32) -> Vec<[i32; 3]> {
        let base = corrupt_chunk(seed);
        let carved = |chasms: &[Chasm], p: [i32; 3]| {
            let surface = surface_height(seed, p[0], p[2]);
            chasms.iter().any(|c| c.carves(p[0], p[1], p[2], surface))
        };
        let mut out = HashSet::new();
        for cz in base.z - r..=base.z + r {
            for cx in base.x - r..=base.x + r {
                let lo = [cx * CHUNK_SIZE, 0, cz * CHUNK_SIZE];
                let chasms = chasms_in(seed, lo, WorldSize::Small);
                if chasms.is_empty() {
                    continue;
                }
                for altar in altars_in(seed, &chasms, WorldSize::Small) {
                    let [ox, oy, oz] = altar.origin;
                    let (dx, dz) = if altar.along_x { (1, 0) } else { (0, 1) };
                    let cell = |i: i32, j: i32| [ox + dx * i, oy + j, oz + dz * i];
                    let cells: Vec<[i32; 3]> = (0..3)
                        .flat_map(|i| (0..2).map(move |j| cell(i, j)))
                        .collect();
                    let floors: Vec<[i32; 3]> = (0..3).map(|i| cell(i, -1)).collect();
                    if cells.iter().all(|p| carved(&chasms, *p))
                        && floors.iter().all(|p| !carved(&chasms, *p))
                    {
                        out.insert(altar.origin);
                    }
                }
            }
        }
        out.into_iter().collect()
    }

    /// Origin cell of the validated site nearest the corruption patch
    /// (block coordinates). The apply-based tests regenerate that chunk's
    /// neighborhood, where the real pass must place the altar. Panics when
    /// no site exists in the ±24-chunk window.
    fn altar_region(seed: u32) -> [i32; 3] {
        let base = corrupt_chunk(seed);
        let center = (base.x * CHUNK_SIZE, base.z * CHUNK_SIZE);
        let sites = altar_sites(seed, 24);
        let Some(origin) = sites.iter().min_by_key(|origin| {
            let dx = origin[0] - center.0;
            let dz = origin[2] - center.1;
            dx * dx + dz * dz
        }) else {
            panic!("seed {seed} has no validated altar site in a ±24-chunk window");
        };
        *origin
    }

    /// The 3x2 altar group rooted at each origin (no neighbour below or
    /// before it), asserting the groups are complete six-cell rectangles,
    /// unique and each entirely inside one chunk.
    fn altar_groups(cells: &HashSet<[i32; 3]>) -> Vec<([i32; 3], bool)> {
        let mut origins = Vec::new();
        for cell in cells {
            let [x, y, z] = *cell;
            if y == 0 || cells.contains(&[x, y - 1, z]) || cells.contains(&[x - 1, y, z]) {
                continue;
            }
            origins.push([x, y, z]);
        }
        origins.sort();
        let mut out = Vec::new();
        for origin in &origins {
            let [x, y, z] = *origin;
            let along_x = cells.contains(&[x + 1, y, z]);
            assert_ne!(along_x, cells.contains(&[x, y, z + 1]), "origin {origin:?}");
            let (dx, dz) = if along_x { (1, 0) } else { (0, 1) };
            for i in 0..3 {
                for j in 0..2 {
                    assert!(
                        cells.contains(&[x + dx * i, y + j, z + dz * i]),
                        "group at {origin:?} missing cell by {i},{j}"
                    );
                }
            }
            let cp = ChunkPos::of_block(x, y, z);
            for i in 0..3 {
                let cell = [x + dx * i, y, z + dz * i];
                assert_eq!(
                    ChunkPos::of_block(cell[0], cell[1], cell[2]),
                    cp,
                    "group at {origin:?} spans chunks"
                );
            }
            out.push((*origin, along_x));
        }
        assert_eq!(
            6 * out.len(),
            cells.len(),
            "stray altar cells outside complete groups: {cells:?}"
        );
        out
    }

    #[test]
    fn altar_generates_only_in_corruption() {
        // End to end: the seeded corruption pass generates altars, and every
        // cell is inside the corruption noise (chasms already obey the
        // 303-block spawn clearance, so that includes "not near spawn").
        let seed = 7;
        let cells = altar_cells(
            seed,
            ChunkPos::of_block(altar_region(seed)[0], 0, altar_region(seed)[2]),
            1,
        );
        assert!(!cells.is_empty(), "no altars generated at the located site");
        assert!(
            cells
                .iter()
                .all(|&[x, _, z]| corruption_at(seed, x, z) > 0.0),
            "altar cell outside corruption"
        );
        for seed in [0u32, 1, 42, 12345] {
            let region = altar_region(seed);
            let cells = altar_cells(seed, ChunkPos::of_block(region[0], 0, region[2]), 1);
            assert!(!cells.is_empty(), "seed {seed} placed no altar at its site");
            for cell in cells {
                assert!(corruption_at(seed, cell[0], cell[2]) > 0.0);
            }
        }
    }

    #[test]
    fn altar_gen_is_deterministic_and_unique() {
        let seed = 7;
        let origin = altar_region(seed);
        let center = ChunkPos::of_block(origin[0], 0, origin[2]);
        let cells = altar_cells(seed, center, 1);
        assert!(
            !cells.is_empty(),
            "no altars found (unique/determinism vacuous)"
        );
        // The same chunk re-applied is bit-identical, so a regenerated chunk
        // shows the same altars (Stage 5 edited chunks persist regardless).
        let mut a = Chunk::filled(DIRT);
        apply(seed, center, &mut a);
        let mut b = Chunk::filled(DIRT);
        apply(seed, center, &mut b);
        assert_eq!(a, b);
        // Every group is a complete 3x2 rectangle inside one chunk, and no
        // two altars share an origin across a whole neighborhood.
        let groups = altar_groups(&cells);
        assert!(
            groups.iter().any(|(o, _)| *o == origin),
            "located site missing"
        );
        let total: HashSet<[i32; 3]> = groups.iter().map(|(o, _)| *o).collect();
        assert_eq!(total.len(), groups.len(), "duplicate origins");
    }

    #[test]
    fn altar_sits_on_chasm_floor() {
        // Every group is three columns of the chasm's carved air with the
        // solid chasm floor directly under the bottom row.
        let seed = 7;
        let origin = altar_region(seed);
        let cells = altar_cells(seed, ChunkPos::of_block(origin[0], 0, origin[2]), 1);
        assert!(!cells.is_empty(), "no altars found to check");
        for (group_origin, along_x) in altar_groups(&cells) {
            let [x0, y0, z0] = group_origin;
            let (dx, dz) = if along_x { (1, 0) } else { (0, 1) };
            for i in 0..3 {
                let (x, z) = (x0 + dx * i, z0 + dz * i);
                let surface = surface_height(seed, x, z);
                let pos = ChunkPos::of_block(x, 0, z);
                let lo = [pos.x, pos.y, pos.z].map(|c| c * CHUNK_SIZE);
                let chasms = chasms_in(seed, lo, WorldSize::Small);
                let mut chunk = Chunk::filled(DIRT);
                apply(seed, pos, &mut chunk);
                for j in 0..2 {
                    // The chasm may carve both rows of every site column.
                    assert!(
                        chasms.iter().any(|c| c.carves(x, y0 + j, z, surface)),
                        "row ({x},{},{z}) not carved by a chasm",
                        y0 + j
                    );
                }
                // The floor: never carved, and solid in the owning chunk.
                assert!(
                    chasms.iter().all(|c| !c.carves(x, y0 - 1, z, surface)),
                    "floor under ({x},{},{z}) is carved",
                    y0 - 1
                );
                let b = chunk.get(
                    (x - lo[0]) as usize,
                    (y0 - 1 - lo[1]) as usize,
                    (z - lo[2]) as usize,
                );
                assert!(
                    b != AIR && b.is_solid(),
                    "floor {b:?} under ({x},{},{z})",
                    y0 - 1
                );
            }
        }
    }

    #[test]
    fn shadow_orbs_generate_in_corruption_chasms() {
        let seed = 7;
        let base = corrupt_chunk(seed);
        let mut total_orbs = 0;
        let mut checked_orbs = 0;

        for cx in base.x - 3..=base.x + 3 {
            for cz in base.z - 3..=base.z + 3 {
                for cy in 0..=2 {
                    let mut chunk = Chunk::filled(DIRT);
                    apply(
                        seed,
                        ChunkPos {
                            x: cx,
                            y: cy,
                            z: cz,
                        },
                        &mut chunk,
                    );
                    for z in 0..CS {
                        for y in 0..CS {
                            for x in 0..CS {
                                if chunk.get(x, y, z) == SHADOW_ORB {
                                    total_orbs += 1;
                                    let wx = cx * CHUNK_SIZE + x as i32;
                                    let wy = cy * CHUNK_SIZE + y as i32;
                                    let wz = cz * CHUNK_SIZE + z as i32;
                                    assert!(
                                        corruption_at(seed, wx, wz) > 0.0,
                                        "shadow orb generated outside corruption at ({wx}, {wy}, {wz})"
                                    );
                                    let surface = surface_height(seed, wx, wz);
                                    assert!(
                                        wy < surface - 10,
                                        "shadow orb ({wx}, {wy}, {wz}) too close to surface ({surface})"
                                    );
                                    checked_orbs += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(
            total_orbs > 0,
            "no shadow orbs generated in corruption chasms"
        );
        assert_eq!(total_orbs, checked_orbs);
    }
}
