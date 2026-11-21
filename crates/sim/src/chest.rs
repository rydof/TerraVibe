//! Chest contents and the pure transfer rules between a chest and an inventory.

use crate::codec::{DecodeError, Reader, Writer};
use crate::inventory::{Inventory, ItemStack};
use crate::item::item_data;

/// Chest.DefaultMaxItems = 40 (gameplay spec).
pub const CHEST_SLOTS: usize = 40;

#[derive(Clone, Debug, PartialEq)]
pub struct ChestInv {
    pub slots: [Option<ItemStack>; CHEST_SLOTS],
}

impl Default for ChestInv {
    fn default() -> Self {
        Self::empty()
    }
}

/// Adds `s` to `slots`: merge into matching stacks first, then empty slots.
/// Returns what did not fit.
fn add_to(slots: &mut [Option<ItemStack>], mut s: ItemStack) -> Option<ItemStack> {
    let max = item_data(s.id).max_stack;
    for slot in slots.iter_mut().flatten() {
        if slot.id == s.id && slot.prefix == s.prefix && slot.count < max {
            let n = (max - slot.count).min(s.count);
            slot.count += n;
            s.count -= n;
        }
    }
    for slot in slots.iter_mut() {
        if s.count == 0 {
            return None;
        }
        if slot.is_none() {
            let n = max.min(s.count);
            *slot = Some(ItemStack { count: n, ..s });
            s.count -= n;
        }
    }
    (s.count > 0).then_some(s)
}

impl ChestInv {
    pub fn empty() -> Self {
        ChestInv {
            slots: [None; CHEST_SLOTS],
        }
    }

    /// Terraria's `Chest.CanDestroyChest` is exactly "no stack left".
    pub fn is_empty(&self) -> bool {
        self.slots.iter().flatten().all(|s| s.count == 0)
    }

    pub fn from_stacks(stacks: &[ItemStack]) -> Self {
        let mut c = Self::empty();
        for s in stacks {
            add_to(&mut c.slots, *s);
        }
        c
    }

    /// Moves inventory slot `slot` into the chest; what does not fit stays.
    /// Returns false when nothing moved.
    pub fn move_in(&mut self, inv: &mut Inventory, slot: usize) -> bool {
        let Some(s) = inv.slots.get(slot).copied().flatten() else {
            return false;
        };
        let left = add_to(&mut self.slots, s);
        let moved = left.is_none_or(|l| l.count != s.count);
        inv.slots[slot] = left;
        moved
    }

    /// Moves chest slot `slot` into the inventory; what does not fit stays.
    pub fn move_out(&mut self, slot: usize, inv: &mut Inventory) -> bool {
        let Some(s) = self.slots.get(slot).copied().flatten() else {
            return false;
        };
        let left = inv.add(s);
        let moved = left.is_none_or(|l| l.count != s.count);
        self.slots[slot] = left;
        moved
    }

    /// Player.QuickStackAllChests: every non-hotbar inventory stack whose item
    /// the chest already holds goes in. Returns whether anything moved.
    pub fn quick_stack(&mut self, inv: &mut Inventory) -> bool {
        let mut any = false;
        for i in 10..crate::inventory::BAG {
            let Some(s) = inv.slots[i] else { continue };
            if self.slots.iter().flatten().any(|c| c.id == s.id) {
                any |= self.move_in(inv, i);
            }
        }
        any
    }

    pub fn write(&self, w: &mut Writer) {
        for s in &self.slots {
            w.stack(s);
        }
    }

    pub fn read(r: &mut Reader) -> Result<Self, DecodeError> {
        let mut c = Self::empty();
        for s in c.slots.iter_mut() {
            *s = r.stack()?;
        }
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{COPPER_PICKAXE, Prefix, WOOD};

    fn st(id: crate::item::ItemId, count: u32) -> ItemStack {
        ItemStack {
            id,
            count,
            prefix: Prefix(0),
        }
    }

    fn total(c: &ChestInv, inv: &Inventory) -> u32 {
        c.slots
            .iter()
            .flatten()
            .filter(|s| s.id == WOOD)
            .map(|s| s.count)
            .sum::<u32>()
            + inv.count_of(WOOD)
    }

    #[test]
    fn quick_stack_ignores_coin_and_ammo_slots() {
        let mut inv = Inventory::empty();
        let mut c = ChestInv::empty();
        c.slots[0] = Some(st(crate::item::COPPER_COIN, 1));
        c.slots[1] = Some(st(crate::item::WOODEN_ARROW, 1));
        inv.slots[50] = Some(st(crate::item::COPPER_COIN, 4));
        inv.slots[54] = Some(st(crate::item::WOODEN_ARROW, 4));
        assert!(!c.quick_stack(&mut inv));
        assert_eq!(inv.slots[50].unwrap().count, 4);
        assert_eq!(inv.slots[54].unwrap().count, 4);
    }

    #[test]
    fn chest_has_forty_slots() {
        assert_eq!(CHEST_SLOTS, 40); // gameplay spec DefaultMaxItems
        assert!(ChestInv::empty().is_empty());
    }

    #[test]
    fn move_in_out_conserves_items() {
        let mut inv = Inventory::empty();
        inv.slots[12] = Some(st(WOOD, 500));
        inv.slots[13] = Some(st(COPPER_PICKAXE, 1));
        let mut c = ChestInv::empty();
        assert!(c.move_in(&mut inv, 12));
        assert_eq!(inv.slots[12], None);
        assert_eq!(total(&c, &inv), 500);
        assert!(c.move_in(&mut inv, 13));
        assert!(!c.move_in(&mut inv, 14), "empty slot moves nothing");
        assert!(!c.is_empty());
        assert!(c.move_out(0, &mut inv));
        assert_eq!(total(&c, &inv), 500);
        assert_eq!(inv.count_of(WOOD), 500);
        assert!(!c.move_out(39, &mut inv));
    }

    #[test]
    fn stacks_merge_up_to_max() {
        let max = item_data(WOOD).max_stack;
        let mut c = ChestInv::empty();
        c.slots[0] = Some(st(WOOD, max - 10));
        let mut inv = Inventory::empty();
        inv.slots[11] = Some(st(WOOD, 25));
        assert!(c.move_in(&mut inv, 11));
        assert_eq!(c.slots[0].unwrap().count, max);
        assert_eq!(c.slots[1].unwrap().count, 15);
        assert_eq!(inv.slots[11], None);
    }

    #[test]
    fn full_chest_leaves_remainder_in_inventory() {
        let mut c = ChestInv::empty();
        for s in c.slots.iter_mut() {
            *s = Some(st(COPPER_PICKAXE, 1));
        }
        let mut inv = Inventory::empty();
        inv.slots[11] = Some(st(WOOD, 5));
        assert!(!c.move_in(&mut inv, 11));
        assert_eq!(inv.slots[11], Some(st(WOOD, 5)));
    }

    #[test]
    fn quick_stack_moves_only_known_items_outside_hotbar() {
        let mut c = ChestInv::empty();
        c.slots[3] = Some(st(WOOD, 1));
        let mut inv = Inventory::empty();
        inv.slots[2] = Some(st(WOOD, 7)); // hotbar: kept
        inv.slots[20] = Some(st(WOOD, 9));
        inv.slots[21] = Some(st(COPPER_PICKAXE, 1));
        assert!(c.quick_stack(&mut inv));
        assert_eq!(inv.slots[2], Some(st(WOOD, 7)));
        assert_eq!(inv.slots[20], None);
        assert_eq!(inv.slots[21], Some(st(COPPER_PICKAXE, 1)));
        assert_eq!(c.slots[3].unwrap().count, 10);
    }

    #[test]
    fn bytes_roundtrip() {
        let mut c = ChestInv::empty();
        c.slots[5] = Some(ItemStack {
            id: WOOD,
            count: 77,
            prefix: Prefix(2),
        });
        let mut w = Writer::default();
        c.write(&mut w);
        assert_eq!(ChestInv::read(&mut Reader(&w.0)).unwrap(), c);
    }
}
