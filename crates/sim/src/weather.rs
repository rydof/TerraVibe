//! Deterministic world weather state driven by world seed and `WorldClock`.

/// Current weather condition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WeatherKind {
    #[default]
    Clear,
    Rain,
    Storm,
}

impl WeatherKind {
    pub fn to_u8(self) -> u8 {
        match self {
            WeatherKind::Clear => 0,
            WeatherKind::Rain => 1,
            WeatherKind::Storm => 2,
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(WeatherKind::Clear),
            1 => Some(WeatherKind::Rain),
            2 => Some(WeatherKind::Storm),
            _ => None,
        }
    }
}

/// Authoritative weather state persisted with the world.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WeatherState {
    pub kind: WeatherKind,
    pub intensity: f32,
    pub ticks_remaining: u32,
}

const RAMP_STEP: f32 = 1.0 / 300.0;

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

impl WeatherState {
    /// Advance weather by one simulation tick deterministically from `seed` and `clock_ticks`.
    /// Returns `true` when `kind` changed.
    pub fn step(&mut self, seed: u32, clock_ticks: u32) -> bool {
        let mut changed = false;
        if self.ticks_remaining == 0 {
            let prev = self.kind;
            let key = ((seed as u64) << 32) ^ (clock_ticks as u64) ^ ((prev.to_u8() as u64) << 56);
            let h1 = splitmix64(key);
            let h2 = splitmix64(h1);
            let roll = (h1 % 100) as u32;
            let next_kind = match prev {
                WeatherKind::Clear => {
                    if roll < 45 {
                        WeatherKind::Clear
                    } else if roll < 85 {
                        WeatherKind::Rain
                    } else {
                        WeatherKind::Storm
                    }
                }
                WeatherKind::Rain => {
                    if roll < 55 {
                        WeatherKind::Clear
                    } else if roll < 80 {
                        WeatherKind::Storm
                    } else {
                        WeatherKind::Rain
                    }
                }
                WeatherKind::Storm => {
                    if roll < 60 {
                        WeatherKind::Clear
                    } else {
                        WeatherKind::Rain
                    }
                }
            };
            self.ticks_remaining = match next_kind {
                WeatherKind::Clear | WeatherKind::Rain => 3_600 + (h2 % 7_200) as u32,
                WeatherKind::Storm => 2_400 + (h2 % 4_800) as u32,
            };
            self.kind = next_kind;
            changed = next_kind != prev;
        } else {
            self.ticks_remaining -= 1;
        }

        let target = match self.kind {
            WeatherKind::Clear => 0.0,
            WeatherKind::Rain => 0.65,
            WeatherKind::Storm => 1.0,
        };
        let delta = target - self.intensity;
        if delta.abs() <= RAMP_STEP {
            self.intensity = target;
        } else {
            self.intensity += delta.signum() * RAMP_STEP;
        }
        self.intensity = self.intensity.clamp(0.0, 1.0);
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_transitions_deterministically_on_world_clock() {
        let mut a = WeatherState::default();
        let mut b = WeatherState::default();
        let mut saw_rain_or_storm = false;
        let mut saw_clear_after_wet = false;
        let mut was_wet = false;

        for tick in 0..crate::time::CYCLE_TICKS {
            a.step(42, tick);
            b.step(42, tick);
            assert_eq!(a, b, "diverged at tick {tick}");
            assert!(
                (0.0..=1.0).contains(&a.intensity),
                "intensity out of range at tick {tick}: {}",
                a.intensity
            );
            if matches!(a.kind, WeatherKind::Rain | WeatherKind::Storm) {
                saw_rain_or_storm = true;
                was_wet = true;
            } else if was_wet {
                saw_clear_after_wet = true;
            }
        }

        assert!(
            saw_rain_or_storm,
            "expected rain or storm during a full cycle"
        );
        assert!(
            saw_clear_after_wet,
            "expected weather to clear after rain/storm"
        );
    }
}
