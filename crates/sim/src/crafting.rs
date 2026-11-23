//! Static recipe table. Plain data: no Bevy, no wall clock.

use crate::block::{
    AIR, ANVIL, BOTTLE, BlockId, DEMON_ALTAR, FURNACE, HELLFORGE, TINKERERS_WORKSHOP, WORKBENCH,
};
use crate::chunk::BlockQuery;
use crate::inventory::{Inventory, ItemStack};
use crate::item::Prefix;
use crate::item::{
    ANVIL_ITEM, BOTTLE_ITEM, CAMPFIRE_ITEM, CHEST_ITEM, COPPER_AXE, COPPER_BAR, COPPER_BROADSWORD,
    COPPER_CHAINMAIL, COPPER_GREAVES, COPPER_HELMET, COPPER_ORE, COPPER_PICKAXE, DEMON_BOW,
    DEMONITE_BAR, DEMONITE_ORE_ITEM, EBONSTONE_BLOCK, EMPTY_BUCKET, ENCHANTED_BOOMERANG,
    FALLEN_STAR, FIERY_GREATSWORD, FLAMARANG, FLAMING_ARROW, FURNACE_ITEM, GEL, GLASS,
    GLOWING_MUSHROOM, GOLD_AXE, GOLD_BAR, GOLD_BROADSWORD, GOLD_CHAINMAIL, GOLD_GREAVES,
    GOLD_HELMET, GOLD_ORE, GOLD_PICKAXE, GRAPPLING_HOOK, HANDGUN, HEALING_POTION,
    HEART_LANTERN_ITEM, HELLSTONE_BAR, HELLSTONE_BRICK_ITEM, HELLSTONE_ITEM, HOOK, IRON_AXE,
    IRON_BAR, IRON_BROADSWORD, IRON_CHAIN, IRON_CHAINMAIL, IRON_GREAVES, IRON_HELMET, IRON_ORE,
    IRON_PICKAXE, IRONSKIN_POTION, ItemId, LENS, LESSER_HEALING_POTION, LESSER_MANA_POTION,
    LIFE_CRYSTAL_ITEM, LIGHTS_BANE, MANA_CRYSTAL, MANA_POTION, MOLTEN_BREASTPLATE, MOLTEN_FURY,
    MOLTEN_GREAVES, MOLTEN_HAMAXE, MOLTEN_HELMET, MOLTEN_PICKAXE, MUSHROOM, NIGHTMARE_PICKAXE,
    OBSIDIAN_BRICK_ITEM, OBSIDIAN_ITEM, OBSIDIAN_SKULL, PHOENIX_BLASTER, REGENERATION_POTION,
    SAND_BLOCK, SHADOW_GREAVES, SHADOW_HELMET, SHADOW_SCALE, SHADOW_SCALEMAIL, SHINE_POTION,
    SILVER_AXE, SILVER_BAR, SILVER_BROADSWORD, SILVER_CHAINMAIL, SILVER_GREAVES, SILVER_HELMET,
    SILVER_ORE, SILVER_PICKAXE, STONE_BLOCK, SUSPICIOUS_LOOKING_EYE, SWIFTNESS_POTION, THE_BREAKER,
    TORCH, WAR_AXE_OF_THE_NIGHT, WATER_WALKING_POTION, WOOD, WOOD_PLATFORM, WOOD_WALL,
    WOODEN_ARROW, WOODEN_BOW, WOODEN_CHAIR, WOODEN_DOOR, WOODEN_HAMMER, WOODEN_SWORD, WOODEN_TABLE,
    WORKBENCH_ITEM, WORM_FOOD,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RecipeId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ingredient {
    pub item: ItemId,
    pub count: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Recipe {
    pub id: RecipeId,
    pub output: Ingredient,
    pub ingredients: &'static [Ingredient],
    /// Tile that must be within reach, if any.
    pub station: Option<BlockId>,
}

pub const WORKBENCH_RECIPE: RecipeId = RecipeId(0);
pub const TORCH_RECIPE: RecipeId = RecipeId(1);
pub const WOODEN_SWORD_RECIPE: RecipeId = RecipeId(2);
pub const COPPER_BAR_RECIPE: RecipeId = RecipeId(3);
pub const IRON_BAR_RECIPE: RecipeId = RecipeId(4);
pub const SILVER_BAR_RECIPE: RecipeId = RecipeId(5);
pub const GOLD_BAR_RECIPE: RecipeId = RecipeId(6);
pub const FURNACE_RECIPE: RecipeId = RecipeId(7);
pub const ANVIL_RECIPE: RecipeId = RecipeId(8);
pub const COPPER_PICKAXE_RECIPE: RecipeId = RecipeId(9);
pub const COPPER_AXE_RECIPE: RecipeId = RecipeId(10);
pub const COPPER_BROADSWORD_RECIPE: RecipeId = RecipeId(11);
pub const IRON_PICKAXE_RECIPE: RecipeId = RecipeId(12);
pub const IRON_AXE_RECIPE: RecipeId = RecipeId(13);
pub const IRON_BROADSWORD_RECIPE: RecipeId = RecipeId(14);
pub const SILVER_PICKAXE_RECIPE: RecipeId = RecipeId(15);
pub const SILVER_AXE_RECIPE: RecipeId = RecipeId(16);
pub const SILVER_BROADSWORD_RECIPE: RecipeId = RecipeId(17);
pub const GOLD_PICKAXE_RECIPE: RecipeId = RecipeId(18);
pub const GOLD_AXE_RECIPE: RecipeId = RecipeId(19);
pub const GOLD_BROADSWORD_RECIPE: RecipeId = RecipeId(20);
pub const COPPER_HELMET_RECIPE: RecipeId = RecipeId(21);
pub const COPPER_CHAINMAIL_RECIPE: RecipeId = RecipeId(22);
pub const COPPER_GREAVES_RECIPE: RecipeId = RecipeId(23);
pub const IRON_HELMET_RECIPE: RecipeId = RecipeId(24);
pub const IRON_CHAINMAIL_RECIPE: RecipeId = RecipeId(25);
pub const IRON_GREAVES_RECIPE: RecipeId = RecipeId(26);
pub const SILVER_HELMET_RECIPE: RecipeId = RecipeId(27);
pub const SILVER_CHAINMAIL_RECIPE: RecipeId = RecipeId(28);
pub const SILVER_GREAVES_RECIPE: RecipeId = RecipeId(29);
pub const GOLD_HELMET_RECIPE: RecipeId = RecipeId(30);
pub const GOLD_CHAINMAIL_RECIPE: RecipeId = RecipeId(31);
pub const GOLD_GREAVES_RECIPE: RecipeId = RecipeId(32);
pub const CHEST_RECIPE: RecipeId = RecipeId(33);

pub const WOODEN_DOOR_RECIPE: RecipeId = RecipeId(34);
pub const WOODEN_CHAIR_RECIPE: RecipeId = RecipeId(35);
pub const WOODEN_TABLE_RECIPE: RecipeId = RecipeId(36);
pub const WOOD_PLATFORM_RECIPE: RecipeId = RecipeId(37);
pub const WOODEN_BOW_RECIPE: RecipeId = RecipeId(38);
pub const WOODEN_ARROW_RECIPE: RecipeId = RecipeId(39);
pub const FLAMING_ARROW_RECIPE: RecipeId = RecipeId(40);

pub const GLASS_RECIPE: RecipeId = RecipeId(41);
pub const BOTTLE_RECIPE: RecipeId = RecipeId(42);
pub const LESSER_HEALING_RECIPE: RecipeId = RecipeId(43);
pub const SUSPICIOUS_EYE_RECIPE: RecipeId = RecipeId(44);
pub const WOOD_WALL_RECIPE: RecipeId = RecipeId(45);
pub const EMPTY_BUCKET_RECIPE: RecipeId = RecipeId(46);
pub const DEMONITE_BAR_RECIPE: RecipeId = RecipeId(47);
pub const WAR_AXE_OF_THE_NIGHT_RECIPE: RecipeId = RecipeId(48);
pub const LIGHTS_BANE_RECIPE: RecipeId = RecipeId(49);
pub const SHADOW_HELMET_RECIPE: RecipeId = RecipeId(50);
pub const SHADOW_SCALEMAIL_RECIPE: RecipeId = RecipeId(51);
pub const SHADOW_GREAVES_RECIPE: RecipeId = RecipeId(52);
pub const NIGHTMARE_PICKAXE_RECIPE: RecipeId = RecipeId(53);
pub const MANA_CRYSTAL_RECIPE: RecipeId = RecipeId(54);
pub const WOODEN_HAMMER_RECIPE: RecipeId = RecipeId(55);
pub const THE_BREAKER_RECIPE: RecipeId = RecipeId(56);
pub const DEMON_BOW_RECIPE: RecipeId = RecipeId(57);

pub const HELLSTONE_BAR_RECIPE: RecipeId = RecipeId(58);
pub const MOLTEN_HELMET_RECIPE: RecipeId = RecipeId(59);
pub const MOLTEN_BREASTPLATE_RECIPE: RecipeId = RecipeId(60);
pub const MOLTEN_GREAVES_RECIPE: RecipeId = RecipeId(61);
pub const MOLTEN_PICKAXE_RECIPE: RecipeId = RecipeId(62);
pub const MOLTEN_HAMAXE_RECIPE: RecipeId = RecipeId(63);
pub const FIERY_GREATSWORD_RECIPE: RecipeId = RecipeId(64);
pub const MOLTEN_FURY_RECIPE: RecipeId = RecipeId(65);
pub const PHOENIX_BLASTER_RECIPE: RecipeId = RecipeId(66);
pub const FLAMARANG_RECIPE: RecipeId = RecipeId(67);
pub const OBSIDIAN_SKULL_RECIPE: RecipeId = RecipeId(68);
pub const OBSIDIAN_BRICK_RECIPE: RecipeId = RecipeId(69);
pub const HELLSTONE_BRICK_RECIPE: RecipeId = RecipeId(70);
pub const WORM_FOOD_RECIPE: RecipeId = RecipeId(71);
pub const HEALING_POTION_RECIPE: RecipeId = RecipeId(72);
pub const MANA_POTION_RECIPE: RecipeId = RecipeId(73);
pub const IRONSKIN_POTION_RECIPE: RecipeId = RecipeId(74);
pub const SWIFTNESS_POTION_RECIPE: RecipeId = RecipeId(75);
pub const REGENERATION_POTION_RECIPE: RecipeId = RecipeId(76);
pub const SHINE_POTION_RECIPE: RecipeId = RecipeId(77);
pub const WATER_WALKING_POTION_RECIPE: RecipeId = RecipeId(78);
pub const IRON_CHAIN_RECIPE: RecipeId = RecipeId(79);
pub const GRAPPLING_HOOK_RECIPE: RecipeId = RecipeId(80);
pub const CAMPFIRE_RECIPE: RecipeId = RecipeId(81);
pub const HEART_LANTERN_RECIPE: RecipeId = RecipeId(82);

const fn ing(item: ItemId, count: u32) -> Ingredient {
    Ingredient { item, count }
}

pub static RECIPES: [Recipe; 83] = [
    // gameplay spec: 10 Wood -> Work Bench, no station
    Recipe {
        id: WORKBENCH_RECIPE,
        output: ing(WORKBENCH_ITEM, 1),
        ingredients: &[ing(WOOD, 10)],
        station: None,
    },
    // gameplay spec: 1 Gel + 1 Wood -> 3 Torches, no station
    Recipe {
        id: TORCH_RECIPE,
        output: ing(TORCH, 3),
        ingredients: &[ing(GEL, 1), ing(WOOD, 1)],
        station: None,
    },
    // gameplay spec: 7 Wood -> Wooden Sword at tile 18
    Recipe {
        id: WOODEN_SWORD_RECIPE,
        output: ing(WOODEN_SWORD, 1),
        ingredients: &[ing(WOOD, 7)],
        station: Some(WORKBENCH),
    },
    // gameplay spec (and 9817, 10076, 10236 for the other bars): ore -> bar at tile 17
    Recipe {
        id: COPPER_BAR_RECIPE,
        output: ing(COPPER_BAR, 1),
        ingredients: &[ing(COPPER_ORE, 3)],
        station: Some(FURNACE),
    },
    Recipe {
        id: IRON_BAR_RECIPE,
        output: ing(IRON_BAR, 1),
        ingredients: &[ing(IRON_ORE, 3)],
        station: Some(FURNACE),
    },
    Recipe {
        id: SILVER_BAR_RECIPE,
        output: ing(SILVER_BAR, 1),
        ingredients: &[ing(SILVER_ORE, 4)],
        station: Some(FURNACE),
    },
    Recipe {
        id: GOLD_BAR_RECIPE,
        output: ing(GOLD_BAR, 1),
        ingredients: &[ing(GOLD_ORE, 4)],
        station: Some(FURNACE),
    },
    // gameplay spec: 20 Stone + 4 Wood + 3 Torch at tile 18 (stone/wood are groups)
    Recipe {
        id: FURNACE_RECIPE,
        output: ing(FURNACE_ITEM, 1),
        ingredients: &[ing(STONE_BLOCK, 20), ing(WOOD, 4), ing(TORCH, 3)],
        station: Some(WORKBENCH),
    },
    // gameplay spec: 5 Iron Bar at tile 18
    Recipe {
        id: ANVIL_RECIPE,
        output: ing(ANVIL_ITEM, 1),
        ingredients: &[ing(IRON_BAR, 5)],
        station: Some(WORKBENCH),
    },
    // gameplay spec: 8 bar + 4 wood at tile 16
    Recipe {
        id: COPPER_PICKAXE_RECIPE,
        output: ing(COPPER_PICKAXE, 1),
        ingredients: &[ing(COPPER_BAR, 8), ing(WOOD, 4)],
        station: Some(ANVIL),
    },
    // gameplay spec: 6 bar + 3 wood at tile 16
    Recipe {
        id: COPPER_AXE_RECIPE,
        output: ing(COPPER_AXE, 1),
        ingredients: &[ing(COPPER_BAR, 6), ing(WOOD, 3)],
        station: Some(ANVIL),
    },
    // gameplay spec: 6 bar at tile 16
    Recipe {
        id: COPPER_BROADSWORD_RECIPE,
        output: ing(COPPER_BROADSWORD, 1),
        ingredients: &[ing(COPPER_BAR, 6)],
        station: Some(ANVIL),
    },
    // gameplay spec: 10 bar + 3 wood at tile 16
    Recipe {
        id: IRON_PICKAXE_RECIPE,
        output: ing(IRON_PICKAXE, 1),
        ingredients: &[ing(IRON_BAR, 10), ing(WOOD, 3)],
        station: Some(ANVIL),
    },
    // gameplay spec: 8 bar + 3 wood at tile 16
    Recipe {
        id: IRON_AXE_RECIPE,
        output: ing(IRON_AXE, 1),
        ingredients: &[ing(IRON_BAR, 8), ing(WOOD, 3)],
        station: Some(ANVIL),
    },
    // gameplay spec: 8 bar at tile 16
    Recipe {
        id: IRON_BROADSWORD_RECIPE,
        output: ing(IRON_BROADSWORD, 1),
        ingredients: &[ing(IRON_BAR, 8)],
        station: Some(ANVIL),
    },
    // gameplay spec: 10 bar + 4 wood at tile 16
    Recipe {
        id: SILVER_PICKAXE_RECIPE,
        output: ing(SILVER_PICKAXE, 1),
        ingredients: &[ing(SILVER_BAR, 10), ing(WOOD, 4)],
        station: Some(ANVIL),
    },
    // gameplay spec: 8 bar + 3 wood at tile 16
    Recipe {
        id: SILVER_AXE_RECIPE,
        output: ing(SILVER_AXE, 1),
        ingredients: &[ing(SILVER_BAR, 8), ing(WOOD, 3)],
        station: Some(ANVIL),
    },
    // gameplay spec: 8 bar at tile 16
    Recipe {
        id: SILVER_BROADSWORD_RECIPE,
        output: ing(SILVER_BROADSWORD, 1),
        ingredients: &[ing(SILVER_BAR, 8)],
        station: Some(ANVIL),
    },
    // gameplay spec: 10 bar + 4 wood at tile 16
    Recipe {
        id: GOLD_PICKAXE_RECIPE,
        output: ing(GOLD_PICKAXE, 1),
        ingredients: &[ing(GOLD_BAR, 10), ing(WOOD, 4)],
        station: Some(ANVIL),
    },
    // gameplay spec: 8 bar + 3 wood at tile 16
    Recipe {
        id: GOLD_AXE_RECIPE,
        output: ing(GOLD_AXE, 1),
        ingredients: &[ing(GOLD_BAR, 8), ing(WOOD, 3)],
        station: Some(ANVIL),
    },
    // gameplay spec: 8 bar at tile 16
    Recipe {
        id: GOLD_BROADSWORD_RECIPE,
        output: ing(GOLD_BROADSWORD, 1),
        ingredients: &[ing(GOLD_BAR, 8)],
        station: Some(ANVIL),
    },
    // gameplay spec: 12 bar at tile 16
    Recipe {
        id: COPPER_HELMET_RECIPE,
        output: ing(COPPER_HELMET, 1),
        ingredients: &[ing(COPPER_BAR, 12)],
        station: Some(ANVIL),
    },
    // gameplay spec: 20 bar at tile 16
    Recipe {
        id: COPPER_CHAINMAIL_RECIPE,
        output: ing(COPPER_CHAINMAIL, 1),
        ingredients: &[ing(COPPER_BAR, 20)],
        station: Some(ANVIL),
    },
    // gameplay spec: 16 bar at tile 16
    Recipe {
        id: COPPER_GREAVES_RECIPE,
        output: ing(COPPER_GREAVES, 1),
        ingredients: &[ing(COPPER_BAR, 16)],
        station: Some(ANVIL),
    },
    // gameplay spec: 15 bar at tile 16
    Recipe {
        id: IRON_HELMET_RECIPE,
        output: ing(IRON_HELMET, 1),
        ingredients: &[ing(IRON_BAR, 15)],
        station: Some(ANVIL),
    },
    // gameplay spec: 25 bar at tile 16
    Recipe {
        id: IRON_CHAINMAIL_RECIPE,
        output: ing(IRON_CHAINMAIL, 1),
        ingredients: &[ing(IRON_BAR, 25)],
        station: Some(ANVIL),
    },
    // gameplay spec: 20 bar at tile 16
    Recipe {
        id: IRON_GREAVES_RECIPE,
        output: ing(IRON_GREAVES, 1),
        ingredients: &[ing(IRON_BAR, 20)],
        station: Some(ANVIL),
    },
    // gameplay spec: 15 bar at tile 16
    Recipe {
        id: SILVER_HELMET_RECIPE,
        output: ing(SILVER_HELMET, 1),
        ingredients: &[ing(SILVER_BAR, 15)],
        station: Some(ANVIL),
    },
    // gameplay spec: 25 bar at tile 16
    Recipe {
        id: SILVER_CHAINMAIL_RECIPE,
        output: ing(SILVER_CHAINMAIL, 1),
        ingredients: &[ing(SILVER_BAR, 25)],
        station: Some(ANVIL),
    },
    // gameplay spec: 20 bar at tile 16
    Recipe {
        id: SILVER_GREAVES_RECIPE,
        output: ing(SILVER_GREAVES, 1),
        ingredients: &[ing(SILVER_BAR, 20)],
        station: Some(ANVIL),
    },
    // gameplay spec: 20 bar at tile 16
    Recipe {
        id: GOLD_HELMET_RECIPE,
        output: ing(GOLD_HELMET, 1),
        ingredients: &[ing(GOLD_BAR, 20)],
        station: Some(ANVIL),
    },
    // gameplay spec: 30 bar at tile 16
    Recipe {
        id: GOLD_CHAINMAIL_RECIPE,
        output: ing(GOLD_CHAINMAIL, 1),
        ingredients: &[ing(GOLD_BAR, 30)],
        station: Some(ANVIL),
    },
    // gameplay spec: 25 bar at tile 16
    Recipe {
        id: GOLD_GREAVES_RECIPE,
        output: ing(GOLD_GREAVES, 1),
        ingredients: &[ing(GOLD_BAR, 25)],
        station: Some(ANVIL),
    },
    // gameplay spec: 8 Wood + 2 Iron Bar (IronBar group) at tile 18
    Recipe {
        id: CHEST_RECIPE,
        output: ing(CHEST_ITEM, 1),
        ingredients: &[ing(WOOD, 8), ing(IRON_BAR, 2)],
        station: Some(WORKBENCH),
    },
    // gameplay spec: 6 Wood -> Wooden Door at tile 18
    Recipe {
        id: WOODEN_DOOR_RECIPE,
        output: ing(WOODEN_DOOR, 1),
        ingredients: &[ing(WOOD, 6)],
        station: Some(WORKBENCH),
    },
    // gameplay spec: 4 Wood -> Wooden Chair at tile 18
    Recipe {
        id: WOODEN_CHAIR_RECIPE,
        output: ing(WOODEN_CHAIR, 1),
        ingredients: &[ing(WOOD, 4)],
        station: Some(WORKBENCH),
    },
    // gameplay spec: 8 Wood -> Wooden Table at tile 18
    Recipe {
        id: WOODEN_TABLE_RECIPE,
        output: ing(WOODEN_TABLE, 1),
        ingredients: &[ing(WOOD, 8)],
        station: Some(WORKBENCH),
    },
    // gameplay spec: 1 Wood -> 2 Wood Platforms, no station
    Recipe {
        id: WOOD_PLATFORM_RECIPE,
        output: ing(WOOD_PLATFORM, 2),
        ingredients: &[ing(WOOD, 1)],
        station: None,
    },
    // gameplay spec: 10 Wood -> Wooden Bow at tile 18
    Recipe {
        id: WOODEN_BOW_RECIPE,
        output: ing(WOODEN_BOW, 1),
        ingredients: &[ing(WOOD, 10)],
        station: Some(WORKBENCH),
    },
    // gameplay spec: 1 Wood + 1 Stone Block -> 25 Wooden Arrows at tile 18
    Recipe {
        id: WOODEN_ARROW_RECIPE,
        output: ing(WOODEN_ARROW, 25),
        ingredients: &[ing(WOOD, 1), ing(STONE_BLOCK, 1)],
        station: Some(WORKBENCH),
    },
    // gameplay spec: 10 Wooden Arrows + 1 Torch -> 10 Flaming Arrows, no station
    Recipe {
        id: FLAMING_ARROW_RECIPE,
        output: ing(FLAMING_ARROW, 10),
        ingredients: &[ing(WOODEN_ARROW, 10), ing(TORCH, 1)],
        station: None,
    },
    // gameplay spec: 2 Sand -> 1 Glass at a Furnace (sand group, tile 17)
    Recipe {
        id: GLASS_RECIPE,
        output: ing(GLASS, 1),
        ingredients: &[ing(SAND_BLOCK, 2)],
        station: Some(FURNACE),
    },
    // gameplay spec: 1 Glass -> 2 Bottles at a Furnace (tile 17)
    Recipe {
        id: BOTTLE_RECIPE,
        output: ing(BOTTLE_ITEM, 2),
        ingredients: &[ing(GLASS, 1)],
        station: Some(FURNACE),
    },
    // gameplay spec: createItem 28 stack 2 from 1 Mushroom + 2 Gel + 2 Bottle at
    // requiredTile 13 (Bottles). Vanilla's placed bottle must be within reach, as here.
    Recipe {
        id: LESSER_HEALING_RECIPE,
        output: ing(LESSER_HEALING_POTION, 2),
        ingredients: &[ing(MUSHROOM, 1), ing(GEL, 2), ing(BOTTLE_ITEM, 2)],
        station: Some(BOTTLE),
    },
    // gameplay spec: 6 Lens at requiredTile 26 (Demon Altar).
    Recipe {
        id: SUSPICIOUS_EYE_RECIPE,
        output: ing(SUSPICIOUS_LOOKING_EYE, 1),
        ingredients: &[ing(LENS, 6)],
        station: Some(DEMON_ALTAR),
    },
    // gameplay spec: 1 Wood -> 4 Wood Walls at tile 18 (Work Bench)
    Recipe {
        id: WOOD_WALL_RECIPE,
        output: ing(WOOD_WALL, 4),
        ingredients: &[ing(WOOD, 1)],
        station: Some(WORKBENCH),
    },
    // gameplay spec: 3 Iron Bar at tile 16 (Anvil)
    Recipe {
        id: EMPTY_BUCKET_RECIPE,
        output: ing(EMPTY_BUCKET, 1),
        ingredients: &[ing(IRON_BAR, 3)],
        station: Some(ANVIL),
    },
    // gameplay spec: 3 Demonite Ore -> 1 Demonite Bar at tile 17 (Furnace)
    Recipe {
        id: DEMONITE_BAR_RECIPE,
        output: ing(DEMONITE_BAR, 1),
        ingredients: &[ing(DEMONITE_ORE_ITEM, 3)],
        station: Some(FURNACE),
    },
    // gameplay spec: 10 Demonite Bar -> 1 War Axe of the Night at tile 16 (Iron Anvil)
    Recipe {
        id: WAR_AXE_OF_THE_NIGHT_RECIPE,
        output: ing(WAR_AXE_OF_THE_NIGHT, 1),
        ingredients: &[ing(DEMONITE_BAR, 10)],
        station: Some(ANVIL),
    },
    // gameplay spec: 10 Demonite Bar -> 1 Light's Bane at tile 16 (Iron Anvil)
    Recipe {
        id: LIGHTS_BANE_RECIPE,
        output: ing(LIGHTS_BANE, 1),
        ingredients: &[ing(DEMONITE_BAR, 10)],
        station: Some(ANVIL),
    },
    // gameplay spec: 15 Demonite Bar + 10 Shadow Scale -> 1 Shadow Helmet at tile 16 (Iron Anvil)
    Recipe {
        id: SHADOW_HELMET_RECIPE,
        output: ing(SHADOW_HELMET, 1),
        ingredients: &[ing(DEMONITE_BAR, 15), ing(SHADOW_SCALE, 10)],
        station: Some(ANVIL),
    },
    // gameplay spec: 25 Demonite Bar + 20 Shadow Scale -> 1 Shadow Scalemail at tile 16 (Iron Anvil)
    Recipe {
        id: SHADOW_SCALEMAIL_RECIPE,
        output: ing(SHADOW_SCALEMAIL, 1),
        ingredients: &[ing(DEMONITE_BAR, 25), ing(SHADOW_SCALE, 20)],
        station: Some(ANVIL),
    },
    // gameplay spec: 20 Demonite Bar + 15 Shadow Scale -> 1 Shadow Greaves at tile 16 (Iron Anvil)
    Recipe {
        id: SHADOW_GREAVES_RECIPE,
        output: ing(SHADOW_GREAVES, 1),
        ingredients: &[ing(DEMONITE_BAR, 20), ing(SHADOW_SCALE, 15)],
        station: Some(ANVIL),
    },
    // gameplay spec: 12 Demonite Bar + 6 Shadow Scale -> 1 Nightmare Pickaxe at tile 16 (Iron Anvil)
    Recipe {
        id: NIGHTMARE_PICKAXE_RECIPE,
        output: ing(NIGHTMARE_PICKAXE, 1),
        ingredients: &[ing(DEMONITE_BAR, 12), ing(SHADOW_SCALE, 6)],
        station: Some(ANVIL),
    },
    // gameplay spec: 5 Fallen Stars -> 1 Mana Crystal, no station
    Recipe {
        id: MANA_CRYSTAL_RECIPE,
        output: ing(MANA_CRYSTAL, 1),
        ingredients: &[ing(FALLEN_STAR, 5)],
        station: None,
    },
    // gameplay spec: 8 Wood -> Wooden Hammer at tile 18 (Workbench)
    Recipe {
        id: WOODEN_HAMMER_RECIPE,
        output: ing(WOODEN_HAMMER, 1),
        ingredients: &[ing(WOOD, 8)],
        station: Some(WORKBENCH),
    },
    // gameplay spec: 10 Demonite Bar at tile 16 (Anvil)
    Recipe {
        id: THE_BREAKER_RECIPE,
        output: ing(THE_BREAKER, 1),
        ingredients: &[ing(DEMONITE_BAR, 10)],
        station: Some(ANVIL),
    },
    // gameplay spec: 8 Demonite Bar -> 1 Demon Bow at tile 16 (Iron Anvil)
    Recipe {
        id: DEMON_BOW_RECIPE,
        output: ing(DEMON_BOW, 1),
        ingredients: &[ing(DEMONITE_BAR, 8)],
        station: Some(ANVIL),
    },
    // M10 spec: 3 Hellstone + 1 Obsidian -> 1 Hellstone Bar at a Hellforge (tile 77)
    Recipe {
        id: HELLSTONE_BAR_RECIPE,
        output: ing(HELLSTONE_BAR, 1),
        ingredients: &[ing(HELLSTONE_ITEM, 3), ing(OBSIDIAN_ITEM, 1)],
        station: Some(HELLFORGE),
    },
    // M10 spec: 10 Hellstone Bar -> 1 Molten Helmet at an Iron Anvil
    Recipe {
        id: MOLTEN_HELMET_RECIPE,
        output: ing(MOLTEN_HELMET, 1),
        ingredients: &[ing(HELLSTONE_BAR, 10)],
        station: Some(ANVIL),
    },
    // M10 spec: 20 Hellstone Bar -> 1 Molten Breastplate at an Iron Anvil
    Recipe {
        id: MOLTEN_BREASTPLATE_RECIPE,
        output: ing(MOLTEN_BREASTPLATE, 1),
        ingredients: &[ing(HELLSTONE_BAR, 20)],
        station: Some(ANVIL),
    },
    // M10 spec: 15 Hellstone Bar -> 1 Molten Greaves at an Iron Anvil
    Recipe {
        id: MOLTEN_GREAVES_RECIPE,
        output: ing(MOLTEN_GREAVES, 1),
        ingredients: &[ing(HELLSTONE_BAR, 15)],
        station: Some(ANVIL),
    },
    // M10 spec: 20 Hellstone Bar -> 1 Molten Pickaxe at an Iron Anvil
    Recipe {
        id: MOLTEN_PICKAXE_RECIPE,
        output: ing(MOLTEN_PICKAXE, 1),
        ingredients: &[ing(HELLSTONE_BAR, 20)],
        station: Some(ANVIL),
    },
    // M10 spec: 15 Hellstone Bar -> 1 Molten Hamaxe at an Iron Anvil
    Recipe {
        id: MOLTEN_HAMAXE_RECIPE,
        output: ing(MOLTEN_HAMAXE, 1),
        ingredients: &[ing(HELLSTONE_BAR, 15)],
        station: Some(ANVIL),
    },
    // M10 spec: 20 Hellstone Bar -> 1 Fiery Greatsword at an Iron Anvil
    Recipe {
        id: FIERY_GREATSWORD_RECIPE,
        output: ing(FIERY_GREATSWORD, 1),
        ingredients: &[ing(HELLSTONE_BAR, 20)],
        station: Some(ANVIL),
    },
    // M10 spec: 15 Hellstone Bar -> 1 Molten Fury at an Iron Anvil
    Recipe {
        id: MOLTEN_FURY_RECIPE,
        output: ing(MOLTEN_FURY, 1),
        ingredients: &[ing(HELLSTONE_BAR, 15)],
        station: Some(ANVIL),
    },
    // M10 spec: 1 Handgun + 10 Hellstone Bar -> 1 Phoenix Blaster at an Iron Anvil
    Recipe {
        id: PHOENIX_BLASTER_RECIPE,
        output: ing(PHOENIX_BLASTER, 1),
        ingredients: &[ing(HANDGUN, 1), ing(HELLSTONE_BAR, 10)],
        station: Some(ANVIL),
    },
    // M10 spec: 1 Enchanted Boomerang + 15 Hellstone Bar -> 1 Flamarang at an Iron Anvil
    Recipe {
        id: FLAMARANG_RECIPE,
        output: ing(FLAMARANG, 1),
        ingredients: &[ing(ENCHANTED_BOOMERANG, 1), ing(HELLSTONE_BAR, 15)],
        station: Some(ANVIL),
    },
    // gameplay spec: 20 Obsidian -> 1 Obsidian Skull at tile 17 (Furnace)
    Recipe {
        id: OBSIDIAN_SKULL_RECIPE,
        output: ing(OBSIDIAN_SKULL, 1),
        ingredients: &[ing(OBSIDIAN_ITEM, 20)],
        station: Some(FURNACE),
    },
    // gameplay spec: 2 Obsidian -> 1 Obsidian Brick at Furnace
    Recipe {
        id: OBSIDIAN_BRICK_RECIPE,
        output: ing(OBSIDIAN_BRICK_ITEM, 1),
        ingredients: &[ing(OBSIDIAN_ITEM, 2)],
        station: Some(FURNACE),
    },
    // gameplay spec: 1 Stone Block + 1 Hellstone -> 1 Hellstone Brick at tile 17 (Furnace)
    Recipe {
        id: HELLSTONE_BRICK_RECIPE,
        output: ing(HELLSTONE_BRICK_ITEM, 1),
        ingredients: &[ing(STONE_BLOCK, 1), ing(HELLSTONE_ITEM, 1)],
        station: Some(FURNACE),
    },
    // Demon Altar Corruption boss summon: 15 Shadow Scale + 15 Ebonstone Block -> 1 Worm Food
    Recipe {
        id: WORM_FOOD_RECIPE,
        output: ing(WORM_FOOD, 1),
        ingredients: &[ing(SHADOW_SCALE, 15), ing(EBONSTONE_BLOCK, 15)],
        station: Some(DEMON_ALTAR),
    },
    // Wave 1 Alchemy, Arena & Grapple recipes
    Recipe {
        id: HEALING_POTION_RECIPE,
        output: ing(HEALING_POTION, 2),
        ingredients: &[ing(LESSER_HEALING_POTION, 2), ing(GLOWING_MUSHROOM, 1)],
        station: Some(BOTTLE),
    },
    Recipe {
        id: MANA_POTION_RECIPE,
        output: ing(MANA_POTION, 2),
        ingredients: &[ing(LESSER_MANA_POTION, 2), ing(GLOWING_MUSHROOM, 1)],
        station: Some(BOTTLE),
    },
    Recipe {
        id: IRONSKIN_POTION_RECIPE,
        output: ing(IRONSKIN_POTION, 1),
        ingredients: &[ing(BOTTLE_ITEM, 1), ing(IRON_ORE, 1), ing(MUSHROOM, 1)],
        station: Some(BOTTLE),
    },
    Recipe {
        id: SWIFTNESS_POTION_RECIPE,
        output: ing(SWIFTNESS_POTION, 1),
        ingredients: &[ing(BOTTLE_ITEM, 1), ing(SAND_BLOCK, 1), ing(MUSHROOM, 1)],
        station: Some(BOTTLE),
    },
    Recipe {
        id: REGENERATION_POTION_RECIPE,
        output: ing(REGENERATION_POTION, 1),
        ingredients: &[
            ing(BOTTLE_ITEM, 1),
            ing(MUSHROOM, 1),
            ing(GLOWING_MUSHROOM, 1),
        ],
        station: Some(BOTTLE),
    },
    Recipe {
        id: SHINE_POTION_RECIPE,
        output: ing(SHINE_POTION, 1),
        ingredients: &[ing(BOTTLE_ITEM, 1), ing(GLOWING_MUSHROOM, 1), ing(TORCH, 1)],
        station: Some(BOTTLE),
    },
    Recipe {
        id: WATER_WALKING_POTION_RECIPE,
        output: ing(WATER_WALKING_POTION, 1),
        ingredients: &[ing(BOTTLE_ITEM, 1), ing(OBSIDIAN_ITEM, 1), ing(GEL, 1)],
        station: Some(BOTTLE),
    },
    Recipe {
        id: IRON_CHAIN_RECIPE,
        output: ing(IRON_CHAIN, 10),
        ingredients: &[ing(IRON_BAR, 1)],
        station: Some(ANVIL),
    },
    Recipe {
        id: GRAPPLING_HOOK_RECIPE,
        output: ing(GRAPPLING_HOOK, 1),
        ingredients: &[ing(HOOK, 1), ing(IRON_CHAIN, 3)],
        station: Some(ANVIL),
    },
    Recipe {
        id: CAMPFIRE_RECIPE,
        output: ing(CAMPFIRE_ITEM, 1),
        ingredients: &[ing(WOOD, 10), ing(TORCH, 5)],
        station: None,
    },
    Recipe {
        id: HEART_LANTERN_RECIPE,
        output: ing(HEART_LANTERN_ITEM, 1),
        ingredients: &[ing(LIFE_CRYSTAL_ITEM, 1), ing(IRON_CHAIN, 4)],
        station: None,
    },
];

pub fn recipe(id: RecipeId) -> Option<&'static Recipe> {
    RECIPES.iter().find(|r| r.id == id)
}

pub fn recipes() -> &'static [Recipe] {
    &RECIPES
}

/// Returns whether the given item is used as an ingredient in any crafting recipe.
pub fn is_material(id: ItemId) -> bool {
    RECIPES
        .iter()
        .any(|recipe| recipe.ingredients.iter().any(|ing| ing.item == id))
}

/// Returns all crafting recipes that use `item` as an ingredient.
pub fn recipes_using_material(item: ItemId) -> Vec<&'static Recipe> {
    RECIPES
        .iter()
        .filter(|recipe| recipe.ingredients.iter().any(|ing| ing.item == item))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftError {
    UnknownRecipe,
    InvalidCount,
    MissingIngredients,
    MissingStation,
    InventoryFull,
}

/// Crafts `count` executions of `id`; returns output item units. Station is
/// the caller's check. Commits only if every ingredient is held and the whole
/// output fits.
pub fn craft(inv: &mut Inventory, id: RecipeId, count: u16) -> Result<u32, CraftError> {
    craft_inner(inv, id, count, None)
}

/// Crafts `count` executions of `id`, rolling a prefix via `rng` when the
/// crafted item does not stack (`max_stack == 1`).
pub fn craft_with_rng(
    inv: &mut Inventory,
    id: RecipeId,
    count: u16,
    rng: &mut crate::item::Rng,
) -> Result<u32, CraftError> {
    craft_inner(inv, id, count, Some(rng))
}

fn craft_inner(
    inv: &mut Inventory,
    id: RecipeId,
    count: u16,
    rng: Option<&mut crate::item::Rng>,
) -> Result<u32, CraftError> {
    let r = recipe(id).ok_or(CraftError::UnknownRecipe)?;
    if count == 0 {
        return Err(CraftError::InvalidCount);
    }
    let n = count as u32;
    let mut next = inv.clone();
    for ing in r.ingredients {
        let need = ing.count.checked_mul(n).ok_or(CraftError::InvalidCount)?;
        if !next.remove(ing.item, need) {
            return Err(CraftError::MissingIngredients);
        }
    }
    let out = r
        .output
        .count
        .checked_mul(n)
        .ok_or(CraftError::InvalidCount)?;
    if let Some(rng) = rng
        && crate::item::item_data(r.output.item).max_stack == 1
    {
        for _ in 0..out {
            let prefix = crate::item::roll_prefix(r.output.item, &mut || rng.next_u32());
            let stack = ItemStack {
                id: r.output.item,
                count: 1,
                prefix,
            };
            if next.add(stack).is_some() {
                return Err(CraftError::InventoryFull);
            }
        }
    } else {
        let stack = ItemStack {
            id: r.output.item,
            count: out,
            prefix: Prefix(0),
        };
        if next.add(stack).is_some() {
            return Err(CraftError::InventoryFull);
        }
    }
    *inv = next;
    Ok(out)
}

/// Display name of the station a recipe needs; `None` when it needs none.
pub fn recipe_station_name(id: RecipeId) -> Option<&'static str> {
    let st = recipe(id)?.station?;
    Some(match st {
        WORKBENCH => "Workbench",
        FURNACE => "Furnace",
        ANVIL => "Iron Anvil",
        BOTTLE => "Bottle",
        DEMON_ALTAR => "Demon Altar",
        HELLFORGE => "Hellforge",
        TINKERERS_WORKSHOP => "Tinkerer's Workshop",
        _ => "Station",
    })
}

/// Player.DefaultTileRangeX/Y, gameplay spec,2477. The server check and the
/// client panel probe both go through `station_in_reach`, so they share these.
pub const CRAFT_RANGE_X: i32 = 5;
pub const CRAFT_RANGE_Y: i32 = 3;

/// True when `station` is within the tile range of the feet block. Terraria
/// scans the body rectangle (`Player.AdjTiles`, gameplay spec); the
/// feet block is a documented simplification.
pub fn station_in_reach(q: &impl BlockQuery, feet: [f32; 3], station: BlockId) -> bool {
    if station == AIR {
        return false;
    }
    let (fx, fy, fz) = (
        feet[0].floor() as i32,
        feet[1].floor() as i32,
        feet[2].floor() as i32,
    );
    for x in fx - CRAFT_RANGE_X..=fx + CRAFT_RANGE_X {
        for y in fy - CRAFT_RANGE_Y..=fy + CRAFT_RANGE_Y {
            for z in fz - CRAFT_RANGE_X..=fz + CRAFT_RANGE_X {
                let b = q.block(x, y, z);
                if b == station || (station == FURNACE && b == HELLFORGE) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{DEMON_ALTAR, STONE};
    use crate::item::STONE_BLOCK;
    use std::collections::HashMap;

    struct Q(HashMap<[i32; 3], BlockId>);
    impl BlockQuery for Q {
        fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
            self.0.get(&[x, y, z]).copied().unwrap_or(AIR)
        }
    }
    fn q_with(b: BlockId, at: [i32; 3]) -> Q {
        Q(HashMap::from([(at, b)]))
    }

    fn st(id: ItemId, count: u32) -> ItemStack {
        ItemStack {
            id,
            count,
            prefix: Prefix(0),
        }
    }

    #[test]
    fn furniture_recipes_match_spec() {
        let chk = |id, out: Ingredient, ings: &[Ingredient], st| {
            let r = recipe(id).unwrap();
            assert_eq!((r.output, r.ingredients, r.station), (out, ings, st));
        };
        let wb = Some(WORKBENCH);
        chk(WOODEN_DOOR_RECIPE, ing(WOODEN_DOOR, 1), &[ing(WOOD, 6)], wb); // gameplay spec
        chk(
            WOODEN_CHAIR_RECIPE,
            ing(WOODEN_CHAIR, 1),
            &[ing(WOOD, 4)],
            wb,
        ); // :4727
        chk(
            WOODEN_TABLE_RECIPE,
            ing(WOODEN_TABLE, 1),
            &[ing(WOOD, 8)],
            wb,
        ); // :4746
        chk(
            WOOD_PLATFORM_RECIPE,
            ing(WOOD_PLATFORM, 2),
            &[ing(WOOD, 1)],
            None,
        ); // :2376
    }

    #[test]
    fn arrow_recipes_match_spec() {
        let chk = |id, out: Ingredient, ings: &[Ingredient], st| {
            let r = recipe(id).unwrap();
            assert_eq!((r.output, r.ingredients, r.station), (out, ings, st));
        };
        chk(
            WOODEN_BOW_RECIPE,
            ing(WOODEN_BOW, 1),
            &[ing(WOOD, 10)],
            Some(WORKBENCH),
        ); // :6810
        chk(
            WOODEN_ARROW_RECIPE,
            ing(WOODEN_ARROW, 25),
            &[ing(WOOD, 1), ing(STONE_BLOCK, 1)],
            Some(WORKBENCH),
        ); // :2077
        chk(
            FLAMING_ARROW_RECIPE,
            ing(FLAMING_ARROW, 10),
            &[ing(WOODEN_ARROW, 10), ing(TORCH, 1)],
            None,
        ); // :2084
    }

    #[test]
    fn glass_recipe_needs_furnace() {
        // gameplay spec
        let r = recipe(GLASS_RECIPE).unwrap();
        assert_eq!(r.output, ing(GLASS, 1));
        assert_eq!(r.ingredients, &[ing(SAND_BLOCK, 2)]);
        assert_eq!(r.station, Some(FURNACE));
    }

    #[test]
    fn bottle_recipe_matches_spec() {
        // gameplay spec: createItem 31 stack 2, 1 Glass, requiredTile 17
        let r = recipe(BOTTLE_RECIPE).unwrap();
        assert_eq!(r.output, ing(BOTTLE_ITEM, 2));
        assert_eq!(r.ingredients, &[ing(GLASS, 1)]);
        assert_eq!(r.station, Some(FURNACE));
    }

    #[test]
    fn lesser_healing_ids_and_recipe_match_spec() {
        assert_eq!(LESSER_HEALING_POTION.0, 28); // gameplay spec
        let r = recipe(LESSER_HEALING_RECIPE).unwrap();
        assert_eq!(r.output, ing(LESSER_HEALING_POTION, 2));
        assert_eq!(
            r.ingredients,
            &[ing(MUSHROOM, 1), ing(GEL, 2), ing(BOTTLE_ITEM, 2)]
        );
        assert_eq!(r.station, Some(BOTTLE)); // requiredTile 13
    }

    #[test]
    fn eye_recipe_id_unchanged() {
        // gameplay spec: createItem 43 stack 1 from 6 Lens at
        // requiredTile 26 (Demon Altar); there is no workbench version. The
        // station field changed but RecipeId 44 must not be renumbered.
        assert_eq!(SUSPICIOUS_EYE_RECIPE, RecipeId(44));
        let r = recipe(SUSPICIOUS_EYE_RECIPE).unwrap();
        assert_eq!(r.id, SUSPICIOUS_EYE_RECIPE);
        assert_eq!(r.output, ing(SUSPICIOUS_LOOKING_EYE, 1));
        assert_eq!(r.ingredients, &[ing(LENS, 6)]);
        assert_eq!(r.station, Some(DEMON_ALTAR));
    }

    #[test]
    fn eye_recipe_needs_altar_not_workbench() {
        // The server's craft step (server/src/lib.rs): UnknownRecipe,
        // MissingStation from station_in_reach, else craft.
        let feet = [0.5, 10.0, 0.5];
        // In reach only with a demon altar within the tile range.
        assert!(station_in_reach(
            &q_with(DEMON_ALTAR, [5, 10, 0]),
            feet,
            DEMON_ALTAR
        ));
        assert!(!station_in_reach(
            &q_with(DEMON_ALTAR, [6, 10, 0]),
            feet,
            DEMON_ALTAR
        ));
        assert!(!station_in_reach(&Q(HashMap::new()), feet, DEMON_ALTAR));
        // A loaded workbench next to the player is not an altar.
        let wb_only = q_with(WORKBENCH, [1, 10, 0]);
        assert!(!station_in_reach(&wb_only, feet, DEMON_ALTAR));

        let mut inv = Inventory::empty();
        inv.add(st(LENS, 6));
        let res = match recipe(SUSPICIOUS_EYE_RECIPE) {
            None => Err(CraftError::UnknownRecipe),
            Some(r)
                if r.station
                    .is_some_and(|st| !station_in_reach(&wb_only, feet, st)) =>
            {
                Err(CraftError::MissingStation)
            }
            Some(_) => craft(&mut inv, SUSPICIOUS_EYE_RECIPE, 1),
        };
        assert_eq!(res, Err(CraftError::MissingStation));
        assert_eq!(inv.count_of(LENS), 6); // refused before any ingredient moved

        // The same path with an altar in reach creates the eye.
        let mut inv = Inventory::empty();
        inv.add(st(LENS, 6));
        let altar_near = q_with(DEMON_ALTAR, [5, 10, 0]);
        let res = match recipe(SUSPICIOUS_EYE_RECIPE) {
            None => Err(CraftError::UnknownRecipe),
            Some(r)
                if r.station
                    .is_some_and(|st| !station_in_reach(&altar_near, feet, st)) =>
            {
                Err(CraftError::MissingStation)
            }
            Some(_) => craft(&mut inv, SUSPICIOUS_EYE_RECIPE, 1),
        };
        assert_eq!(res, Ok(1));
        assert_eq!(inv.count_of(SUSPICIOUS_LOOKING_EYE), 1);
        assert_eq!(inv.count_of(LENS), 0);
    }

    #[test]
    fn altar_station_name() {
        // The craft panel shows the required tile by name (Recipe.needTile).
        assert_eq!(
            recipe_station_name(SUSPICIOUS_EYE_RECIPE),
            Some("Demon Altar")
        );
    }

    #[test]
    fn craft_consumes_and_yields() {
        let mut inv = Inventory::empty();
        inv.add(st(GEL, 1));
        inv.add(st(WOOD, 1));
        assert_eq!(craft(&mut inv, TORCH_RECIPE, 1), Ok(3));
        assert_eq!(inv.count_of(TORCH), 3);
        assert_eq!(inv.count_of(GEL) + inv.count_of(WOOD), 0);
    }

    #[test]
    fn craft_count_above_available_is_atomic() {
        let mut inv = Inventory::empty();
        inv.add(st(GEL, 1));
        inv.add(st(WOOD, 3));
        let before = inv.clone();
        assert_eq!(
            craft(&mut inv, TORCH_RECIPE, 2),
            Err(CraftError::MissingIngredients)
        );
        assert_eq!(inv, before);
    }

    #[test]
    fn craft_stacks_output_into_existing_stack() {
        let mut inv = Inventory::empty();
        inv.add(st(TORCH, 2));
        inv.add(st(GEL, 1));
        inv.add(st(WOOD, 1));
        assert_eq!(craft(&mut inv, TORCH_RECIPE, 1), Ok(3));
        assert_eq!(inv.count_of(TORCH), 5);
        assert_eq!(inv.slots.iter().flatten().count(), 1);
    }

    #[test]
    fn full_inventory_is_not_consumed() {
        let mut inv = Inventory::empty();
        for s in inv.slots.iter_mut() {
            *s = Some(st(STONE_BLOCK, 9999));
        }
        inv.slots[0] = Some(st(WOOD, 10));
        inv.slots[1] = Some(st(GEL, 2));
        let before = inv.clone();
        assert_eq!(
            craft(&mut inv, TORCH_RECIPE, 1),
            Err(CraftError::InventoryFull)
        );
        assert_eq!(inv, before);
    }

    #[test]
    fn craft_rejects_unknown_and_zero() {
        let mut inv = Inventory::empty();
        assert_eq!(
            craft(&mut inv, RecipeId(99), 1),
            Err(CraftError::UnknownRecipe)
        );
        assert_eq!(
            craft(&mut inv, TORCH_RECIPE, 0),
            Err(CraftError::InvalidCount)
        );
    }

    #[test]
    fn recipe_lookup_by_id() {
        for i in 0..83 {
            assert_eq!(recipe(RecipeId(i)).unwrap().id, RecipeId(i));
        }
        assert!(recipe(RecipeId(99)).is_none());
        assert_eq!(recipes().len(), 83);
    }

    #[test]
    fn station_names_match_table() {
        assert_eq!(recipe_station_name(WORKBENCH_RECIPE), None);
        assert_eq!(recipe_station_name(WOODEN_SWORD_RECIPE), Some("Workbench"));
        assert_eq!(recipe_station_name(COPPER_BAR_RECIPE), Some("Furnace"));
        assert_eq!(
            recipe_station_name(COPPER_PICKAXE_RECIPE),
            Some("Iron Anvil")
        );
        assert!(
            recipes()
                .iter()
                .all(|r| r.station.is_none() == recipe_station_name(r.id).is_none())
        );
    }

    #[test]
    fn station_reach_matches_tile_range() {
        let feet = [0.5, 10.0, 0.5];
        for (at, ok) in [
            ([5, 10, 0], true),
            ([6, 10, 0], false),
            ([0, 13, 0], true),
            ([0, 14, 0], false),
        ] {
            assert_eq!(
                station_in_reach(&q_with(WORKBENCH, at), feet, WORKBENCH),
                ok,
                "{at:?}"
            );
        }
    }

    #[test]
    fn unloaded_or_other_blocks_are_not_stations() {
        let feet = [0.5, 10.0, 0.5];
        assert!(!station_in_reach(&Q(HashMap::new()), feet, WORKBENCH));
        assert!(!station_in_reach(
            &q_with(STONE, [1, 10, 0]),
            feet,
            WORKBENCH
        ));
    }

    #[test]
    fn wall_recipe_matches_spec() {
        let r = recipe(WOOD_WALL_RECIPE).unwrap();
        assert_eq!(r.output, ing(WOOD_WALL, 4));
        assert_eq!(r.ingredients, &[ing(WOOD, 1)]);
        assert_eq!(r.station, Some(WORKBENCH));
    }

    #[test]
    fn recipes_match_spec() {
        let r = &RECIPES;
        assert_eq!(r[0].id, WORKBENCH_RECIPE);
        assert_eq!(r[0].output, ing(WORKBENCH_ITEM, 1));
        assert_eq!(r[0].ingredients, &[ing(WOOD, 10)]);
        assert_eq!(r[0].station, None);
        assert_eq!(r[1].id, TORCH_RECIPE);
        assert_eq!(r[1].output, ing(TORCH, 3));
        assert_eq!(r[1].ingredients, &[ing(GEL, 1), ing(WOOD, 1)]);
        assert_eq!(r[1].station, None);
        assert_eq!(r[2].id, WOODEN_SWORD_RECIPE);
        assert_eq!(r[2].output, ing(WOODEN_SWORD, 1));
        assert_eq!(r[2].ingredients, &[ing(WOOD, 7)]);
        assert_eq!(r[2].station, Some(WORKBENCH));
    }

    #[test]
    fn chest_recipe_matches_spec() {
        // gameplay spec: 8 Wood + 2 Iron Bar (IronBar group) at tile 18
        let r = recipe(CHEST_RECIPE).unwrap();
        assert_eq!(r.output, ing(CHEST_ITEM, 1));
        assert_eq!(r.ingredients, &[ing(WOOD, 8), ing(IRON_BAR, 2)]);
        assert_eq!(r.station, Some(WORKBENCH));
    }

    #[test]
    fn bar_recipes_match_spec() {
        // gameplay spec (copper 3), :9817 (iron 3), :10076 (silver 4), :10236 (gold 4), :10572 (demonite 3); tile 17
        for (r, out, ore, n) in [
            (COPPER_BAR_RECIPE, COPPER_BAR, COPPER_ORE, 3),
            (IRON_BAR_RECIPE, IRON_BAR, IRON_ORE, 3),
            (SILVER_BAR_RECIPE, SILVER_BAR, SILVER_ORE, 4),
            (GOLD_BAR_RECIPE, GOLD_BAR, GOLD_ORE, 4),
            (DEMONITE_BAR_RECIPE, DEMONITE_BAR, DEMONITE_ORE_ITEM, 3),
        ] {
            let r = recipe(r).unwrap();
            assert_eq!(r.output, ing(out, 1));
            assert_eq!(r.ingredients, &[ing(ore, n)]);
            assert_eq!(r.station, Some(FURNACE));
        }
    }

    #[test]
    fn furnace_recipe_matches_spec() {
        // gameplay spec
        let r = recipe(FURNACE_RECIPE).unwrap();
        assert_eq!(r.output, ing(FURNACE_ITEM, 1));
        assert_eq!(
            r.ingredients,
            &[ing(STONE_BLOCK, 20), ing(WOOD, 4), ing(TORCH, 3)]
        );
        assert_eq!(r.station, Some(WORKBENCH));
    }

    #[test]
    fn anvil_recipe_matches_spec() {
        // gameplay spec
        let r = recipe(ANVIL_RECIPE).unwrap();
        assert_eq!(r.output, ing(ANVIL_ITEM, 1));
        assert_eq!(r.ingredients, &[ing(IRON_BAR, 5)]);
        assert_eq!(r.station, Some(WORKBENCH));
    }

    #[test]
    fn bar_craft_needs_furnace_within_reach() {
        let feet = [0.5, 10.0, 0.5];
        assert!(station_in_reach(
            &q_with(FURNACE, [5, 13, 0]),
            feet,
            FURNACE
        ));
        assert!(!station_in_reach(
            &q_with(FURNACE, [6, 10, 0]),
            feet,
            FURNACE
        ));
        // A workbench does not stand in for a furnace.
        assert!(!station_in_reach(
            &q_with(WORKBENCH, [1, 10, 0]),
            feet,
            FURNACE
        ));
    }

    #[test]
    fn tool_recipes_match_spec() {
        for (r, out, bar, bars, wood) in [
            (COPPER_PICKAXE_RECIPE, COPPER_PICKAXE, COPPER_BAR, 8, 4), // gameplay spec
            (COPPER_AXE_RECIPE, COPPER_AXE, COPPER_BAR, 6, 3),         // gameplay spec
            (
                COPPER_BROADSWORD_RECIPE,
                COPPER_BROADSWORD,
                COPPER_BAR,
                6,
                0,
            ), // gameplay spec
            (IRON_PICKAXE_RECIPE, IRON_PICKAXE, IRON_BAR, 10, 3),      // gameplay spec
            (IRON_AXE_RECIPE, IRON_AXE, IRON_BAR, 8, 3),               // gameplay spec
            (IRON_BROADSWORD_RECIPE, IRON_BROADSWORD, IRON_BAR, 8, 0), // gameplay spec
            (SILVER_PICKAXE_RECIPE, SILVER_PICKAXE, SILVER_BAR, 10, 4), // gameplay spec
            (SILVER_AXE_RECIPE, SILVER_AXE, SILVER_BAR, 8, 3),         // gameplay spec
            (
                SILVER_BROADSWORD_RECIPE,
                SILVER_BROADSWORD,
                SILVER_BAR,
                8,
                0,
            ), // gameplay spec
            (GOLD_PICKAXE_RECIPE, GOLD_PICKAXE, GOLD_BAR, 10, 4),      // gameplay spec
            (GOLD_AXE_RECIPE, GOLD_AXE, GOLD_BAR, 8, 3),               // gameplay spec
            (GOLD_BROADSWORD_RECIPE, GOLD_BROADSWORD, GOLD_BAR, 8, 0), // gameplay spec
        ] {
            let r = recipe(r).unwrap();
            assert_eq!(r.output, ing(out, 1));
            let mut want = vec![ing(bar, bars)];
            if wood > 0 {
                want.push(ing(WOOD, wood));
            }
            assert_eq!(r.ingredients, &want[..]);
            assert_eq!(r.station, Some(ANVIL));
        }
    }

    #[test]
    fn armor_recipes_match_spec() {
        for (r, out, bar, bars) in [
            (COPPER_HELMET_RECIPE, COPPER_HELMET, COPPER_BAR, 12), // gameplay spec
            (COPPER_CHAINMAIL_RECIPE, COPPER_CHAINMAIL, COPPER_BAR, 20), // gameplay spec
            (COPPER_GREAVES_RECIPE, COPPER_GREAVES, COPPER_BAR, 16), // gameplay spec
            (IRON_HELMET_RECIPE, IRON_HELMET, IRON_BAR, 15),       // gameplay spec
            (IRON_CHAINMAIL_RECIPE, IRON_CHAINMAIL, IRON_BAR, 25), // gameplay spec
            (IRON_GREAVES_RECIPE, IRON_GREAVES, IRON_BAR, 20),     // gameplay spec
            (SILVER_HELMET_RECIPE, SILVER_HELMET, SILVER_BAR, 15), // gameplay spec
            (SILVER_CHAINMAIL_RECIPE, SILVER_CHAINMAIL, SILVER_BAR, 25), // gameplay spec
            (SILVER_GREAVES_RECIPE, SILVER_GREAVES, SILVER_BAR, 20), // gameplay spec
            (GOLD_HELMET_RECIPE, GOLD_HELMET, GOLD_BAR, 20),       // gameplay spec
            (GOLD_CHAINMAIL_RECIPE, GOLD_CHAINMAIL, GOLD_BAR, 30), // gameplay spec
            (GOLD_GREAVES_RECIPE, GOLD_GREAVES, GOLD_BAR, 25),     // gameplay spec
        ] {
            let r = recipe(r).unwrap();
            assert_eq!(r.output, ing(out, 1));
            assert_eq!(r.ingredients, &[ing(bar, bars)]);
            assert_eq!(r.station, Some(ANVIL));
        }
    }

    #[test]
    fn empty_bucket_recipe_needs_anvil_and_three_iron_bars() {
        let r = recipe(EMPTY_BUCKET_RECIPE).expect("empty bucket recipe registered");
        assert_eq!(r.output, ing(EMPTY_BUCKET, 1));
        assert_eq!(r.ingredients, &[ing(IRON_BAR, 3)]);
        assert_eq!(r.station, Some(ANVIL));
    }

    #[test]
    fn mana_crystal_recipe_matches_spec() {
        // gameplay spec: 5 Fallen Stars -> 1 Mana Crystal, no station.
        let r = recipe(MANA_CRYSTAL_RECIPE).expect("mana crystal recipe registered");
        assert_eq!(r.output, ing(MANA_CRYSTAL, 1));
        assert_eq!(r.ingredients, &[ing(FALLEN_STAR, 5)]);
        assert_eq!(r.station, None);
        assert_eq!(recipes().len(), 83);
    }

    #[test]
    fn hammer_recipes_match_spec() {
        // Wooden Hammer: 8 Wood at Workbench (gameplay spec)
        let wh = recipe(WOODEN_HAMMER_RECIPE).expect("wooden hammer recipe");
        assert_eq!(wh.output, ing(WOODEN_HAMMER, 1));
        assert_eq!(wh.ingredients, &[ing(WOOD, 8)]);
        assert_eq!(wh.station, Some(WORKBENCH));

        // The Breaker: 10 Demonite Bars at Anvil (gameplay spec)
        let tb = recipe(THE_BREAKER_RECIPE).expect("the breaker recipe");
        assert_eq!(tb.output, ing(THE_BREAKER, 1));
        assert_eq!(tb.ingredients, &[ing(DEMONITE_BAR, 10)]);
        assert_eq!(tb.station, Some(ANVIL));
    }

    #[test]
    fn full_shadow_gear_crafting_chain() {
        use crate::block::{ANVIL, FURNACE};
        use crate::item::{
            DEMON_BOW, DEMONITE_BAR, DEMONITE_ORE_ITEM, LIGHTS_BANE, NIGHTMARE_PICKAXE,
            SHADOW_GREAVES, SHADOW_HELMET, SHADOW_SCALE, SHADOW_SCALEMAIL, THE_BREAKER,
            WAR_AXE_OF_THE_NIGHT,
        };
        use std::collections::HashMap;

        // Verify recipe definitions
        let r_bar = recipe(DEMONITE_BAR_RECIPE).expect("Demonite Bar recipe");
        assert_eq!(r_bar.output, ing(DEMONITE_BAR, 1));
        assert_eq!(r_bar.ingredients, &[ing(DEMONITE_ORE_ITEM, 3)]);
        assert_eq!(r_bar.station, Some(FURNACE));

        let r_axe = recipe(WAR_AXE_OF_THE_NIGHT_RECIPE).expect("War Axe of the Night recipe");
        assert_eq!(r_axe.output, ing(WAR_AXE_OF_THE_NIGHT, 1));
        assert_eq!(r_axe.ingredients, &[ing(DEMONITE_BAR, 10)]);
        assert_eq!(r_axe.station, Some(ANVIL));

        let r_sword = recipe(LIGHTS_BANE_RECIPE).expect("Light's Bane recipe");
        assert_eq!(r_sword.output, ing(LIGHTS_BANE, 1));
        assert_eq!(r_sword.ingredients, &[ing(DEMONITE_BAR, 10)]);
        assert_eq!(r_sword.station, Some(ANVIL));

        let r_bow = recipe(DEMON_BOW_RECIPE).expect("Demon Bow recipe");
        assert_eq!(r_bow.output, ing(DEMON_BOW, 1));
        assert_eq!(r_bow.ingredients, &[ing(DEMONITE_BAR, 8)]);
        assert_eq!(r_bow.station, Some(ANVIL));

        let r_hammer = recipe(THE_BREAKER_RECIPE).expect("The Breaker recipe");
        assert_eq!(r_hammer.output, ing(THE_BREAKER, 1));
        assert_eq!(r_hammer.ingredients, &[ing(DEMONITE_BAR, 10)]);
        assert_eq!(r_hammer.station, Some(ANVIL));

        let r_pick = recipe(NIGHTMARE_PICKAXE_RECIPE).expect("Nightmare Pickaxe recipe");
        assert_eq!(r_pick.output, ing(NIGHTMARE_PICKAXE, 1));
        assert_eq!(
            r_pick.ingredients,
            &[ing(DEMONITE_BAR, 12), ing(SHADOW_SCALE, 6)]
        );
        assert_eq!(r_pick.station, Some(ANVIL));

        let r_helm = recipe(SHADOW_HELMET_RECIPE).expect("Shadow Helmet recipe");
        assert_eq!(r_helm.output, ing(SHADOW_HELMET, 1));
        assert_eq!(
            r_helm.ingredients,
            &[ing(DEMONITE_BAR, 15), ing(SHADOW_SCALE, 10)]
        );
        assert_eq!(r_helm.station, Some(ANVIL));

        let r_chest = recipe(SHADOW_SCALEMAIL_RECIPE).expect("Shadow Scalemail recipe");
        assert_eq!(r_chest.output, ing(SHADOW_SCALEMAIL, 1));
        assert_eq!(
            r_chest.ingredients,
            &[ing(DEMONITE_BAR, 25), ing(SHADOW_SCALE, 20)]
        );
        assert_eq!(r_chest.station, Some(ANVIL));

        let r_legs = recipe(SHADOW_GREAVES_RECIPE).expect("Shadow Greaves recipe");
        assert_eq!(r_legs.output, ing(SHADOW_GREAVES, 1));
        assert_eq!(
            r_legs.ingredients,
            &[ing(DEMONITE_BAR, 20), ing(SHADOW_SCALE, 15)]
        );
        assert_eq!(r_legs.station, Some(ANVIL));

        // Now execute the full crafting chain from raw drops:
        // Total bars required: 10 + 10 + 8 + 10 + 12 + 15 + 25 + 20 = 110 bars.
        // Total ore required: 110 * 3 = 330 Demonite Ore.
        // Total scales required: 6 + 10 + 20 + 15 = 51 Shadow Scales.
        let mut inv = Inventory::empty();
        inv.add(st(DEMONITE_ORE_ITEM, 330));
        inv.add(st(SHADOW_SCALE, 51));

        let mut stations = HashMap::new();
        stations.insert([1, 0, 0], FURNACE);
        stations.insert([2, 0, 0], ANVIL);
        let q = Q(stations);
        let feet = [0.5, 0.0, 0.5];

        // Smelt 110 bars at Furnace
        assert!(station_in_reach(&q, feet, FURNACE));
        let smelted = craft(&mut inv, DEMONITE_BAR_RECIPE, 110).unwrap();
        assert_eq!(smelted, 110);
        assert_eq!(inv.count_of(DEMONITE_ORE_ITEM), 0);
        assert_eq!(inv.count_of(DEMONITE_BAR), 110);

        // Craft all weapons, tools and armor at Anvil
        assert!(station_in_reach(&q, feet, ANVIL));
        assert_eq!(craft(&mut inv, WAR_AXE_OF_THE_NIGHT_RECIPE, 1).unwrap(), 1);
        assert_eq!(craft(&mut inv, LIGHTS_BANE_RECIPE, 1).unwrap(), 1);
        assert_eq!(craft(&mut inv, DEMON_BOW_RECIPE, 1).unwrap(), 1);
        assert_eq!(craft(&mut inv, THE_BREAKER_RECIPE, 1).unwrap(), 1);
        assert_eq!(craft(&mut inv, NIGHTMARE_PICKAXE_RECIPE, 1).unwrap(), 1);
        assert_eq!(craft(&mut inv, SHADOW_HELMET_RECIPE, 1).unwrap(), 1);
        assert_eq!(craft(&mut inv, SHADOW_SCALEMAIL_RECIPE, 1).unwrap(), 1);
        assert_eq!(craft(&mut inv, SHADOW_GREAVES_RECIPE, 1).unwrap(), 1);

        assert_eq!(inv.count_of(DEMONITE_BAR), 0);
        assert_eq!(inv.count_of(SHADOW_SCALE), 0);
        assert_eq!(inv.count_of(WAR_AXE_OF_THE_NIGHT), 1);
        assert_eq!(inv.count_of(LIGHTS_BANE), 1);
        assert_eq!(inv.count_of(DEMON_BOW), 1);
        assert_eq!(inv.count_of(THE_BREAKER), 1);
        assert_eq!(inv.count_of(NIGHTMARE_PICKAXE), 1);
        assert_eq!(inv.count_of(SHADOW_HELMET), 1);
        assert_eq!(inv.count_of(SHADOW_SCALEMAIL), 1);
        assert_eq!(inv.count_of(SHADOW_GREAVES), 1);
    }

    #[test]
    fn is_material_identifies_recipe_ingredients() {
        assert!(is_material(WOOD));
        assert!(!is_material(COPPER_PICKAXE));
    }

    #[test]
    fn molten_recipes_match_wiki() {
        assert_eq!(recipe_station_name(HELLSTONE_BAR_RECIPE), Some("Hellforge"));
        let bar = recipes()
            .iter()
            .find(|r| r.output.item == HELLSTONE_BAR)
            .unwrap();
        assert_eq!(
            bar.ingredients,
            &[
                Ingredient {
                    item: HELLSTONE_ITEM,
                    count: 3
                },
                Ingredient {
                    item: OBSIDIAN_ITEM,
                    count: 1
                }
            ]
        );
        assert_eq!(bar.station, Some(HELLFORGE));
        assert_eq!(
            recipes()
                .iter()
                .find(|r| r.output.item == MOLTEN_HELMET)
                .unwrap()
                .ingredients[0],
            ing(HELLSTONE_BAR, 10)
        );
        assert_eq!(
            recipes()
                .iter()
                .find(|r| r.output.item == MOLTEN_BREASTPLATE)
                .unwrap()
                .ingredients[0],
            ing(HELLSTONE_BAR, 20)
        );
        assert_eq!(
            recipes()
                .iter()
                .find(|r| r.output.item == MOLTEN_GREAVES)
                .unwrap()
                .ingredients[0],
            ing(HELLSTONE_BAR, 15)
        );
        assert_eq!(
            recipes()
                .iter()
                .find(|r| r.output.item == MOLTEN_PICKAXE)
                .unwrap()
                .ingredients[0],
            ing(HELLSTONE_BAR, 20)
        );
        assert_eq!(
            recipes()
                .iter()
                .find(|r| r.output.item == MOLTEN_HAMAXE)
                .unwrap()
                .ingredients[0],
            ing(HELLSTONE_BAR, 15)
        );
        assert_eq!(
            recipes()
                .iter()
                .find(|r| r.output.item == FIERY_GREATSWORD)
                .unwrap()
                .ingredients[0],
            ing(HELLSTONE_BAR, 20)
        );
        assert_eq!(
            recipes()
                .iter()
                .find(|r| r.output.item == MOLTEN_FURY)
                .unwrap()
                .ingredients[0],
            ing(HELLSTONE_BAR, 15)
        );
        let phoenix = recipes()
            .iter()
            .find(|r| r.output.item == PHOENIX_BLASTER)
            .unwrap();
        assert_eq!(
            (
                phoenix.ingredients[0].item,
                phoenix.ingredients[0].count,
                phoenix.ingredients[1].count
            ),
            (HANDGUN, 1, 10)
        );
        let flam = recipes()
            .iter()
            .find(|r| r.output.item == FLAMARANG)
            .unwrap();
        assert_eq!(
            (
                flam.ingredients[0].item,
                flam.ingredients[0].count,
                flam.ingredients[1].count
            ),
            (ENCHANTED_BOOMERANG, 1, 15)
        );

        // Every other Molten recipe is an Anvil recipe.
        for id in [
            MOLTEN_HELMET_RECIPE,
            MOLTEN_BREASTPLATE_RECIPE,
            MOLTEN_GREAVES_RECIPE,
            MOLTEN_PICKAXE_RECIPE,
            MOLTEN_HAMAXE_RECIPE,
            FIERY_GREATSWORD_RECIPE,
            MOLTEN_FURY_RECIPE,
            PHOENIX_BLASTER_RECIPE,
            FLAMARANG_RECIPE,
        ] {
            assert_eq!(recipe(id).unwrap().station, Some(ANVIL), "{id:?}");
            assert_eq!(recipe_station_name(id), Some("Iron Anvil"), "{id:?}");
        }

        // A Hellstone Bar needs a Hellforge in reach, never a Furnace.
        let feet = [0.5, 10.0, 0.5];
        let at_hellforge = q_with(HELLFORGE, [5, 10, 0]);
        let at_furnace = q_with(FURNACE, [5, 10, 0]);
        assert!(station_in_reach(&at_hellforge, feet, HELLFORGE));
        assert!(!station_in_reach(&at_furnace, feet, HELLFORGE));

        let station_check = |q: &Q, inv: &mut Inventory| match recipe(HELLSTONE_BAR_RECIPE) {
            None => Err(CraftError::UnknownRecipe),
            Some(r) if r.station.is_some_and(|st| !station_in_reach(q, feet, st)) => {
                Err(CraftError::MissingStation)
            }
            Some(_) => craft(inv, HELLSTONE_BAR_RECIPE, 1),
        };

        let mut inv = Inventory::empty();
        inv.add(st(HELLSTONE_ITEM, 3));
        inv.add(st(OBSIDIAN_ITEM, 1));
        assert_eq!(
            station_check(&at_furnace, &mut inv),
            Err(CraftError::MissingStation)
        );
        assert_eq!(inv.count_of(HELLSTONE_ITEM), 3);
        assert_eq!(inv.count_of(OBSIDIAN_ITEM), 1);

        assert_eq!(station_check(&at_hellforge, &mut inv), Ok(1));
        assert_eq!(inv.count_of(HELLSTONE_BAR), 1);
        assert_eq!(inv.count_of(HELLSTONE_ITEM), 0);
        assert_eq!(inv.count_of(OBSIDIAN_ITEM), 0);
    }

    #[test]
    fn obsidian_skull_bricks_and_worm_food_are_craftable() {
        let mut inv = Inventory::empty();
        inv.add(st(OBSIDIAN_ITEM, 22));
        inv.add(st(STONE_BLOCK, 1));
        inv.add(st(HELLSTONE_ITEM, 1));
        inv.add(st(SHADOW_SCALE, 15));
        inv.add(st(EBONSTONE_BLOCK, 15));

        assert_eq!(
            recipe(OBSIDIAN_SKULL_RECIPE).unwrap().station,
            Some(FURNACE)
        );
        assert_eq!(craft(&mut inv, OBSIDIAN_SKULL_RECIPE, 1), Ok(1));
        assert_eq!(inv.count_of(OBSIDIAN_SKULL), 1);
        assert_eq!(inv.count_of(OBSIDIAN_ITEM), 2);

        assert_eq!(
            recipe(OBSIDIAN_BRICK_RECIPE).unwrap().station,
            Some(FURNACE)
        );
        assert_eq!(craft(&mut inv, OBSIDIAN_BRICK_RECIPE, 1), Ok(1));
        assert_eq!(inv.count_of(OBSIDIAN_BRICK_ITEM), 1);
        assert_eq!(inv.count_of(OBSIDIAN_ITEM), 0);
        assert_eq!(recipe_station_name(LESSER_HEALING_RECIPE), Some("Bottle"));
        struct Hf;
        impl BlockQuery for Hf {
            fn block(&self, _x: i32, _y: i32, _z: i32) -> BlockId {
                HELLFORGE
            }
        }
        assert!(station_in_reach(&Hf, [0.0, 0.0, 0.0], FURNACE));

        assert_eq!(
            recipe(HELLSTONE_BRICK_RECIPE).unwrap().station,
            Some(FURNACE)
        );
        assert_eq!(craft(&mut inv, HELLSTONE_BRICK_RECIPE, 1), Ok(1));
        assert_eq!(inv.count_of(HELLSTONE_BRICK_ITEM), 1);

        assert_eq!(recipe(WORM_FOOD_RECIPE).unwrap().station, Some(DEMON_ALTAR));
        assert_eq!(craft(&mut inv, WORM_FOOD_RECIPE, 1), Ok(1));
        assert_eq!(inv.count_of(WORM_FOOD), 1);
    }

    #[test]
    fn recipes_using_material_returns_expected_recipes() {
        let wood_recipes = recipes_using_material(WOOD);
        assert!(wood_recipes.iter().any(|r| r.id == WORKBENCH_RECIPE));
        assert!(wood_recipes.iter().any(|r| r.id == TORCH_RECIPE));
        assert!(wood_recipes.iter().any(|r| r.id == WOODEN_SWORD_RECIPE));

        let iron_recipes = recipes_using_material(IRON_BAR);
        assert!(iron_recipes.iter().any(|r| r.id == ANVIL_RECIPE));
        assert!(iron_recipes.iter().any(|r| r.id == IRON_PICKAXE_RECIPE));
        assert!(iron_recipes.iter().any(|r| r.id == CHEST_RECIPE));

        let hellstone_recipes = recipes_using_material(HELLSTONE_BAR);
        assert!(
            hellstone_recipes
                .iter()
                .any(|r| r.id == FIERY_GREATSWORD_RECIPE)
        );

        let lens_recipes = recipes_using_material(LENS);
        assert_eq!(lens_recipes.len(), 1);
        assert_eq!(lens_recipes[0].id, SUSPICIOUS_EYE_RECIPE);

        assert!(recipes_using_material(WOODEN_SWORD).is_empty());
    }
}
