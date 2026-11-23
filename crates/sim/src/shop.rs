//! Town NPC shops and dialogue. Pure data plus atomic buy/sell on an inventory.
//!
//! Merchant stock: NPC 17 opens shop 1 (gameplay spec `Shop(17, 1)`,
//! `Chest.SetupShop` `case 1`, gameplay spec). Its list starts Mining Helmet,
//! Piggy Bank, Iron Anvil (35), Bug Net, Copper Pickaxe (3509), Copper Axe
//! (3506), Torch (8), Lesser Healing Potion (28), Lesser Mana Potion, Wooden Arrow
//! (40), ...; only items this game has are stocked. Skipped (missing items or
//! conditional on hardmode, biome, events): Mining Helmet, Piggy Bank, Bug Net,
//! Lesser Mana Potion, Shuriken, and everything after. The plan's guess of
//! `case 17` (gameplay spec) is a different shop; Bottle is not in the list.

use crate::coin;
use crate::facts::WorldFacts;
use crate::inventory::{Inventory, ItemStack};
use crate::item::{
    ANVIL_ITEM, BOMB, BUG_NET, COPPER_AXE, COPPER_PICKAXE, DYNAMITE, FLINTLOCK_PISTOL,
    GLOWING_MUSHROOM, GRENADE, HOOK, IRON_CHAIN, ItemId, LESSER_HEALING_POTION, LESSER_MANA_POTION,
    MINISHARK, MUSKET_BALL, Prefix, ROCKET_BOOTS, Rng, TINKERERS_WORKSHOP_ITEM, TORCH, VINE,
    WOODEN_ARROW,
};
use crate::town_npc::TownKind;
use crate::weather::WeatherKind;
use crate::worldgen::biome::Biome;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShopItem {
    pub item: ItemId,
    pub price_copper: u32,
}

/// `Item.value` in copper (delegates to `item_data(id).value`).
pub fn item_value(id: ItemId) -> u32 {
    crate::item::item_data(id).value
}

const fn si(item: ItemId, price_copper: u32) -> ShopItem {
    ShopItem { item, price_copper }
}

/// Buy price is `value * PriceAdjustment` with a neutral 1.0 here
/// (`Player.GetItemExpectedPrice`, gameplay spec).
const MERCHANT: [ShopItem; 8] = [
    si(ANVIL_ITEM, 5000),
    si(COPPER_PICKAXE, 500),
    si(COPPER_AXE, 400),
    si(TORCH, 50),
    si(LESSER_HEALING_POTION, 300),
    si(WOODEN_ARROW, 5),
    si(BUG_NET, 2500),
    si(LESSER_MANA_POTION, 100),
];

/// Arms Dealer stock: Musket Ball (7c), Flintlock Pistol (5g), Minishark (35g).
pub const ARMS_DEALER: [ShopItem; 3] = [
    si(MUSKET_BALL, 7),
    si(FLINTLOCK_PISTOL, 50_000),
    si(MINISHARK, 350_000),
];

pub const DEMOLITIONIST: [ShopItem; 3] = [si(GRENADE, 75), si(BOMB, 300), si(DYNAMITE, 2000)];

pub const DRYAD: [ShopItem; 2] = [si(GLOWING_MUSHROOM, 100), si(VINE, 400)];

pub const GOBLIN_TINKERER: [ShopItem; 4] = [
    si(TINKERERS_WORKSHOP_ITEM, 100_000),
    si(ROCKET_BOOTS, 50_000),
    si(HOOK, 2000),
    si(IRON_CHAIN, 200),
];

pub fn shop_stock(kind: TownKind) -> &'static [ShopItem] {
    match kind {
        TownKind::Merchant => &MERCHANT,
        TownKind::ArmsDealer => &ARMS_DEALER,
        TownKind::Demolitionist => &DEMOLITIONIST,
        TownKind::Dryad => &DRYAD,
        TownKind::GoblinTinkerer => &GOBLIN_TINKERER,
        TownKind::Guide | TownKind::Nurse => &[],
    }
}

/// Nurse healing cost in copper: 1c per missing HP + 75c per active debuff (min 1c if needed).
pub fn nurse_heal_cost(missing_hp: i32, debuff_count: u32) -> u32 {
    let raw = missing_hp.max(0) as u32 + debuff_count * 75;
    if missing_hp > 0 || debuff_count > 0 {
        raw.max(1)
    } else {
        0
    }
}

/// Goblin Tinkerer reforge cost in copper: 1/3 of item sell value (minimum 10 copper).
pub fn reforge_cost(item: ItemId) -> u32 {
    let val = item_value(item);
    if val == 0 { 0 } else { (val / 3).max(10) }
}

/// Whether `item` can be reforged by the Goblin Tinkerer.
pub fn is_reforgeable(item: ItemId) -> bool {
    let d = crate::item::item_data(item);
    matches!(
        d.class,
        crate::item::ItemClass::Melee
            | crate::item::ItemClass::Ranged
            | crate::item::ItemClass::Magic
            | crate::item::ItemClass::Tool
            | crate::item::ItemClass::Accessory
    ) && d.max_stack == 1
        && item_value(item) > 0
}

/// Pays the reforge cost and rolls a new prefix for `inv.slots[slot]`.
pub fn reforge_slot(inv: &mut Inventory, slot: usize, rng: &mut Rng) -> Option<(Prefix, u32)> {
    let st = inv.slots.get(slot).copied().flatten()?;
    if !is_reforgeable(st.id) {
        return None;
    }
    let cost = reforge_cost(st.id);
    if !coin::pay(inv, cost) {
        return None;
    }
    let mut new_prefix = crate::item::roll_prefix(st.id, &mut || rng.next_u32());
    if new_prefix == st.prefix {
        new_prefix = crate::item::roll_prefix(st.id, &mut || rng.next_u32());
    }
    if let Some(s) = inv.slots[slot].as_mut() {
        s.prefix = new_prefix;
    }
    Some((new_prefix, cost))
}

/// Buys up to `count` units of stock entry `index`, clamped by affordability
/// and inventory space. Returns the actual number of units bought.
pub fn buy_many(inv: &mut Inventory, kind: TownKind, index: usize, count: u16) -> u16 {
    if count == 0 {
        return 0;
    }
    let Some(&it) = shop_stock(kind).get(index) else {
        return 0;
    };
    let afford = if it.price_copper == 0 {
        count
    } else {
        (coin::total_copper(inv) / it.price_copper).min(count as u32) as u16
    };
    let mut bought = 0u16;
    for _ in 0..afford {
        let before = inv.clone();
        let stack = ItemStack {
            id: it.item,
            count: 1,
            prefix: Prefix(0),
        };
        if coin::pay(inv, it.price_copper) && inv.add(stack).is_none() {
            bought += 1;
        } else {
            *inv = before;
            break;
        }
    }
    bought
}

/// Buys one unit of stock entry `index`. Atomic: coins are paid and the item
/// added, or `inv` is left untouched.
pub fn buy(inv: &mut Inventory, kind: TownKind, index: usize) -> bool {
    buy_many(inv, kind, index, 1) == 1
}

/// Sells the whole stack in `slot` for `value / 5` per item, at least 1 copper
/// (Player.SellItem, gameplay spec). Coins and valueless items are refused.
pub fn sell(inv: &mut Inventory, slot: usize) -> bool {
    let Some(Some(s)) = inv.slots.get(slot).copied() else {
        return false;
    };
    let value = item_value(s.id);
    if value == 0 {
        return false;
    }
    let before = inv.clone();
    inv.slots[slot] = None;
    if coin::give_copper(inv, (value / 5).max(1) * s.count) {
        return true;
    }
    *inv = before;
    false
}

const GUIDE_LINES: [&str; 3] = [
    "Greetings. Gather wood and craft a workbench to get started.",
    "To live here you need a walled room with a door, a table, a chair and a torch.",
    "Nights are dangerous. Build a house and stay inside until dawn.",
];
const MERCHANT_LINES: [&str; 3] = [
    "Looking to buy something? I have torches, tools and potions.",
    "Coins talk. Care to see my wares?",
    "Best prices around, and I am the only shop in town.",
];
const ARMS_DEALER_LINES: [&str; 3] = [
    "Keep your finger off the trigger until you see the whites of their eyes.",
    "Hey, this ain't a movie. Bullets aren't free!",
    "Make it quick. I've got a date with a shotgun.",
];

/// Context for town NPC dialogue selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DialogueContext {
    pub weather: WeatherKind,
    pub is_day: bool,
    pub biome: Biome,
    pub facts: WorldFacts,
}

impl Default for DialogueContext {
    fn default() -> Self {
        Self {
            weather: WeatherKind::Clear,
            is_day: true,
            biome: Biome::Forest,
            facts: WorldFacts::default(),
        }
    }
}

/// Context-aware line for a town NPC, varying by weather, time, biome, and progression.
pub fn town_dialogue(kind: TownKind, ctx: &DialogueContext, rng: &mut Rng) -> &'static str {
    let mut pool: Vec<&'static str> = Vec::new();

    match (kind, ctx.weather) {
        (TownKind::Guide, WeatherKind::Rain) => {
            pool.push("The rain keeps the slimes down, though torches outside may sputter.");
            pool.push("A sturdy roof over your head is worth its weight in timber during rain.");
        }
        (TownKind::Guide, WeatherKind::Storm) => {
            pool.push("Stay indoors during this storm! Lightning and monsters roam outside.");
            pool.push("This storm is fierce—check your walls and keep the doors shut.");
        }
        (TownKind::Merchant, WeatherKind::Rain) => {
            pool.push("Rainy days are great for indoor shopping. Need any torches?");
            pool.push("Keep your goods dry! I have potions and tools if you are heading out.");
        }
        (TownKind::Merchant, WeatherKind::Storm) => {
            pool.push("Terrible weather for caravans, wonderful weather to buy supplies!");
            pool.push("Hear that thunder? Better stock up on healing potions right now.");
        }
        (TownKind::ArmsDealer, WeatherKind::Rain) => {
            pool.push("Keep your powder dry in this downpour.");
        }
        (TownKind::ArmsDealer, WeatherKind::Storm) => {
            pool.push("Thunder covers gunfire nicely, don't you think?");
        }
        (_, _) => {}
    }

    if !ctx.is_day {
        match kind {
            TownKind::Guide => {
                pool.push("Watch the shadows—zombies and demon eyes prowl until dawn.");
            }
            TownKind::Merchant => {
                pool.push("Nightfall brings out nasty customers. Keep a blade ready.");
            }
            TownKind::ArmsDealer => {
                pool.push("Good night for hunting, if you have enough silver bullets.");
            }
            _ => {}
        }
    }

    match (kind, ctx.biome) {
        (TownKind::Guide, Biome::Desert) => {
            pool.push("The desert heat is unforgiving; watch for antlions and sandstorms.");
        }
        (TownKind::Guide, Biome::Snow) => {
            pool.push("Bundle up! The snow biome hides ice caves beneath the frost.");
        }
        (TownKind::Guide, Biome::Jungle) => {
            pool.push("The jungle is rich in spores and danger—tread carefully.");
        }
        (TownKind::Merchant, Biome::Desert) => {
            pool.push("Sand gets in everything, even my coin purse!");
        }
        (TownKind::Merchant, Biome::Snow) => {
            pool.push("Brrr! Hot deals only—my hands are freezing out here.");
        }
        (TownKind::Merchant, Biome::Jungle) => {
            pool.push("Jungle explorers always need extra arrows and potions.");
        }
        (TownKind::ArmsDealer, Biome::Desert) => {
            pool.push("Desert wind makes long shots tricky—aim steady.");
        }
        (TownKind::ArmsDealer, Biome::Snow) => {
            pool.push("Cold metal sticks to your fingers, so keep your gloves on.");
        }
        (TownKind::ArmsDealer, Biome::Jungle) => {
            pool.push("Plenty of target practice out in the jungle.");
        }
        (_, _) => {}
    }

    if ctx.facts.downed_skeletron {
        match kind {
            TownKind::Guide => {
                pool.push("With Skeletron defeated, the dungeon's treasures are yours to claim.");
            }
            TownKind::Merchant => {
                pool.push("You conquered the dungeon's curse! Surely you found some gold?");
            }
            TownKind::ArmsDealer => {
                pool.push("Nice trophy from Skeletron. Now let's talk firepower.");
            }
            _ => {}
        }
    } else if ctx.facts.downed_eye {
        match kind {
            TownKind::Guide => {
                pool.push("The Eye of Cthulhu has fallen. Seek shadow orbs and stronger ore.");
            }
            TownKind::Merchant => {
                pool.push("Word of your victory over that giant eye is good for business!");
            }
            TownKind::ArmsDealer => {
                pool.push("Dropped that giant eye, huh? You've earned a Minishark upgrade.");
            }
            _ => {}
        }
    } else if ctx.facts.underworld_reached {
        match kind {
            TownKind::Guide => {
                pool.push("You have seen the underworld. Prepare well before disturbing it.");
            }
            TownKind::Merchant => {
                pool.push("Back from the underworld? Spend those coins before you melt!");
            }
            TownKind::ArmsDealer => {
                pool.push("Things get hot down there—bring plenty of lead.");
            }
            _ => {}
        }
    }

    if pool.is_empty() {
        let lines: &[&str] = match kind {
            TownKind::Guide => &GUIDE_LINES,
            TownKind::Merchant => &MERCHANT_LINES,
            TownKind::ArmsDealer => &ARMS_DEALER_LINES,
            TownKind::Nurse => &[
                "Hold still! This will only sting for a second.",
                "Let me patch those scratches right up.",
                "Stop jumping into lava if you want to keep your coins!",
            ],
            TownKind::Demolitionist => &[
                "Nothing solves a cave-in like a well-placed stick of dynamite!",
                "Why mine with a pickaxe when you can blow up the whole mountain?",
                "Keep your torches away from my pockets!",
            ],
            TownKind::Dryad => &[
                "The balance of nature is fragile. Keep the Corruption at bay.",
                "Listen to the trees; they remember every step you take.",
                "You must cleanse the land before the darkness spreads further.",
            ],
            TownKind::GoblinTinkerer => &[
                "I was kicked out for being too clever. Their loss, your gain!",
                "Give me a weapon and some coins—I'll reforge it into something legendary.",
                "Rocket boots? Oh yes, I never leave home without them.",
            ],
        };
        return lines[rng.below(lines.len() as u32) as usize];
    }

    pool[rng.below(pool.len() as u32) as usize]
}

/// A canned line for the NPC, picked at random.
pub fn dialogue(kind: TownKind, rng: &mut Rng) -> &'static str {
    town_dialogue(kind, &DialogueContext::default(), rng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{COPPER_COIN, WOOD};

    fn coins(n: u32) -> Inventory {
        let mut inv = Inventory::empty();
        assert!(coin::give_copper(&mut inv, n));
        inv
    }

    #[test]
    fn buy_adds_item_and_removes_coins() {
        let mut inv = coins(1000);
        assert!(buy(&mut inv, TownKind::Merchant, 3)); // Torch, 50
        assert_eq!(inv.count_of(TORCH), 1);
        assert_eq!(coin::total_copper(&inv), 950);
        assert!(!buy(&mut inv, TownKind::Merchant, 99));
        assert!(!buy(&mut coins(10), TownKind::Merchant, 3));
    }

    #[test]
    fn buy_fails_when_inventory_full_without_losing_coins() {
        let mut inv = coins(300); // 3 silver in one coin slot
        let free: Vec<_> = (0..crate::inventory::SLOTS)
            .filter(|&i| inv.slots[i].is_none())
            .collect();
        for i in free {
            inv.slots[i] = Some(ItemStack {
                id: WOOD,
                count: 1,
                prefix: Prefix(0),
            });
        }
        let before = inv.clone();
        // 250 change needs 2 coin stacks plus the torch slot.
        assert!(!buy(&mut inv, TownKind::Merchant, 3));
        assert_eq!(inv, before);
    }

    #[test]
    fn sell_gives_value_div_5() {
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack {
            id: TORCH,
            count: 4,
            prefix: Prefix(0),
        });
        inv.slots[1] = Some(ItemStack {
            id: COPPER_COIN,
            count: 1,
            prefix: Prefix(0),
        });
        assert!(sell(&mut inv, 0));
        assert_eq!(coin::total_copper(&inv), 1 + 4 * (50 / 5));
        assert_eq!(inv.count_of(TORCH), 0);
        assert!(!sell(&mut inv, 1), "no value, no sale");
        assert!(!sell(&mut inv, 2) && !sell(&mut inv, 99));
    }

    #[test]
    fn stock_only_lists_priced_items() {
        for s in shop_stock(TownKind::Merchant) {
            assert_eq!(s.price_copper, item_value(s.item));
        }
        assert!(shop_stock(TownKind::Guide).is_empty());
    }

    #[test]
    fn every_non_coin_item_has_nonzero_sell_value() {
        for id in crate::item::all_item_ids() {
            if !crate::item::is_coin(id) {
                assert!(
                    item_value(id) > 0,
                    "non-coin item {:?} has zero sell value",
                    id
                );
            }
        }
    }

    #[test]
    fn buy_many_buys_ten_and_clamps_all_to_affordability() {
        // 1000c buys 10 torches at 50c each -> 500c left.
        let mut inv = coins(1000);
        assert_eq!(buy_many(&mut inv, TownKind::Merchant, 3, 10), 10);
        assert_eq!(inv.count_of(TORCH), 10);
        assert_eq!(coin::total_copper(&inv), 500);

        // 250c with count = 10 clamps to 5 torches and 0c left.
        let mut inv = coins(250);
        assert_eq!(buy_many(&mut inv, TownKind::Merchant, 3, 10), 5);
        assert_eq!(inv.count_of(TORCH), 5);
        assert_eq!(coin::total_copper(&inv), 0);

        // 250c with count = u16::MAX ("All") clamps to 5 torches and 0c left.
        let mut inv = coins(250);
        assert_eq!(buy_many(&mut inv, TownKind::Merchant, 3, u16::MAX), 5);
        assert_eq!(inv.count_of(TORCH), 5);
        assert_eq!(coin::total_copper(&inv), 0);

        // Insufficient funds for even 1 unit returns 0 and leaves inventory untouched.
        let mut inv = coins(10);
        assert_eq!(buy_many(&mut inv, TownKind::Merchant, 3, 10), 0);
        assert_eq!(inv.count_of(TORCH), 0);
        assert_eq!(coin::total_copper(&inv), 10);
    }

    #[test]
    fn town_dialogue_varies_by_weather_time_and_biome() {
        use crate::facts::WorldFacts;
        use crate::weather::WeatherKind;
        use crate::worldgen::biome::Biome;

        let base = DialogueContext {
            weather: WeatherKind::Clear,
            is_day: true,
            biome: Biome::Forest,
            facts: WorldFacts::default(),
        };
        let rain = DialogueContext {
            weather: WeatherKind::Rain,
            ..base
        };
        let storm = DialogueContext {
            weather: WeatherKind::Storm,
            ..base
        };
        let night = DialogueContext {
            is_day: false,
            ..base
        };
        let snow = DialogueContext {
            biome: Biome::Snow,
            ..base
        };
        let desert = DialogueContext {
            biome: Biome::Desert,
            ..base
        };
        let jungle = DialogueContext {
            biome: Biome::Jungle,
            ..base
        };
        let prog = DialogueContext {
            facts: WorldFacts {
                downed_eye: true,
                ..WorldFacts::default()
            },
            ..base
        };

        for kind in [TownKind::Guide, TownKind::Merchant, TownKind::ArmsDealer] {
            let base_line = town_dialogue(kind, &base, &mut Rng::new(1));
            let rain_line = town_dialogue(kind, &rain, &mut Rng::new(1));
            let storm_line = town_dialogue(kind, &storm, &mut Rng::new(1));
            let night_line = town_dialogue(kind, &night, &mut Rng::new(1));
            let snow_line = town_dialogue(kind, &snow, &mut Rng::new(1));
            let desert_line = town_dialogue(kind, &desert, &mut Rng::new(1));
            let jungle_line = town_dialogue(kind, &jungle, &mut Rng::new(1));
            let prog_line = town_dialogue(kind, &prog, &mut Rng::new(1));

            assert_ne!(rain_line, base_line);
            assert_ne!(storm_line, base_line);
            assert_ne!(storm_line, rain_line);
            assert_ne!(night_line, base_line);
            assert_ne!(snow_line, base_line);
            assert_ne!(desert_line, base_line);
            assert_ne!(jungle_line, base_line);
            assert_ne!(prog_line, base_line);
        }
    }
}
