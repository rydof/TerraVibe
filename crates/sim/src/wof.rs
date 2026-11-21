//! Wall of Flesh: multi-part Underworld boss AI (Mouth, two Eyes, The Hungry, Leeches,
//! EyeLaser, sweep acceleration, The Tongue pull, and far-edge crush).

use crate::enemy::{Enemy, EnemyId, EnemyKind};
use crate::player::consts::PX_TO_BLOCKS as PX;
use crate::world_bounds::WorldBounds;
use crate::worldgen::underworld::UNDERWORLD_TOP_Y;

pub const WOF_MAX_HP: i32 = 8000;
pub const WOF_SPAWN_OFFSET: f32 = 48.0;
pub const WOF_MOUTH_Y: f32 = 12.0;
pub const WOF_EYE_OFFSET_Y: f32 = 5.5;
pub const WOF_HUNGRY_COUNT: usize = 6;
pub const WOF_HUNGRY_DETACH_HP: i32 = 80;
pub const WOF_LEECH_INTERVAL: i32 = 240;
pub const WOF_MAX_LEECHES: usize = 3;
pub const WOF_TONGUE_PULL_SPEED: f32 = 8.0 * PX;
pub const WOF_TONGUE_DAMAGE: i32 = 25;
pub const WOF_TONGUE_DAMAGE_INTERVAL: i32 = 30;
pub const WOF_FAR_EDGE_MARGIN: f32 = 2.0;
pub const WOF_EYE_LASER_SPEED: f32 = 8.0 * PX;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WofEvent {
    EyeLaser {
        origin: [f32; 3],
        target: [f32; 3],
        damage: i32,
    },
    SpawnLeech {
        pos: [f32; 3],
    },
    FarEdgeCrush,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WofBoss {
    pub mouth_id: EnemyId,
    pub eye_ids: [EnemyId; 2],
    pub wall_x: f32,
    pub wall_z: f32,
    /// Sweep direction along X: `+1` (`-X -> +X`) or `-1` (`+X -> -X`).
    pub dir: i8,
    pub hp: i32,
    last_synced_hp: [i32; 3],
    pub laser_timer: i32,
    pub leech_timer: i32,
    pub tongue_tick: i32,
    pub tick_count: i32,
}

/// Sweep speed in blocks/tick, accelerating as HP drops (`1.5 px/tick` -> `7.5 px/tick`).
pub fn sweep_speed(hp: i32) -> f32 {
    let frac = (hp as f32 / WOF_MAX_HP as f32).clamp(0.0, 1.0);
    (1.5 + 6.0 * (1.0 - frac).powf(1.5)) * PX
}

/// Classic Eye Laser damage scaling (`22` > 50% HP, `26` 25..=50% HP, `30` < 25% HP).
pub fn eye_laser_damage(hp: i32) -> i32 {
    let frac = hp as f32 / WOF_MAX_HP as f32;
    if frac > 0.5 {
        22
    } else if frac >= 0.25 {
        26
    } else {
        30
    }
}

/// Eye Laser firing period in ticks (`120` at full HP -> `50` near 0 HP).
pub fn eye_laser_interval(hp: i32) -> i32 {
    let frac = (hp as f32 / WOF_MAX_HP as f32).clamp(0.0, 1.0);
    (50.0 + 70.0 * frac).round() as i32
}

/// Ceiling above which a player (e.g. respawned on the Surface) is outside
/// the Underworld boss arena and ignored by The Tongue.
pub const WOF_TONGUE_MAX_Y: f32 = UNDERWORLD_TOP_Y as f32 + 28.0;

/// Whether The Tongue should pull `player_pos` back in front of the Wall.
/// Players above `WOF_TONGUE_MAX_Y` (such as players respawned on the Surface)
/// are outside the Underworld engagement zone and are not dragged down.
pub fn should_tongue_pull(player_pos: [f32; 3], wall_x: f32, dir: i8) -> bool {
    if player_pos[1] > WOF_TONGUE_MAX_Y {
        return false;
    }
    let ahead = (player_pos[0] - wall_x) * (dir as f32);
    ahead < -1.0 || ahead > 95.0 || player_pos[1] > (UNDERWORLD_TOP_Y as f32 + 12.0)
}

/// Pulls a player toward the front of the Wall's Mouth, bypassing terrain.
pub fn step_tongue_pull(
    player_pos: &mut [f32; 3],
    player_vel: &mut [f32; 3],
    wall_x: f32,
    wall_z: f32,
    dir: i8,
) {
    let dest = [wall_x + (dir as f32) * 2.5, WOF_MOUTH_Y, wall_z];
    let v = [
        dest[0] - player_pos[0],
        dest[1] - player_pos[1],
        dest[2] - player_pos[2],
    ];
    let dist = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if dist <= WOF_TONGUE_PULL_SPEED {
        *player_pos = dest;
    } else if dist > 1e-4 {
        for a in 0..3 {
            player_pos[a] += WOF_TONGUE_PULL_SPEED * v[a] / dist;
        }
    }
    *player_vel = [0.0; 3];
}

/// Spawns the Wall of Flesh on the side of `player_pos` nearer a world X edge,
/// returning `(WofBoss, spawned_enemies, next_enemy_id)`.
pub fn spawn_wof(
    start_id: u32,
    player_pos: [f32; 3],
    bounds: &WorldBounds,
) -> (WofBoss, Vec<Enemy>, u32) {
    let dir: i8 = if player_pos[0] < 0.0 { 1 } else { -1 };
    let min_x = -(bounds.half as f32) + 4.0;
    let max_x = (bounds.half as f32) - 4.0;
    let wall_x = (player_pos[0] - (dir as f32) * WOF_SPAWN_OFFSET).clamp(min_x, max_x);
    let wall_z = player_pos[2];

    let mouth_id = EnemyId(start_id);
    let eye_ids = [EnemyId(start_id + 1), EnemyId(start_id + 2)];

    let mut mouth = Enemy::new(
        mouth_id,
        EnemyKind::WallOfFleshMouth,
        [wall_x, WOF_MOUTH_Y, wall_z],
    );
    mouth.ai[0] = dir as i32;

    let mut eye_top = Enemy::new(
        eye_ids[0],
        EnemyKind::WallOfFleshEye,
        [wall_x, WOF_MOUTH_Y + WOF_EYE_OFFSET_Y, wall_z],
    );
    eye_top.ai[0] = dir as i32;

    let mut eye_bot = Enemy::new(
        eye_ids[1],
        EnemyKind::WallOfFleshEye,
        [wall_x, WOF_MOUTH_Y - WOF_EYE_OFFSET_Y, wall_z],
    );
    eye_bot.ai[0] = dir as i32;

    let mut spawned = vec![mouth, eye_top, eye_bot];
    for i in 0..WOF_HUNGRY_COUNT {
        let y = 5.0 + i as f32 * 2.8;
        let z_off = (i as f32 - (WOF_HUNGRY_COUNT as f32 - 1.0) * 0.5) * 3.0;
        let mut h = Enemy::new(
            EnemyId(start_id + 3 + i as u32),
            EnemyKind::TheHungry,
            [wall_x + (dir as f32) * 3.0, y, wall_z + z_off],
        );
        h.ai = [i as i32, 0, 0, 0];
        spawned.push(h);
    }

    let wof = WofBoss {
        mouth_id,
        eye_ids,
        wall_x,
        wall_z,
        dir,
        hp: WOF_MAX_HP,
        last_synced_hp: [WOF_MAX_HP; 3],
        laser_timer: 0,
        leech_timer: 0,
        tongue_tick: 0,
        tick_count: 0,
    };

    (wof, spawned, start_id + 3 + WOF_HUNGRY_COUNT as u32)
}

/// Removes all Wall of Flesh boss parts and minions from `enemies`.
pub fn clear_wof(enemies: &mut Vec<Enemy>) {
    enemies.retain(|e| {
        !matches!(
            e.kind,
            EnemyKind::WallOfFleshMouth
                | EnemyKind::WallOfFleshEye
                | EnemyKind::TheHungry
                | EnemyKind::Leech
        )
    });
}

impl WofBoss {
    /// Steps the Wall of Flesh, syncing shared HP, advancing the wall, updating
    /// Eyes / The Hungry, and emitting laser, leech, and far-edge crush events.
    pub fn step(
        &mut self,
        enemies: &mut [Enemy],
        target: [f32; 3],
        bounds: &WorldBounds,
    ) -> Vec<WofEvent> {
        self.tick_count += 1;

        // 1. Aggregate damage dealt to Mouth and both Eyes into the shared HP pool.
        let part_ids = [self.mouth_id, self.eye_ids[0], self.eye_ids[1]];
        let mut total_dmg = 0i32;
        for (idx, pid) in part_ids.iter().enumerate() {
            if let Some(e) = enemies.iter().find(|e| e.id == *pid) {
                let lost = (self.last_synced_hp[idx] - e.hp).max(0);
                total_dmg = total_dmg.saturating_add(lost);
            }
        }
        self.hp = (self.hp - total_dmg).max(0);
        self.last_synced_hp = [self.hp; 3];
        for e in enemies.iter_mut() {
            if part_ids.contains(&e.id) {
                e.hp = self.hp;
            }
        }
        if self.hp <= 0 {
            return Vec::new();
        }

        // 2. Advance wall along sweep axis and track target Z smoothly.
        let vx = (self.dir as f32) * sweep_speed(self.hp);
        self.wall_x += vx;
        let dz = (target[2] - self.wall_z).clamp(-0.15, 0.15);
        self.wall_z += dz;

        let mut events = Vec::new();
        let far_reached = (self.dir > 0 && self.wall_x >= bounds.half as f32 - WOF_FAR_EDGE_MARGIN)
            || (self.dir < 0 && self.wall_x <= -(bounds.half as f32) + WOF_FAR_EDGE_MARGIN);
        if far_reached {
            events.push(WofEvent::FarEdgeCrush);
            return events;
        }

        // 3. Position Mouth and Eyes; update The Hungry tether/detach state.
        let tier = ((1.0 - self.hp as f32 / WOF_MAX_HP as f32) * 3.0).floor() as i32;
        let mut leech_count = 0usize;
        let mut eye_positions = Vec::with_capacity(2);
        for e in enemies.iter_mut() {
            if e.id == self.mouth_id {
                e.pos = [self.wall_x, WOF_MOUTH_Y, self.wall_z];
                e.vel = [vx, 0.0, dz];
                e.ai[0] = self.dir as i32;
            } else if e.id == self.eye_ids[0] {
                e.pos = [self.wall_x, WOF_MOUTH_Y + WOF_EYE_OFFSET_Y, self.wall_z];
                e.vel = [vx, 0.0, dz];
                e.ai[0] = self.dir as i32;
                eye_positions.push(e.pos);
            } else if e.id == self.eye_ids[1] {
                e.pos = [self.wall_x, WOF_MOUTH_Y - WOF_EYE_OFFSET_Y, self.wall_z];
                e.vel = [vx, 0.0, dz];
                e.ai[0] = self.dir as i32;
                eye_positions.push(e.pos);
            } else if e.kind == EnemyKind::TheHungry && e.hp > 0 {
                e.ai[2] = tier.clamp(0, 3);
                if e.hp <= WOF_HUNGRY_DETACH_HP {
                    e.ai[1] = 1;
                }
                if e.ai[1] == 0 {
                    let idx = e.ai[0] as f32;
                    let base_y = 5.0 + idx * 2.8;
                    let base_z = self.wall_z + (idx - (WOF_HUNGRY_COUNT as f32 - 1.0) * 0.5) * 3.0;
                    let lunge =
                        4.0 + 3.0 * ((self.tick_count as f32 * 0.08 + idx).sin() * 0.5 + 0.5);
                    let anchor = [
                        self.wall_x + (self.dir as f32) * lunge,
                        base_y + (target[1] - base_y).clamp(-2.5, 2.5),
                        base_z + (target[2] - base_z).clamp(-3.0, 3.0),
                    ];
                    for a in 0..3 {
                        let d = anchor[a] - e.pos[a];
                        e.vel[a] = d.clamp(-0.25, 0.25);
                        e.pos[a] += e.vel[a];
                    }
                } else {
                    let v = [
                        target[0] - e.pos[0],
                        target[1] + 0.8 - e.pos[1],
                        target[2] - e.pos[2],
                    ];
                    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                    let spd = 4.5 * PX;
                    if len > 1e-4 {
                        for a in 0..3 {
                            e.vel[a] = spd * v[a] / len;
                            e.pos[a] += e.vel[a];
                        }
                    }
                }
            } else if e.kind == EnemyKind::Leech && e.hp > 0 {
                leech_count += 1;
            }
        }

        // 4. Fire Eye Lasers when target is in front of the Wall.
        self.laser_timer += 1;
        let ahead = (target[0] - self.wall_x) * (self.dir as f32);
        if ahead >= 0.0 && self.laser_timer >= eye_laser_interval(self.hp) {
            self.laser_timer = 0;
            let dmg = eye_laser_damage(self.hp);
            for origin in eye_positions {
                events.push(WofEvent::EyeLaser {
                    origin: [origin[0] + (self.dir as f32) * 1.5, origin[1], origin[2]],
                    target: [target[0], target[1] + 0.9, target[2]],
                    damage: dmg,
                });
            }
        }

        // 5. Burp Leeches from Mouth periodically up to cap.
        self.leech_timer += 1;
        if self.leech_timer >= WOF_LEECH_INTERVAL {
            self.leech_timer = 0;
            if leech_count < WOF_MAX_LEECHES {
                events.push(WofEvent::SpawnLeech {
                    pos: [
                        self.wall_x + (self.dir as f32) * 2.0,
                        WOF_MOUTH_Y,
                        self.wall_z,
                    ],
                });
            }
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_bounds::WorldSize;

    #[test]
    fn wof_spawns_at_nearer_edge_and_sweeps_toward_far_edge() {
        let bounds = WorldBounds::new(WorldSize::Small);
        let (wof_left, enemies_left, _) = spawn_wof(1, [-100.0, 10.0, 0.0], &bounds);
        assert_eq!(wof_left.dir, 1);
        assert!(wof_left.wall_x < -100.0);
        assert_eq!(
            enemies_left
                .iter()
                .filter(|e| e.kind == EnemyKind::WallOfFleshMouth)
                .count(),
            1
        );
        assert_eq!(
            enemies_left
                .iter()
                .filter(|e| e.kind == EnemyKind::WallOfFleshEye)
                .count(),
            2
        );
        assert_eq!(
            enemies_left
                .iter()
                .filter(|e| e.kind == EnemyKind::TheHungry)
                .count(),
            WOF_HUNGRY_COUNT
        );

        let (wof_right, _, _) = spawn_wof(100, [100.0, 10.0, 0.0], &bounds);
        assert_eq!(wof_right.dir, -1);
        assert!(wof_right.wall_x > 100.0);
    }

    #[test]
    fn wof_shared_hp_syncs_across_mouth_and_eyes() {
        let bounds = WorldBounds::new(WorldSize::Small);
        let (mut wof, mut enemies, _) = spawn_wof(1, [-100.0, 10.0, 0.0], &bounds);
        // Hit first eye for 300 and mouth for 200
        enemies
            .iter_mut()
            .find(|e| e.id == wof.eye_ids[0])
            .unwrap()
            .hp -= 300;
        enemies
            .iter_mut()
            .find(|e| e.id == wof.mouth_id)
            .unwrap()
            .hp -= 200;

        wof.step(&mut enemies, [-80.0, 10.0, 0.0], &bounds);
        assert_eq!(wof.hp, 7500);
        for pid in [wof.mouth_id, wof.eye_ids[0], wof.eye_ids[1]] {
            assert_eq!(enemies.iter().find(|e| e.id == pid).unwrap().hp, 7500);
        }
    }

    #[test]
    fn wof_speed_and_laser_damage_scale_as_hp_drops() {
        assert!(sweep_speed(1000) > sweep_speed(4000));
        assert!(sweep_speed(4000) > sweep_speed(8000));
        assert_eq!(eye_laser_damage(8000), 22);
        assert_eq!(eye_laser_damage(3000), 26);
        assert_eq!(eye_laser_damage(1000), 30);
        assert!(eye_laser_interval(1000) < eye_laser_interval(8000));
    }

    #[test]
    fn wof_hungry_detaches_at_80_hp_and_scales() {
        let bounds = WorldBounds::new(WorldSize::Small);
        let (mut wof, mut enemies, _) = spawn_wof(1, [-100.0, 10.0, 0.0], &bounds);
        wof.hp = 1000;
        wof.last_synced_hp = [1000; 3];
        for e in enemies.iter_mut() {
            if e.id == wof.mouth_id || e.id == wof.eye_ids[0] || e.id == wof.eye_ids[1] {
                e.hp = 1000;
            }
        }
        let hungry_id = enemies
            .iter()
            .find(|e| e.kind == EnemyKind::TheHungry)
            .unwrap()
            .id;
        enemies.iter_mut().find(|e| e.id == hungry_id).unwrap().hp = 75;
        wof.step(&mut enemies, [-80.0, 10.0, 0.0], &bounds);
        let h = enemies.iter().find(|e| e.id == hungry_id).unwrap();
        assert_eq!(h.ai[1], 1, "The Hungry detaches at <= 80 HP");
        assert!(h.damage() > 30, "damage scales when WoF HP is low");
        assert!(h.defense() > 10, "defense scales when WoF HP is low");
    }

    #[test]
    fn wof_tongue_pull_and_far_edge_crush() {
        let bounds = WorldBounds::new(WorldSize::Small);
        // Behind wall when dir == +1
        assert!(should_tongue_pull([-150.0, 10.0, 0.0], -140.0, 1));
        assert!(!should_tongue_pull([-120.0, 10.0, 0.0], -140.0, 1));
        // Escaped just above Underworld ceiling -> pulled back down
        assert!(should_tongue_pull([-120.0, 50.0, 0.0], -140.0, 1));
        // Player respawned on the Surface (y > WOF_TONGUE_MAX_Y) -> NOT dragged into Hell
        assert!(!should_tongue_pull([0.0, 65.0, 0.0], -140.0, 1));
        assert!(!should_tongue_pull([-160.0, 65.0, 0.0], -140.0, 1));

        let mut ppos = [-160.0, 10.0, 0.0];
        let mut pvel = [-0.2, 0.1, 0.0];
        step_tongue_pull(&mut ppos, &mut pvel, -140.0, 0.0, 1);
        assert!(ppos[0] > -160.0);
        assert_eq!(pvel, [0.0; 3]);

        // Far-edge crush
        let (mut wof, mut enemies, _) = spawn_wof(1, [-100.0, 10.0, 0.0], &bounds);
        wof.wall_x = bounds.half as f32 - WOF_FAR_EDGE_MARGIN;
        let evs = wof.step(&mut enemies, [bounds.half as f32 - 5.0, 10.0, 0.0], &bounds);
        assert!(evs.contains(&WofEvent::FarEdgeCrush));
    }
}
