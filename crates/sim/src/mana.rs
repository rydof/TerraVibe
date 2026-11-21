//! Mana stat, regeneration, and cap constants following classic 1.4-era gameplay specs.
//! (Classic difficulty, singleplayer, gameplay spec, 45750).

/// Default starting maximum mana for a new player or character (gameplay spec: statManaMax = 20).
pub const START_MANA_MAX: i32 = 20;

/// Maximum mana achievable with Mana Crystals (gameplay spec statManaMax < 200).
pub const MANA_CAP: i32 = 200;

/// Mana added per Mana Crystal consumed (gameplay spec statManaMax += 20).
pub const CRYSTAL_MANA: i32 = 20;

/// Calculates the initial mana regeneration delay in ticks after spending mana (gameplay spec).
pub fn mana_regen_delay(mana: i32, mana_max: i32) -> f32 {
    let max = mana_max.max(1);
    let ratio = (mana.clamp(0, max) as f32) / (max as f32);
    let n = (1.0 - ratio) * 60.0 * 4.0 + 45.0;
    (n * 0.7).trunc()
}

/// Calculates the per-tick mana regeneration rate into `manaRegenCount` (gameplay spec).
pub fn mana_regen_rate(mana: i32, mana_max: i32, moving: bool) -> i32 {
    let max = mana_max.max(1);
    let base = max / 3 + 1;
    let rate_base = if moving { base } else { base * 2 };
    let ratio = (mana.clamp(0, max) as f32) / (max as f32);
    let scale = 0.5 + 0.5 * ratio;
    let rate = (rate_base as f32 * scale) as i32;
    rate.max(2)
}

/// Mana pool state including regeneration delay and accumulation count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManaState {
    pub mana: i32,
    pub mana_max: i32,
    pub regen_count: i32,
    pub regen_delay: f32,
}

impl Default for ManaState {
    fn default() -> Self {
        Self::new(START_MANA_MAX, START_MANA_MAX)
    }
}

impl ManaState {
    pub const fn new(mana: i32, mana_max: i32) -> Self {
        Self {
            mana,
            mana_max,
            regen_count: 0,
            regen_delay: 0.0,
        }
    }

    /// Player.ApplyManaRegenerationDelay (gameplay spec).
    pub fn apply_delay(&mut self) {
        self.regen_delay = mana_regen_delay(self.mana, self.mana_max);
    }

    /// Player.UpdateManaRegen, base path (no buffs, Classic, gameplay spec).
    pub fn regen(&mut self, moving: bool) {
        if self.regen_delay > 0.0 {
            self.regen_delay -= 1.0;
            if !moving {
                self.regen_delay -= 1.0;
            }
        }
        let mut regen = 0;
        if self.regen_delay <= 0.0 {
            regen = mana_regen_rate(self.mana, self.mana_max, moving);
        }
        self.regen_count += regen;
        while self.regen_count >= 120 {
            self.regen_count -= 120;
            self.mana = (self.mana + 1).min(self.mana_max);
        }
    }

    /// Spend mana. Returns true if the player had enough mana and was debited.
    pub fn spend(&mut self, amount: i32) -> bool {
        if amount <= 0 {
            return true;
        }
        if self.mana >= amount {
            self.mana -= amount;
            self.apply_delay();
            true
        } else {
            false
        }
    }

    /// Restore mana up to the maximum.
    pub fn restore(&mut self, amount: i32) {
        if amount > 0 {
            self.mana = (self.mana + amount).min(self.mana_max);
        }
    }

    /// Consume a Mana Crystal: +20 max mana and +20 mana, up to MANA_CAP (200).
    pub fn use_crystal(&mut self) -> bool {
        if self.mana_max >= MANA_CAP {
            return false;
        }
        self.mana_max = (self.mana_max + CRYSTAL_MANA).min(MANA_CAP);
        self.mana = (self.mana + CRYSTAL_MANA).min(self.mana_max);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mana_starts_at_20() {
        let m = ManaState::default();
        assert_eq!(m.mana, 20);
        assert_eq!(m.mana_max, 20);
        assert_eq!(m.regen_count, 0);
        assert_eq!(m.regen_delay, 0.0);

        let c = crate::Character::new("Player", crate::character::Appearance::default());
        assert_eq!(c.mana, 20);
        assert_eq!(c.mana_max, 20);

        let v = crate::player::Vitals::new();
        assert_eq!(v.mana, 20);
        assert_eq!(v.mana_max, 20);
    }

    #[test]
    fn regen_matches_spec_formula() {
        // Table: (mana_max, standing_still, mana_missing, expected_ticks_for_1_mana)
        // Formula per Terraria gameplay spec:
        //   base = max / 3 + 1
        //   rate_base = if still { base * 2 } else { base }
        //   ratio = (max - missing) / max
        //   scale = 0.5 + 0.5 * ratio
        //   rate = max(2, int(rate_base * scale))
        //   expected_ticks = (120 + rate - 1) / rate
        let cases = [
            (20, true, 20, 18),   // mana 0/20 standing: rate=7, 18 ticks
            (20, false, 20, 40),  // mana 0/20 moving: rate=3, 40 ticks
            (20, true, 10, 12),   // mana 10/20 standing: rate=10, 12 ticks
            (20, false, 10, 24),  // mana 10/20 moving: rate=5, 24 ticks
            (20, false, 1, 20),   // mana 19/20 moving: rate=6, 20 ticks
            (200, true, 200, 2),  // mana 0/200 standing: rate=67, 2 ticks
            (200, false, 200, 4), // mana 0/200 moving: rate=33, 4 ticks
            (200, true, 100, 2),  // mana 100/200 standing: rate=100, 2 ticks
            (200, false, 100, 3), // mana 100/200 moving: rate=50, 3 ticks
            (200, false, 1, 2),   // mana 199/200 moving: rate=66, 2 ticks
        ];

        for (max, still, missing, expected_ticks) in cases {
            let start_mana = max - missing;
            let mut m = ManaState {
                mana: start_mana,
                mana_max: max,
                regen_count: 0,
                regen_delay: 0.0,
            };
            for tick in 1..expected_ticks {
                m.regen(!still);
                assert_eq!(
                    m.mana, start_mana,
                    "mana should not increase before tick {expected_ticks} (tick {tick}, max={max}, still={still}, missing={missing})"
                );
            }
            m.regen(!still);
            assert_eq!(
                m.mana,
                start_mana + 1,
                "mana should increase by 1 at tick {expected_ticks} (max={max}, still={still}, missing={missing})"
            );
        }
    }

    #[test]
    fn regen_paused_during_delay() {
        let mut m = ManaState::new(0, 20);
        m.apply_delay(); // (1.0 * 240 + 45) * 0.7 = 199.5 -> 199.0
        assert_eq!(m.regen_delay, 199.0);

        // Standing still decreases delay by 2.0 per tick
        for _ in 0..99 {
            m.regen(false); // moving = false (standing still)
        }
        assert_eq!(m.regen_delay, 1.0);
        assert_eq!(m.mana, 0);

        m.regen(false); // delay reaches <= 0.0, first regen tick accumulates rate=7
        assert!(m.regen_delay <= 0.0);
        assert_eq!(m.mana, 0);

        // Standing regen at 0/20 takes 18 ticks total; 1 tick ran, so 17 more to reach 1 mana
        for _ in 0..16 {
            m.regen(false);
            assert_eq!(m.mana, 0);
        }
        m.regen(false);
        assert_eq!(m.mana, 1);
    }

    #[test]
    fn mana_never_exceeds_max() {
        let mut m = ManaState::new(20, 20);
        for _ in 0..1000 {
            m.regen(false);
        }
        assert_eq!(m.mana, 20);

        m.restore(100);
        assert_eq!(m.mana, 20);

        // Raise mana to cap (200) using 9 crystals (+180 = 200)
        for _ in 0..9 {
            assert!(m.use_crystal());
        }
        assert_eq!(m.mana_max, 200);
        assert_eq!(m.mana, 200);

        // At cap, crystal returns false and mana/mana_max remain capped
        assert!(!m.use_crystal());
        assert_eq!(m.mana_max, 200);
        assert_eq!(m.mana, 200);

        for _ in 0..100 {
            m.regen(false);
        }
        assert_eq!(m.mana, 200);
    }

    #[test]
    fn mana_save_roundtrip() {
        let mut c = crate::Character::new("Mage", crate::character::Appearance::default());
        c.mana_max = 160;
        c.mana = 125;

        let bytes = c.to_bytes();
        let loaded = crate::Character::from_bytes(&bytes).expect("decode character");
        assert_eq!(loaded.mana_max, 160);
        assert_eq!(loaded.mana, 125);
    }

    // No migration test: the M4 character format (version 1, `character.rs`) has
    // stored `mana_max`/`mana` since it was introduced, so there is no legacy
    // character file without them. `mana_save_roundtrip` covers persistence and
    // `character.rs::roundtrip_preserves_everything` covers the whole file.
}
