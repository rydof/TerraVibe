//! Dungeon world generation: surface entrance, descending staircase, and underground brick rooms.

use crate::block::*;
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::world_bounds::WorldSize;

const SALT_SECTOR: u32 = 0x7B2E_D4C9;

/// Determines whether the Jungle biome is predominantly on the positive X side (+1)
/// or negative X side (-1).
pub fn jungle_side(seed: u32, size: WorldSize) -> i32 {
    let half = size.half_extent();
    let sample_x = (half as f64 * 0.6) as i32;
    let mut pos_j = 0.0f32;
    let mut neg_j = 0.0f32;
    for z in [-sample_x / 2, 0, sample_x / 2] {
        pos_j += crate::worldgen::biome::biome_weights_for_size(seed, sample_x, z, size).0
            [crate::worldgen::biome::Biome::Jungle as usize];
        neg_j += crate::worldgen::biome::biome_weights_for_size(seed, -sample_x, z, size).0
            [crate::worldgen::biome::Biome::Jungle as usize];
    }
    if pos_j > neg_j {
        1
    } else if neg_j > pos_j {
        -1
    } else {
        let off = crate::worldgen::noise::hash(seed ^ SALT_SECTOR, 0, 0, 0) as f64
            / 4_294_967_296.0
            * 6.0;
        let p_ang = (4.5 - off).rem_euclid(6.0) / 1.5;
        if (1.0..3.0).contains(&p_ang) { -1 } else { 1 }
    }
}

/// The X-direction side for the dungeon (-1 or +1), opposite to the Jungle.
pub fn dungeon_side(seed: u32, size: WorldSize) -> i32 {
    -jungle_side(seed, size)
}

/// World seed selects one of the three classic dungeon brick types.
pub fn dungeon_brick(seed: u32) -> BlockId {
    match crate::worldgen::noise::hash(seed ^ 0xD06E_B10C, 0, 0, 0) % 3 {
        0 => BLUE_DUNGEON_BRICK,
        1 => GREEN_DUNGEON_BRICK,
        _ => PINK_DUNGEON_BRICK,
    }
}

/// Surface entrance position for the dungeon.
pub fn dungeon_entrance_pos(seed: u32, size: WorldSize) -> [f32; 3] {
    let side = dungeon_side(seed, size);
    let half = size.half_extent();
    let jitter_x = ((crate::worldgen::noise::hash(seed ^ 0xD06E_0001, 0, 0, 0) % 41) as i32) - 20;
    let x = side * (half * 68 / 100) + jitter_x;
    let jitter_z = ((crate::worldgen::noise::hash(seed ^ 0xD06E_0002, 0, 0, 0) % 41) as i32) - 20;
    let z = jitter_z;
    let surf_y = crate::worldgen::terrain::surface_height_for_size(seed, x, z, size);
    [x as f32, (surf_y + 1) as f32, z as f32]
}

/// A rectangular volume that has outer brick walls and inner air.
#[derive(Clone, Copy, Debug)]
pub struct DungeonBox {
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub wall_thick: i32,
}

impl DungeonBox {
    pub const fn contains(&self, x: i32, y: i32, z: i32) -> bool {
        x >= self.min[0]
            && x <= self.max[0]
            && y >= self.min[1]
            && y <= self.max[1]
            && z >= self.min[2]
            && z <= self.max[2]
    }
}

/// Axis-aligned bounding box carved to AIR.
#[derive(Clone, Copy, Debug)]
pub struct AirOpening {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

pub struct DungeonLayout {
    pub brick: BlockId,
    pub bounds_min: [i32; 3],
    pub bounds_max: [i32; 3],
    pub boxes: Vec<DungeonBox>,
    pub openings: Vec<AirOpening>,
    pub spikes: Vec<[i32; 3]>,
    pub chests: Vec<[i32; 3]>,
}

impl DungeonLayout {
    pub fn new(seed: u32, size: WorldSize) -> Self {
        let side = dungeon_side(seed, size);
        let pos = dungeon_entrance_pos(seed, size);
        let ex = pos[0] as i32;
        let ey = pos[1] as i32 - 1; // ground level
        let ez = pos[2] as i32;
        let brick = dungeon_brick(seed);

        let mut boxes = Vec::new();
        let mut openings = Vec::new();
        let mut spikes = Vec::new();
        let mut chests = Vec::new();

        // 1. Entrance Building on surface
        boxes.push(DungeonBox {
            min: [ex - 8, ey, ez - 6],
            max: [ex + 8, ey + 8, ez + 6],
            wall_thick: 1,
        });

        // Entrance doorway opening on inner side facing world center (towards x = 0)
        let (door_x0, door_x1) = if side < 0 {
            (ex + 7, ex + 9)
        } else {
            (ex - 9, ex - 7)
        };
        openings.push(AirOpening {
            min: [door_x0, ey + 1, ez - 2],
            max: [door_x1, ey + 4, ez + 2],
        });

        // Clear air in front of door so entrance is accessible
        let (front_x0, front_x1) = if side < 0 {
            (ex + 9, ex + 13)
        } else {
            (ex - 13, ex - 9)
        };
        openings.push(AirOpening {
            min: [front_x0, ey + 1, ez - 3],
            max: [front_x1, ey + 6, ez + 3],
        });

        // 2. Descending Staircase from inside entrance building down underground
        for step in 0..28 {
            let sx = ex - side * step;
            let sy = ey - step;
            boxes.push(DungeonBox {
                min: [sx - 2, sy - 1, ez - 2],
                max: [sx + 2, sy + 3, ez + 2],
                wall_thick: 1,
            });
            // Carve inner stair opening to connect steps
            openings.push(AirOpening {
                min: [sx - 1, sy, ez - 1],
                max: [sx + 1, sy + 2, ez + 1],
            });
        }

        // Helper to add a corridor between two centers
        let add_corridor = |p1: [i32; 3],
                            p2: [i32; 3],
                            boxes: &mut Vec<DungeonBox>,
                            openings: &mut Vec<AirOpening>| {
            let (min_x, max_x) = if p1[0] <= p2[0] {
                (p1[0], p2[0])
            } else {
                (p2[0], p1[0])
            };
            let (min_y, max_y) = if p1[1] <= p2[1] {
                (p1[1], p2[1])
            } else {
                (p2[1], p1[1])
            };
            let (min_z, max_z) = if p1[2] <= p2[2] {
                (p1[2], p2[2])
            } else {
                (p2[2], p1[2])
            };

            // Segment along X
            boxes.push(DungeonBox {
                min: [min_x - 1, p1[1] - 1, p1[2] - 2],
                max: [max_x + 1, p1[1] + 3, p1[2] + 2],
                wall_thick: 1,
            });
            openings.push(AirOpening {
                min: [min_x, p1[1], p1[2] - 1],
                max: [max_x, p1[1] + 2, p1[2] + 1],
            });

            // Segment along Y
            boxes.push(DungeonBox {
                min: [p2[0] - 2, min_y - 1, p1[2] - 2],
                max: [p2[0] + 2, max_y + 3, p1[2] + 2],
                wall_thick: 1,
            });
            openings.push(AirOpening {
                min: [p2[0] - 1, min_y, p1[2] - 1],
                max: [p2[0] + 1, max_y + 2, p1[2] + 1],
            });

            // Segment along Z
            boxes.push(DungeonBox {
                min: [p2[0] - 2, p2[1] - 1, min_z - 1],
                max: [p2[0] + 2, p2[1] + 3, max_z + 1],
                wall_thick: 1,
            });
            openings.push(AirOpening {
                min: [p2[0] - 1, p2[1], min_z],
                max: [p2[0] + 1, p2[1] + 2, max_z],
            });
        };

        // 3. Subterranean Rooms
        let min_room_y = crate::worldgen::underworld::UNDERWORLD_TOP_Y + 2;

        // Room 0: Landing Hall
        let mut r0 = [ex - side * 34, ey - 30, ez];
        r0[1] = r0[1].max(min_room_y + 8);
        boxes.push(DungeonBox {
            min: [r0[0] - 6, r0[1], r0[2] - 5],
            max: [r0[0] + 6, r0[1] + 7, r0[2] + 5],
            wall_thick: 1,
        });
        // Connect staircase to Room 0
        let stair_end = [ex - side * 27, ey - 27, ez];
        add_corridor(stair_end, r0, &mut boxes, &mut openings);

        // Room 1: Spike Chamber
        let mut r1 = [r0[0] - side * 16, r0[1] - 4, r0[2] + 8];
        r1[1] = r1[1].max(min_room_y + 6);
        boxes.push(DungeonBox {
            min: [r1[0] - 7, r1[1], r1[2] - 6],
            max: [r1[0] + 7, r1[1] + 8, r1[2] + 6],
            wall_thick: 1,
        });
        add_corridor(r0, r1, &mut boxes, &mut openings);
        // Spikes on floor
        for dx in -2..=2 {
            spikes.push([r1[0] + dx, r1[1] + 1, r1[2]]);
        }

        // Room 2: Treasure Hall
        let mut r2 = [r1[0] - side * 18, r1[1] - 6, r1[2] - 10];
        r2[1] = r2[1].max(min_room_y + 4);
        boxes.push(DungeonBox {
            min: [r2[0] - 7, r2[1], r2[2] - 6],
            max: [r2[0] + 7, r2[1] + 8, r2[2] + 6],
            wall_thick: 1,
        });
        add_corridor(r1, r2, &mut boxes, &mut openings);
        // Placed Chest
        chests.push([r2[0], r2[1] + 1, r2[2]]);

        // Room 3: Lower Spiked Crypt
        let mut r3 = [r2[0] + side * 10, r2[1] - 8, r2[2] - 12];
        r3[1] = r3[1].max(min_room_y + 2);
        boxes.push(DungeonBox {
            min: [r3[0] - 6, r3[1], r3[2] - 5],
            max: [r3[0] + 6, r3[1] + 7, r3[2] + 5],
            wall_thick: 1,
        });
        add_corridor(r2, r3, &mut boxes, &mut openings);
        // Spikes and chest in Room 3
        for dz in -1..=1 {
            spikes.push([r3[0] - 2, r3[1] + 1, r3[2] + dz]);
        }
        chests.push([r3[0] + 3, r3[1] + 1, r3[2]]);

        // Room 4: Deep Cavern Vault
        let mut r4 = [r3[0] - side * 16, r3[1] - 6, r3[2] + 6];
        r4[1] = r4[1].max(min_room_y);
        boxes.push(DungeonBox {
            min: [r4[0] - 8, r4[1], r4[2] - 7],
            max: [r4[0] + 8, r4[1] + 8, r4[2] + 7],
            wall_thick: 1,
        });
        add_corridor(r3, r4, &mut boxes, &mut openings);
        for dx in -2..=1 {
            spikes.push([r4[0] + dx, r4[1] + 1, r4[2] - 2]);
        }
        chests.push([r4[0] + 2, r4[1] + 1, r4[2] + 2]);

        // Compute overall bounds
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for b in &boxes {
            for a in 0..3 {
                min[a] = min[a].min(b.min[a]);
                max[a] = max[a].max(b.max[a]);
            }
        }
        for a in 0..3 {
            min[a] -= 4;
            max[a] += 4;
        }

        Self {
            brick,
            bounds_min: min,
            bounds_max: max,
            boxes,
            openings,
            spikes,
            chests,
        }
    }
}

/// True if a point falls within any part of the dungeon structure.
pub fn is_dungeon_point(seed: u32, x: i32, y: i32, z: i32, size: WorldSize) -> bool {
    let layout = DungeonLayout::new(seed, size);
    if x < layout.bounds_min[0]
        || x > layout.bounds_max[0]
        || y < layout.bounds_min[1]
        || y > layout.bounds_max[1]
        || z < layout.bounds_min[2]
        || z > layout.bounds_max[2]
    {
        return false;
    }
    layout.boxes.iter().any(|b| b.contains(x, y, z))
}

pub fn apply(seed: u32, pos: ChunkPos, chunk: &mut Chunk) {
    apply_for_size(seed, pos, WorldSize::Small, chunk);
}

pub fn apply_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let cs = CHUNK_SIZE;
    let bx = pos.x * cs;
    let by = pos.y * cs;
    let bz = pos.z * cs;

    let layout = DungeonLayout::new(seed, size);
    if bx + cs <= layout.bounds_min[0]
        || bx > layout.bounds_max[0]
        || by + cs <= layout.bounds_min[1]
        || by > layout.bounds_max[1]
        || bz + cs <= layout.bounds_min[2]
        || bz > layout.bounds_max[2]
    {
        return;
    }

    // 1. Carve boxes: outer brick, inner air
    for b in &layout.boxes {
        let x0 = b.min[0].max(bx);
        let x1 = b.max[0].min(bx + cs - 1);
        let y0 = b.min[1].max(by);
        let y1 = b.max[1].min(by + cs - 1);
        let z0 = b.min[2].max(bz);
        let z1 = b.max[2].min(bz + cs - 1);

        for x in x0..=x1 {
            let lx = (x - bx) as usize;
            for z in z0..=z1 {
                let lz = (z - bz) as usize;
                for y in y0..=y1 {
                    let ly = (y - by) as usize;
                    let is_wall = x < b.min[0] + b.wall_thick
                        || x > b.max[0] - b.wall_thick
                        || y < b.min[1] + b.wall_thick
                        || y > b.max[1] - b.wall_thick
                        || z < b.min[2] + b.wall_thick
                        || z > b.max[2] - b.wall_thick;

                    if is_wall {
                        chunk.set(lx, ly, lz, layout.brick);
                    } else {
                        chunk.set(lx, ly, lz, AIR);
                    }
                }
            }
        }
    }

    // 2. Openings / doorways (carve AIR)
    for op in &layout.openings {
        let x0 = op.min[0].max(bx);
        let x1 = op.max[0].min(bx + cs - 1);
        let y0 = op.min[1].max(by);
        let y1 = op.max[1].min(by + cs - 1);
        let z0 = op.min[2].max(bz);
        let z1 = op.max[2].min(bz + cs - 1);

        for x in x0..=x1 {
            let lx = (x - bx) as usize;
            for z in z0..=z1 {
                let lz = (z - bz) as usize;
                for y in y0..=y1 {
                    let ly = (y - by) as usize;
                    chunk.set(lx, ly, lz, AIR);
                }
            }
        }
    }

    // 3. Spikes
    for &[sx, sy, sz] in &layout.spikes {
        if sx >= bx && sx < bx + cs && sy >= by && sy < by + cs && sz >= bz && sz < bz + cs {
            chunk.set(
                (sx - bx) as usize,
                (sy - by) as usize,
                (sz - bz) as usize,
                SPIKES,
            );
        }
    }

    // 4. Chests
    for &[cx, cy, cz] in &layout.chests {
        if cx >= bx && cx < bx + cs && cy >= by && cy < by + cs && cz >= bz && cz < bz + cs {
            chunk.set(
                (cx - bx) as usize,
                (cy - by) as usize,
                (cz - bz) as usize,
                CHEST,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::generate_chunk_sized;
    use crate::worldgen::terrain::surface_height_for_size;

    #[test]
    fn dungeon_stays_above_underworld() {
        let layout = DungeonLayout::new(42, WorldSize::Small);
        for b in &layout.boxes {
            assert!(
                b.min[1] > crate::worldgen::underworld::UNDERWORLD_TOP_Y,
                "dungeon box min Y {} must be > UNDERWORLD_TOP_Y ({})",
                b.min[1],
                crate::worldgen::underworld::UNDERWORLD_TOP_Y
            );
        }
    }

    #[test]
    fn dungeon_generates_at_world_edge() {
        let seed = 42;
        let size = WorldSize::Small;
        let pos = dungeon_entrance_pos(seed, size);
        let half = size.half_extent() as f32;
        assert!(
            pos[0].abs() >= half * 0.5,
            "entrance x {} should be near world edge (>= {})",
            pos[0],
            half * 0.5
        );
        assert!(
            pos[0].abs() <= half * 0.85,
            "entrance x {} should be within inland bounds (<= {})",
            pos[0],
            half * 0.85
        );
        let surf = surface_height_for_size(seed, pos[0] as i32, pos[2] as i32, size);
        assert!(
            (pos[1] - surf as f32).abs() <= 5.0,
            "entrance y {} near surface {}",
            pos[1],
            surf
        );
    }

    #[test]
    fn dungeon_contains_bricks_spikes_and_chests() {
        let seed = 42;
        let size = WorldSize::Small;
        let pos = dungeon_entrance_pos(seed, size);
        let chunk_x = (pos[0] as i32).div_euclid(CHUNK_SIZE);
        let chunk_z = (pos[2] as i32).div_euclid(CHUNK_SIZE);

        let mut found_brick = false;
        let mut found_spikes = false;
        let mut found_chest = false;

        for dy in -3..=1 {
            for dx in -3..=3 {
                for dz in -3..=3 {
                    let cpos = ChunkPos {
                        x: chunk_x + dx,
                        y: (pos[1] as i32 / CHUNK_SIZE) + dy,
                        z: chunk_z + dz,
                    };
                    let chunk = generate_chunk_sized(seed, cpos, size);
                    for lz in 0..CHUNK_SIZE as usize {
                        for ly in 0..CHUNK_SIZE as usize {
                            for lx in 0..CHUNK_SIZE as usize {
                                let b = chunk.get(lx, ly, lz);
                                if b == BLUE_DUNGEON_BRICK
                                    || b == GREEN_DUNGEON_BRICK
                                    || b == PINK_DUNGEON_BRICK
                                {
                                    found_brick = true;
                                }
                                if b == SPIKES {
                                    found_spikes = true;
                                }
                                if b == CHEST {
                                    found_chest = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        assert!(found_brick, "dungeon should contain dungeon bricks");
        assert!(found_spikes, "dungeon should contain spikes");
        assert!(found_chest, "dungeon should contain chests");
    }

    #[test]
    fn is_dungeon_point_identifies_dungeon_blocks() {
        let seed = 42;
        let size = WorldSize::Small;
        let pos = dungeon_entrance_pos(seed, size);
        let ex = pos[0] as i32;
        let ey = pos[1] as i32 - 1;
        let ez = pos[2] as i32;

        // Inside entrance building
        assert!(is_dungeon_point(seed, ex, ey + 2, ez, size));
        // Outside entrance building
        assert!(!is_dungeon_point(seed, 0, ey, 0, size));
        assert!(!is_dungeon_point(seed, ex + 100, ey, ez, size));
    }

    #[test]
    fn dungeon_is_placed_opposite_jungle() {
        for seed in [1, 42, 100, 999] {
            let size = WorldSize::Small;
            let entrance = dungeon_entrance_pos(seed, size);
            let j_side = jungle_side(seed, size);
            assert_ne!(j_side, 0, "jungle_side must be -1 or +1");
            if j_side > 0 {
                assert!(
                    entrance[0] < 0.0,
                    "seed {seed}: jungle side +X but dungeon entrance at {}",
                    entrance[0]
                );
            } else {
                assert!(
                    entrance[0] > 0.0,
                    "seed {seed}: jungle side -X but dungeon entrance at {}",
                    entrance[0]
                );
            }
        }
    }
}
