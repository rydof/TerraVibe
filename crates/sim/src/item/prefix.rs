//! Terraria prefixes: stat multipliers, rolling and names.
//! modeled after gameplay spec (`prefix stat multipliers`, `Prefix`,
//! `GetRollablePrefixes`), `Terraria/ID/gameplay spec and gameplay spec.

use super::{ItemClass, ItemId, item_data};

/// A prefix id (0 = none). Ids are `PrefixID`'s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefix(pub u8);

/// Multipliers a prefix applies. 1.0 / 0 mean "unchanged".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrefixStats {
    pub damage: f32,
    pub knock_back: f32,
    /// Multiplies use time and animation (lower is faster).
    pub use_time: f32,
    pub scale: f32,
    pub crit: i32,
    pub velocity: f32,
    pub mana: f32,
}

/// `Item.prefix stat multipliers` switch: (dmg, kb, spd, size, shtspd, mcst, crt).
pub fn prefix_stats(p: Prefix) -> PrefixStats {
    let (damage, knock_back, use_time, scale, velocity, mana, crit) = match p.0 {
        1 => (1.0, 1.0, 1.0, 1.12, 1.0, 1.0, 0),
        2 => (1.0, 1.0, 1.0, 1.18, 1.0, 1.0, 0),
        3 => (1.05, 1.0, 1.0, 1.05, 1.0, 1.0, 2),
        4 => (1.1, 1.1, 1.0, 1.1, 1.0, 1.0, 0),
        5 => (1.15, 1.0, 1.0, 1.0, 1.0, 1.0, 0),
        6 => (1.1, 1.0, 1.0, 1.0, 1.0, 1.0, 0),
        81 => (1.15, 1.15, 0.9, 1.1, 1.0, 1.0, 5),
        7 => (1.0, 1.0, 1.0, 0.82, 1.0, 1.0, 0),
        8 => (0.85, 0.85, 1.0, 0.87, 1.0, 1.0, 0),
        9 => (1.0, 1.0, 1.0, 0.9, 1.0, 1.0, 0),
        10 => (0.85, 1.0, 1.0, 1.0, 1.0, 1.0, 0),
        11 => (1.0, 0.9, 1.1, 0.9, 1.0, 1.0, 0),
        12 => (1.05, 1.1, 1.15, 1.1, 1.0, 1.0, 0),
        13 => (0.9, 0.8, 1.0, 1.1, 1.0, 1.0, 0),
        14 => (1.0, 1.15, 1.1, 1.0, 1.0, 1.0, 0),
        15 => (1.0, 0.9, 0.85, 1.0, 1.0, 1.0, 0),
        36 => (1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 3),
        37 => (1.1, 1.1, 1.0, 1.0, 1.0, 1.0, 3),
        38 => (1.0, 1.15, 1.0, 1.0, 1.0, 1.0, 0),
        53 => (1.1, 1.0, 1.0, 1.0, 1.0, 1.0, 0),
        54 => (1.0, 1.15, 1.0, 1.0, 1.0, 1.0, 0),
        55 => (1.05, 1.15, 1.0, 1.0, 1.0, 1.0, 0),
        59 => (1.15, 1.15, 1.0, 1.0, 1.0, 1.0, 5),
        60 => (1.15, 1.0, 1.0, 1.0, 1.0, 1.0, 5),
        61 => (1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 5),
        39 => (0.7, 0.8, 1.0, 1.0, 1.0, 1.0, 0),
        40 => (0.85, 1.0, 1.0, 1.0, 1.0, 1.0, 0),
        56 => (1.0, 0.8, 1.0, 1.0, 1.0, 1.0, 0),
        41 => (0.9, 0.85, 1.0, 1.0, 1.0, 1.0, 0),
        57 => (1.18, 0.9, 1.0, 1.0, 1.0, 1.0, 0),
        42 => (1.0, 1.0, 0.9, 1.0, 1.0, 1.0, 0),
        43 => (1.1, 1.0, 0.9, 1.0, 1.0, 1.0, 0),
        44 => (1.0, 1.0, 0.9, 1.0, 1.0, 1.0, 3),
        45 => (1.0, 1.0, 0.95, 1.0, 1.0, 1.0, 0),
        46 => (1.07, 1.0, 0.94, 1.0, 1.0, 1.0, 3),
        47 => (1.0, 1.0, 1.15, 1.0, 1.0, 1.0, 0),
        48 => (1.0, 1.0, 1.2, 1.0, 1.0, 1.0, 0),
        49 => (1.0, 1.0, 1.08, 1.0, 1.0, 1.0, 0),
        50 => (0.8, 1.0, 1.15, 1.0, 1.0, 1.0, 0),
        51 => (1.05, 0.9, 0.9, 1.0, 1.0, 1.0, 2),
        // Ranged prefixes
        16 => (1.1, 1.0, 1.0, 1.0, 1.0, 1.0, 3),   // Sighted
        17 => (1.0, 1.0, 0.85, 1.0, 1.1, 1.0, 0),  // Rapid
        18 => (1.0, 1.0, 0.9, 1.0, 1.15, 1.0, 0),  // Hasty
        19 => (1.0, 1.15, 1.0, 1.0, 1.05, 1.0, 0), // Intimidating
        20 => (1.1, 1.05, 0.9, 1.0, 1.05, 1.0, 2), // Deadly (ranged)
        21 => (1.1, 1.15, 1.0, 1.0, 1.0, 1.0, 0),  // Staunch
        22 => (0.85, 0.9, 1.0, 1.0, 0.9, 1.0, 0),  // Awful
        23 => (1.0, 1.0, 1.15, 1.0, 0.9, 1.0, 0),  // Lethargic
        24 => (1.0, 0.8, 1.1, 1.0, 1.0, 1.0, 0),   // Awkward
        25 => (1.15, 1.0, 1.1, 1.0, 1.0, 1.0, 1),  // Powerful
        58 => (0.85, 1.0, 0.85, 1.0, 1.0, 1.0, 0), // Frenzying
        82 => (1.15, 1.15, 0.9, 1.0, 1.1, 1.0, 5), // Unreal
        // Magic prefixes
        26 => (1.1, 1.0, 1.0, 1.0, 1.0, 0.85, 0),  // Mystic
        27 => (1.0, 1.0, 1.0, 1.0, 1.0, 0.85, 0),  // Adept
        28 => (1.15, 1.05, 1.0, 1.0, 1.0, 0.9, 0), // Masterful
        29 => (1.0, 1.0, 1.0, 1.0, 1.0, 1.1, 0),   // Inept
        30 => (0.9, 1.0, 1.0, 1.0, 1.0, 1.2, 0),   // Ignorant
        31 => (0.9, 0.9, 1.0, 1.0, 1.0, 1.0, 0),   // Deranged
        32 => (1.1, 1.0, 1.0, 1.0, 1.0, 1.15, 0),  // Intense
        33 => (1.0, 1.1, 0.9, 1.0, 1.0, 1.1, 0),   // Taboo
        34 => (1.1, 1.1, 1.1, 1.0, 1.0, 0.9, 0),   // Celestial
        35 => (1.15, 1.1, 1.0, 1.0, 1.0, 1.2, 0),  // Furious
        52 => (0.9, 1.0, 0.9, 1.0, 1.0, 0.9, 0),   // Manic
        83 => (1.15, 1.15, 0.9, 1.0, 1.0, 0.9, 5), // Mythical
        _ => (1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0),
    };
    PrefixStats {
        damage,
        knock_back,
        use_time,
        scale,
        crit,
        velocity,
        mana,
    }
}

/// Mana a cast costs after a prefix: `Item.mana * prefix mana multiplier`, rounded
/// the way Terraria rounds `GetPrefixStatMultipliersForItem` consumers.
pub fn mana_cost(base: i32, p: Prefix) -> i32 {
    (base as f32 * prefix_stats(p).mana).round_ties_even() as i32
}

/// Stat bonuses granted by an equipped accessory prefix (`PrefixID` 62..=80).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccessoryPrefixBonus {
    pub defense: i32,
    pub mana: i32,
    pub crit: i32,
    pub damage_pct: i32,
    pub move_speed_pct: i32,
    pub melee_speed_pct: i32,
}

/// Returns the flat/percentage bonuses for accessory prefix `p`.
pub fn accessory_prefix_bonus(p: Prefix) -> AccessoryPrefixBonus {
    let mut b = AccessoryPrefixBonus::default();
    match p.0 {
        62..=65 => b.defense = (p.0 - 61) as i32,
        66 => b.mana = 20,
        67 => b.crit = 2,
        68 => b.crit = 4,
        69..=72 => b.damage_pct = (p.0 - 68) as i32,
        73..=76 => b.move_speed_pct = (p.0 - 72) as i32,
        77..=80 => b.melee_speed_pct = (p.0 - 76) as i32,
        _ => {}
    }
    b
}

/// `PrefixLegacy.Prefixes.PrefixesForSwords`: the list swords, hammers, axes and
/// pickaxes (including the copper tools) roll from.
const PREFIXES_FOR_SWORDS: [u8; 40] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 36, 37, 38, 53, 54, 55, 39, 40, 56, 41, 57,
    42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 59, 60, 61, 81,
];

const PREFIXES_FOR_RANGED: [u8; 27] = [
    16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 36, 37, 38, 39, 40, 41, 53, 54, 55, 56, 57, 58, 59, 60,
    61, 82, 42,
];

const PREFIXES_FOR_MAGIC: [u8; 27] = [
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 52, 53, 54, 55, 56, 57, 59, 60,
    61, 83, 42,
];

const PREFIXES_FOR_ACCESSORIES: [u8; 19] = [
    62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80,
];

/// `PrefixID.Sets.ReducedNaturalChance`.
fn reduced_natural_chance(p: u8) -> bool {
    matches!(p, 7..=11 | 22..=24 | 29..=31 | 39 | 40 | 56 | 41 | 47..=49)
}

/// `Item.GetRollablePrefixes`: `None` when the item cannot have prefixes. Copper
/// shortswords, pickaxes and axes are in `SwordsHammersAxesPicks`.
fn rollable_prefixes(id: ItemId) -> Option<&'static [u8]> {
    match item_data(id).class {
        ItemClass::Melee | ItemClass::Tool => Some(&PREFIXES_FOR_SWORDS),
        ItemClass::Ranged => Some(&PREFIXES_FOR_RANGED),
        ItemClass::Magic => Some(&PREFIXES_FOR_MAGIC),
        ItemClass::Accessory => Some(&PREFIXES_FOR_ACCESSORIES),
        _ => None,
    }
}

/// The end of `prefix stat multipliers`: a prefix is refused when its
/// multiplier would not change the rounded stat (C# `Math.Round` is ties-to-even).
fn prefix_fits(id: ItemId, p: Prefix) -> bool {
    let d = item_data(id);
    if d.class == ItemClass::Accessory {
        return p.0 == 0 || (62..=80).contains(&p.0);
    }
    let s = prefix_stats(p);
    let unchanged = |base: i32, m: f32| (base as f32 * m).round_ties_even() as i32 == base;
    if s.damage != 1.0 && unchanged(d.damage, s.damage) {
        return false;
    }
    if s.use_time != 1.0 && unchanged(d.use_animation, s.use_time) {
        return false;
    }
    if s.knock_back != 1.0 && d.knock_back == 0.0 {
        return false;
    }
    true
}

/// `Item.Prefix(-1)` for a new item: 1 in 4 stays unprefixed, otherwise a random
/// eligible prefix; `ReducedNaturalChance` ones are 2/3 downgraded to none, and
/// prefixes that would not change the rounded stats are rerolled.
pub fn roll_prefix(id: ItemId, rng: &mut impl FnMut() -> u32) -> Prefix {
    let Some(list) = rollable_prefixes(id) else {
        return Prefix(0);
    };
    loop {
        let mut rolled = if rng() % 4 == 0 { Some(0u8) } else { None };
        let mut p = match rolled.take() {
            Some(p) => p,
            None => list[rng() as usize % list.len()],
        };
        if p != 0 && reduced_natural_chance(p) && rng() % 3 != 0 {
            p = 0;
        }
        if prefix_fits(id, Prefix(p)) {
            return Prefix(p);
        }
    }
}

/// English names from `Terraria.Localization.Content.en-US.Items.json` ("Prefix"),
/// indexed by `PrefixID`.
const PREFIX_NAMES: [&str; 98] = [
    "",
    "Large",
    "Massive",
    "Dangerous",
    "Savage",
    "Sharp",
    "Pointy",
    "Tiny",
    "Terrible",
    "Small",
    "Dull",
    "Unhappy",
    "Bulky",
    "Shameful",
    "Heavy",
    "Light",
    "Sighted",
    "Rapid",
    "Hasty",
    "Intimidating",
    "Deadly",
    "Staunch",
    "Awful",
    "Lethargic",
    "Awkward",
    "Powerful",
    "Mystic",
    "Adept",
    "Masterful",
    "Inept",
    "Ignorant",
    "Deranged",
    "Intense",
    "Taboo",
    "Celestial",
    "Furious",
    "Keen",
    "Superior",
    "Forceful",
    "Broken",
    "Damaged",
    "Shoddy",
    "Quick",
    "Deadly",
    "Agile",
    "Nimble",
    "Murderous",
    "Slow",
    "Sluggish",
    "Lazy",
    "Annoying",
    "Nasty",
    "Manic",
    "Hurtful",
    "Strong",
    "Unpleasant",
    "Weak",
    "Ruthless",
    "Frenzying",
    "Godly",
    "Demonic",
    "Zealous",
    "Hard",
    "Guarding",
    "Armored",
    "Warding",
    "Arcane",
    "Precise",
    "Lucky",
    "Jagged",
    "Spiked",
    "Angry",
    "Menacing",
    "Brisk",
    "Fleeting",
    "Hasty",
    "Quick",
    "Wild",
    "Rash",
    "Intrepid",
    "Violent",
    "Legendary",
    "Unreal",
    "Mythical",
    "Legendary",
    "Fabled",
    "Loyal",
    "Worthy",
    "Focused",
    "Patient",
    "Rabid",
    "Ill-Tempered",
    "Petty",
    "Feeble",
    "Skittish",
    "Eager",
    "Ballistic",
    "Scraggling",
];

/// The prefix word, or "" for none/unknown.
pub fn prefix_name(p: Prefix) -> &'static str {
    PREFIX_NAMES.get(p.0 as usize).copied().unwrap_or("")
}

/// `Item.AffixName`: "Legendary Copper Shortsword".
pub fn display_name(id: ItemId, p: Prefix) -> String {
    let name = item_data(id).name;
    match prefix_name(p) {
        "" => name.to_string(),
        pre => format!("{pre} {name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::*;

    fn rolls(id: ItemId, n: usize) -> Vec<u8> {
        let mut r = Rng::new(0xC0FFEE);
        let mut f = || r.next_u32();
        (0..n).map(|_| roll_prefix(id, &mut f).0).collect()
    }

    #[test]
    fn prefix_table_samples() {
        // Item.prefix stat multipliers: case 81 Legendary, 59 Godly, 42 Quick, 15 Light, 39 Broken.
        let s = prefix_stats(Prefix(81));
        assert_eq!(
            (s.damage, s.knock_back, s.use_time, s.scale, s.crit),
            (1.15, 1.15, 0.9, 1.1, 5)
        );
        let s = prefix_stats(Prefix(59));
        assert_eq!(
            (s.damage, s.knock_back, s.use_time, s.scale, s.crit),
            (1.15, 1.15, 1.0, 1.0, 5)
        );
        let s = prefix_stats(Prefix(42));
        assert_eq!(
            (s.damage, s.knock_back, s.use_time, s.crit),
            (1.0, 1.0, 0.9, 0)
        );
        let s = prefix_stats(Prefix(15));
        assert_eq!((s.damage, s.knock_back, s.use_time), (1.0, 0.9, 0.85));
        let s = prefix_stats(Prefix(39));
        assert_eq!((s.damage, s.knock_back, s.use_time), (0.7, 0.8, 1.0));
        assert_eq!(prefix_name(Prefix(81)), "Legendary");
        assert_eq!(prefix_name(Prefix(59)), "Godly");
        assert_eq!(prefix_name(Prefix(0)), "");
    }

    #[test]
    fn roll_is_eligible() {
        // PrefixLegacy.PrefixesForSwords, minus the prefixes whose rounded stat change is nil
        // for the item (Item.prefix stat multipliers tail), worked out by hand.
        let swords = PREFIXES_FOR_SWORDS;
        let pick_excluded = [3u8, 4, 6, 12, 13, 37, 41, 43, 46, 51, 53, 55];
        let sword_excluded = [3u8, 12, 46, 51, 55];
        for (id, excluded) in [
            (COPPER_PICKAXE, &pick_excluded[..]),
            (COPPER_SHORTSWORD, &sword_excluded[..]),
        ] {
            let r = rolls(id, 10_000);
            let mut distinct: Vec<u8> = r.iter().copied().filter(|&p| p != 0).collect();
            distinct.sort();
            distinct.dedup();
            assert!(distinct.len() >= 5);
            for p in distinct {
                assert!(swords.contains(&p), "{p} not rollable");
                assert!(!excluded.contains(&p), "{p} should be refused");
            }
        }
    }

    #[test]
    fn roll_none_rate() {
        // Item.Prefix(-1): 1/4 none; else pick of 40, ReducedNaturalChance 2/3 -> none, refused
        // picks reroll. Solving that loop gives 0.500 for the pickaxe and 0.441 for the shortsword.
        for (id, want) in [(COPPER_PICKAXE, 0.500), (COPPER_SHORTSWORD, 0.4414)] {
            let r = rolls(id, 10_000);
            let share = r.iter().filter(|&&p| p == 0).count() as f64 / r.len() as f64;
            assert!((share - want).abs() < 0.03, "{share} vs {want}");
        }
    }

    #[test]
    fn display_name_formats() {
        assert_eq!(display_name(COPPER_PICKAXE, Prefix(0)), "Copper Pickaxe");
        assert_eq!(
            display_name(COPPER_SHORTSWORD, Prefix(81)),
            "Legendary Copper Shortsword"
        );
    }
}
