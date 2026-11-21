//! Budgeted Terraria-style liquid flow simulation with active set and obsidian conversion.

use std::collections::{HashMap, VecDeque};

use crate::block::{AIR, BlockId, OBSIDIAN};
use crate::chunk::{CHUNK_SIZE, ChunkPos, World};
use crate::liquid_cell::{Liquid, LiquidKind};

pub const AMOUNT_FULL: u8 = 255;
pub const MAX_CELL_UPDATES_PER_TICK: usize = 4000;
pub const LAVA_PERIOD: u32 = 3;

const ORTHO_6: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

const HORIZ_4: [[i32; 3]; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];

const WORDS_PER_CHUNK: usize = (32 * 32 * 32) / 64;

pub trait LiquidWorld {
    fn block(&self, x: i32, y: i32, z: i32) -> BlockId;
    fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid>;
    fn loaded(&self, x: i32, y: i32, z: i32) -> bool;
    fn set_liquid(&mut self, x: i32, y: i32, z: i32, l: Option<Liquid>);
    fn set_block(&mut self, x: i32, y: i32, z: i32, b: BlockId);

    fn cell_info(&self, x: i32, y: i32, z: i32) -> (bool, BlockId, Option<Liquid>) {
        if !self.loaded(x, y, z) {
            return (false, AIR, None);
        }
        (true, self.block(x, y, z), self.liquid(x, y, z))
    }
    fn chunk_has_liquid(&self, c: ChunkPos) -> bool {
        let bx = c.x * CHUNK_SIZE;
        let by = c.y * CHUNK_SIZE;
        let bz = c.z * CHUNK_SIZE;
        for lz in 0..CHUNK_SIZE {
            for ly in 0..CHUNK_SIZE {
                for lx in 0..CHUNK_SIZE {
                    if self.liquid(bx + lx, by + ly, bz + lz).is_some() {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Calls `f([x, y, z])` for every cell with liquid (`amount > 0`) in chunk `c`.
    /// Default implementation scans the 32³ volume via `self.liquid`; implementations
    /// backed by `Chunk` can scan the chunk's `LiquidLayer` directly.
    fn for_each_liquid_in_chunk(&self, c: ChunkPos, mut f: impl FnMut([i32; 3])) {
        let bx = c.x * CHUNK_SIZE;
        let by = c.y * CHUNK_SIZE;
        let bz = c.z * CHUNK_SIZE;
        for lz in 0..CHUNK_SIZE {
            let z = bz + lz;
            for ly in 0..CHUNK_SIZE {
                let y = by + ly;
                for lx in 0..CHUNK_SIZE {
                    let x = bx + lx;
                    if self.liquid(x, y, z).is_some() {
                        f([x, y, z]);
                    }
                }
            }
        }
    }
}

impl LiquidWorld for World {
    fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
        self.block(x, y, z)
    }
    fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid> {
        self.liquid(x, y, z)
    }
    fn loaded(&self, x: i32, y: i32, z: i32) -> bool {
        self.chunk(ChunkPos::of_block(x, y, z)).is_some()
    }
    fn set_liquid(&mut self, x: i32, y: i32, z: i32, l: Option<Liquid>) {
        self.set_liquid(x, y, z, l);
    }
    fn set_block(&mut self, x: i32, y: i32, z: i32, b: BlockId) {
        self.set_block(x, y, z, b);
    }
}

#[derive(Default, Debug, Clone)]
pub struct StepReport {
    pub updated: usize,
    pub changed: Vec<[i32; 3]>,
    pub obsidian: Vec<[i32; 3]>,
}

/// Fast per-chunk 32³ bitset for cell deduplication with a 16-entry direct-mapped chunk cache.
struct CellBitset {
    chunks: HashMap<ChunkPos, Box<[u64; WORDS_PER_CHUNK]>>,
    cache_cp: [ChunkPos; 16],
    cache_ptr: [*mut [u64; WORDS_PER_CHUNK]; 16],
    touched_words: Vec<(*mut u64, usize)>,
}

impl Default for CellBitset {
    fn default() -> Self {
        Self {
            chunks: HashMap::new(),
            cache_cp: [ChunkPos {
                x: i32::MIN,
                y: i32::MIN,
                z: i32::MIN,
            }; 16],
            cache_ptr: [std::ptr::null_mut(); 16],
            touched_words: Vec::new(),
        }
    }
}

impl CellBitset {
    #[inline]
    fn cache_slot(cp: ChunkPos) -> usize {
        ((cp.x as usize)
            .wrapping_mul(3)
            .wrapping_add((cp.y as usize).wrapping_mul(5))
            .wrapping_add((cp.z as usize).wrapping_mul(11)))
            & 15
    }

    #[inline]
    fn split(p: [i32; 3]) -> (ChunkPos, usize, u64) {
        let cp = ChunkPos {
            x: p[0].div_euclid(CHUNK_SIZE),
            y: p[1].div_euclid(CHUNK_SIZE),
            z: p[2].div_euclid(CHUNK_SIZE),
        };
        let lx = (p[0] & 31) as usize;
        let ly = (p[1] & 31) as usize;
        let lz = (p[2] & 31) as usize;
        let idx = lx | (ly << 5) | (lz << 10);
        (cp, idx >> 6, 1u64 << (idx & 63))
    }

    #[inline]
    fn get_or_create_bits(&mut self, cp: ChunkPos) -> &mut [u64; WORDS_PER_CHUNK] {
        let slot = Self::cache_slot(cp);
        let ptr = self.cache_ptr[slot];
        if !ptr.is_null() && self.cache_cp[slot] == cp {
            // SAFETY: `ptr` points to the heap-allocated `Box<[u64; 512]>` inside
            // `self.chunks`, which is never moved or dropped while `self` lives.
            return unsafe { &mut *ptr };
        }
        let b = self
            .chunks
            .entry(cp)
            .or_insert_with(|| Box::new([0u64; WORDS_PER_CHUNK]));
        let new_ptr: *mut [u64; WORDS_PER_CHUNK] = &mut **b;
        self.cache_cp[slot] = cp;
        self.cache_ptr[slot] = new_ptr;
        b
    }

    #[inline]
    fn insert(&mut self, p: [i32; 3]) -> bool {
        let (cp, word, mask) = Self::split(p);
        let bits = self.get_or_create_bits(cp);
        let old = bits[word];
        if old & mask != 0 {
            false
        } else {
            bits[word] = old | mask;
            true
        }
    }

    #[inline]
    fn insert_tracked(&mut self, p: [i32; 3]) -> bool {
        let (cp, word, mask) = Self::split(p);
        let slot = Self::cache_slot(cp);
        let ptr = self.cache_ptr[slot];
        let bits = if !ptr.is_null() && self.cache_cp[slot] == cp {
            unsafe { &mut *ptr }
        } else {
            let b = self
                .chunks
                .entry(cp)
                .or_insert_with(|| Box::new([0u64; WORDS_PER_CHUNK]));
            let new_ptr: *mut [u64; WORDS_PER_CHUNK] = &mut **b;
            self.cache_cp[slot] = cp;
            self.cache_ptr[slot] = new_ptr;
            b
        };
        let old = bits[word];
        if old & mask != 0 {
            false
        } else {
            if old == 0 {
                self.touched_words.push((bits.as_mut_ptr(), word));
            }
            bits[word] = old | mask;
            true
        }
    }

    #[inline]
    fn remove(&mut self, p: [i32; 3]) {
        let (cp, word, mask) = Self::split(p);
        let slot = Self::cache_slot(cp);
        let ptr = self.cache_ptr[slot];
        if !ptr.is_null() && self.cache_cp[slot] == cp {
            unsafe {
                (*ptr)[word] &= !mask;
            }
            return;
        }
        if let Some(bits) = self.chunks.get_mut(&cp) {
            let new_ptr: *mut [u64; WORDS_PER_CHUNK] = &mut **bits;
            self.cache_cp[slot] = cp;
            self.cache_ptr[slot] = new_ptr;
            bits[word] &= !mask;
        }
    }

    #[inline]
    #[cfg(test)]
    fn contains(&self, p: &[i32; 3]) -> bool {
        let (cp, word, mask) = Self::split(*p);
        self.chunks
            .get(&cp)
            .is_some_and(|bits| (bits[word] & mask) != 0)
    }

    #[inline]
    fn clear_tracked(&mut self) {
        for &(base, word) in &self.touched_words {
            unsafe {
                *base.add(word) = 0;
            }
        }
        self.touched_words.clear();
    }
}

#[derive(Default)]
pub struct LiquidSim {
    queue: VecDeque<[i32; 3]>,
    active: CellBitset,
    parked: HashMap<ChunkPos, Vec<[i32; 3]>>,
    next_queue: Vec<[i32; 3]>,
    next_set: CellBitset,
    changed_set: CellBitset,
    obsidian_set: CellBitset,
}

#[inline]
fn cell_hash(p: [i32; 3]) -> usize {
    let mut h = (p[0] as u32).wrapping_mul(0x9E37_79B1)
        ^ (p[1] as u32).wrapping_mul(0x85EB_CA77)
        ^ (p[2] as u32).wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h as usize
}

#[inline]
fn opt_liquid(kind: LiquidKind, amount: u8) -> Option<Liquid> {
    if amount == 0 {
        None
    } else {
        Some(Liquid { kind, amount })
    }
}

impl LiquidSim {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn enqueue(&mut self, p: [i32; 3]) {
        if self.active.insert(p) {
            self.queue.push_back(p);
        }
    }

    #[inline]
    fn park(&mut self, unloaded_chunk: ChunkPos, cell: [i32; 3]) {
        let list = self.parked.entry(unloaded_chunk).or_default();
        if !list.contains(&cell) {
            list.push(cell);
        }
    }

    pub fn wake(&mut self, p: [i32; 3]) {
        self.enqueue(p);
        for d in ORTHO_6 {
            self.enqueue([p[0] + d[0], p[1] + d[1], p[2] + d[2]]);
        }
    }

    /// Resumes cells parked on `c` without enqueuing the chunk's own liquid.
    ///
    /// Generated chunks are settled by construction, so installing one must not
    /// wake its liquid; only cells that were waiting for it to load need a nudge.
    pub fn resume_chunk(&mut self, c: ChunkPos) {
        if let Some(waiters) = self.parked.remove(&c) {
            for p in waiters {
                self.wake(p);
            }
        }
    }

    pub fn wake_chunk(&mut self, w: &impl LiquidWorld, c: ChunkPos) {
        self.resume_chunk(c);
        w.for_each_liquid_in_chunk(c, |p| {
            self.enqueue(p);
        });
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    pub fn step(&mut self, w: &mut impl LiquidWorld, tick: u32, budget: usize) -> StepReport {
        let mut report = StepReport::default();
        if self.queue.is_empty() {
            return report;
        }
        self.obsidian_set.clear_tracked();
        self.changed_set.clear_tracked();
        self.next_queue.clear();
        self.next_set.clear_tracked();

        #[inline]
        fn schedule_cell(next_queue: &mut Vec<[i32; 3]>, next_set: &mut CellBitset, p: [i32; 3]) {
            if next_set.insert_tracked(p) {
                next_queue.push(p);
            }
        }

        #[inline]
        fn schedule_wake(next_queue: &mut Vec<[i32; 3]>, next_set: &mut CellBitset, p: [i32; 3]) {
            schedule_cell(next_queue, next_set, p);
            for d in ORTHO_6 {
                schedule_cell(
                    next_queue,
                    next_set,
                    [p[0] + d[0], p[1] + d[1], p[2] + d[2]],
                );
            }
        }

        let initial_len = self.queue.len();
        let mut popped = 0usize;
        let mut conv_buf: [[i32; 3]; 6] = [[0; 3]; 6];

        while report.updated < budget && popped < initial_len {
            let Some(p) = self.queue.pop_front() else {
                break;
            };
            popped += 1;
            self.active.remove(p);
            let [x, y, z] = p;
            let (loaded, block, cur_opt) = w.cell_info(x, y, z);

            if !loaded {
                self.park(ChunkPos::of_block(x, y, z), p);
                continue;
            }

            // If a non-air block occupies the cell, ensure any residual liquid is cleared.
            if block != AIR {
                if cur_opt.is_some() {
                    w.set_liquid(x, y, z, None);
                    if self.changed_set.insert_tracked(p) {
                        report.changed.push(p);
                    }
                    schedule_wake(&mut self.next_queue, &mut self.next_set, p);
                }
                report.updated += 1;
                continue;
            }

            let Some(mut cur) = cur_opt else {
                report.updated += 1;
                continue;
            };

            // Rule 4 (pre-check): Contact with orthogonal neighbour of the other kind.
            let (self_converted, conv_len) =
                self.check_and_convert_contacts(w, p, cur.kind, &mut report, &mut conv_buf);
            for &cp in &conv_buf[..conv_len] {
                schedule_wake(&mut self.next_queue, &mut self.next_set, cp);
            }
            if self_converted {
                report.updated += 1;
                continue;
            }

            // Rule 5 (lava period check): Lava runs rules 1-3 only when tick % LAVA_PERIOD == 0.
            // Otherwise it stays in queue and does NOT count against the budget.
            if cur.kind == LiquidKind::Lava && !tick.is_multiple_of(LAVA_PERIOD) {
                if self.next_set.insert_tracked(p) {
                    self.next_queue.push(p);
                }
                continue;
            }

            report.updated += 1;
            let mut moved = false;

            // Rule 1: Down
            let below = [x, y - 1, z];
            let mut down_blocked_or_full = true;
            let (b_loaded, b_block, b_liq_opt) = w.cell_info(below[0], below[1], below[2]);
            if !b_loaded {
                self.park(ChunkPos::of_block(below[0], below[1], below[2]), p);
            } else if b_block == AIR {
                match b_liq_opt {
                    None => {
                        let flow = cur.amount;
                        cur.amount = 0;
                        w.set_liquid(x, y, z, None);
                        w.set_liquid(
                            below[0],
                            below[1],
                            below[2],
                            Some(Liquid::new(cur.kind, flow)),
                        );
                        if self.changed_set.insert_tracked(p) {
                            report.changed.push(p);
                        }
                        if self.changed_set.insert_tracked(below) {
                            report.changed.push(below);
                        }
                        schedule_wake(&mut self.next_queue, &mut self.next_set, p);
                        schedule_wake(&mut self.next_queue, &mut self.next_set, below);
                        moved = true;
                        down_blocked_or_full = false;
                    }
                    Some(b_liq) if b_liq.kind == cur.kind => {
                        if b_liq.amount < AMOUNT_FULL {
                            let room = AMOUNT_FULL - b_liq.amount;
                            let flow = cur.amount.min(room);
                            cur.amount -= flow;
                            let new_below = b_liq.amount + flow;
                            w.set_liquid(x, y, z, opt_liquid(cur.kind, cur.amount));
                            w.set_liquid(
                                below[0],
                                below[1],
                                below[2],
                                Some(Liquid::new(cur.kind, new_below)),
                            );
                            if self.changed_set.insert_tracked(p) {
                                report.changed.push(p);
                            }
                            if self.changed_set.insert_tracked(below) {
                                report.changed.push(below);
                            }
                            schedule_wake(&mut self.next_queue, &mut self.next_set, p);
                            schedule_wake(&mut self.next_queue, &mut self.next_set, below);
                            moved = true;
                            down_blocked_or_full = new_below == AMOUNT_FULL;
                        }
                    }
                    Some(_other_kind) => {
                        // Handled by contact rule
                    }
                }
            }

            // Rule 2: Spread
            if cur.amount > 1 && down_blocked_or_full {
                let rot = (tick as usize).wrapping_add(cell_hash(p)) & 3;
                let mut cands: [([i32; 3], u8); 4] = [([0; 3], 0); 4];
                let mut k = 0usize;
                let mut sum_nb: u32 = 0;

                for i in 0..4 {
                    let d = HORIZ_4[(rot + i) & 3];
                    let np = [x + d[0], y + d[1], z + d[2]];
                    let (n_loaded, n_block, n_liq) = w.cell_info(np[0], np[1], np[2]);
                    if !n_loaded {
                        self.park(ChunkPos::of_block(np[0], np[1], np[2]), p);
                        continue;
                    }
                    if n_block != AIR {
                        continue;
                    }
                    match n_liq {
                        None => {
                            if 2 <= cur.amount {
                                cands[k] = (np, 0);
                                k += 1;
                            }
                        }
                        Some(nl) if nl.kind == cur.kind => {
                            if (nl.amount as u16) + 2 <= (cur.amount as u16) {
                                cands[k] = (np, nl.amount);
                                sum_nb += nl.amount as u32;
                                k += 1;
                            }
                        }
                        Some(_) => {}
                    }
                }

                if k > 0 {
                    // Iteratively drop any candidate whose amount >= average so remaining
                    // lower candidates still receive liquid up to their equalised target.
                    loop {
                        let total = (cur.amount as u32) + sum_nb;
                        let target = (total / (k as u32 + 1)) as u8;
                        let mut new_k = 0usize;
                        let mut new_sum = 0u32;
                        for idx in 0..k {
                            if cands[idx].1 < target {
                                cands[new_k] = cands[idx];
                                new_sum += cands[idx].1 as u32;
                                new_k += 1;
                            }
                        }
                        if new_k == k || new_k == 0 {
                            k = new_k;
                            sum_nb = new_sum;
                            break;
                        }
                        k = new_k;
                        sum_nb = new_sum;
                    }

                    let mut spread_any = false;
                    if k > 0 {
                        let total = (cur.amount as u32) + sum_nb;
                        let target = (total / (k as u32 + 1)) as u8;
                        for &(np, nb_amt) in &cands[..k] {
                            if target > nb_amt {
                                let give = (target - nb_amt).min(cur.amount);
                                if give > 0 {
                                    cur.amount -= give;
                                    let new_nb = nb_amt + give;
                                    w.set_liquid(
                                        np[0],
                                        np[1],
                                        np[2],
                                        Some(Liquid::new(cur.kind, new_nb)),
                                    );
                                    if self.changed_set.insert_tracked(np) {
                                        report.changed.push(np);
                                    }
                                    schedule_wake(&mut self.next_queue, &mut self.next_set, np);
                                    spread_any = true;
                                }
                            }
                        }
                    }

                    if spread_any {
                        w.set_liquid(x, y, z, opt_liquid(cur.kind, cur.amount));
                        if self.changed_set.insert_tracked(p) {
                            report.changed.push(p);
                        }
                        schedule_wake(&mut self.next_queue, &mut self.next_set, p);
                        moved = true;
                    }
                }
            }

            // Rule 3: Up-equalise (only when no downward/horizontal move happened)
            if !moved
                && down_blocked_or_full
                && cur.amount >= 250
                && let Some(source) = self.find_pressure_source(w, p, cur.kind, cur.amount)
            {
                let src_amt = w
                    .liquid(source[0], source[1], source[2])
                    .map_or(0, |l| l.amount);
                if src_amt > 0 {
                    if cur.amount < AMOUNT_FULL {
                        // Top up `p` to 255 from the higher pressure column
                        w.set_liquid(
                            source[0],
                            source[1],
                            source[2],
                            opt_liquid(cur.kind, src_amt - 1),
                        );
                        cur.amount += 1;
                        w.set_liquid(x, y, z, Some(Liquid::new(cur.kind, cur.amount)));
                        if self.changed_set.insert_tracked(source) {
                            report.changed.push(source);
                        }
                        if self.changed_set.insert_tracked(p) {
                            report.changed.push(p);
                        }
                        schedule_wake(&mut self.next_queue, &mut self.next_set, source);
                        schedule_wake(&mut self.next_queue, &mut self.next_set, p);
                        moved = true;
                    } else {
                        let above = [x, y + 1, z];
                        let above_amt = w
                            .liquid(above[0], above[1], above[2])
                            .map_or(0, |l| l.amount);
                        if above_amt < cur.amount - 1 {
                            w.set_liquid(
                                source[0],
                                source[1],
                                source[2],
                                opt_liquid(cur.kind, src_amt - 1),
                            );
                            let new_above = above_amt + 1;
                            w.set_liquid(
                                above[0],
                                above[1],
                                above[2],
                                Some(Liquid::new(cur.kind, new_above)),
                            );
                            if self.changed_set.insert_tracked(source) {
                                report.changed.push(source);
                            }
                            if self.changed_set.insert_tracked(p) {
                                report.changed.push(p);
                            }
                            if self.changed_set.insert_tracked(above) {
                                report.changed.push(above);
                            }
                            schedule_wake(&mut self.next_queue, &mut self.next_set, source);
                            schedule_wake(&mut self.next_queue, &mut self.next_set, p);
                            schedule_wake(&mut self.next_queue, &mut self.next_set, above);
                            moved = true;
                        }
                    }
                }
            }

            if moved && cur.amount > 0 {
                let (_, conv_len2) =
                    self.check_and_convert_contacts(w, p, cur.kind, &mut report, &mut conv_buf);
                for &cp in &conv_buf[..conv_len2] {
                    schedule_wake(&mut self.next_queue, &mut self.next_set, cp);
                }
            }
        }

        for i in 0..self.next_queue.len() {
            let np = self.next_queue[i];
            self.enqueue(np);
        }

        report
    }

    /// Finds a cell in a connected higher pressure column that can supply 1 unit to `p` or `above(p)`.
    fn find_pressure_source(
        &mut self,
        w: &impl LiquidWorld,
        p: [i32; 3],
        kind: LiquidKind,
        p_amt: u8,
    ) -> Option<[i32; 3]> {
        let [x, y, z] = p;
        let above = [x, y + 1, z];
        let (a_loaded, a_block, a_liq) = w.cell_info(above[0], above[1], above[2]);
        let (target_y, target_amt) = if p_amt < AMOUNT_FULL {
            (y, p_amt)
        } else {
            if !a_loaded {
                self.park(ChunkPos::of_block(above[0], above[1], above[2]), p);
                return None;
            }
            if a_block != AIR {
                return None;
            }
            let above_amt = match a_liq {
                None => 0,
                Some(l) if l.kind == kind => l.amount,
                Some(_) => return None,
            };
            if above_amt >= AMOUNT_FULL - 1 {
                return None;
            }
            (y + 1, above_amt)
        };

        // Check that the vertical chain below `p` is full (`255`) down to a solid block.
        let mut cy = y - 1;
        while cy >= y - 16 {
            let (c_loaded, c_block, c_liq) = w.cell_info(x, cy, z);
            if !c_loaded {
                return None;
            }
            if c_block != AIR {
                break;
            }
            match c_liq {
                Some(l) if l.kind == kind && l.amount == AMOUNT_FULL => {
                    cy -= 1;
                }
                _ => return None,
            }
        }

        // Also if `p` is NOT at the surface of its column (i.e. `above` already has `>= 250` liquid),
        // let the top of the column pull instead of every interior cell running a BFS!
        if p_amt == AMOUNT_FULL && a_liq.is_some_and(|l| l.kind == kind && l.amount >= 250) {
            return None;
        }

        // At the bottom of the column (`bottom_y = cy + 1`), if `p` itself is not capped by a solid
        // block above, at least one horizontal neighbour of `bottom_y` must be a full cell (`>= 250`)
        // that is either capped by a solid block (a pipe/tunnel) or part of a deeper/confined channel.
        let bottom_y = cy + 1;
        if !a_loaded || a_block == AIR {
            let has_pipe = HORIZ_4.iter().any(|d| {
                let nx = x + d[0];
                let nz = z + d[2];
                let (nl_loaded, nl_block, nl_liq) = w.cell_info(nx, bottom_y, nz);
                nl_loaded
                    && nl_block == AIR
                    && nl_liq.is_some_and(|l| l.kind == kind && l.amount >= 250)
                    && {
                        let (na_loaded, na_block, _) = w.cell_info(nx, bottom_y + 1, nz);
                        na_loaded && na_block != AIR
                    }
            });
            if !has_pipe {
                return None;
            }
        }

        // Fixed-size stack-allocated BFS through nearly-full/full cells (`>= 250`) of `kind`.
        // Also verify that the path traverses at least one cell capped by a solid ceiling
        // (i.e. a genuine submerged U-bend pipe), not just an open basin floor.
        const MAX_SEARCH: usize = 32;
        let mut q: [[i32; 3]; MAX_SEARCH] = [[0; 3]; MAX_SEARCH];
        let mut q_capped: [bool; MAX_SEARCH] = [false; MAX_SEARCH];
        let mut q_head = 0usize;
        let mut q_tail = 1usize;
        q[0] = p;
        q_capped[0] = a_loaded && a_block != AIR;

        let mut best: Option<([i32; 3], i32, u8, bool)> = None;
        while q_head < q_tail {
            let curr = q[q_head];
            let curr_capped = q_capped[q_head];
            q_head += 1;
            for d in ORTHO_6 {
                let np = [curr[0] + d[0], curr[1] + d[1], curr[2] + d[2]];
                if np == above || q[..q_tail].contains(&np) {
                    continue;
                }
                let (n_loaded, n_block, n_liq) = w.cell_info(np[0], np[1], np[2]);
                if !n_loaded || n_block != AIR {
                    continue;
                }
                let Some(nl) = n_liq else {
                    continue;
                };
                if nl.kind != kind {
                    continue;
                }
                let (na_loaded, na_block, na_liq) = w.cell_info(np[0], np[1] + 1, np[2]);
                let np_solid_above = na_loaded && na_block != AIR;
                let np_has_liquid_above = na_liq.is_some_and(|l| l.kind == kind && l.amount > 0);
                let path_capped = curr_capped || np_solid_above;
                if path_capped
                    && !np_has_liquid_above
                    && (np[1] > target_y
                        || (np[1] == target_y && (nl.amount as u16) >= (target_amt as u16) + 2))
                {
                    let is_better = match best {
                        None => true,
                        Some((_, by, bamt, _)) => np[1] > by || (np[1] == by && nl.amount > bamt),
                    };
                    if is_better {
                        best = Some((np, np[1], nl.amount, false));
                    }
                }
                if nl.amount >= 250 && q_tail < MAX_SEARCH {
                    q[q_tail] = np;
                    q_capped[q_tail] = path_capped;
                    q_tail += 1;
                }
            }
        }
        best.map(|(pos, _, _, _)| pos)
    }

    /// Rule 4: Contact between water and lava converts the lava cell to `OBSIDIAN`.
    /// Returns `(self_converted, count_of_converted_positions)`.
    fn check_and_convert_contacts(
        &mut self,
        w: &mut impl LiquidWorld,
        p: [i32; 3],
        kind: LiquidKind,
        report: &mut StepReport,
        conv_buf: &mut [[i32; 3]; 6],
    ) -> (bool, usize) {
        let mut self_converted = false;
        let mut conv_len = 0usize;
        for d in ORTHO_6 {
            let np = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
            let (n_loaded, n_block, n_liq) = w.cell_info(np[0], np[1], np[2]);
            if !n_loaded {
                self.park(ChunkPos::of_block(np[0], np[1], np[2]), p);
                continue;
            }
            if n_block != AIR {
                continue;
            }
            let Some(nl) = n_liq else {
                continue;
            };
            if nl.kind != kind {
                let lava_pos = if kind == LiquidKind::Lava { p } else { np };
                w.set_liquid(lava_pos[0], lava_pos[1], lava_pos[2], None);
                w.set_block(lava_pos[0], lava_pos[1], lava_pos[2], OBSIDIAN);
                if self.obsidian_set.insert_tracked(lava_pos) {
                    report.obsidian.push(lava_pos);
                }
                if self.changed_set.insert_tracked(lava_pos) {
                    report.changed.push(lava_pos);
                }
                conv_buf[conv_len] = lava_pos;
                conv_len += 1;
                if lava_pos == p {
                    self_converted = true;
                    break;
                }
            }
        }
        (self_converted, conv_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::STONE;
    use crate::chunk::{Chunk, World};

    struct TestWorld {
        world: World,
        cache_cp: std::cell::Cell<ChunkPos>,
        cache_ptr: std::cell::Cell<*mut Chunk>,
    }

    impl TestWorld {
        fn new() -> Self {
            Self {
                world: World::new(),
                cache_cp: std::cell::Cell::new(ChunkPos {
                    x: i32::MIN,
                    y: i32::MIN,
                    z: i32::MIN,
                }),
                cache_ptr: std::cell::Cell::new(std::ptr::null_mut()),
            }
        }

        fn invalidate_cache(&self) {
            self.cache_ptr.set(std::ptr::null_mut());
        }

        #[inline]
        fn lookup_chunk(&self, cp: ChunkPos) -> Option<&Chunk> {
            let ptr = self.cache_ptr.get();
            if !ptr.is_null() && self.cache_cp.get() == cp {
                return Some(unsafe { &*ptr });
            }
            let c = self.world.chunk(cp)?;
            self.cache_cp.set(cp);
            self.cache_ptr.set(c as *const Chunk as *mut Chunk);
            Some(c)
        }

        #[inline]
        fn lookup_chunk_mut(&mut self, cp: ChunkPos) -> Option<&mut Chunk> {
            let ptr = self.cache_ptr.get();
            if !ptr.is_null() && self.cache_cp.get() == cp {
                return Some(unsafe { &mut *ptr });
            }
            let c = self.world.chunk_mut(cp)?;
            self.cache_cp.set(cp);
            self.cache_ptr.set(c as *mut Chunk);
            Some(c)
        }

        fn ensure_chunk(&mut self, cp: ChunkPos) {
            if self.world.chunk(cp).is_none() {
                self.world.insert(cp, Chunk::filled(AIR));
                self.invalidate_cache();
            }
        }

        fn set_block_load(&mut self, x: i32, y: i32, z: i32, b: BlockId) {
            let cp = ChunkPos::of_block(x, y, z);
            self.ensure_chunk(cp);
            self.set_block(x, y, z, b);
        }

        fn set_liquid_load(&mut self, x: i32, y: i32, z: i32, l: Option<Liquid>) {
            let cp = ChunkPos::of_block(x, y, z);
            self.ensure_chunk(cp);
            self.set_liquid(x, y, z, l);
        }

        fn total_liquid(&self, kind: LiquidKind) -> u64 {
            let mut sum = 0u64;
            for x in -64..128 {
                for y in -32..64 {
                    for z in -64..128 {
                        if let Some(l) = self.world.liquid(x, y, z)
                            && l.kind == kind
                        {
                            sum += l.amount as u64;
                        }
                    }
                }
            }
            sum
        }

        fn place_block(&mut self, sim: &mut LiquidSim, x: i32, y: i32, z: i32, b: BlockId) {
            self.set_block(x, y, z, b);
            if b != AIR {
                self.set_liquid(x, y, z, None);
            }
            sim.wake([x, y, z]);
        }
    }

    impl LiquidWorld for TestWorld {
        #[inline]
        fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
            let cp = ChunkPos {
                x: x.div_euclid(CHUNK_SIZE),
                y: y.div_euclid(CHUNK_SIZE),
                z: z.div_euclid(CHUNK_SIZE),
            };
            let Some(c) = self.lookup_chunk(cp) else {
                return AIR;
            };
            c.get((x & 31) as usize, (y & 31) as usize, (z & 31) as usize)
        }

        #[inline]
        fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid> {
            let cp = ChunkPos {
                x: x.div_euclid(CHUNK_SIZE),
                y: y.div_euclid(CHUNK_SIZE),
                z: z.div_euclid(CHUNK_SIZE),
            };
            let c = self.lookup_chunk(cp)?;
            c.liquid((x & 31) as usize, (y & 31) as usize, (z & 31) as usize)
        }

        #[inline]
        fn loaded(&self, x: i32, y: i32, z: i32) -> bool {
            let cp = ChunkPos {
                x: x.div_euclid(CHUNK_SIZE),
                y: y.div_euclid(CHUNK_SIZE),
                z: z.div_euclid(CHUNK_SIZE),
            };
            self.lookup_chunk(cp).is_some()
        }

        #[inline]
        fn set_liquid(&mut self, x: i32, y: i32, z: i32, l: Option<Liquid>) {
            let cp = ChunkPos {
                x: x.div_euclid(CHUNK_SIZE),
                y: y.div_euclid(CHUNK_SIZE),
                z: z.div_euclid(CHUNK_SIZE),
            };
            if let Some(c) = self.lookup_chunk_mut(cp) {
                c.set_liquid((x & 31) as usize, (y & 31) as usize, (z & 31) as usize, l);
            }
        }

        #[inline]
        fn set_block(&mut self, x: i32, y: i32, z: i32, b: BlockId) {
            let cp = ChunkPos {
                x: x.div_euclid(CHUNK_SIZE),
                y: y.div_euclid(CHUNK_SIZE),
                z: z.div_euclid(CHUNK_SIZE),
            };
            if let Some(c) = self.lookup_chunk_mut(cp) {
                let lx = (x & 31) as usize;
                let ly = (y & 31) as usize;
                let lz = (z & 31) as usize;
                c.set(lx, ly, lz, b);
                if b != AIR {
                    c.set_liquid(lx, ly, lz, None);
                }
            }
        }

        #[inline]
        fn cell_info(&self, x: i32, y: i32, z: i32) -> (bool, BlockId, Option<Liquid>) {
            let cp = ChunkPos {
                x: x.div_euclid(CHUNK_SIZE),
                y: y.div_euclid(CHUNK_SIZE),
                z: z.div_euclid(CHUNK_SIZE),
            };
            let Some(c) = self.lookup_chunk(cp) else {
                return (false, AIR, None);
            };
            let lx = (x & 31) as usize;
            let ly = (y & 31) as usize;
            let lz = (z & 31) as usize;
            (true, c.get(lx, ly, lz), c.liquid(lx, ly, lz))
        }

        fn for_each_liquid_in_chunk(&self, c: ChunkPos, mut f: impl FnMut([i32; 3])) {
            let Some(chunk) = self.world.chunk(c) else {
                return;
            };
            let Some(amounts) = chunk.liquid_amounts() else {
                return;
            };
            let bx = c.x * CHUNK_SIZE;
            let by = c.y * CHUNK_SIZE;
            let bz = c.z * CHUNK_SIZE;
            for lz in 0..CHUNK_SIZE {
                let z = bz + lz;
                let z_off = (lz as usize) << 10;
                for ly in 0..CHUNK_SIZE {
                    let y = by + ly;
                    let yz_off = z_off | ((ly as usize) << 5);
                    for lx in 0..CHUNK_SIZE {
                        if amounts[yz_off | (lx as usize)] > 0 {
                            f([bx + lx, y, z]);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn water_falls_down_a_shaft_and_pools() {
        let mut w = TestWorld::new();
        let mut sim = LiquidSim::new();

        // Build a 1x1 vertical shaft from y=1..=5 with stone walls and stone floor at y=0
        w.set_block_load(5, 0, 5, STONE);
        for y in 1..=6 {
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                w.set_block_load(5 + dx, y, 5 + dz, STONE);
            }
        }

        w.set_liquid_load(5, 5, 5, Some(Liquid::water(255)));
        sim.wake([5, 5, 5]);

        for tick in 0..30 {
            sim.step(&mut w, tick, MAX_CELL_UPDATES_PER_TICK);
        }

        assert_eq!(w.liquid(5, 1, 5), Some(Liquid::water(255)));
        for y in 2..=5 {
            assert_eq!(w.liquid(5, y, 5), None);
        }
        assert_eq!(w.total_liquid(LiquidKind::Water), 255);
        assert_eq!(sim.pending(), 0);
    }

    #[test]
    fn water_spreads_four_ways_on_flat_floor() {
        let mut w = TestWorld::new();
        let mut sim = LiquidSim::new();

        // Open flat floor at y=0 in chunk (0,0,0)
        w.ensure_chunk(ChunkPos { x: 0, y: 0, z: 0 });
        for x in 0..32 {
            for z in 0..32 {
                w.set_block(x, 0, z, STONE);
            }
        }

        w.set_liquid(16, 1, 16, Some(Liquid::water(255)));
        sim.wake([16, 1, 16]);

        for tick in 0..200 {
            sim.step(&mut w, tick, MAX_CELL_UPDATES_PER_TICK);
        }

        assert_eq!(w.total_liquid(LiquidKind::Water), 255);

        let arm_sum = |dx: i32, dz: i32| -> i32 {
            (1..=15)
                .map(|d| {
                    w.liquid(16 + dx * d, 1, 16 + dz * d)
                        .map_or(0, |l| l.amount as i32)
                })
                .sum()
        };

        let pos_x = arm_sum(1, 0);
        let neg_x = arm_sum(-1, 0);
        let pos_z = arm_sum(0, 1);
        let neg_z = arm_sum(0, -1);

        assert!(pos_x > 0, "water should spread to +x");
        assert!((pos_x - neg_x).abs() <= 2, "x arms: {pos_x} vs {neg_x}");
        assert!((pos_z - neg_z).abs() <= 2, "z arms: {pos_z} vs {neg_z}");
        assert!((pos_x - pos_z).abs() <= 2, "+x vs +z: {pos_x} vs {pos_z}");

        // Immediate neighbours should be within ±1 of each other
        let n_amounts = [
            w.liquid(17, 1, 16).map_or(0, |l| l.amount as i32),
            w.liquid(15, 1, 16).map_or(0, |l| l.amount as i32),
            w.liquid(16, 1, 17).map_or(0, |l| l.amount as i32),
            w.liquid(16, 1, 15).map_or(0, |l| l.amount as i32),
        ];
        let min_n = *n_amounts.iter().min().unwrap();
        let max_n = *n_amounts.iter().max().unwrap();
        assert!(max_n - min_n <= 1, "immediate neighbours: {n_amounts:?}");
    }

    #[test]
    fn sealed_basin_levels_out_and_sleeps() {
        let mut w = TestWorld::new();
        let mut sim = LiquidSim::new();

        w.ensure_chunk(ChunkPos { x: 0, y: 0, z: 0 });
        // Build a sealed U-shaped basin in z=5 plane:
        // Left arm x=4, bottom channel y=1 for x=4..=8, right arm x=8, y=1..=5
        for x in 2..=10 {
            for y in 0..=7 {
                for z in 3..=7 {
                    w.set_block(x, y, z, STONE);
                }
            }
        }
        for y in 1..=5 {
            w.set_block(4, y, 5, AIR);
            w.set_block(8, y, 5, AIR);
        }
        for x in 4..=8 {
            w.set_block(x, 1, 5, AIR);
        }

        for y in 1..=5 {
            w.set_liquid(4, y, 5, Some(Liquid::water(255)));
            sim.wake([4, y, 5]);
        }
        for x in 5..=7 {
            w.set_liquid(x, 1, 5, Some(Liquid::water(255)));
            sim.wake([x, 1, 5]);
        }

        let initial_total = w.total_liquid(LiquidKind::Water);
        assert_eq!(initial_total, 8 * 255);

        for tick in 0..3000 {
            sim.step(&mut w, tick, MAX_CELL_UPDATES_PER_TICK);
            if sim.pending() == 0 {
                break;
            }
        }

        assert_eq!(sim.pending(), 0, "basin should settle and sleep");
        assert_eq!(w.total_liquid(LiquidKind::Water), initial_total);

        let left_col: i32 = (1..=5)
            .map(|y| w.liquid(4, y, 5).map_or(0, |l| l.amount as i32))
            .sum();
        let right_col: i32 = (1..=5)
            .map(|y| w.liquid(8, y, 5).map_or(0, |l| l.amount as i32))
            .sum();
        assert!(
            (left_col - right_col).abs() <= 2,
            "U-bend columns should equalise within ±1..2: left={left_col}, right={right_col}"
        );
    }

    #[test]
    fn liquid_total_is_conserved_over_1000_steps() {
        let mut w = TestWorld::new();
        let mut sim = LiquidSim::new();

        w.ensure_chunk(ChunkPos { x: 0, y: 0, z: 0 });
        for x in 1..=21 {
            for y in 0..=16 {
                for z in 1..=21 {
                    if x == 1 || x == 21 || y == 0 || y == 16 || z == 1 || z == 21 {
                        w.set_block(x, y, z, STONE);
                    }
                }
            }
        }
        for x in 2..=20 {
            for y in 1..=15 {
                for z in 2..=20 {
                    let h = ((x * 73856093) ^ (y * 19349663) ^ (z * 83492791)) as u32;
                    if h % 7 == 0 {
                        w.set_block(x, y, z, STONE);
                    } else if h % 11 == 0 && y > 5 {
                        let amt = ((h % 200) + 55) as u8;
                        w.set_liquid(x, y, z, Some(Liquid::water(amt)));
                        sim.wake([x, y, z]);
                    }
                }
            }
        }

        let initial_total = w.total_liquid(LiquidKind::Water);
        assert!(initial_total > 0);

        for tick in 0..1000 {
            sim.step(&mut w, tick, MAX_CELL_UPDATES_PER_TICK);
        }

        assert_eq!(w.total_liquid(LiquidKind::Water), initial_total);
    }

    #[test]
    fn settled_world_does_zero_updates() {
        let mut w = TestWorld::new();
        let mut sim = LiquidSim::new();

        w.ensure_chunk(ChunkPos { x: 0, y: 0, z: 0 });
        for x in 4..=12 {
            for z in 4..=12 {
                w.set_block(x, 0, z, STONE);
                for y in 1..=3 {
                    if x == 4 || x == 12 || z == 4 || z == 12 {
                        w.set_block(x, y, z, STONE);
                    }
                }
            }
        }
        w.set_liquid(8, 2, 8, Some(Liquid::water(255)));
        sim.wake([8, 2, 8]);

        for tick in 0..500 {
            sim.step(&mut w, tick, MAX_CELL_UPDATES_PER_TICK);
            if sim.pending() == 0 {
                break;
            }
        }
        assert_eq!(sim.pending(), 0);

        for tick in 500..600 {
            let report = sim.step(&mut w, tick, MAX_CELL_UPDATES_PER_TICK);
            assert_eq!(report.updated, 0);
            assert!(report.changed.is_empty());
        }
    }

    #[test]
    fn lava_flows_at_one_third_rate() {
        let build_trough = |kind: LiquidKind| -> i32 {
            let mut w = TestWorld::new();
            let mut sim = LiquidSim::new();
            w.ensure_chunk(ChunkPos { x: 0, y: 0, z: 0 });
            for x in 0..=31 {
                w.set_block(x, 0, 5, STONE);
                w.set_block(x, 1, 4, STONE);
                w.set_block(x, 1, 6, STONE);
                w.set_block(x, 2, 4, STONE);
                w.set_block(x, 2, 6, STONE);
            }
            w.set_block(0, 1, 5, STONE);
            w.set_block(0, 2, 5, STONE);
            w.set_block(31, 1, 5, STONE);
            for y in 1..=4 {
                w.set_liquid(1, y, 5, Some(Liquid::new(kind, 255)));
                sim.wake([1, y, 5]);
            }
            for tick in 1..=15 {
                sim.step(&mut w, tick, MAX_CELL_UPDATES_PER_TICK);
            }
            (1..=30)
                .filter(|&x| w.liquid(x, 1, 5).is_some())
                .max()
                .unwrap_or(1)
                - 1
        };

        let water_dist = build_trough(LiquidKind::Water);
        let lava_dist = build_trough(LiquidKind::Lava);
        assert!(water_dist >= 5, "water should advance: {water_dist}");
        assert!(
            (lava_dist as f32) <= (water_dist as f32) * 0.4,
            "lava dist {lava_dist} should be <= 40% of water dist {water_dist}"
        );
    }

    #[test]
    fn water_on_lava_makes_obsidian_once() {
        let mut w = TestWorld::new();
        let mut sim = LiquidSim::new();
        w.ensure_chunk(ChunkPos { x: 0, y: 0, z: 0 });

        w.set_block(5, 0, 5, STONE);
        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            for y in 1..=3 {
                w.set_block(5 + dx, y, 5 + dz, STONE);
            }
        }
        w.set_liquid(5, 1, 5, Some(Liquid::lava(255)));
        w.set_liquid(5, 2, 5, Some(Liquid::water(255)));
        sim.wake([5, 1, 5]);
        sim.wake([5, 2, 5]);

        let mut all_obsidian = Vec::new();
        for tick in 0..20 {
            let rep = sim.step(&mut w, tick, MAX_CELL_UPDATES_PER_TICK);
            all_obsidian.extend(rep.obsidian);
        }

        assert_eq!(w.block(5, 1, 5), OBSIDIAN);
        assert_eq!(w.liquid(5, 1, 5), None);
        assert_eq!(w.liquid(5, 2, 5), Some(Liquid::water(255)));
        assert_eq!(all_obsidian, vec![[5, 1, 5]]);
    }

    #[test]
    fn thin_lava_film_becomes_obsidian() {
        let mut w = TestWorld::new();
        let mut sim = LiquidSim::new();
        w.ensure_chunk(ChunkPos { x: 0, y: 0, z: 0 });

        w.set_block(5, 0, 5, STONE);
        w.set_block(6, 0, 5, STONE);
        w.set_liquid(5, 1, 5, Some(Liquid::lava(10)));
        w.set_liquid(6, 1, 5, Some(Liquid::water(200)));
        sim.wake([5, 1, 5]);
        sim.wake([6, 1, 5]);

        let rep = sim.step(&mut w, 0, MAX_CELL_UPDATES_PER_TICK);

        assert_eq!(w.block(5, 1, 5), OBSIDIAN);
        assert_eq!(w.liquid(5, 1, 5), None);
        assert!(rep.obsidian.contains(&[5, 1, 5]));
        assert_eq!(w.total_liquid(LiquidKind::Lava), 0);
    }

    #[test]
    fn liquid_waits_at_unloaded_chunk_edge_then_continues() {
        let mut w = TestWorld::new();
        let mut sim = LiquidSim::new();

        w.ensure_chunk(ChunkPos { x: 0, y: 0, z: 0 });
        for x in 28..=31 {
            w.set_block(x, 0, 5, STONE);
            w.set_block(x, 1, 4, STONE);
            w.set_block(x, 1, 6, STONE);
        }
        w.set_block(28, 1, 5, STONE);

        w.set_liquid(31, 1, 5, Some(Liquid::water(255)));
        sim.wake([31, 1, 5]);

        for tick in 0..30 {
            sim.step(&mut w, tick, MAX_CELL_UPDATES_PER_TICK);
        }

        assert_eq!(w.total_liquid(LiquidKind::Water), 255);
        assert!(w.liquid(31, 1, 5).is_some());
        assert!(!w.loaded(32, 1, 5));

        let next_chunk = ChunkPos { x: 1, y: 0, z: 0 };
        w.ensure_chunk(next_chunk);
        for x in 32..=36 {
            w.set_block(x, 0, 5, STONE);
            w.set_block(x, 1, 4, STONE);
            w.set_block(x, 1, 6, STONE);
        }
        w.set_block(36, 1, 5, STONE);

        sim.wake_chunk(&w, next_chunk);

        for tick in 30..60 {
            sim.step(&mut w, tick, MAX_CELL_UPDATES_PER_TICK);
        }

        assert_eq!(w.total_liquid(LiquidKind::Water), 255);
        assert!(
            w.liquid(32, 1, 5).is_some(),
            "water should flow into newly loaded chunk"
        );
    }

    #[test]
    fn budget_defers_in_fifo_order() {
        let mut w = TestWorld::new();
        let mut sim = LiquidSim::new();

        for x in 0..22 {
            for y in 0..20 {
                for z in 0..20 {
                    let p = [x, y, z];
                    w.ensure_chunk(ChunkPos::of_block(p[0], p[1], p[2]));
                    sim.wake(p);
                }
            }
        }
        // Make sure all queued cells (including boundary neighbours from wake) are in loaded chunks
        let all_queued: Vec<[i32; 3]> = sim.queue.iter().copied().collect();
        for p in &all_queued {
            w.ensure_chunk(ChunkPos::of_block(p[0], p[1], p[2]));
        }
        let total = sim.pending();
        assert!(total >= 10_000 && total <= 12_000, "pending was {total}");

        let first_batch: Vec<[i32; 3]> = sim.queue.iter().take(4000).copied().collect();
        let second_batch: Vec<[i32; 3]> = sim.queue.iter().skip(4000).take(4000).copied().collect();

        let r1 = sim.step(&mut w, 0, 4000);
        assert_eq!(r1.updated, 4000);
        assert_eq!(sim.pending(), total - 4000);
        for p in &first_batch {
            assert!(
                !sim.active.contains(p),
                "first batch cell {p:?} should be popped"
            );
        }
        for p in &second_batch {
            assert!(
                sim.active.contains(p),
                "second batch cell {p:?} should still be queued"
            );
        }

        let r2 = sim.step(&mut w, 1, 4000);
        assert_eq!(r2.updated, 4000);
        assert_eq!(sim.pending(), total - 8000);
        for p in &second_batch {
            assert!(
                !sim.active.contains(p),
                "second batch cell {p:?} should be popped"
            );
        }

        let r3 = sim.step(&mut w, 2, 4000);
        assert_eq!(r3.updated, total - 8000);
        assert_eq!(sim.pending(), 0);
    }

    #[test]
    fn placing_block_removes_liquid_and_wakes_neighbours() {
        let mut w = TestWorld::new();
        let mut sim = LiquidSim::new();
        w.ensure_chunk(ChunkPos { x: 0, y: 0, z: 0 });

        for x in 4..=8 {
            w.set_block(x, 0, 5, STONE);
            w.set_block(x, 1, 4, STONE);
            w.set_block(x, 1, 6, STONE);
        }
        w.set_block(4, 1, 5, STONE);
        w.set_block(8, 1, 5, STONE);

        w.set_liquid(5, 1, 5, Some(Liquid::water(255)));
        w.set_liquid(6, 1, 5, Some(Liquid::water(255)));
        w.set_liquid(7, 1, 5, Some(Liquid::water(100)));
        assert_eq!(sim.pending(), 0);

        w.place_block(&mut sim, 6, 1, 5, STONE);
        assert_eq!(w.liquid(6, 1, 5), None);
        assert!(sim.pending() > 0, "neighbours should be woken");
        assert!(sim.active.contains(&[5, 1, 5]));
        assert!(sim.active.contains(&[7, 1, 5]));
    }

    #[test]
    #[ignore]
    fn liquid_perf_4000_active_cells_under_2ms() {
        let mut w = TestWorld::new();
        let mut sim = LiquidSim::new();

        // 64x64x8 basin across chunks (0..2, 0..1, 0..2)
        for cx in 0..2 {
            for cz in 0..2 {
                w.ensure_chunk(ChunkPos { x: cx, y: 0, z: cz });
            }
        }
        for x in 0..64 {
            for z in 0..64 {
                w.set_block(x, 0, z, STONE);
                for y in 1..=7 {
                    if x == 0 || x == 63 || z == 0 || z == 63 {
                        w.set_block(x, y, z, STONE);
                    }
                }
            }
        }

        // Pre-activate a 4000-cell flood inside the basin (e.g., 40x25x4 suspended columns/steps)
        let mut placed = 0usize;
        for y in (2..=6).rev() {
            for x in 2..42 {
                for z in 2..22 {
                    if placed < 4000 {
                        w.set_liquid(x, y, z, Some(Liquid::water(255)));
                        sim.wake([x, y, z]);
                        placed += 1;
                    }
                }
            }
        }
        assert_eq!(placed, 4000);
        assert!(sim.pending() >= 4000);

        let mut times_ms = Vec::with_capacity(200);
        for tick in 0..200 {
            let t0 = std::time::Instant::now();
            sim.step(&mut w, tick, MAX_CELL_UPDATES_PER_TICK);
            times_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        }

        let mean = times_ms.iter().sum::<f64>() / times_ms.len() as f64;
        let mut sorted = times_ms.clone();
        sorted.sort_by(f64::total_cmp);
        let p50 = sorted[sorted.len() / 2];
        let p99_idx = ((0.99 * sorted.len() as f64).ceil() as usize).saturating_sub(1);
        let p99 = sorted[p99_idx];
        let max = *sorted.last().unwrap();

        println!(
            "PERF liquid_perf_4000_active_cells_under_2ms: mean={mean:.3}ms p50={p50:.3}ms p99={p99:.3}ms max={max:.3}ms"
        );
        assert!(
            mean <= 2.0,
            "mean step time {mean:.3} ms exceeded 2.0 ms gate"
        );
        assert!(p99 <= 4.0, "p99 step time {p99:.3} ms exceeded 4.0 ms gate");
    }

    #[test]
    #[ignore]
    fn liquid_perf_settled_world_is_free() {
        let mut w = TestWorld::new();
        let mut sim = LiquidSim::new();

        // 1 million liquid cells (31 chunks of 32^3 = 1,015,808 cells), all asleep
        let mut filled_chunk = Chunk::filled(AIR);
        for lz in 0..32 {
            for ly in 0..32 {
                for lx in 0..32 {
                    filled_chunk.set_liquid(lx, ly, lz, Some(Liquid::water(255)));
                }
            }
        }
        for i in 0..31 {
            w.world
                .insert(ChunkPos { x: i, y: 0, z: 0 }, filled_chunk.clone());
        }
        assert_eq!(sim.pending(), 0);

        let mut times_ms = Vec::with_capacity(100);
        for tick in 0..100 {
            let t0 = std::time::Instant::now();
            let rep = sim.step(&mut w, tick, MAX_CELL_UPDATES_PER_TICK);
            times_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(rep.updated, 0);
        }

        let mean = times_ms.iter().sum::<f64>() / times_ms.len() as f64;
        let max = times_ms.iter().copied().fold(0.0, f64::max);
        println!("PERF liquid_perf_settled_world_is_free: mean={mean:.5}ms max={max:.5}ms");
        assert!(
            max <= 0.05,
            "settled step time {max:.5} ms exceeded 0.05 ms gate"
        );
    }

    #[test]
    #[ignore]
    fn liquid_perf_ocean_wake_on_chunk_load() {
        let mut w = TestWorld::new();
        let cp = ChunkPos { x: 0, y: 0, z: 0 };
        let mut chunk = Chunk::filled(AIR);
        for lz in 0..32 {
            for ly in 0..32 {
                for lx in 0..32 {
                    chunk.set_liquid(lx, ly, lz, Some(Liquid::water(255)));
                }
            }
        }
        w.world.insert(cp, chunk);

        let mut times_ms = Vec::with_capacity(20);
        for _ in 0..20 {
            let mut sim = LiquidSim::new();
            let t0 = std::time::Instant::now();
            sim.wake_chunk(&w, cp);
            times_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
            assert!(sim.pending() >= 32 * 32 * 32);
        }

        let mean = times_ms.iter().sum::<f64>() / times_ms.len() as f64;
        let mut sorted = times_ms.clone();
        sorted.sort_by(f64::total_cmp);
        let p50 = sorted[sorted.len() / 2];
        let max = *sorted.last().unwrap();
        println!(
            "PERF liquid_perf_ocean_wake_on_chunk_load: mean={mean:.3}ms p50={p50:.3}ms max={max:.3}ms"
        );
        assert!(
            mean <= 1.0,
            "wake_chunk time {mean:.3} ms exceeded 1.0 ms gate"
        );
    }
}
