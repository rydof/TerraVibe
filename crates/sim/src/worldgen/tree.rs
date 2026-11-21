//! Deterministic tree placement.
//!
//! One candidate per `TREE_CELL × TREE_CELL` cell; the candidate is a pure
//! function of `(seed, cell)`, so a tree is identical no matter which chunk
//! asks for it or in which order chunks are generated.

use crate::block::{AIR, GRASS, JUNGLE_GRASS, TRUNK};
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::world_bounds::WorldSize;
use crate::worldgen::noise::hash;
use crate::worldgen::terrain;

/// One candidate tree per 8×8 cell, so no two trees can share a column.
pub const TREE_CELL: i32 = 8;

/// Height in tiles: `5 + h % 12` forest, +5 jungle (gameplay spec,30112`).
const BASE_HEIGHT: i32 = 5;
const HEIGHT_SPREAD: i32 = 12;
const JUNGLE_BONUS: i32 = 5;
const TREE_CHANCE_PERCENT: u32 = 18;

// Per-feature seed salts.
const SALT_TREE: u32 = 0x7A3E_1B9D;
const SALT_TREE_X: u32 = 0x11C7_5E3B;
const SALT_TREE_Z: u32 = 0x5D2B_8F47;
const SALT_TREE_HEIGHT: u32 = 0x3F91_C6A5;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    /// Lowest trunk block, directly on top of the surface.
    pub base: [i32; 3],
    pub height: i32,
}

/// The tree candidate for one cell, if the cell rolls one and the ground fits.
pub fn tree_in_cell(seed: u32, cell: [i32; 2]) -> Option<Tree> {
    tree_in_cell_for_size(seed, cell, WorldSize::Small)
}

pub fn tree_in_cell_for_size(seed: u32, cell: [i32; 2], size: WorldSize) -> Option<Tree> {
    let [cx, cz] = cell;
    // 12% of cells get a candidate; WorldGen.GrowTree picks trees per area.
    if hash(seed ^ SALT_TREE, cx, 0, cz) % 100 >= TREE_CHANCE_PERCENT {
        return None;
    }
    let ox = (hash(seed ^ SALT_TREE_X, cx, 0, cz) % TREE_CELL as u32) as i32;
    let oz = (hash(seed ^ SALT_TREE_Z, cx, 0, cz) % TREE_CELL as u32) as i32;
    let x = cx.wrapping_mul(TREE_CELL).wrapping_add(ox);
    let z = cz.wrapping_mul(TREE_CELL).wrapping_add(oz);

    // Corruption replaces grass and hosts no trees.
    if super::corruption::corruption_at_for_size(seed, x, z, size) > 0.0 {
        return None;
    }

    // Only grass is plantable; sand, snow, ice and liquids never host trees.
    let (gy, top) = terrain::surface_top_for_size(seed, x, z, size);
    if top != GRASS && top != JUNGLE_GRASS {
        return None;
    }
    // Flat ground one block each way rejects cliff edges.
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let (ny, nb) =
            terrain::surface_top_for_size(seed, x.wrapping_add(dx), z.wrapping_add(dz), size);
        if nb != GRASS && nb != JUNGLE_GRASS {
            return None;
        }
        if (ny - gy).abs() > 1 {
            return None;
        }
    }

    // gameplay spec,30112: 5 + rand(12), +5 for jungle.
    let mut height =
        BASE_HEIGHT + (hash(seed ^ SALT_TREE_HEIGHT, cx, 0, cz) % HEIGHT_SPREAD as u32) as i32;
    if top == JUNGLE_GRASS {
        height += JUNGLE_BONUS;
    }
    let base_y = gy + 1;
    if i64::from(base_y + height) >= terrain::WORLD_HEIGHT {
        return None;
    }
    Some(Tree {
        base: [x, base_y, z],
        height,
    })
}

/// Every tree whose base column lies in the inclusive rectangle.
pub fn trees_in(seed: u32, x0: i32, z0: i32, x1: i32, z1: i32) -> Vec<Tree> {
    trees_in_for_size(seed, x0, z0, x1, z1, WorldSize::Small)
}

pub fn trees_in_for_size(
    seed: u32,
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
    size: WorldSize,
) -> Vec<Tree> {
    if x0 > x1 || z0 > z1 {
        return Vec::new();
    }
    let (cx0, cx1) = (x0.div_euclid(TREE_CELL), x1.div_euclid(TREE_CELL));
    let (cz0, cz1) = (z0.div_euclid(TREE_CELL), z1.div_euclid(TREE_CELL));
    let mut out = Vec::new();
    // Fixed cell order keeps the output deterministic.
    for cz in cz0..=cz1 {
        for cx in cx0..=cx1 {
            if let Some(t) = tree_in_cell_for_size(seed, [cx, cz], size) {
                if (x0..=x1).contains(&t.base[0]) && (z0..=z1).contains(&t.base[2]) {
                    out.push(t);
                }
            }
        }
    }
    out
}

/// Stamp the trunk blocks of every tree column owned by this chunk.
pub fn decorate(seed: u32, pos: ChunkPos, chunk: &mut Chunk) {
    decorate_for_size(seed, pos, WorldSize::Small, chunk);
}

pub fn decorate_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let cs = CHUNK_SIZE;
    let (bx, by, bz) = (
        pos.x.wrapping_mul(cs),
        pos.y.wrapping_mul(cs),
        pos.z.wrapping_mul(cs),
    );
    let (cx0, cx1) = (
        bx.div_euclid(TREE_CELL),
        (bx + cs - 1).div_euclid(TREE_CELL),
    );
    let (cz0, cz1) = (
        bz.div_euclid(TREE_CELL),
        (bz + cs - 1).div_euclid(TREE_CELL),
    );
    for cz in cz0..=cz1 {
        for cx in cx0..=cx1 {
            let Some(t) = tree_in_cell_for_size(seed, [cx, cz], size) else {
                continue;
            };
            // This tree's whole column belongs to exactly one x/z chunk.
            if !(bx..bx + cs).contains(&t.base[0]) || !(bz..bz + cs).contains(&t.base[2]) {
                continue;
            }
            let lx = (t.base[0] - bx) as usize;
            let lz = (t.base[2] - bz) as usize;
            for dy in 0..t.height {
                let y = t.base[1] + dy;
                if y < by || y >= by + cs || i64::from(y) >= terrain::WORLD_HEIGHT {
                    continue;
                }
                let ly = (y - by) as usize;
                // Never overwrite terrain or another tree; the run is complete
                // because every chunk that owns part of it writes its own part.
                if chunk.get(lx, ly, lz) == AIR {
                    chunk.set(lx, ly, lz, TRUNK);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::World;
    use crate::worldgen::biome::{Biome, biome_weights};
    use crate::worldgen::generate_chunk;
    use crate::worldgen::terrain::{surface_height, surface_top};
    use std::collections::HashSet;

    const CS: i32 = CHUNK_SIZE;

    /// First point whose biome weight for `want` is 1.0.
    fn pure_point(seed: u32, want: Biome) -> (i32, i32) {
        for x in (-700..700).step_by(7) {
            for z in (-700..700).step_by(7) {
                if biome_weights(seed, x, z).0[want as usize] == 1.0 {
                    return (x, z);
                }
            }
        }
        panic!("no pure {want:?} point");
    }

    fn assert_trunk_run(seed: u32, t: Tree) {
        let mut w = World::new();
        let cx = t.base[0].div_euclid(CS);
        let cz = t.base[2].div_euclid(CS);
        for cy in 0..4 {
            let pos = ChunkPos {
                x: cx,
                y: cy,
                z: cz,
            };
            w.insert(pos, generate_chunk(seed, pos));
        }
        for dy in 0..t.height {
            assert_eq!(
                w.block(t.base[0], t.base[1] + dy, t.base[2]),
                TRUNK,
                "missing trunk {dy} of {t:?}"
            );
        }
        assert_eq!(
            w.block(t.base[0], t.base[1] + t.height, t.base[2]),
            AIR,
            "block above tree {t:?}"
        );
    }

    #[test]
    fn tree_cells_are_unique() {
        let seed = 1;
        // Scan 100×100 cells in the jungle so the +5 jungle height is covered.
        let (jx, jz) = pure_point(seed, Biome::Jungle);
        let (jcx, jcz) = (jx.div_euclid(TREE_CELL), jz.div_euclid(TREE_CELL));
        let mut columns = HashSet::new();
        let (mut trees, mut jungle) = (0, 0);
        for cz in jcz - 50..jcz + 50 {
            for cx in jcx - 50..jcx + 50 {
                let Some(t) = tree_in_cell(seed, [cx, cz]) else {
                    continue;
                };
                trees += 1;
                assert!(
                    columns.insert((t.base[0], t.base[2])),
                    "duplicate tree column at {:?}",
                    t.base
                );
                let (gy, top) = surface_top(seed, t.base[0], t.base[2]);
                assert_eq!(t.base[1], gy + 1, "base not on the surface: {t:?}");
                if top == JUNGLE_GRASS {
                    jungle += 1;
                    assert!((10..=21).contains(&t.height), "jungle height {}", t.height);
                } else {
                    assert!((5..=16).contains(&t.height), "forest height {}", t.height);
                }
            }
        }
        assert!(trees > 100, "too few trees: {trees}");
        assert!(jungle > 0, "no jungle trees in 100×100 cells");
    }

    #[test]
    fn trunk_matches_tree_in_cell_across_chunks() {
        let seed = 1;
        let mut w = World::new();
        for cz in -1..=1 {
            for cx in -1..=1 {
                for cy in 0..4 {
                    let pos = ChunkPos {
                        x: cx,
                        y: cy,
                        z: cz,
                    };
                    w.insert(pos, generate_chunk(seed, pos));
                }
            }
        }
        let trees = trees_in(seed, -32, -32, 63, 63);
        assert!(!trees.is_empty(), "no trees in the 3×3 chunk area");
        for t in trees {
            for dy in 0..t.height {
                assert_eq!(
                    w.block(t.base[0], t.base[1] + dy, t.base[2]),
                    TRUNK,
                    "half tree at chunk border: {t:?}"
                );
            }
            assert_eq!(w.block(t.base[0], t.base[1] + t.height, t.base[2]), AIR);
        }
    }

    #[test]
    fn tree_spans_chunk_borders_intact() {
        let seed = 1;
        let mut found = None;
        // Widen the band instead of skipping if no border tree is in range.
        for band in [200, 600, 1200] {
            let trees = trees_in(seed, -band, -band, band, band);
            let xb = trees
                .iter()
                .find(|t| t.base[0].rem_euclid(CS) >= CS - 2)
                .copied();
            let zb = trees
                .iter()
                .find(|t| t.base[2].rem_euclid(CS) >= CS - 2)
                .copied();
            let yb = trees
                .iter()
                .find(|t| t.base[1].div_euclid(CS) != (t.base[1] + t.height - 1).div_euclid(CS))
                .copied();
            if let (Some(xb), Some(zb), Some(yb)) = (xb, zb, yb) {
                found = Some((xb, zb, yb));
                break;
            }
        }
        let (xb, zb, yb) = found.expect("no tree crossing a chunk border");
        for t in [xb, zb, yb] {
            assert_trunk_run(seed, t);
        }
    }

    #[test]
    fn trees_need_grass_and_flat_ground() {
        let seed = 1;
        let trees = trees_in(seed, -200, -200, 200, 200);
        assert!(trees.len() > 20, "too few trees: {}", trees.len());
        for t in trees {
            let (gy, ground) = surface_top(seed, t.base[0], t.base[2]);
            assert!(
                ground == GRASS || ground == JUNGLE_GRASS,
                "tree on {ground:?}: {t:?}"
            );
            assert_eq!(t.base[1], gy + 1);
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (ny, nb) = surface_top(seed, t.base[0] + dx, t.base[2] + dz);
                assert!(
                    nb == GRASS || nb == JUNGLE_GRASS,
                    "tree next to {nb:?}: {t:?}"
                );
                assert!(
                    (ny - gy).abs() <= 1,
                    "tree on a cliff, neighbour height {ny} vs {gy}: {t:?}"
                );
            }
        }
    }

    #[test]
    fn trees_avoid_desert_and_snow() {
        let seed = 1;
        for biome in [Biome::Desert, Biome::Snow] {
            let (px, pz) = pure_point(seed, biome);
            let mut w = World::new();
            let (cx0, cx1) = ((px - 4).div_euclid(CS), (px + 4).div_euclid(CS));
            let (cz0, cz1) = ((pz - 4).div_euclid(CS), (pz + 4).div_euclid(CS));
            for cx in cx0..=cx1 {
                for cz in cz0..=cz1 {
                    for cy in 0..4 {
                        let pos = ChunkPos {
                            x: cx,
                            y: cy,
                            z: cz,
                        };
                        w.insert(pos, generate_chunk(seed, pos));
                    }
                }
            }
            for x in px - 4..=px + 4 {
                for z in pz - 4..=pz + 4 {
                    for y in 0..128 {
                        assert_ne!(
                            w.block(x, y, z),
                            TRUNK,
                            "tree in {biome:?} at ({x},{y},{z})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn decorate_is_idempotent() {
        let seed = 1;
        for pos in [
            ChunkPos { x: 0, y: 2, z: 0 },
            ChunkPos { x: -1, y: 1, z: 2 },
        ] {
            let first = generate_chunk(seed, pos);
            assert_eq!(
                first,
                generate_chunk(seed, pos),
                "chunk is not deterministic"
            );
            let mut again = first.clone();
            decorate(seed, pos, &mut again);
            assert_eq!(first, again, "decorate changed an already-decorated chunk");
        }
    }

    #[test]
    fn surface_top_matches_generated_surface() {
        let seed = 3;
        let mut w = World::new();
        for cx in (-40i32).div_euclid(CS)..=40i32.div_euclid(CS) {
            for cz in (-40i32).div_euclid(CS)..=40i32.div_euclid(CS) {
                for cy in 0..4 {
                    let pos = ChunkPos {
                        x: cx,
                        y: cy,
                        z: cz,
                    };
                    w.insert(pos, generate_chunk(seed, pos));
                }
            }
        }
        for x in -40..40 {
            for z in -40..40 {
                let (y, b) = surface_top(seed, x, z);
                assert_eq!(y, surface_height(seed, x, z));
                if crate::worldgen::terrain::in_surface_cave_mouth(seed, x, y, z, WorldSize::Small)
                {
                    assert_eq!(b, AIR);
                    continue;
                }
                assert_ne!(b, AIR);
                let above = w.block(x, y + 1, z);
                assert!(
                    matches!(
                        above,
                        AIR | TRUNK
                            | crate::block::MUSHROOM_PLANT
                            | crate::block::STONE
                            | crate::block::DIRT
                            | crate::block::WOOD_BLOCK
                            | crate::block::TORCH
                            | crate::block::WORKBENCH
                            | crate::block::POT
                            | crate::block::MOSS
                            | crate::block::SMALL_ROCK
                    ),
                    "surface top {y} at ({x},{z}) is covered by {above:?}"
                );
            }
        }
    }
}
