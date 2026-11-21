//! Melee damage, crits, knockback and player i-frames.

use crate::enemy::{Enemy, EnemyId};
use crate::item::Rng;
use crate::player::consts::PX_TO_BLOCKS as PX;
use crate::player::{DeathCause, Player, PlayerEvent};

/// Player.Hurt immunity: 40 ticks, 20 for 1 damage (gameplay spec).
pub const HURT_IFRAMES: i32 = 40;
pub const HURT_IFRAMES_MIN: i32 = 20;
/// Immunity after respawn (gameplay spec).
pub const RESPAWN_IFRAMES: i32 = 60;
/// NPC.immune cooldown after a player hit (gameplay spec).
pub const NPC_HIT_COOLDOWN: i32 = 12;
/// On Fire buff 24: 7 seconds duration (420 ticks), 4 hp/s DoT (lifeRegen -= 8, gameplay spec).
pub const ON_FIRE_TICKS: i32 = 420;
pub const ON_FIRE_DPS: i32 = 4;
/// Sword reach in blocks and forward-cone cosine (Stage 3 design; not a Terraria hitbox).
pub const SWORD_REACH: f32 = 3.2;
pub const SWORD_COS: f32 = 0.5;
/// Firecracker whip reach in blocks (`2.75 * SWORD_REACH = 8.8`).
pub const FIRECRACKER_REACH: f32 = SWORD_REACH * 2.75;
/// Spear extension reach in blocks.
pub const SPEAR_REACH: f32 = 3.5;
/// Boomerang maximum flight distance in blocks before returning.
pub const BOOMERANG_MAX_DIST: f32 = 20.0;
/// Flail maximum flight distance in blocks before returning.
pub const FLAIL_MAX_DIST: f32 = 12.0;
/// Vilethorn maximum extension reach in blocks.
pub const VILETHORN_REACH: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub damage: i32,
    pub crit: bool,
    pub killed: bool,
}

/// Damage a player takes from a `base` hit: `base - (defense + 1) / 2`, at least 1
/// (`Main.CalculateDamagePlayersTake`, Classic: `Damage - ceil(Defense * 0.5)`, floored at 1).
pub fn damage_taken(base: i32, defense: i32) -> i32 {
    (base - (defense + 1) / 2).max(1)
}

/// Damage the player with an attributed death cause; false when ignored (dead or immune).
pub fn hurt_player_with_cause(
    p: &mut Player,
    base: i32,
    cause: DeathCause,
    ev: &mut Vec<PlayerEvent>,
) -> bool {
    if p.dead || p.god_mode || p.i_frames > 0 || base <= 0 {
        return false;
    }
    p.last_damage_cause = Some(cause);
    let amount = damage_taken(base, p.defense());
    p.i_frames = if amount == 1 {
        HURT_IFRAMES_MIN
    } else {
        HURT_IFRAMES
    };
    let died = p.vitals.hurt(amount);
    ev.push(PlayerEvent::Hurt { amount });
    if died {
        p.kill(cause, ev);
    }
    true
}

/// Damage the player (armor applied); false when ignored (dead or immune).
pub fn hurt_player(p: &mut Player, base: i32, ev: &mut Vec<PlayerEvent>) -> bool {
    hurt_player_with_cause(p, base, DeathCause::Unknown, ev)
}

/// Melee hit on an enemy. Damage is `base - defense / 2` (NPC.StrikeNPC), doubled on a crit.
/// Knockback is in px/tick like Terraria, pushed horizontally away from `from`.
pub fn apply_hit(
    e: &mut Enemy,
    from: [f32; 3],
    base: i32,
    knockback: f32,
    crit: i32,
    rng: &mut Rng,
) -> Option<Hit> {
    apply_hit_with_immunity(e, from, base, knockback, crit, rng, false)
}

/// Like [`apply_hit`], optionally ignoring `e.hurt_ticks` immunity (for single-hit projectiles).
pub fn apply_hit_with_immunity(
    e: &mut Enemy,
    from: [f32; 3],
    base: i32,
    knockback: f32,
    crit: i32,
    rng: &mut Rng,
    ignore_immunity: bool,
) -> Option<Hit> {
    if !ignore_immunity && e.hurt_ticks > 0 {
        return None;
    }
    let st = e.kind.stats();
    let mut damage = (base - e.defense() / 2).max(1);
    let is_crit = (rng.below(100) as i32) < crit;
    if is_crit {
        damage *= 2;
    }
    e.hp -= damage;
    let (dx, dz) = (e.pos[0] - from[0], e.pos[2] - from[2]);
    let len = (dx * dx + dz * dz).sqrt();
    let (nx, nz) = if len > 1e-4 {
        (dx / len, dz / len)
    } else {
        (1.0, 0.0)
    };
    let k = knockback * (1.0 - st.knockback_resist) * PX;
    e.vel[0] += nx * k;
    e.vel[2] += nz * k;
    if k > 0.0 && e.on_ground {
        e.vel[1] = e.vel[1].max(0.14 * (1.0 - st.knockback_resist));
    }
    if !ignore_immunity {
        e.hurt_ticks = NPC_HIT_COOLDOWN;
    }
    Some(Hit {
        damage,
        crit: is_crit,
        killed: e.hp <= 0,
    })
}

/// Melee swing reach in blocks for `item` (scaled by `item.scale`; `FIRECRACKER` extends to 2.75x).
pub fn melee_reach(item: crate::item::ItemId) -> f32 {
    if item == crate::item::FIRECRACKER {
        FIRECRACKER_REACH
    } else {
        let scale = crate::item::item_data(item).scale.max(0.5);
        SWORD_REACH * scale
    }
}

/// Living enemies whose AABB is within `reach` and the forward cone of the view ray.
pub fn swing_hits_with_reach(
    eye: [f32; 3],
    yaw: f32,
    pitch: f32,
    reach: f32,
    enemies: &[Enemy],
) -> Vec<EnemyId> {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let dir = [-sy * cp, sp, -cy * cp];
    enemies
        .iter()
        .filter(|e| e.hp > 0 && !e.kind.is_friendly())
        .filter(|e| {
            let st = e.kind.stats();
            let min = [e.pos[0] - st.half[0], e.pos[1], e.pos[2] - st.half[0]];
            let max = [
                e.pos[0] + st.half[0],
                e.pos[1] + 2.0 * st.half[1],
                e.pos[2] + st.half[0],
            ];
            let closest = [
                eye[0].clamp(min[0], max[0]),
                eye[1].clamp(min[1], max[1]),
                eye[2].clamp(min[2], max[2]),
            ];
            let d = [
                closest[0] - eye[0],
                closest[1] - eye[1],
                closest[2] - eye[2],
            ];
            let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if dist <= 1e-4 {
                return true;
            }
            if dist > reach {
                return false;
            }
            let c = [e.pos[0], e.pos[1] + st.half[1], e.pos[2]];
            let dc = [c[0] - eye[0], c[1] - eye[1], c[2] - eye[2]];
            let dc_len = (dc[0] * dc[0] + dc[1] * dc[1] + dc[2] * dc[2]).sqrt();
            (d[0] * dir[0] + d[1] * dir[1] + d[2] * dir[2]) / dist >= SWORD_COS
                || (dc_len > 1e-4
                    && (dc[0] * dir[0] + dc[1] * dir[1] + dc[2] * dir[2]) / dc_len >= SWORD_COS)
        })
        .map(|e| e.id)
        .collect()
}

/// Living enemies hit by a swing of `item` along `(yaw, pitch)`.
pub fn swing_hits_for_item(
    eye: [f32; 3],
    yaw: f32,
    pitch: f32,
    item: crate::item::ItemId,
    enemies: &[Enemy],
) -> Vec<EnemyId> {
    swing_hits_with_reach(eye, yaw, pitch, melee_reach(item), enemies)
}

/// Living enemies whose centre is within reach and the forward cone of the view ray.
pub fn swing_hits(eye: [f32; 3], yaw: f32, pitch: f32, enemies: &[Enemy]) -> Vec<EnemyId> {
    swing_hits_with_reach(eye, yaw, pitch, SWORD_REACH, enemies)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enemy::EnemyKind;
    use crate::player::tick_player;
    use crate::{BlockQuery, PlayerInput};

    fn en(kind: EnemyKind, pos: [f32; 3]) -> Enemy {
        Enemy::new(EnemyId(1), kind, pos)
    }

    #[test]
    fn damage_subtracts_half_defense() {
        let mut rng = Rng::new(1);
        // zombie defense 6 -> 9 - 3 = 6 (crit 0)
        let mut z = en(EnemyKind::Zombie, [0.0; 3]);
        let h = apply_hit(&mut z, [-1.0, 0.0, 0.0], 9, 5.5, 0, &mut rng).unwrap();
        assert_eq!((h.damage, z.hp), (6, 45 - 6));
        // never below 1
        let mut z = en(EnemyKind::Zombie, [0.0; 3]);
        assert_eq!(
            apply_hit(&mut z, [0.0; 3], 2, 0.0, 0, &mut rng)
                .unwrap()
                .damage,
            1
        );
        // cooldown blocks a second hit
        assert!(apply_hit(&mut z, [0.0; 3], 9, 0.0, 0, &mut rng).is_none());
        assert_eq!(z.hurt_ticks, NPC_HIT_COOLDOWN);
    }

    #[test]
    fn crit_rate_is_four_percent() {
        let mut rng = Rng::new(42);
        let mut crits = 0;
        for _ in 0..10_000 {
            let mut s = en(EnemyKind::Slime, [0.0; 3]);
            let h = apply_hit(&mut s, [1.0, 0.0, 0.0], 9, 5.5, 4, &mut rng).unwrap();
            if h.crit {
                crits += 1;
                assert_eq!(h.damage, (9 - 1) * 2);
            }
        }
        assert!((300..=500).contains(&crits), "crits {crits}");
    }

    #[test]
    fn knockback_scales_by_resist() {
        let mut rng = Rng::new(1);
        let mut s = en(EnemyKind::Slime, [1.0, 0.0, 0.0]);
        apply_hit(&mut s, [0.0; 3], 9, 5.5, 0, &mut rng);
        let full = 5.5 * (1.0 - s.kind.stats().knockback_resist) * PX;
        assert!((s.vel[0] - full).abs() < 1e-6 && s.vel[2].abs() < 1e-6);
    }

    fn player() -> Player {
        Player::new([0.5, 10.0, 0.5])
    }

    #[test]
    fn hurt_ignores_damage_during_iframes() {
        let mut p = player();
        let mut ev = Vec::new();
        assert!(hurt_player(&mut p, 14, &mut ev));
        assert_eq!((p.i_frames, p.vitals.life), (HURT_IFRAMES, 100 - 14));
        assert!(!hurt_player(&mut p, 14, &mut ev));
        assert_eq!(p.vitals.life, 100 - 14);
        assert_eq!(ev, [PlayerEvent::Hurt { amount: 14 }]);
    }

    #[test]
    fn armor_damage_is_reduced_by_half_defense_never_below_one() {
        use crate::item::{GOLD_CHAINMAIL, IRON_HELMET};
        assert_eq!(damage_taken(14, 0), 14);
        assert_eq!(damage_taken(14, 5), 11);
        assert_eq!(damage_taken(3, 40), 1);
        let mut p = player();
        p.armor = [Some(IRON_HELMET), Some(GOLD_CHAINMAIL), None]; // 2 + 5 = 7 -> (7+1)/2 = 4
        let mut ev = Vec::new();
        assert!(hurt_player(&mut p, 14, &mut ev));
        assert_eq!(p.vitals.life, 100 - 10);
        assert_eq!(ev, [PlayerEvent::Hurt { amount: 10 }]);
    }

    #[test]
    fn one_damage_uses_20_tick_iframes() {
        let mut p = player();
        assert!(hurt_player(&mut p, 1, &mut Vec::new()));
        assert_eq!(p.i_frames, HURT_IFRAMES_MIN);
    }

    struct Floor;
    impl BlockQuery for Floor {
        fn block(&self, _: i32, y: i32, _: i32) -> crate::BlockId {
            if y < 5 {
                crate::block::STONE
            } else {
                crate::block::AIR
            }
        }
    }

    #[test]
    fn hurt_player_with_cause_sets_cause_and_kills() {
        let mut p = player();
        let mut ev = Vec::new();
        assert!(hurt_player_with_cause(
            &mut p,
            10,
            DeathCause::Contact(EnemyKind::Zombie),
            &mut ev
        ));
        assert_eq!(
            p.last_damage_cause,
            Some(DeathCause::Contact(EnemyKind::Zombie))
        );
        assert!(!p.dead);

        // Fatal hit
        p.i_frames = 0;
        assert!(hurt_player_with_cause(
            &mut p,
            1000,
            DeathCause::Fall,
            &mut ev
        ));
        assert!(p.dead);
        assert_eq!(p.last_damage_cause, Some(DeathCause::Fall));
        assert!(ev.contains(&PlayerEvent::Died {
            cause: DeathCause::Fall
        }));
    }

    #[test]
    fn hurt_player_calls_hurt_player_with_cause_unknown() {
        let mut p = player();
        let mut ev = Vec::new();
        assert!(hurt_player(&mut p, 1000, &mut ev));
        assert!(p.dead);
        assert_eq!(p.last_damage_cause, Some(DeathCause::Unknown));
        assert!(ev.contains(&PlayerEvent::Died {
            cause: DeathCause::Unknown
        }));
    }

    #[test]
    fn respawn_grants_60_ticks_immunity() {
        let mut p = player();
        let mut ev = Vec::new();
        assert!(hurt_player(&mut p, 1000, &mut ev));
        assert!(p.dead);
        let mut respawned = false;
        for _ in 0..1000 {
            if tick_player(&Floor, &mut p, &PlayerInput::default())
                .contains(&PlayerEvent::Respawned)
            {
                respawned = true;
                break;
            }
        }
        assert!(respawned && p.i_frames == RESPAWN_IFRAMES);
        assert!(!hurt_player(&mut p, 10, &mut ev));
        tick_player(&Floor, &mut p, &PlayerInput::default());
        assert_eq!(p.i_frames, RESPAWN_IFRAMES - 1);
    }

    #[test]
    fn sword_cone_hits_front_not_back() {
        let eye = [0.0, 1.6, 0.0];
        // yaw 0 faces -z
        let front = en(EnemyKind::Zombie, [0.0, 0.8, -2.0]);
        let mut back = en(EnemyKind::Zombie, [0.0, 0.8, 2.0]);
        back.id = EnemyId(2);
        let mut far = en(EnemyKind::Zombie, [0.0, 0.8, -6.0]);
        far.id = EnemyId(3);
        let mut dead = en(EnemyKind::Zombie, [0.0, 0.8, -1.5]);
        dead.id = EnemyId(4);
        dead.hp = 0;
        let hits = swing_hits(eye, 0.0, 0.0, &[front, back, far, dead]);
        assert_eq!(hits, [EnemyId(1)]);
    }

    mod onfire {
        use super::*;
        use crate::item::FLAMING_ARROW;
        use crate::projectile::{Projectile, hit_enemy};

        #[test]
        fn flaming_arrow_applies_on_fire() {
            let mut enemy = Enemy::new(EnemyId(1), EnemyKind::Zombie, [0.0, 0.0, 0.0]);
            let p =
                Projectile::fire_from_bow(1, 0, FLAMING_ARROW, [0.0, 0.5, 0.0], [1.0, 0.0, 0.0])
                    .expect("projectile fired");
            let mut rng = Rng::new(42);
            let hit = hit_enemy(&p, std::slice::from_mut(&mut enemy), &mut rng);
            assert_eq!(hit, Some(EnemyId(1)));
            assert!(enemy.is_on_fire());
            assert_eq!(enemy.on_fire_ticks, 420);
        }

        #[test]
        fn on_fire_ticks_down_and_expires() {
            let mut enemy = Enemy::new(EnemyId(1), EnemyKind::Zombie, [0.0, 0.0, 0.0]);
            enemy.hp = 40;
            enemy.apply_on_fire(420);
            assert!(enemy.is_on_fire());

            // 15 ticks = 1 tick of DoT (4 hp/sec at 60 Hz = 1 hp per 15 ticks)
            for _ in 0..15 {
                enemy.tick_on_fire();
            }
            assert_eq!(enemy.hp, 39);
            assert_eq!(enemy.on_fire_ticks, 405);

            // Finish the 420 ticks
            for _ in 15..420 {
                enemy.tick_on_fire();
            }
            assert_eq!(enemy.on_fire_ticks, 0);
            assert!(!enemy.is_on_fire());
            assert_eq!(enemy.hp, 40 - 28); // 4 hp/s * 7s = 28 damage
        }

        #[test]
        fn on_fire_does_not_stack() {
            let mut enemy = Enemy::new(EnemyId(1), EnemyKind::Zombie, [0.0, 0.0, 0.0]);
            enemy.hp = 40;
            enemy.apply_on_fire(200);
            enemy.apply_on_fire(420);
            assert_eq!(enemy.on_fire_ticks, 420);

            for _ in 0..15 {
                enemy.tick_on_fire();
            }
            // Still deals 1 damage per 15 ticks, not 2
            assert_eq!(enemy.hp, 39);
        }

        #[test]
        fn firecracker_has_extended_2_75x_melee_reach() {
            use crate::item::{FIERY_GREATSWORD, FIRECRACKER};
            let eye = [0.0, 1.62, 0.0];
            let enemies = vec![
                Enemy::new(EnemyId(1), EnemyKind::Zombie, [0.0, 0.72, -2.5]),
                Enemy::new(EnemyId(2), EnemyKind::Zombie, [0.0, 0.72, -7.5]),
                Enemy::new(EnemyId(3), EnemyKind::Zombie, [0.0, 0.72, -9.5]),
            ];
            let sword_hits = swing_hits_for_item(eye, 0.0, 0.0, FIERY_GREATSWORD, &enemies);
            assert_eq!(sword_hits, vec![EnemyId(1)]);

            let whip_hits = swing_hits_for_item(eye, 0.0, 0.0, FIRECRACKER, &enemies);
            assert_eq!(whip_hits, vec![EnemyId(1), EnemyId(2)]);
        }

        #[test]
        fn critters_are_hittable_by_melee_and_god_mode_ignores_damage() {
            use crate::item::IRON_BROADSWORD;
            let eye = [0.0, 1.62, 0.0];
            let mut enemies = vec![
                Enemy::new(EnemyId(1), EnemyKind::Bunny, [0.0, 1.0, -1.5]),
                Enemy::new(EnemyId(2), EnemyKind::Guide, [0.0, 1.0, -1.5]),
            ];
            let hits = swing_hits_for_item(eye, 0.0, 0.0, IRON_BROADSWORD, &enemies);
            assert_eq!(hits, vec![EnemyId(1)]);

            let mut rng = Rng::new(7);
            let h = apply_hit(&mut enemies[0], eye, 10, 2.0, 0, &mut rng).unwrap();
            assert!(h.killed);

            let mut p = player();
            p.god_mode = true;
            let mut ev = Vec::new();
            assert!(!hurt_player(&mut p, 50, &mut ev));
            assert_eq!(p.vitals.life, 100);
            assert!(ev.is_empty());
        }
    }
}
