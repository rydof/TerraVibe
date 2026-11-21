//! Seeded biome map: a forest disk around spawn, ringed by six angular sectors
//! of desert, snow and jungle. Borders are domain-warped and blended.

use super::noise::{fbm2, hash};
use crate::world_bounds::WorldSize;
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Biome {
    Forest,
    Desert,
    Snow,
    Jungle,
}

/// Blend weights indexed by `Biome as usize`; they sum to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiomeWeights(pub [f32; 4]);

const FOREST_RADIUS: f64 = 300.0;
/// Half the blend band width, in blocks (the band is ~24 blocks wide).
const BLEND_HALF: f64 = 12.0;
const WARP_AMP: f64 = 80.0;
const WARP_FREQ: f64 = 1.0 / 400.0;
const SALT_WARP_X: u32 = 0x5A3C_91E7;
const SALT_WARP_Z: u32 = 0x1F0D_6B25;
const SALT_SECTOR: u32 = 0x7B2E_D4C9;
/// Outer ring in angular order; sector k spans `[k, k+1)` after the seed rotation.
const SECTORS: [Biome; 6] = [
    Biome::Desert,
    Biome::Forest,
    Biome::Snow,
    Biome::Forest,
    Biome::Jungle,
    Biome::Forest,
];

fn smoothstep01(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Monotone angle in `[0, 4)`, one unit per quarter turn. Uses only +, -, *, /
/// so it is bit-identical across platforms (no `atan2`).
fn pseudo_angle(x: f64, y: f64) -> f64 {
    let d = x.abs() + y.abs();
    if d == 0.0 {
        return 0.0;
    }
    let p = y / d;
    let a = if x >= 0.0 { p } else { 2.0 - p };
    a.rem_euclid(4.0)
}

pub fn biome_weights(seed: u32, x: i32, z: i32) -> BiomeWeights {
    biome_weights_for_size(seed, x, z, WorldSize::Small)
}

pub fn biome_weights_for_size(seed: u32, x: i32, z: i32, size: WorldSize) -> BiomeWeights {
    let scale = f64::from(size.half_extent()) / f64::from(WorldSize::Small.half_extent());
    let (xf, zf) = (f64::from(x) / scale, f64::from(z) / scale);
    let dx = WARP_AMP * fbm2(seed ^ SALT_WARP_X, xf, zf, WARP_FREQ, 3);
    let dz = WARP_AMP * fbm2(seed ^ SALT_WARP_Z, xf, zf, WARP_FREQ, 3);
    let (wx, wz) = (xf + dx, zf + dz);
    let r = (wx * wx + wz * wz).sqrt();

    // `s` is 0 inside the forest disk and 1 well outside it.
    let s = smoothstep01((r - FOREST_RADIUS + BLEND_HALF) / (2.0 * BLEND_HALF));
    let mut w = [0.0f64; 4];
    w[Biome::Forest as usize] += 1.0 - s;
    if s > 0.0 {
        // Sector coordinate in [0, 6), rotated per seed.
        let off = hash(seed ^ SALT_SECTOR, 0, 0, 0) as f64 / 4_294_967_296.0 * 6.0;
        let t = (pseudo_angle(wx, wz) * 1.5 + off).rem_euclid(6.0);
        // Nearest sector border. Arc length per unit is r * π/3 (a turn is 2π).
        let b = t.round();
        let arc = r * PI / 3.0;
        let up = smoothstep01(((t - b) * arc + BLEND_HALF) / (2.0 * BLEND_HALF));
        let bi = b as i64;
        let above = SECTORS[bi.rem_euclid(6) as usize];
        let below = SECTORS[(bi - 1).rem_euclid(6) as usize];
        w[above as usize] += s * up;
        w[below as usize] += s * (1.0 - up);
    }
    BiomeWeights([w[0] as f32, w[1] as f32, w[2] as f32, w[3] as f32])
}

pub fn dominant(w: &BiomeWeights) -> Biome {
    let mut best = 0;
    for i in 1..4 {
        if w.0[i] > w.0[best] {
            best = i;
        }
    }
    [Biome::Forest, Biome::Desert, Biome::Snow, Biome::Jungle][best]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sum(w: &BiomeWeights) -> f32 {
        w.0.iter().sum()
    }

    /// Radius at which each of 20 rays from spawn first leaves the forest.
    fn exit_radii(seed: u32) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        for i in 0..20 {
            // Offset so no ray lies on a radial sector border.
            let a = (i as f64 + 0.37) * PI * 2.0 / 20.0;
            let mut r = 200.0;
            while r < 600.0 {
                let w = biome_weights(seed, (a.cos() * r) as i32, (a.sin() * r) as i32);
                if dominant(&w) != Biome::Forest {
                    out.push((a, r));
                    break;
                }
                r += 1.0;
            }
        }
        out
    }

    #[test]
    fn spawn_is_forest_for_many_seeds() {
        for seed in 0..200 {
            let w = biome_weights(seed, 0, 0);
            assert_eq!(dominant(&w), Biome::Forest, "seed {seed}");
            assert_eq!(w.0[Biome::Forest as usize], 1.0, "seed {seed}");
        }
    }

    #[test]
    fn weights_sum_to_one_everywhere() {
        for seed in [1u32, 42] {
            for i in -143..=143 {
                for j in -143..=143 {
                    let w = biome_weights(seed, i * 7, j * 7);
                    assert!(w.0.iter().all(|v| !v.is_nan()), "NaN at {i},{j}");
                    assert!((sum(&w) - 1.0).abs() < 1e-4, "sum {} at {i},{j}", sum(&w));
                }
            }
        }
    }

    #[test]
    fn all_biomes_present() {
        let mut seen = [false; 4];
        for i in -150..=150 {
            for j in -150..=150 {
                let w = biome_weights(1, i * 10, j * 10);
                seen[dominant(&w) as usize] = true;
            }
        }
        assert!(seen.iter().all(|&s| s), "missing biome: {seen:?}");
    }

    #[test]
    fn border_not_straight() {
        let radii: Vec<f64> = exit_radii(1).into_iter().map(|(_, r)| r).collect();
        assert!(radii.len() >= 5, "too few crossings: {}", radii.len());
        let lo = radii.iter().cloned().fold(f64::MAX, f64::min);
        let hi = radii.iter().cloned().fold(f64::MIN, f64::max);
        assert!(hi - lo > 20.0, "border too straight: spread {}", hi - lo);
    }

    #[test]
    fn blend_band_width() {
        let exits = exit_radii(1);
        let mut bands = Vec::new();
        for (a, r) in exits {
            // Skip rays running along a radial sector border: there the outside
            // is a 50/50 blend, not a pure sector, so it is not the disk edge.
            let at = |rr: f64| biome_weights(1, (a.cos() * rr) as i32, (a.sin() * rr) as i32);
            if at(r + 60.0).0.iter().all(|&v| v < 0.99) {
                continue;
            }
            let mut run = 0;
            let mut longest = 0;
            let mut rr = r - 60.0;
            while rr < r + 60.0 {
                let w = biome_weights(1, (a.cos() * rr) as i32, (a.sin() * rr) as i32);
                // Only the disk-edge band (forest weight > 0): a ray running
                // along a radial sector border would otherwise never be "pure".
                if w.0[0] > 0.0 && w.0.iter().all(|&v| v <= 0.99) {
                    run += 1;
                    longest = longest.max(run);
                } else {
                    run = 0;
                }
                rr += 1.0;
            }
            // Where a sector border meets the disk edge the run can be longer.
            // Judge the typical band, not the worst corner.
            bands.push(longest);
        }
        assert!(bands.len() >= 5, "too few borders checked: {}", bands.len());
        bands.sort_unstable();
        let median = bands[bands.len() / 2];
        assert!(
            (10..=60).contains(&median),
            "median band {median}, all {bands:?}"
        );
    }

    #[test]
    fn spawn_is_forest_and_far_from_ocean() {
        use crate::world_bounds::{OCEAN_WIDTH, WorldBounds, WorldSize};
        for size in [WorldSize::Small, WorldSize::Medium, WorldSize::Large] {
            let bounds = WorldBounds::new(size);
            assert!(
                bounds.edge_distance(0, 0) > OCEAN_WIDTH + 500,
                "spawn too close to ocean for {size:?}"
            );
            for seed in [1u32, 7, 42] {
                let w = super::biome_weights_for_size(seed, 0, 0, size);
                assert_eq!(dominant(&w), Biome::Forest);
                assert_eq!(w.0[Biome::Forest as usize], 1.0);
            }
        }
    }

    #[test]
    fn desert_snow_jungle_corruption_all_inside_bounds_for_each_size() {
        use crate::world_bounds::{BEACH_WIDTH, OCEAN_WIDTH, WorldBounds, WorldSize};
        use crate::worldgen::corruption::corruption_at_for_size;
        for size in [WorldSize::Small, WorldSize::Medium, WorldSize::Large] {
            let bounds = WorldBounds::new(size);
            let inland = bounds.half - OCEAN_WIDTH - BEACH_WIDTH;
            for seed in [1u32, 7, 42] {
                let mut seen_biomes = [false; 4];
                let mut seen_corruption = false;
                for x in (-inland..inland).step_by(32) {
                    for z in (-inland..inland).step_by(32) {
                        let w = super::biome_weights_for_size(seed, x, z, size);
                        seen_biomes[dominant(&w) as usize] = true;
                        if corruption_at_for_size(seed, x, z, size) > 0.1 {
                            seen_corruption = true;
                        }
                    }
                }
                assert!(
                    seen_biomes.iter().all(|&b| b),
                    "missing biome for {size:?} seed {seed}: {seen_biomes:?}"
                );
                assert!(
                    seen_corruption,
                    "missing corruption inside bounds for {size:?} seed {seed}"
                );
            }
        }
    }
}
