//! Cursor stack and Terraria's slot click rules across inventory, armor and chest.

use crate::chest::ChestInv;
use crate::inventory::{AMMO0, COIN0, Inventory, ItemStack, SLOTS};
use crate::item::{ArmorSlot, Prefix, is_ammo, is_coin, item_data};
use crate::player::armor_index;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotRef {
    Inv(u8),
    Armor(ArmorSlot),
    Accessory(u8),
    VanityArmor(ArmorSlot),
    VanityAccessory(u8),
    Chest(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickKind {
    Left,
    Right,
    Shift,
}

/// The stack on the cursor.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Held(pub Option<ItemStack>);

/// Worn armor pieces, one id per slot (the `Player.armor` layout).
pub type Armor = [Option<crate::item::ItemId>; 3];

/// Worn accessories (5 slots).
pub type Accessories = [Option<crate::item::ItemId>; 5];

fn accepts(at: SlotRef, s: &ItemStack) -> bool {
    match at {
        SlotRef::Inv(i) => {
            let i = i as usize;
            if i < COIN0 {
                true
            } else if i < AMMO0 {
                is_coin(s.id)
            } else {
                is_ammo(s.id)
            }
        }
        SlotRef::Armor(a) | SlotRef::VanityArmor(a) => item_data(s.id).armor_slot == Some(a),
        SlotRef::Accessory(idx) | SlotRef::VanityAccessory(idx) => {
            idx < 5 && item_data(s.id).class == crate::item::ItemClass::Accessory
        }
        SlotRef::Chest(_) => true,
    }
}

fn armor_stack(id: Option<crate::item::ItemId>) -> Option<ItemStack> {
    id.map(|id| ItemStack {
        id,
        count: 1,
        prefix: crate::item::Prefix(0),
    })
}

/// Left/Right click on one slot's contents.
fn click_slot(cur: &mut Option<ItemStack>, held: &mut Held, ok: bool, kind: ClickKind) -> bool {
    match (kind, held.0, *cur) {
        (ClickKind::Left, None, Some(c)) => {
            held.0 = Some(c);
            *cur = None;
            true
        }
        (ClickKind::Left, Some(h), _) if ok => match *cur {
            None => {
                *cur = Some(h);
                held.0 = None;
                true
            }
            Some(c) if c.id == h.id && c.prefix == h.prefix => {
                let max = item_data(c.id).max_stack;
                let n = max.saturating_sub(c.count).min(h.count);
                if n == 0 {
                    return false;
                }
                cur.as_mut().unwrap().count += n;
                held.0 = (h.count > n).then_some(ItemStack {
                    count: h.count - n,
                    ..h
                });
                true
            }
            Some(c) => {
                *cur = Some(h);
                held.0 = Some(c);
                true
            }
        },
        (ClickKind::Right, None, Some(c)) => {
            held.0 = Some(ItemStack { count: 1, ..c });
            take_one(cur);
            true
        }
        (ClickKind::Right, Some(h), Some(c))
            if c.id == h.id && c.prefix == h.prefix && h.count < item_data(h.id).max_stack =>
        {
            held.0 = Some(ItemStack {
                count: h.count + 1,
                ..h
            });
            take_one(cur);
            true
        }
        _ => false,
    }
}

fn take_one(cur: &mut Option<ItemStack>) {
    let c = cur.as_mut().unwrap();
    c.count -= 1;
    if c.count == 0 {
        *cur = None;
    }
}

/// Applies one click. Returns whether anything changed.
pub fn click(
    inv: &mut Inventory,
    armor: &mut Armor,
    accessories: &mut Accessories,
    chest: Option<&mut ChestInv>,
    held: &mut Held,
    at: SlotRef,
    kind: ClickKind,
) -> bool {
    let mut dummy_prefixes = [Prefix(0); 5];
    click_with_prefixes(
        inv,
        armor,
        accessories,
        &mut dummy_prefixes,
        chest,
        held,
        at,
        kind,
    )
}

/// Applies one click while preserving prefixes stored alongside `accessories`.
#[allow(clippy::too_many_arguments)]
pub fn click_with_prefixes(
    inv: &mut Inventory,
    armor: &mut Armor,
    accessories: &mut Accessories,
    accessory_prefixes: &mut [Prefix; 5],
    chest: Option<&mut ChestInv>,
    held: &mut Held,
    at: SlotRef,
    kind: ClickKind,
) -> bool {
    let mut dummy_vanity_armor = [None; 3];
    let mut dummy_vanity_acc = [None; 5];
    let mut dummy_vanity_prefixes = [Prefix(0); 5];
    click_with_vanity(
        inv,
        armor,
        accessories,
        accessory_prefixes,
        &mut dummy_vanity_armor,
        &mut dummy_vanity_acc,
        &mut dummy_vanity_prefixes,
        chest,
        held,
        at,
        kind,
    )
}

fn click_armor_slot(
    slot: &mut Option<crate::item::ItemId>,
    held: &mut Held,
    ok: bool,
    kind: ClickKind,
) -> bool {
    let mut cur = armor_stack(*slot);
    let changed = click_slot(&mut cur, held, ok, kind);
    *slot = cur.map(|s| s.id);
    changed
}

fn click_accessory_slot(
    accessories: &mut Accessories,
    accessory_prefixes: &mut [Prefix; 5],
    i: usize,
    held: &mut Held,
    ok: bool,
    kind: ClickKind,
    forbid_duplicate: bool,
) -> bool {
    if forbid_duplicate
        && let Some(h) = held.0
        && accessories
            .iter()
            .enumerate()
            .any(|(idx, &a)| idx != i && a == Some(h.id))
    {
        return false;
    }
    let mut cur = accessories[i].map(|id| ItemStack {
        id,
        count: 1,
        prefix: accessory_prefixes[i],
    });
    let changed = click_slot(&mut cur, held, ok, kind);
    accessories[i] = cur.map(|s| s.id);
    accessory_prefixes[i] = cur.map_or(Prefix(0), |s| s.prefix);
    changed
}

/// Applies one click across inventory, equipment, vanity, and chest slots.
#[allow(clippy::too_many_arguments)]
pub fn click_with_vanity(
    inv: &mut Inventory,
    armor: &mut Armor,
    accessories: &mut Accessories,
    accessory_prefixes: &mut [Prefix; 5],
    vanity_armor: &mut Armor,
    vanity_accessories: &mut Accessories,
    vanity_accessory_prefixes: &mut [Prefix; 5],
    chest: Option<&mut ChestInv>,
    held: &mut Held,
    at: SlotRef,
    kind: ClickKind,
) -> bool {
    if kind == ClickKind::Shift {
        let Some(chest) = chest else { return false };
        return match at {
            SlotRef::Inv(i) if (i as usize) < SLOTS => chest.move_in(inv, i as usize),
            SlotRef::Chest(i) if (i as usize) < chest.slots.len() => {
                chest.move_out(i as usize, inv)
            }
            _ => false,
        };
    }
    let ok = held.0.is_none_or(|h| accepts(at, &h));
    match at {
        SlotRef::Inv(i) => match inv.slots.get_mut(i as usize) {
            Some(slot) => click_slot(slot, held, ok, kind),
            None => false,
        },
        SlotRef::Chest(i) => match chest.and_then(|c| c.slots.get_mut(i as usize)) {
            Some(slot) => click_slot(slot, held, ok, kind),
            None => false,
        },
        SlotRef::Armor(a) => click_armor_slot(&mut armor[armor_index(a)], held, ok, kind),
        SlotRef::VanityArmor(a) => {
            let i = armor_index(a);
            if kind == ClickKind::Right {
                if held.0.is_some() || (armor[i].is_none() && vanity_armor[i].is_none()) {
                    return false;
                }
                std::mem::swap(&mut armor[i], &mut vanity_armor[i]);
                return true;
            }
            click_armor_slot(&mut vanity_armor[i], held, ok, kind)
        }
        SlotRef::Accessory(slot) => {
            let i = slot as usize;
            if i >= 5 {
                return false;
            }
            click_accessory_slot(accessories, accessory_prefixes, i, held, ok, kind, true)
        }
        SlotRef::VanityAccessory(slot) => {
            let i = slot as usize;
            if i >= 5 {
                return false;
            }
            if kind == ClickKind::Right {
                if held.0.is_some() || (accessories[i].is_none() && vanity_accessories[i].is_none())
                {
                    return false;
                }
                if let Some(vanity_id) = vanity_accessories[i]
                    && accessories
                        .iter()
                        .enumerate()
                        .any(|(idx, &a)| idx != i && a == Some(vanity_id))
                {
                    return false;
                }
                std::mem::swap(&mut accessories[i], &mut vanity_accessories[i]);
                std::mem::swap(
                    &mut accessory_prefixes[i],
                    &mut vanity_accessory_prefixes[i],
                );
                return true;
            }
            click_accessory_slot(
                vanity_accessories,
                vanity_accessory_prefixes,
                i,
                held,
                ok,
                kind,
                false,
            )
        }
    }
}

/// Puts the held stack back into the bag; returns what did not fit.
pub fn return_held(inv: &mut Inventory, held: &mut Held) -> Option<ItemStack> {
    let s = held.0.take()?;
    inv.add(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::BAG;
    use crate::item::*;

    fn st(id: ItemId, count: u32) -> ItemStack {
        ItemStack {
            id,
            count,
            prefix: Prefix(0),
        }
    }

    fn go(inv: &mut Inventory, held: &mut Held, at: SlotRef, kind: ClickKind) -> bool {
        click(inv, &mut [None; 3], &mut [None; 5], None, held, at, kind)
    }

    #[test]
    fn left_pick_place_merge_swap() {
        let mut inv = Inventory::empty();
        let mut h = Held::default();
        inv.slots[0] = Some(st(DIRT_BLOCK, 5));
        inv.slots[1] = Some(st(DIRT_BLOCK, 7));
        inv.slots[2] = Some(st(STONE_BLOCK, 2));
        assert!(go(&mut inv, &mut h, SlotRef::Inv(0), ClickKind::Left));
        assert_eq!((h.0, inv.slots[0]), (Some(st(DIRT_BLOCK, 5)), None));
        assert!(go(&mut inv, &mut h, SlotRef::Inv(3), ClickKind::Left));
        assert_eq!((h.0, inv.slots[3]), (None, Some(st(DIRT_BLOCK, 5))));
        go(&mut inv, &mut h, SlotRef::Inv(3), ClickKind::Left);
        assert!(go(&mut inv, &mut h, SlotRef::Inv(1), ClickKind::Left));
        assert_eq!((h.0, inv.slots[1]), (None, Some(st(DIRT_BLOCK, 12))));
        go(&mut inv, &mut h, SlotRef::Inv(1), ClickKind::Left);
        assert!(go(&mut inv, &mut h, SlotRef::Inv(2), ClickKind::Left));
        assert_eq!(h.0, Some(st(STONE_BLOCK, 2)));
        assert_eq!(inv.slots[2], Some(st(DIRT_BLOCK, 12)));
    }

    #[test]
    fn right_takes_one() {
        let mut inv = Inventory::empty();
        let mut h = Held::default();
        inv.slots[0] = Some(st(DIRT_BLOCK, 3));
        assert!(go(&mut inv, &mut h, SlotRef::Inv(0), ClickKind::Right));
        assert!(go(&mut inv, &mut h, SlotRef::Inv(0), ClickKind::Right));
        assert_eq!(h.0, Some(st(DIRT_BLOCK, 2)));
        assert_eq!(inv.slots[0], Some(st(DIRT_BLOCK, 1)));
        assert!(go(&mut inv, &mut h, SlotRef::Inv(0), ClickKind::Right));
        assert_eq!((h.0, inv.slots[0]), (Some(st(DIRT_BLOCK, 3)), None));
    }

    #[test]
    fn coin_slot_rejects_non_coin() {
        let mut inv = Inventory::empty();
        let mut h = Held(Some(st(DIRT_BLOCK, 1)));
        assert!(!go(
            &mut inv,
            &mut h,
            SlotRef::Inv(COIN0 as u8),
            ClickKind::Left
        ));
        assert!(!go(
            &mut inv,
            &mut h,
            SlotRef::Inv(AMMO0 as u8),
            ClickKind::Left
        ));
        assert_eq!(h.0, Some(st(DIRT_BLOCK, 1)));
        let mut h = Held(Some(st(COPPER_COIN, 1)));
        assert!(go(
            &mut inv,
            &mut h,
            SlotRef::Inv(COIN0 as u8),
            ClickKind::Left
        ));
    }

    #[test]
    fn armor_slot_rejects_wrong_piece() {
        let mut inv = Inventory::empty();
        let mut armor = [None; 3];
        let mut acc = [None; 5];
        let mut h = Held(Some(st(COPPER_CHAINMAIL, 1)));
        assert!(!click(
            &mut inv,
            &mut armor,
            &mut acc,
            None,
            &mut h,
            SlotRef::Armor(ArmorSlot::Head),
            ClickKind::Left
        ));
        assert!(click(
            &mut inv,
            &mut armor,
            &mut acc,
            None,
            &mut h,
            SlotRef::Armor(ArmorSlot::Body),
            ClickKind::Left
        ));
        assert_eq!(armor[1], Some(COPPER_CHAINMAIL));
        assert_eq!(h.0, None);
    }

    #[test]
    fn accessory_slot_accepts_only_accessories() {
        let mut inv = Inventory::empty();
        let mut armor = [None; 3];
        let mut acc = [None; 5];
        let mut h = Held(Some(st(DIRT_BLOCK, 1)));
        // Reject non-accessory in accessory slot
        assert!(!click(
            &mut inv,
            &mut armor,
            &mut acc,
            None,
            &mut h,
            SlotRef::Accessory(0),
            ClickKind::Left
        ));
        // Accept Hermes Boots (accessory)
        let mut h = Held(Some(st(HERMES_BOOTS, 1)));
        assert!(click(
            &mut inv,
            &mut armor,
            &mut acc,
            None,
            &mut h,
            SlotRef::Accessory(0),
            ClickKind::Left
        ));
        assert_eq!(acc[0], Some(HERMES_BOOTS));
        assert_eq!(h.0, None);
        // Click again picks it back up
        assert!(click(
            &mut inv,
            &mut armor,
            &mut acc,
            None,
            &mut h,
            SlotRef::Accessory(0),
            ClickKind::Left
        ));
        assert_eq!(acc[0], None);
        assert_eq!(h.0, Some(st(HERMES_BOOTS, 1)));
    }

    #[test]
    fn shift_moves_to_open_chest() {
        let mut inv = Inventory::empty();
        let mut chest = ChestInv::empty();
        let mut armor = [None; 3];
        let mut acc = [None; 5];
        let mut h = Held::default();
        inv.slots[4] = Some(st(WOOD, 9));
        assert!(!click(
            &mut inv,
            &mut armor,
            &mut acc,
            None,
            &mut h,
            SlotRef::Inv(4),
            ClickKind::Shift
        ));
        assert!(click(
            &mut inv,
            &mut armor,
            &mut acc,
            Some(&mut chest),
            &mut h,
            SlotRef::Inv(4),
            ClickKind::Shift
        ));
        assert_eq!((inv.slots[4], chest.slots[0]), (None, Some(st(WOOD, 9))));
        assert!(click(
            &mut inv,
            &mut armor,
            &mut acc,
            Some(&mut chest),
            &mut h,
            SlotRef::Chest(0),
            ClickKind::Shift
        ));
        assert_eq!(inv.count_of(WOOD), 9);
    }

    #[test]
    fn return_held_drops_when_bag_full_even_if_coin_ammo_empty() {
        let mut inv = Inventory::empty();
        for s in inv.slots[..BAG].iter_mut() {
            *s = Some(st(STONE_BLOCK, item_data(STONE_BLOCK).max_stack));
        }
        let mut h = Held(Some(st(COPPER_BROADSWORD, 1)));
        assert_eq!(
            return_held(&mut inv, &mut h),
            Some(st(COPPER_BROADSWORD, 1))
        );
        assert!(inv.slots[BAG..].iter().all(|s| s.is_none()));
    }

    #[test]
    fn return_held_into_bag_or_leftover() {
        let mut inv = Inventory::empty();
        let mut h = Held(Some(st(DIRT_BLOCK, 4)));
        assert_eq!(return_held(&mut inv, &mut h), None);
        assert_eq!((h.0, inv.count_of(DIRT_BLOCK)), (None, 4));
        for s in inv.slots.iter_mut() {
            *s = Some(st(STONE_BLOCK, item_data(STONE_BLOCK).max_stack));
        }
        let mut h = Held(Some(st(DIRT_BLOCK, 4)));
        assert_eq!(return_held(&mut inv, &mut h), Some(st(DIRT_BLOCK, 4)));
        assert_eq!(h.0, None);
    }

    #[test]
    fn accessory_slot_click_preserves_prefix() {
        let mut inv = Inventory::empty();
        let mut armor = [None; 3];
        let mut acc = [None; 5];
        let mut acc_prefixes = [Prefix(0); 5];
        let mut h = Held(Some(ItemStack {
            id: COBALT_SHIELD,
            count: 1,
            prefix: Prefix(65),
        }));
        assert!(click_with_prefixes(
            &mut inv,
            &mut armor,
            &mut acc,
            &mut acc_prefixes,
            None,
            &mut h,
            SlotRef::Accessory(2),
            ClickKind::Left,
        ));
        assert_eq!(h.0, None);
        assert_eq!(acc[2], Some(COBALT_SHIELD));
        assert_eq!(acc_prefixes[2], Prefix(65));

        assert!(click_with_prefixes(
            &mut inv,
            &mut armor,
            &mut acc,
            &mut acc_prefixes,
            None,
            &mut h,
            SlotRef::Accessory(2),
            ClickKind::Left,
        ));
        assert_eq!(
            h.0,
            Some(ItemStack {
                id: COBALT_SHIELD,
                count: 1,
                prefix: Prefix(65),
            })
        );
        assert_eq!(acc[2], None);
        assert_eq!(acc_prefixes[2], Prefix(0));
    }

    #[test]
    fn vanity_armor_and_accessory_validation_and_right_click_swap() {
        let mut inv = Inventory::empty();
        let mut armor = [Some(IRON_HELMET), None, None];
        let mut acc = [Some(HERMES_BOOTS), None, None, None, None];
        let mut acc_prefixes = [Prefix(65), Prefix(0), Prefix(0), Prefix(0), Prefix(0)];
        let mut vanity_armor = [None; 3];
        let mut vanity_acc = [None; 5];
        let mut vanity_prefixes = [Prefix(0); 5];

        // Vanity armor rejects wrong piece and non-armor
        let mut h = Held(Some(st(COPPER_CHAINMAIL, 1)));
        assert!(!click_with_vanity(
            &mut inv,
            &mut armor,
            &mut acc,
            &mut acc_prefixes,
            &mut vanity_armor,
            &mut vanity_acc,
            &mut vanity_prefixes,
            None,
            &mut h,
            SlotRef::VanityArmor(ArmorSlot::Head),
            ClickKind::Left,
        ));

        // Vanity armor accepts matching piece
        let mut h = Held(Some(st(GOLD_HELMET, 1)));
        assert!(click_with_vanity(
            &mut inv,
            &mut armor,
            &mut acc,
            &mut acc_prefixes,
            &mut vanity_armor,
            &mut vanity_acc,
            &mut vanity_prefixes,
            None,
            &mut h,
            SlotRef::VanityArmor(ArmorSlot::Head),
            ClickKind::Left,
        ));
        assert_eq!(h.0, None);
        assert_eq!(vanity_armor[0], Some(GOLD_HELMET));

        // Right-click on vanity armor swaps with equipped armor
        assert!(click_with_vanity(
            &mut inv,
            &mut armor,
            &mut acc,
            &mut acc_prefixes,
            &mut vanity_armor,
            &mut vanity_acc,
            &mut vanity_prefixes,
            None,
            &mut h,
            SlotRef::VanityArmor(ArmorSlot::Head),
            ClickKind::Right,
        ));
        assert_eq!(armor[0], Some(GOLD_HELMET));
        assert_eq!(vanity_armor[0], Some(IRON_HELMET));

        // Vanity accessory accepts accessory with prefix and rejects non-accessory
        let mut h = Held(Some(st(DIRT_BLOCK, 1)));
        assert!(!click_with_vanity(
            &mut inv,
            &mut armor,
            &mut acc,
            &mut acc_prefixes,
            &mut vanity_armor,
            &mut vanity_acc,
            &mut vanity_prefixes,
            None,
            &mut h,
            SlotRef::VanityAccessory(0),
            ClickKind::Left,
        ));
        let mut h = Held(Some(ItemStack {
            id: COBALT_SHIELD,
            count: 1,
            prefix: Prefix(76),
        }));
        assert!(click_with_vanity(
            &mut inv,
            &mut armor,
            &mut acc,
            &mut acc_prefixes,
            &mut vanity_armor,
            &mut vanity_acc,
            &mut vanity_prefixes,
            None,
            &mut h,
            SlotRef::VanityAccessory(0),
            ClickKind::Left,
        ));
        assert_eq!(h.0, None);
        assert_eq!(vanity_acc[0], Some(COBALT_SHIELD));
        assert_eq!(vanity_prefixes[0], Prefix(76));

        // Right-click swaps vanity accessory + prefix with equipped accessory + prefix
        assert!(click_with_vanity(
            &mut inv,
            &mut armor,
            &mut acc,
            &mut acc_prefixes,
            &mut vanity_armor,
            &mut vanity_acc,
            &mut vanity_prefixes,
            None,
            &mut h,
            SlotRef::VanityAccessory(0),
            ClickKind::Right,
        ));
        assert_eq!(acc[0], Some(COBALT_SHIELD));
        assert_eq!(acc_prefixes[0], Prefix(76));
        assert_eq!(vanity_acc[0], Some(HERMES_BOOTS));
        assert_eq!(vanity_prefixes[0], Prefix(65));

        // Right-click refuses swap if it would duplicate an already-equipped accessory
        let mut h = Held(Some(st(COBALT_SHIELD, 1)));
        assert!(click_with_vanity(
            &mut inv,
            &mut armor,
            &mut acc,
            &mut acc_prefixes,
            &mut vanity_armor,
            &mut vanity_acc,
            &mut vanity_prefixes,
            None,
            &mut h,
            SlotRef::VanityAccessory(1),
            ClickKind::Left,
        ));
        assert!(!click_with_vanity(
            &mut inv,
            &mut armor,
            &mut acc,
            &mut acc_prefixes,
            &mut vanity_armor,
            &mut vanity_acc,
            &mut vanity_prefixes,
            None,
            &mut h,
            SlotRef::VanityAccessory(1),
            ClickKind::Right,
        ));
        assert_eq!(acc[1], None);
        assert_eq!(vanity_acc[1], Some(COBALT_SHIELD));
    }
}
