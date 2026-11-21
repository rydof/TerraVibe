//! Biome terrain: per-column surface height blended from biome parameters,
//! dithered surface layers, caves and ores.
//!
//! Uses only IEEE-exact f64 ops (+, -, *, /, floor) so output is bit-identical
//! across platforms.

use super::biome::{Biome, BiomeWeights, biome_weights_for_size, dominant};
use super::noise::{fbm2, fbm3, hash, value3};
use super::tree;
use crate::block::*;
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, SEA_LEVEL_Y, WorldBounds, WorldSize};

/// Blocks exist only for world y in `0..WORLD_HEIGHT`.
pub(crate) const WORLD_HEIGHT: i64 = 128;
const MIN_SURFACE: f64 = 8.0;
const MAX_SURFACE: f64 = 120.0;

/// Depth bands corresponding to classic Terraria world layers:
/// - `Surface`: top surface down to depth 24
/// - `Underground`: depth 25 to 69 below surface
/// - `Cavern`: depth 70+ below surface
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DepthBand {
    Surface,
    Underground,
    Cavern,
    Underworld,
}

/// Classifies a column height and vertical position into a `DepthBand`.
pub fn depth_band(surface_y: i32, y: i32) -> DepthBand {
    if y <= super::underworld::UNDERWORLD_TOP_Y {
        DepthBand::Underworld
    } else if y > surface_y - 25 {
        DepthBand::Surface
    } else if y > surface_y - 70 {
        DepthBand::Underground
    } else {
        DepthBand::Cavern
    }
}

// Per-feature seed salts.
const SALT_HEIGHT: u32 = 0x68E1_9A3D;
const SALT_ROUGH: u32 = 0x2D4B_F1A7;
const SALT_CAVE: u32 = 0xC0FF_EE11;
const SALT_CAVE_MOUTH: u32 = 0x4341_5645;
const SALT_CAVE_MOUTH_DIR: u32 = 0x4449_5254;
const CAVE_MOUTH_CELL: i32 = 48;
const SALT_ICE: u32 = 0x4E1C_7D93;
const SALT_LAYER: u32 = 0x9A07_3B5F;
/// Shallow Forest/Jungle basins that the liquid pass fills as surface ponds.
const SALT_POND: u32 = 0x504F_4E44;
/// One potential pond centre per `POND_CELL`-block grid cell.
const POND_CELL: i32 = 24;
/// Hash gate: roughly one in four grid cells gets a basin.
const POND_CHANCE: u32 = 64;
/// Small enclosed underground rooms that the liquid pass fills as cave pools.
const SALT_POCKET: u32 = 0x504F_434B;
const POCKET_CELL: i32 = 16;
const POCKET_HEIGHT: i32 = 4;
const POCKET_WIDTH: i32 = 4;
/// Distance from a room to its grid cell edge; keeps every room interior at
/// least four blocks inside one 32-block chunk.
const POCKET_MARGIN: i32 = 4;
/// Rooms are only carved at least this many blocks below the column surface.
const POCKET_MIN_DEPTH: i64 = 12;
/// Water rooms sit just above `LAVA_DEPTH_Y` (24); lava rooms below it.
const POCKET_WATER_Y0: i32 = 22;
const POCKET_WATER_SPAN: u32 = 6;
const POCKET_LAVA_Y0: i32 = 4;
const POCKET_LAVA_SPAN: u32 = 16;

const CAVE_THRESHOLD: f64 = 0.08;
const UNDERGROUND_CAVE_THRESHOLD: f64 = 0.11;
const CAVERN_CAVE_THRESHOLD: f64 = 0.14;
/// Extra cave width at full jungle weight: 1.5x as many caves.
const JUNGLE_CAVE_EASE: f64 = 0.5;

/// Per-biome height parameters, blended linearly by biome weight.
struct Params {
    base: f64,
    amp: f64,
    freq: f64,
}

fn params(b: Biome) -> Params {
    match b {
        Biome::Forest => Params {
            base: 64.0,
            amp: 20.0,
            freq: 1.0 / 96.0,
        },
        // Low dunes: base −4, amp 0.5×.
        Biome::Desert => Params {
            base: 60.0,
            amp: 10.0,
            freq: 1.0 / 80.0,
        },
        // Snow: base +10, amp 1.6×.
        Biome::Snow => Params {
            base: 74.0,
            amp: 32.0,
            freq: 1.0 / 96.0,
        },
        Biome::Jungle => Params {
            base: 64.0,
            amp: 26.0,
            freq: 1.0 / 88.0,
        },
    }
}

const BIOMES: [Biome; 4] = [Biome::Forest, Biome::Desert, Biome::Snow, Biome::Jungle];

/// Everything the block rules need about one world column.
struct Column {
    height: i64,
    weights: BiomeWeights,
    /// Biome whose layers this column uses (dithered by weight).
    layer: Biome,
    /// Distance in blocks to the nearest world edge.
    edge_dist: i32,
    size: WorldSize,
}

/// Depth of a shallow pond basin at `(x, z)`, in blocks.
///
/// One deterministic candidate centre per `POND_CELL` grid cell. A selected
/// centre is three blocks low with its four cardinal neighbours one block low,
/// giving the `liquid` pass a two-block depression whose radius-3 rim is
/// untouched. The shape is continuous (adjacent columns differ by at most two).
fn pond_depression(seed: u32, x: i32, z: i32) -> i64 {
    let gx = x.div_euclid(POND_CELL);
    let gz = z.div_euclid(POND_CELL);
    let span = (POND_CELL - 8) as u32;
    let mut depth = 0;
    for dx in -1..=1 {
        for dz in -1..=1 {
            let h = hash(seed ^ SALT_POND, gx + dx, 0, gz + dz);
            if (h & 0xFF) >= POND_CHANCE {
                continue;
            }
            let ox = 4 + ((h >> 8) % span) as i32;
            let oz = 4 + ((h >> 16) % span) as i32;
            let cx = (gx + dx) * POND_CELL + ox;
            let cz = (gz + dz) * POND_CELL + oz;
            depth = depth.max(match (x - cx).abs() + (z - cz).abs() {
                0 => 3,
                1 => 1,
                _ => 0,
            });
        }
    }
    depth
}

/// Bounds of the enclosed underground room in this column's `POCKET_CELL`
/// grid cell: `(x0, z0, y0, y1)`. Rooms are independent of surface height so
/// every column of a room agrees on its vertical extent.
///
/// A room is carved only when the current column is deep enough; a column that
/// is too shallow leaves solid rock in place, which can only split the room
/// into smaller still-enclosed parts.
fn pocket_room(seed: u32, x: i32, z: i32) -> Option<(i32, i32, i32, i32)> {
    let gx = x.div_euclid(POCKET_CELL);
    let gz = z.div_euclid(POCKET_CELL);
    let h = hash(seed ^ SALT_POCKET, gx, 0, gz);
    let span = (POCKET_CELL - 2 * POCKET_MARGIN - (POCKET_WIDTH - 1)) as u32;
    let ox = gx * POCKET_CELL + POCKET_MARGIN + ((h >> 8) % span) as i32;
    let oz = gz * POCKET_CELL + POCKET_MARGIN + ((h >> 16) % span) as i32;
    let (y0, span) = if (h >> 24) & 3 == 0 {
        (POCKET_LAVA_Y0, POCKET_LAVA_SPAN)
    } else {
        (POCKET_WATER_Y0, POCKET_WATER_SPAN)
    };
    let y0 = y0 + ((h >> 20) % span) as i32;
    Some((ox, oz, y0, y0 + POCKET_HEIGHT - 1))
}

fn column(seed: u32, x: i32, z: i32) -> Column {
    column_for_size(seed, x, z, WorldSize::Small)
}

fn column_for_size(seed: u32, x: i32, z: i32, size: WorldSize) -> Column {
    let w = biome_weights_for_size(seed, x, z, size).0;
    let (mut base, mut amp, mut freq) = (0.0, 0.0, 0.0);
    for b in BIOMES {
        let wi = w[b as usize] as f64;
        let p = params(b);
        base += wi * p.base;
        amp += wi * p.amp;
        freq += wi * p.freq;
    }
    let (xf, zf) = (x as f64, z as f64);
    let n = fbm2(seed ^ SALT_HEIGHT, xf, zf, freq, 4);
    // Jungle's rougher octave.
    let rough = fbm2(seed ^ SALT_ROUGH, xf, zf, freq * 4.0, 1);
    let jungle = w[Biome::Jungle as usize] as f64;
    let n = n + jungle * 0.3 * rough;
    let mut inland_h = (base + amp * n).floor().clamp(MIN_SURFACE, MAX_SURFACE);

    let edge_dist = WorldBounds::new(size).edge_distance(x, z);
    // Shallow Forest/Jungle basins give the liquid pass a depression to fill.
    // Keep the coast untouched so the ocean and beach shaping stays exact.
    if edge_dist >= OCEAN_WIDTH + BEACH_WIDTH
        && matches!(dominant(&BiomeWeights(w)), Biome::Forest | Biome::Jungle)
    {
        inland_h = (inland_h - pond_depression(seed, x, z) as f64).clamp(MIN_SURFACE, MAX_SURFACE);
    }
    let sea = f64::from(SEA_LEVEL_Y);
    let height = if edge_dist < OCEAN_WIDTH {
        // Ocean floor: slopes from sea - 1 at the shoreline down to sea - 24 at the border.
        let t = (edge_dist.max(0) as f64) / (OCEAN_WIDTH as f64);
        let smooth = t * t * (3.0 - 2.0 * t);
        let floor_h = (sea - 24.0) + smooth * 23.0;
        (floor_h.floor() as i64).clamp(MIN_SURFACE as i64, SEA_LEVEL_Y as i64 - 1)
    } else if edge_dist < OCEAN_WIDTH + BEACH_WIDTH {
        // Beach: sandy ramp from sea level up to inland terrain.
        let u = ((edge_dist - OCEAN_WIDTH) as f64 + 1.0) / (BEACH_WIDTH as f64);
        let smooth = u * u * (3.0 - 2.0 * u);
        let target = inland_h.max(sea + 6.0);
        let beach_h = sea + smooth * (target - sea);
        (beach_h.floor() as i64).clamp(SEA_LEVEL_Y as i64, MAX_SURFACE as i64)
    } else {
        inland_h as i64
    };

    // Dithered pick: one hash per column, compared against the cumulative weights.
    let u = hash(seed ^ SALT_LAYER, x, 0, z) as f64 / 4_294_967_296.0;
    let mut acc = 0.0;
    let mut layer = Biome::Forest;
    for b in BIOMES {
        acc += w[b as usize] as f64;
        if u < acc {
            layer = b;
            break;
        }
    }
    Column {
        height,
        weights: BiomeWeights(w),
        layer,
        edge_dist,
        size,
    }
}

pub(crate) fn inland_surface_height(seed: u32, x: i32, z: i32, size: WorldSize) -> i32 {
    let w = biome_weights_for_size(seed, x, z, size).0;
    let (mut base, mut amp, mut freq) = (0.0, 0.0, 0.0);
    for b in BIOMES {
        let wi = w[b as usize] as f64;
        let p = params(b);
        base += wi * p.base;
        amp += wi * p.amp;
        freq += wi * p.freq;
    }
    let (xf, zf) = (x as f64, z as f64);
    let n = fbm2(seed ^ SALT_HEIGHT, xf, zf, freq, 4);
    let rough = fbm2(seed ^ SALT_ROUGH, xf, zf, freq * 4.0, 1);
    let jungle = w[Biome::Jungle as usize] as f64;
    let n = n + jungle * 0.3 * rough;
    let mut h = (base + amp * n).floor().clamp(MIN_SURFACE, MAX_SURFACE);
    if matches!(dominant(&BiomeWeights(w)), Biome::Forest | Biome::Jungle) {
        h = (h - pond_depression(seed, x, z) as f64).clamp(MIN_SURFACE, MAX_SURFACE);
    }
    h as i32
}

/// Surface height (y of the top solid block) for a world column.
pub fn surface_height(seed: u32, x: i32, z: i32) -> i32 {
    column(seed, x, z).height as i32
}

pub fn surface_height_for_size(seed: u32, x: i32, z: i32, size: WorldSize) -> i32 {
    column_for_size(seed, x, z, size).height as i32
}

/// Surface height and the block sitting at that height.
#[cfg(test)]
pub(crate) fn surface_top(seed: u32, x: i32, z: i32) -> (i32, BlockId) {
    surface_top_for_size(seed, x, z, WorldSize::Small)
}

pub fn surface_top_for_size(seed: u32, x: i32, z: i32, size: WorldSize) -> (i32, BlockId) {
    let c = column_for_size(seed, x, z, size);
    (
        c.height as i32,
        block_at(seed, x as i64, c.height, z as i64, &c),
    )
}

/// The natural (pre-ore, pre-tree) block at a world position; pure in `(seed, x, y, z)`.
#[cfg(test)]
pub(crate) fn natural_block(seed: u32, x: i32, y: i32, z: i32) -> BlockId {
    natural_block_for_size(seed, x, y, z, WorldSize::Small)
}

pub(crate) fn natural_block_for_size(
    seed: u32,
    x: i32,
    y: i32,
    z: i32,
    size: WorldSize,
) -> BlockId {
    let c = column_for_size(seed, x, z, size);
    block_at(seed, x as i64, y as i64, z as i64, &c)
}

/// Pure deterministic surface cave entrance carver.
///
/// Checks `CAVE_MOUTH_CELL`-sized grid cells in `-1..=1` around `(x, z)` and
/// carves a slanted, gently meandering 3D tunnel from slightly above the surface
/// down 28 blocks into the underground cave network.
pub fn in_surface_cave_mouth(seed: u32, x: i32, y: i32, z: i32, size: WorldSize) -> bool {
    let gx = x.div_euclid(CAVE_MOUTH_CELL);
    let gz = z.div_euclid(CAVE_MOUTH_CELL);
    let span = (CAVE_MOUTH_CELL - 16) as u32;
    let bounds = WorldBounds::new(size);
    let mut de_cached: Option<[f32; 3]> = None;

    for dx in -1..=1 {
        for dz in -1..=1 {
            let cell_x = gx + dx;
            let cell_z = gz + dz;
            let h = hash(seed ^ SALT_CAVE_MOUTH, cell_x, 0, cell_z);
            if (h & 0xFF) >= 115 {
                continue;
            }
            let ox = 8 + ((h >> 8) % span) as i32;
            let oz = 8 + ((h >> 16) % span) as i32;
            let cx = cell_x * CAVE_MOUTH_CELL + ox;
            let cz = cell_z * CAVE_MOUTH_CELL + oz;

            if (x - cx).abs() > 32 || (z - cz).abs() > 32 {
                continue;
            }
            if cx.abs() <= 20 && cz.abs() <= 20 {
                continue;
            }
            if bounds.edge_distance(cx, cz) < OCEAN_WIDTH + BEACH_WIDTH + 8 {
                continue;
            }

            let h2 = hash(seed ^ SALT_CAVE_MOUTH_DIR, cell_x, 0, cell_z);
            let theta = ((h2 & 0xFFFF) as f64 / 65536.0) * std::f64::consts::TAU;
            let slope = 0.55 + (((h2 >> 16) & 0xFF) as f64 / 255.0) * 0.35;
            let (sin_t, cos_t) = theta.sin_cos();
            let rel_x = (x - cx) as f64;
            let rel_z = (z - cz) as f64;
            let u = rel_x * cos_t + rel_z * sin_t;
            let v = -rel_x * sin_t + rel_z * cos_t;
            if v.abs() > 4.7 || u < -2.0 * slope - 3.3 || u > 28.0 * slope + 3.3 {
                continue;
            }

            let de =
                *de_cached.get_or_insert_with(|| super::dungeon::dungeon_entrance_pos(seed, size));
            if (cx - de[0] as i32).abs() < 18 && (cz - de[2] as i32).abs() < 18 {
                continue;
            }

            let sy = surface_height_for_size(seed, cx, cz, size);
            if y > sy + 6 || y < sy - 32 {
                continue;
            }

            let phase = (((h2 >> 24) & 0xFF) as f64 / 255.0) * std::f64::consts::TAU;
            let dx_dir = cos_t * slope;
            let dz_dir = sin_t * slope;
            let px_dir = -sin_t;
            let pz_dir = cos_t;

            let t_min = (sy - y - 4).max(-2);
            let t_max = (sy - y + 4).min(28);
            for ti in t_min..=t_max {
                let t = ti as f64;
                let ty = sy as f64 - t;
                let meander = (t * 0.35 + phase).sin() * 1.4;
                let tx = cx as f64 + dx_dir * t + px_dir * meander;
                let tz = cz as f64 + dz_dir * t + pz_dir * meander;
                let r = 2.4 + 0.8 * ((t * 0.25 + phase).cos() * 0.5 + 0.5);
                let ddx = x as f64 - tx;
                let ddy = y as f64 - ty;
                let ddz = z as f64 - tz;
                if ddx * ddx + ddy * ddy + ddz * ddz <= r * r {
                    return true;
                }
            }
        }
    }
    false
}

fn block_at(seed: u32, x: i64, y: i64, z: i64, c: &Column) -> BlockId {
    let height = c.height;
    if y > height {
        return AIR;
    }
    let depth = height - y;
    let in_coastal_ring = c.edge_dist < OCEAN_WIDTH + BEACH_WIDTH;
    if in_coastal_ring {
        if depth < 6 {
            return SAND;
        }
        if depth < 10 {
            return HARDENED_SAND;
        }
    }
    let (fx, fy, fz) = (x as f64, y as f64, z as f64);
    // Enclosed rooms for the liquid pass: carve the interior, but never let a
    // cave open the shell. A room column that is too shallow stays solid.
    let room_top = POCKET_WATER_Y0 + POCKET_WATER_SPAN as i32 - 1 + POCKET_HEIGHT;
    let pocket = if in_coastal_ring || !(POCKET_LAVA_Y0 - 1..=room_top).contains(&(y as i32)) {
        None
    } else {
        pocket_room(seed, x as i32, z as i32)
    };
    let mut protected = false;
    if let Some((ox, oz, y0, y1)) = pocket {
        let (xi, yi, zi) = (x as i32, y as i32, z as i32);
        let interior = (ox..ox + POCKET_WIDTH).contains(&xi)
            && (oz..oz + POCKET_WIDTH).contains(&zi)
            && (y0..=y1).contains(&yi);
        if interior {
            if depth >= POCKET_MIN_DEPTH {
                return AIR;
            }
            protected = true;
        } else {
            protected = (ox - 1..=ox + POCKET_WIDTH).contains(&xi)
                && (oz - 1..=oz + POCKET_WIDTH).contains(&zi)
                && (y0 - 1..=y1 + 1).contains(&yi);
        }
    }
    // Caves: carve below the top few layers so the surface stays intact (never carve ocean floor).
    // Cave network density increases with depth across Underground and Cavern layers.
    let jungle = c.weights.0[Biome::Jungle as usize] as f64;
    let band = depth_band(height as i32, y as i32);
    let band_threshold = match band {
        DepthBand::Surface => CAVE_THRESHOLD,
        DepthBand::Underground => UNDERGROUND_CAVE_THRESHOLD,
        DepthBand::Cavern | DepthBand::Underworld => CAVERN_CAVE_THRESHOLD,
    };
    let threshold = band_threshold * (1.0 + JUNGLE_CAVE_EASE * jungle);
    if !in_coastal_ring
        && !protected
        && depth <= 38
        && in_surface_cave_mouth(seed, x as i32, y as i32, z as i32, c.size)
    {
        return AIR;
    }
    if !in_coastal_ring
        && !protected
        && depth >= 3
        && fbm3(seed ^ SALT_CAVE, fx, fy, fz, 1.0 / 24.0, 2).abs() < threshold
    {
        return AIR;
    }
    // Surface layers, picked by the column's dithered biome.
    match c.layer {
        Biome::Forest if depth == 0 => return GRASS,
        Biome::Forest if depth <= 4 => return DIRT,
        Biome::Desert if depth < 6 => return SAND,
        Biome::Desert if depth < 10 => return HARDENED_SAND,
        Biome::Desert => return SANDSTONE,
        Biome::Snow if depth < 5 => return SNOW,
        Biome::Jungle if depth == 0 => return JUNGLE_GRASS,
        Biome::Jungle if depth <= 8 => return MUD,
        _ => {}
    }
    if c.layer == Biome::Snow && value3(seed ^ SALT_ICE, fx / 6.0, fy / 6.0, fz / 6.0) > 0.6 {
        return ICE;
    }
    STONE
}

/// Generate one chunk of biome terrain. Deterministic in `(seed, pos)`.
pub fn generate_chunk(seed: u32, pos: ChunkPos) -> Chunk {
    generate_chunk_sized(seed, pos, WorldSize::Small)
}

/// Generate one chunk of biome terrain for a world of `size`.
pub fn generate_chunk_sized(seed: u32, pos: ChunkPos, size: WorldSize) -> Chunk {
    let cs = CHUNK_SIZE as i64;
    let (bx, by, bz) = (pos.x as i64 * cs, pos.y as i64 * cs, pos.z as i64 * cs);
    let mut chunk = Chunk::filled(AIR);
    if by + cs <= 0 || by >= WORLD_HEIGHT {
        return chunk;
    }
    for lz in 0..CHUNK_SIZE as usize {
        for lx in 0..CHUNK_SIZE as usize {
            let (x, z) = ((bx + lx as i64) as i32, (bz + lz as i64) as i32);
            let c = column_for_size(seed, x, z, size);
            for ly in 0..CHUNK_SIZE as usize {
                let y = by + ly as i64;
                if !(0..WORLD_HEIGHT).contains(&y) {
                    continue;
                }
                chunk.set(lx, ly, lz, block_at(seed, x as i64, y, z as i64, &c));
            }
        }
    }
    // Corruption first: it converts stone to ebonstone, which the ore pass's
    // demonite veins can host. Ore veins then replace stone/dirt before trees
    // so a trunk base is never ore.
    super::corruption::apply_for_size(seed, pos, size, &mut chunk);
    super::ore::apply_for_size(seed, pos, size, &mut chunk);
    super::crystal::apply_for_size(seed, pos, size, &mut chunk);
    super::underworld::apply_for_size(seed, pos, size, &mut chunk);
    super::chest_gen::apply_for_size(seed, pos, size, &mut chunk);
    tree::decorate_for_size(seed, pos, size, &mut chunk);
    super::mushroom::apply_for_size(seed, pos, size, &mut chunk);
    super::liquid::apply(seed, pos, size, &mut chunk);
    super::dressing::apply_for_size(seed, pos, size, &mut chunk);
    apply_cabins_for_size(seed, pos, size, &mut chunk);
    super::dungeon::apply_for_size(seed, pos, size, &mut chunk);
    chunk
}

/// Underground wooden cabins: rectangular rooms embedded in cave walls in Underground and Cavern layers.
pub fn apply_cabins(seed: u32, pos: ChunkPos, chunk: &mut Chunk) {
    apply_cabins_for_size(seed, pos, WorldSize::Small, chunk);
}

pub fn apply_cabins_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let cs = CHUNK_SIZE;
    let (bx, by, bz) = (pos.x * cs, pos.y * cs, pos.z * cs);
    if by + cs <= 0 || by as i64 >= WORLD_HEIGHT {
        return;
    }

    // A cabin extends at most 4 blocks from its chest position.
    // Check chests in this chunk and immediate neighbors.
    for dz in -1..=1 {
        for dy in -1..=1 {
            let ny = by + dy * cs;
            if ny + cs <= 0 || ny as i64 >= WORLD_HEIGHT {
                continue;
            }
            for dx in -1..=1 {
                let npos = ChunkPos {
                    x: pos.x + dx,
                    y: pos.y + dy,
                    z: pos.z + dz,
                };
                for (p, _) in super::chest_gen::chests_in_for_size(seed, npos, size) {
                    let surf = surface_height_for_size(seed, p[0], p[2], size);
                    // Generate cabins only in Underground and Cavern layers
                    if surf - p[1] < super::chest_gen::SURFACE_BAND_DEPTH
                        || super::chest_gen::is_underworld_chest(seed, p, size)
                        || p[1] <= super::underworld::UNDERWORLD_TOP_Y
                    {
                        continue;
                    }

                    // Cabin dimensions: inner 6x4x4, outer 8x6x6
                    let x_min = p[0] - 2;
                    let x_max = p[0] + 3;
                    let y_min = p[1] - 1;
                    let y_max = p[1] + 3;
                    let z_min = p[2] - 2;
                    let z_max = p[2] + 2;

                    // Quick bounding box check with this chunk
                    if x_max < bx
                        || x_min >= bx + cs
                        || y_max < by
                        || y_min >= by + cs
                        || z_max < bz
                        || z_min >= bz + cs
                    {
                        continue;
                    }

                    for wy in y_min..=y_max {
                        if wy < by || wy >= by + cs {
                            continue;
                        }
                        let ly = (wy - by) as usize;
                        for wz in z_min..=z_max {
                            if wz < bz || wz >= bz + cs {
                                continue;
                            }
                            let lz = (wz - bz) as usize;
                            for wx in x_min..=x_max {
                                if wx < bx || wx >= bx + cs {
                                    continue;
                                }
                                let lx = (wx - bx) as usize;

                                let is_border = wx == x_min
                                    || wx == x_max
                                    || wy == y_min
                                    || wy == y_max
                                    || wz == z_min
                                    || wz == z_max;

                                if is_border {
                                    // Doorway opening on one side
                                    if wx == x_min && wz == p[2] && (wy == p[1] || wy == p[1] + 1) {
                                        chunk.set(lx, ly, lz, AIR);
                                    } else {
                                        chunk.set(lx, ly, lz, WOOD_BLOCK);
                                    }
                                } else {
                                    // Room interior
                                    chunk.set(lx, ly, lz, AIR);
                                    chunk.set_wall(lx, ly, lz, true);
                                    chunk.set_liquid(lx, ly, lz, None);

                                    // Furniture on the floor
                                    if wy == p[1] {
                                        if wx == p[0] && wz == p[2] {
                                            chunk.set(lx, ly, lz, CHEST);
                                        } else if wx == p[0] + 1 && wz == p[2] {
                                            chunk.set(lx, ly, lz, TABLE);
                                        } else if wx == p[0] + 2 && wz == p[2] {
                                            chunk.set(lx, ly, lz, CHAIR);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::item_data;
    use crate::worldgen::biome::biome_weights;
    use std::collections::HashSet;
    fn top_block(seed: u32, x: i32, z: i32) -> BlockId {
        let c = column(seed, x, z);
        block_at(seed, x as i64, c.height, z as i64, &c)
    }

    const CS: usize = CHUNK_SIZE as usize;

    fn chunk_at(seed: u32, x: i32, y: i32, z: i32) -> Chunk {
        generate_chunk(seed, ChunkPos { x, y, z })
    }

    #[test]
    fn heights_continuous_across_chunks() {
        let seed = 1;
        for z in 0..32 {
            let (ha, hb) = (surface_height(seed, 31, z), surface_height(seed, 32, z));
            assert!((ha - hb).abs() <= 2, "x 31/32 z {z}: {ha} vs {hb}");
        }
        // Also along a wider sweep of chunk borders, including biome borders.
        for cx in -12..12 {
            for z in 0..32 {
                let x = cx * 32;
                let (ha, hb) = (surface_height(seed, x - 1, z), surface_height(seed, x, z));
                assert!((ha - hb).abs() <= 2, "chunk x {cx} z {z}: {ha} vs {hb}");
            }
        }
    }

    #[test]
    fn surface_in_bounds() {
        for seed in [1u32, 42] {
            for x in -500..500 {
                for z in -500..500 {
                    let h = surface_height(seed, x, z);
                    assert!((8..=120).contains(&h), "seed {seed} ({x},{z}) h {h}");
                }
            }
        }
    }

    #[test]
    fn desert_surface_is_sand() {
        let seed = 1;
        let mut found = None;
        'search: for x in (-600..600).step_by(3) {
            for z in (-600..600).step_by(3) {
                if biome_weights(seed, x, z).0[Biome::Desert as usize] == 1.0 {
                    found = Some((x, z));
                    break 'search;
                }
            }
        }
        let (x, z) = found.expect("no pure desert point");
        assert_eq!(top_block(seed, x, z), SAND);
    }

    #[test]
    fn blend_mixes_surface() {
        // Forest/desert band: columns with forest weight between 0.2 and 0.8.
        let seed = 1;
        let (mut grass, mut sand, mut n) = (0usize, 0usize, 0usize);
        for x in (-400..400).step_by(2) {
            for z in (-400..400).step_by(2) {
                let w = biome_weights(seed, x, z).0;
                let f = w[Biome::Forest as usize];
                let d = w[Biome::Desert as usize];
                if (0.2..0.8).contains(&f) && d > 0.1 {
                    n += 1;
                    match top_block(seed, x, z) {
                        GRASS => grass += 1,
                        SAND => sand += 1,
                        _ => {}
                    }
                }
            }
        }
        assert!(n > 0, "no forest/desert band found");
        assert!(grass * 10 >= n, "GRASS {grass} of {n}");
        assert!(sand * 10 >= n, "SAND {sand} of {n}");
    }

    #[test]
    fn deterministic_regardless_of_order() {
        let seed = 9;
        let first = chunk_at(seed, 3, 1, -2);
        let _ = chunk_at(seed, 0, 0, 0);
        let _ = chunk_at(seed, -5, 2, 7);
        assert!(first == chunk_at(seed, 3, 1, -2));
    }

    #[test]
    fn new_tiles_drop_items() {
        let seed = 1;
        let mut seen = HashSet::new();
        // A ring of chunk columns out in the sectors, 320..580 blocks from spawn.
        for k in 0..16 {
            let ang = (k as f64 + 0.37) * std::f64::consts::TAU / 16.0;
            for r in [10.0f64, 14.0, 18.0] {
                let (cx, cz) = ((ang.cos() * r) as i32, (ang.sin() * r) as i32);
                for cy in 0..4 {
                    let c = chunk_at(seed, cx, cy, cz);
                    for lz in 0..CS {
                        for ly in 0..CS {
                            for lx in 0..CS {
                                seen.insert(c.get(lx, ly, lz));
                            }
                        }
                    }
                }
            }
        }
        seen.remove(&AIR);
        // Trees are chopped by the axe and yield Wood through `chop_tree`,
        // not through the generic block drop table.
        seen.remove(&TRUNK);
        // The mushroom plant drops its Mushroom pickup, which is not placeable.
        seen.remove(&crate::block::MUSHROOM_PLANT);
        // Pots drop server-rolled coins/potions and dressing tiles are worldgen-only.
        for dressing in [POT, STALACTITE, MOSS, SMALL_ROCK, SHELF_MUSHROOM] {
            seen.remove(&dressing);
        }
        for b in [SAND, HARDENED_SAND, SANDSTONE, SNOW, ICE, MUD, JUNGLE_GRASS] {
            assert!(seen.contains(&b), "biome tile {b:?} never generated");
        }
        for b in seen {
            let item = b.drop_item().unwrap_or_else(|| panic!("no drop for {b:?}"));
            assert!(
                item_data(item).create_tile.is_some(),
                "{b:?} drops a non-placeable item"
            );
        }
    }

    #[test]
    fn spawn_has_surface_grass_and_ore() {
        let (mut grass, mut ore) = (false, false);
        for cx in 0..2 {
            for cz in 0..2 {
                for cy in 0..4 {
                    let c = chunk_at(7, cx, cy, cz);
                    for lz in 0..CS {
                        for ly in 0..CS {
                            for lx in 0..CS {
                                let b = c.get(lx, ly, lz);
                                grass |= b == GRASS;
                                ore |= matches!(b, COPPER | IRON | SILVER | GOLD);
                                if cy == 3 && ly == CS - 1 {
                                    assert_eq!(b, AIR, "y=127 must be AIR");
                                }
                            }
                        }
                    }
                }
                for cy in [4, -1] {
                    assert!(
                        chunk_at(7, cx, cy, cz) == Chunk::filled(AIR),
                        "chunk y={cy} must be all AIR"
                    );
                }
            }
        }
        assert!(grass, "no GRASS generated");
        assert!(ore, "no ore generated");
    }

    #[test]
    fn generated_chunk_applies_corruption() {
        let seed = 7;
        let mut found = None;
        'search: for x in (-6000..=6000).step_by(64) {
            for z in (-6000..=6000).step_by(64) {
                if crate::worldgen::corruption::corruption_at(seed, x, z) > 0.1 {
                    found = Some(ChunkPos::of_block(x, 0, z));
                    break 'search;
                }
            }
        }
        let pos = found.expect("seed 7 has no corruption within 6000 blocks");
        let (mut corrupt, mut plants) = (false, false);
        for cy in 0..4 {
            let chunk = chunk_at(seed, pos.x, cy, pos.z);
            for lz in 0..CS {
                for ly in 0..CS {
                    for lx in 0..CS {
                        let b = chunk.get(lx, ly, lz);
                        corrupt |= matches!(b, EBONSTONE | CORRUPT_GRASS | DEMONITE_ORE);
                        plants |= matches!(b, TRUNK | MUSHROOM_PLANT);
                    }
                }
            }
        }
        assert!(corrupt, "corruption tiles missing at {pos:?}");
        assert!(!plants, "plants generated inside corruption at {pos:?}");
    }

    #[test]
    fn ocean_ring_starts_at_edge_minus_ocean_width() {
        use crate::world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, SEA_LEVEL_Y, WorldBounds, WorldSize};
        let seed = 7;
        for size in [WorldSize::Small, WorldSize::Medium, WorldSize::Large] {
            let bounds = WorldBounds::new(size);
            let half = bounds.half;
            // Ocean columns: edge_distance < OCEAN_WIDTH
            for d in [0, 10, 80, OCEAN_WIDTH - 1] {
                let x = half - 1 - d;
                let (h, top) = super::surface_top_for_size(seed, x, 0, size);
                assert!(
                    h < SEA_LEVEL_Y,
                    "ocean column at edge_distance={d} ({size:?}) has surface {h} >= SEA_LEVEL_Y ({SEA_LEVEL_Y})"
                );
                assert_eq!(top, SAND, "ocean floor at d={d} must be SAND, got {top:?}");
            }
            // Beach columns: edge_distance in OCEAN_WIDTH..OCEAN_WIDTH+BEACH_WIDTH
            let (h_shore, top_shore) =
                super::surface_top_for_size(seed, half - 1 - OCEAN_WIDTH, 0, size);
            let (h_inland, top_inland) = super::surface_top_for_size(
                seed,
                half - 1 - (OCEAN_WIDTH + BEACH_WIDTH - 1),
                0,
                size,
            );
            assert_eq!(top_shore, SAND);
            assert_eq!(top_inland, SAND);
            assert!(
                h_inland > h_shore,
                "beach must ramp up from shore ({h_shore}) to inland ({h_inland})"
            );
            assert!(
                h_inland >= SEA_LEVEL_Y,
                "inland beach edge ({h_inland}) must reach at least SEA_LEVEL_Y ({SEA_LEVEL_Y})"
            );
        }
    }

    #[test]
    fn medium_and_large_generated_trees_sit_on_sized_surface() {
        for size in [WorldSize::Medium, WorldSize::Large] {
            let seed = 7;
            let mut trunks_found = 0;
            // Scan chunks in an outer forest sector where scaled biome weights/heights
            // differ from Small (in Small cx=18..20 is Desert, in Medium/Large it is Forest).
            for cx in 18..21 {
                for cz in -3..0 {
                    let mut chunks = Vec::new();
                    for cy in 0..4 {
                        chunks.push(generate_chunk_sized(
                            seed,
                            ChunkPos {
                                x: cx,
                                y: cy,
                                z: cz,
                            },
                            size,
                        ));
                    }
                    let get_block = |lx: usize, y: i32, lz: usize| -> BlockId {
                        let cy = (y / CHUNK_SIZE) as usize;
                        let ly = (y % CHUNK_SIZE) as usize;
                        chunks[cy].get(lx, ly, lz)
                    };
                    for lz in 0..CS {
                        for lx in 0..CS {
                            let wx = cx * CHUNK_SIZE + lx as i32;
                            let wz = cz * CHUNK_SIZE + lz as i32;
                            for y in 1..WORLD_HEIGHT as i32 {
                                if get_block(lx, y, lz) == TRUNK
                                    && get_block(lx, y - 1, lz) != TRUNK
                                {
                                    trunks_found += 1;
                                    let (gy, top) = surface_top_for_size(seed, wx, wz, size);
                                    assert!(
                                        top == GRASS || top == JUNGLE_GRASS,
                                        "{size:?} trunk at ({wx},{y},{wz}) over {top:?}"
                                    );
                                    assert_eq!(
                                        y - 1,
                                        gy,
                                        "{size:?} trunk base at y={y} does not sit on surface gy={gy} at ({wx},{wz})"
                                    );
                                    assert_eq!(
                                        get_block(lx, y - 1, lz),
                                        top,
                                        "{size:?} block below trunk at ({wx},{y},{wz}) is not surface {top:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
            assert!(
                trunks_found > 0,
                "expected trees in {size:?} world sample chunks"
            );
        }
    }

    #[test]
    fn medium_world_contains_guaranteed_corruption_patch() {
        let seed = 7;
        let size = WorldSize::Medium;
        let bounds = WorldBounds::new(size);
        let inland = bounds.half - OCEAN_WIDTH - BEACH_WIDTH;
        let mut guaranteed = None;
        let mut any_corrupt = None;
        for x in (-inland..inland).step_by(32) {
            for z in (-inland..inland).step_by(32) {
                if crate::worldgen::corruption::corruption_at_for_size(seed, x, z, size) > 0.5 {
                    any_corrupt.get_or_insert(ChunkPos::of_block(x, 0, z));
                    if crate::worldgen::corruption::corruption_at(seed, x, z) == 0.0 {
                        guaranteed = Some(ChunkPos::of_block(x, 0, z));
                        break;
                    }
                }
            }
            if guaranteed.is_some() {
                break;
            }
        }
        let pos = guaranteed
            .or(any_corrupt)
            .expect("seed 7 Medium world has no column with corruption > 0.5");
        let mut corrupt = false;
        for cy in 0..4 {
            let chunk = generate_chunk_sized(
                seed,
                ChunkPos {
                    x: pos.x,
                    y: cy,
                    z: pos.z,
                },
                size,
            );
            for lz in 0..CS {
                for ly in 0..CS {
                    for lx in 0..CS {
                        let b = chunk.get(lx, ly, lz);
                        corrupt |= matches!(b, EBONSTONE | CORRUPT_GRASS);
                    }
                }
            }
        }
        assert!(
            corrupt,
            "Medium world guaranteed corruption patch missing EBONSTONE/CORRUPT_GRASS at {pos:?}"
        );
    }

    #[test]
    fn depth_band_matches_terraria_layers() {
        let surface = 100;
        // Surface: y > surface_y - 25
        assert_eq!(depth_band(surface, 105), DepthBand::Surface);
        assert_eq!(depth_band(surface, 100), DepthBand::Surface);
        assert_eq!(depth_band(surface, 76), DepthBand::Surface);

        // Underground: surface_y - 25 >= y > surface_y - 70
        assert_eq!(depth_band(surface, 75), DepthBand::Underground);
        assert_eq!(depth_band(surface, 50), DepthBand::Underground);
        assert_eq!(depth_band(surface, 31), DepthBand::Underground);

        // Cavern: y <= surface_y - 70 and y > UNDERWORLD_TOP_Y
        assert_eq!(depth_band(surface, 30), DepthBand::Cavern);

        // Underworld: y <= UNDERWORLD_TOP_Y (24)
        assert_eq!(depth_band(surface, 24), DepthBand::Underworld);
        assert_eq!(depth_band(surface, 10), DepthBand::Underworld);
        assert_eq!(depth_band(surface, 0), DepthBand::Underworld);
        assert_eq!(depth_band(surface, -20), DepthBand::Underworld);
    }

    #[test]
    fn cavern_has_caves() {
        let seed = 42;
        let mut cavern_air = 0;
        let mut cavern_solid = 0;
        for x in (-300..300).step_by(16) {
            for z in (-300..300).step_by(16) {
                let h = surface_height(seed, x, z);
                let cavern_top = h - 70;
                if cavern_top > 0 {
                    for y in 0..=cavern_top {
                        let block = natural_block(seed, x, y, z);
                        if block == AIR {
                            cavern_air += 1;
                        } else {
                            cavern_solid += 1;
                        }
                    }
                }
            }
        }
        assert!(cavern_air > 0, "cavern layer must contain carved caves");
        assert!(
            cavern_solid > 0,
            "cavern layer must preserve structural support"
        );
        let ratio = cavern_air as f64 / (cavern_air + cavern_solid) as f64;
        assert!(
            ratio >= 0.18 && ratio <= 0.35,
            "cavern cave ratio {ratio:.3} out of expected range (0.18..0.35)"
        );
    }

    #[test]
    fn cavern_has_denser_caves_than_surface() {
        let seed = 42;
        let mut surface_air = 0;
        let mut surface_total = 0;
        let mut cavern_air = 0;
        let mut cavern_total = 0;

        for x in (-300..300).step_by(16) {
            for z in (-300..300).step_by(16) {
                let h = surface_height(seed, x, z);
                for d in 3..25 {
                    let y = h - d;
                    if y >= 0 {
                        surface_total += 1;
                        if natural_block(seed, x, y, z) == AIR {
                            surface_air += 1;
                        }
                    }
                }
                let cavern_top = h - 70;
                if cavern_top > 0 {
                    for y in 0..=cavern_top {
                        cavern_total += 1;
                        if natural_block(seed, x, y, z) == AIR {
                            cavern_air += 1;
                        }
                    }
                }
            }
        }
        let surface_ratio = surface_air as f64 / surface_total as f64;
        let cavern_ratio = cavern_air as f64 / cavern_total as f64;
        assert!(
            cavern_ratio > surface_ratio * 1.25,
            "cavern cave ratio ({cavern_ratio:.3}) must be significantly richer than surface subterranean ratio ({surface_ratio:.3})"
        );
    }

    #[test]
    fn underground_cabin_generates_with_chest() {
        let seed = 12345;
        let mut found_cabin = false;
        'outer: for cx in -10..10 {
            for cz in -10..10 {
                for cy in 0..3 {
                    let pos = ChunkPos {
                        x: cx,
                        y: cy,
                        z: cz,
                    };
                    let chunk = generate_chunk(seed, pos);
                    if chunk.has_walls() {
                        let mut has_wood = false;
                        let mut has_chest = false;
                        let mut chest_local = None;
                        for lz in 0..32 {
                            for ly in 0..32 {
                                for lx in 0..32 {
                                    let b = chunk.get(lx, ly, lz);
                                    if b == WOOD_BLOCK {
                                        has_wood = true;
                                    } else if b == CHEST {
                                        has_chest = true;
                                        chest_local = Some((lx, ly, lz));
                                    }
                                }
                            }
                        }
                        if has_wood && has_chest {
                            let (lx, ly, lz) = chest_local.unwrap();
                            let wy = cy * 32 + ly as i32;
                            let wx = cx * 32 + lx as i32;
                            let wz = cz * 32 + lz as i32;
                            let surf = surface_height(seed, wx, wz);
                            let band = depth_band(surf, wy);
                            assert!(
                                matches!(band, DepthBand::Underground | DepthBand::Cavern),
                                "cabin chest at wy={wy}, surface={surf} must be underground/cavern"
                            );
                            let chests = crate::worldgen::chest_gen::chests_in(seed, pos);
                            assert!(
                                chests
                                    .iter()
                                    .any(|(p, loot)| { *p == [wx, wy, wz] && !loot.is_empty() }),
                                "cabin chest at ({wx}, {wy}, {wz}) must have loot registered in chests_in"
                            );
                            found_cabin = true;
                            break 'outer;
                        }
                    }
                }
            }
        }
        assert!(found_cabin, "an underground cabin with chest must generate");
    }

    #[test]
    fn surface_structures_and_pots_generate_deterministically_in_new_chunks() {
        let seed = 42;
        let mut pots = 0usize;
        let mut stalactites = 0usize;
        let mut moss = 0usize;
        let mut small_rocks = 0usize;
        let mut shelf_mushrooms = 0usize;
        let mut structure_kinds = std::collections::HashSet::new();

        for cx in -2..=2 {
            for cz in -2..=2 {
                for cy in 0..4 {
                    let pos = ChunkPos {
                        x: cx,
                        y: cy,
                        z: cz,
                    };
                    let a = generate_chunk(seed, pos);
                    let b = generate_chunk(seed, pos);
                    assert_eq!(a, b, "chunk {pos:?} must be deterministic");
                    for lz in 0..CS {
                        for ly in 0..CS {
                            for lx in 0..CS {
                                match a.get(lx, ly, lz) {
                                    POT => pots += 1,
                                    STALACTITE => stalactites += 1,
                                    MOSS => moss += 1,
                                    SMALL_ROCK => small_rocks += 1,
                                    SHELF_MUSHROOM => shelf_mushrooms += 1,
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
        for cx in -6..=6 {
            for cz in -6..=6 {
                if let Some(kind) =
                    super::super::dressing::surface_structure_kind_in_chunk(seed, cx, cz)
                {
                    structure_kinds.insert(kind);
                }
            }
        }
        assert!(pots > 0, "expected pots to generate");
        assert!(stalactites > 0, "expected stalactites to generate");
        assert!(moss > 0, "expected moss to generate");
        assert!(small_rocks > 0, "expected small rocks to generate");
        assert!(shelf_mushrooms > 0, "expected shelf mushrooms to generate");
        assert_eq!(
            structure_kinds.len(),
            4,
            "expected all 4 surface structure types, got {structure_kinds:?}"
        );
    }

    #[test]
    fn surface_cave_mouths_open_around_spawn_while_spawn_stays_solid() {
        let seed = 7;
        // Spawn itself stays solid.
        for x in -4..=4 {
            for z in -4..=4 {
                let sy = surface_height(seed, x, z);
                assert_ne!(
                    natural_block(seed, x, sy, z),
                    AIR,
                    "spawn surface at ({x}, {sy}, {z}) must stay solid"
                );
            }
        }
        // Scanning a 200x200 area around spawn finds multiple genuine surface cave openings.
        let mut openings = 0usize;
        for z in -100..=100 {
            for x in -100..=100 {
                let sy = surface_height(seed, x, z);
                if natural_block(seed, x, sy, z) == AIR && natural_block(seed, x, sy - 4, z) == AIR
                {
                    openings += 1;
                }
            }
        }
        assert!(
            openings >= 20,
            "expected multiple surface cave openings in 200x200 around spawn, got {openings}"
        );
    }
}
