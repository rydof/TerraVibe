//! World generation of liquids: the ocean ring, surface ponds, underground
//! water pools and deep lava pools.
//!
//! Pure function of `(seed, pos, size)`: deterministic, chunk-local for ponds
//! and pools, and settled by construction so freshly generated chunks sleep
//! after one initial wake scan.

use crate::block::AIR;
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::liquid::AMOUNT_FULL;
use crate::liquid_cell::{Liquid, LiquidKind};
use crate::world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, SEA_LEVEL_Y, WorldBounds, WorldSize};
use crate::worldgen::biome::{Biome, biome_weights_for_size, dominant};
use crate::worldgen::corruption::corruption_at_for_size;
use crate::worldgen::noise::fbm3;
use crate::worldgen::terrain::{WORLD_HEIGHT, surface_height_for_size};

/// The plan's "220 below the surface datum" is deeper than this 128-block world,
/// so use absolute Y=24 (deep caves). Lava pools never appear above `LAVA_DEPTH_Y`.
pub const LAVA_DEPTH_Y: i32 = crate::worldgen::underworld::UNDERWORLD_TOP_Y;

const CS: usize = CHUNK_SIZE as usize;
/// No generated liquid in any column within this radius of the world origin.
const SPAWN_GUARD_RADIUS_SQ: i32 = 48 * 48;
/// Salt for the deterministic underground-pool gate.
const SALT_POOL: u32 = 0x504F_4F4C;
/// Noise threshold for starting a new pool on a cave floor. Tuned so pools are
/// common enough to be found without flooding the caves.
const POOL_GATE: f64 = 0.15;
/// A puddle grows at most this many cells tall (floor + three above).
const POOL_MAX_HEIGHT: i32 = 4;
/// Keep underground pools small; larger cave cross-sections are left dry.
const MAX_POOL_AREA: usize = 128;
/// Pools stay at least this many blocks below the column surface.
const POOL_MIN_DEPTH: i32 = 12;
/// Ponds and pools stay at least this many blocks from the chunk edge so the
/// whole feature (and every neighbour check) lies inside one chunk.
const EDGE_MARGIN: usize = 4;

/// Fills the ocean ring, surface ponds and underground pools of one chunk.
///
/// Runs after all block-generating passes and only ever writes liquid into
/// `AIR` cells.
pub fn apply(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let cs = CHUNK_SIZE;
    let (bx, by, bz) = (pos.x * cs, pos.y * cs, pos.z * cs);
    if by + cs <= 0 || i64::from(by) >= WORLD_HEIGHT {
        return;
    }

    let bounds = WorldBounds::new(size);
    let mut surf = [[0i32; CS]; CS];
    let mut edge_dist = [[0i32; CS]; CS];
    let mut blocked = [[false; CS]; CS];
    for lz in 0..CS {
        let wz = bz + lz as i32;
        for lx in 0..CS {
            let wx = bx + lx as i32;
            surf[lz][lx] = surface_height_for_size(seed, wx, wz, size);
            edge_dist[lz][lx] = bounds.edge_distance(wx, wz);
            blocked[lz][lx] =
                !bounds.contains_column(wx, wz) || wx * wx + wz * wz <= SPAWN_GUARD_RADIUS_SQ;
        }
    }

    fill_ocean(chunk, by, &edge_dist, &blocked);
    fill_ponds(seed, size, chunk, bx, by, bz, &surf, &edge_dist, &blocked);
    fill_pools(seed, chunk, bx, by, bz, &surf, &edge_dist, &blocked);
}

/// Every `AIR` cell at or below sea level in the coastal ring becomes water.
/// Task 8 terrain puts the ocean floor below sea level and the beach above it,
/// so this only ever fills the ocean itself.
fn fill_ocean(chunk: &mut Chunk, by: i32, edge_dist: &[[i32; CS]; CS], blocked: &[[bool; CS]; CS]) {
    if by > SEA_LEVEL_Y {
        return;
    }
    let y_max = SEA_LEVEL_Y.min(by + CHUNK_SIZE - 1);
    let y_min = by.max(0);
    let coastal_limit = OCEAN_WIDTH + BEACH_WIDTH;
    for lz in 0..CS {
        for lx in 0..CS {
            if blocked[lz][lx] || edge_dist[lz][lx] >= coastal_limit {
                continue;
            }
            for wy in y_min..=y_max {
                let ly = (wy - by) as usize;
                if ly < CS && chunk.get(lx, ly, lz) == AIR {
                    chunk.set_liquid(lx, ly, lz, Some(Liquid::water(AMOUNT_FULL)));
                }
            }
        }
    }
}

/// Noise-gated depressions in Forest and Jungle fill to their lowest rim.
fn fill_ponds(
    seed: u32,
    size: WorldSize,
    chunk: &mut Chunk,
    bx: i32,
    by: i32,
    bz: i32,
    surf: &[[i32; CS]; CS],
    edge_dist: &[[i32; CS]; CS],
    blocked: &[[bool; CS]; CS],
) {
    let coastal_limit = OCEAN_WIDTH + BEACH_WIDTH;
    let mut pond_rim = [[0i32; CS]; CS];
    let mut pond_ok = [[false; CS]; CS];
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;

    for lz in EDGE_MARGIN..CS - EDGE_MARGIN {
        let wz = bz + lz as i32;
        for lx in EDGE_MARGIN..CS - EDGE_MARGIN {
            let wx = bx + lx as i32;
            if blocked[lz][lx] || edge_dist[lz][lx] < coastal_limit {
                continue;
            }
            let w = biome_weights_for_size(seed, wx, wz, size);
            if !matches!(dominant(&w), Biome::Forest | Biome::Jungle) {
                continue;
            }
            if corruption_at_for_size(seed, wx, wz, size) > 0.0 {
                continue;
            }
            let s = surf[lz][lx];
            let rim = surf[lz][lx + 3]
                .min(surf[lz][lx - 3])
                .min(surf[lz + 3][lx])
                .min(surf[lz - 3][lx]);
            // At least two blocks below every cardinal neighbour at radius 3.
            if s > rim - 2 {
                continue;
            }
            let diag = surf[lz + 2][lx + 2]
                .min(surf[lz + 2][lx - 2])
                .min(surf[lz - 2][lx + 2])
                .min(surf[lz - 2][lx - 2]);
            // Also lower than all four diagonal neighbours at radius 2.
            if s >= diag {
                continue;
            }
            let y0 = s + 1;
            let y1 = rim - 1;
            // Keep the whole pond (and the cell below its floor) in this chunk.
            if y0 < by + 1 || y1 > by + CHUNK_SIZE - 1 {
                continue;
            }
            let mut all_air = true;
            for wy in y0..=y1 {
                let ly = (wy - by) as usize;
                if chunk.get(lx, ly, lz) != AIR {
                    all_air = false;
                    break;
                }
            }
            if !all_air {
                continue;
            }
            pond_rim[lz][lx] = rim;
            pond_ok[lz][lx] = true;
            min_y = min_y.min(y0);
            max_y = max_y.max(y1);
        }
    }

    // Fill bottom-up so every cell can check a solid or already-filled floor.
    for wy in min_y..=max_y {
        let ly = (wy - by) as usize;
        let mut cand = [[false; CS]; CS];
        let mut any = false;
        for lz in EDGE_MARGIN..CS - EDGE_MARGIN {
            for lx in EDGE_MARGIN..CS - EDGE_MARGIN {
                if !pond_ok[lz][lx] {
                    continue;
                }
                let s = surf[lz][lx];
                let rim = pond_rim[lz][lx];
                if wy <= s || wy >= rim || chunk.get(lx, ly, lz) != AIR {
                    continue;
                }
                let below = chunk.get(lx, ly - 1, lz).is_solid()
                    || chunk
                        .liquid(lx, ly - 1, lz)
                        .is_some_and(|l| l.kind == LiquidKind::Water);
                if below {
                    cand[lz][lx] = true;
                    any = true;
                }
            }
        }
        if !any {
            continue;
        }
        // A pond layer is valid only where every side is solid or water.
        loop {
            let mut changed = false;
            for lz in EDGE_MARGIN..CS - EDGE_MARGIN {
                for lx in EDGE_MARGIN..CS - EDGE_MARGIN {
                    if !cand[lz][lx] {
                        continue;
                    }
                    let walled = [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)]
                        .into_iter()
                        .all(|(dx, dz)| {
                            let nx = (lx as isize + dx) as usize;
                            let nz = (lz as isize + dz) as usize;
                            chunk.get(nx, ly, nz).is_solid()
                                || cand[nz][nx]
                                || chunk
                                    .liquid(nx, ly, nz)
                                    .is_some_and(|l| l.kind == LiquidKind::Water)
                        });
                    if !walled {
                        cand[lz][lx] = false;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        for lz in EDGE_MARGIN..CS - EDGE_MARGIN {
            for lx in EDGE_MARGIN..CS - EDGE_MARGIN {
                if cand[lz][lx] {
                    chunk.set_liquid(lx, ly, lz, Some(Liquid::water(AMOUNT_FULL)));
                }
            }
        }
    }
}

/// Cave floors selected by the pool gate grow flat-topped, walled puddles.
fn fill_pools(
    seed: u32,
    chunk: &mut Chunk,
    bx: i32,
    by: i32,
    bz: i32,
    surf: &[[i32; CS]; CS],
    edge_dist: &[[i32; CS]; CS],
    blocked: &[[bool; CS]; CS],
) {
    let coastal_limit = OCEAN_WIDTH + BEACH_WIDTH;
    // Height (number of liquid layers) and kind of the pool cell at the
    // previous Y. Only the current top layer is needed to continue a pool.
    let mut pool_height = [[0u8; CS]; CS];
    let mut pool_kind = [[LiquidKind::Water; CS]; CS];

    for ly in 1..CS {
        let wy = by + ly as i32;
        if wy < 1 || i64::from(wy) >= WORLD_HEIGHT {
            continue;
        }
        let kind = if wy <= LAVA_DEPTH_Y {
            LiquidKind::Lava
        } else {
            LiquidKind::Water
        };
        let target = Liquid::new(kind, AMOUNT_FULL);

        let mut prev_mask = [[false; CS]; CS];
        for lz in 0..CS {
            for lx in 0..CS {
                prev_mask[lz][lx] = pool_height[lz][lx] > 0 && pool_kind[lz][lx] == kind;
            }
        }

        // All cave AIR cells at this level. A pool fills a whole connected
        // cross-section, so its boundary is solid and every cell is supported
        // by solid or liquid below.
        let mut air = [[false; CS]; CS];
        let mut supported = [[false; CS]; CS];
        let mut any_air = false;
        for lz in EDGE_MARGIN..CS - EDGE_MARGIN {
            for lx in EDGE_MARGIN..CS - EDGE_MARGIN {
                if blocked[lz][lx] || edge_dist[lz][lx] < coastal_limit {
                    continue;
                }
                if wy > surf[lz][lx] - POOL_MIN_DEPTH {
                    continue;
                }
                if chunk.get(lx, ly, lz) != AIR || chunk.liquid(lx, ly, lz).is_some() {
                    continue;
                }
                air[lz][lx] = true;
                any_air = true;
                let below_solid = chunk.get(lx, ly - 1, lz).is_solid();
                let below_pool = prev_mask[lz][lx] && pool_height[lz][lx] < POOL_MAX_HEIGHT as u8;
                supported[lz][lx] = below_solid || below_pool;
            }
        }
        if !any_air {
            pool_height = [[0u8; CS]; CS];
            continue;
        }

        let mut visited = [[false; CS]; CS];
        let mut next_height = [[0u8; CS]; CS];
        let mut next_kind = [[LiquidKind::Water; CS]; CS];
        for lz in EDGE_MARGIN..CS - EDGE_MARGIN {
            for lx in EDGE_MARGIN..CS - EDGE_MARGIN {
                if !air[lz][lx] || visited[lz][lx] {
                    continue;
                }
                let mut comp = Vec::new();
                let mut stack = vec![(lx, lz)];
                visited[lz][lx] = true;
                while let Some((x, z)) = stack.pop() {
                    comp.push((x, z));
                    for (dx, dz) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                        let nx = x as isize + dx;
                        let nz = z as isize + dz;
                        if (EDGE_MARGIN as isize..(CS - EDGE_MARGIN) as isize).contains(&nx)
                            && (EDGE_MARGIN as isize..(CS - EDGE_MARGIN) as isize).contains(&nz)
                        {
                            let (nx, nz) = (nx as usize, nz as usize);
                            if air[nz][nx] && !visited[nz][nx] {
                                visited[nz][nx] = true;
                                stack.push((nx, nz));
                            }
                        }
                    }
                }

                // Every cell must be supported from below, and the whole
                // cross-section must be enclosed by solid blocks (or the
                // liquid already placed in this pool).
                let mut in_comp = [[false; CS]; CS];
                for &(x, z) in &comp {
                    in_comp[z][x] = true;
                }
                let mut enclosed = true;
                for &(x, z) in &comp {
                    if !supported[z][x] {
                        enclosed = false;
                        break;
                    }
                    for (dx, dz) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                        let nx = x as isize + dx;
                        let nz = z as isize + dz;
                        if nx < 0 || nx >= CS as isize || nz < 0 || nz >= CS as isize {
                            enclosed = false;
                            break;
                        }
                        let (nx, nz) = (nx as usize, nz as usize);
                        if in_comp[nz][nx] {
                            continue;
                        }
                        if !chunk.get(nx, ly, nz).is_solid() {
                            enclosed = false;
                            break;
                        }
                    }
                    if !enclosed {
                        break;
                    }
                }
                if !enclosed || comp.len() > MAX_POOL_AREA {
                    continue;
                }

                let has_cont = comp.iter().any(|&(x, z)| prev_mask[z][x]);
                if has_cont {
                    // A pool may only rise if every column of the pool below
                    // rises with it; otherwise the top would not be flat.
                    let mut lower_visited = [[false; CS]; CS];
                    let mut lower_cells = Vec::new();
                    let mut lower_stack = Vec::new();
                    for &(x, z) in &comp {
                        if prev_mask[z][x] && !lower_visited[z][x] {
                            lower_visited[z][x] = true;
                            lower_stack.push((x, z));
                        }
                    }
                    while let Some((x, z)) = lower_stack.pop() {
                        lower_cells.push((x, z));
                        for (dx, dz) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                            let nx = x as isize + dx;
                            let nz = z as isize + dz;
                            if nx < 0 || nx >= CS as isize || nz < 0 || nz >= CS as isize {
                                continue;
                            }
                            let (nx, nz) = (nx as usize, nz as usize);
                            if prev_mask[nz][nx] && !lower_visited[nz][nx] {
                                lower_visited[nz][nx] = true;
                                lower_stack.push((nx, nz));
                            }
                        }
                    }
                    if !lower_cells.iter().all(|&(x, z)| in_comp[z][x]) {
                        continue;
                    }
                    for &(x, z) in &comp {
                        chunk.set_liquid(x, ly, z, Some(target));
                        next_height[z][x] = if prev_mask[z][x] {
                            pool_height[z][x] + 1
                        } else {
                            1
                        };
                        next_kind[z][x] = kind;
                    }
                } else {
                    // A brand-new floor basin fills only when the deterministic
                    // gate selects it.
                    let gated = comp.iter().any(|&(x, z)| {
                        chunk.get(x, ly - 1, z).is_solid()
                            && fbm3(
                                seed ^ SALT_POOL,
                                (bx + x as i32) as f64,
                                wy as f64,
                                (bz + z as i32) as f64,
                                1.0 / 18.0,
                                3,
                            ) > POOL_GATE
                    });
                    if !gated {
                        continue;
                    }
                    for &(x, z) in &comp {
                        chunk.set_liquid(x, ly, z, Some(target));
                        next_height[z][x] = 1;
                        next_kind[z][x] = kind;
                    }
                }
            }
        }
        pool_height = next_height;
        pool_kind = next_kind;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::SAND;
    use crate::worldgen::terrain::{generate_chunk_sized, surface_top_for_size};

    fn generate(seed: u32, x: i32, y: i32, z: i32, size: WorldSize) -> Chunk {
        generate_chunk_sized(seed, ChunkPos { x, y, z }, size)
    }

    fn is_water(chunk: &Chunk, lx: usize, ly: usize, lz: usize) -> bool {
        chunk
            .liquid(lx, ly, lz)
            .is_some_and(|l| l.kind == LiquidKind::Water)
    }

    #[test]
    fn sea_level_const_is_below_flat_land_surface() {
        let seed = 7;
        let size = WorldSize::Small;
        let bounds = WorldBounds::new(size);
        let inland = bounds.half - OCEAN_WIDTH - BEACH_WIDTH - 16;
        let mut total = 0usize;
        let mut above = 0usize;
        // 50 x 40 = 2000 inland columns spread across the world.
        for i in 0..50 {
            let x = -inland + (2 * inland * i) / 49;
            for j in 0..40 {
                let z = -inland + (2 * inland * j) / 39;
                total += 1;
                if surface_height_for_size(seed, x, z, size) > SEA_LEVEL_Y {
                    above += 1;
                }
            }
        }
        assert_eq!(total, 2000);
        assert!(
            above * 100 >= total * 95,
            "expected >= 95% of inland columns above SEA_LEVEL_Y ({SEA_LEVEL_Y}), got {above}/{total}"
        );
    }

    #[test]
    fn ocean_ring_filled_to_sea_level() {
        let seed = 7;
        let size = WorldSize::Small;
        let bounds = WorldBounds::new(size);
        let mut ocean_columns = 0usize;
        let mut water_cells = 0usize;
        for cx in -45..-39 {
            for cz in -2..=2 {
                for cy in 0..=2 {
                    let chunk = generate(seed, cx, cy, cz, size);
                    for lz in 0..CS {
                        let wz = cz * CHUNK_SIZE + lz as i32;
                        for lx in 0..CS {
                            let wx = cx * CHUNK_SIZE + lx as i32;
                            let ed = bounds.edge_distance(wx, wz);
                            if !(0..OCEAN_WIDTH).contains(&ed) {
                                continue;
                            }
                            if cy == 0 {
                                ocean_columns += 1;
                            }
                            for ly in 0..CS {
                                let wy = cy * CHUNK_SIZE + ly as i32;
                                let liq = chunk.liquid(lx, ly, lz);
                                if wy <= SEA_LEVEL_Y {
                                    if chunk.get(lx, ly, lz) == AIR {
                                        assert_eq!(
                                            liq,
                                            Some(Liquid::water(AMOUNT_FULL)),
                                            "expected water 255 in ocean AIR at ({wx},{wy},{wz}), ed={ed}"
                                        );
                                        water_cells += 1;
                                    }
                                } else {
                                    assert_eq!(
                                        liq, None,
                                        "expected no liquid above SEA_LEVEL_Y in ocean at ({wx},{wy},{wz})"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(ocean_columns > 100, "checked too few ocean columns");
        assert!(water_cells > 1000, "expected ocean water cells");
    }

    #[test]
    fn ocean_floor_is_sand() {
        let seed = 7;
        let size = WorldSize::Small;
        let bounds = WorldBounds::new(size);
        let half = bounds.half;
        let mut checked = 0usize;
        for d in (0..OCEAN_WIDTH).step_by(7) {
            for along in (-128..=128).step_by(32) {
                let columns = [
                    (-half + d, along),
                    (half - 1 - d, along),
                    (along, -half + d),
                    (along, half - 1 - d),
                ];
                for (wx, wz) in columns {
                    let (h, top) = surface_top_for_size(seed, wx, wz, size);
                    assert!(
                        h < SEA_LEVEL_Y,
                        "ocean floor should be below SEA_LEVEL_Y at ({wx},{wz}): h={h}"
                    );
                    assert_eq!(
                        top, SAND,
                        "top solid block in ocean ring at ({wx},{wz}) should be SAND, got {top:?}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 100);
    }

    #[test]
    fn no_liquid_within_spawn_radius() {
        let seed = 7;
        let size = WorldSize::Small;
        let mut checked_columns = 0usize;
        for cx in -2..=2 {
            for cz in -2..=2 {
                for cy in 0..4 {
                    let chunk = generate(seed, cx, cy, cz, size);
                    for lz in 0..CS {
                        let wz = cz * CHUNK_SIZE + lz as i32;
                        for lx in 0..CS {
                            let wx = cx * CHUNK_SIZE + lx as i32;
                            if wx * wx + wz * wz > SPAWN_GUARD_RADIUS_SQ {
                                continue;
                            }
                            if cy == 0 {
                                checked_columns += 1;
                            }
                            for ly in 0..CS {
                                let wy = cy * CHUNK_SIZE + ly as i32;
                                if wy <= LAVA_DEPTH_Y {
                                    continue;
                                }
                                assert_eq!(
                                    chunk.liquid(lx, ly, lz),
                                    None,
                                    "found liquid inside spawn guard at ({wx},{wy},{wz})"
                                );
                            }
                        }
                    }
                }
            }
        }
        assert!(checked_columns > 5000, "checked too few spawn columns");
    }

    #[test]
    fn ponds_only_in_forest_and_jungle() {
        let seed = 7;
        let size = WorldSize::Small;
        let mut pond_cells = 0usize;
        for cx in -4..4 {
            for cz in -4..4 {
                for cy in 0..4 {
                    let chunk = generate(seed, cx, cy, cz, size);
                    for lz in 0..CS {
                        let wz = cz * CHUNK_SIZE + lz as i32;
                        for lx in 0..CS {
                            let wx = cx * CHUNK_SIZE + lx as i32;
                            let surf = surface_height_for_size(seed, wx, wz, size);
                            for ly in 0..CS {
                                let wy = cy * CHUNK_SIZE + ly as i32;
                                if wy <= SEA_LEVEL_Y + 1 || wy <= surf {
                                    continue;
                                }
                                let Some(liq) = chunk.liquid(lx, ly, lz) else {
                                    continue;
                                };
                                if liq.kind != LiquidKind::Water {
                                    continue;
                                }
                                pond_cells += 1;
                                let w = biome_weights_for_size(seed, wx, wz, size);
                                assert!(
                                    matches!(dominant(&w), Biome::Forest | Biome::Jungle),
                                    "surface pond at ({wx},{wy},{wz}) in {:?}",
                                    dominant(&w)
                                );
                                assert_eq!(
                                    corruption_at_for_size(seed, wx, wz, size),
                                    0.0,
                                    "surface pond at ({wx},{wy},{wz}) in corrupt column"
                                );
                            }
                        }
                    }
                }
            }
        }
        assert!(pond_cells > 0, "expected at least one surface pond");
    }

    #[test]
    fn generated_pond_cells_are_bounded_by_solid_or_water() {
        let seed = 7;
        let size = WorldSize::Small;
        let mut checked = 0usize;
        for cx in -4..4 {
            for cz in -4..4 {
                for cy in 0..4 {
                    let chunk = generate(seed, cx, cy, cz, size);
                    for lz in 0..CS {
                        let wz = cz * CHUNK_SIZE + lz as i32;
                        for lx in 0..CS {
                            let wx = cx * CHUNK_SIZE + lx as i32;
                            let surf = surface_height_for_size(seed, wx, wz, size);
                            for ly in 0..CS {
                                let wy = cy * CHUNK_SIZE + ly as i32;
                                // Surface ponds sit above the terrain surface.
                                // This also skips ocean water (<= sea level) and
                                // underground pools (below the surface).
                                if wy <= SEA_LEVEL_Y + 1 || wy <= surf {
                                    continue;
                                }
                                let Some(liq) = chunk.liquid(lx, ly, lz) else {
                                    continue;
                                };
                                if liq.kind != LiquidKind::Water {
                                    continue;
                                }
                                checked += 1;
                                for (dx, dz) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
                                    let nx = (lx as isize + dx) as usize;
                                    let nz = (lz as isize + dz) as usize;
                                    assert!(
                                        chunk.get(nx, ly, nz).is_solid()
                                            || is_water(&chunk, nx, ly, nz),
                                        "pond cell ({wx},{wy},{wz}) has unbounded side ({nx},{nz})"
                                    );
                                }
                                assert!(ly > 0, "pond cell at chunk floor");
                                assert!(
                                    chunk.get(lx, ly - 1, lz).is_solid()
                                        || is_water(&chunk, lx, ly - 1, lz),
                                    "pond cell ({wx},{wy},{wz}) has unsupported underside"
                                );
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 0, "expected to inspect generated pond cells");
    }

    #[test]
    fn underground_pool_top_is_flat() {
        let seed = 7;
        let size = WorldSize::Small;
        let mut pools = 0usize;
        let mut multi_column_pools = 0usize;
        for cx in -3..3 {
            for cz in -3..3 {
                for cy in 0..3 {
                    let chunk = generate(seed, cx, cy, cz, size);
                    let mut visited = [[[false; CS]; CS]; CS];
                    for lz in 0..CS {
                        for ly in 0..CS {
                            for lx in 0..CS {
                                if visited[lz][ly][lx] {
                                    continue;
                                }
                                let Some(liq) = chunk.liquid(lx, ly, lz) else {
                                    continue;
                                };
                                // Flood the 6-connected same-kind component.
                                let mut stack = vec![(lx, ly, lz)];
                                visited[lz][ly][lx] = true;
                                let mut cells = Vec::new();
                                while let Some((x, y, z)) = stack.pop() {
                                    cells.push((x, y, z));
                                    for (dx, dy, dz) in [
                                        (1isize, 0isize, 0isize),
                                        (-1, 0, 0),
                                        (0, 1, 0),
                                        (0, -1, 0),
                                        (0, 0, 1),
                                        (0, 0, -1),
                                    ] {
                                        let nx = x as isize + dx;
                                        let ny = y as isize + dy;
                                        let nz = z as isize + dz;
                                        if (0..CS as isize).contains(&nx)
                                            && (0..CS as isize).contains(&ny)
                                            && (0..CS as isize).contains(&nz)
                                        {
                                            let (ux, uy, uz) =
                                                (nx as usize, ny as usize, nz as usize);
                                            if !visited[uz][uy][ux]
                                                && chunk
                                                    .liquid(ux, uy, uz)
                                                    .is_some_and(|l| l.kind == liq.kind)
                                            {
                                                visited[uz][uy][ux] = true;
                                                stack.push((ux, uy, uz));
                                            }
                                        }
                                    }
                                }
                                // Cave pools lie at or below the terrain surface
                                // everywhere in the component.
                                let underground = cells.iter().all(|&(x, y, z)| {
                                    let wx = cx * CHUNK_SIZE + x as i32;
                                    let wy = cy * CHUNK_SIZE + y as i32;
                                    let wz = cz * CHUNK_SIZE + z as i32;
                                    wy <= surface_height_for_size(seed, wx, wz, size)
                                });
                                if !underground {
                                    continue;
                                }
                                pools += 1;
                                let mut column_tops = std::collections::HashMap::new();
                                for &(x, y, z) in &cells {
                                    let top = column_tops.entry((x, z)).or_insert(y);
                                    *top = (*top).max(y);
                                }
                                let first = *column_tops.values().next().unwrap();
                                assert!(
                                    column_tops.values().all(|&top| top == first),
                                    "pool in chunk ({cx},{cy},{cz}) has uneven top: {column_tops:?}"
                                );
                                if column_tops.len() > 1 {
                                    multi_column_pools += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(pools > 0, "expected at least one underground pool");
        assert!(
            multi_column_pools > 0,
            "expected at least one multi-column underground pool"
        );
    }

    #[test]
    fn lava_pools_only_below_lava_depth() {
        let seed = 7;
        let size = WorldSize::Small;
        let mut lava_cells = 0usize;
        for cx in -6..6 {
            for cz in -6..6 {
                for cy in 0..4 {
                    let chunk = generate(seed, cx, cy, cz, size);
                    for lz in 0..CS {
                        for ly in 0..CS {
                            let wy = cy * CHUNK_SIZE + ly as i32;
                            for lx in 0..CS {
                                if let Some(liq) = chunk.liquid(lx, ly, lz) {
                                    if liq.kind == LiquidKind::Lava {
                                        assert!(
                                            wy <= LAVA_DEPTH_Y,
                                            "lava at wy={wy} above LAVA_DEPTH_Y ({LAVA_DEPTH_Y})"
                                        );
                                        lava_cells += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(
            lava_cells > 0,
            "expected at least one lava cell in 12x12 chunks"
        );
    }

    #[test]
    fn generation_is_deterministic() {
        let pos = ChunkPos { x: 3, y: 0, z: -2 };
        let a = generate_chunk_sized(7, pos, WorldSize::Small);
        let b = generate_chunk_sized(7, pos, WorldSize::Small);
        assert_eq!(a.to_bytes(), b.to_bytes());

        let mut differs = false;
        for cx in -3..3 {
            for cy in 0..3 {
                let p = ChunkPos { x: cx, y: cy, z: 2 };
                if generate_chunk_sized(7, p, WorldSize::Small).to_bytes()
                    != generate_chunk_sized(8, p, WorldSize::Small).to_bytes()
                {
                    differs = true;
                    break;
                }
            }
        }
        assert!(differs, "different seeds must produce different chunks");
    }

    #[test]
    fn liquid_never_replaces_non_air() {
        let seed = 7;
        let size = WorldSize::Small;
        for (cx, cy, cz) in [(0, 0, 0), (2, 0, 2), (-45, 0, 0), (4, 1, 4)] {
            let pos = ChunkPos {
                x: cx,
                y: cy,
                z: cz,
            };
            let with_liquid = generate_chunk_sized(seed, pos, size);
            let mut terrain_only = with_liquid.clone();
            for lz in 0..CS {
                for ly in 0..CS {
                    for lx in 0..CS {
                        if with_liquid.liquid(lx, ly, lz).is_some() {
                            assert_eq!(
                                with_liquid.get(lx, ly, lz),
                                AIR,
                                "liquid occupies a non-AIR block at ({lx},{ly},{lz})"
                            );
                            terrain_only.set_liquid(lx, ly, lz, None);
                        }
                    }
                }
            }
            let mut reapplied = terrain_only.clone();
            apply(seed, pos, size, &mut reapplied);
            for lz in 0..CS {
                for ly in 0..CS {
                    for lx in 0..CS {
                        assert_eq!(
                            terrain_only.get(lx, ly, lz),
                            reapplied.get(lx, ly, lz),
                            "liquid::apply modified block at ({lx},{ly},{lz})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn liquid_amount_ratio_in_world_sane() {
        let seed = 7;
        let size = WorldSize::Small;
        let mut cave_air = 0usize;
        let mut liquid_cells = 0usize;
        // 4x4-chunk inland sample below the surface.
        for cx in 2..6 {
            for cz in 2..6 {
                for cy in 0..3 {
                    let chunk = generate(seed, cx, cy, cz, size);
                    for lz in 0..CS {
                        let wz = cz * CHUNK_SIZE + lz as i32;
                        for lx in 0..CS {
                            let wx = cx * CHUNK_SIZE + lx as i32;
                            let surf = surface_height_for_size(seed, wx, wz, size);
                            for ly in 0..CS {
                                let wy = cy * CHUNK_SIZE + ly as i32;
                                if wy <= LAVA_DEPTH_Y {
                                    continue;
                                }
                                if wy < surf && chunk.get(lx, ly, lz) == AIR {
                                    cave_air += 1;
                                    if chunk.liquid(lx, ly, lz).is_some() {
                                        liquid_cells += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(cave_air > 0, "expected cave air in 4x4 inland sample");
        let ratio = liquid_cells as f64 / cave_air as f64;
        assert!(
            (0.003..=0.03).contains(&ratio),
            "expected liquid/cave_air ratio 0.3%..=3%, got {liquid_cells}/{cave_air} ({:.2}%)",
            ratio * 100.0
        );
    }
}
