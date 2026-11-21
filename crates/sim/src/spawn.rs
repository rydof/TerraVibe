//! Natural enemy spawner: a fixed-interval, seeded stand-in for NPC.SpawnNPC.

use crate::enemy::EnemyKind;
use crate::item::Rng;
use crate::time::WorldClock;

/// NPC.spawnRate = 600 (gameplay spec): one spawn attempt per 600 ticks. Terraria
/// rolls `rand.Next(rate) == 0` each tick; a fixed interval has the same mean.
pub const SPAWN_RATE: u32 = 600;
use crate::worldgen::underworld::UNDERWORLD_TOP_Y;

pub const UNDERWORLD_POOL: [EnemyKind; 12] = [
    EnemyKind::LavaSlime,
    EnemyKind::LavaSlime,
    EnemyKind::LavaSlime,
    EnemyKind::Hellbat,
    EnemyKind::Hellbat,
    EnemyKind::Hellbat,
    EnemyKind::FireImp,
    EnemyKind::FireImp,
    EnemyKind::Demon,
    EnemyKind::Demon,
    EnemyKind::VoodooDemon,
    EnemyKind::BoneSerpentHead,
];

/// NPC.defaultMaxSpawns = 5 (gameplay spec).
pub const DEFAULT_MAX_SPAWNS: f32 = 5.0;
/// Spawn ring around a player in blocks (gameplay spec: 62-84 tiles' worth).
pub const SPAWN_RING: (f32, f32) = (62.0, 84.0);
/// No spawn within 0.52 of a 90-block half-screen of any player (gameplay spec).
pub const SAFE_RADIUS: f32 = 0.52 * 90.0;
const CANDIDATES: u32 = 20;

/// Cap on live enemies: `floor(5 * (2 + 0.3 * players))` (gameplay spec).
pub fn max_spawns(players: usize) -> usize {
    (DEFAULT_MAX_SPAWNS * (2.0 + 0.3 * players as f32)).floor() as usize
}

/// 3D stage-6 rule: four Eaters per live player, in addition to the total cap.
pub fn eater_cap(players: usize) -> usize {
    4 * players
}

pub trait SpawnWorld {
    /// Y of the top solid block in column (x, z); `None` if any y-chunk of the
    /// column is missing or the column has no solid block.
    fn surface_y(&self, x: i32, z: i32) -> Option<i32>;
    /// Whether the candidate column belongs to the world's corruption biome.
    fn corrupt_at(&self, x: i32, z: i32) -> bool;
    /// Whether `(x, y, z)` and `(x, y + 1, z)` have 2-block vertical air clearance
    /// (non-solid and not submerged in liquid).
    fn has_clearance(&self, _x: i32, _y: i32, _z: i32) -> bool {
        true
    }
}

fn find_clearance_y(world: &impl SpawnWorld, x: i32, base_y: i32, z: i32) -> Option<i32> {
    if base_y >= 2 && world.has_clearance(x, base_y, z) {
        return Some(base_y);
    }
    for d in 1..=16 {
        for cand in [base_y - d, base_y + d] {
            if cand >= 2 && world.has_clearance(x, cand, z) {
                return Some(cand);
            }
        }
    }
    None
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spawner {
    pub ticks: u32,
}

fn unit(rng: &mut Rng) -> f32 {
    (rng.next_u32() >> 8) as f32 / (1u32 << 24) as f32
}

/// One 60 Hz step. Every `SPAWN_RATE` ticks, if under the cap, tries up to 20
/// ring candidates around a seeded player. The server assigns the `EnemyId`.
/// Day spawns slimes; night picks zombie or demon eye 50/50 (simplification).
/// Corrupt candidates take precedence at any hour, subject to a separate Eater cap.
pub fn tick_spawner(
    world: &impl SpawnWorld,
    clock: &WorldClock,
    players: &[[f32; 3]],
    existing: usize,
    existing_eaters: usize,
    spawner: &mut Spawner,
    rng: &mut Rng,
) -> Option<(EnemyKind, [f32; 3])> {
    if players.is_empty() {
        return None;
    }
    spawner.ticks += 1;
    if spawner.ticks < SPAWN_RATE {
        return None;
    }
    spawner.ticks = 0;
    if existing >= max_spawns(players.len()) {
        return None;
    }
    for _ in 0..CANDIDATES {
        let p = players[rng.below(players.len() as u32) as usize];
        let ang = unit(rng) * std::f32::consts::TAU;
        let r = SPAWN_RING.0 + unit(rng) * (SPAWN_RING.1 - SPAWN_RING.0);
        let (x, z) = (
            (p[0] + ang.cos() * r).floor() as i32,
            (p[2] + ang.sin() * r).floor() as i32,
        );
        if (p[1] as i32) <= UNDERWORLD_TOP_Y {
            let Some(clear_y) = find_clearance_y(world, x, p[1].floor() as i32, z) else {
                continue;
            };
            let pos = [x as f32 + 0.5, clear_y as f32, z as f32 + 0.5];
            let near = players
                .iter()
                .any(|q| (q[0] - pos[0]).hypot(q[2] - pos[2]) < SAFE_RADIUS);
            if near {
                continue;
            }
            let idx = rng.below(UNDERWORLD_POOL.len() as u32) as usize;
            let kind = UNDERWORLD_POOL[idx];
            return Some((kind, pos));
        }
        let Some(sy) = world.surface_y(x, z) else {
            continue;
        };
        let is_cave = (p[1] as i32) <= sy - 30;
        let pos = if is_cave {
            let base_y = (p[1].floor() as i32).min(sy - 30);
            let Some(clear_y) = find_clearance_y(world, x, base_y, z) else {
                continue;
            };
            [x as f32 + 0.5, clear_y as f32, z as f32 + 0.5]
        } else {
            [x as f32 + 0.5, (sy + 1) as f32, z as f32 + 0.5]
        };
        let near = players
            .iter()
            .any(|q| (q[0] - pos[0]).hypot(q[2] - pos[2]) < SAFE_RADIUS);
        if near {
            continue;
        }
        let kind = if world.corrupt_at(x, z) && existing_eaters < eater_cap(players.len()) {
            EnemyKind::EaterOfSouls
        } else if is_cave {
            match rng.below(3) {
                0 => EnemyKind::Skeleton,
                1 => EnemyKind::CaveBat,
                _ => EnemyKind::GiantWorm,
            }
        } else if world.corrupt_at(x, z) {
            continue;
        } else if clock.is_day() {
            EnemyKind::Slime
        } else if rng.below(2) == 0 {
            EnemyKind::Zombie
        } else {
            EnemyKind::DemonEye
        };
        return Some((kind, pos));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flat;
    impl SpawnWorld for Flat {
        fn surface_y(&self, _x: i32, _z: i32) -> Option<i32> {
            Some(50)
        }
        fn corrupt_at(&self, _x: i32, _z: i32) -> bool {
            false
        }
    }

    struct Void;
    impl SpawnWorld for Void {
        fn surface_y(&self, _x: i32, _z: i32) -> Option<i32> {
            None
        }
        fn corrupt_at(&self, _x: i32, _z: i32) -> bool {
            false
        }
    }

    const DAY: WorldClock = WorldClock { tick: 0 };
    const NIGHT: WorldClock = WorldClock { tick: 60_000 };

    /// Mixed biome: testing the candidate column also catches querying the player.
    struct Mixed;
    impl SpawnWorld for Mixed {
        fn surface_y(&self, _x: i32, _z: i32) -> Option<i32> {
            Some(50)
        }
        fn corrupt_at(&self, x: i32, _z: i32) -> bool {
            x > 0
        }
    }

    struct Corrupt;
    impl SpawnWorld for Corrupt {
        fn surface_y(&self, _x: i32, _z: i32) -> Option<i32> {
            Some(50)
        }
        fn corrupt_at(&self, _x: i32, _z: i32) -> bool {
            true
        }
    }

    #[test]
    fn eater_spawns_only_in_corruption() {
        for clock in [DAY, NIGHT] {
            let mut rng = Rng::new(11);
            let mut seen = [false; 2];
            for _ in 0..100 {
                let mut sp = Spawner {
                    ticks: SPAWN_RATE - 1,
                };
                let (kind, pos) =
                    tick_spawner(&Mixed, &clock, &[[0.5, 51.0, 0.5]], 0, 0, &mut sp, &mut rng)
                        .unwrap();
                if pos[0].floor() as i32 > 0 {
                    assert_eq!(kind, EnemyKind::EaterOfSouls);
                    seen[0] = true;
                } else {
                    if clock.is_day() {
                        assert_eq!(kind, EnemyKind::Slime);
                    } else {
                        assert!(matches!(kind, EnemyKind::Zombie | EnemyKind::DemonEye));
                    }
                    seen[1] = true;
                }
            }
            assert_eq!(seen, [true, true]);
        }
    }

    #[test]
    fn eater_spawn_cap_respected() {
        assert_eq!((eater_cap(0), eater_cap(1), eater_cap(2)), (0, 4, 8));
        for players in [vec![[0.5, 51.0, 0.5]], vec![[0.5, 51.0, 0.5]; 2]] {
            let cap = 4 * players.len();
            for clock in [DAY, NIGHT] {
                let mut rng = Rng::new(5);
                let attempt = |world: &Corrupt, total, eaters, rng: &mut Rng| {
                    tick_spawner(
                        world,
                        &clock,
                        &players,
                        total,
                        eaters,
                        &mut Spawner {
                            ticks: SPAWN_RATE - 1,
                        },
                        rng,
                    )
                };
                assert_eq!(
                    attempt(&Corrupt, cap - 1, cap - 1, &mut rng).unwrap().0,
                    EnemyKind::EaterOfSouls
                );
                assert!(attempt(&Corrupt, cap, cap, &mut rng).is_none());
                assert!(attempt(&Corrupt, cap + 1, cap + 1, &mut rng).is_none());
                assert!(attempt(&Corrupt, max_spawns(players.len()), 0, &mut rng).is_none());
                // The Eater cap must not suppress normal biome spawns.
                assert!(
                    tick_spawner(
                        &Flat,
                        &clock,
                        &players,
                        cap,
                        cap,
                        &mut Spawner {
                            ticks: SPAWN_RATE - 1
                        },
                        &mut rng
                    )
                    .is_some()
                );
            }
        }
    }

    /// Runs a full interval; returns the spawn if any.
    fn interval(
        w: &impl SpawnWorld,
        c: &WorldClock,
        players: &[[f32; 3]],
        sp: &mut Spawner,
        rng: &mut Rng,
    ) -> Option<(EnemyKind, [f32; 3])> {
        let mut out = None;
        for _ in 0..SPAWN_RATE {
            out = out.or(tick_spawner(w, c, players, 0, 0, sp, rng));
        }
        out
    }

    #[test]
    fn spawn_cap_scales_with_players() {
        assert_eq!(max_spawns(1), 11);
        assert_eq!(max_spawns(2), 13);
    }

    #[test]
    fn night_spawns_only_zombies_and_eyes() {
        let mut rng = Rng::new(1);
        let mut sp = Spawner::default();
        for _ in 0..50 {
            let (k, _) = interval(&Flat, &NIGHT, &[[0.5, 51.0, 0.5]], &mut sp, &mut rng).unwrap();
            assert!(matches!(k, EnemyKind::Zombie | EnemyKind::DemonEye));
        }
    }

    #[test]
    fn demon_eye_spawns_only_at_night() {
        let mut rng = Rng::new(3);
        let mut sp = Spawner::default();
        let mut night_eye = false;
        for _ in 0..100 {
            let (k, _) = interval(&Flat, &DAY, &[[0.5, 51.0, 0.5]], &mut sp, &mut rng).unwrap();
            assert_ne!(k, EnemyKind::DemonEye);
            let (k, _) = interval(&Flat, &NIGHT, &[[0.5, 51.0, 0.5]], &mut sp, &mut rng).unwrap();
            night_eye |= k == EnemyKind::DemonEye;
        }
        assert!(night_eye);
    }

    #[test]
    fn day_spawns_only_slimes() {
        let mut rng = Rng::new(2);
        let mut sp = Spawner::default();
        for _ in 0..50 {
            let (k, _) = interval(&Flat, &DAY, &[[0.5, 51.0, 0.5]], &mut sp, &mut rng).unwrap();
            assert_eq!(k, EnemyKind::Slime);
        }
    }

    #[test]
    fn spawn_points_are_in_ring_outside_safe_radius() {
        let mut rng = Rng::new(3);
        let mut sp = Spawner::default();
        let players = [[0.5, 51.0, 0.5], [30.5, 51.0, 0.5]];
        for _ in 0..50 {
            let (_, p) = interval(&Flat, &DAY, &players, &mut sp, &mut rng).unwrap();
            assert_eq!(p[1], 51.0, "stands on the surface block");
            let d: Vec<f32> = players
                .iter()
                .map(|q| (q[0] - p[0]).hypot(q[2] - p[2]))
                .collect();
            assert!(d.iter().all(|&d| d >= SAFE_RADIUS), "{d:?}");
            // Within the ring of at least one player (block centre shifts <1).
            assert!(
                d.iter()
                    .any(|&d| d >= SPAWN_RING.0 - 1.0 && d <= SPAWN_RING.1 + 1.0),
                "{d:?}"
            );
        }
    }

    #[test]
    fn unloaded_columns_never_spawn() {
        let mut rng = Rng::new(4);
        let mut sp = Spawner::default();
        for _ in 0..20 {
            assert!(interval(&Void, &NIGHT, &[[0.5, 51.0, 0.5]], &mut sp, &mut rng).is_none());
        }
    }

    #[test]
    fn spawn_rate_is_600() {
        let mut rng = Rng::new(5);
        let mut sp = Spawner::default();
        let p = [[0.5, 51.0, 0.5]];
        for _ in 0..SPAWN_RATE - 1 {
            assert!(tick_spawner(&Flat, &DAY, &p, 0, 0, &mut sp, &mut rng).is_none());
        }
        assert!(tick_spawner(&Flat, &DAY, &p, 0, 0, &mut sp, &mut rng).is_some());
        // At the cap nothing spawns.
        for _ in 0..SPAWN_RATE {
            assert!(tick_spawner(&Flat, &DAY, &p, 11, 0, &mut sp, &mut rng).is_none());
        }
    }

    #[test]
    fn spawn_is_deterministic() {
        let run = || {
            let mut rng = Rng::new(9);
            let mut sp = Spawner::default();
            (0..5)
                .map(|_| interval(&Flat, &NIGHT, &[[0.5, 51.0, 0.5]], &mut sp, &mut rng))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn no_players_no_spawn() {
        let mut rng = Rng::new(6);
        let mut sp = Spawner::default();
        for _ in 0..SPAWN_RATE * 2 {
            assert!(tick_spawner(&Flat, &DAY, &[], 0, 0, &mut sp, &mut rng).is_none());
        }
    }

    #[test]
    fn rng_below_is_bounded() {
        let mut rng = Rng::new(7);
        for n in [1u32, 2, 3, 100] {
            for _ in 0..1000 {
                assert!(rng.below(n) < n);
            }
        }
    }

    #[test]
    fn underworld_spawner_picks_only_underworld_kinds() {
        let mut rng = Rng::new(42);
        let mut seen = std::collections::HashSet::new();

        for _ in 0..300 {
            let mut sp = Spawner {
                ticks: SPAWN_RATE - 1,
            };
            let (kind, pos) =
                tick_spawner(&Flat, &DAY, &[[0.5, 10.0, 0.5]], 0, 0, &mut sp, &mut rng).unwrap();
            assert!(
                UNDERWORLD_POOL.contains(&kind),
                "spawned kind {:?} must be in UNDERWORLD_POOL",
                kind
            );
            assert_eq!(pos[1], 10.0, "spawn y must match player y band");
            seen.insert(kind);
        }

        assert_eq!(
            seen.len(),
            6,
            "all six kinds in UNDERWORLD_POOL must appear: {:?}",
            seen
        );
        assert!(seen.contains(&EnemyKind::LavaSlime));
        assert!(seen.contains(&EnemyKind::Hellbat));
        assert!(seen.contains(&EnemyKind::FireImp));
        assert!(seen.contains(&EnemyKind::Demon));
        assert!(seen.contains(&EnemyKind::VoodooDemon));
        assert!(seen.contains(&EnemyKind::BoneSerpentHead));
    }

    #[test]
    fn surface_spawner_never_picks_underworld_kinds() {
        let mut rng = Rng::new(123);
        for clock in [DAY, NIGHT] {
            for _ in 0..200 {
                let mut sp = Spawner {
                    ticks: SPAWN_RATE - 1,
                };
                let (kind, _) =
                    tick_spawner(&Flat, &clock, &[[0.5, 50.0, 0.5]], 0, 0, &mut sp, &mut rng)
                        .unwrap();
                assert!(
                    !UNDERWORLD_POOL.contains(&kind),
                    "surface spawner must never pick underworld kind {:?}",
                    kind
                );
            }
        }
    }

    #[test]
    fn cave_and_underworld_spawns_require_air_clearance() {
        struct RockBelow20;
        impl SpawnWorld for RockBelow20 {
            fn surface_y(&self, _x: i32, _z: i32) -> Option<i32> {
                Some(50)
            }
            fn corrupt_at(&self, _x: i32, _z: i32) -> bool {
                false
            }
            fn has_clearance(&self, _x: i32, y: i32, _z: i32) -> bool {
                y == 18
            }
        }
        let mut rng = Rng::new(7);
        let mut sp = Spawner {
            ticks: SPAWN_RATE - 1,
        };
        let (_, pos) = tick_spawner(
            &RockBelow20,
            &DAY,
            &[[0.5, 10.0, 0.5]],
            0,
            0,
            &mut sp,
            &mut rng,
        )
        .unwrap();
        assert_eq!(pos[1], 18.0, "must probe to clear air pocket at y=18");
    }

    #[test]
    fn underground_corruption_spawns_eaters() {
        struct CorruptCave;
        impl SpawnWorld for CorruptCave {
            fn surface_y(&self, _x: i32, _z: i32) -> Option<i32> {
                Some(300)
            }
            fn corrupt_at(&self, _x: i32, _z: i32) -> bool {
                true
            }
        }
        let mut rng = Rng::new(9);
        let mut sp = Spawner {
            ticks: SPAWN_RATE - 1,
        };
        let (kind, _) = tick_spawner(
            &CorruptCave,
            &DAY,
            &[[0.5, 200.0, 0.5]],
            0,
            0,
            &mut sp,
            &mut rng,
        )
        .unwrap();
        assert_eq!(kind, EnemyKind::EaterOfSouls);
    }
}
