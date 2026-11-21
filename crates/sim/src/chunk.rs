//! 32³ chunks and the sparse chunk map.

use std::collections::HashMap;

use crate::block::{AIR, BlockId};
use crate::codec::{DecodeError, Reader, Writer};
use crate::liquid_cell::{Liquid, LiquidKind};

pub const CHUNK_SIZE: i32 = 32;
const CS: usize = CHUNK_SIZE as usize;
const VOLUME: usize = CS * CS * CS;

/// Chunk coordinates (block coordinates floor-divided by `CHUNK_SIZE`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ChunkPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ChunkPos {
    pub fn of_block(x: i32, y: i32, z: i32) -> ChunkPos {
        ChunkPos {
            x: x.div_euclid(CHUNK_SIZE),
            y: y.div_euclid(CHUNK_SIZE),
            z: z.div_euclid(CHUNK_SIZE),
        }
    }
}

/// A 32³ block volume. Layout: index = lx + ly*32 + lz*32*32 (x fastest).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Chunk {
    blocks: Box<[BlockId]>,
    /// Background walls, 1 bit per cell (same index as `blocks`). Allocated on
    /// the first wall and dropped again when the last one goes, so chunks
    /// without walls cost nothing and compare equal to freshly generated ones.
    walls: Option<Box<[u64]>>,
    /// Per-cell liquid layer. Allocated on the first cell with amount > 0;
    /// dropped when the last liquid cell is cleared.
    liquid: Option<Box<LiquidLayer>>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct LiquidLayer {
    amounts: [u8; VOLUME],
    kind_bits: [u64; VOLUME / 64], // 0 = water, 1 = lava
    non_zero: usize,
}

impl LiquidLayer {
    fn new_boxed() -> Box<Self> {
        Box::new(LiquidLayer {
            amounts: [0; VOLUME],
            kind_bits: [0; VOLUME / 64],
            non_zero: 0,
        })
    }
}

const WALL_WORDS: usize = VOLUME / 64;

impl Chunk {
    pub fn filled(b: BlockId) -> Self {
        Chunk {
            blocks: vec![b; VOLUME].into_boxed_slice(),
            walls: None,
            liquid: None,
        }
    }

    #[inline]
    fn index(lx: usize, ly: usize, lz: usize) -> usize {
        assert!(lx < CS && ly < CS && lz < CS, "local coord out of range");
        lx + ly * CS + lz * CS * CS
    }

    /// Panics if any local coordinate is >= 32.
    pub fn get(&self, lx: usize, ly: usize, lz: usize) -> BlockId {
        self.blocks[Self::index(lx, ly, lz)]
    }

    /// Whether cell `(lx, ly, lz)` has a background wall. Panics if out of range.
    pub fn wall(&self, lx: usize, ly: usize, lz: usize) -> bool {
        let i = Self::index(lx, ly, lz);
        self.walls
            .as_ref()
            .is_some_and(|w| w[i / 64] >> (i % 64) & 1 == 1)
    }

    /// Whether any cell has a wall (lets the mesher skip wall-free chunks).
    pub fn has_walls(&self) -> bool {
        self.walls.is_some()
    }

    /// Sets or clears a wall bit. Panics if out of range.
    pub fn set_wall(&mut self, lx: usize, ly: usize, lz: usize, on: bool) {
        let i = Self::index(lx, ly, lz);
        if on {
            let w = self
                .walls
                .get_or_insert_with(|| vec![0u64; WALL_WORDS].into_boxed_slice());
            w[i / 64] |= 1 << (i % 64);
        } else if let Some(w) = self.walls.as_mut() {
            w[i / 64] &= !(1 << (i % 64));
            if w.iter().all(|&x| x == 0) {
                self.walls = None;
            }
        }
    }

    pub fn liquid(&self, lx: usize, ly: usize, lz: usize) -> Option<Liquid> {
        let i = Self::index(lx, ly, lz);
        let layer = self.liquid.as_ref()?;
        let amount = layer.amounts[i];
        if amount == 0 {
            return None;
        }
        let kind = if (layer.kind_bits[i / 64] >> (i % 64)) & 1 == 1 {
            LiquidKind::Lava
        } else {
            LiquidKind::Water
        };
        Some(Liquid { kind, amount })
    }

    pub fn set_liquid(&mut self, lx: usize, ly: usize, lz: usize, l: Option<Liquid>) {
        let i = Self::index(lx, ly, lz);
        match l {
            Some(liquid) if liquid.amount > 0 => {
                let layer = self.liquid.get_or_insert_with(LiquidLayer::new_boxed);
                if layer.amounts[i] == 0 {
                    layer.non_zero += 1;
                }
                layer.amounts[i] = liquid.amount;
                match liquid.kind {
                    LiquidKind::Water => layer.kind_bits[i / 64] &= !(1u64 << (i % 64)),
                    LiquidKind::Lava => layer.kind_bits[i / 64] |= 1u64 << (i % 64),
                }
            }
            _ => {
                if let Some(layer) = self.liquid.as_mut() {
                    if layer.amounts[i] != 0 {
                        layer.amounts[i] = 0;
                        layer.kind_bits[i / 64] &= !(1u64 << (i % 64));
                        layer.non_zero -= 1;
                        if layer.non_zero == 0 {
                            self.liquid = None;
                        }
                    }
                }
            }
        }
    }

    pub fn has_liquid(&self) -> bool {
        self.liquid.is_some()
    }

    /// Raw slice of per-cell liquid amounts (`32*32*32`), or `None` if the chunk has no liquid.
    pub fn liquid_amounts(&self) -> Option<&[u8; VOLUME]> {
        self.liquid.as_ref().map(|l| &l.amounts)
    }

    /// Run-length encoded blocks: `(run: u32, id: u16)` pairs covering the volume,
    /// then (only when walls exist) a `1u8` flag and the 4096-byte wall bitset,
    /// then (only when liquid exists) a `2u8` flag and RLE `(run: u32, packed: u16)` pairs.
    /// Files written before walls/liquids end earlier and decode with none.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        let mut i = 0;
        while i < VOLUME {
            let b = self.blocks[i];
            let mut n = 1;
            while i + n < VOLUME && self.blocks[i + n] == b {
                n += 1;
            }
            w.u32(n as u32);
            w.u16(b.0);
            i += n;
        }
        if let Some(bits) = &self.walls {
            w.u8(1);
            for word in bits.iter() {
                w.bytes(&word.to_le_bytes());
            }
        }
        if let Some(layer) = &self.liquid {
            w.u8(2);
            let packed_at = |idx: usize| -> u16 {
                let amount = layer.amounts[idx];
                if amount == 0 {
                    0u16
                } else {
                    let kind_bit = (layer.kind_bits[idx / 64] >> (idx % 64)) & 1;
                    ((kind_bit as u16) << 8) | (amount as u16)
                }
            };
            let mut idx = 0;
            while idx < VOLUME {
                let p = packed_at(idx);
                let mut n = 1;
                while idx + n < VOLUME && packed_at(idx + n) == p {
                    n += 1;
                }
                w.u32(n as u32);
                w.u16(p);
                idx += n;
            }
        }
        w.0
    }

    /// Inverse of `to_bytes`; rejects runs that do not cover exactly the volume.
    pub fn from_bytes(bytes: &[u8]) -> Result<Chunk, DecodeError> {
        let mut r = Reader(bytes);
        let mut blocks = Vec::with_capacity(VOLUME);
        while blocks.len() < VOLUME {
            let n = r.u32()? as usize;
            let b = BlockId(r.u16()?);
            if n == 0 || blocks.len() + n > VOLUME {
                return Err(DecodeError);
            }
            blocks.resize(blocks.len() + n, b);
        }
        let mut walls = None;
        let mut liquid = None;
        while !r.is_empty() {
            match r.u8()? {
                1 => {
                    if walls.is_some() {
                        return Err(DecodeError);
                    }
                    let mut bits = vec![0u64; WALL_WORDS];
                    for b in bits.iter_mut() {
                        *b = u64::from_le_bytes(r.take(8)?.try_into().unwrap());
                    }
                    walls = bits
                        .iter()
                        .any(|&x| x != 0)
                        .then(|| bits.into_boxed_slice());
                }
                2 => {
                    if liquid.is_some() {
                        return Err(DecodeError);
                    }
                    let mut layer = LiquidLayer::new_boxed();
                    let mut count = 0;
                    while count < VOLUME {
                        let run = r.u32()? as usize;
                        let packed = r.u16()?;
                        if run == 0 || count + run > VOLUME {
                            return Err(DecodeError);
                        }
                        if let Some(l) = Liquid::unpack(packed) {
                            layer.non_zero += run;
                            for idx in count..(count + run) {
                                layer.amounts[idx] = l.amount;
                                if l.kind == LiquidKind::Lava {
                                    layer.kind_bits[idx / 64] |= 1u64 << (idx % 64);
                                }
                            }
                        }
                        count += run;
                    }
                    if layer.non_zero > 0 {
                        liquid = Some(layer);
                    }
                }
                _ => return Err(DecodeError),
            }
        }
        Ok(Chunk {
            blocks: blocks.into_boxed_slice(),
            walls,
            liquid,
        })
    }

    /// Panics if any local coordinate is >= 32.
    pub fn set(&mut self, lx: usize, ly: usize, lz: usize, b: BlockId) {
        self.blocks[Self::index(lx, ly, lz)] = b;
    }
}

/// Sparse map of loaded chunks.
#[derive(Clone, Default)]
pub struct World {
    chunks: HashMap<ChunkPos, Chunk>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, pos: ChunkPos, chunk: Chunk) {
        self.chunks.insert(pos, chunk);
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&pos)
    }

    pub fn chunk_mut(&mut self, pos: ChunkPos) -> Option<&mut Chunk> {
        self.chunks.get_mut(&pos)
    }

    /// Block at world coordinates; `AIR` if the chunk isn't loaded. Never panics.
    pub fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
        let Some(c) = self.chunks.get(&ChunkPos::of_block(x, y, z)) else {
            return AIR;
        };
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        c.get(l(x), l(y), l(z))
    }

    /// Sets the block at world coordinates; returns `false` if chunk is not loaded.
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, b: BlockId) -> bool {
        let Some(c) = self.chunks.get_mut(&ChunkPos::of_block(x, y, z)) else {
            return false;
        };
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        c.set(l(x), l(y), l(z), b);
        true
    }

    /// Wall bit at world coordinates; `false` if the chunk isn't loaded.
    pub fn wall(&self, x: i32, y: i32, z: i32) -> bool {
        let Some(c) = self.chunks.get(&ChunkPos::of_block(x, y, z)) else {
            return false;
        };
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        c.wall(l(x), l(y), l(z))
    }

    pub fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid> {
        let c = self.chunks.get(&ChunkPos::of_block(x, y, z))?;
        let l = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        c.liquid(l(x), l(y), l(z))
    }

    pub fn set_liquid(&mut self, x: i32, y: i32, z: i32, l: Option<Liquid>) -> bool {
        let Some(c) = self.chunks.get_mut(&ChunkPos::of_block(x, y, z)) else {
            return false;
        };
        let l_coord = |v: i32| v.rem_euclid(CHUNK_SIZE) as usize;
        c.set_liquid(l_coord(x), l_coord(y), l_coord(z), l);
        true
    }
}

/// Read-only block lookup used by physics. Physics queries should return
/// `SOLID_UNLOADED` for unloaded space.
pub trait BlockQuery {
    fn block(&self, x: i32, y: i32, z: i32) -> BlockId;
    fn liquid(&self, _x: i32, _y: i32, _z: i32) -> Option<Liquid> {
        None
    }
}

/// Block lookup that can also edit.
pub trait BlockEdit: BlockQuery {
    fn set_block(&mut self, x: i32, y: i32, z: i32, b: BlockId);
}

impl BlockQuery for World {
    fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
        World::block(self, x, y, z)
    }
    fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid> {
        World::liquid(self, x, y, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{DIRT, STONE};

    #[test]
    fn chunkpos_floor_div() {
        assert_eq!(
            ChunkPos::of_block(-1, 0, 32),
            ChunkPos { x: -1, y: 0, z: 1 }
        );
        assert_eq!(
            ChunkPos::of_block(-32, -33, 31),
            ChunkPos { x: -1, y: -2, z: 0 }
        );
    }

    #[test]
    fn world_block_across_chunks() {
        let mut w = World::new();
        let mut a = Chunk::filled(AIR);
        a.set(31, 0, 0, STONE);
        let mut b = Chunk::filled(AIR);
        b.set(0, 0, 0, DIRT);
        w.insert(ChunkPos { x: 0, y: 0, z: 0 }, a);
        w.insert(ChunkPos { x: 1, y: 0, z: 0 }, b);
        assert_eq!(w.block(31, 0, 0), STONE);
        assert_eq!(w.block(32, 0, 0), DIRT);
        assert_eq!(w.block(30, 0, 0), AIR);
    }

    #[test]
    fn wall_bit_roundtrips_through_chunk_codec() {
        let mut c = Chunk::filled(AIR);
        assert!(!c.has_walls());
        c.set_wall(1, 2, 3, true);
        c.set_wall(31, 31, 31, true);
        c.set(0, 0, 0, STONE);
        let d = Chunk::from_bytes(&c.to_bytes()).unwrap();
        assert_eq!(d, c);
        assert!(d.wall(1, 2, 3) && d.wall(31, 31, 31) && !d.wall(3, 2, 1));
        // Clearing the last wall frees the bitset and restores plain equality.
        let mut e = d.clone();
        e.set_wall(1, 2, 3, false);
        e.set_wall(31, 31, 31, false);
        assert!(!e.has_walls());
    }

    #[test]
    fn old_chunk_bytes_load_without_walls() {
        let c = Chunk::filled(DIRT);
        let b = c.to_bytes(); // no trailer: identical to the pre-wall format
        assert_eq!(b.len(), 6);
        let d = Chunk::from_bytes(&b).unwrap();
        assert!(!d.has_walls());
        assert_eq!(d, c);
        let mut bad = c.to_bytes();
        bad.push(2);
        assert!(Chunk::from_bytes(&bad).is_err());
        let mut short = Chunk::filled(AIR);
        short.set_wall(0, 0, 0, true);
        let mut t = short.to_bytes();
        t.pop();
        assert!(Chunk::from_bytes(&t).is_err());
    }

    #[test]
    fn missing_chunk_is_air() {
        assert_eq!(World::new().block(5, 5, 5), AIR);
    }

    #[test]
    fn world_block_never_panics_at_extremes() {
        let mut w = World::new();
        w.insert(
            ChunkPos::of_block(i32::MIN, i32::MIN, i32::MAX),
            Chunk::filled(STONE),
        );
        assert_eq!(w.block(i32::MIN, i32::MIN, i32::MAX), STONE);
        assert_eq!(w.block(i32::MAX, i32::MAX, i32::MIN), AIR);
    }

    #[test]
    fn liquid_roundtrips_through_bytes() {
        let mut c = Chunk::filled(AIR);
        assert!(!c.has_liquid());
        c.set_liquid(0, 0, 0, Some(Liquid::water(255)));
        c.set_liquid(1, 2, 3, Some(Liquid::lava(100)));
        c.set_liquid(31, 31, 31, Some(Liquid::water(1)));
        assert!(c.has_liquid());

        let bytes = c.to_bytes();
        let decoded = Chunk::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, c);
        assert!(decoded.has_liquid());
        assert_eq!(decoded.liquid(0, 0, 0), Some(Liquid::water(255)));
        assert_eq!(decoded.liquid(1, 2, 3), Some(Liquid::lava(100)));
        assert_eq!(decoded.liquid(31, 31, 31), Some(Liquid::water(1)));
        assert_eq!(decoded.liquid(2, 2, 2), None);
    }

    #[test]
    fn chunk_without_liquid_encodes_identically_to_before() {
        let mut c = Chunk::filled(STONE);
        c.set(0, 0, 0, AIR);
        c.set_wall(5, 5, 5, true);
        assert!(!c.has_liquid());

        let mut expected = Vec::new();
        expected.extend_from_slice(&1u32.to_le_bytes());
        expected.extend_from_slice(&AIR.0.to_le_bytes());
        expected.extend_from_slice(&(VOLUME as u32 - 1).to_le_bytes());
        expected.extend_from_slice(&STONE.0.to_le_bytes());
        expected.push(1);
        let mut wall_words = [0u64; WALL_WORDS];
        let idx = Chunk::index(5, 5, 5);
        wall_words[idx / 64] |= 1u64 << (idx % 64);
        for w in wall_words {
            expected.extend_from_slice(&w.to_le_bytes());
        }

        assert_eq!(c.to_bytes(), expected);
    }

    #[test]
    fn last_liquid_removed_drops_layer_and_equals_fresh_chunk() {
        let fresh = Chunk::filled(AIR);
        let mut c = Chunk::filled(AIR);
        assert_eq!(c, fresh);

        c.set_liquid(10, 10, 10, Some(Liquid::water(128)));
        assert!(c.has_liquid());
        assert_ne!(c, fresh);

        c.set_liquid(10, 10, 10, None);
        assert!(!c.has_liquid());
        assert_eq!(c, fresh);

        c.set_liquid(10, 10, 10, Some(Liquid::lava(50)));
        assert!(c.has_liquid());
        c.set_liquid(10, 10, 10, Some(Liquid::lava(0)));
        assert!(!c.has_liquid());
        assert_eq!(c, fresh);
    }

    #[test]
    fn walls_and_liquid_both_roundtrip() {
        let mut c = Chunk::filled(STONE);
        c.set_wall(1, 2, 3, true);
        c.set_liquid(4, 5, 6, Some(Liquid::water(200)));
        c.set_liquid(7, 8, 9, Some(Liquid::lava(150)));

        let bytes = c.to_bytes();
        let decoded = Chunk::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, c);
        assert!(decoded.wall(1, 2, 3));
        assert_eq!(decoded.liquid(4, 5, 6), Some(Liquid::water(200)));
        assert_eq!(decoded.liquid(7, 8, 9), Some(Liquid::lava(150)));
        assert!(!decoded.wall(4, 5, 6));
        assert_eq!(decoded.liquid(1, 2, 3), None);
    }

    #[test]
    fn world_liquid_across_chunk_boundary() {
        let mut w = World::new();
        let a = Chunk::filled(AIR);
        let b = Chunk::filled(AIR);
        w.insert(ChunkPos { x: 0, y: 0, z: 0 }, a);
        w.insert(ChunkPos { x: 1, y: 0, z: 0 }, b);

        assert!(w.set_liquid(31, 0, 0, Some(Liquid::water(200))));
        assert!(w.set_liquid(32, 0, 0, Some(Liquid::lava(100))));
        assert!(!w.set_liquid(64, 0, 0, Some(Liquid::water(255))));
        assert_eq!(w.liquid(64, 0, 0), None);

        assert_eq!(w.liquid(31, 0, 0), Some(Liquid::water(200)));
        assert_eq!(w.liquid(32, 0, 0), Some(Liquid::lava(100)));
        assert_eq!(w.liquid(30, 0, 0), None);
    }

    #[test]
    fn corrupted_liquid_bytes_return_decode_error() {
        let mut c = Chunk::filled(AIR);
        c.set_liquid(0, 0, 0, Some(Liquid::water(255)));
        let valid = c.to_bytes();

        // Truncated by 1 byte
        let mut truncated = valid.clone();
        truncated.pop();
        assert_eq!(Chunk::from_bytes(&truncated), Err(DecodeError));

        // Unknown tag 3
        let mut unknown_tag = valid.clone();
        unknown_tag.push(3);
        assert_eq!(Chunk::from_bytes(&unknown_tag), Err(DecodeError));

        // Incomplete run (run claims more than volume)
        let mut invalid_run = Chunk::filled(AIR).to_bytes();
        invalid_run.push(2);
        // run = VOLUME + 1
        invalid_run.extend_from_slice(&((VOLUME as u32) + 1).to_le_bytes());
        invalid_run.extend_from_slice(&0u16.to_le_bytes());
        assert_eq!(Chunk::from_bytes(&invalid_run), Err(DecodeError));

        // Run is 0
        let mut zero_run = Chunk::filled(AIR).to_bytes();
        zero_run.push(2);
        zero_run.extend_from_slice(&0u32.to_le_bytes());
        zero_run.extend_from_slice(&0u16.to_le_bytes());
        assert_eq!(Chunk::from_bytes(&zero_run), Err(DecodeError));
    }
}
