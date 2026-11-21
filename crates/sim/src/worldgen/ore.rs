//! Ore veins: copper, iron, silver and gold blobs.
//!
//! Vanilla runs `WorldGen.TileRunner` a fixed number of times per world
//! (`OresAndShinies` pass, gameplay spec). This game streams an
//! unbounded world, so veins come from hashed 32^3 cells instead (like
//! `tree_in_cell`): each cell rolls its vein origins from `(seed, cell)` alone,
//! and any chunk regenerates every vein that can reach it, so chunk order never
//! matters.

use super::noise::hash;
use super::terrain::surface_height_for_size;
use crate::block::{BlockId, COPPER, DIRT, GOLD, IRON, SILVER, STONE};
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::world_bounds::WorldSize;

const SALT_VEIN: u32 = 0x0AE1_9C07;
const CELL: i32 = 32;
const CELL_VOLUME: f64 = (CELL * CELL * CELL) as f64;
/// Farthest a vein voxel can sit from its origin: radius `strength / 2` (max 4.5)
/// plus the walk, `steps` (max 8) moves of at most sqrt(3) per axis-clamped step.
const REACH: i32 = 20;

/// Depth below the column's surface, in blocks: shallow 0..24, mid 24..70,
/// deep 70+ (this game's stand-in for `worldSurface`/`rockLayer`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Shallow,
    Mid,
    Deep,
}

impl Band {
    pub fn of_depth(depth: i32) -> Option<Band> {
        match depth {
            ..0 => None,
            0..24 => Some(Band::Shallow),
            24..70 => Some(Band::Mid),
            _ => Some(Band::Deep),
        }
    }
}

/// One vein kind: veins started per voxel of its band, and the vanilla
/// `TileRunner` size ranges (strength, steps), both half-open.
pub struct Kind {
    pub tile: BlockId,
    pub band: Band,
    /// Veins per voxel. Vanilla count is `rate * maxX * maxY` over a band of
    /// `bandH` rows, i.e. `rate * maxY / bandH` per tile; divided by ~5 (a
    /// vein's thickness) to move from a 2-D slice to 3-D volume. Vanilla
    /// small-world bands (maxY 1200): shallow ~60 rows, mid ~200-250, deep
    /// ~600-700.
    pub density: f64,
    pub strength: (i32, i32),
    pub steps: (i32, i32),
}

/// Vein table. Rates and size ranges: gameplay spec
/// (copper 6E-05 / 8E-05 / 2E-04, iron 3E-05 / 8E-05 / 2E-04, silver 1.5E-04
/// in worldSurface..rockLayer and 2.6E-05 deep, gold 1.2E-04). Deviation: vanilla
/// has no deep gold in this depth range; a half-density deep band keeps gold
/// findable in a world with no Underworld.
pub const KINDS: [Kind; 10] = [
    Kind {
        tile: COPPER,
        band: Band::Shallow,
        density: 2.4e-4,
        strength: (3, 7),
        steps: (2, 7),
    },
    Kind {
        tile: COPPER,
        band: Band::Mid,
        density: 9.6e-5,
        strength: (3, 8),
        steps: (3, 8),
    },
    Kind {
        tile: COPPER,
        band: Band::Deep,
        density: 8.0e-5,
        strength: (4, 10),
        steps: (4, 9),
    },
    Kind {
        tile: IRON,
        band: Band::Shallow,
        density: 1.2e-4,
        strength: (3, 8),
        steps: (2, 6),
    },
    Kind {
        tile: IRON,
        band: Band::Mid,
        density: 9.6e-5,
        strength: (3, 7),
        steps: (3, 7),
    },
    Kind {
        tile: IRON,
        band: Band::Deep,
        density: 8.0e-5,
        strength: (4, 10),
        steps: (4, 9),
    },
    Kind {
        tile: SILVER,
        band: Band::Mid,
        density: 1.44e-4,
        strength: (4, 10),
        steps: (4, 9),
    },
    Kind {
        tile: SILVER,
        band: Band::Deep,
        density: 9.0e-6,
        strength: (3, 7),
        steps: (3, 7),
    },
    Kind {
        tile: GOLD,
        band: Band::Mid,
        density: 1.15e-4,
        strength: (4, 9),
        steps: (4, 9),
    },
    Kind {
        tile: GOLD,
        band: Band::Deep,
        density: 6.0e-5,
        strength: (4, 9),
        steps: (4, 9),
    },
];

/// SplitMix32 stream: a tiny deterministic stand-in for `genRand`.
struct Gen(u32);

impl Gen {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9);
        let mut z = self.0;
        z = (z ^ (z >> 16)).wrapping_mul(0x85EB_CA6B);
        z = (z ^ (z >> 13)).wrapping_mul(0xC2B2_AE35);
        z ^ (z >> 16)
    }

    /// `genRand.Next(lo, hi)`: uniform in `lo..hi`.
    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next() % (hi - lo) as u32) as i32
    }
}

/// Random-walk blob modelled on `WorldGen.TileRunner` (gameplay spec):
/// the radius shrinks linearly, `strength * stepsLeft / steps`; each step fills
/// the diamond `|dx|+|dy|+|dz| < radius * (1 + Next(-10,11) * 0.015)`; then the
/// centre moves by a velocity that starts at `Next(-10,11) * 0.1` per axis and
/// drifts by `Next(-10,11) * 0.05`, clamped to +-1. Vanilla is 2-D; z is added
/// with the same rules. Returns each voxel once, sorted.
pub fn tile_runner(
    seed: u32,
    origin: [i32; 3],
    strength: i32,
    steps: i32,
    tile: BlockId,
) -> Vec<([i32; 3], BlockId)> {
    let mut g = Gen(hash(seed ^ SALT_VEIN, origin[0], origin[1], origin[2]));
    let mut pos = origin.map(f64::from);
    let mut vel = [0; 3].map(|_| f64::from(g.range(-10, 11)) * 0.1);
    let mut out = Vec::new();
    let strength = f64::from(strength);
    for left in (1..=steps).rev() {
        let r = strength * f64::from(left) / f64::from(steps) * 0.5;
        let reach = r.ceil() as i32;
        let c = pos.map(|p| p as i32);
        for dx in -reach..=reach {
            for dy in -reach..=reach {
                for dz in -reach..=reach {
                    let d = f64::from(dx.abs() + dy.abs() + dz.abs());
                    if d < r * (1.0 + f64::from(g.range(-10, 11)) * 0.015) {
                        out.push(([c[0] + dx, c[1] + dy, c[2] + dz], tile));
                    }
                }
            }
        }
        for i in 0..3 {
            pos[i] += vel[i];
            vel[i] = (vel[i] + f64::from(g.range(-10, 11)) * 0.05).clamp(-1.0, 1.0);
        }
    }
    out.sort_unstable_by_key(|(p, _)| *p);
    out.dedup_by_key(|(p, _)| *p);
    out
}

/// A vein's start: where, what and how big.
#[derive(Clone, Copy, Debug)]
pub struct Vein {
    pub origin: [i32; 3],
    pub tile: BlockId,
    pub band: Band,
    pub strength: i32,
    pub steps: i32,
}

/// Veins started in one 32^3 cell, before the surface test (`band_of` maps an
/// origin to its band, or `None` to drop it). Depends on `(seed, cell)` only.
pub fn cell_veins(
    seed: u32,
    cell: [i32; 3],
    band_of: &mut dyn FnMut([i32; 3]) -> Option<Band>,
) -> Vec<Vein> {
    let mut out = Vec::new();
    for (k, kind) in KINDS.iter().enumerate() {
        let mut g = Gen(hash(
            seed ^ SALT_VEIN ^ (k as u32 + 1).wrapping_mul(0x51ED_270B),
            cell[0],
            cell[1],
            cell[2],
        ));
        let e = kind.density * CELL_VOLUME;
        // Random rounding keeps the expected count exact for fractional `e`.
        let n = e as i32 + i32::from(f64::from(g.next()) / 4_294_967_296.0 < e.fract());
        for _ in 0..n {
            let origin = [
                cell[0] * CELL + g.range(0, CELL),
                cell[1] * CELL + g.range(0, CELL),
                cell[2] * CELL + g.range(0, CELL),
            ];
            let (strength, steps) = (
                g.range(kind.strength.0, kind.strength.1),
                g.range(kind.steps.0, kind.steps.1),
            );
            if band_of(origin) == Some(kind.band) {
                out.push(Vein {
                    origin,
                    tile: kind.tile,
                    band: kind.band,
                    strength,
                    steps,
                });
            }
        }
    }
    out
}

fn surface_band(seed: u32, origin: [i32; 3], size: WorldSize) -> Option<Band> {
    Band::of_depth(surface_height_for_size(seed, origin[0], origin[2], size) - origin[1])
}

/// Every vein voxel inside chunk `pos`, in world coordinates. Chunk-order
/// independent: it only reads `(seed, cell)` hashes and the column surface.
pub fn veins_in(seed: u32, pos: ChunkPos) -> Vec<([i32; 3], BlockId)> {
    veins_in_for_size(seed, pos, WorldSize::Small)
}

pub fn veins_in_for_size(seed: u32, pos: ChunkPos, size: WorldSize) -> Vec<([i32; 3], BlockId)> {
    let lo = [pos.x, pos.y, pos.z].map(|c| c * CHUNK_SIZE);
    let hi = lo.map(|l| l + CHUNK_SIZE);
    let cells = |i: usize| (lo[i] - REACH).div_euclid(CELL)..=(hi[i] + REACH).div_euclid(CELL);
    let mut out = Vec::new();
    for cx in cells(0) {
        for cy in cells(1) {
            for cz in cells(2) {
                // Cheap box cull before the surface lookup and the walk.
                let mut near = |o: [i32; 3]| {
                    let inside = (0..3).all(|i| o[i] >= lo[i] - REACH && o[i] < hi[i] + REACH);
                    if inside {
                        surface_band(seed, o, size)
                    } else {
                        None
                    }
                };
                for v in cell_veins(seed, [cx, cy, cz], &mut near) {
                    for (p, t) in tile_runner(seed, v.origin, v.strength, v.steps, v.tile) {
                        if (0..3).all(|i| p[i] >= lo[i] && p[i] < hi[i]) {
                            out.push((p, t));
                        }
                    }
                }
            }
        }
    }
    out
}

/// Writes the chunk's vein voxels over STONE and DIRT only (vanilla ore
/// `TileRunner` overrides any clearable tile; air, grass, sand, snow and
/// trees are left alone here so ore is never exposed above ground).
pub fn apply(seed: u32, pos: ChunkPos, chunk: &mut Chunk) {
    apply_for_size(seed, pos, WorldSize::Small, chunk);
}

pub fn apply_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let lo = [pos.x, pos.y, pos.z].map(|c| (c * CHUNK_SIZE) as i32);
    for (p, tile) in veins_in_for_size(seed, pos, size) {
        let l = [0, 1, 2].map(|i| (p[i] - lo[i]) as usize);
        let cur = chunk.get(l[0], l[1], l[2]);
        if cur == STONE || cur == DIRT {
            chunk.set(l[0], l[1], l[2], tile);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::generate_chunk;
    use crate::worldgen::terrain::surface_height;

    fn cp(x: i32, y: i32, z: i32) -> ChunkPos {
        ChunkPos { x, y, z }
    }

    #[test]
    fn tile_runner_is_deterministic_and_bounded() {
        let a = tile_runner(7, [10, 50, -3], 9, 8, COPPER);
        assert_eq!(a, tile_runner(7, [10, 50, -3], 9, 8, COPPER));
        assert_ne!(a, tile_runner(8, [10, 50, -3], 9, 8, COPPER));
        assert!(!a.is_empty());
        for (p, t) in &a {
            assert_eq!(*t, COPPER);
            for (i, o) in [10, 50, -3].into_iter().enumerate() {
                assert!((p[i] - o).abs() <= REACH, "{p:?} out of reach");
            }
        }
    }

    #[test]
    fn veins_are_chunk_order_independent() {
        // Same chunk, generated cold and again after its neighbours: identical.
        let seed = 5;
        let a = generate_chunk(seed, cp(2, 1, -1));
        for n in [cp(1, 1, -1), cp(3, 1, -1), cp(2, 1, 0), cp(2, 2, -1)] {
            generate_chunk(seed, n);
        }
        assert_eq!(a, generate_chunk(seed, cp(2, 1, -1)));
        // A vein straddling a chunk edge shows the same voxels from both sides.
        let v = tile_runner(seed, [31, 40, 40], 9, 8, IRON);
        let left: Vec<_> = veins_in_at(&v, cp(0, 1, 1));
        let right: Vec<_> = veins_in_at(&v, cp(1, 1, 1));
        assert!(!left.is_empty() && !right.is_empty());
        assert_eq!(
            left.len() + right.len(),
            v.iter().filter(|(p, _)| in_chunks(p)).count()
        );
    }

    fn in_chunks(p: &[i32; 3]) -> bool {
        (0..64).contains(&p[0]) && (32..64).contains(&p[1]) && (32..64).contains(&p[2])
    }

    fn veins_in_at(v: &[([i32; 3], BlockId)], pos: ChunkPos) -> Vec<[i32; 3]> {
        let lo = [pos.x, pos.y, pos.z].map(|c| c * CHUNK_SIZE);
        v.iter()
            .filter(|(p, _)| (0..3).all(|i| p[i] >= lo[i] && p[i] < lo[i] + CHUNK_SIZE))
            .map(|(p, _)| *p)
            .collect()
    }

    #[test]
    fn veins_only_replace_stone_and_dirt() {
        use crate::block::{AIR, GRASS};
        let mut c = Chunk::filled(AIR);
        c.set(0, 0, 0, GRASS);
        c.set(1, 0, 0, STONE);
        let before = c.clone();
        apply(1, cp(0, 0, 0), &mut c);
        assert_eq!(c.get(0, 0, 0), GRASS);
        for z in 0..CHUNK_SIZE as usize {
            for y in 0..CHUNK_SIZE as usize {
                for x in 0..CHUNK_SIZE as usize {
                    if before.get(x, y, z) != STONE {
                        assert_eq!(c.get(x, y, z), before.get(x, y, z));
                    }
                }
            }
        }
    }

    #[test]
    fn ore_density_matches_target_within_tolerance() {
        // Vein starts per kind over a sample of cells vs the table (the band
        // test is the real surface lookup). Origins are 3-D hashes, so the
        // sampled count is the table's expectation +- Poisson noise.
        let seed = 3;
        let (mut got, mut volume) = ([0usize; KINDS.len()], [0f64; KINDS.len()]);
        for cx in -8..8 {
            for cz in -8..8 {
                for cy in 0..4 {
                    // Band volume in this cell: count voxels per band by column sampling.
                    let mut vols = [0f64; 3];
                    for x in (0..CELL).step_by(4) {
                        for z in (0..CELL).step_by(4) {
                            let h = surface_height(seed, cx * CELL + x, cz * CELL + z);
                            for y in (0..CELL).step_by(4) {
                                if let Some(b) = Band::of_depth(h - (cy * CELL + y)) {
                                    vols[b as usize] += 64.0;
                                }
                            }
                        }
                    }
                    for (k, kind) in KINDS.iter().enumerate() {
                        volume[k] += vols[kind.band as usize];
                    }
                    for v in cell_veins(seed, [cx, cy, cz], &mut |o| {
                        surface_band(seed, o, WorldSize::Small)
                    }) {
                        let k = KINDS
                            .iter()
                            .position(|k| k.tile == v.tile && k.band == v.band);
                        got[k.unwrap()] += 1;
                    }
                }
            }
        }
        for (k, kind) in KINDS.iter().enumerate() {
            let want = kind.density * volume[k];
            if want < 25.0 {
                continue; // too few expected for a +-30% bound
            }
            let ratio = got[k] as f64 / want;
            assert!(
                (0.7..=1.3).contains(&ratio),
                "kind {k}: got {} want {want:.0}",
                got[k]
            );
        }
    }

    #[test]
    fn all_four_ores_appear_by_depth() {
        let seed = 9;
        let mut seen = std::collections::BTreeSet::new();
        for cx in -3..3 {
            for cz in -3..3 {
                for cy in 0..4 {
                    let c = generate_chunk(seed, cp(cx, cy, cz));
                    for z in 0..CHUNK_SIZE as usize {
                        for y in 0..CHUNK_SIZE as usize {
                            for x in 0..CHUNK_SIZE as usize {
                                let b = c.get(x, y, z);
                                if [COPPER, IRON, SILVER, GOLD].contains(&b) {
                                    seen.insert(b.0);
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(seen.len(), 4, "ores seen: {seen:?}");
    }

    #[test]
    fn no_ore_exposed_above_surface() {
        let seed = 11;
        for cx in -2..2 {
            for cz in -2..2 {
                for cy in 0..4 {
                    let c = generate_chunk(seed, cp(cx, cy, cz));
                    for z in 0..CHUNK_SIZE as usize {
                        for x in 0..CHUNK_SIZE as usize {
                            let h = surface_height(
                                seed,
                                cx * CHUNK_SIZE + x as i32,
                                cz * CHUNK_SIZE + z as i32,
                            );
                            for y in 0..CHUNK_SIZE as usize {
                                let b = c.get(x, y, z);
                                if [COPPER, IRON, SILVER, GOLD].contains(&b) {
                                    assert!(
                                        cy * CHUNK_SIZE + (y as i32) < h,
                                        "ore at/above surface"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
