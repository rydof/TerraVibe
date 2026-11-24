//! Falling stars: Terraria's nightly `WorldGen.StarFall` spawner and the
//! projectile-12 star it releases.

use crate::chunk::BlockQuery;
use crate::item::Rng;
use crate::world_bounds::WorldBounds;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Star {
    pub id: u32,
    pub pos: [f32; 3],
    pub vel: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StarStep {
    Falling,
    Landed,
}

pub fn spawn_chance(rng: &mut Rng, max_tiles_x: f32, boost: f32, is_day: bool) -> bool {
    if is_day {
        return false;
    }
    // gameplay spec: num24 = (maxTilesX / 4200.0) * starfallBoost;
    // if Next(8000) < 10.0 * num24
    let threshold = 10.0 * (max_tiles_x as f64 / 4200.0) * (boost as f64);
    (rng.below(8000) as f64) < threshold
}

pub fn spawn_star(
    rng: &mut Rng,
    bounds: &WorldBounds,
    ground_y: f32,
    boost: f32,
    is_day: bool,
) -> Option<Star> {
    let max_tiles_x = match bounds.size() {
        crate::world_bounds::WorldSize::Small => 4200.0,
        crate::world_bounds::WorldSize::Medium => 6400.0,
        crate::world_bounds::WorldSize::Large => 8400.0,
    };
    if !spawn_chance(rng, max_tiles_x, boost, is_day) {
        return None;
    }
    Some(star_at(rng, bounds, ground_y, 0))
}

pub fn despawns_at_dawn(is_day: bool) -> bool {
    is_day
}

pub fn night_setup(rng: &mut Rng) -> f32 {
    // gameplay spec: starfallBoost = 1f;
    // 1/10 roll -> Next(300, 501) * 0.01
    // else 1/3 roll -> Next(100, 151) * 0.01
    if rng.below(10) == 0 {
        (300 + rng.below(201)) as f32 * 0.01
    } else if rng.below(3) == 0 {
        (100 + rng.below(51)) as f32 * 0.01
    } else {
        1.0
    }
}

pub fn star_at(rng: &mut Rng, bounds: &WorldBounds, ground_y: f32, id: u32) -> Star {
    // gameplay spec / gameplay spec:
    // speed 12 px/tick, vx -100..100, vy 100..300 (downwards)
    // To ensure vx != 0, if vx == 0 pick 1.
    let mut vx_raw = (rng.below(201) as i32) - 100;
    if vx_raw == 0 {
        vx_raw = 1;
    }
    let vx = vx_raw as f32;
    let vy = (100 + rng.below(200)) as f32;
    let len = (vx * vx + vy * vy).sqrt();
    let speed = 12.0 * crate::player::consts::PX_TO_BLOCKS;
    let norm = speed / len;
    let vel_x = vx * norm;
    let vel_y = -vy * norm; // downward in TerraVibe coordinates
    let half = bounds.size().half_extent() as f32;
    let min_x = -half + 10.0;
    let max_x = half - 10.0;
    let span = (max_x - min_x).max(1.0);
    let x = min_x + (rng.below(10000) as f32 / 10000.0) * span;
    let y = ground_y + 30.0;
    Star {
        id,
        pos: [x, y, 0.5],
        vel: [vel_x, vel_y, 0.0],
    }
}

pub fn step_star(q: &impl BlockQuery, s: &mut Star) -> StarStep {
    s.pos[0] += s.vel[0];
    s.pos[1] += s.vel[1];
    s.pos[2] += s.vel[2];
    let bx = s.pos[0].floor() as i32;
    let by = s.pos[1].floor() as i32;
    let bz = s.pos[2].floor() as i32;
    if q.block(bx, by, bz) != crate::block::AIR {
        s.pos[1] = by as f32 + 1.0;
        StarStep::Landed
    } else {
        StarStep::Falling
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{AIR, BlockId, STONE};
    use crate::chunk::BlockQuery;
    use crate::item::Rng;
    use crate::world_bounds::{WorldBounds, WorldSize};

    struct Floor;
    impl BlockQuery for Floor {
        fn block(&self, _x: i32, y: i32, _z: i32) -> BlockId {
            if y <= 0 { STONE } else { AIR }
        }
    }

    #[test]
    fn star_spawns_only_at_night() {
        // gameplay spec: the loop returns while `Main.dayTime`.
        let mut rng = Rng::new(7);
        for _ in 0..2_000 {
            assert!(!spawn_chance(&mut rng, 4_200.0, 5.0, true));
        }
        let mut rng = Rng::new(7);
        let night = (0..20_000)
            .filter(|_| spawn_chance(&mut rng, 4_200.0, 1.0, false))
            .count();
        // 10/8000 per tick: ~25 hits in 20,000 draws.
        assert!((10..60).contains(&night), "night spawns: {night}");

        let mut rng = Rng::new(7);
        assert!(
            spawn_star(
                &mut rng,
                &WorldBounds::new(WorldSize::Small),
                128.0,
                5.0,
                true
            )
            .is_none()
        );
        let mut rng = Rng::new(7);
        let mut stars = 0;
        for _ in 0..2_000 {
            if spawn_star(
                &mut rng,
                &WorldBounds::new(WorldSize::Small),
                128.0,
                5.0,
                false,
            )
            .is_some()
            {
                stars += 1;
            }
        }
        assert!(stars > 0, "night produces falling stars");
    }

    #[test]
    fn star_despawns_at_dawn() {
        // gameplay spec: type 12 with damage 1000 is killed in the day.
        assert!(despawns_at_dawn(true));
        assert!(!despawns_at_dawn(false));
    }

    #[test]
    fn star_falls_slanted_and_lands() {
        // gameplay spec: speed 12 px/tick, vx -100..100, vy 100..300.
        let mut rng = Rng::new(11);
        let mut s = star_at(&mut rng, &WorldBounds::new(WorldSize::Small), 128.0, 1);
        assert!(s.vel[1] < 0.0, "falls toward the ground");
        assert!(s.vel[0] != 0.0, "slanted, not vertical");
        let speed = (s.vel[0].powi(2) + s.vel[1].powi(2)).sqrt();
        assert!((speed - 12.0 * crate::player::consts::PX_TO_BLOCKS).abs() < 1e-5);

        let q = Floor;
        let mut steps = 0;
        while step_star(&q, &mut s) == StarStep::Falling {
            steps += 1;
            assert!(steps < 1_000);
        }
        assert!(
            s.pos[1] > 0.0 && s.pos[1] < 2.0,
            "lands on the floor: {}",
            s.pos[1]
        );
    }

    #[test]
    fn night_setup_matches_spec() {
        // gameplay spec: boost resets to 1; 1/10 gives 3.00..5.00; else 1/3 gives 1.00..1.50.
        let mut rng = Rng::new(3);
        let mut boosts = [0usize; 3];
        for _ in 0..10_000 {
            let b = night_setup(&mut rng);
            if b == 1.0 {
                boosts[0] += 1;
            } else if (3.0..=5.0).contains(&b) {
                boosts[1] += 1;
            } else if (1.0..=1.5).contains(&b) {
                boosts[2] += 1;
            } else {
                panic!("boost out of range: {b}");
            }
        }
        assert!((900..=1100).contains(&boosts[1]), "1/10 roll: {boosts:?}");
        assert!((2700..=3300).contains(&boosts[2]), "1/3 roll: {boosts:?}");
    }
}
