//! Health, mana, fall damage and respawn, following classic 1.4-era gameplay specs
//! (Classic difficulty, singleplayer, no buffs or equipment).

use super::consts::{self, BREATH_MAX, TILE_BLOCKS};
use crate::mana::{ManaState, START_MANA_MAX};

/// Player.statLifeMax / statManaMax defaults.
pub const LIFE_MAX: i32 = 100;
pub const MANA_MAX: i32 = START_MANA_MAX;
/// Player.ItemCheck_UseLifeCrystal: `statLifeMax < 400` (gameplay spec).
pub const LIFE_CAP: i32 = 400;
/// ... adds 20 to statLifeMax and statLife (gameplay spec).
pub const LIFE_CRYSTAL_BONUS: i32 = 20;
/// Picking up a Heart (item 58) calls `Heal(20)` (gameplay spec).
pub const HEART_HEAL: i32 = 20;
/// Item.potionDelay = 3600 ticks = 60 s at 60 tps (gameplay spec).
pub const POTION_DELAY_TICKS: i32 = 3600;
/// Lesser Healing Potion `healLife` (gameplay spec, case 28).
pub const LESSER_HEALING_HEAL: i32 = 50;
/// Player.GetRespawnTime: 600 ticks with no boss, Classic.
pub const RESPAWN_TICKS: i32 = 600;
/// Player.Update fall check: `num10 = 25` tiles of free fall.
const SAFE_FALL_TILES: i32 = 25;

#[derive(Clone, Debug, PartialEq)]
pub struct Vitals {
    pub life: i32,
    pub life_max: i32,
    pub mana: i32,
    pub mana_max: i32,
    /// Player.lifeRegenTime: ticks since last damage, capped at 3600.
    pub life_regen_time: f32,
    /// Player.lifeRegenCount: accumulates toward 120 per life point.
    pub life_regen_count: i32,
    /// Player.manaRegenDelay: ticks before mana starts regenerating.
    pub mana_regen_delay: f32,
    /// Player.manaRegenCount: accumulates toward 120 per mana point.
    pub mana_regen_count: i32,
    /// Remaining ticks of Potion Sickness (BuffID.PotionSickness = 21, gameplay spec).
    pub potion_delay_ticks: i32,
    pub breath: i32,
    pub burn_ticks: i32,
    pub burning_ticks: i32,
    pub breath_cd: u32,
    pub lava_cd: u32,
    pub lava_time: i32,
    pub lava_max: i32,
    pub obsidian_skin_ticks: i32,
    pub ironskin_ticks: i32,
    pub swiftness_ticks: i32,
    pub regen_ticks: i32,
    pub shine_ticks: i32,
    pub water_walking_ticks: i32,
    pub campfire_active: bool,
    pub heart_lantern_active: bool,
}

impl Default for Vitals {
    fn default() -> Self {
        Self::new()
    }
}

impl Vitals {
    pub fn new() -> Self {
        Self {
            life: LIFE_MAX,
            life_max: LIFE_MAX,
            mana: MANA_MAX,
            mana_max: MANA_MAX,
            life_regen_time: 0.0,
            life_regen_count: 0,
            mana_regen_delay: 0.0,
            mana_regen_count: 0,
            potion_delay_ticks: 0,
            breath: BREATH_MAX,
            burn_ticks: 0,
            burning_ticks: 0,
            breath_cd: 0,
            lava_cd: 0,
            lava_time: 0,
            lava_max: 0,
            obsidian_skin_ticks: 0,
            ironskin_ticks: 0,
            swiftness_ticks: 0,
            regen_ticks: 0,
            shine_ticks: 0,
            water_walking_ticks: 0,
            campfire_active: false,
            heart_lantern_active: false,
        }
    }

    pub fn tick_obsidian_skin(&mut self) {
        self.obsidian_skin_ticks = (self.obsidian_skin_ticks - 1).max(0);
    }

    pub fn tick_buffs(&mut self) {
        self.ironskin_ticks = (self.ironskin_ticks - 1).max(0);
        self.swiftness_ticks = (self.swiftness_ticks - 1).max(0);
        self.regen_ticks = (self.regen_ticks - 1).max(0);
        self.shine_ticks = (self.shine_ticks - 1).max(0);
        self.water_walking_ticks = (self.water_walking_ticks - 1).max(0);
    }

    pub fn tick_lava(&mut self, in_lava: bool) {
        if in_lava {
            if self.obsidian_skin_ticks == 0 && self.lava_time > 0 {
                self.lava_time -= 1;
            }
        } else {
            self.lava_time = (self.lava_time + 1).min(self.lava_max);
        }
    }

    pub fn tick_breath(&mut self, submerged: bool) -> bool {
        if submerged {
            self.breath_cd += 1;
            if self.breath_cd >= consts::BREATH_TICKS {
                self.breath_cd = 0;
                if self.breath > 0 {
                    self.breath -= 1;
                    false
                } else {
                    self.hurt(consts::DROWN_DAMAGE);
                    true
                }
            } else {
                false
            }
        } else {
            self.breath_cd = 0;
            self.breath = (self.breath + consts::BREATH_RECOVER).min(consts::BREATH_MAX);
            false
        }
    }

    pub fn ignite(&mut self) {
        self.burn_ticks = consts::BURN_TICKS;
    }

    pub fn extinguish(&mut self) {
        self.burn_ticks = 0;
    }

    pub fn tick_burn(&mut self) -> bool {
        if self.burn_ticks > 0 {
            self.burn_ticks -= 1;
            if self.burn_ticks % 15 == 0 {
                self.life = (self.life - 1).max(0);
                return true;
            }
        }
        false
    }

    /// Hellstone / Hellstone Brick Burning debuff: 30 HP/s (1 damage per 2 ticks), silent.
    pub fn tick_burning(&mut self) -> bool {
        if self.burning_ticks > 0 {
            self.burning_ticks -= 1;
            if self.burning_ticks % 2 == 0 {
                self.life = (self.life - 1).max(0);
                return true;
            }
        }
        false
    }

    /// Life crystal: +20 max life and life while below the cap. False at the cap
    /// (nothing changes, so the caller keeps the item).
    pub fn use_crystal(&mut self) -> bool {
        if self.life_max >= LIFE_CAP {
            return false;
        }
        self.life_max += LIFE_CRYSTAL_BONUS;
        self.life += LIFE_CRYSTAL_BONUS;
        true
    }

    /// Mana crystal: +20 max mana and current mana up to MANA_CAP (200).
    pub fn use_mana_crystal(&mut self) -> bool {
        let mut m = self.mana_state();
        if !m.use_crystal() {
            return false;
        }
        self.sync_mana_state(m);
        true
    }

    /// Spend mana. Returns true if the player had enough mana and was debited.
    pub fn spend_mana(&mut self, amount: i32) -> bool {
        let mut m = self.mana_state();
        if !m.spend(amount) {
            return false;
        }
        self.sync_mana_state(m);
        true
    }

    /// Healing potion: heals and starts Potion Sickness. Refused (false, nothing
    /// changes) at full life or while sick, so sickness cannot be refreshed.
    pub fn use_potion(&mut self, heal: i32) -> bool {
        if self.life <= 0 || self.life >= self.life_max || self.potion_delay_ticks > 0 {
            return false;
        }
        self.heal(heal);
        self.potion_delay_ticks = POTION_DELAY_TICKS;
        true
    }

    /// Counts Potion Sickness down one tick.
    pub fn tick_potion(&mut self) {
        self.potion_delay_ticks = (self.potion_delay_ticks - 1).max(0);
    }

    /// Player.Heal: add life, clamped to the maximum (gameplay spec).
    pub fn heal(&mut self, amount: i32) {
        if self.life > 0 {
            self.life = (self.life + amount).min(self.life_max);
        }
    }

    /// Apply damage. Returns true only if this call killed the player.
    /// Player.Hurt resets lifeRegenTime.
    pub fn hurt(&mut self, amount: i32) -> bool {
        if self.life <= 0 || amount <= 0 {
            return false;
        }
        self.life = (self.life - amount).max(0);
        self.life_regen_time = 0.0;
        self.life == 0
    }

    pub fn mana_state(&self) -> ManaState {
        ManaState {
            mana: self.mana,
            mana_max: self.mana_max,
            regen_count: self.mana_regen_count,
            regen_delay: self.mana_regen_delay,
        }
    }

    pub fn sync_mana_state(&mut self, m: ManaState) {
        self.mana = m.mana;
        self.mana_max = m.mana_max;
        self.mana_regen_count = m.regen_count;
        self.mana_regen_delay = m.regen_delay;
    }

    /// Player.ApplyManaRegenerationDelay (after spending mana).
    pub fn apply_mana_delay(&mut self) {
        let mut m = self.mana_state();
        m.apply_delay();
        self.sync_mana_state(m);
    }

    /// Player.life regeneration, base path (no buffs, Classic).
    pub fn regen_life(&mut self, moving: bool) {
        self.regen_life_ext(moving, 0);
    }

    /// `regen_life` with an additional accumulation rate per tick (e.g. Band of Regeneration).
    pub fn regen_life_ext(&mut self, moving: bool, bonus_rate: i32) {
        self.life_regen_time += 1.0;
        let t = self.life_regen_time;
        let mut n = [300.0, 600.0, 900.0, 1200.0, 1500.0, 1800.0, 2400.0, 3000.0]
            .iter()
            .filter(|&&th| t >= th)
            .count() as f32;
        if t >= 3600.0 {
            n += 1.0;
            self.life_regen_time = 3600.0;
        }
        n *= if moving { 0.5 } else { 1.25 };
        n *= self.life_max as f32 / 400.0 * 0.85 + 0.15;
        self.life_regen_count += n.round_ties_even() as i32 + bonus_rate;
        while self.life_regen_count >= 120 {
            self.life_regen_count -= 120;
            self.life = (self.life + 1).min(self.life_max);
        }
    }

    /// Player.UpdateManaRegen, base path (no buffs, Classic).
    pub fn regen_mana(&mut self, moving: bool) {
        self.regen_mana_with_max(moving, self.mana_max);
    }

    /// Player.UpdateManaRegen with an effective mana cap (e.g. Band of Starpower).
    pub fn regen_mana_with_max(&mut self, moving: bool, effective_mana_max: i32) {
        let base_max = self.mana_max;
        let mut m = self.mana_state();
        m.mana_max = effective_mana_max;
        m.regen(moving);
        self.sync_mana_state(m);
        self.mana_max = base_max;
    }
}

/// Player.Update landing check: tiles fallen beyond 25 do 10 damage each.
/// Input is blocks fallen; converted to whole tiles first.
pub fn fall_damage(fall_blocks: f32) -> i32 {
    // +1e-3 absorbs f32 error so an exact N-tile fall (N * 0.686) isn't floored to N-1.
    let tiles = (fall_blocks / TILE_BLOCKS + 1e-3).floor() as i32;
    ((tiles - SAFE_FALL_TILES) * 10).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{AIR, BlockId, STONE};
    use crate::chunk::BlockQuery;
    use crate::player::{DeathCause, Player, PlayerEvent, PlayerInput, tick_player};

    struct Floor;
    impl BlockQuery for Floor {
        fn block(&self, _: i32, y: i32, _: i32) -> BlockId {
            if y < 0 { STONE } else { AIR }
        }
    }

    #[test]
    fn fall_damage_threshold() {
        assert_eq!(fall_damage(25.0 * 0.686 - 0.1), 0);
        assert_eq!(fall_damage(0.0), 0);
        // gameplay spec landing check (`num11 > num10`, `(num11 - num10) * 10`):
        // 35 tiles -> (35 - 25) * 10 = 100.
        assert_eq!(fall_damage(35.0 * 0.686), 100);
    }

    fn life_after(start: i32, n: usize, moving: bool) -> i32 {
        let mut v = Vitals::new();
        v.life = start;
        for _ in 0..n {
            v.regen_life(moving);
        }
        v.life
    }

    #[test]
    fn life_regen_matches_terraria() {
        // Standing, max 100: scale 0.3625. life regeneration rounds
        // 1.25*0.3625*k: k=2 (t 600..899) and k=3 (900..1199) give +1/tick,
        // k=4 (t=1200) gives +2. Total 600 + 2 = 602 -> 5 life (rem 2).
        assert_eq!(life_after(50, 1200, false), 55);
        assert_eq!(life_after(50, 599, false), 50);
        // Moving: 0.5*0.3625*k rounds to 0 until k=3 (t=900): +1/tick.
        // t=900..1200 is 301 ticks -> 2 life.
        assert_eq!(life_after(50, 1200, true), 52);
    }

    #[test]
    fn mana_regen_matches_terraria() {
        let run = |n: usize, moving: bool| {
            let mut v = Vitals::new();
            v.mana = 0;
            for _ in 0..n {
                v.regen_mana(moving);
            }
            v.mana
        };
        // Standing at 0/20: manaRegen = (7+7) * 0.5 = 7/tick; 7*18 = 126 >= 120.
        assert_eq!(run(17, false), 0);
        assert_eq!(run(18, false), 1);
        // Moving: 7 * 0.5 = 3/tick; 3*40 = 120.
        assert_eq!(run(39, true), 0);
        assert_eq!(run(40, true), 1);
    }

    #[test]
    fn mana_delay_blocks_regen() {
        let mut v = Vitals::new();
        v.mana = 0;
        v.apply_mana_delay(); // (1*240 + 45) * 0.7 = 199
        assert_eq!(v.mana_regen_delay, 199.0);
        for _ in 0..99 {
            v.regen_mana(false); // delay drops 2/tick when standing
        }
        assert_eq!(v.mana, 0);
    }

    #[test]
    fn damage_clamps_and_dead_ignores_damage() {
        let mut v = Vitals::new();
        assert!(v.hurt(1000));
        assert_eq!(v.life, 0);
        let snap = v.clone();
        assert!(!v.hurt(5));
        assert_eq!(v, snap);
    }

    #[test]
    fn potion_heals_and_starts_60s_sickness() {
        let mut v = Vitals::new();
        v.hurt(80);
        assert!(v.use_potion(LESSER_HEALING_HEAL));
        assert_eq!(v.life, 70);
        assert_eq!(v.potion_delay_ticks, 3600);
    }

    #[test]
    fn potion_refused_at_full_life() {
        let mut v = Vitals::new();
        assert!(!v.use_potion(50));
        assert_eq!(v.potion_delay_ticks, 0);
    }

    #[test]
    fn potion_refused_during_sickness() {
        let mut v = Vitals::new();
        v.hurt(90);
        assert!(v.use_potion(50));
        v.hurt(10);
        let life = v.life;
        assert!(!v.use_potion(50));
        assert_eq!((v.life, v.potion_delay_ticks), (life, 3600));
    }

    #[test]
    fn sickness_counts_down_and_expires() {
        let mut v = Vitals::new();
        v.hurt(90);
        v.use_potion(50);
        for _ in 0..3599 {
            v.tick_potion();
        }
        assert_eq!(v.potion_delay_ticks, 1);
        v.tick_potion();
        v.tick_potion();
        assert_eq!(v.potion_delay_ticks, 0);
        v.hurt(5);
        assert!(v.use_potion(50));
    }

    #[test]
    fn respawn_after_timer() {
        let spawn = [0.5, 0.0, 0.5];
        let mut p = Player::new(spawn);
        p.body.pos = [3.5, 0.0, 3.5];
        p.vitals.hurt(1000);
        p.dead = true;
        p.respawn_ticks = RESPAWN_TICKS;
        let idle = PlayerInput::default();
        for i in 1..RESPAWN_TICKS {
            let ev = tick_player(&Floor, &mut p, &idle);
            assert!(!ev.contains(&PlayerEvent::Respawned), "early at {i}");
            assert!(p.dead);
        }
        let ev = tick_player(&Floor, &mut p, &idle);
        assert!(ev.contains(&PlayerEvent::Respawned));
        assert!(!p.dead);
        assert_eq!(p.vitals.life, LIFE_MAX);
        assert_eq!(p.body.pos, spawn);
    }

    #[test]
    fn fall_damage_causes_fall_death() {
        let mut p = Player::new([0.0, 0.0, 0.0]);
        p.vitals.life = 10;
        let mut ev = Vec::new();
        p.kill(DeathCause::Fall, &mut ev);
        assert_eq!(
            ev,
            vec![PlayerEvent::Died {
                cause: DeathCause::Fall
            }]
        );
        assert!(p.dead);
        assert_eq!(p.respawn_ticks, RESPAWN_TICKS);
        assert_eq!(p.last_damage_cause, Some(DeathCause::Fall));
    }

    #[test]
    fn player_respawn_helper_resets_state() {
        let mut p = Player::new([10.0, 20.0, 30.0]);
        p.dead = true;
        p.respawn_ticks = 400;
        let mut ev = Vec::new();
        p.respawn(&mut ev);
        assert!(!p.dead);
        assert_eq!(p.respawn_ticks, 0);
        assert_eq!(p.body.pos, [10.0, 20.0, 30.0]);
        assert_eq!(ev, vec![PlayerEvent::Respawned]);
    }

    #[test]
    fn fall_hurts_and_kills() {
        let idle = PlayerInput::default();
        // 20 blocks ~ 29 tiles -> (29 - 25) * 10 = 40.
        let mut p = Player::new([0.5, 20.0, 0.5]);
        let mut hurt = 0;
        for _ in 0..300 {
            for e in tick_player(&Floor, &mut p, &idle) {
                if let PlayerEvent::Hurt { amount } = e {
                    hurt += amount;
                }
            }
        }
        assert_eq!(hurt, 40);
        assert!(p.vitals.life <= 60 && p.vitals.life > 0);
        // Lethal fall emits Died and freezes the body.
        let mut p = Player::new([0.5, 60.0, 0.5]);
        let mut died = 0;
        for _ in 0..400 {
            died += tick_player(&Floor, &mut p, &idle)
                .iter()
                .filter(|e| {
                    **e == PlayerEvent::Died {
                        cause: DeathCause::Fall,
                    }
                })
                .count();
        }
        assert_eq!(died, 1);
        assert!(p.dead);
        let y = p.body.pos[1];
        tick_player(&Floor, &mut p, &idle);
        assert_eq!(p.body.pos[1], y);
    }

    #[test]
    fn standing_start_takes_no_fall_damage() {
        let mut p = Player::new([0.5, 0.0, 0.5]);
        for _ in 0..60 {
            let ev = tick_player(&Floor, &mut p, &PlayerInput::default());
            assert!(ev.is_empty());
        }
        assert_eq!(p.vitals.life, LIFE_MAX);
    }

    #[test]
    fn crystal_use_adds_20_life_max_and_life() {
        let mut v = Vitals::new();
        v.life = 60;
        assert!(v.use_crystal());
        assert_eq!((v.life_max, v.life), (120, 80));
    }

    #[test]
    fn crystal_use_stops_at_400() {
        let mut v = Vitals::new();
        for _ in 0..15 {
            assert!(v.use_crystal());
        }
        assert_eq!(v.life_max, LIFE_CAP);
        let before = v.clone();
        assert!(!v.use_crystal());
        assert_eq!(v, before);
    }

    #[test]
    fn heart_heal_is_clamped_to_max() {
        let mut v = Vitals::new();
        v.life = 90;
        v.heal(HEART_HEAL);
        assert_eq!(v.life, 100);
    }

    #[test]
    fn breath_depletes_one_per_7_ticks_when_head_submerged() {
        let mut v = Vitals::new();
        assert_eq!(v.breath, consts::BREATH_MAX);
        for _ in 0..6 {
            assert!(!v.tick_breath(true));
            assert_eq!(v.breath, consts::BREATH_MAX);
        }
        assert!(!v.tick_breath(true));
        assert_eq!(v.breath, consts::BREATH_MAX - 1);
        for _ in 0..6 {
            assert!(!v.tick_breath(true));
            assert_eq!(v.breath, consts::BREATH_MAX - 1);
        }
        assert!(!v.tick_breath(true));
        assert_eq!(v.breath, consts::BREATH_MAX - 2);
    }

    #[test]
    fn breath_zero_hurts_two_every_7_ticks() {
        let mut v = Vitals::new();
        v.breath = 0;
        let initial_life = v.life;
        for _ in 0..6 {
            assert!(!v.tick_breath(true));
            assert_eq!(v.life, initial_life);
        }
        assert!(v.tick_breath(true));
        assert_eq!(v.life, initial_life - consts::DROWN_DAMAGE);
        for _ in 0..6 {
            assert!(!v.tick_breath(true));
            assert_eq!(v.life, initial_life - consts::DROWN_DAMAGE);
        }
        assert!(v.tick_breath(true));
        assert_eq!(v.life, initial_life - 2 * consts::DROWN_DAMAGE);
    }

    #[test]
    fn breath_recovers_3_per_tick_in_air() {
        let mut v = Vitals::new();
        v.breath = 100;
        assert!(!v.tick_breath(false));
        assert_eq!(v.breath, 103);
        assert!(!v.tick_breath(false));
        assert_eq!(v.breath, 106);

        v.breath = consts::BREATH_MAX - 1;
        assert!(!v.tick_breath(false));
        assert_eq!(v.breath, consts::BREATH_MAX);
    }

    #[test]
    fn burning_ticks_down_and_damages() {
        let mut v = Vitals::new();
        v.ignite();
        assert_eq!(v.burn_ticks, consts::BURN_TICKS);
        let initial_life = v.life;

        for _ in 0..14 {
            assert!(!v.tick_burn());
            assert_eq!(v.life, initial_life);
        }
        assert!(v.tick_burn());
        assert_eq!(v.burn_ticks, consts::BURN_TICKS - 15);
        assert_eq!(v.life, initial_life - 1);

        for _ in 0..14 {
            assert!(!v.tick_burn());
        }
        assert!(v.tick_burn());
        assert_eq!(v.life, initial_life - 2);
    }

    #[test]
    fn burning_dot_deals_30_hp_per_sec() {
        let mut v = Vitals::new();
        v.burning_ticks = 20;
        let mut dmg = 0;
        for _ in 0..20 {
            if v.tick_burning() {
                dmg += 1;
            }
        }
        assert_eq!(dmg, 10, "20 ticks at 30 HP/s = 10 dmg");
        assert_eq!(v.life, LIFE_MAX - 10);
    }

    #[test]
    fn lava_time_counts_down_and_refills() {
        let mut v = Vitals::new();
        v.lava_max = consts::LAVA_CHARM_TICKS;
        v.lava_time = consts::LAVA_CHARM_TICKS;
        for _ in 0..420 {
            v.tick_lava(true);
        }
        assert_eq!(v.lava_time, 0);
        for _ in 0..100 {
            v.tick_lava(false);
        }
        assert_eq!(v.lava_time, 100);
    }

    #[test]
    fn obsidian_skin_lasts_six_minutes() {
        let mut v = Vitals::new();
        v.obsidian_skin_ticks = consts::OBSIDIAN_SKIN_TICKS;
        for _ in 0..consts::OBSIDIAN_SKIN_TICKS {
            v.tick_obsidian_skin();
        }
        assert_eq!(v.obsidian_skin_ticks, 0);
    }
}
