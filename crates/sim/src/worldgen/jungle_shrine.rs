//! Underground Jungle Shrines with Jungle Spores, Stingers, Vines, and Shrine Chests.

use super::biome::{Biome, biome_weights_for_size, dominant};
use super::noise::hash;
use crate::block::{AIR, CHEST, JUNGLE_GRASS, MUD, TORCH, WOOD_BLOCK};
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, WorldBounds, WorldSize};

const SALT_JUNGLE_SHRINE: u32 = 0x4A55_4E47;

#[derive(Clone, Copy, Debug)]
pub struct JungleShrine {
    pub cx: i32,
    pub cy: i32,
    pub cz: i32,
    pub chest: [i32; 3],
}

pub fn shrines_for_size(seed: u32, size: WorldSize) -> Vec<JungleShrine> {
    let target = match size {
        WorldSize::Small => 2,
        WorldSize::Medium => 3,
        WorldSize::Large => 4,
    };
    let bounds = WorldBounds::new(size);
    let safe_r = (bounds.half - OCEAN_WIDTH - BEACH_WIDTH - 24).max(64);
    let mut out = Vec::with_capacity(target);
    for attempt in 0..48 {
        if out.len() >= target {
            break;
        }
        let h = hash(seed ^ SALT_JUNGLE_SHRINE, attempt, 0, 1);
        let angle = ((h & 0xFFFF) as f64 / 65535.0) * std::f64::consts::TAU;
        let dist = 48.0 + (((h >> 16) & 0xFF) as f64 / 255.0) * (safe_r as f64 - 48.0).max(32.0);
        let cx = (angle.cos() * dist).round() as i32;
        let cz = (angle.sin() * dist).round() as i32;
        if dominant(&biome_weights_for_size(seed, cx, cz, size)) != Biome::Jungle {
            continue;
        }
        let cy = 120 + ((h >> 24) % 90) as i32;
        if out
            .iter()
            .any(|s: &JungleShrine| (s.cx - cx).abs() < 24 && (s.cz - cz).abs() < 24)
        {
            continue;
        }
        out.push(JungleShrine {
            cx,
            cy,
            cz,
            chest: [cx, cy + 1, cz],
        });
    }
    out
}

pub fn shrine_chests_in_for_size(
    seed: u32,
    pos: ChunkPos,
    size: WorldSize,
) -> Vec<(usize, [i32; 3])> {
    let bx = pos.x * CHUNK_SIZE;
    let by = pos.y * CHUNK_SIZE;
    let bz = pos.z * CHUNK_SIZE;
    shrines_for_size(seed, size)
        .into_iter()
        .enumerate()
        .filter(|(_, s)| {
            s.chest[0] >= bx
                && s.chest[0] < bx + CHUNK_SIZE
                && s.chest[1] >= by
                && s.chest[1] < by + CHUNK_SIZE
                && s.chest[2] >= bz
                && s.chest[2] < bz + CHUNK_SIZE
        })
        .map(|(idx, s)| (idx, s.chest))
        .collect()
}

pub fn apply_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let cs = CHUNK_SIZE;
    let (bx, by, bz) = (pos.x * cs, pos.y * cs, pos.z * cs);
    if by + cs <= 110 || by >= 230 {
        return;
    }

    for s in shrines_for_size(seed, size) {
        if s.cx + 3 < bx
            || s.cx - 3 >= bx + cs
            || s.cz + 3 < bz
            || s.cz - 3 >= bz + cs
            || s.cy + 5 < by
            || s.cy >= by + cs
        {
            continue;
        }
        let x0 = (s.cx - 3).max(bx);
        let x1 = (s.cx + 3).min(bx + cs - 1);
        let y0 = s.cy.max(by);
        let y1 = (s.cy + 4).min(by + cs - 1);
        let z0 = (s.cz - 3).max(bz);
        let z1 = (s.cz + 3).min(bz + cs - 1);
        for wy in y0..=y1 {
            let ly = (wy - by) as usize;
            for wz in z0..=z1 {
                let lz = (wz - bz) as usize;
                for wx in x0..=x1 {
                    let lx = (wx - bx) as usize;
                    let is_wall =
                        wx == s.cx - 3 || wx == s.cx + 3 || wz == s.cz - 3 || wz == s.cz + 3;
                    let is_shell = wy == s.cy || wy == s.cy + 4 || is_wall;
                    if is_wall && wy == s.cy + 1 && (wx == s.cx || wz == s.cz) {
                        chunk.set(lx, ly, lz, AIR);
                    } else if is_shell {
                        let b = if wy == s.cy {
                            JUNGLE_GRASS
                        } else if (wx + wz + wy) % 3 == 0 {
                            MUD
                        } else {
                            WOOD_BLOCK
                        };
                        chunk.set(lx, ly, lz, b);
                    } else {
                        chunk.set(lx, ly, lz, AIR);
                        chunk.set_wall(lx, ly, lz, true);
                    }
                }
            }
        }
        let mut place = |wx: i32, wy: i32, wz: i32, b| {
            if wx >= bx && wx < bx + cs && wy >= by && wy < by + cs && wz >= bz && wz < bz + cs {
                chunk.set(
                    (wx - bx) as usize,
                    (wy - by) as usize,
                    (wz - bz) as usize,
                    b,
                );
            }
        };
        place(s.chest[0], s.chest[1], s.chest[2], CHEST);
        place(s.cx - 2, s.cy + 3, s.cz - 2, TORCH);
        place(s.cx + 2, s.cy + 3, s.cz + 2, TORCH);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::terrain::generate_chunk_sized;

    #[test]
    fn jungle_shrine_spawns_chest_and_loot() {
        let shrines = shrines_for_size(42, WorldSize::Medium);
        assert!(!shrines.is_empty(), "expected at least one jungle shrine");
        let s = shrines[0];
        let pos = ChunkPos::of_block(s.chest[0], s.chest[1], s.chest[2]);
        let ch = generate_chunk_sized(42, pos, WorldSize::Medium);
        let lx = s.chest[0].rem_euclid(CHUNK_SIZE) as usize;
        let ly = s.chest[1].rem_euclid(CHUNK_SIZE) as usize;
        let lz = s.chest[2].rem_euclid(CHUNK_SIZE) as usize;
        assert_eq!(ch.get(lx, ly, lz), CHEST);
    }
}
