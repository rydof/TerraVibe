//! Finite square world bounds and world size presets.

use crate::chunk::{CHUNK_SIZE, ChunkPos};

/// World size preset, matching Terraria's widths scaled by `TILE_BLOCKS` (0.686).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum WorldSize {
    #[default]
    Small,
    Medium,
    Large,
}

impl WorldSize {
    /// Half-extent in blocks from the world origin (`[-half, half)` on X and Z).
    /// Terraria widths 4200 / 6400 / 8400 tiles × 0.686 / 2 = 1440 / 2195 / 2880 blocks.
    pub const fn half_extent(self) -> i32 {
        match self {
            WorldSize::Small => 1440,
            WorldSize::Medium => 2195,
            WorldSize::Large => 2880,
        }
    }

    pub const fn to_u8(self) -> u8 {
        match self {
            WorldSize::Small => 0,
            WorldSize::Medium => 1,
            WorldSize::Large => 2,
        }
    }

    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(WorldSize::Small),
            1 => Some(WorldSize::Medium),
            2 => Some(WorldSize::Large),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            WorldSize::Small => "Small",
            WorldSize::Medium => "Medium",
            WorldSize::Large => "Large",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            WorldSize::Small => WorldSize::Medium,
            WorldSize::Medium => WorldSize::Large,
            WorldSize::Large => WorldSize::Small,
        }
    }
}

/// Ocean width in blocks inward from the world edge (Terraria ~250 tiles × 0.686 ≈ 170).
pub const OCEAN_WIDTH: i32 = 170;
/// Sandy beach ramp width in blocks inward from the ocean.
pub const BEACH_WIDTH: i32 = 40;
/// Sea level datum in world Y coordinates (just below normal inland minimum surface).
pub const SEA_LEVEL_Y: i32 = 40;

/// Square world bounds centred on the origin: `x, z` in `[-half, half)`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct WorldBounds {
    pub half: i32,
}

impl Default for WorldBounds {
    fn default() -> Self {
        Self::new(WorldSize::Small)
    }
}

impl WorldBounds {
    pub const fn new(size: WorldSize) -> Self {
        Self {
            half: size.half_extent(),
        }
    }

    pub const fn size(&self) -> WorldSize {
        if self.half >= 2880 {
            WorldSize::Large
        } else if self.half >= 2195 {
            WorldSize::Medium
        } else {
            WorldSize::Small
        }
    }

    /// True if column `(x, z)` lies within `[-half, half)`.
    #[inline]
    pub const fn contains_column(&self, x: i32, z: i32) -> bool {
        x >= -self.half && x < self.half && z >= -self.half && z < self.half
    }

    /// Clamps a body's X and Z position to `[-half + half_width, half - half_width]`
    /// and zeroes velocity on any clamped axis.
    #[inline]
    pub fn clamp_body(&self, pos: &mut [f32; 3], vel: &mut [f32; 3], half_width: f32) {
        let min = -(self.half as f32) + half_width;
        let max = (self.half as f32) - half_width;
        for axis in [0, 2] {
            if pos[axis] < min {
                pos[axis] = min;
                if vel[axis] < 0.0 {
                    vel[axis] = 0.0;
                }
            } else if pos[axis] > max {
                pos[axis] = max;
                if vel[axis] > 0.0 {
                    vel[axis] = 0.0;
                }
            }
        }
    }

    /// Distance in blocks from column `(x, z)` to the nearest of the 4 world borders.
    /// Non-negative inside `[-half, half)`, negative outside.
    #[inline]
    pub fn edge_distance(&self, x: i32, z: i32) -> i32 {
        let dx = (x + self.half).min(self.half - 1 - x);
        let dz = (z + self.half).min(self.half - 1 - z);
        dx.min(dz)
    }

    /// True if any column of chunk `c` lies inside `[-half, half)`.
    #[inline]
    pub const fn chunk_inside(&self, c: ChunkPos) -> bool {
        let x0 = c.x * CHUNK_SIZE;
        let x1 = x0 + CHUNK_SIZE;
        let z0 = c.z * CHUNK_SIZE;
        let z1 = z0 + CHUNK_SIZE;
        x1 > -self.half && x0 < self.half && z1 > -self.half && z0 < self.half
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_extents_match_terraria_widths() {
        assert_eq!(WorldSize::Small.half_extent(), 1440);
        assert_eq!(WorldSize::Medium.half_extent(), 2195);
        assert_eq!(WorldSize::Large.half_extent(), 2880);

        let b_small = WorldBounds::new(WorldSize::Small);
        assert_eq!(b_small.half, 1440);
        assert!(b_small.contains_column(0, 0));
        assert!(b_small.contains_column(-1440, 1439));
        assert!(!b_small.contains_column(1440, 0));
        assert!(!b_small.contains_column(-1441, 0));
        assert_eq!(b_small.edge_distance(-1440, 0), 0);
        assert_eq!(b_small.edge_distance(1439, 0), 0);
        assert_eq!(b_small.edge_distance(0, 0), 1439);
        assert!(b_small.chunk_inside(ChunkPos { x: -45, y: 0, z: 0 })); // -1440..-1408
        assert!(!b_small.chunk_inside(ChunkPos { x: -46, y: 0, z: 0 })); // -1472..-1440
        assert!(b_small.chunk_inside(ChunkPos { x: 44, y: 0, z: 0 })); // 1408..1440
        assert!(!b_small.chunk_inside(ChunkPos { x: 45, y: 0, z: 0 })); // 1440..1472
    }
}
