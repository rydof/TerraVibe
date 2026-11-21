//! 58-slot inventory (bag, coins, ammo) and the starting kit.

use crate::codec::{DecodeError, Reader, Writer};
use crate::item::{
    COPPER_AXE, COPPER_PICKAXE, COPPER_SHORTSWORD, ItemClass, ItemId, Prefix, Rng, is_ammo,
    is_coin, item_data, roll_prefix,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortMode {
    #[default]
    ByType,
    ByName,
    ByValue,
}

fn item_class_rank(class: ItemClass) -> u8 {
    match class {
        ItemClass::Melee => 0,
        ItemClass::Ranged => 1,
        ItemClass::Magic => 2,
        ItemClass::Tool => 3,
        ItemClass::Armor => 4,
        ItemClass::Accessory => 5,
        ItemClass::Ammo => 6,
        ItemClass::Consumable => 7,
        ItemClass::Block => 8,
        ItemClass::Placeable => 9,
        ItemClass::Wall => 10,
        ItemClass::Other => 11,
    }
}

/// Bag slots 0..50 (hotbar first), coin slots 50..54, ammo slots 54..58.
pub const BAG: usize = 50;
pub const COIN0: usize = 50;
pub const AMMO0: usize = 54;
pub const SLOTS: usize = 58;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemStack {
    pub id: ItemId,
    pub count: u32,
    pub prefix: Prefix,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Inventory {
    pub slots: [Option<ItemStack>; SLOTS],
    /// Hotbar selection, 0..10.
    pub selected: usize,
}

impl Inventory {
    pub fn empty() -> Inventory {
        Inventory {
            slots: [None; SLOTS],
            selected: 0,
        }
    }

    /// Slots then the hotbar selection (58 stacks + u8).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        for s in &self.slots {
            w.stack(s);
        }
        w.u8(self.selected as u8);
        w.0
    }

    /// Reads `n` slots (50 = old layout, 58 = current) then the selection.
    pub fn read(r: &mut Reader, n: usize) -> Result<Inventory, DecodeError> {
        let mut inv = Inventory::empty();
        for s in inv.slots[..n].iter_mut() {
            *s = r.stack()?;
        }
        inv.selected = r.u8()? as usize;
        if inv.selected >= 10 {
            return Err(DecodeError);
        }
        Ok(inv)
    }

    /// Current 58-slot layout, falling back to the old 50-slot layout.
    pub fn from_bytes(bytes: &[u8]) -> Result<Inventory, DecodeError> {
        for n in [SLOTS, BAG] {
            let mut r = Reader(bytes);
            if let Ok(inv) = Inventory::read(&mut r, n)
                && r.is_empty()
            {
                return Ok(inv);
            }
        }
        Err(DecodeError)
    }

    /// Merges into matching stacks first, then fills the first empty slot.
    /// Coins and ammo try their dedicated slots before the rest.
    /// Returns what did not fit.
    pub fn add(&mut self, mut s: ItemStack) -> Option<ItemStack> {
        let first = if is_coin(s.id) {
            Some(COIN0..AMMO0)
        } else if is_ammo(s.id) {
            Some(AMMO0..SLOTS)
        } else {
            None
        };
        if let Some(range) = first {
            s = self.add_in(range, s)?;
        }
        self.add_in(0..BAG, s)
    }

    fn add_in(&mut self, range: std::ops::Range<usize>, mut s: ItemStack) -> Option<ItemStack> {
        let max = item_data(s.id).max_stack;
        let slots = &mut self.slots[range];
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

    /// Total of `item` across all slots.
    pub fn count_of(&self, item: ItemId) -> u32 {
        self.slots
            .iter()
            .flatten()
            .filter(|s| s.id == item)
            .map(|s| s.count)
            .sum()
    }

    /// Removes `count` of `item` across slots (last slots first), or nothing
    /// at all when fewer are held.
    pub fn remove(&mut self, item: ItemId, count: u32) -> bool {
        if self.count_of(item) < count {
            return false;
        }
        let mut left = count;
        for slot in self.slots.iter_mut().rev() {
            if left == 0 {
                break;
            }
            if let Some(s) = slot.as_mut().filter(|s| s.id == item) {
                let n = s.count.min(left);
                s.count -= n;
                left -= n;
                if s.count == 0 {
                    *slot = None;
                }
            }
        }
        true
    }

    /// Removes one item from the selected slot, emptying it at zero.
    pub fn take_one_selected(&mut self) -> Option<ItemStack> {
        let slot = &mut self.slots[self.selected];
        let s = slot.as_mut()?;
        let one = ItemStack { count: 1, ..*s };
        s.count -= 1;
        if s.count == 0 {
            *slot = None;
        }
        Some(one)
    }

    pub fn selected_stack(&self) -> Option<&ItemStack> {
        self.slots[self.selected].as_ref()
    }

    /// The first ammo slot that matches Terraria's ammo selection order
    /// (`Player.PickAmmo`, gameplay spec): ammo slots 54..58 first, then bag 0..50.
    pub fn first_ammo_slot(&self) -> Option<usize> {
        (AMMO0..SLOTS)
            .chain(0..BAG)
            .find(|&i| self.slots[i].is_some_and(|a| a.count > 0 && is_ammo(a.id)))
    }

    /// The first ammo slot holding ammo matching `weapon` (arrows for bows, bullets for guns).
    pub fn first_ammo_slot_for(&self, weapon: ItemId) -> Option<usize> {
        (AMMO0..SLOTS).chain(0..BAG).find(|&i| {
            self.slots[i].is_some_and(|a| a.count > 0 && crate::item::matches_ammo(weapon, a.id))
        })
    }

    /// The first matching ammo stack and its count, consumed when firing a bow.
    pub fn matching_ammo(&self) -> Option<&ItemStack> {
        self.first_ammo_slot().and_then(|i| self.slots[i].as_ref())
    }

    /// Count of the first matching ammo stack consumed by a bow.
    pub fn ammo_count(&self) -> u32 {
        self.matching_ammo().map_or(0, |s| s.count)
    }

    /// Merges stackable items in bag slots `0..BAG` and sorts them in-place.
    /// Coin (`COIN0..AMMO0`) and ammo (`AMMO0..SLOTS`) slots are untouched.
    pub fn sort(&mut self, mode: SortMode) {
        let mut merged: Vec<ItemStack> = Vec::new();
        for mut st in self.slots[..BAG].iter().flatten().copied() {
            let max = item_data(st.id).max_stack;
            for existing in &mut merged {
                if existing.id == st.id && existing.prefix == st.prefix && existing.count < max {
                    let add = (max - existing.count).min(st.count);
                    existing.count += add;
                    st.count -= add;
                    if st.count == 0 {
                        break;
                    }
                }
            }
            while st.count > 0 {
                let take = st.count.min(max);
                merged.push(ItemStack { count: take, ..st });
                st.count -= take;
            }
        }
        merged.sort_by(|a, b| {
            let da = item_data(a.id);
            let db = item_data(b.id);
            match mode {
                SortMode::ByType => (
                    item_class_rank(da.class),
                    da.name,
                    a.id.0,
                    a.prefix.0,
                    std::cmp::Reverse(a.count),
                )
                    .cmp(&(
                        item_class_rank(db.class),
                        db.name,
                        b.id.0,
                        b.prefix.0,
                        std::cmp::Reverse(b.count),
                    )),
                SortMode::ByName => (da.name, a.id.0, a.prefix.0, std::cmp::Reverse(a.count))
                    .cmp(&(db.name, b.id.0, b.prefix.0, std::cmp::Reverse(b.count))),
                SortMode::ByValue => (
                    std::cmp::Reverse(da.value),
                    da.name,
                    a.id.0,
                    a.prefix.0,
                    std::cmp::Reverse(a.count),
                )
                    .cmp(&(
                        std::cmp::Reverse(db.value),
                        db.name,
                        b.id.0,
                        b.prefix.0,
                        std::cmp::Reverse(b.count),
                    )),
            }
        });
        for (i, slot) in self.slots[..BAG].iter_mut().enumerate() {
            *slot = merged.get(i).copied();
        }
    }

    /// Shortsword, pickaxe, axe (each with a rolled prefix).
    pub fn starting(rng: &mut Rng) -> Inventory {
        let mut inv = Inventory::empty();
        for (i, id) in [COPPER_SHORTSWORD, COPPER_PICKAXE, COPPER_AXE]
            .into_iter()
            .enumerate()
        {
            let prefix = roll_prefix(id, &mut || rng.next_u32());
            inv.slots[i] = Some(ItemStack {
                id,
                count: 1,
                prefix,
            });
        }
        inv
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::*;

    fn st(id: ItemId, count: u32) -> ItemStack {
        ItemStack {
            id,
            count,
            prefix: Prefix(0),
        }
    }

    #[test]
    fn add_merges_then_fills() {
        let mut inv = Inventory::empty();
        inv.slots[5] = Some(st(DIRT_BLOCK, 10));
        assert_eq!(inv.add(st(DIRT_BLOCK, 5)), None);
        assert_eq!(inv.slots[5].unwrap().count, 15);
        assert_eq!(inv.add(st(STONE_BLOCK, 1)), None);
        assert_eq!(inv.slots[0].unwrap().id, STONE_BLOCK);
    }

    #[test]
    fn add_respects_max_stack() {
        let mut inv = Inventory::empty();
        let max = item_data(DIRT_BLOCK).max_stack;
        inv.slots[0] = Some(st(DIRT_BLOCK, max - 1));
        assert_eq!(inv.add(st(DIRT_BLOCK, 3)), None);
        assert_eq!(inv.slots[0].unwrap().count, max);
        assert_eq!(inv.slots[1].unwrap().count, 2);
    }

    #[test]
    fn add_to_full_returns_leftover() {
        let mut inv = Inventory::empty();
        for s in inv.slots.iter_mut() {
            *s = Some(st(STONE_BLOCK, item_data(STONE_BLOCK).max_stack));
        }
        assert_eq!(inv.add(st(DIRT_BLOCK, 4)), Some(st(DIRT_BLOCK, 4)));
    }

    #[test]
    fn take_one_selected_empties_slot() {
        let mut inv = Inventory::empty();
        inv.selected = 2;
        inv.slots[2] = Some(st(DIRT_BLOCK, 2));
        assert_eq!(inv.take_one_selected(), Some(st(DIRT_BLOCK, 1)));
        assert_eq!(inv.selected_stack().unwrap().count, 1);
        assert!(inv.take_one_selected().is_some());
        assert!(inv.slots[2].is_none());
        assert_eq!(inv.take_one_selected(), None);
    }

    #[test]
    fn remove_is_all_or_nothing() {
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(st(WOOD, 5));
        inv.slots[3] = Some(st(WOOD, 4));
        assert_eq!(inv.count_of(WOOD), 9);
        let before = inv.clone();
        assert!(!inv.remove(WOOD, 10));
        assert_eq!(inv, before);
        assert!(inv.remove(WOOD, 6));
        assert_eq!(inv.count_of(WOOD), 3);
        assert!(inv.slots[3].is_none());
        assert!(inv.remove(WOOD, 3));
        assert!(inv.slots.iter().all(|s| s.is_none()));
    }

    #[test]
    fn starting_kit_has_no_torches() {
        let inv = Inventory::starting(&mut Rng::new(7));
        let ids: Vec<_> = inv.slots[..3].iter().map(|s| s.unwrap().id).collect();
        assert_eq!(ids, [COPPER_SHORTSWORD, COPPER_PICKAXE, COPPER_AXE]);
        assert_eq!(inv.count_of(TORCH), 0);
        assert!(inv.slots[3..].iter().all(|s| s.is_none()));
        let mut rng = Rng::new(7);
        for (i, id) in [COPPER_SHORTSWORD, COPPER_PICKAXE, COPPER_AXE]
            .into_iter()
            .enumerate()
        {
            let p = roll_prefix(id, &mut || rng.next_u32());
            assert_eq!(inv.slots[i].unwrap().prefix, p);
        }
        assert_eq!(inv, Inventory::starting(&mut Rng::new(7)));
    }

    #[test]
    fn old_50_slot_bytes_load_into_58() {
        let mut w = Writer::default();
        for i in 0..BAG {
            w.stack(&(i == 7).then(|| st(DIRT_BLOCK, 3)));
        }
        w.u8(4);
        let inv = Inventory::from_bytes(&w.0).unwrap();
        assert_eq!(inv.slots[7], Some(st(DIRT_BLOCK, 3)));
        assert!(inv.slots[BAG..].iter().all(|s| s.is_none()));
        assert_eq!(inv.selected, 4);
        let back = Inventory::from_bytes(&inv.to_bytes()).unwrap();
        assert_eq!(back, inv);
    }

    #[test]
    fn old_50_slot_multi_item_bytes_load() {
        let mut w = Writer::default();
        for i in 0..BAG {
            w.stack(&(i % 10 < 4).then(|| st(DIRT_BLOCK, 3)));
        }
        w.u8(2);
        let inv = Inventory::from_bytes(&w.0).unwrap();
        assert_eq!(inv.slots[30], Some(st(DIRT_BLOCK, 3)));
        assert_eq!(inv.slots[34], None);
        assert!(inv.slots[BAG..].iter().all(|s| s.is_none()));
        assert_eq!(inv.selected, 2);
    }

    #[test]
    fn plain_items_never_enter_coin_or_ammo_slots() {
        let mut inv = Inventory::empty();
        for s in inv.slots[..BAG].iter_mut() {
            *s = Some(st(STONE_BLOCK, item_data(STONE_BLOCK).max_stack));
        }
        assert_eq!(
            inv.add(st(COPPER_SHORTSWORD, 1)),
            Some(st(COPPER_SHORTSWORD, 1))
        );
        assert!(inv.slots[BAG..].iter().all(|s| s.is_none()));
        // Coins and ammo spill into the bag once their slots are full.
        for s in inv.slots[COIN0..].iter_mut() {
            *s = Some(st(COPPER_COIN, 1));
        }
        inv.slots[0] = None;
        assert_eq!(inv.add(st(WOODEN_ARROW, 5)), None);
        assert_eq!(inv.slots[0], Some(st(WOODEN_ARROW, 5)));
    }

    #[test]
    fn coins_route_to_coin_slots_first() {
        let mut inv = Inventory::empty();
        assert_eq!(inv.add(st(GOLD_COIN, 5)), None);
        assert_eq!(inv.slots[COIN0], Some(st(GOLD_COIN, 5)));
        assert!(inv.slots[..BAG].iter().all(|s| s.is_none()));
    }

    #[test]
    fn ammo_routes_to_ammo_slots_first() {
        let mut inv = Inventory::empty();
        assert_eq!(inv.add(st(WOODEN_ARROW, 20)), None);
        assert_eq!(inv.slots[AMMO0], Some(st(WOODEN_ARROW, 20)));
        assert!(inv.slots[..BAG].iter().all(|s| s.is_none()));
    }

    #[test]
    fn sort_by_type_name_value_preserves_counts_and_coins() {
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(st(WOOD, 5));
        inv.slots[2] = Some(st(ANVIL_ITEM, 1));
        inv.slots[4] = Some(st(WOOD, 3));
        inv.slots[7] = Some(st(COPPER_SHORTSWORD, 1));
        inv.slots[COIN0] = Some(st(GOLD_COIN, 2));
        inv.slots[AMMO0] = Some(st(WOODEN_ARROW, 15));

        for mode in [SortMode::ByType, SortMode::ByName, SortMode::ByValue] {
            inv.sort(mode);
            assert_eq!(inv.count_of(WOOD), 8);
            assert_eq!(inv.count_of(ANVIL_ITEM), 1);
            assert_eq!(inv.count_of(COPPER_SHORTSWORD), 1);
            assert_eq!(inv.slots[COIN0], Some(st(GOLD_COIN, 2)));
            assert_eq!(inv.slots[AMMO0], Some(st(WOODEN_ARROW, 15)));
            // All bag items packed into 0..3 with no gaps
            assert!(inv.slots[..3].iter().all(|s| s.is_some()));
            assert!(inv.slots[3..BAG].iter().all(|s| s.is_none()));
        }

        inv.sort(SortMode::ByName);
        let names: Vec<_> = inv.slots[..3]
            .iter()
            .map(|s| item_data(s.unwrap().id).name)
            .collect();
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(names, sorted_names);

        inv.sort(SortMode::ByValue);
        let values: Vec<_> = inv.slots[..3]
            .iter()
            .map(|s| item_data(s.unwrap().id).value)
            .collect();
        assert!(values.windows(2).all(|w| w[0] >= w[1]));
    }
}
