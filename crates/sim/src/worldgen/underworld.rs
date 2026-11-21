//! Underworld worldgen: ash, hellstone veins, lava sea, lava lakes, and ruined houses.

use crate::block::*;
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::liquid_cell::Liquid;
use crate::world_bounds::WorldSize;
use crate::worldgen::noise::{fbm2, fbm3, hash};

pub const UNDERWORLD_TOP_Y: i32 = 48;

const SALT_HELLSTONE: u32 = 0x8E11_5701;
const SALT_HOUSE: u32 = 0x484F_5553;
const SALT_HOUSE_POS: u32 = 0x504F_5348;
const SALT_LAKE: u32 = 0x4C41_4B45;
const SALT_LAKE_POS: u32 = 0x504F_534C;
const SALT_LAKE_PARAM: u32 = 0x5041_524C;
const SALT_CAVERN_FLOOR: u32 = 0x5557_464C;
const SALT_CAVERN_CEIL: u32 = 0x5557_4345;
const SALT_CAVERN_PILLAR: u32 = 0x5557_5049;

const CELL_SIZE: i32 = 96;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuinedHouse {
    pub chest: [i32; 3],
    pub hellforge: [i32; 3],
    pub min: [i32; 3],
    pub max: [i32; 3],
}

pub fn house_in_cell(seed: u32, cell_x: i32, cell_z: i32) -> Option<RuinedHouse> {
    if hash(seed ^ SALT_HOUSE, cell_x, cell_z, 0) & 1 != 0 {
        return None;
    }
    let ox = cell_x * CELL_SIZE;
    let oz = cell_z * CELL_SIZE;
    let hx = ox
        + 10
        + (hash(seed ^ SALT_HOUSE_POS, cell_x, cell_z, 1) % ((CELL_SIZE - 7 - 20) as u32)) as i32;
    let hz = oz
        + 10
        + (hash(seed ^ SALT_HOUSE_POS, cell_x, cell_z, 2) % ((CELL_SIZE - 7 - 20) as u32)) as i32;
    let hy = 8 + (hash(seed ^ SALT_HOUSE_POS, cell_x, cell_z, 3) % 15) as i32;

    Some(RuinedHouse {
        min: [hx, hy, hz],
        max: [hx + 6, hy + 4, hz + 6],
        hellforge: [hx + 1, hy + 1, hz + 5],
        chest: [hx + 3, hy + 1, hz + 3],
    })
}

struct LavaLake {
    cx: i32,
    cz: i32,
    r: i32,
    level: i32,
}

fn lake_in_cell(seed: u32, cell_x: i32, cell_z: i32) -> Option<LavaLake> {
    if hash(seed ^ SALT_LAKE, cell_x, cell_z, 0) & 1 != 0 {
        return None;
    }
    let ox = cell_x * CELL_SIZE;
    let oz = cell_z * CELL_SIZE;
    let cx = ox
        + 20
        + (hash(seed ^ SALT_LAKE_POS, cell_x, cell_z, 1) % ((CELL_SIZE - 40) as u32)) as i32;
    let cz = oz
        + 20
        + (hash(seed ^ SALT_LAKE_POS, cell_x, cell_z, 2) % ((CELL_SIZE - 40) as u32)) as i32;
    let r = 5 + (hash(seed ^ SALT_LAKE_PARAM, cell_x, cell_z, 3) % 5) as i32;
    let level = 10 + (hash(seed ^ SALT_LAKE_PARAM, cell_x, cell_z, 4) % 10) as i32;

    Some(LavaLake { cx, cz, r, level })
}

pub fn houses_in_for_size(seed: u32, pos: ChunkPos, _size: WorldSize) -> Vec<RuinedHouse> {
    let cs = CHUNK_SIZE;
    let chunk_lo_x = pos.x * cs;
    let chunk_hi_x = (pos.x + 1) * cs;
    let chunk_lo_y = pos.y * cs;
    let chunk_hi_y = (pos.y + 1) * cs;
    let chunk_lo_z = pos.z * cs;
    let chunk_hi_z = (pos.z + 1) * cs;

    if chunk_lo_y > UNDERWORLD_TOP_Y || chunk_hi_y <= 0 {
        return Vec::new();
    }

    let cx_min = (chunk_lo_x - 7).div_euclid(CELL_SIZE);
    let cx_max = (chunk_hi_x + 7).div_euclid(CELL_SIZE);
    let cz_min = (chunk_lo_z - 7).div_euclid(CELL_SIZE);
    let cz_max = (chunk_hi_z + 7).div_euclid(CELL_SIZE);

    let mut out = Vec::new();
    for cx in cx_min..=cx_max {
        for cz in cz_min..=cz_max {
            if let Some(h) = house_in_cell(seed, cx, cz) {
                let overlaps_x = h.min[0] < chunk_hi_x && h.max[0] >= chunk_lo_x;
                let overlaps_y = h.min[1] < chunk_hi_y && h.max[1] >= chunk_lo_y;
                let overlaps_z = h.min[2] < chunk_hi_z && h.max[2] >= chunk_lo_z;
                if overlaps_x && overlaps_y && overlaps_z {
                    out.push(h);
                }
            }
        }
    }
    out
}

pub fn is_house_chest(seed: u32, pos: [i32; 3], _size: WorldSize) -> bool {
    if pos[1] < 0 || pos[1] > UNDERWORLD_TOP_Y {
        return false;
    }
    let cell_x = pos[0].div_euclid(CELL_SIZE);
    let cell_z = pos[2].div_euclid(CELL_SIZE);
    if let Some(h) = house_in_cell(seed, cell_x, cell_z) {
        h.chest == pos
    } else {
        false
    }
}

pub fn apply_for_size(seed: u32, pos: ChunkPos, _size: WorldSize, chunk: &mut Chunk) {
    let cs = CHUNK_SIZE;
    let chunk_lo_x = pos.x * cs;
    let chunk_hi_x = (pos.x + 1) * cs;
    let chunk_lo_y = pos.y * cs;
    let chunk_hi_y = (pos.y + 1) * cs;
    let chunk_lo_z = pos.z * cs;
    let chunk_hi_z = (pos.z + 1) * cs;

    if chunk_lo_y > UNDERWORLD_TOP_Y || chunk_hi_y <= 0 {
        return;
    }

    // Collect houses in nearby cells that might intersect or affect this chunk
    let cx_min = (chunk_lo_x - 12).div_euclid(CELL_SIZE);
    let cx_max = (chunk_hi_x + 12).div_euclid(CELL_SIZE);
    let cz_min = (chunk_lo_z - 12).div_euclid(CELL_SIZE);
    let cz_max = (chunk_hi_z + 12).div_euclid(CELL_SIZE);

    let mut houses = Vec::new();
    let mut lakes = Vec::new();
    for cx in cx_min..=cx_max {
        for cz in cz_min..=cz_max {
            if let Some(h) = house_in_cell(seed, cx, cz) {
                houses.push(h);
            }
            if let Some(l) = lake_in_cell(seed, cx, cz) {
                lakes.push(l);
            }
        }
    }

    let in_any_house = |wx: i32, wy: i32, wz: i32| -> bool {
        houses.iter().any(|h| {
            wx >= h.min[0]
                && wx <= h.max[0]
                && wy >= h.min[1]
                && wy <= h.max[1]
                && wz >= h.min[2]
                && wz <= h.max[2]
        })
    };

    // 1. Shape open Underworld cavern: undulating Ash floor (6..15) and hanging Ash ceiling (38..48),
    //    with occasional Ash pillars, replacing remaining STONE with ASH.
    for lx in 0..cs as usize {
        let wx = chunk_lo_x + lx as i32;
        for lz in 0..cs as usize {
            let wz = chunk_lo_z + lz as i32;
            let floor_n = fbm2(
                seed ^ SALT_CAVERN_FLOOR,
                wx as f64,
                wz as f64,
                1.0 / 32.0,
                3,
            );
            let ceil_n = fbm2(seed ^ SALT_CAVERN_CEIL, wx as f64, wz as f64, 1.0 / 28.0, 3);
            // Map noise [-1, 1] -> floor_y in [5, 16] and ceil_y in [36, 48].
            let floor_y = (10.5 + floor_n * 5.5).round().clamp(5.0, 16.0) as i32;
            let ceil_y = (42.0 + ceil_n * 6.0).round().clamp(36.0, 48.0) as i32;

            for ly in 0..cs as usize {
                let wy = chunk_lo_y + ly as i32;
                if wy > 0 && wy <= UNDERWORLD_TOP_Y {
                    if wy > floor_y && wy < ceil_y && !in_any_house(wx, wy, wz) {
                        let pillar_n = fbm3(
                            seed ^ SALT_CAVERN_PILLAR,
                            wx as f64,
                            wy as f64 * 0.45,
                            wz as f64,
                            1.0 / 18.0,
                            2,
                        );
                        if pillar_n > 0.48 {
                            chunk.set(lx, ly, lz, ASH);
                        } else {
                            chunk.set(lx, ly, lz, AIR);
                            chunk.set_liquid(lx, ly, lz, None);
                        }
                    } else if chunk.get(lx, ly, lz) == STONE {
                        chunk.set(lx, ly, lz, ASH);
                    }
                }
            }
        }
    }

    // 2. Hellstone veins: fbm3 > 0.55 replaces ash, skipping house bounds
    for lx in 0..cs as usize {
        let wx = chunk_lo_x + lx as i32;
        for lz in 0..cs as usize {
            let wz = chunk_lo_z + lz as i32;
            for ly in 0..cs as usize {
                let wy = chunk_lo_y + ly as i32;
                if wy > 0 && wy <= UNDERWORLD_TOP_Y {
                    if chunk.get(lx, ly, lz) == ASH && !in_any_house(wx, wy, wz) {
                        let val = fbm3(
                            seed ^ SALT_HELLSTONE,
                            wx as f64 / 8.0,
                            wy as f64 / 8.0,
                            wz as f64 / 8.0,
                            1.0 / 6.0,
                            2,
                        );
                        if val > 0.55 {
                            chunk.set(lx, ly, lz, HELLSTONE);
                        }
                    }
                }
            }
        }
    }

    // 3. Lava sea: y == 0 becomes ASH; y in 1..=6 (where open or below sea level) becomes AIR + Liquid::lava(255); skip house bounds
    for lx in 0..cs as usize {
        let wx = chunk_lo_x + lx as i32;
        for lz in 0..cs as usize {
            let wz = chunk_lo_z + lz as i32;
            for ly in 0..cs as usize {
                let wy = chunk_lo_y + ly as i32;
                if wy == 0 && !in_any_house(wx, wy, wz) {
                    chunk.set(lx, ly, lz, ASH);
                } else if (1..=6).contains(&wy)
                    && !in_any_house(wx, wy, wz)
                    && (wy <= 3 || chunk.get(lx, ly, lz) == AIR)
                {
                    chunk.set(lx, ly, lz, AIR);
                    chunk.set_liquid(lx, ly, lz, Some(Liquid::lava(255)));
                }
            }
        }
    }

    // 4. Lava lakes: radius 5-9, level 7-18; build a solid ASH volcanic basin
    //    (floor at level-3, rim at level-2..=level-1) and fill interior level-2..=level-1 with lava,
    //    skipping any lake whose footprint overlaps a ruined house.
    for lake in &lakes {
        let outer_r = lake.r + 2;
        let overlaps_house = houses.iter().any(|h| {
            lake.cx + outer_r >= h.min[0]
                && lake.cx - outer_r <= h.max[0]
                && lake.cz + outer_r >= h.min[2]
                && lake.cz - outer_r <= h.max[2]
                && lake.level >= h.min[1]
                && lake.level - 3 <= h.max[1]
        });
        if overlaps_house {
            continue;
        }
        let min_x = (lake.cx - outer_r).max(chunk_lo_x);
        let max_x = (lake.cx + outer_r).min(chunk_hi_x - 1);
        let min_z = (lake.cz - outer_r).max(chunk_lo_z);
        let max_z = (lake.cz + outer_r).min(chunk_hi_z - 1);
        let r_sq = lake.r * lake.r;
        let floor_r_sq = (lake.r + 1) * (lake.r + 1);
        let outer_r_sq = outer_r * outer_r;

        for wx in min_x..=max_x {
            let dx = wx - lake.cx;
            let lx = (wx - chunk_lo_x) as usize;
            for wz in min_z..=max_z {
                let dz = wz - lake.cz;
                let dist_sq = dx * dx + dz * dz;
                let lz = (wz - chunk_lo_z) as usize;

                // Basin floor at level - 3
                let floor_y = lake.level - 3;
                if floor_y >= chunk_lo_y
                    && floor_y < chunk_hi_y
                    && floor_y >= 0
                    && dist_sq <= floor_r_sq
                    && !in_any_house(wx, floor_y, wz)
                {
                    let ly = (floor_y - chunk_lo_y) as usize;
                    chunk.set(lx, ly, lz, ASH);
                    chunk.set_liquid(lx, ly, lz, None);
                }

                // Containing ASH rim around the lava pool
                if dist_sq > r_sq && dist_sq <= outer_r_sq {
                    for wy in (lake.level - 2)..=(lake.level - 1) {
                        if wy >= chunk_lo_y
                            && wy < chunk_hi_y
                            && wy >= 0
                            && !in_any_house(wx, wy, wz)
                        {
                            let ly = (wy - chunk_lo_y) as usize;
                            chunk.set(lx, ly, lz, ASH);
                            chunk.set_liquid(lx, ly, lz, None);
                        }
                    }
                }

                // Carve interior and fill lava
                if dist_sq <= r_sq {
                    for wy in (lake.level - 2)..=lake.level {
                        if wy >= chunk_lo_y
                            && wy < chunk_hi_y
                            && wy >= 0
                            && !in_any_house(wx, wy, wz)
                        {
                            let ly = (wy - chunk_lo_y) as usize;
                            chunk.set(lx, ly, lz, AIR);
                            if wy <= lake.level - 1 {
                                chunk.set_liquid(lx, ly, lz, Some(Liquid::lava(255)));
                            } else {
                                chunk.set_liquid(lx, ly, lz, None);
                            }
                        }
                    }
                }
            }
        }
    }

    // 5. Ruined houses: outer 7x5x7 obsidian-brick shell with hellstone-brick floor, hollow 5x3x5 interior, 2x3 doorway, HELLFORGE at inner corner, chest block position at centre
    for h in &houses {
        let min_x = h.min[0].max(chunk_lo_x);
        let max_x = h.max[0].min(chunk_hi_x - 1);
        let min_y = h.min[1].max(chunk_lo_y);
        let max_y = h.max[1].min(chunk_hi_y - 1);
        let min_z = h.min[2].max(chunk_lo_z);
        let max_z = h.max[2].min(chunk_hi_z - 1);

        for wx in min_x..=max_x {
            let lx = (wx - chunk_lo_x) as usize;
            for wz in min_z..=max_z {
                let lz = (wz - chunk_lo_z) as usize;
                for wy in min_y..=max_y {
                    let ly = (wy - chunk_lo_y) as usize;

                    // Doorway on wall wz == h.min[2]: x in h.min[0]+2..=h.min[0]+3, y in h.min[1]+1..=h.min[1]+3
                    if (h.min[0] + 2..=h.min[0] + 3).contains(&wx)
                        && (h.min[1] + 1..=h.min[1] + 3).contains(&wy)
                        && wz == h.min[2]
                    {
                        chunk.set(lx, ly, lz, AIR);
                        chunk.set_liquid(lx, ly, lz, None);
                    } else if [wx, wy, wz] == h.hellforge {
                        chunk.set(lx, ly, lz, HELLFORGE);
                        chunk.set_liquid(lx, ly, lz, None);
                    } else if [wx, wy, wz] == h.chest {
                        chunk.set(lx, ly, lz, AIR);
                        chunk.set_liquid(lx, ly, lz, None);
                    } else if wy == h.min[1] {
                        chunk.set(lx, ly, lz, HELLSTONE_BRICK);
                        chunk.set_liquid(lx, ly, lz, None);
                    } else if (h.min[0] + 1..=h.min[0] + 5).contains(&wx)
                        && (h.min[1] + 1..=h.min[1] + 3).contains(&wy)
                        && (h.min[2] + 1..=h.min[2] + 5).contains(&wz)
                    {
                        chunk.set(lx, ly, lz, AIR);
                        chunk.set_liquid(lx, ly, lz, None);
                    } else {
                        chunk.set(lx, ly, lz, OBSIDIAN_BRICK);
                        chunk.set_liquid(lx, ly, lz, None);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::generate_chunk_sized;

    #[test]
    fn underworld_has_ash_hellstone_lava_and_houses() {
        let seed = 42;
        let size = WorldSize::Small;
        let mut ash_count = 0;
        let mut hellstone_count = 0;
        let mut lava_count = 0;
        let mut brick_count = 0;
        let mut all_houses = Vec::new();
        let mut chunk_map = std::collections::HashMap::new();

        for x in 0..12 {
            for y in 0..=1 {
                for z in 0..12 {
                    let pos = ChunkPos { x, y, z };
                    let chunk = generate_chunk_sized(seed, pos, size);
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
                                    if liq.kind == crate::liquid_cell::LiquidKind::Lava {
                                        lava_count += 1;
                                    }
                                }
                            }
                        }
                    }
                    all_houses.extend(houses_in_for_size(seed, pos, size));
                    chunk_map.insert(pos, chunk);
                }
            }
        }

        assert!(
            ash_count > 1000,
            "ash_count was {ash_count}, expected > 1000"
        );
        assert!(
            hellstone_count > 50,
            "hellstone_count was {hellstone_count}, expected > 50"
        );
        assert!(
            lava_count > 100,
            "lava_count was {lava_count}, expected > 100"
        );
        assert!(
            brick_count > 100,
            "brick_count was {brick_count}, expected > 100"
        );
        assert!(!all_houses.is_empty(), "expected at least one ruined house");

        let house = all_houses.iter().find(|h| {
            let cp = ChunkPos::of_block(h.chest[0], h.chest[1], h.chest[2]);
            if let Some(ch) = chunk_map.get(&cp) {
                let lo = [cp.x * CHUNK_SIZE, cp.y * CHUNK_SIZE, cp.z * CHUNK_SIZE];
                let l = [0, 1, 2].map(|i| (h.chest[i] - lo[i]) as usize);
                let b = ch.get(l[0], l[1], l[2]);
                (b == AIR || b == CHEST)
                    && (0..3).all(|i| h.hellforge[i] >= h.min[i] && h.hellforge[i] <= h.max[i])
            } else {
                false
            }
        });
        assert!(
            house.is_some(),
            "expected at least one house whose chest is AIR/CHEST and whose hellforge is within house bounds"
        );
    }

    #[test]
    fn underworld_lava_is_supported_and_not_floating() {
        let seed = 42;
        let size = WorldSize::Small;
        let mut chunk_map = std::collections::HashMap::new();
        for x in 0..8 {
            for y in 0..=1 {
                for z in 0..8 {
                    let pos = ChunkPos { x, y, z };
                    chunk_map.insert(pos, generate_chunk_sized(seed, pos, size));
                }
            }
        }
        let get_block = |wx: i32, wy: i32, wz: i32| {
            let cp = ChunkPos::of_block(wx, wy, wz);
            chunk_map.get(&cp).map(|c| {
                c.get(
                    wx.rem_euclid(CHUNK_SIZE) as usize,
                    wy.rem_euclid(CHUNK_SIZE) as usize,
                    wz.rem_euclid(CHUNK_SIZE) as usize,
                )
            })
        };
        let get_liq = |wx: i32, wy: i32, wz: i32| {
            let cp = ChunkPos::of_block(wx, wy, wz);
            chunk_map.get(&cp).and_then(|c| {
                c.liquid(
                    wx.rem_euclid(CHUNK_SIZE) as usize,
                    wy.rem_euclid(CHUNK_SIZE) as usize,
                    wz.rem_euclid(CHUNK_SIZE) as usize,
                )
            })
        };

        for cx in 1..7 {
            for cz in 1..7 {
                for cy in 0..=1 {
                    let cp = ChunkPos {
                        x: cx,
                        y: cy,
                        z: cz,
                    };
                    let chunk = &chunk_map[&cp];
                    for lz in 0..CHUNK_SIZE {
                        for ly in 0..CHUNK_SIZE {
                            for lx in 0..CHUNK_SIZE {
                                let wx = cx * CHUNK_SIZE + lx;
                                let wy = cy * CHUNK_SIZE + ly;
                                let wz = cz * CHUNK_SIZE + lz;
                                if let Some(liq) =
                                    chunk.liquid(lx as usize, ly as usize, lz as usize)
                                {
                                    if liq.kind == crate::liquid_cell::LiquidKind::Lava {
                                        let below_b = get_block(wx, wy - 1, wz);
                                        let below_l = get_liq(wx, wy - 1, wz);
                                        assert!(
                                            below_b.is_some_and(|b| b.is_solid())
                                                || below_l.is_some(),
                                            "floating lava at ({wx},{wy},{wz})"
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
}
