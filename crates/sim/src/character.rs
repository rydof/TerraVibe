//! Character data model, appearance presets, validation and versioned file format.

use crate::codec::{DecodeError, Reader, Writer};
use crate::facts::CharacterFacts;
use crate::inventory::Inventory;
use crate::item::{ItemId, Prefix, Rng};

pub const FORMAT_VERSION: u8 = 5;
pub const MAGIC: &[u8; 4] = b"TVCH";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Difficulty {
    #[default]
    Softcore,
}

impl Difficulty {
    pub const fn to_u8(self) -> u8 {
        match self {
            Difficulty::Softcore => 0,
        }
    }

    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Difficulty::Softcore),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HairStyle {
    #[default]
    Short,
    Long,
    Spiky,
    Mohawk,
    Ponytail,
    Bald,
}

impl HairStyle {
    pub const ALL: [HairStyle; 6] = [
        HairStyle::Short,
        HairStyle::Long,
        HairStyle::Spiky,
        HairStyle::Mohawk,
        HairStyle::Ponytail,
        HairStyle::Bald,
    ];

    pub const fn to_u8(self) -> u8 {
        match self {
            HairStyle::Short => 0,
            HairStyle::Long => 1,
            HairStyle::Spiky => 2,
            HairStyle::Mohawk => 3,
            HairStyle::Ponytail => 4,
            HairStyle::Bald => 5,
        }
    }

    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(HairStyle::Short),
            1 => Some(HairStyle::Long),
            2 => Some(HairStyle::Spiky),
            3 => Some(HairStyle::Mohawk),
            4 => Some(HairStyle::Ponytail),
            5 => Some(HairStyle::Bald),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            HairStyle::Short => "Short",
            HairStyle::Long => "Long",
            HairStyle::Spiky => "Spiky",
            HairStyle::Mohawk => "Mohawk",
            HairStyle::Ponytail => "Ponytail",
            HairStyle::Bald => "Bald",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Appearance {
    pub hair: HairStyle,
    pub skin: [u8; 3],
    pub hair_color: [u8; 3],
    pub eyes: [u8; 3],
    pub shirt: [u8; 3],
    pub pants: [u8; 3],
}

pub const SKIN_PRESETS: [[u8; 3]; 6] = [
    [255, 206, 175], // pale / fair
    [234, 182, 142], // peach
    [199, 142, 102], // tan
    [153, 98, 64],   // brown
    [102, 62, 38],   // dark
    [210, 190, 160], // olive
];

pub const HAIR_COLOR_PRESETS: [[u8; 3]; 6] = [
    [160, 90, 44],   // brown
    [230, 180, 80],  // blonde
    [40, 40, 40],    // black
    [190, 50, 30],   // red
    [180, 180, 180], // grey
    [80, 160, 220],  // blue
];

pub const EYE_PRESETS: [[u8; 3]; 6] = [
    [40, 100, 180],  // blue
    [70, 140, 60],   // green
    [90, 60, 30],    // brown
    [120, 120, 120], // hazel / grey
    [140, 40, 140],  // violet
    [180, 100, 40],  // amber
];

pub const SHIRT_PRESETS: [[u8; 3]; 6] = [
    [200, 40, 40],   // red
    [40, 100, 200],  // blue
    [40, 160, 60],   // green
    [220, 180, 40],  // yellow
    [140, 60, 180],  // purple
    [220, 220, 220], // white
];

pub const PANTS_PRESETS: [[u8; 3]; 6] = [
    [40, 60, 120],   // blue jeans
    [50, 50, 50],    // charcoal
    [110, 80, 50],   // khakis
    [120, 40, 40],   // crimson
    [40, 90, 50],    // olive
    [140, 140, 140], // grey
];

impl Default for Appearance {
    fn default() -> Self {
        Self {
            hair: HairStyle::Short,
            skin: SKIN_PRESETS[0],
            hair_color: HAIR_COLOR_PRESETS[0],
            eyes: EYE_PRESETS[0],
            shirt: SHIRT_PRESETS[0],
            pants: PANTS_PRESETS[0],
        }
    }
}

/// Why a character name is unusable.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NameError {
    Empty,
    TooLong,
    Illegal(char),
}

/// Single source of truth for character names: trimmed, 1–32 chars, ASCII
/// letters/digits/space/`_`/`-`.
pub fn validate_name(name: &str) -> Result<&str, NameError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name.chars().count() > 32 {
        return Err(NameError::TooLong);
    }
    for c in name.chars() {
        if !(c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '-')) {
            return Err(NameError::Illegal(c));
        }
    }
    Ok(name)
}

impl Default for Character {
    fn default() -> Self {
        Self::new("Player", Appearance::default())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Character {
    pub name: String,
    pub difficulty: Difficulty,
    pub appearance: Appearance,
    pub inventory: Inventory,
    pub armor: [Option<ItemId>; 3],
    pub accessories: [Option<ItemId>; 5],
    pub accessory_prefixes: [Prefix; 5],
    pub vanity_armor: [Option<ItemId>; 3],
    pub vanity_accessories: [Option<ItemId>; 5],
    pub vanity_accessory_prefixes: [Prefix; 5],
    pub life_max: i32,
    pub life: i32,
    pub mana_max: i32,
    pub mana: i32,
    pub potion_delay: i32,
    pub facts: CharacterFacts,
    pub obsidian_skin_ticks: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CharacterError {
    BadMagic,
    UnknownVersion(u8),
    Decode,
    InvalidName(NameError),
}

impl From<DecodeError> for CharacterError {
    fn from(_: DecodeError) -> Self {
        CharacterError::Decode
    }
}

impl Character {
    /// Creates a fresh character with starting kit: 100 life, 20 mana, starting tools and copper coins.
    pub fn new(name: &str, appearance: Appearance) -> Self {
        let valid_name = validate_name(name).unwrap_or("Player");
        let mut rng = Rng::new(valid_name.bytes().fold(0x9E37_79B9u64, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as u64)
        }));
        let inventory = Inventory::starting(&mut rng);
        Self {
            name: valid_name.to_string(),
            difficulty: Difficulty::Softcore,
            appearance,
            inventory,
            armor: [None; 3],
            accessories: [None; 5],
            accessory_prefixes: [Prefix(0); 5],
            vanity_armor: [None; 3],
            vanity_accessories: [None; 5],
            vanity_accessory_prefixes: [Prefix(0); 5],
            life_max: 100,
            life: 100,
            mana_max: 20,
            mana: 20,
            potion_delay: 0,
            facts: CharacterFacts::default(),
            obsidian_skin_ticks: 0,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.bytes(MAGIC);
        w.u8(FORMAT_VERSION);
        w.u8(self.difficulty.to_u8());

        let name_bytes = self.name.as_bytes();
        w.u8(name_bytes.len().min(255) as u8);
        w.bytes(&name_bytes[..name_bytes.len().min(255)]);

        w.u8(self.appearance.hair.to_u8());
        for c in [
            self.appearance.skin,
            self.appearance.hair_color,
            self.appearance.eyes,
            self.appearance.shirt,
            self.appearance.pants,
        ] {
            w.u8(c[0]);
            w.u8(c[1]);
            w.u8(c[2]);
        }

        w.i32(self.life_max);
        w.i32(self.life);
        w.i32(self.mana_max);
        w.i32(self.mana);
        w.i32(self.potion_delay);

        let inv_bytes = self.inventory.to_bytes();
        w.u32(inv_bytes.len() as u32);
        w.bytes(&inv_bytes);

        for a in self.armor {
            w.u16(a.map_or(u16::MAX, |i| i.0));
        }
        for a in self.accessories {
            w.u16(a.map_or(u16::MAX, |i| i.0));
        }

        w.u8(self.facts.onboarding_dismissed as u8);
        w.u32(self.facts.advice_seen);
        w.u8(self.facts.first_night_survived as u8);
        w.u8(self.facts.has_shelter as u8);
        w.u8(self.facts.has_ore as u8);
        w.i32(self.obsidian_skin_ticks);
        for p in self.accessory_prefixes {
            w.u8(p.0);
        }
        for a in self.vanity_armor {
            w.u16(a.map_or(u16::MAX, |i| i.0));
        }
        for a in self.vanity_accessories {
            w.u16(a.map_or(u16::MAX, |i| i.0));
        }
        for p in self.vanity_accessory_prefixes {
            w.u8(p.0);
        }

        w.0
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self, CharacterError> {
        if b.len() < 5 {
            return Err(CharacterError::Decode);
        }
        if &b[0..4] != MAGIC {
            return Err(CharacterError::BadMagic);
        }
        let version = b[4];
        if !(1..=FORMAT_VERSION).contains(&version) {
            return Err(CharacterError::UnknownVersion(version));
        }

        let mut r = Reader(&b[5..]);
        let diff_u8 = r.u8()?;
        let difficulty = Difficulty::from_u8(diff_u8).ok_or(CharacterError::Decode)?;

        let name_len = r.u8()? as usize;
        let name_bytes = r.take(name_len)?;
        let name_str = std::str::from_utf8(name_bytes).map_err(|_| CharacterError::Decode)?;
        let name = validate_name(name_str)
            .map_err(CharacterError::InvalidName)?
            .to_string();

        let hair_u8 = r.u8()?;
        let hair = HairStyle::from_u8(hair_u8).ok_or(CharacterError::Decode)?;
        let read_rgb =
            |r: &mut Reader| -> Result<[u8; 3], CharacterError> { Ok([r.u8()?, r.u8()?, r.u8()?]) };
        let appearance = Appearance {
            hair,
            skin: read_rgb(&mut r)?,
            hair_color: read_rgb(&mut r)?,
            eyes: read_rgb(&mut r)?,
            shirt: read_rgb(&mut r)?,
            pants: read_rgb(&mut r)?,
        };

        let life_max = r.i32()?;
        let life = r.i32()?;
        let mana_max = r.i32()?;
        let mana = r.i32()?;
        let potion_delay = r.i32()?;

        let inv_len = r.u32()? as usize;
        let inv_bytes = r.take(inv_len)?;
        let inventory = Inventory::from_bytes(inv_bytes).map_err(|_| CharacterError::Decode)?;

        let mut armor = [None; 3];
        for a in armor.iter_mut() {
            let v = r.u16()?;
            *a = (v != u16::MAX).then_some(ItemId(v));
        }

        let mut accessories = [None; 5];
        for a in accessories.iter_mut() {
            let v = r.u16()?;
            *a = (v != u16::MAX).then_some(ItemId(v));
        }

        let facts = if version == 1 {
            CharacterFacts::default()
        } else {
            let onboarding_dismissed = r.u8()? != 0;
            let advice_seen = r.u32()?;
            let first_night_survived = r.u8()? != 0;
            let has_shelter = r.u8()? != 0;
            let has_ore = r.u8()? != 0;
            CharacterFacts {
                onboarding_dismissed,
                advice_seen,
                first_night_survived,
                has_shelter,
                has_ore,
            }
        };

        let obsidian_skin_ticks = if version >= 3 { r.i32()? } else { 0 };
        let mut accessory_prefixes = [Prefix(0); 5];
        if version >= 4 {
            for p in accessory_prefixes.iter_mut() {
                *p = Prefix(r.u8()?);
            }
        }
        let mut vanity_armor = [None; 3];
        let mut vanity_accessories = [None; 5];
        let mut vanity_accessory_prefixes = [Prefix(0); 5];
        if version >= 5 {
            for a in vanity_armor.iter_mut() {
                let v = r.u16()?;
                *a = (v != u16::MAX).then_some(ItemId(v));
            }
            for a in vanity_accessories.iter_mut() {
                let v = r.u16()?;
                *a = (v != u16::MAX).then_some(ItemId(v));
            }
            for p in vanity_accessory_prefixes.iter_mut() {
                *p = Prefix(r.u8()?);
            }
        }

        if !r.is_empty() {
            return Err(CharacterError::Decode);
        }

        Ok(Character {
            name,
            difficulty,
            appearance,
            inventory,
            armor,
            accessories,
            accessory_prefixes,
            vanity_armor,
            vanity_accessories,
            vanity_accessory_prefixes,
            life_max,
            life,
            mana_max,
            mana,
            potion_delay,
            facts,
            obsidian_skin_ticks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{COPPER_AXE, COPPER_BROADSWORD, COPPER_PICKAXE, COPPER_SHORTSWORD};

    #[test]
    fn new_character_has_starting_kit_100_life_20_mana() {
        let c = Character::new("Hero", Appearance::default());
        assert_eq!(c.name, "Hero");
        assert_eq!(c.life_max, 100);
        assert_eq!(c.life, 100);
        assert_eq!(c.mana_max, 20);
        assert_eq!(c.mana, 20);
        assert_eq!(c.difficulty, Difficulty::Softcore);

        // Slots 0, 1, 2 hold starting copper tools/sword
        assert_eq!(
            c.inventory.slots[0].as_ref().map(|s| s.id),
            Some(COPPER_SHORTSWORD)
        );
        assert_eq!(
            c.inventory.slots[1].as_ref().map(|s| s.id),
            Some(COPPER_PICKAXE)
        );
        assert_eq!(
            c.inventory.slots[2].as_ref().map(|s| s.id),
            Some(COPPER_AXE)
        );
    }

    #[test]
    fn roundtrip_preserves_everything() {
        let mut c = Character::new(
            "Valiant",
            Appearance {
                hair: HairStyle::Mohawk,
                skin: SKIN_PRESETS[2],
                hair_color: HAIR_COLOR_PRESETS[3],
                eyes: EYE_PRESETS[1],
                shirt: SHIRT_PRESETS[4],
                pants: PANTS_PRESETS[0],
            },
        );
        c.life = 85;
        c.mana = 14;
        c.potion_delay = 120;
        c.armor[0] = Some(COPPER_BROADSWORD);
        c.accessories[1] = Some(COPPER_PICKAXE);

        let bytes = c.to_bytes();
        let loaded = Character::from_bytes(&bytes).expect("decode character");
        assert_eq!(c, loaded);
    }

    #[test]
    fn rejects_bad_magic_and_future_version() {
        let mut c = Character::new("Hero", Appearance::default()).to_bytes();
        c[0] = b'X';
        assert_eq!(Character::from_bytes(&c), Err(CharacterError::BadMagic));

        let mut c2 = Character::new("Hero", Appearance::default()).to_bytes();
        c2[4] = 99; // future version
        assert_eq!(
            Character::from_bytes(&c2),
            Err(CharacterError::UnknownVersion(99))
        );
    }

    #[test]
    fn truncated_bytes_error_not_panic() {
        let c = Character::new("Hero", Appearance::default()).to_bytes();
        for len in 0..c.len() - 1 {
            assert!(matches!(
                Character::from_bytes(&c[..len]),
                Err(CharacterError::Decode) | Err(CharacterError::BadMagic)
            ));
        }
    }

    #[test]
    fn name_validation_matches_worlds() {
        assert_eq!(validate_name("  Bob  "), Ok("Bob"));
        assert_eq!(validate_name(""), Err(NameError::Empty));
        assert_eq!(validate_name("   "), Err(NameError::Empty));
        assert_eq!(
            validate_name("a".repeat(33).as_str()),
            Err(NameError::TooLong)
        );
        assert_eq!(validate_name("Bob/1"), Err(NameError::Illegal('/')));
        assert_eq!(validate_name("Bob?"), Err(NameError::Illegal('?')));
    }

    #[test]
    fn accessory_slots_persist_in_character() {
        let mut c = Character::new("Hero", Appearance::default());
        c.accessories[0] = Some(crate::item::HERMES_BOOTS);
        c.accessories[1] = Some(crate::item::CLOUD_IN_A_BOTTLE);
        c.accessories[2] = Some(crate::item::BAND_OF_REGENERATION);
        c.accessories[3] = Some(crate::item::SHINY_RED_BALLOON);
        c.accessories[4] = Some(crate::item::LUCKY_HORSESHOE);
        let bytes = c.to_bytes();
        let loaded = Character::from_bytes(&bytes).expect("decode");
        assert_eq!(loaded.accessories, c.accessories);
    }

    #[test]
    fn accessories_roundtrip_inert() {
        let mut c = Character::new("Hero", Appearance::default());
        c.accessories[0] = Some(ItemId(42));
        c.accessories[4] = Some(ItemId(99));
        let bytes = c.to_bytes();
        let loaded = Character::from_bytes(&bytes).unwrap();
        assert_eq!(loaded.accessories[0], Some(ItemId(42)));
        assert_eq!(loaded.accessories[4], Some(ItemId(99)));
        assert_eq!(loaded.accessories[2], None);
    }

    #[test]
    fn character_v1_bytes_load_with_default_facts() {
        let mut c = Character::default();
        c.name = "Hero".to_string();
        let mut bytes = c.to_bytes();
        // Force version byte to 1
        bytes[4] = 1;
        // Truncate any v2 trailing fact bytes
        // v1 layout: MAGIC (4) + ver (1) + diff (1) + name_len (1) + name (4) + hair (1) + rgb*5 (15) + vitals*5 (20) + inv_len (4) + inv + armor (6) + accessories (10)
        let inv_len = c.inventory.to_bytes().len();
        let v1_len = 5 + 1 + 1 + 4 + 1 + 15 + 20 + 4 + inv_len + 6 + 10;
        bytes.truncate(v1_len);

        let loaded = Character::from_bytes(&bytes).expect("v1 character must load");
        assert_eq!(loaded.facts, CharacterFacts::default());
        assert!(!loaded.facts.onboarding_dismissed);
    }

    #[test]
    fn character_v4_roundtrip_and_v2_v3_backward_compat() {
        let mut c = Character::default();
        c.name = "Explorer".to_string();
        c.facts.onboarding_dismissed = true;
        c.facts.first_night_survived = true;
        c.facts.has_shelter = true;
        c.facts.has_ore = true;
        c.facts.advice_seen = 0b1011;
        c.obsidian_skin_ticks = 9000;
        c.accessories[0] = Some(crate::item::COBALT_SHIELD);
        c.accessory_prefixes[0] = Prefix(65);
        c.accessories[3] = Some(crate::item::HERMES_BOOTS);
        c.accessory_prefixes[3] = Prefix(76);

        let bytes = c.to_bytes();
        assert_eq!(bytes[4], FORMAT_VERSION);
        let loaded = Character::from_bytes(&bytes).expect("v4 character must load");
        assert_eq!(loaded.facts, c.facts);
        assert_eq!(loaded.obsidian_skin_ticks, 9000);
        assert_eq!(loaded.accessory_prefixes, c.accessory_prefixes);

        let mut v4 = bytes[..bytes.len() - 21].to_vec();
        v4[4] = 4;
        let loaded_v4 = Character::from_bytes(&v4).expect("v4 character must still load");
        assert_eq!(loaded_v4.accessory_prefixes, c.accessory_prefixes);
        assert_eq!(loaded_v4.vanity_armor, [None; 3]);
        assert_eq!(loaded_v4.vanity_accessories, [None; 5]);
        assert_eq!(loaded_v4.vanity_accessory_prefixes, [Prefix(0); 5]);

        let mut v3 = bytes[..bytes.len() - 26].to_vec();
        v3[4] = 3;
        let loaded_v3 = Character::from_bytes(&v3).expect("v3 character must still load");
        assert_eq!(loaded_v3.obsidian_skin_ticks, 9000);
        assert_eq!(loaded_v3.accessory_prefixes, [Prefix(0); 5]);
        assert_eq!(loaded_v3.vanity_armor, [None; 3]);

        let mut v2 = bytes[..bytes.len() - 30].to_vec();
        v2[4] = 2;
        let loaded_v2 = Character::from_bytes(&v2).expect("v2 character must still load");
        assert_eq!(loaded_v2.facts, c.facts);
        assert_eq!(loaded_v2.obsidian_skin_ticks, 0);
        assert_eq!(loaded_v2.accessory_prefixes, [Prefix(0); 5]);
    }

    #[test]
    fn character_v5_vanity_roundtrip() {
        let mut c = Character::new("VanityHero", Appearance::default());
        c.vanity_armor = [
            Some(crate::item::COPPER_HELMET),
            None,
            Some(crate::item::GOLD_GREAVES),
        ];
        c.vanity_accessories[0] = Some(crate::item::HERMES_BOOTS);
        c.vanity_accessory_prefixes[0] = Prefix(65);
        c.vanity_accessories[4] = Some(crate::item::CLOUD_IN_A_BOTTLE);
        c.vanity_accessory_prefixes[4] = Prefix(76);

        let bytes = c.to_bytes();
        let loaded = Character::from_bytes(&bytes).expect("v5 character must decode");
        assert_eq!(loaded.vanity_armor, c.vanity_armor);
        assert_eq!(loaded.vanity_accessories, c.vanity_accessories);
        assert_eq!(
            loaded.vanity_accessory_prefixes,
            c.vanity_accessory_prefixes
        );
    }
}
