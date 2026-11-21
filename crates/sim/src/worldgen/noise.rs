//! Hash-based value noise shared by worldgen. Only IEEE-exact f64 ops are used,
//! so output is bit-identical across platforms.

pub(super) fn hash(seed: u32, x: i32, y: i32, z: i32) -> u32 {
    let mut h = seed ^ 0x9E37_79B9;
    h = (h ^ x as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = (h ^ y as u32).wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    h = (h ^ z as u32).wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h
}

/// Lattice value in [-1, 1].
pub(super) fn lattice(seed: u32, x: i32, y: i32, z: i32) -> f64 {
    (hash(seed, x, y, z) >> 8) as f64 / ((1u32 << 24) - 1) as f64 * 2.0 - 1.0
}

pub(super) fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Trilinear value noise with smoothstep fade, in [-1, 1].
pub(super) fn value3(seed: u32, x: f64, y: f64, z: f64) -> f64 {
    let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
    let (ix, iy, iz) = (fx as i32, fy as i32, fz as i32);
    let (tx, ty, tz) = (smooth(x - fx), smooth(y - fy), smooth(z - fz));
    let l = |dx: i32, dy: i32, dz: i32| {
        lattice(
            seed,
            ix.wrapping_add(dx),
            iy.wrapping_add(dy),
            iz.wrapping_add(dz),
        )
    };
    let x00 = lerp(l(0, 0, 0), l(1, 0, 0), tx);
    let x10 = lerp(l(0, 1, 0), l(1, 1, 0), tx);
    let x01 = lerp(l(0, 0, 1), l(1, 0, 1), tx);
    let x11 = lerp(l(0, 1, 1), l(1, 1, 1), tx);
    lerp(lerp(x00, x10, ty), lerp(x01, x11, ty), tz)
}

/// Fractal sum of `octaves` value-noise layers, normalized to [-1, 1].
pub(super) fn fbm3(seed: u32, x: f64, y: f64, z: f64, freq: f64, octaves: u32) -> f64 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 1.0, 0.0, freq);
    for o in 0..octaves {
        let s = seed.wrapping_add(o.wrapping_mul(0x9E37_79B9));
        sum += amp * value3(s, x * f, y * f, z * f);
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    sum / norm
}

pub(super) fn fbm2(seed: u32, x: f64, z: f64, freq: f64, octaves: u32) -> f64 {
    fbm3(seed, x, 0.0, z, freq, octaves)
}
