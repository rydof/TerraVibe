//! Coin arithmetic on an inventory: counting, paying with change, giving and
//! the auto-stacking of full coin stacks.

use crate::inventory::{Inventory, ItemStack};
use crate::item::{COPPER_COIN, GOLD_COIN, ItemId, PLATINUM_COIN, Prefix, SILVER_COIN};

/// Copper, silver, gold, platinum with their copper values (1 / 100 / 10000 /
/// 1000000, `NPC.merchant spawn check` gameplay spec).
const COINS: [(ItemId, u64); 4] = [
    (COPPER_COIN, 1),
    (SILVER_COIN, 100),
    (GOLD_COIN, 10_000),
    (PLATINUM_COIN, 1_000_000),
];

pub fn total_copper(inv: &Inventory) -> u32 {
    let t: u64 = COINS
        .iter()
        .map(|&(id, v)| inv.count_of(id) as u64 * v)
        .sum();
    t.min(u32::MAX as u64) as u32
}

pub fn coin_unit_value(id: ItemId) -> u32 {
    match id {
        COPPER_COIN => 1,
        SILVER_COIN => 100,
        GOLD_COIN => 10_000,
        PLATINUM_COIN => 1_000_000,
        _ => 0,
    }
}

/// Breaks `copper` into (platinum, gold, silver, copper) denominations.
pub fn split_coins(copper: u32) -> (u32, u32, u32, u32) {
    let p = copper / 1_000_000;
    let rem_p = copper % 1_000_000;
    let g = rem_p / 10_000;
    let rem_g = rem_p % 10_000;
    let s = rem_g / 100;
    let c = rem_g % 100;
    (p, g, s, c)
}

/// Formats `copper` as a space-separated string of non-zero denominations (e.g. "1p 2g 3s 4c").
/// If `copper` is 0, returns "0c".
pub fn format_coins(copper: u32) -> String {
    if copper == 0 {
        return "0c".to_string();
    }
    let (p, g, s, c) = split_coins(copper);
    let mut parts = Vec::new();
    if p > 0 {
        parts.push(format!("{p}p"));
    }
    if g > 0 {
        parts.push(format!("{g}g"));
    }
    if s > 0 {
        parts.push(format!("{s}s"));
    }
    if c > 0 {
        parts.push(format!("{c}c"));
    }
    parts.join(" ")
}

/// `Player.DoCoins` (gameplay spec): a stack of exactly 100 copper, silver or
/// gold becomes one coin of the next denomination (merging into its stack), and
/// the check repeats upward. Platinum stops at its stack limit.
pub fn normalize(inv: &mut Inventory) {
    'again: loop {
        for slot in 0..inv.slots.len() {
            let Some(s) = inv.slots[slot] else { continue };
            let Some(i) = COINS[..3].iter().position(|&(id, _)| id == s.id) else {
                continue;
            };
            if s.count == 100 {
                inv.slots[slot] = None;
                let up = ItemStack {
                    id: COINS[i + 1].0,
                    count: 1,
                    prefix: Prefix(0),
                };
                // The freed slot guarantees room.
                inv.add(up);
                continue 'again;
            }
        }
        return;
    }
}

/// Adds `amount` copper as the fewest coins. Atomic: `false` leaves `inv` as it was.
pub fn give_copper(inv: &mut Inventory, amount: u32) -> bool {
    let before = inv.clone();
    let mut left = amount as u64;
    for &(id, v) in COINS.iter().rev() {
        let n = left / v;
        left %= v;
        if n > 0
            && inv
                .add(ItemStack {
                    id,
                    count: n as u32,
                    prefix: Prefix(0),
                })
                .is_some()
        {
            *inv = before;
            return false;
        }
    }
    normalize(inv);
    true
}

/// Takes `cost` copper. Coins are pooled and the remainder is handed back in the
/// correct denominations, so change is always exact. Atomic.
pub fn pay(inv: &mut Inventory, cost: u32) -> bool {
    let total = total_copper(inv);
    if total < cost {
        return false;
    }
    let before = inv.clone();
    for &(id, _) in &COINS {
        let n = inv.count_of(id);
        inv.remove(id, n);
    }
    if !give_copper(inv, total - cost) {
        *inv = before;
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(coins: &[(ItemId, u32)]) -> Inventory {
        let mut inv = Inventory::empty();
        for &(id, count) in coins {
            assert!(
                inv.add(ItemStack {
                    id,
                    count,
                    prefix: Prefix(0)
                })
                .is_none()
            );
        }
        inv
    }

    #[test]
    fn format_coins_denominations() {
        assert_eq!(split_coins(0), (0, 0, 0, 0));
        assert_eq!(split_coins(105), (0, 0, 1, 5));
        assert_eq!(split_coins(1_020_304), (1, 2, 3, 4));

        assert_eq!(format_coins(0), "0c");
        assert_eq!(format_coins(99), "99c");
        assert_eq!(format_coins(100), "1s");
        assert_eq!(format_coins(105), "1s 5c");
        assert_eq!(format_coins(9999), "99s 99c");
        assert_eq!(format_coins(10_000), "1g");
        assert_eq!(format_coins(1_000_000), "1p");
        assert_eq!(format_coins(1_020_304), "1p 2g 3s 4c");
    }

    #[test]
    fn test_coin_unit_value() {
        assert_eq!(coin_unit_value(COPPER_COIN), 1);
        assert_eq!(coin_unit_value(SILVER_COIN), 100);
        assert_eq!(coin_unit_value(GOLD_COIN), 10_000);
        assert_eq!(coin_unit_value(PLATINUM_COIN), 1_000_000);
        assert_eq!(coin_unit_value(ItemId(9999)), 0);
    }

    #[test]
    fn coins_stack_to_next_denomination() {
        let mut inv = with(&[(COPPER_COIN, 99)]);
        assert!(give_copper(&mut inv, 1));
        assert_eq!(inv.count_of(COPPER_COIN), 0);
        assert_eq!(inv.count_of(SILVER_COIN), 1);
        let mut inv = with(&[(SILVER_COIN, 99), (COPPER_COIN, 99)]);
        give_copper(&mut inv, 1);
        assert_eq!((inv.count_of(GOLD_COIN), inv.count_of(SILVER_COIN)), (1, 0));
        assert_eq!(total_copper(&inv), 10_000);
    }

    #[test]
    fn pay_makes_change_correctly() {
        let mut inv = with(&[(GOLD_COIN, 1)]);
        assert!(pay(&mut inv, 1));
        assert_eq!(total_copper(&inv), 9_999);
        assert_eq!(
            (inv.count_of(SILVER_COIN), inv.count_of(COPPER_COIN)),
            (99, 99)
        );
        assert_eq!(inv.count_of(GOLD_COIN), 0);
    }

    #[test]
    fn pay_fails_without_funds_and_changes_nothing() {
        let mut inv = with(&[(SILVER_COIN, 3)]);
        let before = inv.clone();
        assert!(!pay(&mut inv, 301));
        assert_eq!(inv, before);
        assert!(pay(&mut inv, 300));
        assert_eq!(total_copper(&inv), 0);
    }
}
