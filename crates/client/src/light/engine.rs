//! Pure light engine: flood-fill a padded RGB light volume for one chunk.
//!
//! Numbers from the gameplay reference spec:
//! - `Terraria.Graphics.Light.LightingEngine.light decay` sets
//!   `LightDecayThroughAir = 0.91f` and `LightDecayThroughSolid = 0.56f`.
//! - `Terraria.Graphics.Light.LightingEngine.ProcessScan` inflates the scan
//!   area, and `LightMap.Blur` stops a channel below `0.0185`; this engine
//!   uses the plan's fixed 15-block border budget (`PAD`) instead, so the
//!   padded box covers every source that can light a neighbouring chunk in
//!   the common case. See the p5 ledger ruling R1.
//! - `Terraria.ID.TorchID.Initialize` gives the plain torch colour
//!   `(1f, 0.95f, 0.8f)`.
//!
//! The volume is computed in `u16` fixed point (8 fractional bits); the
//! public output is `[u8; 4]` per block; alpha is the sky-only
//! intensity (same decay, no torches) so the shader can scale sky light by the
//! time of day without a relight. That keeps a 62³ BFS
//! cheap while matching `round(255 * c * 0.91^d)` within one unit.

use std::collections::HashMap;
use std::sync::Arc;

use terravibe_sim::{
    CAMPFIRE, CHUNK_SIZE, Chunk, ChunkPos, GLOWING_MUSHROOM_PLANT, HEART_LANTERN, LiquidKind,
    METEORITE_ORE, MUSHROOM_GRASS, TORCH, World,
};

use super::{LIGHT_BORDER, LIGHT_IMAGE_SIZE};

/// Blocks a source can cross a chunk border and still be included. The plan's
/// fixed budget; see the module docs and ledger R1.
pub const PAD: i32 = 15;

/// `LightingEngine.light decay`: `LightDecayThroughAir = 0.91f`.
#[allow(dead_code)] // named reference value used by tests and docs
pub const AIR_DECAY: f32 = 0.91;
/// `LightingEngine.light decay`: `LightDecayThroughSolid = 0.56f`.
#[allow(dead_code)] // named reference value used by tests and docs
pub const SOLID_DECAY: f32 = 0.56;
/// `TorchID.Initialize`: plain torch `(1f, 0.95f, 0.8f)`.
pub const TORCH_LIGHT: [f32; 3] = [1.0, 0.95, 0.8];
/// Lava light emitted at full intensity from every lava cell.
pub const LAVA_LIGHT: [f32; 3] = [1.0, 0.45, 0.15];
/// Water extra decay per cell (darkening depth).
#[allow(dead_code)] // named reference value used by tests and docs
pub const WATER_DECAY: f32 = 0.85;

/// World height in blocks (`y` is 0..384).
pub const WORLD_HEIGHT: i32 = terravibe_sim::WORLD_HEIGHT as i32;
/// Padded box side: the chunk plus `PAD` blocks on every horizontal side.
const W: usize = (CHUNK_SIZE as usize) + 2 * (PAD as usize);
/// Padded box height: `PAD` blocks above and below the chunk too. The volume
/// used to span the full world height; the sky above the box is read from the
/// world column instead (see `sky_open_above`), which keeps the flood fill
/// small without darkening open sky just outside the box.
const H: usize = W;

// Fixed point: 0..=255 in 8 fractional bits.
const FULL: u16 = 255 * 256;
const AIR_MUL: u32 = 233; // round(0.91 * 256) = 232.96
const SOLID_MUL: u32 = 143; // round(0.56 * 256) = 143.36
const WATER_MUL: u32 = 218; // round(0.85 * 256) = 217.6
const SHIFT: u32 = 8;

/// A chunk's padded RGB light volume: the 32³ chunk plus one texel of border
/// (`-1..=32`) on every side, so face sampling at a chunk boundary reads the
/// neighbouring air cell instead of clamp-to-edge. 34³ texels, indexed
/// `(x+1) + (y+1)*34 + (z+1)*34*34` (x fastest); alpha = sky-only light.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LightVolume {
    pub data: Vec<[u8; 4]>,
}

impl LightVolume {
    /// Light at chunk-local `(x, y, z)`; panics outside 0..32. This is the
    /// public 32³ accessor; the border is available via [`Self::padded`].
    #[allow(dead_code)] // sampled by tests; release image upload reads `data`
    pub fn get(&self, x: usize, y: usize, z: usize) -> [u8; 3] {
        let n = CHUNK_SIZE as usize;
        assert!(x < n && y < n && z < n, "local coord out of range");
        self.padded(x as i32, y as i32, z as i32)
    }

    /// Light at padded local coordinates `-1..=32` (one block outside the
    /// chunk), as stored in the uploaded 34³ texture.
    #[allow(dead_code)] // sampled by tests; release image upload reads `data`
    pub fn padded(&self, x: i32, y: i32, z: i32) -> [u8; 3] {
        let side = LIGHT_IMAGE_SIZE as i32;
        let border = LIGHT_BORDER as i32;
        assert!(
            (-border..CHUNK_SIZE + border).contains(&x)
                && (-border..CHUNK_SIZE + border).contains(&y)
                && (-border..CHUNK_SIZE + border).contains(&z),
            "padded coord out of range"
        );
        let i = ((x + border) + (y + border) * side + (z + border) * side * side) as usize;
        let [r, g, b, _] = self.data[i];
        [r, g, b]
    }
}

/// True if no solid block sits between `y + 1` and the sky at `y = 128` in
/// column `(x, z)`. A missing chunk above counts as open sky, so a chunk that
/// arrives before its upper neighbour is lit as if the sky continued; it is
/// re-lit when that neighbour arrives.
#[allow(dead_code)]
pub fn sky_open_above(world: &World, x: i32, z: i32, y: i32) -> bool {
    if y < 0 {
        return false;
    }
    if y >= WORLD_HEIGHT - 1 {
        return true;
    }
    ((y + 1)..WORLD_HEIGHT).all(|yy| !world.block(x, yy, z).is_solid())
}

/// Chunks whose padded (`PAD`) region contains the block at world `changed`.
/// A block deep inside a chunk affects only that chunk; one within `PAD` of a
/// border also affects the neighbours on that side (so up to eight chunks at a
/// corner). Order is deterministic: `-y..=+y`, then `-z..=+z`, then `-x..=+x`.
pub fn relight_targets(changed: [i32; 3]) -> Vec<ChunkPos> {
    let c = ChunkPos::of_block(changed[0], changed[1], changed[2]);
    let inside = |cp: ChunkPos| {
        let axis = |chunk: i32, v: i32| {
            let lo = chunk * CHUNK_SIZE - PAD;
            let hi = chunk * CHUNK_SIZE + CHUNK_SIZE - 1 + PAD;
            (lo..=hi).contains(&v)
        };
        axis(cp.x, changed[0]) && axis(cp.y, changed[1]) && axis(cp.z, changed[2])
    };
    let mut out = Vec::with_capacity(8);
    for dy in -1..=1 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let p = ChunkPos {
                    x: c.x + dx,
                    y: c.y + dy,
                    z: c.z + dz,
                };
                if inside(p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// Chunks in a world column (`128 / 32`).
const WORLD_CHUNKS_Y: i32 = WORLD_HEIGHT / CHUNK_SIZE;

/// Every chunk below `pos` in the same column, nearest first. Sky light only
/// travels downward, so inserting or removing a chunk can change
/// [`sky_open_above`] for the chunks under it but never for those above.
/// Positions outside the world are not returned; the caller's `queue_light`
/// still skips chunks that are not generated.
pub fn relight_below(pos: ChunkPos) -> Vec<ChunkPos> {
    (0..pos.y.min(WORLD_CHUNKS_Y))
        .rev()
        .map(|y| ChunkPos {
            x: pos.x,
            y,
            z: pos.z,
        })
        .collect()
}

/// Chunks whose sky may change when the solidity of the block at `changed`
/// changes: the 3×3 chunk columns around the edited chunk, every layer below
/// the edited chunk. The edited layer and the layers above need no sky relight
/// (`sky_open_above` only reads upward) and are covered by
/// [`relight_targets`] / `queue_light_around`. Deterministic order: nearest
/// chunk layer first, then `-z..=+z`, then `-x..=+x`; deduplicated.
pub fn relight_below_edit(changed: [i32; 3]) -> Vec<ChunkPos> {
    let c = ChunkPos::of_block(changed[0], changed[1], changed[2]);
    let top = c.y.min(WORLD_CHUNKS_Y);
    let mut out = Vec::with_capacity(9 * top.max(0) as usize);
    let mut seen = std::collections::HashSet::new();
    for y in (0..top).rev() {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let p = ChunkPos {
                    x: c.x + dx,
                    y,
                    z: c.z + dz,
                };
                if seen.insert(p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

const CELL_AIR: u8 = 0;
const CELL_SOLID: u8 = 1;
const CELL_WATER: u8 = 2;
const CELL_TORCH: u8 = 3;
const CELL_LAVA: u8 = 4;
const CELL_CAMPFIRE: u8 = 5;
const CELL_HEART_LANTERN: u8 = 6;
const CELL_MUSHROOM: u8 = 7;
const CELL_MUSHROOM_GRASS: u8 = 8;
const CELL_METEORITE: u8 = 9;

/// Zero-clone compact snapshot for lighting one chunk on a worker thread.
pub struct LightInput {
    pub pos: ChunkPos,
    cells: Box<[u8]>,
    sky_open: Box<[u8]>,
}

#[inline]
fn pack_cell(chunk: &Chunk, lx: usize, ly: usize, lz: usize) -> u8 {
    let b = chunk.get(lx, ly, lz);
    if b == METEORITE_ORE {
        CELL_METEORITE
    } else if b == MUSHROOM_GRASS {
        CELL_MUSHROOM_GRASS
    } else if b.is_solid() {
        CELL_SOLID
    } else if b == TORCH {
        CELL_TORCH
    } else if b == CAMPFIRE {
        CELL_CAMPFIRE
    } else if b == HEART_LANTERN {
        CELL_HEART_LANTERN
    } else if b == GLOWING_MUSHROOM_PLANT {
        CELL_MUSHROOM
    } else if let Some(liq) = chunk.liquid(lx, ly, lz)
        && liq.amount >= 1
    {
        match liq.kind {
            LiquidKind::Lava => CELL_LAVA,
            LiquidKind::Water => CELL_WATER,
        }
    } else {
        CELL_AIR
    }
}

impl LightInput {
    /// Collects only the `Arc<Chunk>` pointers needed to pack `LightInput` for
    /// `pos` so packing (`62^3` cells + `sky_open`) can run off the main thread.
    pub fn collect_chunk_refs(
        pos: ChunkPos,
        generated: &HashMap<ChunkPos, Arc<Chunk>>,
    ) -> HashMap<ChunkPos, Arc<Chunk>> {
        let base_x = pos.x * CHUNK_SIZE - PAD;
        let base_y = pos.y * CHUNK_SIZE - PAD;
        let base_z = pos.z * CHUNK_SIZE - PAD;
        let x0 = base_x;
        let x1 = base_x + W as i32 - 1;
        let y0 = base_y;
        let y1 = base_y + H as i32 - 1;
        let z0 = base_z;
        let z1 = base_z + W as i32 - 1;
        let cy_lo = y0.div_euclid(CHUNK_SIZE);
        let cy_hi = if y1 >= WORLD_HEIGHT - 1 || y1 < 0 {
            y1.div_euclid(CHUNK_SIZE)
        } else {
            (WORLD_HEIGHT - 1).div_euclid(CHUNK_SIZE)
        };
        let mut out = HashMap::with_capacity(36);
        for cx in x0.div_euclid(CHUNK_SIZE)..=x1.div_euclid(CHUNK_SIZE) {
            for cz in z0.div_euclid(CHUNK_SIZE)..=z1.div_euclid(CHUNK_SIZE) {
                for cy in cy_lo..=cy_hi {
                    let cp = ChunkPos {
                        x: cx,
                        y: cy,
                        z: cz,
                    };
                    if let Some(c) = generated.get(&cp) {
                        out.insert(cp, Arc::clone(c));
                    }
                }
            }
        }
        out
    }

    /// Packs the 62³ cell flags and 62x62 `sky_open` column mask directly from
    /// `generated` without cloning any `Chunk`s.
    pub fn from_chunk_map(pos: ChunkPos, generated: &HashMap<ChunkPos, Arc<Chunk>>) -> Self {
        Self::build(pos, true, |cp| generated.get(&cp).map(|c| c.as_ref()))
    }

    /// Packs from a `World` (used by `light_chunk` for diorama / tests).
    pub fn from_world(pos: ChunkPos, world: &World) -> Self {
        Self::build(pos, false, |cp| world.chunk(cp))
    }

    fn build<'a>(
        pos: ChunkPos,
        underground_missing_is_solid: bool,
        get_chunk: impl Fn(ChunkPos) -> Option<&'a Chunk>,
    ) -> Self {
        let base_x = pos.x * CHUNK_SIZE - PAD;
        let base_y = pos.y * CHUNK_SIZE - PAD;
        let base_z = pos.z * CHUNK_SIZE - PAD;
        let mut cells = vec![CELL_AIR; W * H * W].into_boxed_slice();

        let x0 = base_x;
        let x1 = base_x + W as i32 - 1;
        let y0 = base_y;
        let y1 = base_y + H as i32 - 1;
        let z0 = base_z;
        let z1 = base_z + W as i32 - 1;

        for cy in y0.div_euclid(CHUNK_SIZE)..=y1.div_euclid(CHUNK_SIZE) {
            for cx in x0.div_euclid(CHUNK_SIZE)..=x1.div_euclid(CHUNK_SIZE) {
                for cz in z0.div_euclid(CHUNK_SIZE)..=z1.div_euclid(CHUNK_SIZE) {
                    let cp = ChunkPos {
                        x: cx,
                        y: cy,
                        z: cz,
                    };
                    let Some(chunk) = get_chunk(cp) else {
                        continue;
                    };
                    let cx0 = cp.x * CHUNK_SIZE;
                    let cy0 = cp.y * CHUNK_SIZE;
                    let cz0 = cp.z * CHUNK_SIZE;
                    let bx0 = cx0.max(x0);
                    let bx1 = (cx0 + CHUNK_SIZE - 1).min(x1);
                    let by0 = cy0.max(y0);
                    let by1 = (cy0 + CHUNK_SIZE - 1).min(y1);
                    let bz0 = cz0.max(z0);
                    let bz1 = (cz0 + CHUNK_SIZE - 1).min(z1);
                    for wy in by0..=by1 {
                        let ly = (wy - cy0) as usize;
                        let py = (wy - base_y) as usize;
                        for wz in bz0..=bz1 {
                            let lz = (wz - cz0) as usize;
                            let pz = (wz - base_z) as usize;
                            for wx in bx0..=bx1 {
                                let lx = (wx - cx0) as usize;
                                let px = (wx - base_x) as usize;
                                cells[index(px, py, pz)] = pack_cell(chunk, lx, ly, lz);
                            }
                        }
                    }
                }
            }
        }

        let mut sky_open = vec![0u8; W * W].into_boxed_slice();
        if y1 >= WORLD_HEIGHT - 1 {
            sky_open.fill(1);
        } else if y1 >= 0 {
            sky_open.fill(1);
            let cy_lo = (y1 + 1).div_euclid(CHUNK_SIZE);
            let cy_hi = (WORLD_HEIGHT - 1).div_euclid(CHUNK_SIZE);
            for cx in x0.div_euclid(CHUNK_SIZE)..=x1.div_euclid(CHUNK_SIZE) {
                let cx0 = cx * CHUNK_SIZE;
                let bx0 = cx0.max(x0);
                let bx1 = (cx0 + CHUNK_SIZE - 1).min(x1);
                for cz in z0.div_euclid(CHUNK_SIZE)..=z1.div_euclid(CHUNK_SIZE) {
                    let cz0 = cz * CHUNK_SIZE;
                    let bz0 = cz0.max(z0);
                    let bz1 = (cz0 + CHUNK_SIZE - 1).min(z1);
                    for cy in cy_lo..=cy_hi {
                        let Some(chunk) = get_chunk(ChunkPos {
                            x: cx,
                            y: cy,
                            z: cz,
                        }) else {
                            if underground_missing_is_solid
                                && cy < (terravibe_sim::SEA_LEVEL_Y - 32) / CHUNK_SIZE
                            {
                                for wz in bz0..=bz1 {
                                    let pz = (wz - base_z) as usize;
                                    for wx in bx0..=bx1 {
                                        let px = (wx - base_x) as usize;
                                        sky_open[px + pz * W] = 0;
                                    }
                                }
                            }
                            continue;
                        };
                        let cy0 = cy * CHUNK_SIZE;
                        let wy_lo = (y1 + 1).max(cy0);
                        let wy_hi = (WORLD_HEIGHT - 1).min(cy0 + CHUNK_SIZE - 1);
                        for wz in bz0..=bz1 {
                            let lz = (wz - cz0) as usize;
                            let pz = (wz - base_z) as usize;
                            for wx in bx0..=bx1 {
                                let px = (wx - base_x) as usize;
                                if sky_open[px + pz * W] == 0 {
                                    continue;
                                }
                                let lx = (wx - cx0) as usize;
                                for wy in wy_lo..=wy_hi {
                                    let ly = (wy - cy0) as usize;
                                    if chunk.get(lx, ly, lz).is_solid() {
                                        sky_open[px + pz * W] = 0;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Self {
            pos,
            cells,
            sky_open,
        }
    }
}

/// Computes the light volume for chunk `pos` from `world` (a snapshot with
/// the chunk and its neighbours). Missing chunks are air, except that the
/// sky continues above `y = 128`, as Terraria's sky does.
pub fn light_chunk(pos: ChunkPos, world: &World) -> LightVolume {
    let input = LightInput::from_world(pos, world);
    light_chunk_input(&input)
}

/// Computes the light volume from a pre-packed zero-clone [`LightInput`].
pub fn light_chunk_input(input: &LightInput) -> LightVolume {
    let pos = input.pos;
    let cells = &input.cells;
    let sky_open = &input.sky_open;

    // 2. Sky columns: full white above the highest solid/water block, if
    //    the real world column above the padded box is open.
    let mut light = vec![[0u16; 4]; cells.len()];
    let mut queue: Vec<u32> = Vec::with_capacity(cells.len() / 4);
    for pz in 0..W {
        for px in 0..W {
            if sky_open[px + pz * W] == 0 {
                continue;
            }
            let mut top = None;
            for py in (0..H).rev() {
                let c = cells[index(px, py, pz)];
                if c == CELL_SOLID
                    || c == CELL_WATER
                    || c == CELL_MUSHROOM_GRASS
                    || c == CELL_METEORITE
                {
                    top = Some(py);
                    break;
                }
            }
            let first = top.map_or(0, |t| t + 1);
            for py in first..H {
                let i = index(px, py, pz);
                light[i] = [FULL; 4];
                queue.push(i as u32);
            }
        }
    }

    // 3. Torches & Lava emitters.
    let torch = [
        FULL,
        (TORCH_LIGHT[1] * f32::from(FULL)).round() as u16,
        (TORCH_LIGHT[2] * f32::from(FULL)).round() as u16,
        0, // sky-only channel: torches add nothing
    ];
    let lava = [
        FULL,
        (LAVA_LIGHT[1] * f32::from(FULL)).round() as u16,
        (LAVA_LIGHT[2] * f32::from(FULL)).round() as u16,
        0, // sky-only channel: lava adds nothing
    ];
    let rgb_source = |r: f32, g: f32, b: f32| -> [u16; 4] {
        [
            (r * f32::from(FULL)).round() as u16,
            (g * f32::from(FULL)).round() as u16,
            (b * f32::from(FULL)).round() as u16,
            0,
        ]
    };
    let campfire = rgb_source(1.0, 0.72, 0.35);
    let heart_lantern = rgb_source(1.0, 0.38, 0.50);
    let mushroom = rgb_source(0.25, 0.55, 1.0);
    let mushroom_grass = rgb_source(0.18, 0.42, 0.85);
    let meteorite = rgb_source(0.75, 0.25, 0.08);
    for (i, &c) in cells.iter().enumerate() {
        let src = match c {
            CELL_TORCH => Some(torch),
            CELL_LAVA => Some(lava),
            CELL_CAMPFIRE => Some(campfire),
            CELL_HEART_LANTERN => Some(heart_lantern),
            CELL_MUSHROOM => Some(mushroom),
            CELL_MUSHROOM_GRASS => Some(mushroom_grass),
            CELL_METEORITE => Some(meteorite),
            _ => None,
        };
        if let Some(s) = src {
            let mut brighter = false;
            for ch in 0..3 {
                if s[ch] > light[i][ch] {
                    light[i][ch] = s[ch];
                    brighter = true;
                }
            }
            if brighter {
                queue.push(i as u32);
            }
        }
    }

    // 4. Flood fill. Air multiplies by 0.91 per block; water multiplies by 0.85;
    //    solid blocks take light at their face but do not pass it on. Liquid cells
    //    do not block light.
    let mut q = 0;
    while q < queue.len() {
        let i = queue[q] as usize;
        q += 1;
        let cur = light[i];
        let (px, py, pz) = decode(i);
        for (dx, dy, dz) in NEIGHBOURS {
            let nx = px as i32 + dx;
            let ny = py as i32 + dy;
            let nz = pz as i32 + dz;
            if !(0..W as i32).contains(&nx)
                || !(0..H as i32).contains(&ny)
                || !(0..W as i32).contains(&nz)
            {
                continue;
            }
            let ni = index(nx as usize, ny as usize, nz as usize);
            let cell = cells[ni];
            let solid = cell == CELL_SOLID || cell == CELL_MUSHROOM_GRASS || cell == CELL_METEORITE;
            let mul = if solid {
                SOLID_MUL
            } else if cell == CELL_WATER {
                WATER_MUL
            } else {
                AIR_MUL
            };
            let next = [
                scale(cur[0], mul),
                scale(cur[1], mul),
                scale(cur[2], mul),
                scale(cur[3], mul),
            ];
            let increased = next[0] > light[ni][0]
                || next[1] > light[ni][1]
                || next[2] > light[ni][2]
                || next[3] > light[ni][3];
            if !increased {
                continue;
            }
            light[ni] = [
                light[ni][0].max(next[0]),
                light[ni][1].max(next[1]),
                light[ni][2].max(next[2]),
                light[ni][3].max(next[3]),
            ];
            if !solid {
                queue.push(ni as u32);
            }
        }
    }

    // 5. Extract the chunk's own 32³ box plus one texel of border on every
    //    side (local -1..=32), quantised to u8. The border is what a face on a
    //    chunk boundary samples, so it sees the neighbouring air cell instead
    //    of clamp-to-edge repeating the boundary block.
    let side = LIGHT_IMAGE_SIZE as usize;
    let mut data = Vec::with_capacity(side * side * side);
    for lz in -(LIGHT_BORDER as i32)..=CHUNK_SIZE {
        for ly in -(LIGHT_BORDER as i32)..=CHUNK_SIZE {
            for lx in -(LIGHT_BORDER as i32)..=CHUNK_SIZE {
                let wy = pos.y * CHUNK_SIZE + ly;
                let rgba = if wy >= WORLD_HEIGHT {
                    // Above the world is open sky; a block at y=127 must not
                    // get a black top face.
                    [255, 255, 255, 255]
                } else if wy < 0 {
                    [0, 0, 0, 0]
                } else {
                    let c = light[index(
                        (lx + PAD) as usize,
                        (ly + PAD) as usize,
                        (lz + PAD) as usize,
                    )];
                    [to_u8(c[0]), to_u8(c[1]), to_u8(c[2]), to_u8(c[3])]
                };
                data.push(rgba);
            }
        }
    }
    LightVolume { data }
}

const NEIGHBOURS: [(i32, i32, i32); 6] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

fn index(px: usize, py: usize, pz: usize) -> usize {
    px + pz * W + py * W * W
}

fn decode(i: usize) -> (usize, usize, usize) {
    (i % W, i / (W * W), (i / W) % W)
}

fn scale(v: u16, mul: u32) -> u16 {
    ((u32::from(v) * mul) >> SHIFT) as u16
}

fn to_u8(v: u16) -> u8 {
    (((u32::from(v) + 128) >> SHIFT).min(255)) as u8
}

/// Builds a test world from sparse chunk entries.
#[cfg(test)]
fn world(chunks: impl IntoIterator<Item = (ChunkPos, Chunk)>) -> World {
    let mut w = World::new();
    for (p, c) in chunks {
        w.insert(p, c);
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::{AIR, STONE};

    fn cp(x: i32, y: i32, z: i32) -> ChunkPos {
        ChunkPos { x, y, z }
    }

    /// A stone world (layers 0..4) with a one-block tunnel along +x at
    /// `y = 16`, `z = 16`, and an optional torch in it.
    fn tunnel(torch_x: Option<i32>) -> World {
        let mut chunks = Vec::new();
        for y in 0..4 {
            for x in 0..2 {
                for z in 0..1 {
                    let mut c = Chunk::filled(STONE);
                    if y == 0 {
                        for lx in 0..32 {
                            c.set(lx, 16, 16, AIR);
                        }
                        if let Some(t) = torch_x.filter(|t| (0..32).contains(t)) {
                            c.set(t as usize, 16, 16, TORCH);
                        }
                    }
                    chunks.push((cp(x, y, z), c));
                }
            }
        }
        world(chunks)
    }

    #[test]
    fn alpha_is_sky_only_light() {
        // Torch in a tunnel: block light is not in the sky channel. Open sky: alpha full.
        let w = tunnel(Some(4));
        let v = light_chunk(cp(0, 0, 0), &w);
        let texel = |x: usize, y: usize, z: usize| (x + 1) + (y + 1) * 34 + (z + 1) * 34 * 34;
        // The torch adds nothing to alpha: it stays below the combined red.
        let t = v.data[texel(4, 16, 16)];
        assert!(t[3] < t[0], "{t:?}");
        let open = world([(cp(0, 0, 0), Chunk::filled(AIR))]);
        let v = light_chunk(cp(0, 0, 0), &open);
        assert_eq!(v.data[texel(16, 16, 16)][3], 255);
    }

    #[test]
    fn open_sky_full_at_surface() {
        // Stone up to y = 10 in layer 0; air above; no upper chunks means the
        // sky continues, as it does above the world.
        let mut c = Chunk::filled(STONE);
        for lx in 0..32 {
            for lz in 0..32 {
                for ly in 11..32 {
                    c.set(lx, ly, lz, AIR);
                }
            }
        }
        let w = world([(cp(0, 0, 0), c)]);
        assert!(sky_open_above(&w, 16, 16, 11));
        let v = light_chunk(cp(0, 0, 0), &w);
        assert_eq!(v.get(16, 11, 16), [255, 255, 255]);
    }

    #[test]
    fn sealed_cave_is_dark() {
        let mut c = Chunk::filled(STONE);
        for x in 10..15 {
            for y in 10..15 {
                for z in 10..15 {
                    c.set(x, y, z, AIR);
                }
            }
        }
        // Fill the layers above with stone too, so the pocket is sealed.
        let w = world([
            (cp(0, 0, 0), c),
            (cp(0, 1, 0), Chunk::filled(STONE)),
            (cp(0, 2, 0), Chunk::filled(STONE)),
            (cp(0, 3, 0), Chunk::filled(STONE)),
        ]);
        let v = light_chunk(cp(0, 0, 0), &w);
        assert_eq!(v.get(12, 12, 12), [0, 0, 0]);
    }

    #[test]
    fn torch_falloff_matches_terraria() {
        // LightingEngine.light decay: air decays by 0.91 per block;
        // TorchID.Initialize: plain torch (1, 0.95, 0.8).
        let w = tunnel(Some(4));
        let v = light_chunk(cp(0, 0, 0), &w);
        for d in 0..=10 {
            let got = v.get(4 + d, 16, 16);
            for (c, got_c) in TORCH_LIGHT.iter().zip(got) {
                let want = (255.0 * c * AIR_DECAY.powi(d as i32)).round() as i32;
                assert!(
                    (i32::from(got_c) - want).abs() <= 1,
                    "d={d} got={got:?} want~{want}"
                );
            }
        }
    }

    #[test]
    fn torch_light_crosses_border() {
        // Torch at world x=30, one block inside chunk 0. Chunk 1 starts at 32.
        let w = tunnel(Some(30));
        let a = light_chunk(cp(0, 0, 0), &w);
        let b = light_chunk(cp(1, 0, 0), &w);
        // The last block of chunk 0 and the first of chunk 1 are adjacent, so
        // the falloff steps once across the border, exactly as it does inside
        // one chunk. Chunk 1 sees the torch through its padded border.
        let want = |d: i32| (255.0 * TORCH_LIGHT[0] * AIR_DECAY.powi(d)).round() as i32;
        let at_a = i32::from(a.get(31, 16, 16)[0]);
        let at_b = i32::from(b.get(0, 16, 16)[0]);
        assert!(
            (at_b - want(2)).abs() <= 1,
            "light did not cross the border correctly: {at_b} want~{}",
            want(2)
        );
        assert!(
            (at_a - want(1)).abs() <= 1,
            "seam before the border: {at_a}"
        );
        assert!(at_b > 0, "light did not cross the border: {at_b}");
    }

    #[test]
    fn missing_upper_neighbour_is_sky() {
        let mut c = Chunk::filled(STONE);
        for lx in 0..32 {
            for lz in 0..32 {
                for ly in 11..32 {
                    c.set(lx, ly, lz, AIR);
                }
            }
        }
        // Only layer 0 exists; y=50 is therefore open sky.
        let w = world([(cp(0, 0, 0), c)]);
        assert!(sky_open_above(&w, 16, 16, 50));
        let v = light_chunk(cp(0, 0, 0), &w);
        assert_eq!(v.get(16, 20, 16), [255, 255, 255]);
    }

    #[test]
    fn relight_on_block_change() {
        // Local (16, 16, 16) is deep inside chunk 0: only its own chunk.
        assert_eq!(relight_targets([16, 16, 16]), vec![cp(0, 0, 0)]);

        // Local x = 0 is within PAD of the -x border: own and -x chunk.
        let edge = relight_targets([0, 16, 16]);
        assert_eq!(edge.len(), 2);
        assert!(edge.contains(&cp(0, 0, 0)) && edge.contains(&cp(-1, 0, 0)));

        // A block at the corner is inside the padded region of the eight
        // chunks on the -x/-y/-z side, each once and in a stable order.
        let corner = relight_targets([0, 0, 0]);
        let unique: std::collections::HashSet<_> = corner.iter().copied().collect();
        assert_eq!(corner.len(), 8);
        assert_eq!(unique.len(), 8);
        for x in [-1, 0] {
            for y in [-1, 0] {
                for z in [-1, 0] {
                    assert!(unique.contains(&cp(x, y, z)), "missing {x},{y},{z}");
                }
            }
        }
        assert_eq!(corner, relight_targets([0, 0, 0]));
    }

    #[test]
    fn insert_relights_only_chunks_below_the_same_column() {
        // Chunk layer 2 casts sky over layers 1 and 0, nearest first.
        assert_eq!(
            relight_below(cp(2, 2, -1)),
            vec![cp(2, 1, -1), cp(2, 0, -1)]
        );
        assert!(relight_below(cp(2, 0, -1)).is_empty());
        // The top in-world layer shadows all three below it, never above.
        assert_eq!(
            relight_below(cp(0, 3, 0)),
            vec![cp(0, 2, 0), cp(0, 1, 0), cp(0, 0, 0)]
        );
    }

    #[test]
    fn block_edit_relights_lower_chunks_in_3x3_columns() {
        // Local y = 16 in chunk layer 1 (world y = 48): the edited layer and
        // above are queue_light_around's job, the layer below is sky-only.
        let lower = relight_below_edit([16, 48, 16]);
        assert_eq!(lower.len(), 9);
        assert!(lower.iter().all(|p| p.y == 0));
        let unique: std::collections::HashSet<_> = lower.iter().copied().collect();
        assert_eq!(lower.len(), unique.len());
        for x in -1..=1 {
            for z in -1..=1 {
                assert!(unique.contains(&cp(x, 0, z)), "missing column {x},{z}");
            }
        }
        assert_eq!(lower, relight_below_edit([16, 48, 16]));

        // An edit in layer 0 has no lower layer.
        assert!(relight_below_edit([16, 16, 16]).is_empty());

        // A top-layer edit queues all three layers below, 27 unique chunks.
        let deep = relight_below_edit([16, 100, 16]);
        assert_eq!(deep.len(), 27);
        assert_eq!(
            deep.iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            27
        );
        assert!(deep[..9].iter().all(|p| p.y == 2));
        assert!(deep[9..18].iter().all(|p| p.y == 1));
        assert!(deep[18..].iter().all(|p| p.y == 0));
    }

    #[test]
    fn roof_blocks_sky_below_and_absence_restores_it() {
        // A solid roof at y = 100, covering the padded columns of chunk 0.
        let mut chunks = vec![(cp(0, 0, 0), Chunk::filled(AIR))];
        for x in -1..=1 {
            for z in -1..=1 {
                let mut roof = Chunk::filled(AIR);
                for lx in 0..CHUNK_SIZE as usize {
                    for lz in 0..CHUNK_SIZE as usize {
                        roof.set(lx, 4, lz, STONE);
                    }
                }
                chunks.push((cp(x, 3, z), roof));
            }
        }
        let roofed = world(chunks);
        assert!(!sky_open_above(&roofed, 16, 16, 46));
        let v = light_chunk(cp(0, 0, 0), &roofed);
        assert_eq!(v.get(16, 16, 16), [0, 0, 0], "roofed chunk should be dark");

        // Without the roof the same column is open to the sky.
        let open = world([(cp(0, 0, 0), Chunk::filled(AIR))]);
        assert!(sky_open_above(&open, 16, 16, 46));
        let v = light_chunk(cp(0, 0, 0), &open);
        assert_eq!(v.get(16, 16, 16), [255, 255, 255]);
    }

    #[test]
    fn padded_border_reads_neighbour_air_not_edge_block() {
        // Chunk 0 is solid, chunk -1 is open air. The local -1 border texel is
        // the neighbouring air cell, while local 0 is the solid edge block
        // (one solid-decay step darker).
        let w = world([
            (cp(-1, 0, 0), Chunk::filled(AIR)),
            (cp(0, 0, 0), Chunk::filled(STONE)),
        ]);
        let v = light_chunk(cp(0, 0, 0), &w);
        let neighbour = light_chunk(cp(-1, 0, 0), &w);
        assert_eq!(v.padded(-1, 16, 16), neighbour.get(31, 16, 16));
        assert_eq!(v.padded(-1, 16, 16), [255, 255, 255]);
        assert!(
            v.get(0, 16, 16)[0] < v.padded(-1, 16, 16)[0],
            "edge block must not be the border sample"
        );
        // Interior texels keep their 32³ values.
        for (x, y, z) in [(0, 16, 16), (7, 3, 29), (31, 31, 0)] {
            assert_eq!(v.padded(x, y, z), v.get(x as usize, y as usize, z as usize));
        }
    }

    #[test]
    fn padded_border_above_world_is_sky_lit() {
        // A chunk ending at y=WORLD_HEIGHT-1 has one border texel at y=WORLD_HEIGHT, outside the
        // world. That is open sky, not black, or blocks at the top lose their top-face light.
        let top_cy = WORLD_CHUNKS_Y - 1;
        let w = world([(cp(0, top_cy, 0), Chunk::filled(AIR))]);
        let v = light_chunk(cp(0, top_cy, 0), &w);
        assert_eq!(v.padded(16, 32, 16), [255, 255, 255]);
    }

    #[test]
    fn sky_above_padded_box_is_not_dark() {
        // Chunk 1's padded box tops out at world y = 78; terrain ends at
        // y = 39 and the real column above is open. The shortened volume must
        // read that column rather than going dark at its top.
        let mut c = Chunk::filled(AIR);
        for lx in 0..32 {
            for lz in 0..32 {
                for ly in 0..8 {
                    c.set(lx, ly, lz, STONE);
                }
            }
        }
        let air = Chunk::filled(AIR);
        let w = world([
            (cp(0, 1, 0), c),
            (cp(0, 2, 0), air.clone()),
            (cp(0, 3, 0), air),
        ]);
        let v = light_chunk(cp(0, 1, 0), &w);
        assert_eq!(v.get(16, 28, 16), [255, 255, 255]);
    }

    #[test]
    fn solid_above_padded_box_shadows_it() {
        // All air inside chunk 1, but chunk 2 (which contains the top of the
        // box, y = 78) is stone: no sky from above, so the box is dark. The
        // horizontal neighbours are present so the padded columns do not read
        // as missing (missing chunks count as open sky).
        let mut chunks = Vec::new();
        for x in -1..=1 {
            for z in -1..=1 {
                chunks.push((cp(x, 1, z), Chunk::filled(AIR)));
                chunks.push((cp(x, 2, z), Chunk::filled(STONE)));
            }
        }
        let w = world(chunks);
        let v = light_chunk(cp(0, 1, 0), &w);
        assert_eq!(v.get(16, 16, 16), [0, 0, 0]);
    }

    #[test]
    fn pad_radius_is_the_plan_budget() {
        // Ruling R1: the plan fixes PAD=15 rather than the 59 blocks that
        // 0.91^N < 1/255 would imply. The budget must still carry visible
        // light across a border (checked by torch_light_crosses_border).
        assert_eq!(PAD, 15);
        assert!((255.0 * AIR_DECAY.powi(PAD)).round() > 0.0);
    }

    #[test]
    fn fixed_point_matches_the_terraria_decay() {
        assert_eq!(AIR_MUL, (AIR_DECAY * 256.0).round() as u32);
        assert_eq!(SOLID_MUL, (SOLID_DECAY * 256.0).round() as u32);
        assert_eq!(WATER_MUL, (WATER_DECAY * 256.0).round() as u32);
    }

    #[test]
    #[ignore = "timing: run with --release --ignored"]
    fn light_chunk_speed() {
        let w = tunnel(Some(4));
        let start = std::time::Instant::now();
        for _ in 0..20 {
            std::hint::black_box(light_chunk(cp(0, 0, 0), &w));
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / 20.0;
        println!("light_chunk: {ms:.2} ms per surface chunk");
        assert!(ms < 15.0, "light_chunk too slow: {ms:.2} ms");
    }

    #[test]
    fn lava_cell_emits_light_in_engine() {
        // Sealed stone world with an air pocket at y=16, z=16, lx in 10..20.
        // Lava placed at (14, 16, 16).
        let mut chunks = Vec::new();
        for y in 0..4 {
            for x in 0..2 {
                for z in 0..1 {
                    let mut c = Chunk::filled(STONE);
                    if y == 0 && x == 0 {
                        for lx in 10..20 {
                            c.set(lx, 16, 16, AIR);
                        }
                        c.set_liquid(14, 16, 16, Some(terravibe_sim::Liquid::lava(255)));
                    }
                    chunks.push((cp(x, y, z), c));
                }
            }
        }
        let w = world(chunks);
        let v = light_chunk(cp(0, 0, 0), &w);
        let at_lava = v.get(14, 16, 16);
        assert!(
            (i32::from(at_lava[0]) - 255).abs() <= 1,
            "lava red full: got {}",
            at_lava[0]
        );
        let want_g = (255.0 * LAVA_LIGHT[1]).round() as i32;
        let want_b = (255.0 * LAVA_LIGHT[2]).round() as i32;
        assert!(
            (i32::from(at_lava[1]) - want_g).abs() <= 1,
            "lava green: got {}, want ~{}",
            at_lava[1],
            want_g
        );
        assert!(
            (i32::from(at_lava[2]) - want_b).abs() <= 1,
            "lava blue: got {}, want ~{}",
            at_lava[2],
            want_b
        );
        // Light decays into neighbouring air cell
        let at_next = v.get(15, 16, 16);
        assert!(at_next[0] < at_lava[0] && at_next[0] > 0);
    }

    #[test]
    fn water_darkens_light_per_cell() {
        // Tunnel with a torch at (4, 16, 16).
        // World A has air at (5, 16, 16).
        // World B has water at (5, 16, 16).
        let make_w = |water: bool| {
            let mut chunks = Vec::new();
            for y in 0..4 {
                for x in 0..2 {
                    for z in 0..1 {
                        let mut c = Chunk::filled(STONE);
                        if y == 0 {
                            for lx in 0..32 {
                                c.set(lx, 16, 16, AIR);
                            }
                            if x == 0 {
                                c.set(4, 16, 16, TORCH);
                                if water {
                                    c.set_liquid(
                                        5,
                                        16,
                                        16,
                                        Some(terravibe_sim::Liquid::water(255)),
                                    );
                                }
                            }
                        }
                        chunks.push((cp(x, y, z), c));
                    }
                }
            }
            world(chunks)
        };
        let va = light_chunk(cp(0, 0, 0), &make_w(false));
        let vb = light_chunk(cp(0, 0, 0), &make_w(true));
        let air_light = va.get(5, 16, 16)[0];
        let water_light = vb.get(5, 16, 16)[0];
        assert!(
            water_light < air_light,
            "water ({water_light}) must darken more than air ({air_light})"
        );
        let want_water = (255.0 * WATER_DECAY).round() as i32;
        assert!(
            (i32::from(water_light) - want_water).abs() <= 1,
            "got {water_light}, want ~{want_water}"
        );
    }
}
