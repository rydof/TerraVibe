//! Glowing Mushroom Biome worldgen: blue-glowing cavern pockets (`MUSHROOM_GRASS` + `GLOWING_MUSHROOM_PLANT`).

use super::noise::{fbm3, hash};
use crate::block::{AIR, DIRT, GLOWING_MUSHROOM_PLANT, MUD, MUSHROOM_GRASS, STONE};
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, WorldBounds, WorldSize};

const SALT_MUSH_BIOME: u32 = 0x4D55_5348;
const MUSH_Y_MIN: i32 = 76;
const MUSH_Y_MAX: i32 = 180;
const MUSH_THRESHOLD: f64 = 0.56;

fn mushroom_noise(seed: u32, wx: i32, wy: i32, wz: i32) -> f64 {
    fbm3(
        seed ^ SALT_MUSH_BIOME,
        wx as f64 / 42.0,
        wy as f64 / 28.0,
        wz as f64 / 42.0,
        1.0,
        2,
    )
}

pub fn is_mushroom_pocket(seed: u32, wx: i32, wy: i32, wz: i32, size: WorldSize) -> bool {
    if !(MUSH_Y_MIN..=MUSH_Y_MAX).contains(&wy) {
        return false;
    }
    let bounds = WorldBounds::new(size);
    if !bounds.contains_column(wx, wz)
        || bounds.edge_distance(wx, wz) < OCEAN_WIDTH + BEACH_WIDTH + 16
    {
        return false;
    }
    mushroom_noise(seed, wx, wy, wz) > MUSH_THRESHOLD
}

pub fn apply_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let cs = CHUNK_SIZE;
    let (bx, by, bz) = (pos.x * cs, pos.y * cs, pos.z * cs);
    if by + cs <= MUSH_Y_MIN || by > MUSH_Y_MAX {
        return;
    }

    let bounds = WorldBounds::new(size);
    let mut coarse_hit = false;
    for oz in [0, 16, 31] {
        for ox in [0, 16, 31] {
            if !bounds.contains_column(bx + ox, bz + oz)
                || bounds.edge_distance(bx + ox, bz + oz) < OCEAN_WIDTH + BEACH_WIDTH + 16
            {
                continue;
            }
            for oy in [0, 16, 31] {
                if mushroom_noise(seed, bx + ox, by + oy, bz + oz) > MUSH_THRESHOLD - 0.16 {
                    coarse_hit = true;
                    break;
                }
            }
            if coarse_hit {
                break;
            }
        }
        if coarse_hit {
            break;
        }
    }
    if !coarse_hit {
        return;
    }

    for lz in 0..cs as usize {
        let wz = bz + lz as i32;
        for lx in 0..cs as usize {
            let wx = bx + lx as i32;
            if !bounds.contains_column(wx, wz)
                || bounds.edge_distance(wx, wz) < OCEAN_WIDTH + BEACH_WIDTH + 16
            {
                continue;
            }
            for ly in 0..cs as usize {
                let wy = by + ly as i32;
                if !(MUSH_Y_MIN..=MUSH_Y_MAX).contains(&wy) {
                    continue;
                }
                let b = chunk.get(lx, ly, lz);
                if !matches!(b, STONE | DIRT | MUD) {
                    continue;
                }
                let exposed = (ly + 1 < cs as usize && chunk.get(lx, ly + 1, lz) == AIR)
                    || (ly > 0 && chunk.get(lx, ly - 1, lz) == AIR)
                    || (lx + 1 < cs as usize && chunk.get(lx + 1, ly, lz) == AIR)
                    || (lx > 0 && chunk.get(lx - 1, ly, lz) == AIR)
                    || (lz + 1 < cs as usize && chunk.get(lx, ly, lz + 1) == AIR)
                    || (lz > 0 && chunk.get(lx, ly, lz - 1) == AIR);
                if exposed && mushroom_noise(seed, wx, wy, wz) > MUSH_THRESHOLD {
                    chunk.set(lx, ly, lz, MUSHROOM_GRASS);
                }
            }
        }
    }

    // Second pass: place Glowing Mushroom plants on open AIR above Mushroom Grass.
    for lz in 0..cs as usize {
        let wz = bz + lz as i32;
        for lx in 0..cs as usize {
            let wx = bx + lx as i32;
            for ly in 1..cs as usize {
                let wy = by + ly as i32;
                if chunk.get(lx, ly, lz) == AIR
                    && chunk.get(lx, ly - 1, lz) == MUSHROOM_GRASS
                    && hash(seed ^ SALT_MUSH_BIOME ^ 0x1234, wx, wy, wz) % 4 == 0
                {
                    chunk.set(lx, ly, lz, GLOWING_MUSHROOM_PLANT);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::terrain::generate_chunk_sized;

    #[test]
    fn mushroom_biome_generates_grass_and_glowing_mushrooms() {
        let seed = 77;
        let mut grass = 0usize;
        let mut plants = 0usize;
        for cx in -3..=3 {
            for cz in -3..=3 {
                for cy in 2..=5 {
                    let ch = generate_chunk_sized(
                        seed,
                        ChunkPos {
                            x: cx,
                            y: cy,
                            z: cz,
                        },
                        WorldSize::Small,
                    );
                    for lz in 0..32 {
                        for ly in 0..32 {
                            for lx in 0..32 {
                                match ch.get(lx, ly, lz) {
                                    MUSHROOM_GRASS => grass += 1,
                                    GLOWING_MUSHROOM_PLANT => plants += 1,
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(grass > 20, "expected mushroom grass in cavern, got {grass}");
        assert!(
            plants > 0,
            "expected glowing mushroom plants in cavern, got {plants}"
        );
    }
}
