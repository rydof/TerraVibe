//! Deterministic surface structures, breakable pots, and cave dressing.
//!
//! Runs during chunk generation so only newly generated chunks receive them;
//! existing saved chunks are untouched.

use crate::block::{
    AIR, DIRT, GRASS, ICE, MOSS, MUD, POT, SAND, SANDSTONE, SHELF_MUSHROOM, SMALL_ROCK, SNOW,
    STALACTITE, STONE, TORCH, WOOD_BLOCK, WORKBENCH,
};
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, WorldBounds, WorldSize};
use crate::worldgen::noise::hash;
use crate::worldgen::terrain;

const SALT_STRUCT: u32 = 0x5354_5255;
const SALT_DRESS: u32 = 0x4452_4553;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SurfaceStructureKind {
    StoneRuins,
    CollapsedWell,
    AbandonedCamp,
    RoadsideShrine,
}

/// Returns the deterministic surface structure center `(wx, wy, wz, kind)` for
/// chunk column `(cx, cz)`, if any.
pub fn surface_structure_in_chunk(
    seed: u32,
    cx: i32,
    cz: i32,
    size: WorldSize,
) -> Option<(i32, i32, i32, SurfaceStructureKind)> {
    let h = hash(seed ^ SALT_STRUCT, cx, 0, cz);
    if h % 24 != 0 {
        return None;
    }
    let ox = 6 + ((h >> 8) % 20) as i32;
    let oz = 6 + ((h >> 16) % 20) as i32;
    let wx = cx * CHUNK_SIZE + ox;
    let wz = cz * CHUNK_SIZE + oz;
    if wx.abs() <= 72 && wz.abs() <= 72 {
        return None;
    }
    if WorldBounds::new(size).edge_distance(wx, wz) < OCEAN_WIDTH + BEACH_WIDTH + 6 {
        return None;
    }
    if super::corruption::corruption_at_for_size(seed, wx, wz, size) > 0.0 {
        return None;
    }
    let de = super::dungeon::dungeon_entrance_pos(seed, size);
    if (wx - de[0] as i32).abs() < 16 && (wz - de[2] as i32).abs() < 16 {
        return None;
    }
    let wy = terrain::surface_height_for_size(seed, wx, wz, size);
    if !(8..(terrain::WORLD_HEIGHT as i32 - 10)).contains(&wy) {
        return None;
    }
    let kind = match (h >> 24) % 4 {
        0 => SurfaceStructureKind::StoneRuins,
        1 => SurfaceStructureKind::CollapsedWell,
        2 => SurfaceStructureKind::AbandonedCamp,
        _ => SurfaceStructureKind::RoadsideShrine,
    };
    Some((wx, wy, wz, kind))
}

/// Convenience helper for tests using `WorldSize::Small`.
pub fn surface_structure_kind_in_chunk(
    seed: u32,
    cx: i32,
    cz: i32,
) -> Option<SurfaceStructureKind> {
    surface_structure_in_chunk(seed, cx, cz, WorldSize::Small).map(|(_, _, _, k)| k)
}

fn set_if_in_chunk(
    chunk: &mut Chunk,
    pos: ChunkPos,
    wx: i32,
    wy: i32,
    wz: i32,
    block: crate::block::BlockId,
) {
    let cs = CHUNK_SIZE;
    let (bx, by, bz) = (pos.x * cs, pos.y * cs, pos.z * cs);
    if (bx..bx + cs).contains(&wx) && (by..by + cs).contains(&wy) && (bz..bz + cs).contains(&wz) {
        let lx = (wx - bx) as usize;
        let ly = (wy - by) as usize;
        let lz = (wz - bz) as usize;
        chunk.set(lx, ly, lz, block);
        chunk.set_liquid(lx, ly, lz, None);
    }
}

pub fn stone_ruins(chunk: &mut Chunk, pos: ChunkPos, cx: i32, cy: i32, cz: i32) {
    for dx in -2..=2 {
        for dz in -2..=2 {
            // Stone floor and shallow foundation
            for dy in -1..=0 {
                set_if_in_chunk(chunk, pos, cx + dx, cy + dy, cz + dz, STONE);
            }
            let edge = dx.abs() == 2 || dz.abs() == 2;
            if edge && !(dx == 0 && dz == -2) && (dx + dz).abs() % 2 == 0 {
                set_if_in_chunk(chunk, pos, cx + dx, cy + 1, cz + dz, STONE);
                if dx.abs() == 2 && dz.abs() == 2 {
                    set_if_in_chunk(chunk, pos, cx + dx, cy + 2, cz + dz, STONE);
                }
            }
        }
    }
    set_if_in_chunk(chunk, pos, cx + 1, cy + 1, cz, POT);
    set_if_in_chunk(chunk, pos, cx - 1, cy + 1, cz + 1, POT);
    set_if_in_chunk(chunk, pos, cx, cy + 1, cz - 1, MOSS);
}

pub fn collapsed_well(chunk: &mut Chunk, pos: ChunkPos, cx: i32, cy: i32, cz: i32) {
    for dx in -1..=1 {
        for dz in -1..=1 {
            if dx == 0 && dz == 0 {
                for dy in -2..=1 {
                    set_if_in_chunk(chunk, pos, cx, cy + dy, cz, AIR);
                }
            } else {
                set_if_in_chunk(chunk, pos, cx + dx, cy, cz + dz, STONE);
                if !(dx == -1 && dz == -1) {
                    set_if_in_chunk(chunk, pos, cx + dx, cy + 1, cz + dz, STONE);
                }
            }
        }
    }
    set_if_in_chunk(chunk, pos, cx + 1, cy + 2, cz, WOOD_BLOCK);
    set_if_in_chunk(chunk, pos, cx + 2, cy + 1, cz, POT);
    set_if_in_chunk(chunk, pos, cx - 2, cy + 1, cz, SMALL_ROCK);
}

pub fn abandoned_camp(chunk: &mut Chunk, pos: ChunkPos, cx: i32, cy: i32, cz: i32) {
    for dx in -1..=1 {
        for dz in -1..=1 {
            set_if_in_chunk(chunk, pos, cx + dx, cy, cz + dz, DIRT);
        }
    }
    set_if_in_chunk(chunk, pos, cx, cy + 1, cz, TORCH);
    set_if_in_chunk(chunk, pos, cx - 1, cy + 1, cz, WOOD_BLOCK);
    set_if_in_chunk(chunk, pos, cx + 1, cy + 1, cz, WOOD_BLOCK);
    set_if_in_chunk(chunk, pos, cx, cy + 1, cz + 2, WORKBENCH);
    set_if_in_chunk(chunk, pos, cx - 1, cy + 1, cz + 1, POT);
}

pub fn roadside_shrine(chunk: &mut Chunk, pos: ChunkPos, cx: i32, cy: i32, cz: i32) {
    for dx in -1..=1 {
        for dz in -1..=1 {
            set_if_in_chunk(chunk, pos, cx + dx, cy, cz + dz, STONE);
        }
    }
    set_if_in_chunk(chunk, pos, cx, cy + 1, cz, STONE);
    set_if_in_chunk(chunk, pos, cx, cy + 2, cz, STONE);
    set_if_in_chunk(chunk, pos, cx, cy + 3, cz, TORCH);
    set_if_in_chunk(chunk, pos, cx + 1, cy + 1, cz, POT);
    set_if_in_chunk(chunk, pos, cx - 1, cy + 1, cz, MOSS);
}

/// Stamp surface structures, breakable pots, and cave dressing into `chunk`.
pub fn apply_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let cs = CHUNK_SIZE;
    let (bx, by, bz) = (pos.x * cs, pos.y * cs, pos.z * cs);
    if by + cs <= 0 || by as i64 >= terrain::WORLD_HEIGHT {
        return;
    }

    if let Some((wx, wy, wz, kind)) = surface_structure_in_chunk(seed, pos.x, pos.z, size) {
        match kind {
            SurfaceStructureKind::StoneRuins => stone_ruins(chunk, pos, wx, wy, wz),
            SurfaceStructureKind::CollapsedWell => collapsed_well(chunk, pos, wx, wy, wz),
            SurfaceStructureKind::AbandonedCamp => abandoned_camp(chunk, pos, wx, wy, wz),
            SurfaceStructureKind::RoadsideShrine => roadside_shrine(chunk, pos, wx, wy, wz),
        }
    }

    let bounds = WorldBounds::new(size);
    for lz in 0..cs as usize {
        let wz = bz + lz as i32;
        for lx in 0..cs as usize {
            let wx = bx + lx as i32;
            if bounds.edge_distance(wx, wz) < OCEAN_WIDTH + BEACH_WIDTH {
                continue;
            }
            if super::corruption::corruption_at_for_size(seed, wx, wz, size) > 0.0 {
                continue;
            }
            let surf = terrain::surface_height_for_size(seed, wx, wz, size);
            for ly in 1..(cs as usize - 1) {
                let wy = by + ly as i32;
                if wy <= super::underworld::UNDERWORLD_TOP_Y
                    || wy >= terrain::WORLD_HEIGHT as i32 - 2
                {
                    continue;
                }
                if chunk.get(lx, ly, lz) != AIR || chunk.liquid(lx, ly, lz).is_some() {
                    continue;
                }
                let below = chunk.get(lx, ly - 1, lz);
                let above = chunk.get(lx, ly + 1, lz);
                let h = hash(seed ^ SALT_DRESS, wx, wy, wz);

                if wy < surf - 4 {
                    if matches!(above, STONE | ICE | SANDSTONE) && h % 45 == 0 {
                        chunk.set(lx, ly, lz, STALACTITE);
                    } else if matches!(below, STONE | DIRT | MUD | SANDSTONE | ICE) && h % 110 == 1
                    {
                        chunk.set(lx, ly, lz, POT);
                    } else if matches!(below, STONE | MUD | DIRT) && h % 60 == 2 {
                        chunk.set(lx, ly, lz, MOSS);
                    } else if matches!(below, STONE | DIRT) && h % 70 == 3 {
                        chunk.set(lx, ly, lz, SMALL_ROCK);
                    } else if wy < surf - 8
                        && (matches!(below, STONE | MUD)
                            || (lx > 0 && matches!(chunk.get(lx - 1, ly, lz), STONE | MUD)))
                        && h % 75 == 4
                    {
                        chunk.set(lx, ly, lz, SHELF_MUSHROOM);
                    }
                } else if wy == surf + 1
                    && (wx.abs() > 40 || wz.abs() > 40)
                    && matches!(below, GRASS | DIRT | STONE | SAND | SNOW)
                {
                    if h % 160 == 5 {
                        chunk.set(lx, ly, lz, SMALL_ROCK);
                    } else if matches!(below, GRASS | STONE) && h % 220 == 6 {
                        chunk.set(lx, ly, lz, MOSS);
                    } else if matches!(below, GRASS | STONE | DIRT) && h % 480 == 7 {
                        chunk.set(lx, ly, lz, POT);
                    }
                }
            }
        }
    }
}
