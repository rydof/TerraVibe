//! Progression facts tracking world and character milestones.

use crate::codec::{DecodeError, Reader, Writer};

pub const FACTS_MAGIC: &[u8; 4] = b"TVWF";
pub const FACTS_VERSION: u8 = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorldFacts {
    pub merchant_housed: bool,
    pub downed_eye: bool,
    pub downed_skeletron: bool,
    pub shadow_orbs_broken: u32,
    pub dungeon_reached: bool,
    pub underworld_reached: bool,
    pub downed_wall_of_flesh: bool,
    pub downed_eow: bool,
    pub meteorite_crashed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CharacterFacts {
    pub onboarding_dismissed: bool,
    pub advice_seen: u32,
    pub first_night_survived: bool,
    pub has_shelter: bool,
    pub has_ore: bool,
}

pub fn encode_world_facts(facts: &WorldFacts) -> Vec<u8> {
    let mut w = Writer::default();
    w.bytes(FACTS_MAGIC);
    w.u8(FACTS_VERSION);
    w.u8(facts.merchant_housed as u8);
    w.u8(facts.downed_eye as u8);
    w.u8(facts.downed_skeletron as u8);
    w.u32(facts.shadow_orbs_broken);
    w.u8(facts.dungeon_reached as u8);
    w.u8(facts.underworld_reached as u8);
    w.u8(facts.downed_wall_of_flesh as u8);
    w.u8(facts.downed_eow as u8);
    w.u8(facts.meteorite_crashed as u8);
    w.0
}

pub fn decode_world_facts(bytes: &[u8]) -> Result<WorldFacts, DecodeError> {
    if bytes.len() < 5 {
        return Err(DecodeError);
    }
    if &bytes[0..4] != FACTS_MAGIC {
        return Err(DecodeError);
    }
    let version = bytes[4];
    if !(1..=FACTS_VERSION).contains(&version) {
        return Err(DecodeError);
    }
    let mut r = Reader(&bytes[5..]);
    let merchant_housed = r.u8()? != 0;
    let downed_eye = r.u8()? != 0;
    let downed_skeletron = r.u8()? != 0;
    let shadow_orbs_broken = r.u32()?;
    let dungeon_reached = r.u8()? != 0;
    let underworld_reached = r.u8()? != 0;
    let downed_wall_of_flesh = if version >= 2 { r.u8()? != 0 } else { false };
    let downed_eow = if version >= 3 { r.u8()? != 0 } else { false };
    let meteorite_crashed = if version >= 4 { r.u8()? != 0 } else { false };
    if !r.is_empty() {
        return Err(DecodeError);
    }
    Ok(WorldFacts {
        merchant_housed,
        downed_eye,
        downed_skeletron,
        shadow_orbs_broken,
        dungeon_reached,
        underworld_reached,
        downed_wall_of_flesh,
        downed_eow,
        meteorite_crashed,
    })
}

pub fn current_guide_advice(world: &WorldFacts, char_facts: &CharacterFacts) -> &'static str {
    if !char_facts.first_night_survived {
        "You should build a shelter before monsters arrive at night."
    } else if !char_facts.has_shelter {
        "Build a house with walls, a door, a table, a chair, and a torch so someone can move in."
    } else if !world.merchant_housed {
        "Gather 50 silver coins to attract a Merchant to an empty house."
    } else if !char_facts.has_ore {
        "Explore caves underground to find copper, iron, silver, or gold ore for better equipment."
    } else if !world.downed_eye {
        "You can craft a Suspicious Looking Eye at a Demon Altar with lenses from Demon Eyes to summon the Eye of Cthulhu."
    } else if world.shadow_orbs_broken == 0 {
        "Venture into the Corruption chasms and smash Shadow Orbs with a hammer to awaken the Eater of Worlds and forge a Nightmare Pickaxe."
    } else if !world.dungeon_reached {
        "There is an ancient Dungeon at the edge of the world. Seek it out, but beware of its guardian."
    } else if !world.downed_skeletron {
        "An Old Man guards the Dungeon entrance. Talk to him at night to free him from his curse."
    } else if !world.underworld_reached {
        "Dig deeper into the earth until you reach the molten Underworld."
    } else if !world.downed_wall_of_flesh {
        "Mine Hellstone with a Nightmare Pickaxe and smelt Hellstone Bars with Obsidian at a Hellforge for Molten gear, then drop a Guide Voodoo Doll into Underworld lava to summon the Wall of Flesh."
    } else {
        "You have defeated the Wall of Flesh! This pre-Hardmode era is complete — keep exploring or start a new adventure."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_facts_roundtrip() {
        let wf = WorldFacts {
            merchant_housed: true,
            downed_eye: true,
            downed_skeletron: true,
            shadow_orbs_broken: 7,
            dungeon_reached: true,
            underworld_reached: true,
            downed_wall_of_flesh: true,
            downed_eow: true,
            meteorite_crashed: true,
        };
        let bytes = encode_world_facts(&wf);
        assert_eq!(bytes[4], FACTS_VERSION);
        let decoded = decode_world_facts(&bytes).expect("decoding valid world facts");
        assert_eq!(decoded, wf);
    }

    #[test]
    fn world_facts_v2_decodes_with_downed_eow_false() {
        let mut w = Writer::default();
        w.bytes(FACTS_MAGIC);
        w.u8(2);
        w.u8(1);
        w.u8(1);
        w.u8(1);
        w.u32(3);
        w.u8(1);
        w.u8(1);
        w.u8(1);
        let decoded = decode_world_facts(&w.0).expect("v2 world facts must still decode");
        assert!(decoded.downed_wall_of_flesh);
        assert!(!decoded.downed_eow);
    }

    #[test]
    fn world_facts_v1_decodes_with_downed_wof_false() {
        let mut w = Writer::default();
        w.bytes(FACTS_MAGIC);
        w.u8(1);
        w.u8(1);
        w.u8(1);
        w.u8(1);
        w.u32(3);
        w.u8(1);
        w.u8(1);
        let decoded = decode_world_facts(&w.0).expect("v1 world facts must still decode");
        assert!(decoded.merchant_housed);
        assert!(decoded.underworld_reached);
        assert!(!decoded.downed_wall_of_flesh);
    }

    #[test]
    fn world_facts_decode_rejects_bad_magic() {
        let mut bytes = encode_world_facts(&WorldFacts::default());
        bytes[0] = b'X';
        assert!(decode_world_facts(&bytes).is_err());
    }

    #[test]
    fn world_facts_decode_rejects_unknown_version() {
        let mut bytes = encode_world_facts(&WorldFacts::default());
        bytes[4] = 99;
        assert!(decode_world_facts(&bytes).is_err());
    }

    #[test]
    fn guide_advice_priority_chain() {
        let mut wf = WorldFacts::default();
        let mut cf = CharacterFacts::default();

        // 1. First night
        assert_eq!(
            current_guide_advice(&wf, &cf),
            "You should build a shelter before monsters arrive at night."
        );
        cf.first_night_survived = true;

        // 2. Shelter
        assert_eq!(
            current_guide_advice(&wf, &cf),
            "Build a house with walls, a door, a table, a chair, and a torch so someone can move in."
        );
        cf.has_shelter = true;

        // 3. Merchant
        assert_eq!(
            current_guide_advice(&wf, &cf),
            "Gather 50 silver coins to attract a Merchant to an empty house."
        );
        wf.merchant_housed = true;

        // 4. Ore
        assert_eq!(
            current_guide_advice(&wf, &cf),
            "Explore caves underground to find copper, iron, silver, or gold ore for better equipment."
        );
        cf.has_ore = true;

        // 5. Eye of Cthulhu
        assert_eq!(
            current_guide_advice(&wf, &cf),
            "You can craft a Suspicious Looking Eye at a Demon Altar with lenses from Demon Eyes to summon the Eye of Cthulhu."
        );
        wf.downed_eye = true;

        // 6. Corruption / Shadow Orbs / Eater of Worlds / Nightmare Pickaxe
        assert_eq!(
            current_guide_advice(&wf, &cf),
            "Venture into the Corruption chasms and smash Shadow Orbs with a hammer to awaken the Eater of Worlds and forge a Nightmare Pickaxe."
        );
        wf.shadow_orbs_broken = 1;

        // 7. Dungeon
        assert_eq!(
            current_guide_advice(&wf, &cf),
            "There is an ancient Dungeon at the edge of the world. Seek it out, but beware of its guardian."
        );
        wf.dungeon_reached = true;

        // 8. Skeletron
        assert_eq!(
            current_guide_advice(&wf, &cf),
            "An Old Man guards the Dungeon entrance. Talk to him at night to free him from his curse."
        );
        wf.downed_skeletron = true;

        // 9. Underworld
        assert_eq!(
            current_guide_advice(&wf, &cf),
            "Dig deeper into the earth until you reach the molten Underworld."
        );
        wf.underworld_reached = true;

        // 10. Underworld reached -> Hellstone/Hellforge Molten gear + Wall of Flesh summon hint
        assert_eq!(
            current_guide_advice(&wf, &cf),
            "Mine Hellstone with a Nightmare Pickaxe and smelt Hellstone Bars with Obsidian at a Hellforge for Molten gear, then drop a Guide Voodoo Doll into Underworld lava to summon the Wall of Flesh."
        );
        wf.downed_wall_of_flesh = true;

        // 11. Wall of Flesh defeated -> pre-Hardmode complete
        assert_eq!(
            current_guide_advice(&wf, &cf),
            "You have defeated the Wall of Flesh! This pre-Hardmode era is complete — keep exploring or start a new adventure."
        );
    }
}
