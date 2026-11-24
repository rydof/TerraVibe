//! Deterministic fixture world generation for QA scenarios.

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;

use terravibe_server::save::{PlayerSave, SaveDir, TownSave, WorldSnapshot};
use terravibe_server::worlds::create_world;
use terravibe_sim::block::*;
use terravibe_sim::chest::ChestInv;
use terravibe_sim::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use terravibe_sim::inventory::{Inventory, ItemStack};
use terravibe_sim::item::*;
use terravibe_sim::liquid::AMOUNT_FULL;
use terravibe_sim::liquid_cell::Liquid;
use terravibe_sim::town_npc::TownKind;

use crate::qa::{FIXTURE_SEED, FixtureKind};

fn set_block_in_map(chunks: &mut HashMap<ChunkPos, Chunk>, x: i32, y: i32, z: i32, block: BlockId) {
    let cp = ChunkPos::of_block(x, y, z);
    if let Some(c) = chunks.get_mut(&cp) {
        let lx = x.rem_euclid(CHUNK_SIZE) as usize;
        let ly = y.rem_euclid(CHUNK_SIZE) as usize;
        let lz = z.rem_euclid(CHUNK_SIZE) as usize;
        c.set(lx, ly, lz, block);
    }
}

fn set_wall_in_map(chunks: &mut HashMap<ChunkPos, Chunk>, x: i32, y: i32, z: i32, wall: bool) {
    let cp = ChunkPos::of_block(x, y, z);
    if let Some(c) = chunks.get_mut(&cp) {
        let lx = x.rem_euclid(CHUNK_SIZE) as usize;
        let ly = y.rem_euclid(CHUNK_SIZE) as usize;
        let lz = z.rem_euclid(CHUNK_SIZE) as usize;
        c.set_wall(lx, ly, lz, wall);
    }
}

fn set_liquid_in_map(
    chunks: &mut HashMap<ChunkPos, Chunk>,
    x: i32,
    y: i32,
    z: i32,
    liq: Option<Liquid>,
) {
    let cp = ChunkPos::of_block(x, y, z);
    if let Some(c) = chunks.get_mut(&cp) {
        let lx = x.rem_euclid(CHUNK_SIZE) as usize;
        let ly = y.rem_euclid(CHUNK_SIZE) as usize;
        let lz = z.rem_euclid(CHUNK_SIZE) as usize;
        c.set_liquid(lx, ly, lz, liq);
    }
}

/// Builds a deterministic world fixture at `<root>/<world>`.
///
/// If `<root>/<world>` already exists, skips writing (idempotent for phase 2).
pub fn build_fixture(kind: FixtureKind, root: &Path, world: &str) -> io::Result<()> {
    let world_dir = root.join(world);
    if world_dir.exists() {
        return Ok(());
    }

    std::fs::create_dir_all(root)?;
    create_world(root, world, FIXTURE_SEED, terravibe_sim::WorldSize::Small)?;
    let save_dir = SaveDir::open(root, world)?;

    let [sx, sy, sz] = terravibe_server::spawn_point(FIXTURE_SEED);
    let spawn_x = sx.floor() as i32;
    let spawn_z = sz.floor() as i32;
    let ground_y = (sy - 1.0).round() as i32;
    let ground_cy = ground_y.div_euclid(CHUNK_SIZE);
    let ground_ly = ground_y.rem_euclid(CHUNK_SIZE) as usize;

    let mut chunks = HashMap::new();

    // Generate a 3x3 column of chunks around spawn
    for cx in -1..=1 {
        for cz in -1..=1 {
            for cy in (ground_cy - 1)..=(ground_cy + 1) {
                let pos = ChunkPos {
                    x: cx,
                    y: cy,
                    z: cz,
                };
                let chunk = if cy < ground_cy {
                    Chunk::filled(
                        if matches!(kind, FixtureKind::Corruption | FixtureKind::Boss) {
                            EBONSTONE
                        } else {
                            STONE
                        },
                    )
                } else if cy == ground_cy {
                    let mut c = Chunk::filled(AIR);
                    let (surface_mat, deep_mat) = match kind {
                        FixtureKind::Corruption | FixtureKind::Boss => (CORRUPT_GRASS, EBONSTONE),
                        _ => (GRASS, DIRT),
                    };
                    for lx in 0..32 {
                        for lz in 0..32 {
                            for ly in 0..ground_ly {
                                c.set(lx, ly, lz, deep_mat);
                            }
                            c.set(lx, ground_ly, lz, surface_mat);
                        }
                    }
                    c
                } else {
                    Chunk::filled(AIR)
                };
                chunks.insert(pos, chunk);
            }
        }
    }

    let mut chests = Vec::new();
    let mut towns = TownSave::default();
    let mut inv = Inventory::starting(&mut Rng::new(FIXTURE_SEED as u64));

    match kind {
        FixtureKind::Flat => {
            // Level ground + workbench + materials
            set_block_in_map(&mut chunks, spawn_x + 1, ground_y + 1, spawn_z, WORKBENCH);
            set_block_in_map(&mut chunks, spawn_x + 2, ground_y + 1, spawn_z, CHEST);

            let mut cinv = ChestInv::empty();
            cinv.slots[0] = Some(ItemStack {
                id: WOOD,
                count: 99,
                prefix: Prefix(0),
            });
            cinv.slots[1] = Some(ItemStack {
                id: STONE_BLOCK,
                count: 99,
                prefix: Prefix(0),
            });
            cinv.slots[2] = Some(ItemStack {
                id: IRON_BAR,
                count: 50,
                prefix: Prefix(0),
            });
            cinv.slots[3] = Some(ItemStack {
                id: COPPER_BAR,
                count: 50,
                prefix: Prefix(0),
            });
            cinv.slots[4] = Some(ItemStack {
                id: GEL,
                count: 50,
                prefix: Prefix(0),
            });
            cinv.slots[5] = Some(ItemStack {
                id: FURNACE_ITEM,
                count: 1,
                prefix: Prefix(0),
            });
            cinv.slots[6] = Some(ItemStack {
                id: ANVIL_ITEM,
                count: 1,
                prefix: Prefix(0),
            });
            chests.push(([spawn_x + 2, ground_y + 1, spawn_z], cinv));

            inv.slots[4] = Some(ItemStack {
                id: WOOD,
                count: 50,
                prefix: Prefix(0),
            });
            inv.slots[5] = Some(ItemStack {
                id: STONE_BLOCK,
                count: 50,
                prefix: Prefix(0),
            });
            inv.slots[6] = Some(ItemStack {
                id: terravibe_sim::item::TORCH,
                count: 20,
                prefix: Prefix(0),
            });
        }
        FixtureKind::Combat => {
            // Flat + weapons
            set_block_in_map(&mut chunks, spawn_x + 2, ground_y + 1, spawn_z, CHEST);
            let mut cinv = ChestInv::empty();
            cinv.slots[0] = Some(ItemStack {
                id: WOODEN_BOW,
                count: 1,
                prefix: Prefix(0),
            });
            cinv.slots[1] = Some(ItemStack {
                id: WOODEN_ARROW,
                count: 250,
                prefix: Prefix(0),
            });
            cinv.slots[2] = Some(ItemStack {
                id: WOODEN_SWORD,
                count: 1,
                prefix: Prefix(0),
            });
            cinv.slots[3] = Some(ItemStack {
                id: LESSER_HEALING_POTION,
                count: 10,
                prefix: Prefix(0),
            });
            chests.push(([spawn_x + 2, ground_y + 1, spawn_z], cinv));

            inv.slots[1] = Some(ItemStack {
                id: WOODEN_SWORD,
                count: 1,
                prefix: Prefix(0),
            });
            inv.slots[2] = Some(ItemStack {
                id: WOODEN_BOW,
                count: 1,
                prefix: Prefix(0),
            });
            inv.slots[3] = Some(ItemStack {
                id: WOODEN_ARROW,
                count: 100,
                prefix: Prefix(0),
            });
        }
        FixtureKind::Corruption => {
            // Chasm + DEMON_ALTAR reachable
            for dx in -2..=2 {
                for dz in -2..=2 {
                    for dy in -8..=0 {
                        set_block_in_map(
                            &mut chunks,
                            spawn_x + dx,
                            ground_y + dy,
                            spawn_z + 4 + dz,
                            AIR,
                        );
                    }
                    set_block_in_map(
                        &mut chunks,
                        spawn_x + dx,
                        ground_y - 9,
                        spawn_z + 4 + dz,
                        EBONSTONE,
                    );
                }
            }
            set_block_in_map(&mut chunks, spawn_x, ground_y - 8, spawn_z + 4, DEMON_ALTAR);
        }
        FixtureKind::Boss => {
            // Corruption + altar + Worm Food ingredients & eye
            for dx in -2..=2 {
                for dz in -2..=2 {
                    for dy in -8..=0 {
                        set_block_in_map(
                            &mut chunks,
                            spawn_x + dx,
                            ground_y + dy,
                            spawn_z + 4 + dz,
                            AIR,
                        );
                    }
                    set_block_in_map(
                        &mut chunks,
                        spawn_x + dx,
                        ground_y - 9,
                        spawn_z + 4 + dz,
                        EBONSTONE,
                    );
                }
            }
            set_block_in_map(&mut chunks, spawn_x, ground_y - 8, spawn_z + 4, DEMON_ALTAR);

            set_block_in_map(&mut chunks, spawn_x + 1, ground_y + 1, spawn_z, CHEST);
            let mut cinv = ChestInv::empty();
            cinv.slots[0] = Some(ItemStack {
                id: LENS,
                count: 12,
                prefix: Prefix(0),
            });
            cinv.slots[1] = Some(ItemStack {
                id: SUSPICIOUS_LOOKING_EYE,
                count: 2,
                prefix: Prefix(0),
            });
            cinv.slots[2] = Some(ItemStack {
                id: WORM_FOOD,
                count: 5,
                prefix: Prefix(0),
            });
            cinv.slots[3] = Some(ItemStack {
                id: WOODEN_BOW,
                count: 1,
                prefix: Prefix(0),
            });
            cinv.slots[4] = Some(ItemStack {
                id: WOODEN_ARROW,
                count: 200,
                prefix: Prefix(0),
            });
            chests.push(([spawn_x + 1, ground_y + 1, spawn_z], cinv));

            inv.slots[1] = Some(ItemStack {
                id: SUSPICIOUS_LOOKING_EYE,
                count: 2,
                prefix: Prefix(0),
            });
            inv.slots[2] = Some(ItemStack {
                id: WORM_FOOD,
                count: 5,
                prefix: Prefix(0),
            });
            inv.slots[3] = Some(ItemStack {
                id: LENS,
                count: 12,
                prefix: Prefix(0),
            });
        }
        FixtureKind::Housing => {
            // Flat + a valid room shell + Guide
            let o = [spawn_x + 3, ground_y + 1, spawn_z];
            for dx in -1..=5 {
                for dy in -1..=4 {
                    for dz in -1..=4 {
                        let p = [o[0] + dx, o[1] + dy, o[2] + dz];
                        let inside =
                            (0..5).contains(&dx) && (0..4).contains(&dy) && (0..4).contains(&dz);
                        if inside {
                            set_block_in_map(&mut chunks, p[0], p[1], p[2], AIR);
                            set_wall_in_map(&mut chunks, p[0], p[1], p[2], true);
                        } else {
                            set_block_in_map(&mut chunks, p[0], p[1], p[2], WOOD_BLOCK);
                        }
                    }
                }
            }
            set_block_in_map(&mut chunks, o[0] - 1, o[1], o[2], DOOR_CLOSED);
            set_block_in_map(&mut chunks, o[0] + 1, o[1], o[2], CHAIR);
            set_block_in_map(&mut chunks, o[0] + 2, o[1], o[2], TABLE);
            set_block_in_map(
                &mut chunks,
                o[0] + 3,
                o[1],
                o[2],
                terravibe_sim::block::TORCH,
            );

            towns.guide_spawned = true;
            towns
                .npcs
                .push((TownKind::Guide, [o[0] + 1, o[1], o[2]], o));
        }
        FixtureKind::Liquids => {
            // Prepared liquid environment:
            // 1. Water pond in front of spawn (facing -Z: spawn_z-7..=spawn_z-2, spawn_x-2..=spawn_x+2)
            // Carved 3 blocks deep so player head is fully submerged upon entry.
            for dx in -2..=2 {
                for dz in -7..=-2 {
                    for dy in -2..=0 {
                        set_block_in_map(
                            &mut chunks,
                            spawn_x + dx,
                            ground_y + dy,
                            spawn_z + dz,
                            AIR,
                        );
                        set_liquid_in_map(
                            &mut chunks,
                            spawn_x + dx,
                            ground_y + dy,
                            spawn_z + dz,
                            Some(Liquid::water(AMOUNT_FULL)),
                        );
                    }
                }
            }
            // 2. Glowing lava pool to the right (spawn_x+4..=spawn_x+7, spawn_z-4..=spawn_z)
            for dx in 4..=7 {
                for dz in -4..=0 {
                    for dy in -1..=0 {
                        set_block_in_map(
                            &mut chunks,
                            spawn_x + dx,
                            ground_y + dy,
                            spawn_z + dz,
                            AIR,
                        );
                        set_liquid_in_map(
                            &mut chunks,
                            spawn_x + dx,
                            ground_y + dy,
                            spawn_z + dz,
                            Some(Liquid::lava(AMOUNT_FULL)),
                        );
                    }
                }
            }
            // 3. Sealed water basin with a single stone/dirt plug on the left:
            // Interior at spawn_x-4, spawn_z-3; plug at spawn_x-3, ground_y+1, spawn_z-3.
            for dx in -5..=-3 {
                for dz in -4..=-2 {
                    for dy in 0..=2 {
                        let is_interior = dx == -4 && dz == -3 && (dy == 0 || dy == 1);
                        let is_plug = dx == -3 && dz == -3 && dy == 0;
                        if is_interior {
                            set_block_in_map(
                                &mut chunks,
                                spawn_x + dx,
                                ground_y + 1 + dy,
                                spawn_z + dz,
                                AIR,
                            );
                            set_liquid_in_map(
                                &mut chunks,
                                spawn_x + dx,
                                ground_y + 1 + dy,
                                spawn_z + dz,
                                Some(Liquid::water(AMOUNT_FULL)),
                            );
                        } else if is_plug {
                            set_block_in_map(
                                &mut chunks,
                                spawn_x + dx,
                                ground_y + 1 + dy,
                                spawn_z + dz,
                                DIRT,
                            );
                        } else {
                            set_block_in_map(
                                &mut chunks,
                                spawn_x + dx,
                                ground_y + 1 + dy,
                                spawn_z + dz,
                                STONE,
                            );
                        }
                    }
                }
            }

            // Chest with extra buckets
            let mut cinv = ChestInv::empty();
            cinv.slots[0] = Some(ItemStack {
                id: EMPTY_BUCKET,
                count: 5,
                prefix: Prefix(0),
            });
            cinv.slots[1] = Some(ItemStack {
                id: WATER_BUCKET,
                count: 5,
                prefix: Prefix(0),
            });
            cinv.slots[2] = Some(ItemStack {
                id: LAVA_BUCKET,
                count: 5,
                prefix: Prefix(0),
            });
            chests.push(([spawn_x + 1, ground_y + 1, spawn_z], cinv));

            // Starting inventory
            inv.slots[0] = Some(ItemStack {
                id: IRON_PICKAXE,
                count: 1,
                prefix: Prefix(0),
            });
            inv.slots[1] = Some(ItemStack {
                id: EMPTY_BUCKET,
                count: 5,
                prefix: Prefix(0),
            });
            inv.slots[2] = Some(ItemStack {
                id: WATER_BUCKET,
                count: 1,
                prefix: Prefix(0),
            });
            inv.slots[3] = Some(ItemStack {
                id: LAVA_BUCKET,
                count: 1,
                prefix: Prefix(0),
            });
        }
    }

    let (armor, life_max) = if kind == FixtureKind::Combat {
        (
            [Some(IRON_HELMET), Some(IRON_CHAINMAIL), Some(IRON_GREAVES)],
            200,
        )
    } else {
        ([None, None, None], 100)
    };

    let player_save = PlayerSave {
        inventory: inv,
        armor,
        life_max,
        life: life_max,
        pos: [sx, sy, sz],
        potion_delay: 0,
    };

    let mut chunk_vec: Vec<(ChunkPos, Arc<Chunk>)> = chunks
        .into_iter()
        .map(|(pos, chunk)| (pos, Arc::new(chunk)))
        .collect();
    chunk_vec.sort_by_key(|(pos, _)| (pos.x, pos.y, pos.z));

    let snap = WorldSnapshot {
        seed: FIXTURE_SEED,
        // The Eye of Cthulhu only answers the Suspicious Looking Eye at night.
        clock_tick: if kind == FixtureKind::Boss {
            terravibe_sim::time::DAY_TICKS
        } else {
            13_500
        },
        size: terravibe_sim::WorldSize::Small,
        chunks: chunk_vec,
        players: vec![("player".to_string(), player_save)],
        chests,
        towns,
        facts: terravibe_sim::WorldFacts::default(),
        weather: Default::default(),
        locked_chests: vec![],
        shadow_chests: vec![],
    };

    save_dir.save_world(&snap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use terravibe_server::save::SaveDir;
    use terravibe_sim::block::{AIR, GRASS};
    use terravibe_sim::chunk::{CHUNK_SIZE, ChunkPos};

    struct TempDirGuard(PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_dir(tag: &str) -> (PathBuf, TempDirGuard) {
        let p = std::env::temp_dir().join(format!("tv_fix_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("create temp dir");
        (p.clone(), TempDirGuard(p))
    }

    #[test]
    fn fixture_flat_world_is_flat() {
        let (root, _guard) = test_dir("flat");
        build_fixture(FixtureKind::Flat, &root, "flat_world").expect("build flat fixture");

        let save_dir = SaveDir::open(&root, "flat_world").expect("open save dir");
        let snap = save_dir
            .load_world()
            .expect("load world")
            .expect("snapshot exists");

        // Find the spawn column chunk containing the surface
        let [sx, sy, sz] = terravibe_server::spawn_point(crate::qa::FIXTURE_SEED);
        let ground_y = (sy - 1.0).round() as i32;
        let ground_cy = ground_y.div_euclid(CHUNK_SIZE);
        let ground_ly = ground_y.rem_euclid(CHUNK_SIZE) as usize;

        let spawn_chunk_pos = ChunkPos {
            x: (sx.floor() as i32).div_euclid(CHUNK_SIZE),
            y: ground_cy,
            z: (sz.floor() as i32).div_euclid(CHUNK_SIZE),
        };

        let chunk = snap
            .chunks
            .iter()
            .find(|(pos, _)| *pos == spawn_chunk_pos)
            .map(|(_, c)| c)
            .expect("spawn surface chunk must be in snapshot");

        // Surface layer must be level across all columns in the spawn chunk
        for lx in 0..32 {
            for lz in 0..32 {
                assert_eq!(
                    chunk.get(lx, ground_ly, lz),
                    GRASS,
                    "surface at ({lx}, {ground_ly}, {lz}) must be GRASS"
                );
                // Above the ground layer must be air (or workbench at specific position)
                if ground_ly + 1 < 32 {
                    let above = chunk.get(lx, ground_ly + 1, lz);
                    assert!(
                        above == AIR
                            || above == terravibe_sim::block::WORKBENCH
                            || above == terravibe_sim::block::CHEST,
                        "above surface must be AIR, WORKBENCH, or CHEST, got {:?}",
                        above
                    );
                }
            }
        }
    }

    #[test]
    fn fixture_build_is_idempotent() {
        let (root, _guard) = test_dir("idempotent");
        build_fixture(FixtureKind::Flat, &root, "idem_world").expect("first build");

        let wb_path = root.join("idem_world/world.bin");
        let mtime_before = wb_path.metadata().expect("meta").modified().expect("mtime");

        // Small sleep to ensure mtime would differ if touched
        std::thread::sleep(std::time::Duration::from_millis(10));

        build_fixture(FixtureKind::Flat, &root, "idem_world").expect("second build (noop)");
        let mtime_after = wb_path.metadata().expect("meta").modified().expect("mtime");

        assert_eq!(
            mtime_before, mtime_after,
            "idempotent second build must not touch world.bin"
        );
    }

    #[test]
    fn fixture_kinds_differ_deterministically() {
        let (root_a, _guard_a) = test_dir("det_a");
        let (root_b, _guard_b) = test_dir("det_b");

        build_fixture(FixtureKind::Corruption, &root_a, "corrupt").expect("build corruption a");
        build_fixture(FixtureKind::Corruption, &root_b, "corrupt").expect("build corruption b");

        // Read all chunks from both builds
        let chunks_a_dir = root_a.join("corrupt/chunks");
        let chunks_b_dir = root_b.join("corrupt/chunks");

        let mut files_a: Vec<PathBuf> = std::fs::read_dir(&chunks_a_dir)
            .expect("read chunks a")
            .flatten()
            .map(|e| e.path())
            .collect();
        files_a.sort();

        let mut files_b: Vec<PathBuf> = std::fs::read_dir(&chunks_b_dir)
            .expect("read chunks b")
            .flatten()
            .map(|e| e.path())
            .collect();
        files_b.sort();

        assert!(
            !files_a.is_empty(),
            "corruption fixture must produce chunks"
        );
        assert_eq!(
            files_a.len(),
            files_b.len(),
            "both corruption builds must produce same number of chunk files"
        );

        for (fa, fb) in files_a.iter().zip(files_b.iter()) {
            assert_eq!(
                fa.file_name(),
                fb.file_name(),
                "chunk file names must match"
            );
            let bytes_a = std::fs::read(fa).expect("read fa");
            let bytes_b = std::fs::read(fb).expect("read fb");
            assert_eq!(
                bytes_a,
                bytes_b,
                "chunk file {:?} must be byte-identical across both builds",
                fa.file_name()
            );
        }
    }

    #[test]
    fn combat_fixture_bridges_to_server_demo_enemies() {
        use terravibe_sim::net::{ClientTransport, ServerMsg};

        let (root, _guard) = test_dir("combat_bridge");
        build_fixture(FixtureKind::Combat, &root, "qa-combat").expect("build combat fixture");

        // The save format (WorldSnapshot) deterministically persists terrain chunks,
        // chests, and player inventory with weapons (WOODEN_BOW, WOODEN_ARROW, WOODEN_SWORD).
        // Enemies are dynamic server simulation entities instantiated via ServerCfg::demo_enemies.
        let cfg = terravibe_server::ServerCfg {
            demo_enemies: true,
            natural_spawns: false,
            save: Some(terravibe_server::SaveCfg {
                root: root.clone(),
                world: "qa-combat".to_string(),
            }),
            ..Default::default()
        };
        let mut server = terravibe_server::Server::with_cfg(crate::qa::FIXTURE_SEED, cfg);
        let (client, conn) = terravibe_sim::net::in_process_pair();
        server.connect(Box::new(conn));
        client
            .send(terravibe_sim::net::ClientMsg::Hello {
                seed: crate::qa::FIXTURE_SEED,
                name: Some("player".to_string()),
                character: None,
            })
            .unwrap();

        let mut spawned = Vec::new();
        for _ in 0..300 {
            server.tick();
            std::thread::sleep(std::time::Duration::from_millis(5));
            while let Ok(Some(msg)) = client.try_recv() {
                if let ServerMsg::EnemySpawn { kind, .. } = msg {
                    spawned.push(kind);
                }
            }
            if spawned.len() >= 3 {
                break;
            }
        }

        assert_eq!(
            spawned.len(),
            3,
            "demo enemies near spawn must emit 3 EnemySpawn messages, got {:?}",
            spawned
        );
        assert_eq!(spawned[0], terravibe_sim::enemy::EnemyKind::Zombie);
        assert_eq!(spawned[1], terravibe_sim::enemy::EnemyKind::DemonEye);
        assert_eq!(spawned[2], terravibe_sim::enemy::EnemyKind::Slime);
    }

    #[test]
    fn fixture_boss_starts_at_night_so_the_eye_can_be_used() {
        let (root, _guard) = test_dir("boss_night");
        build_fixture(FixtureKind::Boss, &root, "boss_world").expect("build boss fixture");
        let snap = SaveDir::open(&root, "boss_world")
            .expect("open save dir")
            .load_world()
            .expect("load world")
            .expect("snapshot exists");
        assert!(snap.clock_tick >= terravibe_sim::time::DAY_TICKS);
    }

    #[test]
    fn combat_fixture_has_survivability_headroom() {
        let (root, _guard) = test_dir("combat_headroom");
        build_fixture(FixtureKind::Combat, &root, "combat_world").expect("build combat fixture");
        let snap = SaveDir::open(&root, "combat_world")
            .expect("open save dir")
            .load_world()
            .expect("load world")
            .expect("snapshot exists");
        let (_, player) = &snap.players[0];
        assert_eq!(player.life_max, 200);
        assert_eq!(player.life, 200);
        assert_eq!(
            player.armor,
            [Some(IRON_HELMET), Some(IRON_CHAINMAIL), Some(IRON_GREAVES)]
        );
    }
}
