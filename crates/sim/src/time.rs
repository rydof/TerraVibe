//! World clock: one 86,400-tick day/night cycle at 60 Hz.

/// Main.dayLength = 54000 (gameplay spec).
pub const DAY_TICKS: u32 = 54_000;
/// Main.nightLength = 32400 (gameplay spec).
pub const NIGHT_TICKS: u32 = 32_400;
pub const CYCLE_TICKS: u32 = DAY_TICKS + NIGHT_TICKS;

/// Ticks since the start of a day, wrapping every `CYCLE_TICKS`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorldClock {
    pub tick: u32,
}

impl WorldClock {
    pub fn advance(&mut self, n: u32) {
        self.tick = (self.tick + n % CYCLE_TICKS) % CYCLE_TICKS;
    }

    pub fn is_day(&self) -> bool {
        self.tick < DAY_TICKS
    }

    /// Progress through the current day or night phase, 0..1.
    pub fn day_fraction(&self) -> f32 {
        if self.is_day() {
            self.tick as f32 / DAY_TICKS as f32
        } else {
            (self.tick - DAY_TICKS) as f32 / NIGHT_TICKS as f32
        }
    }
}

/// Lowest sky-light multiplier, so the world stays readable at night.
pub const NIGHT_FLOOR: f32 = 0.15;
/// Dawn and dusk each last 3 of the 24 clock hours (4.5h-7.5h, 16.5h-19.5h).
/// Day starts at 4:30 (tick 0) and ends at 19:30 (`DAY_TICKS` = 15 h).
const RAMP_TICKS: f32 = DAY_TICKS as f32 * 3.0 / 15.0;

/// Sky-light multiplier: `NIGHT_FLOOR` at night, 1.0 from 7:30 to 16:30,
/// smoothstep ramps across dawn and dusk. Presentation only (client).
pub fn sky_brightness(tick: u32) -> f32 {
    let t = (tick % CYCLE_TICKS) as f32;
    let day = DAY_TICKS as f32;
    let k = if t >= day {
        0.0
    } else if t < RAMP_TICKS {
        t / RAMP_TICKS
    } else if t > day - RAMP_TICKS {
        (day - t) / RAMP_TICKS
    } else {
        1.0
    };
    let s = k * k * (3.0 - 2.0 * k);
    NIGHT_FLOOR + (1.0 - NIGHT_FLOOR) * s
}

/// Sun (day) or moon (night) arc angle in radians: 0 at the horizon where it
/// rises, pi/2 at its zenith, pi where it sets. Night restarts at 0.
pub fn sun_angle(tick: u32) -> f32 {
    let c = WorldClock {
        tick: tick % CYCLE_TICKS,
    };
    c.day_fraction() * std::f32::consts::PI
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sky_brightness_is_monotonic_through_dawn_and_dusk() {
        let mut prev = sky_brightness(0);
        for t in (0..=(DAY_TICKS / 5)).step_by(60) {
            let b = sky_brightness(t);
            assert!(b >= prev - 1e-6, "dawn dipped at {t}");
            prev = b;
        }
        let mut prev = sky_brightness(DAY_TICKS * 4 / 5);
        for t in (DAY_TICKS * 4 / 5..=DAY_TICKS).step_by(60) {
            let b = sky_brightness(t);
            assert!(b <= prev + 1e-6, "dusk rose at {t}");
            prev = b;
        }
    }

    #[test]
    fn sky_brightness_noon_is_one_midnight_is_floor() {
        assert_eq!(sky_brightness(DAY_TICKS / 2), 1.0);
        assert_eq!(sky_brightness(DAY_TICKS + NIGHT_TICKS / 2), NIGHT_FLOOR);
        assert_eq!(sky_brightness(0), NIGHT_FLOOR);
    }

    #[test]
    fn sky_brightness_wraps_at_cycle_end() {
        assert_eq!(sky_brightness(CYCLE_TICKS), sky_brightness(0));
        assert_eq!(sky_brightness(CYCLE_TICKS - 1), NIGHT_FLOOR);
        assert!(sun_angle(DAY_TICKS / 2) > 1.5 && sun_angle(DAY_TICKS / 2) < 1.65);
    }

    #[test]
    fn clock_rolls_over_at_54000() {
        assert!(WorldClock { tick: 53_999 }.is_day());
        assert!(!WorldClock { tick: 54_000 }.is_day());
        assert!(!WorldClock { tick: 86_399 }.is_day());
        let mut c = WorldClock { tick: 86_399 };
        c.advance(1);
        assert_eq!(c.tick, 0);
        assert!(c.is_day());
    }

    #[test]
    fn clock_advance_is_wrapping() {
        let mut c = WorldClock { tick: 86_000 };
        c.advance(1000);
        assert_eq!(c.tick, 600);
        c.advance(CYCLE_TICKS * 3 + 5);
        assert_eq!(c.tick, 605);
        assert_eq!(WorldClock { tick: 54_000 }.day_fraction(), 0.0);
    }
}
