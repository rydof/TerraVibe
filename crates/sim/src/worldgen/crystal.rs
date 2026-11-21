//! Underground life crystals.
//!
//! Vanilla (gameplay spec, count `maxTilesX * maxTilesY * 2E-05`; loop at
//! `:16905-16920`) picks a random column and a `j` in `(worldSurface*2 + rockLayer)/3 ..
//! maxTilesY-300`, then `AddLifeCrystal` (`:32235`) drops down to the first solid floor
//! and needs empty tiles above it and solid tiles under both footprint columns.
//! This game streams an unbounded world, so like the veins each hashed 32^3 cell rolls
//! at most one attempt from `(seed, cell)` alone and any chunk can regenerate the
//! crystals that land in it. The floor test reads the pure natural terrain, never a chunk.

use super::noise::hash;
use super::terrain::{natural_block_for_size, surface_height_for_size};
use crate::block::{AIR, LIFE_CRYSTAL, STONE};
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::world_bounds::WorldSize;

const SALT_CRYSTAL: u32 = 0x1F3C_A5E9;
const CELL: i32 = 32;
/// Chance (per 1000) that a cell rolls a placement attempt.
const ATTEMPT_PERMILLE: u32 = 11;
/// How far below the rolled y the floor search goes (`AddLifeCrystal` scans to the world bottom).
const SEARCH: i32 = 12;
/// Minimum blocks below the column's surface (stand-in for the vanilla underground band).
pub const MIN_DEPTH: i32 = 40;
/// Air blocks required above the floor block (`EmptyTileCheck` needs a 2x2 gap in vanilla).
const CLEARANCE: i32 = 2;

fn crystal_in_cell_for_size(seed: u32, cell: [i32; 3], size: WorldSize) -> Option<[i32; 3]> {
    let h = hash(seed ^ SALT_CRYSTAL, cell[0], cell[1], cell[2]);
    if h % 1000 >= ATTEMPT_PERMILLE {
        return None;
    }
    let r = hash(seed ^ SALT_CRYSTAL ^ 0x5bd1_e995, cell[0], cell[1], cell[2]);
    let x = cell[0] * CELL + (r % CELL as u32) as i32;
    let z = cell[2] * CELL + ((r >> 8) % CELL as u32) as i32;
    let y0 = cell[1] * CELL + ((r >> 16) % CELL as u32) as i32;
    let surface = surface_height_for_size(seed, x, z, size);
    // Scan down to the first stone floor with air above it, like AddLifeCrystal.
    for y in (y0 - SEARCH..=y0).rev() {
        if y <= super::underworld::UNDERWORLD_TOP_Y || surface - y < MIN_DEPTH {
            continue;
        }
        if natural_block_for_size(seed, x, y - 1, z, size) == STONE
            && (0..CLEARANCE).all(|d| natural_block_for_size(seed, x, y + d, z, size) == AIR)
        {
            return Some([x, y, z]);
        }
    }
    None
}

/// Crystal positions (world coordinates) inside chunk `pos`. Chunk-order independent.
pub fn crystals_in(seed: u32, pos: ChunkPos) -> Vec<[i32; 3]> {
    crystals_in_for_size(seed, pos, WorldSize::Small)
}

pub fn crystals_in_for_size(seed: u32, pos: ChunkPos, size: WorldSize) -> Vec<[i32; 3]> {
    let lo = [pos.x, pos.y, pos.z].map(|c| c * CHUNK_SIZE);
    let mut out = Vec::new();
    // A crystal sits at most SEARCH below its cell, never above it or outside it in x/z.
    let cy = (lo[1] / CELL)..=((lo[1] + CHUNK_SIZE - 1 + SEARCH).div_euclid(CELL));
    for cx in lo[0].div_euclid(CELL)..=(lo[0] + CHUNK_SIZE - 1).div_euclid(CELL) {
        for cz in lo[2].div_euclid(CELL)..=(lo[2] + CHUNK_SIZE - 1).div_euclid(CELL) {
            for cy in cy.clone() {
                if let Some(p) = crystal_in_cell_for_size(seed, [cx, cy, cz], size)
                    && (0..3).all(|i| p[i] >= lo[i] && p[i] < lo[i] + CHUNK_SIZE)
                {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// Places the chunk's crystals into air.
pub fn apply(seed: u32, pos: ChunkPos, chunk: &mut Chunk) {
    apply_for_size(seed, pos, WorldSize::Small, chunk);
}

pub fn apply_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let lo = [pos.x, pos.y, pos.z].map(|c| c * CHUNK_SIZE);
    for p in crystals_in_for_size(seed, pos, size) {
        let l = [0, 1, 2].map(|i| (p[i] - lo[i]) as usize);
        if chunk.get(l[0], l[1], l[2]) == AIR {
            chunk.set(l[0], l[1], l[2], LIFE_CRYSTAL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::generate_chunk;
    use crate::worldgen::terrain::{natural_block, surface_height};

    fn cp(x: i32, y: i32, z: i32) -> ChunkPos {
        ChunkPos { x, y, z }
    }

    fn all_in_area(seed: u32, chunks: i32) -> Vec<[i32; 3]> {
        let mut v = Vec::new();
        for x in 0..chunks {
            for z in 0..chunks {
                for y in 0..crate::worldgen::terrain::WORLD_CHUNK_LAYERS {
                    v.extend(crystals_in(seed, cp(x, y, z)));
                }
            }
        }
        v
    }

    #[test]
    fn crystals_are_deterministic_and_chunk_order_independent() {
        let seed = 3;
        let a = all_in_area(seed, 6);
        assert!(!a.is_empty());
        assert_eq!(a, all_in_area(seed, 6));
        assert_ne!(a, all_in_area(4, 6));
        // Generating neighbours first changes nothing in a crystal's chunk.
        let p = a[0];
        let c = ChunkPos::of_block(p[0], p[1], p[2]);
        let cold = generate_chunk(seed, c);
        for d in [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 1, 0)] {
            generate_chunk(seed, cp(c.x + d.0, c.y + d.1, c.z + d.2));
        }
        assert_eq!(cold, generate_chunk(seed, c));
        let l = [p[0], p[1], p[2]].map(|v| v.rem_euclid(CHUNK_SIZE) as usize);
        assert_eq!(cold.get(l[0], l[1], l[2]), LIFE_CRYSTAL);
    }

    #[test]
    fn crystals_sit_on_stone_in_air_pockets() {
        for p in all_in_area(1, 10) {
            assert_eq!(natural_block(1, p[0], p[1] - 1, p[2]), STONE);
            for d in 0..CLEARANCE {
                assert_eq!(natural_block(1, p[0], p[1] + d, p[2]), AIR);
            }
        }
    }

    #[test]
    fn crystals_only_below_min_depth() {
        for seed in [1, 2] {
            for p in all_in_area(seed, 10) {
                assert!(
                    surface_height(seed, p[0], p[2]) - p[1] >= MIN_DEPTH,
                    "{p:?}"
                );
            }
        }
    }

    #[test]
    fn crystal_density_in_target_range() {
        // 512x512 columns (16x16 chunks): about 4-8 crystals per seed on average.
        let n: usize = [1, 2, 3, 4, 5, 6]
            .iter()
            .map(|&s| all_in_area(s, 16).len())
            .sum();
        let avg = n as f64 / 6.0;
        assert!((4.0..=8.0).contains(&avg), "avg {avg}");
    }
}
