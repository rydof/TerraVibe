//! Eye of Cthulhu: a pure state machine modeled after `NPC.boss AI` (aiStyle 4,
//! gameplay spec, classic difficulty). The server owns one `Enemy` of kind
//! `EyeOfCthulhu` for position, velocity and life; `EyeBoss` holds only the AI
//! registers (`NPC.ai[0..4]`). Terraria numbers are in px/tick and are scaled by
//! `PX_TO_BLOCKS`. Terraria is 2D: vectors here gain a z component, nothing else changes.
//!
//! Deviations (this game): the Servant of Cthulhu spawns of gameplay spec are left
//! out, and the expert-mode branches are ignored.

use crate::enemy::{Enemy, EnemyKind};
use crate::inventory::ItemStack;
use crate::item::{DEMONITE_ORE_ITEM, HEART, LESSER_HEALING_POTION, Prefix, Rng};
use crate::player::consts::PX_TO_BLOCKS as PX;

/// Phase 1 hover (gameplay spec): speed 5 px, acceleration 0.04 px, 200 px above.
const P1_HOVER_SPEED: f32 = 5.0;
const P1_HOVER_ACCEL: f32 = 0.04;
const P1_HOVER_ABOVE: f32 = 200.0;
/// Ticks of hovering before a dash series (gameplay spec).
const P1_HOVER_TICKS: f32 = 600.0;
/// Phase 1 dash speed (gameplay spec), fast ticks (`:20281`), decay (`:20283`), ticks per dash (`:20305`), dashes (`:20320`).
const P1_DASH_SPEED: f32 = 6.0;
const P1_DASH_FAST: f32 = 40.0;
const P1_DASH_DECAY: f32 = 0.98;
const P1_DASH_TICKS: f32 = 150.0;
const DASHES: f32 = 3.0;
/// Servants of Cthulhu spawn cadence during phase 1 hover (gameplay spec).
pub const SERVANT_SPAWN_INTERVAL: f32 = 110.0;
/// Maximum live Servants of Cthulhu at any time.
pub const MAX_SERVANTS: usize = 8;
/// Phase 2 starts below this fraction of max life (gameplay spec).
const PHASE2_LIFE: f32 = 0.5;
/// Transition: spin up by 0.005 to 0.5 (`:20350-20354`), two 100-tick stages (`:20409`), drag 0.98 (`:20442`).
const SPIN_STEP: f32 = 0.005;
const SPIN_MAX: f32 = 0.5;
const TRANSITION_STAGE_TICKS: f32 = 100.0;
const TRANSITION_DRAG: f32 = 0.98;
/// Phase 2 hover (gameplay spec,20481`): speed 6 px, accel 0.07 px, 120 px above.
const P2_HOVER_SPEED: f32 = 6.0;
const P2_HOVER_ACCEL: f32 = 0.07;
const P2_HOVER_ABOVE: f32 = 120.0;
/// Hover ticks (`:20539`), dash speed (`:20564`), fast ticks (`:20589`), decay (`:20597`), ticks per dash (`:20615`).
const P2_HOVER_TICKS: f32 = 200.0;
const P2_DASH_SPEED: f32 = 6.8;
const P2_DASH_FAST: f32 = 40.0;
const P2_DASH_DECAY: f32 = 0.97;
const P2_DASH_TICKS: f32 = 130.0;
/// Daytime / dead target: drift up 0.04 px per tick and despawn after 10 ticks (`:20128-20132`).
const FLEE_ACCEL: f32 = 0.04;
const FLEE_TICKS: i32 = 10;
/// Velocity components under this many px/tick snap to 0 (`:20292`).
const SNAP: f32 = 0.1;

/// Blocks above the player at which the summoned Eye appears (plan: ~40 tiles;
/// `NPC.SpawnOnPlayer` picks a spot off screen, gameplay spec).
pub const SUMMON_HEIGHT: f32 = 40.0;
/// This game's tuning: nightly spawn chance 1 in `NIGHT_SPAWN_ODDS`, needs `life_max >= MIN_LIFE_MAX`.
/// Vanilla rolls `1/maxValue` once town NPCs exist (gameplay spec,66771`); this game has none.
pub const NIGHT_SPAWN_ODDS: u32 = 3;
pub const MIN_LIFE_MAX: i32 = 200;
pub const MIN_DEFENSE: i32 = 10;
pub const MIN_TOWN_NPCS: usize = 3;
/// Terraria night tick after which the eye is spawned (gameplay spec).
pub const SPAWN_AFTER_NIGHT_TICK: u32 = 4860;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BossEvent {
    None,
    /// Fled at daybreak or no target: remove the boss.
    Despawn,
    /// Spawn a Servant of Cthulhu (gameplay spec).
    SpawnServant {
        pos: [f32; 3],
        vel: [f32; 3],
    },
}

/// `NPC.ai`: [phase (0 = first, 1-2 = transition, 3 = second), state, timer, dash count].
#[derive(Clone, Debug, PartialEq)]
pub struct EyeBoss {
    pub ai: [f32; 4],
    /// Spin angle during the transition, radians.
    pub rotation: f32,
    flee: i32,
    pub servant_timer: f32,
}

impl Default for EyeBoss {
    fn default() -> Self {
        EyeBoss::new()
    }
}

fn len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// `velocity` eases toward `want` by `accel` per axis, doubled when crossing zero
/// (gameplay spec).
fn steer(vel: &mut [f32; 3], want: [f32; 3], accel: f32) {
    for a in 0..3 {
        if vel[a] < want[a] {
            vel[a] += accel;
            if vel[a] < 0.0 && want[a] > 0.0 {
                vel[a] += accel;
            }
        } else if vel[a] > want[a] {
            vel[a] -= accel;
            if vel[a] > 0.0 && want[a] < 0.0 {
                vel[a] -= accel;
            }
        }
    }
}

fn snap(vel: &mut [f32; 3]) {
    for v in vel {
        if v.abs() < SNAP * PX {
            *v = 0.0;
        }
    }
}

impl EyeBoss {
    pub fn new() -> Self {
        EyeBoss {
            ai: [0.0; 4],
            rotation: 0.0,
            flee: 0,
            servant_timer: 0.0,
        }
    }

    /// 1 while the first form, 2 from the transition on.
    pub fn phase(&self) -> u8 {
        if self.ai[0] == 0.0 { 1 } else { 2 }
    }

    /// Client-facing animation/pose state code:
    /// - `0`: hovering
    /// - `1`: dashing
    /// - `2`: transform stage 1 (spinning up with iris)
    /// - `3`: fleeing (daytime)
    /// - `4`: transform stage 2 (mouth exposed, gore burst)
    pub fn state_code(&self, day: bool) -> u8 {
        if day {
            3
        } else if self.ai[0] == 1.0 {
            2
        } else if self.ai[0] == 2.0 {
            4
        } else if self.ai[1] >= 1.0 {
            1
        } else {
            0
        }
    }

    /// One 60 Hz step toward `target` (the player's body centre).
    pub fn step(
        &mut self,
        e: &mut Enemy,
        target: [f32; 3],
        day: bool,
        servants_alive: usize,
    ) -> BossEvent {
        e.ai[0] = self.ai[0] as i32; // read by Enemy::defense / damage
        let half_h = e.kind.stats().half[1];
        let centre = |e: &Enemy| [e.pos[0], e.pos[1] + half_h, e.pos[2]];
        if day {
            e.vel[1] += FLEE_ACCEL * PX;
            self.advance(e);
            self.flee += 1;
            return if self.flee > FLEE_TICKS {
                BossEvent::Despawn
            } else {
                BossEvent::None
            };
        }
        let c = centre(e);
        let to = [target[0] - c[0], target[1] - c[1], target[2] - c[2]];
        let ai = &mut self.ai;
        if ai[0] == 0.0 {
            match ai[1] as i32 {
                0 => {
                    let goal = [to[0], to[1] + P1_HOVER_ABOVE * PX, to[2]];
                    let k = P1_HOVER_SPEED * PX / len(goal).max(1e-4);
                    steer(&mut e.vel, goal.map(|v| v * k), P1_HOVER_ACCEL * PX);
                    ai[2] += 1.0;
                    self.servant_timer += 1.0;
                    let mut spawn = None;
                    if self.servant_timer >= SERVANT_SPAWN_INTERVAL {
                        self.servant_timer = 0.0;
                        if servants_alive < MAX_SERVANTS {
                            let s_speed = 5.0 * PX;
                            let s_len = len(to).max(1e-4);
                            let vel = [
                                to[0] * s_speed / s_len,
                                to[1] * s_speed / s_len,
                                to[2] * s_speed / s_len,
                            ];
                            let pos = [
                                c[0] + vel[0] * 10.0,
                                c[1] + vel[1] * 10.0,
                                c[2] + vel[2] * 10.0,
                            ];
                            spawn = Some((pos, vel));
                        }
                    }
                    if ai[2] >= P1_HOVER_TICKS {
                        *ai = [0.0, 1.0, 0.0, 0.0];
                        self.servant_timer = 0.0;
                    }
                    if (e.hp as f32) < e.kind.stats().hp as f32 * PHASE2_LIFE {
                        *ai = [1.0, 0.0, 0.0, 0.0];
                        self.servant_timer = 0.0;
                    }
                    self.advance(e);
                    if let Some((pos, vel)) = spawn {
                        return BossEvent::SpawnServant { pos, vel };
                    }
                    return BossEvent::None;
                }
                1 => {
                    let k = P1_DASH_SPEED * PX / len(to).max(1e-4);
                    e.vel = to.map(|v| v * k);
                    ai[1] = 2.0;
                }
                _ => {
                    ai[2] += 1.0;
                    if ai[2] >= P1_DASH_FAST {
                        e.vel = e.vel.map(|v| v * P1_DASH_DECAY);
                        snap(&mut e.vel);
                    }
                    if ai[2] >= P1_DASH_TICKS {
                        ai[3] += 1.0;
                        ai[2] = 0.0;
                        if ai[3] >= DASHES {
                            ai[1] = 0.0;
                            ai[3] = 0.0;
                        } else {
                            ai[1] = 1.0;
                        }
                    }
                }
            }
            if (e.hp as f32) < e.kind.stats().hp as f32 * PHASE2_LIFE {
                *ai = [1.0, 0.0, 0.0, 0.0];
                self.servant_timer = 0.0;
            }
            self.advance(e);
            return BossEvent::None;
        }
        if ai[0] == 1.0 || ai[0] == 2.0 {
            ai[2] = (ai[2] + SPIN_STEP).min(SPIN_MAX);
            self.rotation += ai[2];
            ai[1] += 1.0;
            if ai[1] >= TRANSITION_STAGE_TICKS {
                ai[0] += 1.0;
                ai[1] = 0.0;
                if ai[0] == 3.0 {
                    ai[2] = 0.0;
                }
            }
            e.vel = e.vel.map(|v| v * TRANSITION_DRAG);
            snap(&mut e.vel);
            self.advance(e);
            return BossEvent::None;
        }
        match ai[1] as i32 {
            0 => {
                let goal = [to[0], to[1] + P2_HOVER_ABOVE * PX, to[2]];
                let k = P2_HOVER_SPEED * PX / len(goal).max(1e-4);
                steer(&mut e.vel, goal.map(|v| v * k), P2_HOVER_ACCEL * PX);
                ai[2] += 1.0;
                if ai[2] >= P2_HOVER_TICKS {
                    *ai = [3.0, 1.0, 0.0, 0.0];
                }
            }
            1 => {
                let k = P2_DASH_SPEED * PX / len(to).max(1e-4);
                e.vel = to.map(|v| v * k);
                ai[1] = 2.0;
            }
            _ => {
                ai[2] += 1.0;
                if ai[2] >= P2_DASH_FAST {
                    e.vel = e.vel.map(|v| v * P2_DASH_DECAY);
                    snap(&mut e.vel);
                }
                if ai[2] >= P2_DASH_TICKS {
                    ai[3] += 1.0;
                    ai[2] = 0.0;
                    if ai[3] >= DASHES {
                        ai[1] = 0.0;
                        ai[3] = 0.0;
                    } else {
                        ai[1] = 1.0;
                    }
                }
            }
        }
        e.ai[0] = ai[0] as i32;
        self.advance(e);
        BossEvent::None
    }

    /// `noTileCollide`: the Eye moves straight through blocks.
    fn advance(&self, e: &mut Enemy) {
        for a in 0..3 {
            e.pos[a] += e.vel[a];
        }
        e.ai[0] = self.ai[0] as i32;
    }
}

/// Whether the Suspicious Looking Eye may be used: only at night with no Eye alive
/// (gameplay spec case 43 / `NPC.SpawnOnPlayer` `AnyNPCs` check, gameplay spec).
pub fn can_summon(day: bool, boss_alive: bool) -> bool {
    !day && !boss_alive
}

/// Spawn position of a summoned or natural Eye: `SUMMON_HEIGHT` blocks above the player.
pub fn spawn_pos(player: [f32; 3]) -> [f32; 3] {
    [player[0], player[1] + SUMMON_HEIGHT, player[2]]
}

/// Night-start roll (this game's stand-in for gameplay spec): 1 in `NIGHT_SPAWN_ODDS`
/// when the player has at least `MIN_LIFE_MAX` max life.
pub fn natural_spawn_roll(life_max: i32, rng: &mut Rng) -> bool {
    life_max >= MIN_LIFE_MAX && rng.below(NIGHT_SPAWN_ODDS) == 0
}

/// Terraria-accurate natural Eye of Cthulhu spawn eligibility:
/// at least 200 max life, 10 defense, and 3 town NPCs.
pub fn natural_spawn_eligible(life_max: i32, defense: i32, town_npcs: usize) -> bool {
    life_max >= MIN_LIFE_MAX && defense >= MIN_DEFENSE && town_npcs >= MIN_TOWN_NPCS
}

/// Full night-start roll gated by `natural_spawn_eligible` and 1-in-`NIGHT_SPAWN_ODDS`.
pub fn natural_spawn_roll_full(
    life_max: i32,
    defense: i32,
    town_npcs: usize,
    rng: &mut Rng,
) -> bool {
    natural_spawn_eligible(life_max, defense, town_npcs) && rng.below(NIGHT_SPAWN_ODDS) == 0
}

/// Boss-only drops, `NPC.boss death drops` (gameplay spec):
/// 5-15 Lesser Healing Potions (`num = 28`, `Next(5, 16)`) and 5-9 Hearts
/// (`Next(5) + 5` of item 58). The Eye's own table (gameplay spec: masks,
/// trophy, Demonite/Crimtane ore, Unholy Arrows, seeds) uses items this game lacks and is
/// skipped. Coins come from `NPC.value` through `drop::loot_for`.
pub fn boss_extra_loot(rng: &mut Rng) -> Vec<ItemStack> {
    let mut out = vec![ItemStack {
        id: LESSER_HEALING_POTION,
        count: 5 + rng.below(11),
        prefix: Prefix(0),
    }];
    for _ in 0..5 + rng.below(5) {
        out.push(ItemStack {
            id: HEART,
            count: 1,
            prefix: Prefix(0),
        });
    }
    out
}

/// Demonite Ore dropped by Eye of Cthulhu (30..=90).
pub fn eye_demonite_loot(rng: &mut Rng) -> ItemStack {
    ItemStack {
        id: DEMONITE_ORE_ITEM,
        count: 30 + rng.below(61),
        prefix: Prefix(0),
    }
}

/// Fresh Eye at `pos` (feet centre).
pub fn new_enemy(id: crate::enemy::EnemyId, pos: [f32; 3]) -> Enemy {
    Enemy::new(id, EnemyKind::EyeOfCthulhu, pos)
}

/// Clears all Servants of Cthulhu when the boss dies or despawns.
pub fn clear_servants(enemies: &mut Vec<Enemy>) {
    enemies.retain(|e| e.kind != EnemyKind::ServantOfCthulhu);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enemy::EnemyId;

    fn eye() -> (EyeBoss, Enemy) {
        (EyeBoss::new(), new_enemy(EnemyId(1), [0.0, 40.0, 0.0]))
    }

    #[test]
    fn eye_stats_match_spec() {
        // gameplay spec
        let s = EnemyKind::EyeOfCthulhu.stats();
        assert_eq!((s.hp, s.damage, s.defense, s.value), (2800, 15, 12, 30000));
        assert_eq!(EnemyKind::EyeOfCthulhu.npc_id(), 4);
        assert!((s.half[0] - 50.0 * PX).abs() < 1e-6 && (s.half[1] - 55.0 * PX).abs() < 1e-6);
    }

    #[test]
    fn eye_starts_phase_one_and_enters_phase_two_below_half_life() {
        let (mut b, mut e) = eye();
        b.step(&mut e, [0.0, 10.0, 0.0], false, 0);
        assert_eq!(b.phase(), 1);
        assert_eq!(e.defense(), 12);
        e.hp = 1399;
        b.step(&mut e, [0.0, 10.0, 0.0], false, 0);
        assert_eq!(b.phase(), 2);
        for _ in 0..200 {
            b.step(&mut e, [0.0, 10.0, 0.0], false, 0);
        }
        assert_eq!(b.ai[0], 3.0);
        assert_eq!((e.defense(), e.damage()), (0, 23));
    }

    #[test]
    fn eye_dashes_toward_target_in_phase_one() {
        let (mut b, mut e) = eye();
        b.ai = [0.0, 1.0, 0.0, 0.0];
        let t = [20.0, 40.0, 0.0];
        b.step(&mut e, t, false, 0);
        assert_eq!(b.ai[1], 2.0);
        // 6 px/tick (gameplay spec) straight at the target (the move of this tick included).
        assert!((len(e.vel) - 6.0 * PX).abs() < 1e-5);
        assert!(e.vel[0] > 0.0);
        assert!(e.pos[0] > 0.0);
    }

    #[test]
    fn eye_hovers_above_the_player_before_dashing() {
        let (mut b, mut e) = eye();
        e.pos = [0.0, 10.0, 0.0];
        for _ in 0..300 {
            b.step(&mut e, [0.0, 10.0, 0.0], false, 0);
        }
        assert!(
            e.pos[1] > 10.0 + 5.0,
            "hovers ~12 blocks above, got {}",
            e.pos[1]
        );
        for _ in 0..300 {
            b.step(&mut e, [0.0, 10.0, 0.0], false, 0);
        }
        assert_eq!(b.ai[1], 1.0, "after 600 ticks of hovering it dashes");
        b.step(&mut e, [0.0, 10.0, 0.0], false, 0);
        assert_eq!(b.ai[1], 2.0);
    }

    #[test]
    fn eye_flees_at_day() {
        let (mut b, mut e) = eye();
        let y0 = e.pos[1];
        let mut ev = BossEvent::None;
        for _ in 0..20 {
            ev = b.step(&mut e, [0.0, 10.0, 0.0], true, 0);
            if ev == BossEvent::Despawn {
                break;
            }
        }
        assert_eq!(ev, BossEvent::Despawn);
        assert!(e.pos[1] > y0);
    }

    #[test]
    fn summon_rules_and_spawn_height() {
        assert!(!can_summon(true, false));
        assert!(!can_summon(false, true));
        assert!(can_summon(false, false));
        assert_eq!(spawn_pos([1.0, 2.0, 3.0]), [1.0, 42.0, 3.0]);
    }

    #[test]
    fn natural_spawn_requires_life_max_and_roll() {
        let mut rng = Rng::new(7);
        assert!((0..50).all(|_| !natural_spawn_roll(199, &mut rng)));
        let hits = (0..300)
            .filter(|_| natural_spawn_roll(200, &mut rng))
            .count();
        assert!((60..140).contains(&hits), "about 1 in 3, got {hits}");
    }

    #[test]
    fn natural_spawn_full_requires_life_defense_and_town_npcs() {
        assert!(!natural_spawn_eligible(100, 0, 1));
        assert!(!natural_spawn_eligible(200, 9, 3));
        assert!(!natural_spawn_eligible(200, 10, 2));
        assert!(!natural_spawn_eligible(199, 10, 3));
        assert!(natural_spawn_eligible(200, 10, 3));

        let mut rng = Rng::new(7);
        // Fresh world (Night 1: 100 HP, 0 defense, 1 town NPC) never rolls true.
        assert!((0..50).all(|_| !natural_spawn_roll_full(100, 0, 1, &mut rng)));
        // 200 HP with <10 defense or <3 town NPCs never rolls true.
        assert!((0..50).all(|_| !natural_spawn_roll_full(200, 9, 3, &mut rng)));
        assert!((0..50).all(|_| !natural_spawn_roll_full(200, 10, 2, &mut rng)));
        let hits = (0..300)
            .filter(|_| natural_spawn_roll_full(200, 10, 3, &mut rng))
            .count();
        assert!((60..140).contains(&hits), "about 1 in 3, got {hits}");
    }

    #[test]
    fn state_code_covers_all_boss_states() {
        let mut b = EyeBoss::new();
        assert_eq!(b.state_code(false), 0); // hovering
        assert_eq!(b.state_code(true), 3); // fleeing
        b.ai = [0.0, 1.0, 0.0, 0.0];
        assert_eq!(b.state_code(false), 1); // dashing
        b.ai = [1.0, 0.0, 0.0, 0.0];
        assert_eq!(b.state_code(false), 2); // transform stage 1
        b.ai = [2.0, 0.0, 0.0, 0.0];
        assert_eq!(b.state_code(false), 4); // transform stage 2
        assert_eq!(b.state_code(true), 3); // day flee overrides transform
    }

    #[test]
    fn boss_extra_loot_ranges() {
        let mut rng = Rng::new(3);
        for _ in 0..50 {
            let l = boss_extra_loot(&mut rng);
            assert_eq!(l[0].id, LESSER_HEALING_POTION);
            assert!((5..=15).contains(&l[0].count));
            assert!((5..=9).contains(&(l.len() - 1)));
        }
    }

    #[test]
    fn eye_demonite_loot_range() {
        let mut rng = Rng::new(42);
        let mut lo = u32::MAX;
        let mut hi = 0;
        for _ in 0..200 {
            let st = eye_demonite_loot(&mut rng);
            assert_eq!(st.id, DEMONITE_ORE_ITEM);
            assert!((30..=90).contains(&st.count));
            lo = lo.min(st.count);
            hi = hi.max(st.count);
        }
        assert!(
            lo <= 35 && hi >= 85,
            "expected spread across 30..=90, got {lo}..={hi}"
        );
    }

    #[test]
    fn servant_id_matches_terraria() {
        assert_eq!(EnemyKind::ServantOfCthulhu.npc_id(), 5);
        let s = EnemyKind::ServantOfCthulhu.stats();
        assert_eq!((s.hp, s.damage, s.defense, s.value), (8, 12, 0, 0));
        assert!((s.half[0] - 10.0 * PX).abs() < 1e-6 && (s.half[1] - 10.0 * PX).abs() < 1e-6);
    }

    #[test]
    fn servants_spawn_only_in_phase_one() {
        let (mut b, mut e) = eye();
        let mut spawns = 0;
        for _ in 0..600 {
            if let BossEvent::SpawnServant { .. } = b.step(&mut e, [0.0, 10.0, 0.0], false, 0) {
                spawns += 1;
            }
        }
        assert!(spawns > 0, "spawns servants in phase 1, got {spawns}");

        // Phase 2: life < 50%
        e.hp = 1000;
        b.step(&mut e, [0.0, 10.0, 0.0], false, 0);
        assert_eq!(b.phase(), 2);
        let mut p2_spawns = 0;
        for _ in 0..1000 {
            if let BossEvent::SpawnServant { .. } = b.step(&mut e, [0.0, 10.0, 0.0], false, 0) {
                p2_spawns += 1;
            }
        }
        assert_eq!(p2_spawns, 0, "no servants spawn in phase 2");
    }

    #[test]
    fn servant_cap_is_respected() {
        let (mut b, mut e) = eye();
        let mut spawns_at_cap = 0;
        for _ in 0..600 {
            if let BossEvent::SpawnServant { .. } = b.step(&mut e, [0.0, 10.0, 0.0], false, 8) {
                spawns_at_cap += 1;
            }
        }
        assert_eq!(
            spawns_at_cap, 0,
            "no servants spawn when cap of 8 is reached"
        );
    }

    #[test]
    fn servants_cleared_when_boss_dies() {
        let mut enemies = vec![
            Enemy::new(EnemyId(1), EnemyKind::ServantOfCthulhu, [0.0, 0.0, 0.0]),
            Enemy::new(EnemyId(2), EnemyKind::Zombie, [1.0, 0.0, 0.0]),
            Enemy::new(EnemyId(3), EnemyKind::ServantOfCthulhu, [2.0, 0.0, 0.0]),
        ];
        clear_servants(&mut enemies);
        assert_eq!(enemies.len(), 1);
        assert_eq!(enemies[0].kind, EnemyKind::Zombie);
    }
}
