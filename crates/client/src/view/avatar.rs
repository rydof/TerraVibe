//! The third-person player avatar: the `models/player` rig, armor from the
//! worn slots and the held item model in the right hand.
//!
//! `TERRAVIBE_DEMO=armor` shows the gold set with a gold broadsword, turned
//! three-quarters toward the camera (screenshots).

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use terravibe_sim::BlockId;
use terravibe_sim::item::{
    ABEEMINATION, AGLET, AMAZON, ANKLET_OF_THE_WIND, BEE_BREASTPLATE, BEE_GREAVES, BEE_GUN,
    BEE_HEADGEAR, BEE_KEEPER, BEE_WAX, BEENADE, BLADE_OF_GRASS, BLUE_PHASEBLADE, BREAKER_BLADE,
    CLOUD_IN_A_BALLOON, COPPER_BAR, COPPER_BROADSWORD, COPPER_CHAINMAIL, COPPER_COIN,
    COPPER_GREAVES, COPPER_HELMET, COPPER_SHORTSWORD, DARK_LANCE, DEMON_BOW, EMPTY_BUCKET,
    FALLEN_STAR, FERAL_CLAWS, FIERY_GREATSWORD, FLAMELASH, FLAMING_ARROW, FLOWER_OF_FIRE,
    GLOWING_MUSHROOM, GOBLIN_BATTLE_STANDARD, GOLD_BAR, GOLD_BROADSWORD, GOLD_CHAINMAIL, GOLD_COIN,
    GOLD_GREAVES, GOLD_HELMET, HARPOON, HEALING_POTION, HEART, HONEY_COMB, HOOK, IRON_BAR,
    IRON_BROADSWORD, IRON_CHAIN, IRON_CHAINMAIL, IRON_GREAVES, IRON_HELMET, IRONSKIN_POTION,
    ItemClass, ItemId, JUNGLE_HAT, JUNGLE_PANTS, JUNGLE_SHIRT, JUNGLE_SPORES, LAVA_BUCKET, LENS,
    LESSER_HEALING_POTION, LESSER_MANA_POTION, LIGHTNING_BOOTS, LIGHTS_BANE, MANA_CRYSTAL,
    MANA_POTION, METEOR_HELMET, METEOR_LEGGINGS, METEOR_SUIT, METEORITE_BAR, MOLTEN_BREASTPLATE,
    MOLTEN_GREAVES, MOLTEN_HAMAXE, MOLTEN_HELMET, MOLTEN_PICKAXE, MONEY_TROUGH, MURAMASA, MUSHROOM,
    NECRO_BREASTPLATE, NECRO_GREAVES, NECRO_HELMET, NIGHTMARE_PICKAXE, NIGHTS_EDGE, NINJA_HOOD,
    NINJA_PANTS, NINJA_SHIRT, OBSIDIAN_SHIELD, PLATINUM_COIN, PWNHAMMER, RECALL_POTION,
    REGENERATION_POTION, ROCKET_BOOTS, SHADOW_GREAVES, SHADOW_HELMET, SHADOW_SCALEMAIL,
    SHARK_TOOTH_NECKLACE, SHINE_POTION, SILVER_BAR, SILVER_BROADSWORD, SILVER_CHAINMAIL,
    SILVER_COIN, SILVER_GREAVES, SILVER_HELMET, SLIME_CROWN, SLIME_HOOK, SLIMY_SADDLE, SPACE_GUN,
    SPEAR, SPECTRE_BOOTS, SPIKY_BALL, STAR_CANNON, STINGER, SUNFURY, SUSPICIOUS_LOOKING_EYE,
    SWIFTNESS_POTION, THORN_CHAKRAM, TORCH, VINE, WAND_OF_SPARKING, WAR_AXE_OF_THE_NIGHT,
    WATER_BOLT, WATER_BUCKET, WATER_WALKING_POTION, WOOD_WALL, WOODEN_SWORD, all_item_ids,
    item_data,
};

use super::camera::{CameraMode, ControlMode, FlyCamera};
use super::held_anim::{HeldAnimKind, HeldAnimTracker, compute_tp_arm_rot, held_anim_kind};
use super::interact::Swing;
use super::player::{EYE_HEIGHT, LocalPlayer, PlayerView};
use terravibe_sim::character::Appearance;

use crate::art::art_dir;
use crate::art::rig::{self, Rig};
use crate::art::rig_view::{self, RigAnim, RigPart, animate_rigs, load_mesh, set_part_model};
use crate::art::vox;
use crate::art::voxmesh::{VOXELS_PER_UNIT, mesh};
use crate::net::{ClientVanity, LocalArmor, LocalInventory};
use crate::session::AppState;

/// Root scale: the player art is 2 * VOXELS_PER_UNIT voxels (2 blocks) tall, the sim's body 1.8.
pub(crate) const AVATAR_SCALE: f32 = 0.9;

#[derive(Resource, Default, Clone)]
pub struct LocalAppearance(pub Option<Appearance>);
/// Held item scale on the avatar, and the hand's rest spot in `arm_r` space
/// (blocks, below the shoulder pivot).
const HELD_SCALE: f32 = 0.72;
const HAND_POS: Vec3 = Vec3::new(0.0, -9.5 / VOXELS_PER_UNIT, 0.06);
/// First-person size of a held model, blocks across its longest side.
const FP_SIZE: f32 = 0.55;
/// Horizontal speed (blocks per tick) above which the walk anim plays.
pub(crate) const WALK_SPEED: f32 = 0.02;
/// Heading turn rate, 1/s.
pub(crate) const TURN_RATE: f32 = 12.0;
/// Armor models are padded by one voxel on every side.
const ARMOR_PAD: f32 = 1.0;
/// Demo heading offset from facing the camera, radians.
const DEMO_TURN: f32 = 0.6;

/// `armor_parts` rows: rig part, file under `models/armor/`.
type Parts = &'static [(&'static str, &'static str)];

/// Rig parts and `models/armor/` files an armor item draws.
pub fn armor_parts(item: ItemId) -> Parts {
    macro_rules! set {
        (head $s:literal) => {
            &[("head", concat!($s, "_head.vox"))]
        };
        (body $s:literal) => {
            &[
                ("torso", concat!($s, "_body.vox")),
                ("arm_l", concat!($s, "_arm_l.vox")),
                ("arm_r", concat!($s, "_arm_r.vox")),
            ]
        };
        (legs $s:literal) => {
            &[
                ("leg_l", concat!($s, "_leg_l.vox")),
                ("leg_r", concat!($s, "_leg_r.vox")),
            ]
        };
    }
    match item {
        COPPER_HELMET => set!(head "copper"),
        COPPER_CHAINMAIL => set!(body "copper"),
        COPPER_GREAVES => set!(legs "copper"),
        IRON_HELMET => set!(head "iron"),
        IRON_CHAINMAIL => set!(body "iron"),
        IRON_GREAVES => set!(legs "iron"),
        SILVER_HELMET => set!(head "silver"),
        SILVER_CHAINMAIL => set!(body "silver"),
        SILVER_GREAVES => set!(legs "silver"),
        GOLD_HELMET => set!(head "gold"),
        GOLD_CHAINMAIL => set!(body "gold"),
        GOLD_GREAVES => set!(legs "gold"),
        SHADOW_HELMET => set!(head "shadow"),
        SHADOW_SCALEMAIL => set!(body "shadow"),
        SHADOW_GREAVES => set!(legs "shadow"),
        MOLTEN_HELMET => set!(head "molten"),
        MOLTEN_BREASTPLATE => set!(body "molten"),
        MOLTEN_GREAVES => set!(legs "molten"),
        METEOR_HELMET | JUNGLE_HAT | BEE_HEADGEAR | NINJA_HOOD => set!(head "shadow"),
        METEOR_SUIT | JUNGLE_SHIRT | BEE_BREASTPLATE | NINJA_SHIRT => set!(body "shadow"),
        METEOR_LEGGINGS | JUNGLE_PANTS | BEE_GREAVES | NINJA_PANTS => set!(legs "shadow"),
        NECRO_HELMET => set!(head "silver"),
        NECRO_BREASTPLATE => set!(body "silver"),
        NECRO_GREAVES => set!(legs "silver"),
        _ => &[],
    }
}

/// 3D held-item representation for an item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HeldVisualKind {
    Vox { file: &'static str, tint: [f32; 4] },
    Block(BlockId),
}

const UNTINTED: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Resolves every item in `all_item_ids()` to a 3D held representation
/// (`.vox` model with optional tint, or 3D block/placed mesh).
pub fn held_visual_for_item(id: ItemId) -> HeldVisualKind {
    let vox = |file: &'static str, tint: [f32; 4]| HeldVisualKind::Vox { file, tint };
    if let Some(file) = explicit_vox_file(id) {
        return vox(file, UNTINTED);
    }
    if let Some(tile) = item_data(id).create_tile {
        return HeldVisualKind::Block(tile);
    }
    match id {
        // Bars -> Demonite Bar model with per-metal tint
        GOLD_BAR => vox("57.vox", [1.0, 0.85, 0.25, 1.0]),
        COPPER_BAR => vox("57.vox", [0.88, 0.48, 0.25, 1.0]),
        SILVER_BAR => vox("57.vox", [0.85, 0.88, 0.92, 1.0]),
        IRON_BAR => vox("57.vox", [0.65, 0.65, 0.68, 1.0]),
        // Coins -> bar/ingot model with coin tint
        COPPER_COIN => vox("57.vox", [0.85, 0.48, 0.24, 1.0]),
        SILVER_COIN => vox("57.vox", [0.84, 0.86, 0.90, 1.0]),
        GOLD_COIN => vox("57.vox", [1.0, 0.84, 0.22, 1.0]),
        PLATINUM_COIN => vox("57.vox", [0.90, 0.95, 1.0, 1.0]),
        // Armor sets -> Shadow Armor models with tier tint
        COPPER_GREAVES => vox("100.vox", [0.88, 0.48, 0.25, 1.0]),
        IRON_GREAVES => vox("100.vox", [0.65, 0.65, 0.68, 1.0]),
        SILVER_GREAVES => vox("100.vox", [0.85, 0.88, 0.92, 1.0]),
        GOLD_GREAVES => vox("100.vox", [1.0, 0.85, 0.25, 1.0]),
        COPPER_CHAINMAIL => vox("101.vox", [0.88, 0.48, 0.25, 1.0]),
        IRON_CHAINMAIL => vox("101.vox", [0.65, 0.65, 0.68, 1.0]),
        SILVER_CHAINMAIL => vox("101.vox", [0.85, 0.88, 0.92, 1.0]),
        GOLD_CHAINMAIL => vox("101.vox", [1.0, 0.85, 0.25, 1.0]),
        COPPER_HELMET => vox("102.vox", [0.88, 0.48, 0.25, 1.0]),
        IRON_HELMET => vox("102.vox", [0.65, 0.65, 0.68, 1.0]),
        SILVER_HELMET => vox("102.vox", [0.85, 0.88, 0.92, 1.0]),
        GOLD_HELMET => vox("102.vox", [1.0, 0.85, 0.25, 1.0]),
        // Potions and buckets
        LESSER_HEALING_POTION => vox("288.vox", [1.0, 0.28, 0.35, 1.0]),
        HEALING_POTION => vox("288.vox", [1.0, 0.12, 0.24, 1.0]),
        LESSER_MANA_POTION => vox("288.vox", [0.35, 0.55, 1.0, 1.0]),
        MANA_POTION => vox("288.vox", [0.18, 0.35, 1.0, 1.0]),
        REGENERATION_POTION => vox("288.vox", [0.95, 0.38, 0.65, 1.0]),
        SWIFTNESS_POTION => vox("288.vox", [0.40, 0.92, 0.62, 1.0]),
        IRONSKIN_POTION => vox("288.vox", [0.95, 0.86, 0.32, 1.0]),
        SHINE_POTION => vox("288.vox", [1.0, 0.96, 0.55, 1.0]),
        WATER_WALKING_POTION => vox("288.vox", [0.22, 0.72, 0.95, 1.0]),
        RECALL_POTION => vox("288.vox", [0.35, 0.85, 0.92, 1.0]),
        GLOWING_MUSHROOM => vox("23.vox", [0.28, 0.62, 1.0, 1.0]),
        IRON_CHAIN | HOOK => vox("57.vox", [0.68, 0.72, 0.78, 1.0]),
        EMPTY_BUCKET => vox("53.vox", [0.65, 0.68, 0.72, 1.0]),
        WATER_BUCKET => vox("53.vox", [0.25, 0.55, 0.95, 1.0]),
        LAVA_BUCKET => vox("53.vox", [1.0, 0.42, 0.12, 1.0]),
        // Ranged, ammo, magic
        FLAMING_ARROW => vox("40.vox", [1.0, 0.55, 0.15, 1.0]),
        DEMON_BOW => vox("39.vox", [0.55, 0.35, 0.85, 1.0]),
        WAND_OF_SPARKING => vox("157.vox", [0.95, 0.65, 0.30, 1.0]),
        // Walls, pickups, orbs, crystals
        WOOD_WALL => vox("9.vox", [0.72, 0.56, 0.42, 1.0]),
        MUSHROOM => vox("23.vox", [0.92, 0.72, 0.55, 1.0]),
        LENS => vox("115.vox", [0.85, 0.92, 1.0, 1.0]),
        SUSPICIOUS_LOOKING_EYE => vox("115.vox", [0.95, 0.35, 0.35, 1.0]),
        HEART => vox("23.vox", [1.0, 0.20, 0.30, 1.0]),
        FALLEN_STAR => vox("115.vox", [1.0, 0.95, 0.35, 1.0]),
        MANA_CRYSTAL => vox("115.vox", [0.25, 0.55, 1.0, 1.0]),
        // Wave 2: Jungle, Night's Edge, Meteorite, Shadow Chest, Class Armor, Accessories
        JUNGLE_SPORES => vox("23.vox", [0.45, 0.95, 0.35, 1.0]),
        STINGER => vox("40.vox", [0.85, 0.78, 0.25, 1.0]),
        VINE => vox("9.vox", [0.22, 0.65, 0.25, 1.0]),
        BLADE_OF_GRASS => vox("121.vox", [0.30, 0.88, 0.28, 1.0]),
        THORN_CHAKRAM => vox("119.vox", [0.35, 0.85, 0.30, 1.0]),
        AMAZON => vox("3317.vox", [0.28, 0.82, 0.35, 1.0]),
        NIGHTS_EDGE => vox("121.vox", [0.58, 0.20, 0.85, 1.0]),
        METEORITE_BAR => vox("57.vox", [0.82, 0.35, 0.22, 1.0]),
        METEOR_HELMET | BEE_HEADGEAR => vox("102.vox", [0.75, 0.38, 0.25, 1.0]),
        METEOR_SUIT | BEE_BREASTPLATE => vox("101.vox", [0.75, 0.38, 0.25, 1.0]),
        METEOR_LEGGINGS | BEE_GREAVES => vox("100.vox", [0.75, 0.38, 0.25, 1.0]),
        NECRO_HELMET => vox("102.vox", [0.85, 0.82, 0.78, 1.0]),
        NECRO_BREASTPLATE => vox("101.vox", [0.85, 0.82, 0.78, 1.0]),
        NECRO_GREAVES => vox("100.vox", [0.85, 0.82, 0.78, 1.0]),
        JUNGLE_HAT => vox("102.vox", [0.25, 0.72, 0.32, 1.0]),
        JUNGLE_SHIRT => vox("101.vox", [0.25, 0.72, 0.32, 1.0]),
        JUNGLE_PANTS => vox("100.vox", [0.25, 0.72, 0.32, 1.0]),
        SPACE_GUN => vox("514.vox", [0.45, 0.95, 0.35, 1.0]),
        BLUE_PHASEBLADE => vox("155.vox", [0.25, 0.75, 1.0, 1.0]),
        STAR_CANNON => vox("434.vox", [0.35, 0.55, 0.95, 1.0]),
        WATER_BOLT => vox("157.vox", [0.20, 0.55, 1.0, 1.0]),
        SUNFURY => vox("163.vox", [1.0, 0.45, 0.12, 1.0]),
        DARK_LANCE => vox("274.vox", [0.48, 0.18, 0.65, 1.0]),
        FLAMELASH => vox("113.vox", [1.0, 0.42, 0.15, 1.0]),
        FLOWER_OF_FIRE => vox("157.vox", [1.0, 0.35, 0.12, 1.0]),
        ROCKET_BOOTS => vox("54.vox", [0.85, 0.35, 0.25, 1.0]),
        SPECTRE_BOOTS => vox("54.vox", [0.45, 0.85, 1.0, 1.0]),
        LIGHTNING_BOOTS => vox("54.vox", [0.35, 0.95, 0.45, 1.0]),
        OBSIDIAN_SHIELD => vox("156.vox", [0.38, 0.28, 0.55, 1.0]),
        CLOUD_IN_A_BALLOON => vox("159.vox", [0.75, 0.92, 1.0, 1.0]),
        AGLET | ANKLET_OF_THE_WIND => vox("49.vox", [0.45, 0.90, 0.45, 1.0]),
        FERAL_CLAWS => vox("156.vox", [0.55, 0.38, 0.22, 1.0]),
        SHARK_TOOTH_NECKLACE => vox("49.vox", [0.92, 0.90, 0.85, 1.0]),
        // Wave 4: King Slime & Queen Bee
        SLIME_CROWN => vox("102.vox", [0.30, 0.72, 1.0, 1.0]),
        NINJA_HOOD => vox("102.vox", [0.22, 0.22, 0.30, 1.0]),
        NINJA_SHIRT => vox("101.vox", [0.22, 0.22, 0.30, 1.0]),
        NINJA_PANTS => vox("100.vox", [0.22, 0.22, 0.30, 1.0]),
        SLIME_HOOK => vox("84.vox", [0.35, 0.78, 1.0, 1.0]),
        SLIMY_SADDLE => vox("23.vox", [0.30, 0.75, 1.0, 1.0]),
        ABEEMINATION => vox("23.vox", [0.96, 0.78, 0.18, 1.0]),
        BEE_WAX => vox("23.vox", [0.95, 0.75, 0.15, 1.0]),
        BEENADE => vox("168.vox", [0.98, 0.82, 0.15, 1.0]),
        BEE_GUN => vox("514.vox", [0.96, 0.76, 0.12, 1.0]),
        BEE_KEEPER => vox("121.vox", [0.95, 0.75, 0.15, 1.0]),
        HONEY_COMB => vox("49.vox", [0.98, 0.74, 0.12, 1.0]),
        // Wave 5: Events
        GOBLIN_BATTLE_STANDARD => vox("39.vox", [0.45, 0.72, 0.35, 1.0]),
        HARPOON => vox("274.vox", [0.72, 0.76, 0.82, 1.0]),
        SPIKY_BALL => vox("168.vox", [0.65, 0.68, 0.72, 1.0]),
        MONEY_TROUGH => vox("23.vox", [0.88, 0.52, 0.65, 1.0]),
        _ => vox("23.vox", UNTINTED),
    }
}

fn explicit_vox_file(id: ItemId) -> Option<&'static str> {
    Some(match id.0 {
        1 => "1.vox",
        4 => "4.vox",
        8 => "8.vox",
        9 => "9.vox",
        10 => "10.vox",
        23 => "23.vox",
        24 => "24.vox",
        39 => "39.vox",
        40 => "40.vox",
        45 => "45.vox",
        46 => "46.vox",
        49 => "49.vox",
        50 => "50.vox",
        53 => "53.vox",
        54 => "54.vox",
        55 => "55.vox",
        56 => "56.vox",
        57 => "57.vox",
        64 => "64.vox",
        65 => "65.vox",
        70 => "70.vox",
        71 => "71.vox",
        72 => "72.vox",
        73 => "73.vox",
        74 => "74.vox",
        84 => "84.vox",
        86 => "86.vox",
        95 => "95.vox",
        96 => "96.vox",
        97 => "97.vox",
        98 => "98.vox",
        100 => "100.vox",
        101 => "101.vox",
        102 => "102.vox",
        103 => "103.vox",
        104 => "104.vox",
        111 => "111.vox",
        113 => "113.vox",
        115 => "115.vox",
        119 => "119.vox",
        120 => "120.vox",
        121 => "121.vox",
        122 => "122.vox",
        127 => "127.vox",
        154 => "154.vox",
        155 => "155.vox",
        156 => "156.vox",
        157 => "157.vox",
        158 => "158.vox",
        159 => "159.vox",
        162 => "162.vox",
        163 => "163.vox",
        164 => "164.vox",
        165 => "165.vox",
        166 => "166.vox",
        167 => "167.vox",
        168 => "168.vox",
        172 => "172.vox",
        173 => "173.vox",
        174 => "174.vox",
        175 => "175.vox",
        188 => "188.vox",
        189 => "189.vox",
        192 => "192.vox",
        193 => "193.vox",
        196 => "196.vox",
        214 => "214.vox",
        217 => "217.vox",
        219 => "219.vox",
        221 => "221.vox",
        231 => "231.vox",
        232 => "232.vox",
        233 => "233.vox",
        267 => "267.vox",
        273 => "273.vox",
        274 => "274.vox",
        288 => "288.vox",
        327 => "327.vox",
        329 => "329.vox",
        361 => "361.vox",
        367 => "367.vox",
        426 => "426.vox",
        434 => "434.vox",
        489 => "489.vox",
        490 => "490.vox",
        491 => "491.vox",
        514 => "514.vox",
        560 => "560.vox",
        906 => "906.vox",
        1133 => "1133.vox",
        1991 => "1991.vox",
        2007 => "2007.vox",
        2015 => "2015.vox",
        2018 => "2018.vox",
        2019 => "2019.vox",
        3317 => "3317.vox",
        4912 => "4912.vox",
        3506 => "3506.vox",
        3507 => "3507.vox",
        3508 => "3508.vox",
        3509 => "3509.vox",
        3512 => "3512.vox",
        3514 => "3514.vox",
        3515 => "3515.vox",
        3518 => "3518.vox",
        3520 => "3520.vox",
        3521 => "3521.vox",
        _ => return None,
    })
}

/// File under `models/items/` drawing an item with its own authored `.vox` model.
pub fn item_model_file(id: ItemId) -> Option<String> {
    explicit_vox_file(id).map(str::to_owned)
}

/// Anim for the local state: swinging beats airborne beats walking.
pub fn pick_anim(vel: Vec2, grounded: bool, using: bool) -> &'static str {
    if using {
        "swing"
    } else if !grounded {
        "jump"
    } else if vel.length() > WALK_SPEED {
        "walk"
    } else {
        "idle"
    }
}

/// Visual scale multiplier for held items in first- and third-person views.
pub fn item_visual_scale(id: ItemId) -> f32 {
    match id {
        COPPER_SHORTSWORD => 0.72,
        WOODEN_SWORD | COPPER_BROADSWORD | IRON_BROADSWORD => 1.05,
        SILVER_BROADSWORD | GOLD_BROADSWORD => 1.15,
        LIGHTS_BANE | WAR_AXE_OF_THE_NIGHT | NIGHTMARE_PICKAXE => 1.28,
        MURAMASA => 1.35,
        FIERY_GREATSWORD => 1.65,
        BREAKER_BLADE => 1.85,
        MOLTEN_PICKAXE | MOLTEN_HAMAXE | PWNHAMMER => 1.25,
        SPEAR => 1.55,
        _ => item_data(id).scale.clamp(0.85, 1.4),
    }
}

/// Meshes of a held item model: `tp` pivots at the grip for the avatar's hand,
/// `fp` at the centre for the first-person view.
pub(crate) struct ItemModel {
    pub(crate) tp: Handle<Mesh>,
    pub(crate) fp: Handle<Mesh>,
    pub(crate) fp_scale: f32,
    pub(crate) tp_scale: f32,
    pub(crate) material: Handle<StandardMaterial>,
    /// Torches and bows stand upright in the fist; the rest extend the arm.
    upright: bool,
}

#[derive(Resource)]
pub(crate) struct ItemModels {
    pub(crate) map: HashMap<u16, ItemModel>,
}

/// First-person scale that makes an `sx` x `sy` voxel item `FP_SIZE` blocks across.
fn fp_scale(sx: f32, sy: f32) -> f32 {
    FP_SIZE / (sx.max(sy) / VOXELS_PER_UNIT)
}

fn build_block_cube_mesh(art: &crate::art::blocks::BlockArt, tile: BlockId) -> Option<Mesh> {
    use crate::art::chunk_mesh::face_lighting_weight;
    use crate::art::mesh_data::{Face, MeshData};
    let mut out = MeshData::default();
    const CORNERS: [(u32, u32); 4] = [(0, 0), (0, 1), (1, 1), (1, 0)];
    let along = |d: i32, t: u32| match d {
        1 => t as f32,
        -1 => 1.0 - t as f32,
        _ => 0.0,
    };
    for face in Face::ALL {
        let uv = art
            .uv
            .get(&(tile.0, face, 0))
            .copied()
            .unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let (n, u, v) = (face.normal(), face.u_axis(), face.v_axis());
        let pos = CORNERS.map(|(c, r)| {
            std::array::from_fn(|a| f32::from(n[a] > 0) + along(u[a], c) + along(v[a], r) - 0.5)
        });
        let uvs = CORNERS.map(|(c, r)| {
            [
                uv[0] + c as f32 * (uv[2] - uv[0]),
                uv[1] + r as f32 * (uv[3] - uv[1]),
            ]
        });
        let w = face_lighting_weight(face);
        out.push_quad_colored(
            pos,
            [n[0] as f32, n[1] as f32, n[2] as f32],
            uvs,
            [[w, w, w, 1.0]; 4],
        );
    }
    super::to_bevy_mesh(out)
}

fn setup_item_models(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    art_res: Option<Res<super::chunks::ArtRes>>,
) {
    let base_material = materials.add(StandardMaterial {
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });
    let art = art_res
        .map(|a| a.0.clone())
        .unwrap_or_else(|| Arc::new(crate::art::blocks::BlockArt::load(&art_dir())));
    let atlas_tex = images.add(super::to_bevy_image(&art.atlas));
    let block_opaque_mat = materials.add(StandardMaterial {
        base_color_texture: Some(atlas_tex.clone()),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });
    let block_glass_mat = materials.add(StandardMaterial {
        base_color_texture: Some(atlas_tex),
        alpha_mode: AlphaMode::Mask(0.5),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });

    let mut tinted_mats: HashMap<[u32; 4], Handle<StandardMaterial>> = HashMap::new();
    let mut map = HashMap::new();
    for id in all_item_ids() {
        match held_visual_for_item(id) {
            HeldVisualKind::Vox { file, tint } => {
                let path = art_dir().join("models/items").join(file);
                let m = match vox::try_load(&path) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!("{e}");
                        continue;
                    }
                };
                // File size is (x, depth, y); the sprite plane is game x/y.
                let (sx, sy, sz) = (m.size[0] as f32, m.size[2] as f32, m.size[1] as f32);
                let upright = id == TORCH || item_data(id).class == ItemClass::Ranged;
                let grip = match (upright, item_data(id).class == ItemClass::Ranged) {
                    (true, true) => [sx * 0.75, sy / 2.0, sz / 2.0],
                    (true, false) => [sx / 2.0, 2.0, sz / 2.0],
                    _ => [2.0, 2.0, sz / 2.0],
                };
                let make = |meshes: &mut Assets<Mesh>, pivot| {
                    super::to_bevy_mesh(mesh(&m, pivot)).map(|m| meshes.add(m))
                };
                let (Some(tp), Some(fp)) = (make(&mut meshes, grip), make(&mut meshes, grip))
                else {
                    continue;
                };
                let material = if tint == UNTINTED {
                    base_material.clone()
                } else {
                    let key = tint.map(f32::to_bits);
                    tinted_mats
                        .entry(key)
                        .or_insert_with(|| {
                            materials.add(StandardMaterial {
                                base_color: Color::srgba(tint[0], tint[1], tint[2], tint[3]),
                                perceptual_roughness: 1.0,
                                reflectance: 0.0,
                                ..default()
                            })
                        })
                        .clone()
                };
                let vis_scale = item_visual_scale(id);
                map.insert(
                    id.0,
                    ItemModel {
                        tp,
                        fp,
                        fp_scale: fp_scale(sx, sy) * vis_scale.clamp(0.75, 1.45),
                        tp_scale: HELD_SCALE * vis_scale,
                        material,
                        upright,
                    },
                );
            }
            HeldVisualKind::Block(tile) => {
                if crate::art::is_model_block(tile) {
                    let Some(file) = super::block_models::model_file(tile) else {
                        continue;
                    };
                    let path = art_dir().join("models/blocks").join(file);
                    let m = match vox::try_load(&path) {
                        Ok(m) => m,
                        Err(e) => {
                            warn!("{e}");
                            continue;
                        }
                    };
                    let (sx, sy, sz) = (m.size[0] as f32, m.size[2] as f32, m.size[1] as f32);
                    let make = |meshes: &mut Assets<Mesh>, pivot| {
                        super::to_bevy_mesh(mesh(&m, pivot)).map(|m| meshes.add(m))
                    };
                    let (Some(tp), Some(fp)) = (
                        make(&mut meshes, [sx / 2.0, 2.0, sz / 2.0]),
                        make(&mut meshes, [sx / 2.0, sy / 2.0, sz / 2.0]),
                    ) else {
                        continue;
                    };
                    map.insert(
                        id.0,
                        ItemModel {
                            tp,
                            fp,
                            fp_scale: fp_scale(sx, sy) * item_visual_scale(id).clamp(0.75, 1.45),
                            tp_scale: HELD_SCALE * item_visual_scale(id),
                            material: base_material.clone(),
                            upright: false,
                        },
                    );
                } else {
                    let Some(cube) = build_block_cube_mesh(&art, tile) else {
                        continue;
                    };
                    let h = meshes.add(cube);
                    let material = if tile == terravibe_sim::GLASS_BLOCK {
                        block_glass_mat.clone()
                    } else {
                        block_opaque_mat.clone()
                    };
                    map.insert(
                        id.0,
                        ItemModel {
                            tp: h.clone(),
                            fp: h,
                            fp_scale: FP_SIZE,
                            tp_scale: HELD_SCALE,
                            material,
                            upright: false,
                        },
                    );
                }
            }
        }
    }
    commands.insert_resource(ItemModels { map });
}

#[derive(Resource)]
pub(crate) struct Avatar {
    root: Entity,
    rig: Rig,
    heading: f32,
    armor: [Option<ItemId>; 3],
    applied_appearance: Option<Appearance>,
    held: Option<(ItemId, Entity)>,
    cache: HashMap<String, Handle<Mesh>>,
    material: Handle<StandardMaterial>,
}

/// `TERRAVIBE_DEMO=armor`.
#[derive(Resource)]
struct DemoArmor;

/// Spawned 3D player rig state shared by local and remote avatars.
pub(crate) struct SpawnedPlayerRig {
    pub(crate) root: Entity,
    pub(crate) rig: Rig,
    pub(crate) material: Handle<StandardMaterial>,
    pub(crate) cache: HashMap<String, Handle<Mesh>>,
}

/// Spawns a 3D player rig entity tree scaled to [`AVATAR_SCALE`] and pre-warms
/// any custom [`Appearance`] part meshes in the returned cache.
pub(crate) fn spawn_player_rig_parts(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    appearance: Option<&Appearance>,
) -> SpawnedPlayerRig {
    let dir = art_dir().join("models/player");
    let rig = rig::load(&dir);
    let mut rig_cache = rig_view::MeshCache::default();
    let (root, material) =
        rig_view::spawn_rig_with(commands, meshes, materials, &rig, &dir, &mut rig_cache);
    commands.entity(root).insert((
        Transform::from_scale(Vec3::splat(AVATAR_SCALE)),
        DespawnOnExit(AppState::InGame),
    ));
    let mut cache = HashMap::new();
    if let Some(app) = appearance {
        for p in &rig.parts {
            let _ = mesh_for_rig_part(&rig, &mut cache, meshes, &p.name, None, Some(app));
        }
    }
    SpawnedPlayerRig {
        root,
        rig,
        material,
        cache,
    }
}

/// Resolves effective displayed armor slots, preferring vanity over worn armor.
pub(crate) fn resolve_effective_armor(
    armor: [Option<ItemId>; 3],
    vanity_armor: [Option<ItemId>; 3],
) -> [Option<ItemId>; 3] {
    [
        vanity_armor[0].or(armor[0]),
        vanity_armor[1].or(armor[1]),
        vanity_armor[2].or(armor[2]),
    ]
}

fn spawn_avatar(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cam: Single<&FlyCamera>,
    app: Option<Res<LocalAppearance>>,
) {
    let spawned = spawn_player_rig_parts(
        &mut commands,
        &mut meshes,
        &mut materials,
        app.as_ref().and_then(|a| a.0.as_ref()),
    );
    let av = Avatar {
        root: spawned.root,
        rig: spawned.rig,
        heading: cam.yaw + std::f32::consts::PI,
        armor: [None; 3],
        applied_appearance: None,
        held: None,
        cache: spawned.cache,
        material: spawned.material,
    };
    commands.insert_resource(av);
}

pub(crate) fn wrap_pi(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// Returns `true` on alternating frames while the player has active i-frames.
pub(crate) fn iframe_blink_hidden(i_frames: i32) -> bool {
    i_frames > 0 && (i_frames / 4) % 2 == 1
}

/// Positions, turns, animates and shows or hides the avatar.
#[allow(clippy::too_many_arguments)]
fn drive_avatar(
    mut av: ResMut<Avatar>,
    player: Res<LocalPlayer>,
    view: Res<PlayerView>,
    fixed: Res<Time<Fixed>>,
    time: Res<Time<Real>>,
    swing: Res<Swing>,
    cam: Single<&FlyCamera>,
    cmode: Res<CameraMode>,
    demo: Option<Res<DemoArmor>>,
    mut last_swing: Local<f32>,
    mut root: Query<(&mut Transform, &mut RigAnim, &mut Visibility)>,
) {
    let Ok((mut tf, mut anim, mut vis)) = root.get_mut(av.root) else {
        return;
    };
    let body = &player.0.body;
    let vel = Vec2::new(body.vel[0], body.vel[2]);
    let using = demo.is_none() && time.elapsed_secs() - swing.start < swing.secs;
    let facing_cam = cam.yaw + std::f32::consts::PI;
    // Face the swing (camera) direction while using, else the movement direction.
    let target = if demo.is_some() {
        cam.yaw + DEMO_TURN
    } else if using {
        facing_cam
    } else if vel.length() > WALK_SPEED {
        vel.x.atan2(vel.y)
    } else {
        av.heading
    };
    let k = 1.0 - (-TURN_RATE * time.delta_secs()).exp();
    av.heading = wrap_pi(av.heading + wrap_pi(target - av.heading) * k);
    let feet = view.prev_eye.lerp(view.eye, fixed.overstep_fraction()) + view.offset
        - Vec3::Y * EYE_HEIGHT;
    tf.translation = feet;
    tf.rotation = Quat::from_rotation_y(av.heading);
    anim.play(pick_anim(vel, body.on_ground, using));
    if using && (swing.start - *last_swing).abs() > 1e-4 {
        *last_swing = swing.start;
        anim.t = 0.0;
    }
    *vis = if *cmode == CameraMode::First || player.0.dead || iframe_blink_hidden(player.0.i_frames)
    {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
}

pub(crate) fn mesh_for_rig_part(
    rig: &Rig,
    cache: &mut HashMap<String, Handle<Mesh>>,
    meshes: &mut Assets<Mesh>,
    part: &str,
    armor_file: Option<&str>,
    app: Option<&Appearance>,
) -> Option<Handle<Mesh>> {
    let p = rig.parts.iter().find(|p| p.name == part)?;
    if let Some(f) = armor_file {
        let key = format!("armor/{f}");
        if let Some(h) = cache.get(&key) {
            return Some(h.clone());
        }
        let path = art_dir().join("models/armor").join(f);
        let h = load_mesh(meshes, &path, p.pivot.map(|v| v + ARMOR_PAD))?;
        cache.insert(key, h.clone());
        return Some(h);
    }
    if let Some(app) = app {
        let key = format!("char/{}/{part}", app.hair.to_u8());
        if let Some(h) = cache.get(&key) {
            return Some(h.clone());
        }
        let dir = art_dir().join("models/player");
        if let Some(data) = super::player_art::build_player_part_mesh(part, app, &dir) {
            if let Some(m) = crate::view::to_bevy_mesh(data) {
                let h = meshes.add(m);
                cache.insert(key, h.clone());
                return Some(h);
            }
        }
    }
    let key = format!("player/{}", p.file);
    if let Some(h) = cache.get(&key) {
        return Some(h.clone());
    }
    let path = art_dir().join("models/player").join(&p.file);
    let h = load_mesh(meshes, &path, p.pivot)?;
    cache.insert(key, h.clone());
    Some(h)
}

/// Applies effective armor slots and base [`Appearance`] meshes to every
/// [`RigPart`] belonging to `root`. Returns `true` if at least one part was
/// found for `root`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_armor_to_rig(
    commands: &mut Commands,
    root: Entity,
    rig: &Rig,
    cache: &mut HashMap<String, Handle<Mesh>>,
    meshes: &mut Assets<Mesh>,
    effective_armor: [Option<ItemId>; 3],
    app: Option<&Appearance>,
    parts: &Query<(Entity, &RigPart)>,
) -> bool {
    let worn: Vec<(&str, &str)> = effective_armor
        .iter()
        .flatten()
        .flat_map(|id| armor_parts(*id).iter().copied())
        .collect();
    let mut found_any = false;
    for (e, part) in parts {
        if part.root != root {
            continue;
        }
        found_any = true;
        let file = worn.iter().find(|(p, _)| *p == part.name).map(|(_, f)| *f);
        if let Some(h) = mesh_for_rig_part(rig, cache, meshes, &part.name, file, app) {
            set_part_model(commands, e, h);
        }
    }
    found_any
}

/// Swaps part models when the worn armor changes.
fn sync_armor(
    mut commands: Commands,
    mut av: ResMut<Avatar>,
    mut meshes: ResMut<Assets<Mesh>>,
    armor: Option<Res<LocalArmor>>,
    vanity: Option<Res<ClientVanity>>,
    demo: Option<Res<DemoArmor>>,
    app: Option<Res<LocalAppearance>>,
    parts: Query<(Entity, &RigPart)>,
) {
    let want = if demo.is_some() {
        [Some(GOLD_HELMET), Some(GOLD_CHAINMAIL), Some(GOLD_GREAVES)]
    } else {
        let arm = armor.map_or([None; 3], |a| a.0);
        let van = vanity.map_or([None; 3], |v| v.armor);
        resolve_effective_armor(arm, van)
    };
    let cur_app = app.as_ref().and_then(|a| a.0.clone());
    if want == av.armor && cur_app == av.applied_appearance {
        return;
    }
    av.armor = want;
    av.applied_appearance = cur_app.clone();
    let root = av.root;
    let Avatar {
        ref rig,
        ref mut cache,
        ..
    } = *av;
    apply_armor_to_rig(
        &mut commands,
        root,
        rig,
        cache,
        &mut meshes,
        want,
        cur_app.as_ref(),
        &parts,
    );
}

/// Third-person hand-mount rotation for held items in `arm_r` local space.
///
/// Upright items (torches, bows) stand along `+Z` (world `+Y` when the arm is
/// raised forward). Diagonal tools and swords extend outward along `-Y` (world
/// `+Z` forward when the arm is raised) and `+Z` (upward/forward from the fist).
pub(crate) fn tp_held_rotation(upright: bool) -> Quat {
    if upright {
        Quat::from_mat3(&Mat3::from_cols(Vec3::Y, Vec3::Z, Vec3::X))
    } else {
        Quat::from_rotation_x(-0.95)
            * Quat::from_mat3(&Mat3::from_cols(Vec3::Z, Vec3::NEG_Y, Vec3::X))
    }
}

/// Attaches, swaps, or despawns the held item mesh on `root`'s `arm_r` part.
/// Returns `true` when synced (or cleared / no model exists), or `false` if
/// `ItemModels` or `arm_r` is not yet ready and the caller should retry.
pub(crate) fn update_held_item_on_rig(
    commands: &mut Commands,
    root: Entity,
    want: Option<ItemId>,
    current: &mut Option<(ItemId, Entity)>,
    models: Option<&ItemModels>,
    parts: &Query<(Entity, &RigPart)>,
) -> bool {
    if want == current.map(|(id, _)| id) {
        return true;
    }
    if let Some((_, e)) = current.take() {
        commands.entity(e).despawn();
    }
    let Some(id) = want else {
        return true;
    };
    let Some(models) = models else {
        return false;
    };
    let Some(m) = models.map.get(&id.0) else {
        return true;
    };
    let Some((arm, _)) = parts
        .iter()
        .find(|(_, p)| p.root == root && p.name == "arm_r")
    else {
        return false;
    };
    let rot = tp_held_rotation(m.upright);
    let e = commands
        .spawn((
            Name::new("held"),
            Mesh3d(m.tp.clone()),
            MeshMaterial3d(m.material.clone()),
            Transform {
                translation: HAND_POS,
                rotation: rot,
                scale: Vec3::splat(m.tp_scale),
            },
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    commands.entity(arm).add_child(e);
    *current = Some((id, e));
    true
}

/// Keeps the held item model attached to `arm_r`'s hand.
fn sync_held(
    mut commands: Commands,
    mut av: ResMut<Avatar>,
    inv: Option<Res<LocalInventory>>,
    demo: Option<Res<DemoArmor>>,
    models: Option<Res<ItemModels>>,
    parts: Query<(Entity, &RigPart)>,
) {
    let want = if demo.is_some() {
        Some(GOLD_BROADSWORD)
    } else {
        inv.and_then(|i| i.0.selected_stack().map(|s| s.id))
    };
    let root = av.root;
    update_held_item_on_rig(
        &mut commands,
        root,
        want,
        &mut av.held,
        models.as_deref(),
        &parts,
    );
}

/// Applies per-class third-person `arm_r` rotation over the base rig pose
/// when using an item, holding a torch, or during equip/recoil/kick transitions.
fn animate_held_arm(
    av: Res<Avatar>,
    inv: Option<Res<LocalInventory>>,
    demo: Option<Res<DemoArmor>>,
    tracker: Option<Res<HeldAnimTracker>>,
    mut parts: Query<(&RigPart, &mut Transform)>,
) {
    if demo.is_some() {
        return;
    }
    let Some(tracker) = tracker else { return };
    let Some(held_id) = inv.and_then(|i| i.0.selected_stack().map(|s| s.id)) else {
        return;
    };
    let kind = held_anim_kind(held_id);
    let st = &tracker.state;
    let active = st.use_t.is_some()
        || kind == HeldAnimKind::TorchHold
        || st.equip_t < 1.0
        || st.recoil_t > 0.0
        || st.kick_t > 0.0;
    if !active {
        return;
    }
    let rot = compute_tp_arm_rot(kind, st);
    for (part, mut tf) in &mut parts {
        if part.root == av.root && part.name == "arm_r" {
            tf.rotation = rot;
            break;
        }
    }
}

fn is_metallic_armor(id: ItemId) -> bool {
    matches!(
        id,
        COPPER_HELMET
            | COPPER_CHAINMAIL
            | COPPER_GREAVES
            | IRON_HELMET
            | IRON_CHAINMAIL
            | IRON_GREAVES
            | SILVER_HELMET
            | SILVER_CHAINMAIL
            | SILVER_GREAVES
            | GOLD_HELMET
            | GOLD_CHAINMAIL
            | GOLD_GREAVES
    )
}

fn is_silver_or_gold_armor(id: ItemId) -> bool {
    matches!(
        id,
        SILVER_HELMET
            | SILVER_CHAINMAIL
            | SILVER_GREAVES
            | GOLD_HELMET
            | GOLD_CHAINMAIL
            | GOLD_GREAVES
    )
}

fn is_shadow_armor(id: ItemId) -> bool {
    matches!(id, SHADOW_HELMET | SHADOW_SCALEMAIL | SHADOW_GREAVES)
}

fn is_molten_armor(id: ItemId) -> bool {
    matches!(id, MOLTEN_HELMET | MOLTEN_BREASTPLATE | MOLTEN_GREAVES)
}

fn drive_armor_effects(
    time: Res<Time<Real>>,
    av: Res<Avatar>,
    player: Res<LocalPlayer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut pool: Option<ResMut<super::particles::ParticlePool>>,
    settings: Option<Res<crate::settings::Settings>>,
    mut accum: Local<f32>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();
    let s = settings.as_deref().copied().unwrap_or_default();

    let has_metal = av.armor.iter().flatten().any(|&id| is_metallic_armor(id));
    let has_silver_gold = av
        .armor
        .iter()
        .flatten()
        .any(|&id| is_silver_or_gold_armor(id));
    let has_shadow = av.armor.iter().flatten().any(|&id| is_shadow_armor(id));
    let has_molten = av.armor.iter().flatten().any(|&id| is_molten_armor(id));

    if let Some(mut mat) = materials.get_mut(&av.material) {
        if has_molten {
            let pulse = 0.25 + 0.18 * (t * 5.0).sin().abs();
            mat.metallic = 0.25;
            mat.perceptual_roughness = 0.65;
            mat.reflectance = 0.3;
            mat.emissive = LinearRgba::new(pulse * 1.4, pulse * 0.45, 0.02, 1.0);
        } else if has_shadow {
            let pulse = 0.18 + 0.16 * (t * 4.5).sin().abs();
            mat.metallic = 0.2;
            mat.perceptual_roughness = 0.7;
            mat.reflectance = 0.25;
            mat.emissive = LinearRgba::new(pulse * 0.65, 0.05, pulse * 1.25, 1.0);
        } else if has_metal {
            mat.metallic = 0.72;
            mat.perceptual_roughness = 0.35;
            mat.reflectance = 0.65;
            mat.emissive = LinearRgba::BLACK;
        } else {
            mat.metallic = 0.0;
            mat.perceptual_roughness = 1.0;
            mat.reflectance = 0.0;
            mat.emissive = LinearRgba::BLACK;
        }
    }

    if s.reduced_motion {
        return;
    }
    let Some(ref mut p) = pool else { return };
    *accum += dt;
    if *accum < 0.12 {
        return;
    }
    *accum = 0.0;

    let pos = Vec3::from(player.0.body.pos);
    let vel = Vec2::new(player.0.body.vel[0], player.0.body.vel[2]);
    let jitter = Vec3::new(
        (t * 13.0).sin() * 0.35,
        0.4 + (t * 17.0).cos() * 0.4,
        (t * 19.0).sin() * 0.35,
    );

    if has_molten {
        p.spawn(
            pos + jitter,
            Vec3::new(0.0, 1.4, 0.0),
            Color::srgb(1.0, 0.50, 0.08),
            0.10,
            0.45,
            -1.2,
            &s,
        );
    } else if has_shadow && vel.length() > WALK_SPEED {
        p.spawn(
            pos + jitter,
            Vec3::new(
                -player.0.body.vel[0] * 0.2,
                0.6,
                -player.0.body.vel[2] * 0.2,
            ),
            Color::srgb(0.55, 0.20, 0.90),
            0.11,
            0.40,
            -0.5,
            &s,
        );
    } else if has_silver_gold && ((t * 10.0) as u32).is_multiple_of(3) {
        p.spawn(
            pos + jitter + Vec3::Y * 0.4,
            Vec3::new(0.0, 0.4, 0.0),
            Color::srgb(1.0, 0.96, 0.72),
            0.08,
            0.30,
            0.0,
            &s,
        );
    }
}

fn player_mode(m: Res<ControlMode>) -> bool {
    *m == ControlMode::Player
}

pub struct AvatarPlugin;

impl Plugin for AvatarPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("TERRAVIBE_DEMO").as_deref() == Ok("armor") {
            app.insert_resource(DemoArmor);
        }
        // Every rig animates, in any control mode (enemies, bosses).
        app.add_systems(Startup, setup_item_models)
            .add_systems(OnExit(AppState::InGame), |mut commands: Commands| {
                commands.remove_resource::<Avatar>();
            })
            .add_systems(
                Update,
                animate_rigs
                    .after(drive_avatar)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                animate_held_arm
                    .after(animate_rigs)
                    .run_if(player_mode)
                    .run_if(resource_exists::<Avatar>)
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                (
                    spawn_avatar
                        .run_if(not(resource_exists::<Avatar>))
                        .run_if(resource_exists::<LocalPlayer>)
                        .run_if(resource_exists::<PlayerView>),
                    (sync_armor, sync_held, drive_avatar, drive_armor_effects)
                        .chain()
                        .run_if(resource_exists::<Avatar>)
                        .run_if(resource_exists::<LocalPlayer>)
                        .run_if(resource_exists::<PlayerView>),
                )
                    .chain()
                    .run_if(player_mode)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::item::WOODEN_ARROW;

    #[test]
    fn hand_and_first_person_scale_follow_voxel_unit() {
        assert_eq!(HAND_POS.y * VOXELS_PER_UNIT, -9.5);
        // A 16-voxel-tall item is exactly one block, so it scales by FP_SIZE.
        assert_eq!(fp_scale(16.0, 16.0), FP_SIZE / 1.0);
        assert_eq!(fp_scale(8.0, 32.0), FP_SIZE / 2.0);
    }

    #[test]
    fn walk_anim_chosen_when_moving() {
        assert_eq!(pick_anim(Vec2::new(0.1, 0.0), true, false), "walk");
        assert_eq!(pick_anim(Vec2::ZERO, true, false), "idle");
        assert_eq!(pick_anim(Vec2::new(0.1, 0.0), false, false), "jump");
        assert_eq!(pick_anim(Vec2::ZERO, true, true), "swing");
    }

    #[test]
    fn every_armor_item_has_parts_and_files() {
        let mut n = 0;
        for id in all_item_ids().filter(|i| item_data(*i).class == ItemClass::Armor) {
            let parts = armor_parts(id);
            assert!(!parts.is_empty(), "{}", item_data(id).name);
            for (part, file) in parts {
                let p = art_dir().join("models/armor").join(file);
                assert!(p.exists(), "{}: {}", item_data(id).name, p.display());
                assert!(
                    ["head", "torso", "arm_l", "arm_r", "leg_l", "leg_r"].contains(part),
                    "{part}"
                );
            }
            n += 1;
        }
        assert_eq!(n, 33);
    }

    #[test]
    fn armor_parts_cover_body_parts_by_filename() {
        let dir = art_dir();
        let mut pairs = 0;
        for id in all_item_ids().filter(|i| item_data(*i).class == ItemClass::Armor) {
            for (part, file) in armor_parts(id) {
                let body = vox::try_load(&dir.join("models/player").join(format!("{part}.vox")))
                    .unwrap_or_else(|e| panic!("{e}"));
                let armor = vox::try_load(&dir.join("models/armor").join(file))
                    .unwrap_or_else(|e| panic!("{e}"));
                for axis in 0..3 {
                    assert_eq!(
                        armor.size[axis],
                        body.size[axis] + 2,
                        "{}: {file} {:?} is not the player {part} {:?} plus 2",
                        item_data(id).name,
                        armor.size,
                        body.size
                    );
                }
                pairs += 1;
            }
        }
        assert_eq!(pairs, 66);
    }

    #[test]
    fn every_tool_item_has_model() {
        let mut n = 0;
        for id in all_item_ids() {
            if let Some(f) = item_model_file(id) {
                let p = art_dir().join("models/items").join(&f);
                assert!(p.exists(), "{}: {}", item_data(id).name, p.display());
                n += 1;
            }
        }
        // Every pick, axe, sword and bow, plus the arrow and the torch.
        assert!(n >= 20, "{n}");
        assert!(item_model_file(WOODEN_ARROW).is_some() && item_model_file(TORCH).is_some());
    }

    #[test]
    fn all_items_have_3d_held_representation() {
        let ids: Vec<_> = all_item_ids().collect();
        assert_eq!(ids.len(), 225);
        for id in ids {
            match held_visual_for_item(id) {
                HeldVisualKind::Vox { file, .. } => {
                    let p = art_dir().join("models/items").join(file);
                    assert!(
                        p.exists(),
                        "item {:?} ({}) maps to missing vox {}",
                        item_data(id).name,
                        id.0,
                        p.display()
                    );
                }
                HeldVisualKind::Block(_) => {}
            }
        }
    }

    #[test]
    fn iframe_blink_alternates_when_hurt() {
        assert!(!iframe_blink_hidden(0));
        assert!(!iframe_blink_hidden(1));
        assert!(iframe_blink_hidden(4));
        assert!(!iframe_blink_hidden(8));
    }

    #[test]
    fn tp_held_rotation_extends_tools_forward_and_outward() {
        let tool_rot = tp_held_rotation(false);
        assert!(tool_rot.is_normalized());
        // Model diagonal (+1, +1, 0) in arm_r local space must extend forward (+Z)
        // and slightly upward (+Y) so tools/swords never clip into the floor.
        let tip_in_arm = tool_rot * Vec3::new(1.0, 1.0, 0.0);
        assert!(
            tip_in_arm.y > 0.0,
            "tip should angle upward away from floor: {tip_in_arm:?}"
        );
        assert!(
            tip_in_arm.z > 0.5,
            "tip should point forward: {tip_in_arm:?}"
        );

        // When arm_r is raised forward by -90 deg around X, tip points world +Y (overhead).
        let arm_forward = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        let tip_in_world = arm_forward * tip_in_arm;
        assert!(tip_in_world.y > 0.5, "world up +Y: {tip_in_world:?}");

        let upright_rot = tp_held_rotation(true);
        assert!(upright_rot.is_normalized());

        // Axe cutting edge (+X in model space) must point forward (+Z in arm_r space)
        let edge_in_arm = tool_rot * Vec3::X;
        assert!(
            edge_in_arm.z > 0.5,
            "tool/axe cutting edge (+X) must face forward (+Z) in arm_r: {edge_in_arm:?}"
        );
    }
}
