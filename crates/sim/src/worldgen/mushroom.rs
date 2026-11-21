//! Surface mushrooms: a non-solid plant on top of grass, a pure function of
//! `(seed, x, z)` so regenerating a chunk never moves or duplicates one.
//!
//! Deviation: vanilla grows them as tile 3 plants (`frameX = 144`,
//! gameplay spec, 1/50 per eligible grass tile with plant-friendly walls)
//! which has no counterpart in this columnar world, so the density is tuned to
//! about 1 per 32 surface grass columns (natural sparse herb patches, 1/25-1/40).

use crate::block::{AIR, GRASS, MUSHROOM_PLANT};
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::world_bounds::WorldSize;
use crate::worldgen::noise::hash;
use crate::worldgen::terrain;

const SALT_MUSHROOM: u32 = 0x2B6D_93E1;
/// One in this many grass columns carries a mushroom.
pub const MUSHROOM_ONE_IN: u32 = 32;

/// Y of the mushroom cell for this column, if it has one.
pub fn mushroom_in_column(seed: u32, x: i32, z: i32) -> Option<i32> {
    mushroom_in_column_for_size(seed, x, z, WorldSize::Small)
}

pub fn mushroom_in_column_for_size(seed: u32, x: i32, z: i32, size: WorldSize) -> Option<i32> {
    if hash(seed ^ SALT_MUSHROOM, x, 0, z) % MUSHROOM_ONE_IN != 0 {
        return None;
    }
    // Corruption replaces grass and hosts no mushrooms.
    if super::corruption::corruption_at_for_size(seed, x, z, size) > 0.0 {
        return None;
    }
    let (gy, top) = terrain::surface_top_for_size(seed, x, z, size);
    (top == GRASS).then_some(gy + 1)
}

/// Stamp this chunk's mushrooms. Runs after trees: never overwrites a trunk.
pub fn apply(seed: u32, pos: ChunkPos, chunk: &mut Chunk) {
    apply_for_size(seed, pos, WorldSize::Small, chunk);
}

pub fn apply_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let cs = CHUNK_SIZE;
    let (bx, by, bz) = (pos.x * cs, pos.y * cs, pos.z * cs);
    for lz in 0..cs as usize {
        for lx in 0..cs as usize {
            let Some(y) = mushroom_in_column_for_size(seed, bx + lx as i32, bz + lz as i32, size)
            else {
                continue;
            };
            if (by..by + cs).contains(&y) && chunk.get(lx, (y - by) as usize, lz) == AIR {
                chunk.set(lx, (y - by) as usize, lz, MUSHROOM_PLANT);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::generate_chunk;

    #[test]
    fn mushrooms_only_spawn_on_surface_grass() {
        let seed = 7;
        let mut n = 0;
        for cx in 0..6 {
            for cz in 0..6 {
                for cy in 0..crate::worldgen::terrain::WORLD_CHUNK_LAYERS {
                    let pos = ChunkPos {
                        x: cx,
                        y: cy,
                        z: cz,
                    };
                    let ch = generate_chunk(seed, pos);
                    for lx in 0..CHUNK_SIZE as usize {
                        for lz in 0..CHUNK_SIZE as usize {
                            for ly in 0..CHUNK_SIZE as usize {
                                if ch.get(lx, ly, lz) != MUSHROOM_PLANT {
                                    continue;
                                }
                                n += 1;
                                let (x, y, z) = (
                                    cx * CHUNK_SIZE + lx as i32,
                                    cy * CHUNK_SIZE + ly as i32,
                                    cz * CHUNK_SIZE + lz as i32,
                                );
                                let (gy, top) = terrain::surface_top(seed, x, z);
                                assert_eq!((y, top), (gy + 1, GRASS));
                            }
                        }
                    }
                }
            }
        }
        assert!(n > 0, "expected some mushrooms in a 96x96 area");
    }

    #[test]
    fn mushroom_density_is_one_per_25_to_40_grass_columns() {
        let seed = 7;
        let (mut grass, mut mushrooms) = (0u32, 0u32);
        for x in -64..64 {
            for z in -64..64 {
                let (_, top) = terrain::surface_top(seed, x, z);
                if top != GRASS || super::super::corruption::corruption_at(seed, x, z) > 0.0 {
                    continue;
                }
                grass += 1;
                if mushroom_in_column(seed, x, z).is_some() {
                    mushrooms += 1;
                }
            }
        }
        let ratio = f64::from(mushrooms) / f64::from(grass);
        assert!(
            (1.0 / 40.0..=1.0 / 25.0).contains(&ratio),
            "{mushrooms} mushrooms over {grass} grass columns = 1/{:.1}, want 1/25..1/40",
            1.0 / ratio
        );
    }

    #[test]
    fn mushroom_placement_is_deterministic() {
        let pos = ChunkPos { x: 2, y: 1, z: 3 };
        assert_eq!(generate_chunk(5, pos), generate_chunk(5, pos));
        assert_eq!(mushroom_in_column(5, 40, 50), mushroom_in_column(5, 40, 50));
    }
}
