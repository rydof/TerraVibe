//! Jungle Bee Hives: hollow `HIVE_BLOCK` chambers with `LiquidKind::Honey` and `BEE_LARVA`.

use super::biome::{Biome, biome_weights_for_size, dominant};
use super::noise::hash;
use crate::block::{AIR, BEE_LARVA, HIVE_BLOCK};
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::liquid_cell::Liquid;
use crate::world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, WorldBounds, WorldSize};

const SALT_BEE_HIVE: u32 = 0x4849_5645;

#[derive(Clone, Copy, Debug)]
pub struct BeeHive {
    pub cx: i32,
    pub cy: i32,
    pub cz: i32,
    pub radius: i32,
    pub larva: [i32; 3],
}

pub fn hives_for_size(seed: u32, size: WorldSize) -> Vec<BeeHive> {
    let target = match size {
        WorldSize::Small => 2,
        WorldSize::Medium => 3,
        WorldSize::Large => 4,
    };
    let bounds = WorldBounds::new(size);
    let safe_r = (bounds.half - OCEAN_WIDTH - BEACH_WIDTH - 24).max(64);
    let mut out = Vec::with_capacity(target);
    for attempt in 0..64 {
        if out.len() >= target {
            break;
        }
        let h = hash(seed ^ SALT_BEE_HIVE, attempt, 0, 2);
        let angle = ((h & 0xFFFF) as f64 / 65535.0) * std::f64::consts::TAU;
        let dist = 56.0 + (((h >> 16) & 0xFF) as f64 / 255.0) * (safe_r as f64 - 56.0).max(32.0);
        let cx = (angle.cos() * dist).round() as i32;
        let cz = (angle.sin() * dist).round() as i32;
        if dominant(&biome_weights_for_size(seed, cx, cz, size)) != Biome::Jungle {
            continue;
        }
        let cy = 115 + ((h >> 24) % 85) as i32;
        if out
            .iter()
            .any(|b: &BeeHive| (b.cx - cx).abs() < 28 && (b.cz - cz).abs() < 28)
        {
            continue;
        }
        out.push(BeeHive {
            cx,
            cy,
            cz,
            radius: 7,
            larva: [cx, cy - 1, cz],
        });
    }
    out
}

pub fn apply_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let cs = CHUNK_SIZE;
    let (bx, by, bz) = (pos.x * cs, pos.y * cs, pos.z * cs);
    if by + cs <= 100 || by >= 225 {
        return;
    }

    for hive in hives_for_size(seed, size) {
        let r = hive.radius;
        if hive.cx + r < bx
            || hive.cx - r >= bx + cs
            || hive.cz + r < bz
            || hive.cz - r >= bz + cs
            || hive.cy + r < by
            || hive.cy - r >= by + cs
        {
            continue;
        }
        let x0 = (hive.cx - r).max(bx);
        let x1 = (hive.cx + r).min(bx + cs - 1);
        let y0 = (hive.cy - r).max(by);
        let y1 = (hive.cy + r).min(by + cs - 1);
        let z0 = (hive.cz - r).max(bz);
        let z1 = (hive.cz + r).min(bz + cs - 1);

        for wz in z0..=z1 {
            let dz = wz - hive.cz;
            let lz = (wz - bz) as usize;
            for wy in y0..=y1 {
                let dy = wy - hive.cy;
                let ly = (wy - by) as usize;
                for wx in x0..=x1 {
                    let dx = wx - hive.cx;
                    let lx = (wx - bx) as usize;
                    let d2 = dx * dx + dy * dy + dz * dz;
                    if d2 > r * r {
                        continue;
                    }
                    if d2 >= (r - 2) * (r - 2) {
                        chunk.set(lx, ly, lz, HIVE_BLOCK);
                        chunk.set_liquid(lx, ly, lz, None);
                    } else {
                        chunk.set(lx, ly, lz, AIR);
                        if wy <= hive.cy - 2 {
                            chunk.set_liquid(lx, ly, lz, Some(Liquid::honey(255)));
                        } else {
                            chunk.set_liquid(lx, ly, lz, None);
                        }
                    }
                }
            }
        }
        let [lx_w, ly_w, lz_w] = hive.larva;
        if lx_w >= bx
            && lx_w < bx + cs
            && ly_w >= by
            && ly_w < by + cs
            && lz_w >= bz
            && lz_w < bz + cs
        {
            let lx = (lx_w - bx) as usize;
            let ly = (ly_w - by) as usize;
            let lz = (lz_w - bz) as usize;
            chunk.set(lx, ly - 1, lz, HIVE_BLOCK);
            chunk.set(lx, ly, lz, BEE_LARVA);
            chunk.set_liquid(lx, ly, lz, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LiquidKind;
    use crate::worldgen::terrain::generate_chunk_sized;

    #[test]
    fn bee_hive_generates_hive_honey_and_larva() {
        let hives = hives_for_size(99, WorldSize::Medium);
        assert!(!hives.is_empty(), "expected at least one jungle bee hive");
        let h = hives[0];
        let pos = ChunkPos::of_block(h.larva[0], h.larva[1], h.larva[2]);
        let ch = generate_chunk_sized(99, pos, WorldSize::Medium);
        let lx = h.larva[0].rem_euclid(CHUNK_SIZE) as usize;
        let ly = h.larva[1].rem_euclid(CHUNK_SIZE) as usize;
        let lz = h.larva[2].rem_euclid(CHUNK_SIZE) as usize;
        assert_eq!(ch.get(lx, ly, lz), BEE_LARVA);
        let mut saw_honey = false;
        for z in 0..32 {
            for y in 0..32 {
                for x in 0..32 {
                    if ch
                        .liquid(x, y, z)
                        .is_some_and(|l| l.kind == LiquidKind::Honey)
                    {
                        saw_honey = true;
                    }
                }
            }
        }
        assert!(saw_honey, "bee hive should contain a honey pool");
    }
}
