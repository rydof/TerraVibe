//! Floating Islands with Cloud undersides, grassy caps, and Sunplate Skyware Houses.

use super::noise::hash;
use crate::block::{
    AIR, CHAIR, CHEST, CLOUD_BLOCK, DIRT, DOOR_CLOSED, GRASS, SUNPLATE_BLOCK, TABLE,
};
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, WorldBounds, WorldSize};

const SALT_SKY_ISLAND: u32 = 0x534B_5949;
pub const SKY_ISLAND_Y: i32 = 340;

#[derive(Clone, Copy, Debug)]
pub struct SkyIsland {
    pub cx: i32,
    pub cy: i32,
    pub cz: i32,
    pub radius: i32,
    pub chest: [i32; 3],
}

pub fn islands_for_size(seed: u32, size: WorldSize) -> Vec<SkyIsland> {
    let count = match size {
        WorldSize::Small => 2,
        WorldSize::Medium => 3,
        WorldSize::Large => 5,
    };
    let bounds = WorldBounds::new(size);
    let safe_r = (bounds.half - OCEAN_WIDTH - BEACH_WIDTH - 48).max(96);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let h = hash(seed ^ SALT_SKY_ISLAND, i as i32, 0, count as i32);
        let angle =
            (i as f64 / count as f64) * std::f64::consts::TAU + ((h & 0xFF) as f64 / 255.0) * 0.4;
        let dist = (safe_r as f64 * (0.35 + ((h >> 8) & 0xFF) as f64 / 510.0)).max(80.0);
        let cx = (angle.cos() * dist).round() as i32;
        let cz = (angle.sin() * dist).round() as i32;
        let cy = SKY_ISLAND_Y + ((h >> 16) % 9) as i32;
        let radius = 11 + ((h >> 20) % 4) as i32;
        let chest = [cx, cy + 2, cz];
        out.push(SkyIsland {
            cx,
            cy,
            cz,
            radius,
            chest,
        });
    }
    out
}

pub fn skyware_chests_in_for_size(
    seed: u32,
    pos: ChunkPos,
    size: WorldSize,
) -> Vec<(usize, [i32; 3])> {
    let bx = pos.x * CHUNK_SIZE;
    let by = pos.y * CHUNK_SIZE;
    let bz = pos.z * CHUNK_SIZE;
    islands_for_size(seed, size)
        .into_iter()
        .enumerate()
        .filter(|(_, isl)| {
            isl.chest[0] >= bx
                && isl.chest[0] < bx + CHUNK_SIZE
                && isl.chest[1] >= by
                && isl.chest[1] < by + CHUNK_SIZE
                && isl.chest[2] >= bz
                && isl.chest[2] < bz + CHUNK_SIZE
        })
        .map(|(idx, isl)| (idx, isl.chest))
        .collect()
}

pub fn apply_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let cs = CHUNK_SIZE;
    let (bx, by, bz) = (pos.x * cs, pos.y * cs, pos.z * cs);
    if by + cs <= 325 || by >= 370 {
        return;
    }

    for isl in islands_for_size(seed, size) {
        let r = isl.radius;
        if isl.cx + r < bx
            || isl.cx - r >= bx + cs
            || isl.cz + r < bz
            || isl.cz - r >= bz + cs
            || isl.cy + 7 < by
            || isl.cy - 6 >= by + cs
        {
            continue;
        }

        // 1. Cloud bowl + Dirt/Grass meadow cap
        let x0 = (isl.cx - r).max(bx);
        let x1 = (isl.cx + r).min(bx + cs - 1);
        let z0 = (isl.cz - r).max(bz);
        let z1 = (isl.cz + r).min(bz + cs - 1);
        for wz in z0..=z1 {
            let dz = wz - isl.cz;
            let lz = (wz - bz) as usize;
            for wx in x0..=x1 {
                let dx = wx - isl.cx;
                let lx = (wx - bx) as usize;
                let dist_sq = dx * dx + dz * dz;
                if dist_sq > r * r {
                    continue;
                }
                let norm = (dist_sq as f32 / (r * r) as f32).sqrt();
                let depth = ((1.0 - norm * norm) * 5.0).round() as i32 + 1;
                for wy in (isl.cy - depth)..=isl.cy + 1 {
                    if wy < by || wy >= by + cs {
                        continue;
                    }
                    let ly = (wy - by) as usize;
                    if wy == isl.cy + 1 {
                        if norm < 0.78 {
                            chunk.set(lx, ly, lz, GRASS);
                        }
                    } else if wy >= isl.cy - 1 && norm < 0.85 {
                        chunk.set(lx, ly, lz, DIRT);
                    } else {
                        chunk.set(lx, ly, lz, CLOUD_BLOCK);
                    }
                }
            }
        }

        // 2. Sunplate Skyware House (7x5x7 centered at cx, cz, floor at cy+1)
        let hx0 = isl.cx - 3;
        let hx1 = isl.cx + 3;
        let hy0 = isl.cy + 1;
        let hy1 = isl.cy + 5;
        let hz0 = isl.cz - 3;
        let hz1 = isl.cz + 3;
        for wy in hy0.max(by)..=hy1.min(by + cs - 1) {
            let ly = (wy - by) as usize;
            for wz in hz0.max(bz)..=hz1.min(bz + cs - 1) {
                let lz = (wz - bz) as usize;
                for wx in hx0.max(bx)..=hx1.min(bx + cs - 1) {
                    let lx = (wx - bx) as usize;
                    let is_wall = wx == hx0 || wx == hx1 || wz == hz0 || wz == hz1;
                    let is_shell = wy == hy0 || wy == hy1 || is_wall;
                    if wx == hx0 && wz == isl.cz && (wy == hy0 + 1 || wy == hy0 + 2) {
                        chunk.set(lx, ly, lz, DOOR_CLOSED);
                    } else if is_shell {
                        chunk.set(lx, ly, lz, SUNPLATE_BLOCK);
                    } else {
                        chunk.set(lx, ly, lz, AIR);
                        chunk.set_wall(lx, ly, lz, true);
                    }
                }
            }
        }
        // Furniture + Skyware Chest inside house
        let place = |chunk: &mut Chunk, wx: i32, wy: i32, wz: i32, b| {
            if wx >= bx && wx < bx + cs && wy >= by && wy < by + cs && wz >= bz && wz < bz + cs {
                chunk.set(
                    (wx - bx) as usize,
                    (wy - by) as usize,
                    (wz - bz) as usize,
                    b,
                );
            }
        };
        place(chunk, isl.chest[0], isl.chest[1], isl.chest[2], CHEST);
        place(chunk, isl.cx + 1, hy0 + 1, isl.cz + 1, TABLE);
        place(chunk, isl.cx + 1, hy0 + 1, isl.cz + 2, CHAIR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::terrain::generate_chunk_sized;

    #[test]
    fn floating_islands_generate_cloud_sunplate_and_skyware_chest() {
        let seed = 42;
        let islands = islands_for_size(seed, WorldSize::Small);
        assert_eq!(islands.len(), 2);
        let isl = islands[0];
        let cp = ChunkPos::of_block(isl.chest[0], isl.chest[1], isl.chest[2]);
        let ch = generate_chunk_sized(seed, cp, WorldSize::Small);
        let lx = isl.chest[0].rem_euclid(CHUNK_SIZE) as usize;
        let ly = isl.chest[1].rem_euclid(CHUNK_SIZE) as usize;
        let lz = isl.chest[2].rem_euclid(CHUNK_SIZE) as usize;
        assert_eq!(ch.get(lx, ly, lz), CHEST);
        // Below floor should have cloud/sunplate
        let mut saw_cloud = false;
        let mut saw_sunplate = false;
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    match ch.get(x, y, z) {
                        CLOUD_BLOCK => saw_cloud = true,
                        SUNPLATE_BLOCK => saw_sunplate = true,
                        _ => {}
                    }
                }
            }
        }
        assert!(saw_cloud, "island should have cloud blocks");
        assert!(saw_sunplate, "island should have sunplate house");
    }
}
