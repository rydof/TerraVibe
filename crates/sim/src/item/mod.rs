//! Item ids and static item data, following classic 1.4-era gameplay specs (`Item.item defaults`).

use crate::block::{
    ANVIL, ASH, BOTTLE, BlockId, CAMPFIRE, CHAIR, CHEST, CLOUD_BLOCK, COPPER, DEMONITE_ORE, DIRT,
    DOOR_CLOSED, EBONSTONE, FURNACE, GLASS_BLOCK, GOLD, HARDENED_SAND, HEART_LANTERN, HELLFORGE,
    HELLSTONE, HELLSTONE_BRICK, HIVE_BLOCK, ICE, IRON, METEORITE_ORE, MUD, OBSIDIAN,
    OBSIDIAN_BRICK, PLATFORM, SAND, SANDSTONE, SILVER, SNOW, STONE, SUNPLATE_BLOCK, TABLE,
    TINKERERS_WORKSHOP, WOOD_BLOCK, WORKBENCH,
};

mod prefix;
pub use prefix::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemId(pub u16);

// Ids from `Terraria/ID/gameplay spec.
pub const COPPER_AXE: ItemId = ItemId(3506);
pub const COPPER_SHORTSWORD: ItemId = ItemId(3507);
pub const COPPER_BROADSWORD: ItemId = ItemId(3508);
pub const COPPER_PICKAXE: ItemId = ItemId(3509);
/// ItemID.LesserHealingPotion (gameplay spec).
pub const LESSER_HEALING_POTION: ItemId = ItemId(28);
/// ItemID.Lens (gameplay spec).
pub const LENS: ItemId = ItemId(38);
/// ItemID.SuspiciousLookingEye (gameplay spec).
pub const SUSPICIOUS_LOOKING_EYE: ItemId = ItemId(43);
pub const GEL: ItemId = ItemId(23);
/// ItemID.Mushroom (gameplay spec). Pickup only here; vanilla also eats it (healLife 15).
pub const MUSHROOM: ItemId = ItemId(5);
/// ItemID.Bottle (gameplay spec).
pub const BOTTLE_ITEM: ItemId = ItemId(31);
/// ItemID.Glass (gameplay spec).
pub const GLASS: ItemId = ItemId(170);
/// gameplay spec
pub const WOODEN_SWORD: ItemId = ItemId(24);
/// gameplay spec
pub const WORKBENCH_ITEM: ItemId = ItemId(36);
// gameplay spec (door), :1610 (table), :1614 (chair), :1734 (platform)
pub const WOODEN_DOOR: ItemId = ItemId(25);
pub const WOODEN_TABLE: ItemId = ItemId(32);
pub const WOODEN_CHAIR: ItemId = ItemId(34);
pub const WOOD_PLATFORM: ItemId = ItemId(94);
// gameplay spec (wood wall; createWall 4 = WallID.Wood, gameplay spec)
pub const WOOD_WALL: ItemId = ItemId(93);
// gameplay spec (bow), :1626 (arrow), :1628 (flaming arrow)
pub const WOODEN_BOW: ItemId = ItemId(39);
pub const WOODEN_ARROW: ItemId = ItemId(40);
pub const FLAMING_ARROW: ItemId = ItemId(41);
/// ItemID.Chest, gameplay spec.
pub const CHEST_ITEM: ItemId = ItemId(48);
// gameplay spec
pub const COPPER_COIN: ItemId = ItemId(71);
pub const SILVER_COIN: ItemId = ItemId(72);
pub const GOLD_COIN: ItemId = ItemId(73);
pub const PLATINUM_COIN: ItemId = ItemId(74);
pub const TORCH: ItemId = ItemId(8);
pub const WOOD: ItemId = ItemId(9);
pub const DIRT_BLOCK: ItemId = ItemId(2);
pub const STONE_BLOCK: ItemId = ItemId(3);
pub const IRON_ORE: ItemId = ItemId(11);
pub const COPPER_ORE: ItemId = ItemId(12);
pub const GOLD_ORE: ItemId = ItemId(13);
pub const SILVER_ORE: ItemId = ItemId(14);
// gameplay spec (bars), :1612 (Furnace), :1616 (Iron Anvil)
pub const GOLD_BAR: ItemId = ItemId(19);
pub const COPPER_BAR: ItemId = ItemId(20);
pub const SILVER_BAR: ItemId = ItemId(21);
pub const IRON_BAR: ItemId = ItemId(22);
pub const FURNACE_ITEM: ItemId = ItemId(33);
pub const ANVIL_ITEM: ItemId = ItemId(35);
pub const LIFE_CRYSTAL_ITEM: ItemId = ItemId(29);
/// ItemID.FallenStar (gameplay spec); also `AmmoID.FallenStar` (not used yet).
pub const FALLEN_STAR: ItemId = ItemId(75);
/// ItemID.ManaCrystal (gameplay spec): +20 max mana up to 200.
pub const MANA_CRYSTAL: ItemId = ItemId(109);
/// ItemID.WandofSparking (gameplay spec).
pub const WAND_OF_SPARKING: ItemId = ItemId(3069);
/// ItemID.Heart (gameplay spec): picked up for 20 life, never stored.
pub const HEART: ItemId = ItemId(58);
// Tiered tools (gameplay spec iron pickaxe, :1554 iron broadsword, :1566 iron axe, :8558-8588 others)
pub const IRON_PICKAXE: ItemId = ItemId(1);
pub const IRON_BROADSWORD: ItemId = ItemId(4);
pub const IRON_AXE: ItemId = ItemId(10);
pub const SILVER_AXE: ItemId = ItemId(3512);
pub const SILVER_BROADSWORD: ItemId = ItemId(3514);
pub const SILVER_PICKAXE: ItemId = ItemId(3515);
pub const GOLD_AXE: ItemId = ItemId(3518);
pub const GOLD_BROADSWORD: ItemId = ItemId(3520);
pub const GOLD_PICKAXE: ItemId = ItemId(3521);
// Armor ids, gameplay spec
pub const COPPER_HELMET: ItemId = ItemId(89);
pub const COPPER_CHAINMAIL: ItemId = ItemId(80);
pub const COPPER_GREAVES: ItemId = ItemId(76);
pub const IRON_HELMET: ItemId = ItemId(90);
pub const IRON_CHAINMAIL: ItemId = ItemId(81);
pub const IRON_GREAVES: ItemId = ItemId(77);
pub const SILVER_HELMET: ItemId = ItemId(91);
pub const SILVER_CHAINMAIL: ItemId = ItemId(82);
pub const SILVER_GREAVES: ItemId = ItemId(78);
pub const GOLD_HELMET: ItemId = ItemId(92);
pub const GOLD_CHAINMAIL: ItemId = ItemId(83);
pub const GOLD_GREAVES: ItemId = ItemId(79);
pub const SAND_BLOCK: ItemId = ItemId(169);
pub const MUD_BLOCK: ItemId = ItemId(176);
pub const SNOW_BLOCK: ItemId = ItemId(593);
pub const ICE_BLOCK: ItemId = ItemId(664);
pub const SANDSTONE_BLOCK: ItemId = ItemId(3271);
pub const HARDENED_SAND_BLOCK: ItemId = ItemId(3272);
/// ItemID.DemoniteOre (gameplay spec); places tile 22 (gameplay spec).
pub const DEMONITE_ORE_ITEM: ItemId = ItemId(56);
/// ItemID.EbonstoneBlock (gameplay spec); places tile 25 (gameplay spec).
pub const EBONSTONE_BLOCK: ItemId = ItemId(61);
/// ItemID.WormFood (gameplay spec); crafted at a Demon Altar and used in
/// the Corruption to summon the Eater of Worlds.
pub const WORM_FOOD: ItemId = ItemId(70);
/// ItemID.ShadowScale (gameplay spec); crafting material (gameplay spec).
pub const SHADOW_SCALE: ItemId = ItemId(86);
/// ItemID.TheBreaker (gameplay spec).
pub const THE_BREAKER: ItemId = ItemId(104);
/// ItemID.WoodenHammer (gameplay spec).
pub const WOODEN_HAMMER: ItemId = ItemId(196);

pub const EMPTY_BUCKET: ItemId = ItemId(205);
pub const WATER_BUCKET: ItemId = ItemId(206);
pub const LAVA_BUCKET: ItemId = ItemId(207);
pub const OBSIDIAN_ITEM: ItemId = ItemId(173);

pub const DEMONITE_BAR: ItemId = ItemId(57);
pub const DEMON_BOW: ItemId = ItemId(44);
pub const WAR_AXE_OF_THE_NIGHT: ItemId = ItemId(45);
pub const LIGHTS_BANE: ItemId = ItemId(46);
pub const SHADOW_GREAVES: ItemId = ItemId(100);
pub const SHADOW_SCALEMAIL: ItemId = ItemId(101);
pub const SHADOW_HELMET: ItemId = ItemId(102);
pub const NIGHTMARE_PICKAXE: ItemId = ItemId(103);

// M6 Accessories and Weapons (gameplay spec, 1646, 1652, 1654, 1656, 1862, 1864, 2094)
pub const BAND_OF_REGENERATION: ItemId = ItemId(49);
pub const MAGIC_MIRROR: ItemId = ItemId(50);
pub const CLOUD_IN_A_BOTTLE: ItemId = ItemId(53);
pub const HERMES_BOOTS: ItemId = ItemId(54);
pub const ENCHANTED_BOOMERANG: ItemId = ItemId(55);
pub const LUCKY_HORSESHOE: ItemId = ItemId(158);
pub const SHINY_RED_BALLOON: ItemId = ItemId(159);
pub const SPEAR: ItemId = ItemId(274);

// M7 Shadow Orb items and weapons (gameplay spec, 1738, 1740, 1768, 1776, 1870)
pub const VILETHORN: ItemId = ItemId(64);
pub const MUSKET: ItemId = ItemId(96);
pub const FLINTLOCK_PISTOL: ItemId = ItemId(95);
pub const MINISHARK: ItemId = ItemId(98);
pub const MUSKET_BALL: ItemId = ItemId(97);
pub const BAND_OF_STARPOWER: ItemId = ItemId(111);
pub const SHADOW_ORB_ITEM: ItemId = ItemId(115);
pub const BALL_O_HURT: ItemId = ItemId(162);

// M8 Dungeon items (gameplay spec, 2198)
pub const MAGIC_MISSILE: ItemId = ItemId(113);
pub const BONE: ItemId = ItemId(154);
pub const MURAMASA: ItemId = ItemId(155);
pub const COBALT_SHIELD: ItemId = ItemId(156);
pub const AQUA_SCEPTER: ItemId = ItemId(157);
pub const BLUE_MOON: ItemId = ItemId(163);
pub const HANDGUN: ItemId = ItemId(164);
pub const GOLDEN_KEY: ItemId = ItemId(327);
pub const SHADOW_KEY: ItemId = ItemId(329);
pub const VALOR: ItemId = ItemId(3317);

// M9 Underworld items (gameplay spec, 1894, 1930, 1974, 1988, 2122, 3358)
pub const ASH_BLOCK: ItemId = ItemId(172);
pub const HELLSTONE_ITEM: ItemId = ItemId(174);
pub const OBSIDIAN_BRICK_ITEM: ItemId = ItemId(192);
pub const OBSIDIAN_SKULL: ItemId = ItemId(193);
pub const HELLSTONE_BRICK_ITEM: ItemId = ItemId(214);
pub const HELLFORGE_ITEM: ItemId = ItemId(221);
pub const OBSIDIAN_SKIN_POTION: ItemId = ItemId(288);
pub const GUIDE_VOODOO_DOLL: ItemId = ItemId(267);
pub const LAVA_CHARM: ItemId = ItemId(906);

// M10 Molten gear (gameplay spec: 119, 120, 121, 122, 175, 217, 219, 231-233)
pub const FLAMARANG: ItemId = ItemId(119);
pub const MOLTEN_FURY: ItemId = ItemId(120);
pub const FIERY_GREATSWORD: ItemId = ItemId(121);
pub const MOLTEN_PICKAXE: ItemId = ItemId(122);
pub const HELLSTONE_BAR: ItemId = ItemId(175);
pub const MOLTEN_HAMAXE: ItemId = ItemId(217);
pub const PHOENIX_BLASTER: ItemId = ItemId(219);
pub const MOLTEN_HELMET: ItemId = ItemId(231);
pub const MOLTEN_BREASTPLATE: ItemId = ItemId(232);
pub const MOLTEN_GREAVES: ItemId = ItemId(233);

// M11 Wall of Flesh drops (gameplay spec: 367, 426, 434, 489, 490, 491, 514, 4912)
pub const PWNHAMMER: ItemId = ItemId(367);
pub const BREAKER_BLADE: ItemId = ItemId(426);
pub const CLOCKWORK_ASSAULT_RIFLE: ItemId = ItemId(434);
pub const SORCERER_EMBLEM: ItemId = ItemId(489);
pub const WARRIOR_EMBLEM: ItemId = ItemId(490);
pub const RANGER_EMBLEM: ItemId = ItemId(491);
pub const LASER_RIFLE: ItemId = ItemId(514);
pub const FIRECRACKER: ItemId = ItemId(4912);
// Critter catching & caught critter items (gameplay spec)
pub const BUG_NET: ItemId = ItemId(1991);
pub const BUNNY_ITEM: ItemId = ItemId(2019);
pub const BIRD_ITEM: ItemId = ItemId(2015);
pub const SQUIRREL_ITEM: ItemId = ItemId(2018);
pub const CAVE_CRITTER_ITEM: ItemId = ItemId(2007);

// Wave 1 Pre-Hardmode items (Terraria item ids)
pub const STARFURY: ItemId = ItemId(65);
pub const GRAPPLING_HOOK: ItemId = ItemId(84);
pub const IRON_CHAIN: ItemId = ItemId(85);
pub const LESSER_MANA_POTION: ItemId = ItemId(110);
pub const METEORITE_ITEM: ItemId = ItemId(116);
pub const BOMB: ItemId = ItemId(166);
pub const DYNAMITE: ItemId = ItemId(167);
pub const GRENADE: ItemId = ItemId(168);
pub const GLOWING_MUSHROOM: ItemId = ItemId(183);
pub const HEALING_POTION: ItemId = ItemId(188);
pub const MANA_POTION: ItemId = ItemId(189);
pub const HOOK: ItemId = ItemId(210);
pub const REGENERATION_POTION: ItemId = ItemId(289);
pub const SWIFTNESS_POTION: ItemId = ItemId(290);
pub const IRONSKIN_POTION: ItemId = ItemId(292);
pub const SHINE_POTION: ItemId = ItemId(298);
pub const WATER_WALKING_POTION: ItemId = ItemId(302);
pub const TINKERERS_WORKSHOP_ITEM: ItemId = ItemId(398);
pub const CLOUD_ITEM: ItemId = ItemId(751);
pub const SUNPLATE_ITEM: ItemId = ItemId(824);
pub const CAMPFIRE_ITEM: ItemId = ItemId(966);
pub const HIVE_ITEM: ItemId = ItemId(1124);
pub const HEART_LANTERN_ITEM: ItemId = ItemId(1859);
pub const RECALL_POTION: ItemId = ItemId(2350);

pub fn is_coin(id: ItemId) -> bool {
    [COPPER_COIN, SILVER_COIN, GOLD_COIN, PLATINUM_COIN].contains(&id)
}

pub fn is_ammo(id: ItemId) -> bool {
    item_data(id).class == ItemClass::Ammo
}

pub fn is_arrow(id: ItemId) -> bool {
    id == WOODEN_ARROW || id == FLAMING_ARROW
}

pub fn is_bullet(id: ItemId) -> bool {
    id == MUSKET_BALL
}

pub fn matches_ammo(weapon: ItemId, ammo: ItemId) -> bool {
    if weapon == MUSKET
        || weapon == FLINTLOCK_PISTOL
        || weapon == MINISHARK
        || weapon == HANDGUN
        || weapon == PHOENIX_BLASTER
        || weapon == CLOCKWORK_ASSAULT_RIFLE
    {
        is_bullet(ammo)
    } else {
        is_arrow(ammo)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rarity {
    Gray,
    White,
    Blue,
    Green,
    Orange,
    LightRed,
    Pink,
    LightPurple,
    Lime,
    Yellow,
    Cyan,
    Red,
    Purple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemClass {
    Melee,
    /// Bow: fires the first ammo stack in the inventory.
    Ranged,
    /// Arrow (`Item.ammo == AmmoID.Arrow`).
    Ammo,
    Tool,
    Block,
    Placeable,
    /// Background wall (`Item.createWall`): sets a wall bit instead of a block.
    Wall,
    Armor,
    /// Used up on use (life crystal); the effect is applied by the server.
    Consumable,
    /// Magic weapon: the server checks and spends `ItemData::mana`, then spawns
    /// the wand's projectile. No melee-class bonus.
    Magic,
    Accessory,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmorSlot {
    Head,
    Body,
    Legs,
}

pub struct ItemData {
    pub name: &'static str,
    pub max_stack: u32,
    pub damage: i32,
    pub knock_back: f32,
    pub crit: i32,
    pub use_time: i32,
    pub use_animation: i32,
    pub pick: i32,
    pub axe: i32,
    pub hammer: i32,
    pub create_tile: Option<BlockId>,
    pub class: ItemClass,
    pub scale: f32,
    /// `Item.defense`, summed over equipped armor.
    pub defense: i32,
    /// Which armor slot the item equips into (`headSlot`/`bodySlot`/`legSlot`).
    pub armor_slot: Option<ArmorSlot>,
    /// `Item.shootSpeed` in px/tick (bows and ammo add theirs, Player.PickAmmo).
    pub shoot_speed: f32,
    /// `Item.mana`: mana spent per successful magic cast.
    pub mana: i32,
    /// `Item.autoReuse`: weapon continues swinging/firing while use button is held.
    pub auto_reuse: bool,
    /// `Item.value` in copper coins.
    pub value: u32,
    /// Rarity tier for tooltip coloration.
    pub rarity: Rarity,
    /// Concise tooltip text describing the item's function or flavor.
    pub tooltip: &'static str,
    /// Buff duration in seconds applied on consumption, if applicable.
    pub buff_secs: u16,
}

impl ItemData {
    pub const fn with_value(mut self, value: u32) -> Self {
        self.value = value;
        self
    }

    pub const fn with_rarity(mut self, rarity: Rarity) -> Self {
        self.rarity = rarity;
        self
    }

    pub const fn with_tooltip(mut self, tooltip: &'static str) -> Self {
        self.tooltip = tooltip;
        self
    }

    pub const fn with_buff_secs(mut self, buff_secs: u16) -> Self {
        self.buff_secs = buff_secs;
        self
    }
}

const fn block(name: &'static str, tile: BlockId) -> ItemData {
    // Common block item defaults: consumable, createTile = tile, maxStack 9999.
    // Non-weapons have damage -1 in Terraria; 0 here since nothing reads it.
    ItemData {
        name,
        max_stack: 9999,
        damage: 0,
        knock_back: 0.0,
        crit: 0,
        use_time: 10,
        use_animation: 15,
        pick: 0,
        axe: 0,
        hammer: 0,
        create_tile: Some(tile),
        class: ItemClass::Block,
        scale: 1.0,
        defense: 0,
        armor_slot: None,
        shoot_speed: 0.0,
        mana: 0,
        auto_reuse: true,
        value: 1,
        rarity: Rarity::White,
        tooltip: "Can be placed",
        buff_secs: 0,
    }
}

#[allow(clippy::too_many_arguments)]
const fn tool(
    name: &'static str,
    class: ItemClass,
    damage: i32,
    knock_back: f32,
    use_time: i32,
    use_animation: i32,
    pick: i32,
    axe: i32,
    scale: f32,
) -> ItemData {
    // crit 4 is Item.item defaults' default for weapons; tools stack to 1.
    // Terraria gives pickaxes, axes and hammers `autoReuse = true`, so a held
    // button keeps mining; swords keep the fresh-press rule.
    ItemData {
        name,
        max_stack: 1,
        damage,
        knock_back,
        crit: 4,
        use_time,
        use_animation,
        pick,
        axe,
        hammer: 0,
        create_tile: None,
        class,
        scale,
        defense: 0,
        armor_slot: None,
        shoot_speed: 0.0,
        mana: 0,
        auto_reuse: matches!(class, ItemClass::Tool),
        value: 100,
        rarity: Rarity::White,
        tooltip: "Tool",
        buff_secs: 0,
    }
}

const fn hammer_tool(
    name: &'static str,
    damage: i32,
    knock_back: f32,
    use_time: i32,
    use_animation: i32,
    hammer: i32,
) -> ItemData {
    ItemData {
        name,
        max_stack: 1,
        damage,
        knock_back,
        crit: 4,
        use_time,
        use_animation,
        pick: 0,
        axe: 0,
        hammer,
        create_tile: None,
        class: ItemClass::Tool,
        scale: 1.0,
        defense: 0,
        armor_slot: None,
        shoot_speed: 0.0,
        mana: 0,
        auto_reuse: true,
        value: 100,
        rarity: Rarity::White,
        tooltip: "Destroys background walls",
        buff_secs: 0,
    }
}

/// Armor piece: stack 1, `defense` and slot from the `Item.item defaults` case.
const fn armor(name: &'static str, slot: ArmorSlot, defense: i32) -> ItemData {
    ItemData {
        name,
        max_stack: 1,
        damage: 0,
        knock_back: 0.0,
        crit: 0,
        use_time: 100,
        use_animation: 100,
        pick: 0,
        axe: 0,
        hammer: 0,
        create_tile: None,
        class: ItemClass::Armor,
        scale: 1.0,
        defense,
        armor_slot: Some(slot),
        shoot_speed: 0.0,
        mana: 0,
        auto_reuse: false,
        value: 500,
        rarity: Rarity::White,
        tooltip: "Equips into armor slot",
        buff_secs: 0,
    }
}

/// Accessory: stack 1, class `Accessory`.
const fn accessory(name: &'static str) -> ItemData {
    ItemData {
        name,
        max_stack: 1,
        damage: 0,
        knock_back: 0.0,
        crit: 0,
        use_time: 10,
        use_animation: 15,
        pick: 0,
        axe: 0,
        hammer: 0,
        create_tile: None,
        class: ItemClass::Accessory,
        scale: 1.0,
        defense: 0,
        armor_slot: None,
        shoot_speed: 0.0,
        mana: 0,
        auto_reuse: false,
        value: 50000,
        rarity: Rarity::Blue,
        tooltip: "Equip to gain special effects",
        buff_secs: 0,
    }
}

/// Non-placeable, non-weapon item (gel, coins).
const fn misc(name: &'static str, max_stack: u32) -> ItemData {
    ItemData {
        name,
        max_stack,
        damage: 0,
        knock_back: 0.0,
        crit: 0,
        use_time: 10,
        use_animation: 15,
        pick: 0,
        axe: 0,
        hammer: 0,
        create_tile: None,
        class: ItemClass::Other,
        scale: 1.0,
        defense: 0,
        armor_slot: None,
        shoot_speed: 0.0,
        mana: 0,
        auto_reuse: false,
        value: 10,
        rarity: Rarity::White,
        tooltip: "Material",
        buff_secs: 0,
    }
}

/// Consumable with the given `useTime`/`useAnimation` (gameplay spec `consumable = true`).
/// Arrow-class ammo: stacks like a block, never used on its own.
const fn ammo(name: &'static str, damage: i32, knock_back: f32) -> ItemData {
    ItemData {
        damage,
        knock_back,
        crit: 0,
        class: ItemClass::Ammo,
        value: 5,
        rarity: Rarity::White,
        tooltip: "Ammunition",
        buff_secs: 0,
        ..misc(name, 9999)
    }
}

const fn consumable(name: &'static str, max_stack: u32, time: i32) -> ItemData {
    ItemData {
        class: ItemClass::Consumable,
        use_time: time,
        use_animation: time,
        value: 100,
        rarity: Rarity::White,
        tooltip: "Consumable",
        buff_secs: 0,
        ..misc(name, max_stack)
    }
}

const fn bucket(name: &'static str, max_stack: u32) -> ItemData {
    ItemData {
        use_time: 15,
        use_animation: 15,
        value: 400,
        rarity: Rarity::White,
        tooltip: "Can hold liquids",
        buff_secs: 0,
        ..misc(name, max_stack)
    }
}

// Names: Terraria.Localization.Content.en-US.Items.json, ItemName.
// TODO(localization): hand table limited to the items in play.
static ITEMS: [(ItemId, ItemData); 172] = [
    (
        COPPER_HELMET,
        armor("Copper Helmet", ArmorSlot::Head, 1)
            .with_value(750)
            .with_tooltip("1 defense"),
    ),
    (
        COPPER_CHAINMAIL,
        armor("Copper Chainmail", ArmorSlot::Body, 2)
            .with_value(1000)
            .with_tooltip("2 defense"),
    ),
    (
        COPPER_GREAVES,
        armor("Copper Greaves", ArmorSlot::Legs, 1)
            .with_value(750)
            .with_tooltip("1 defense"),
    ),
    (
        IRON_HELMET,
        armor("Iron Helmet", ArmorSlot::Head, 2)
            .with_value(1250)
            .with_tooltip("2 defense"),
    ),
    (
        IRON_CHAINMAIL,
        armor("Iron Chainmail", ArmorSlot::Body, 3)
            .with_value(1600)
            .with_tooltip("3 defense"),
    ),
    (
        IRON_GREAVES,
        armor("Iron Greaves", ArmorSlot::Legs, 2)
            .with_value(1250)
            .with_tooltip("2 defense"),
    ),
    (
        SILVER_HELMET,
        armor("Silver Helmet", ArmorSlot::Head, 3)
            .with_value(2500)
            .with_tooltip("3 defense"),
    ),
    (
        SILVER_CHAINMAIL,
        armor("Silver Chainmail", ArmorSlot::Body, 4)
            .with_value(3000)
            .with_tooltip("4 defense"),
    ),
    (
        SILVER_GREAVES,
        armor("Silver Greaves", ArmorSlot::Legs, 3)
            .with_value(2500)
            .with_tooltip("3 defense"),
    ),
    (
        GOLD_HELMET,
        armor("Gold Helmet", ArmorSlot::Head, 4)
            .with_value(5000)
            .with_tooltip("4 defense"),
    ),
    (
        GOLD_CHAINMAIL,
        armor("Gold Chainmail", ArmorSlot::Body, 5)
            .with_value(6000)
            .with_tooltip("5 defense"),
    ),
    (
        GOLD_GREAVES,
        armor("Gold Greaves", ArmorSlot::Legs, 4)
            .with_value(5000)
            .with_tooltip("4 defense"),
    ),
    (
        IRON_PICKAXE,
        tool("Iron Pickaxe", ItemClass::Tool, 5, 2.0, 13, 20, 40, 0, 1.0)
            .with_value(2000)
            .with_tooltip("Can mine iron, silver, and gold ore"),
    ),
    (
        IRON_BROADSWORD,
        tool(
            "Iron Broadsword",
            ItemClass::Melee,
            12,
            5.5,
            20,
            20,
            0,
            0,
            1.0,
        )
        .with_value(1800)
        .with_tooltip("A standard iron blade"),
    ),
    (
        IRON_AXE,
        tool("Iron Axe", ItemClass::Tool, 5, 4.5, 19, 27, 0, 9, 1.0)
            .with_value(1600)
            .with_tooltip("Chops down trees"),
    ),
    (
        SILVER_AXE,
        tool("Silver Axe", ItemClass::Tool, 6, 4.5, 18, 26, 0, 10, 1.0)
            .with_value(4000)
            .with_tooltip("Chops down trees quickly"),
    ),
    (
        SILVER_BROADSWORD,
        tool(
            "Silver Broadsword",
            ItemClass::Melee,
            14,
            6.0,
            20,
            20,
            0,
            0,
            1.0,
        )
        .with_value(4500)
        .with_tooltip("A sturdy silver blade"),
    ),
    (
        SILVER_PICKAXE,
        tool(
            "Silver Pickaxe",
            ItemClass::Tool,
            6,
            2.0,
            11,
            19,
            45,
            0,
            1.05,
        )
        .with_value(5000)
        .with_tooltip("Can mine demonite ore"),
    ),
    (
        GOLD_AXE,
        tool("Gold Axe", ItemClass::Tool, 7, 4.5, 18, 26, 0, 11, 1.0)
            .with_value(8000)
            .with_tooltip("Chops down trees very quickly"),
    ),
    (
        GOLD_BROADSWORD,
        tool(
            "Gold Broadsword",
            ItemClass::Melee,
            15,
            6.5,
            18,
            18,
            0,
            0,
            1.0,
        )
        .with_value(9000)
        .with_tooltip("A heavy golden blade"),
    ),
    (
        GOLD_PICKAXE,
        tool("Gold Pickaxe", ItemClass::Tool, 6, 2.0, 17, 20, 55, 0, 1.05)
            .with_value(10000)
            .with_tooltip("Can mine demonite ore and meteorite"),
    ),
    (
        GOLD_BAR,
        misc("Gold Bar", 9999)
            .with_value(500)
            .with_tooltip("Smelted metal ready for smithing"),
    ),
    (
        COPPER_BAR,
        misc("Copper Bar", 9999)
            .with_value(60)
            .with_tooltip("Smelted metal ready for smithing"),
    ),
    (
        SILVER_BAR,
        misc("Silver Bar", 9999)
            .with_value(250)
            .with_tooltip("Smelted metal ready for smithing"),
    ),
    (
        IRON_BAR,
        misc("Iron Bar", 9999)
            .with_value(120)
            .with_tooltip("Smelted metal ready for smithing"),
    ),
    (
        LIFE_CRYSTAL_ITEM,
        consumable("Life Crystal", 9999, 30)
            .with_value(75000)
            .with_rarity(Rarity::Green)
            .with_tooltip("Permanently increases maximum life by 20"),
    ),
    (
        HEART,
        misc("Heart", 9999)
            .with_value(10)
            .with_tooltip("Restores 20 life on pickup"),
    ),
    (
        FURNACE_ITEM,
        block("Furnace", FURNACE)
            .with_value(1500)
            .with_tooltip("Used for smelting ore into bars"),
    ),
    (
        ANVIL_ITEM,
        block("Iron Anvil", ANVIL)
            .with_value(5000)
            .with_tooltip("Used for crafting metal tools and weapons"),
    ),
    (
        COPPER_BROADSWORD,
        tool(
            "Copper Broadsword",
            ItemClass::Melee,
            9,
            5.5,
            20,
            21,
            0,
            0,
            1.0,
        )
        .with_value(450)
        .with_tooltip("A broad copper blade"),
    ),
    (
        GEL,
        misc("Gel", 9999)
            .with_value(5)
            .with_tooltip("Flammable slime residue"),
    ),
    (
        WOODEN_SWORD,
        tool("Wooden Sword", ItemClass::Melee, 7, 5.0, 20, 20, 0, 0, 1.0)
            .with_value(100)
            .with_tooltip("Better than nothing"),
    ),
    (
        WORKBENCH_ITEM,
        block("Work Bench", WORKBENCH)
            .with_value(150)
            .with_tooltip("Used for basic crafting"),
    ),
    (
        LESSER_HEALING_POTION,
        consumable("Lesser Healing Potion", 9999, 17)
            .with_value(300)
            .with_tooltip("Restores 50 life")
            .with_buff_secs(60),
    ),
    (
        LENS,
        misc("Lens", 9999)
            .with_value(100)
            .with_tooltip("Plucked from a wandering eye"),
    ),
    (
        SHADOW_SCALE,
        misc("Shadow Scale", 9999)
            .with_value(500)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Torn from the Eater of Worlds"),
    ),
    (
        SUSPICIOUS_LOOKING_EYE,
        consumable("Suspicious Looking Eye", 9999, 45)
            .with_value(15000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Summons the Eye of Cthulhu"),
    ),
    (
        MUSHROOM,
        consumable("Mushroom", 9999, 17)
            .with_value(25)
            .with_tooltip("Restores 15 life; causes Potion Sickness"),
    ),
    (
        BOTTLE_ITEM,
        block("Bottle", BOTTLE)
            .with_value(20)
            .with_tooltip("Can be placed or used to brew potions"),
    ),
    (
        GLASS,
        block("Glass", GLASS_BLOCK)
            .with_value(100)
            .with_tooltip("Can be placed or crafted into bottles"),
    ),
    (
        CHEST_ITEM,
        block("Chest", CHEST)
            .with_value(500)
            .with_tooltip("Stores up to 20 items"),
    ),
    (
        WOODEN_DOOR,
        block("Wooden Door", DOOR_CLOSED)
            .with_value(200)
            .with_tooltip("Keeps monsters outside"),
    ),
    (
        WOODEN_TABLE,
        block("Wooden Table", TABLE)
            .with_value(300)
            .with_tooltip("Required for town NPC housing"),
    ),
    (
        WOODEN_CHAIR,
        block("Wooden Chair", CHAIR)
            .with_value(150)
            .with_tooltip("Required for town NPC housing"),
    ),
    (
        WOOD_PLATFORM,
        block("Wood Platform", PLATFORM)
            .with_value(10)
            .with_tooltip("Can be stepped on or dropped through"),
    ),
    (
        WOOD_WALL,
        ItemData {
            use_time: 7,
            create_tile: None,
            class: ItemClass::Wall,
            value: 5,
            rarity: Rarity::White,
            tooltip: "Places background wall",
            buff_secs: 0,
            ..block("Wood Wall", BlockId(0))
        },
    ),
    (
        WOODEN_BOW,
        ItemData {
            shoot_speed: 6.1,
            mana: 0,
            class: ItemClass::Ranged,
            value: 100,
            rarity: Rarity::White,
            tooltip: "Fires wooden and flaming arrows",
            buff_secs: 0,
            ..tool("Wooden Bow", ItemClass::Ranged, 4, 0.0, 30, 30, 0, 0, 1.0)
        },
    ),
    (
        WORM_FOOD,
        consumable("Worm Food", 9999, 45)
            .with_value(20000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Summons the Eater of Worlds"),
    ),
    (
        WOODEN_ARROW,
        ItemData {
            shoot_speed: 3.0,
            mana: 0,
            value: 5,
            rarity: Rarity::White,
            tooltip: "Basic wooden ammunition",
            buff_secs: 0,
            ..ammo("Wooden Arrow", 5, 2.0)
        },
    ),
    (
        FLAMING_ARROW,
        ItemData {
            shoot_speed: 3.5,
            mana: 0,
            value: 10,
            rarity: Rarity::White,
            tooltip: "Sets targets on fire",
            buff_secs: 0,
            ..ammo("Flaming Arrow", 7, 2.0)
        },
    ),
    (
        COPPER_COIN,
        misc("Copper Coin", 100)
            .with_value(0)
            .with_tooltip("Basic currency"),
    ),
    (
        SILVER_COIN,
        misc("Silver Coin", 100)
            .with_value(0)
            .with_tooltip("Equal to 100 copper coins"),
    ),
    (
        GOLD_COIN,
        misc("Gold Coin", 100)
            .with_value(0)
            .with_tooltip("Equal to 100 silver coins"),
    ),
    (
        PLATINUM_COIN,
        misc("Platinum Coin", 100)
            .with_value(0)
            .with_tooltip("Equal to 100 gold coins"),
    ),
    (
        COPPER_SHORTSWORD,
        tool(
            "Copper Shortsword",
            ItemClass::Melee,
            5,
            4.0,
            13,
            13,
            0,
            0,
            0.8,
        )
        .with_value(350)
        .with_tooltip("Pointy end goes toward the enemy"),
    ),
    (
        COPPER_PICKAXE,
        tool(
            "Copper Pickaxe",
            ItemClass::Tool,
            4,
            2.0,
            15,
            23,
            35,
            0,
            0.9,
        )
        .with_value(500)
        .with_tooltip("Can mine basic ores and dirt"),
    ),
    (
        COPPER_AXE,
        tool("Copper Axe", ItemClass::Tool, 3, 4.5, 21, 30, 0, 7, 1.0)
            .with_value(400)
            .with_tooltip("Chops down trees"),
    ),
    (
        TORCH,
        ItemData {
            name: "Torch",
            max_stack: 9999,
            damage: 0,
            knock_back: 0.0,
            crit: 0,
            use_time: 10,
            use_animation: 15,
            pick: 0,
            axe: 0,
            hammer: 0,
            create_tile: Some(BlockId(4)),
            class: ItemClass::Placeable,
            shoot_speed: 0.0,
            mana: 0,
            scale: 1.0,
            defense: 0,
            armor_slot: None,
            auto_reuse: true,
            value: 50,
            rarity: Rarity::White,
            tooltip: "Provides light",
            buff_secs: 0,
        },
    ),
    (
        DIRT_BLOCK,
        block("Dirt Block", DIRT)
            .with_value(1)
            .with_tooltip("Can be placed"),
    ),
    (
        WOOD,
        block("Wood", WOOD_BLOCK)
            .with_value(10)
            .with_tooltip("A versatile building and crafting material"),
    ),
    (
        STONE_BLOCK,
        block("Stone Block", STONE)
            .with_value(2)
            .with_tooltip("Can be placed"),
    ),
    (
        IRON_ORE,
        block("Iron Ore", IRON)
            .with_value(30)
            .with_tooltip("Can be smelted into iron bars"),
    ),
    (
        COPPER_ORE,
        block("Copper Ore", COPPER)
            .with_value(15)
            .with_tooltip("Can be smelted into copper bars"),
    ),
    (
        GOLD_ORE,
        block("Gold Ore", GOLD)
            .with_value(120)
            .with_tooltip("Can be smelted into gold bars"),
    ),
    (
        SILVER_ORE,
        block("Silver Ore", SILVER)
            .with_value(60)
            .with_tooltip("Can be smelted into silver bars"),
    ),
    (
        SAND_BLOCK,
        block("Sand Block", SAND)
            .with_value(2)
            .with_tooltip("Can be smelted into glass"),
    ),
    (
        MUD_BLOCK,
        block("Mud Block", MUD)
            .with_value(1)
            .with_tooltip("Can be placed"),
    ),
    (
        SNOW_BLOCK,
        block("Snow Block", SNOW)
            .with_value(1)
            .with_tooltip("Cold and fluffy"),
    ),
    (
        ICE_BLOCK,
        block("Ice Block", ICE)
            .with_value(2)
            .with_tooltip("Slippery surface"),
    ),
    (
        SANDSTONE_BLOCK,
        block("Sandstone Block", SANDSTONE)
            .with_value(3)
            .with_tooltip("Compressed desert stone"),
    ),
    (
        HARDENED_SAND_BLOCK,
        block("Hardened Sand Block", HARDENED_SAND)
            .with_value(3)
            .with_tooltip("Durable sand formation"),
    ),
    (
        DEMONITE_ORE_ITEM,
        block("Demonite Ore", DEMONITE_ORE)
            .with_value(400)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Glows with demonic energy"),
    ),
    (
        EBONSTONE_BLOCK,
        block("Ebonstone Block", EBONSTONE)
            .with_value(5)
            .with_tooltip("Pulsing corruption stone"),
    ),
    (
        EMPTY_BUCKET,
        bucket("Empty Bucket", 99)
            .with_value(400)
            .with_tooltip("Can hold water or lava"),
    ),
    (
        WATER_BUCKET,
        bucket("Water Bucket", 99)
            .with_value(500)
            .with_tooltip("Contains fresh water"),
    ),
    (
        LAVA_BUCKET,
        bucket("Lava Bucket", 99)
            .with_value(500)
            .with_tooltip("Contains molten lava"),
    ),
    (
        OBSIDIAN_ITEM,
        block("Obsidian", OBSIDIAN)
            .with_value(50)
            .with_tooltip("Formed when water meets lava"),
    ),
    (
        DEMONITE_BAR,
        misc("Demonite Bar", 9999)
            .with_value(1600)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Infused with shadowy power"),
    ),
    (
        DEMON_BOW,
        ItemData {
            shoot_speed: 6.7,
            mana: 0,
            class: ItemClass::Ranged,
            value: 18000,
            rarity: Rarity::Blue,
            tooltip: "A demonic bow that fires arrows with high velocity",
            buff_secs: 0,
            ..tool("Demon Bow", ItemClass::Ranged, 14, 1.0, 25, 25, 0, 0, 1.0)
        },
    ),
    (
        WAR_AXE_OF_THE_NIGHT,
        tool(
            "War Axe of the Night",
            ItemClass::Tool,
            20,
            6.0,
            15,
            30,
            0,
            15,
            1.0,
        )
        .with_value(27000)
        .with_rarity(Rarity::Blue)
        .with_tooltip("A wicked axe forged from demonite"),
    ),
    (
        LIGHTS_BANE,
        tool("Light's Bane", ItemClass::Melee, 18, 5.0, 20, 20, 0, 0, 1.0)
            .with_value(27000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("A glowing demonite blade"),
    ),
    (
        NIGHTMARE_PICKAXE,
        tool(
            "Nightmare Pickaxe",
            ItemClass::Tool,
            9,
            3.0,
            15,
            20,
            65,
            0,
            1.0,
        )
        .with_value(27000)
        .with_rarity(Rarity::Blue)
        .with_tooltip("Capable of mining hellstone and ebonstone"),
    ),
    (
        SHADOW_HELMET,
        armor("Shadow Helmet", ArmorSlot::Head, 6)
            .with_value(37500)
            .with_rarity(Rarity::Blue)
            .with_tooltip("6 defense"),
    ),
    (
        SHADOW_SCALEMAIL,
        armor("Shadow Scalemail", ArmorSlot::Body, 7)
            .with_value(30000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("7 defense"),
    ),
    (
        SHADOW_GREAVES,
        armor("Shadow Greaves", ArmorSlot::Legs, 6)
            .with_value(22500)
            .with_rarity(Rarity::Blue)
            .with_tooltip("6 defense"),
    ),
    (
        FALLEN_STAR,
        ItemData {
            use_time: 17,
            use_animation: 17,
            value: 2500,
            rarity: Rarity::Blue,
            tooltip: "Shooting star fallen from the night sky",
            buff_secs: 0,
            ..misc("Fallen Star", 9999)
        },
    ),
    (
        MANA_CRYSTAL,
        consumable("Mana Crystal", 9999, 30)
            .with_value(10000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Permanently increases maximum mana by 20"),
    ),
    (
        WAND_OF_SPARKING,
        ItemData {
            damage: 14,
            mana: 2,
            use_time: 26,
            use_animation: 26,
            crit: 10,
            shoot_speed: 7.0,
            class: ItemClass::Magic,
            value: 10000,
            rarity: Rarity::White,
            tooltip: "Shoots a spark of fire",
            buff_secs: 0,
            ..misc("Wand of Sparking", 1)
        },
    ),
    (
        HERMES_BOOTS,
        accessory("Hermes Boots")
            .with_value(50000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("The wearer can run super fast"),
    ),
    (
        CLOUD_IN_A_BOTTLE,
        accessory("Cloud in a Bottle")
            .with_value(50000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Allows the holder to double jump"),
    ),
    (
        BAND_OF_REGENERATION,
        accessory("Band of Regeneration")
            .with_value(50000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Slowly regenerates life"),
    ),
    (
        MAGIC_MIRROR,
        ItemData {
            use_time: 90,
            use_animation: 90,
            value: 50000,
            rarity: Rarity::Blue,
            tooltip: "Gaze into the mirror to return home",
            buff_secs: 0,
            ..misc("Magic Mirror", 1)
        },
    ),
    (
        ENCHANTED_BOOMERANG,
        ItemData {
            damage: 17,
            knock_back: 8.0,
            use_time: 15,
            use_animation: 15,
            shoot_speed: 10.0,
            class: ItemClass::Melee,
            crit: 4,
            value: 100000,
            rarity: Rarity::Blue,
            tooltip: "Returns to the thrower and emits light",
            buff_secs: 0,
            ..misc("Enchanted Boomerang", 1)
        },
    ),
    (
        SPEAR,
        ItemData {
            damage: 8,
            knock_back: 6.5,
            use_time: 31,
            use_animation: 31,
            shoot_speed: 3.7,
            scale: 1.2,
            class: ItemClass::Melee,
            crit: 4,
            value: 1000,
            rarity: Rarity::White,
            tooltip: "Thrusts at enemies at a distance",
            buff_secs: 0,
            ..misc("Spear", 1)
        },
    ),
    (
        SHINY_RED_BALLOON,
        accessory("Shiny Red Balloon")
            .with_value(75000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Increases jump height"),
    ),
    (
        LUCKY_HORSESHOE,
        accessory("Lucky Horseshoe")
            .with_value(100000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Negates fall damage"),
    ),
    (
        WOODEN_HAMMER,
        hammer_tool("Wooden Hammer", 2, 5.5, 25, 37, 25)
            .with_value(100)
            .with_tooltip("Destroys background walls"),
    ),
    (
        THE_BREAKER,
        hammer_tool("The Breaker", 24, 6.0, 19, 45, 55)
            .with_value(27000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("A colossal demonic warhammer"),
    ),
    (
        VILETHORN,
        ItemData {
            damage: 10,
            knock_back: 1.0,
            mana: 10,
            use_time: 28,
            use_animation: 28,
            crit: 4,
            shoot_speed: 32.0,
            class: ItemClass::Magic,
            value: 50000,
            rarity: Rarity::Blue,
            tooltip: "Summons piercing thorns through solid blocks",
            buff_secs: 0,
            ..misc("Vilethorn", 1)
        },
    ),
    (
        FLINTLOCK_PISTOL,
        ItemData {
            damage: 13,
            knock_back: 1.0,
            use_time: 16,
            use_animation: 16,
            crit: 4,
            shoot_speed: 6.5,
            class: ItemClass::Ranged,
            value: 50000,
            rarity: Rarity::White,
            tooltip: "A classic flintlock pistol",
            buff_secs: 0,
            ..misc("Flintlock Pistol", 1)
        },
    ),
    (
        MUSKET,
        ItemData {
            damage: 31,
            knock_back: 5.25,
            use_time: 36,
            use_animation: 36,
            crit: 4,
            shoot_speed: 9.0,
            class: ItemClass::Ranged,
            value: 50000,
            rarity: Rarity::Blue,
            tooltip: "A powerful, slow-firing rifle",
            buff_secs: 0,
            ..misc("Musket", 1)
        },
    ),
    (
        MINISHARK,
        ItemData {
            damage: 6,
            knock_back: 0.0,
            use_time: 8,
            use_animation: 8,
            crit: 4,
            shoot_speed: 7.0,
            auto_reuse: true,
            class: ItemClass::Ranged,
            value: 350000,
            rarity: Rarity::Green,
            tooltip: "33% chance to save ammo; half shark, half gun",
            buff_secs: 0,
            ..misc("Minishark", 1)
        },
    ),
    (
        MUSKET_BALL,
        ItemData {
            shoot_speed: 4.0,
            mana: 0,
            value: 7,
            rarity: Rarity::White,
            tooltip: "Standard bullet for firearms",
            buff_secs: 0,
            ..ammo("Musket Ball", 7, 2.0)
        },
    ),
    (
        BAND_OF_STARPOWER,
        accessory("Band of Starpower")
            .with_value(50000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Increases maximum mana by 20"),
    ),
    (
        SHADOW_ORB_ITEM,
        ItemData {
            use_time: 20,
            use_animation: 20,
            value: 75000,
            rarity: Rarity::Blue,
            tooltip: "A mysterious glowing orb from the Corruption",
            buff_secs: 0,
            ..misc("Shadow Orb", 1)
        },
    ),
    (
        BALL_O_HURT,
        ItemData {
            damage: 30,
            knock_back: 5.5,
            use_time: 45,
            use_animation: 45,
            crit: 4,
            shoot_speed: 12.0,
            class: ItemClass::Melee,
            value: 27000,
            rarity: Rarity::Blue,
            tooltip: "A heavy spiky flail",
            buff_secs: 0,
            ..misc("Ball O' Hurt", 1)
        },
    ),
    (
        BONE,
        misc("Bone", 9999)
            .with_value(50)
            .with_tooltip("Dungeon skeleton remains"),
    ),
    (
        GOLDEN_KEY,
        misc("Golden Key", 99)
            .with_value(25000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Unlocks one locked golden chest in the Dungeon"),
    ),
    (
        MAGIC_MISSILE,
        ItemData {
            damage: 35,
            knock_back: 7.5,
            mana: 14,
            use_time: 17,
            use_animation: 17,
            crit: 4,
            shoot_speed: 6.0,
            class: ItemClass::Magic,
            value: 50000,
            rarity: Rarity::Blue,
            tooltip: "Casts a controllable missile of energy",
            buff_secs: 0,
            ..misc("Magic Missile", 1)
        },
    ),
    (
        MURAMASA,
        ItemData {
            damage: 24,
            knock_back: 3.0,
            use_time: 18,
            use_animation: 18,
            crit: 4,
            class: ItemClass::Melee,
            auto_reuse: true,
            value: 87500,
            rarity: Rarity::Blue,
            tooltip: "Rapidly slashes and sheds light",
            buff_secs: 0,
            ..misc("Muramasa", 1)
        },
    ),
    (
        COBALT_SHIELD,
        ItemData {
            defense: 1,
            value: 54000,
            rarity: Rarity::Blue,
            tooltip: "Grants immunity to knockback",
            buff_secs: 0,
            ..accessory("Cobalt Shield")
        },
    ),
    (
        AQUA_SCEPTER,
        ItemData {
            damage: 27,
            knock_back: 7.0,
            mana: 7,
            use_time: 16,
            use_animation: 16,
            crit: 4,
            shoot_speed: 11.0,
            class: ItemClass::Magic,
            value: 87500,
            rarity: Rarity::Blue,
            tooltip: "Sprays a continuous stream of water",
            buff_secs: 0,
            ..misc("Aqua Scepter", 1)
        },
    ),
    (
        BLUE_MOON,
        ItemData {
            damage: 54,
            knock_back: 6.0,
            use_time: 45,
            use_animation: 45,
            crit: 4,
            shoot_speed: 12.0,
            class: ItemClass::Melee,
            value: 54000,
            rarity: Rarity::Green,
            tooltip: "A spinning ball of blue fire",
            buff_secs: 0,
            ..misc("Blue Moon", 1)
        },
    ),
    (
        HANDGUN,
        ItemData {
            damage: 26,
            knock_back: 3.0,
            use_time: 12,
            use_animation: 12,
            crit: 4,
            shoot_speed: 10.0,
            class: ItemClass::Ranged,
            value: 50000,
            rarity: Rarity::Green,
            tooltip: "Semi-automatic pistol",
            buff_secs: 0,
            ..misc("Handgun", 1)
        },
    ),
    (
        SHADOW_KEY,
        misc("Shadow Key", 1)
            .with_value(75000)
            .with_rarity(Rarity::Orange)
            .with_tooltip("Infinitely unlocks shadow chests in the Underworld"),
    ),
    (
        VALOR,
        ItemData {
            damage: 28,
            knock_back: 3.5,
            use_time: 25,
            use_animation: 25,
            crit: 4,
            shoot_speed: 12.0,
            class: ItemClass::Melee,
            value: 70000,
            rarity: Rarity::Green,
            tooltip: "A high-momentum dungeon yoyo",
            buff_secs: 0,
            ..misc("Valor", 1)
        },
    ),
    (
        ASH_BLOCK,
        block("Ash Block", ASH)
            .with_value(1)
            .with_tooltip("Remnants of the Underworld"),
    ),
    (
        HELLSTONE_ITEM,
        block("Hellstone", HELLSTONE)
            .with_value(400)
            .with_rarity(Rarity::Green)
            .with_tooltip("Burns to the touch"),
    ),
    (
        OBSIDIAN_BRICK_ITEM,
        block("Obsidian Brick", OBSIDIAN_BRICK)
            .with_value(10)
            .with_tooltip("Decorative fire-resistant brick"),
    ),
    (
        OBSIDIAN_SKULL,
        ItemData {
            defense: 1,
            value: 45000,
            rarity: Rarity::Green,
            tooltip: "Grants immunity to fire blocks",
            buff_secs: 0,
            ..accessory("Obsidian Skull")
        },
    ),
    (
        HELLSTONE_BRICK_ITEM,
        block("Hellstone Brick", HELLSTONE_BRICK)
            .with_value(10)
            .with_tooltip("Burns to the touch"),
    ),
    (
        HELLFORGE_ITEM,
        block("Hellforge", HELLFORGE)
            .with_value(15000)
            .with_rarity(Rarity::Green)
            .with_tooltip("Used to smelt hellstone into bars"),
    ),
    (
        OBSIDIAN_SKIN_POTION,
        consumable("Obsidian Skin Potion", 9999, 17)
            .with_value(1000)
            .with_rarity(Rarity::Blue)
            .with_tooltip("Immunity to lava")
            .with_buff_secs(360),
    ),
    (
        GUIDE_VOODOO_DOLL,
        accessory("Guide Voodoo Doll")
            .with_value(1000)
            .with_rarity(Rarity::Orange)
            .with_tooltip("You are a terrible person"),
    ),
    (
        LAVA_CHARM,
        accessory("Lava Charm")
            .with_value(150000)
            .with_rarity(Rarity::Green)
            .with_tooltip("Provides 7 seconds of immunity to lava"),
    ),
    (
        HELLSTONE_BAR,
        misc("Hellstone Bar", 9999)
            .with_value(4000)
            .with_rarity(Rarity::Orange)
            .with_tooltip("Hot to the touch, ready for molten smithing"),
    ),
    (
        MOLTEN_HELMET,
        armor("Molten Helmet", ArmorSlot::Head, 8)
            .with_value(9000)
            .with_rarity(Rarity::Orange)
            .with_tooltip("8 defense, 7% increased melee critical strike chance"),
    ),
    (
        MOLTEN_BREASTPLATE,
        armor("Molten Breastplate", ArmorSlot::Body, 9)
            .with_value(6000)
            .with_rarity(Rarity::Orange)
            .with_tooltip("9 defense, 7% increased melee damage"),
    ),
    (
        MOLTEN_GREAVES,
        armor("Molten Greaves", ArmorSlot::Legs, 8)
            .with_value(6000)
            .with_rarity(Rarity::Orange)
            .with_tooltip("8 defense, 7% increased melee speed"),
    ),
    (
        MOLTEN_PICKAXE,
        tool(
            "Molten Pickaxe",
            ItemClass::Tool,
            12,
            2.0,
            23,
            23,
            100,
            0,
            1.0,
        )
        .with_value(27000)
        .with_rarity(Rarity::Orange)
        .with_tooltip("10% chance to set enemies on fire"),
    ),
    (
        MOLTEN_HAMAXE,
        ItemData {
            hammer: 70,
            ..tool(
                "Molten Hamaxe",
                ItemClass::Tool,
                20,
                7.0,
                14,
                14,
                0,
                30,
                1.0,
            )
        }
        .with_value(27000)
        .with_rarity(Rarity::Orange)
        .with_tooltip("Chops trees and hammers walls; 20% chance to set enemies on fire"),
    ),
    (
        FIERY_GREATSWORD,
        tool(
            "Fiery Greatsword",
            ItemClass::Melee,
            40,
            6.5,
            40,
            40,
            0,
            0,
            1.0,
        )
        .with_value(27000)
        .with_rarity(Rarity::Orange)
        .with_tooltip("50% chance to set enemies on fire; explodes on hit"),
    ),
    (
        MOLTEN_FURY,
        ItemData {
            damage: 31,
            knock_back: 2.0,
            use_time: 22,
            use_animation: 22,
            crit: 4,
            shoot_speed: 8.5,
            class: ItemClass::Ranged,
            value: 27000,
            rarity: Rarity::Orange,
            tooltip: "Turns wooden arrows into flaming arrows",
            buff_secs: 0,
            ..misc("Molten Fury", 1)
        },
    ),
    (
        PHOENIX_BLASTER,
        ItemData {
            damage: 33,
            knock_back: 2.0,
            use_time: 11,
            use_animation: 11,
            crit: 4,
            shoot_speed: 13.0,
            auto_reuse: false,
            class: ItemClass::Ranged,
            value: 27000,
            rarity: Rarity::Orange,
            tooltip: "Semi-automatic pistol that uses bullets",
            buff_secs: 0,
            ..misc("Phoenix Blaster", 1)
        },
    ),
    (
        FLAMARANG,
        ItemData {
            damage: 49,
            knock_back: 8.0,
            use_time: 14,
            use_animation: 14,
            crit: 4,
            shoot_speed: 10.0,
            class: ItemClass::Melee,
            value: 27000,
            rarity: Rarity::Orange,
            tooltip: "Pierces enemies and sets them on fire",
            buff_secs: 0,
            ..misc("Flamarang", 1)
        },
    ),
    (
        PWNHAMMER,
        ItemData {
            hammer: 80,
            auto_reuse: true,
            ..tool("Pwnhammer", ItemClass::Tool, 26, 7.5, 14, 14, 0, 0, 1.0)
        }
        .with_value(39000)
        .with_rarity(Rarity::LightRed)
        .with_tooltip("A sacred hammer dropped by the Wall of Flesh"),
    ),
    (
        SORCERER_EMBLEM,
        accessory("Sorcerer Emblem")
            .with_value(100000)
            .with_rarity(Rarity::LightRed)
            .with_tooltip("15% increased magic damage"),
    ),
    (
        WARRIOR_EMBLEM,
        accessory("Warrior Emblem")
            .with_value(100000)
            .with_rarity(Rarity::LightRed)
            .with_tooltip("15% increased melee damage"),
    ),
    (
        RANGER_EMBLEM,
        accessory("Ranger Emblem")
            .with_value(100000)
            .with_rarity(Rarity::LightRed)
            .with_tooltip("15% increased ranged damage"),
    ),
    (
        BREAKER_BLADE,
        tool(
            "Breaker Blade",
            ItemClass::Melee,
            70,
            8.0,
            35,
            35,
            0,
            0,
            1.4,
        )
        .with_value(150000)
        .with_rarity(Rarity::LightRed)
        .with_tooltip("Deals 2.5x damage to enemies at or above 90% health"),
    ),
    (
        CLOCKWORK_ASSAULT_RIFLE,
        ItemData {
            damage: 17,
            knock_back: 0.0,
            use_time: 12,
            use_animation: 12,
            crit: 4,
            shoot_speed: 7.75,
            auto_reuse: true,
            class: ItemClass::Ranged,
            value: 150000,
            rarity: Rarity::LightRed,
            tooltip: "Three round burst; only the first shot consumes ammo",
            buff_secs: 0,
            ..misc("Clockwork Assault Rifle", 1)
        },
    ),
    (
        LASER_RIFLE,
        ItemData {
            damage: 29,
            knock_back: 2.5,
            use_time: 12,
            use_animation: 12,
            crit: 4,
            shoot_speed: 14.0,
            mana: 8,
            auto_reuse: true,
            class: ItemClass::Magic,
            value: 150000,
            rarity: Rarity::LightRed,
            tooltip: "Fires high-velocity purple lasers",
            buff_secs: 0,
            ..misc("Laser Rifle", 1)
        },
    ),
    (
        FIRECRACKER,
        ItemData {
            ..tool("Firecracker", ItemClass::Melee, 40, 2.0, 30, 30, 0, 0, 1.3)
        }
        .with_value(150000)
        .with_rarity(Rarity::LightRed)
        .with_tooltip("Cracks with fiery energy; sets enemies ablaze and explodes on hit"),
    ),
    (
        BUG_NET,
        ItemData {
            ..tool("Bug Net", ItemClass::Tool, 0, 0.0, 22, 22, 0, 0, 1.2)
        }
        .with_value(2500)
        .with_rarity(Rarity::White)
        .with_tooltip("Used to catch critters"),
    ),
    (
        BUNNY_ITEM,
        ItemData {
            ..consumable("Bunny", 999, 15)
        }
        .with_value(500)
        .with_rarity(Rarity::White)
        .with_tooltip("Releases a bunny"),
    ),
    (
        BIRD_ITEM,
        ItemData {
            ..consumable("Bird", 999, 15)
        }
        .with_value(500)
        .with_rarity(Rarity::White)
        .with_tooltip("Releases a bird"),
    ),
    (
        SQUIRREL_ITEM,
        ItemData {
            ..consumable("Squirrel", 999, 15)
        }
        .with_value(500)
        .with_rarity(Rarity::White)
        .with_tooltip("Releases a squirrel"),
    ),
    (
        CAVE_CRITTER_ITEM,
        ItemData {
            ..consumable("Cave Critter", 999, 15)
        }
        .with_value(500)
        .with_rarity(Rarity::White)
        .with_tooltip("Releases a cave critter"),
    ),
    // Wave 1 Pre-Hardmode items
    (
        STARFURY,
        ItemData {
            name: "Starfury",
            max_stack: 1,
            damage: 25,
            knock_back: 5.0,
            crit: 4,
            use_time: 20,
            use_animation: 20,
            pick: 0,
            axe: 0,
            hammer: 0,
            create_tile: None,
            class: ItemClass::Melee,
            scale: 1.15,
            defense: 0,
            armor_slot: None,
            shoot_speed: 12.0,
            mana: 0,
            auto_reuse: true,
            value: 10000,
            rarity: Rarity::Green,
            tooltip: "Forged with the fury of heaven; rains falling stars from the sky",
            buff_secs: 0,
        },
    ),
    (
        GRAPPLING_HOOK,
        ItemData {
            name: "Grappling Hook",
            max_stack: 1,
            damage: 0,
            knock_back: 0.0,
            crit: 0,
            use_time: 20,
            use_animation: 20,
            pick: 0,
            axe: 0,
            hammer: 0,
            create_tile: None,
            class: ItemClass::Tool,
            scale: 1.0,
            defense: 0,
            armor_slot: None,
            shoot_speed: 11.5,
            mana: 0,
            auto_reuse: false,
            value: 2000,
            rarity: Rarity::Blue,
            tooltip: "Get over here! Latches onto solid blocks and pulls you in",
            buff_secs: 0,
        },
    ),
    (
        IRON_CHAIN,
        ItemData {
            name: "Chain",
            max_stack: 9999,
            damage: 0,
            knock_back: 0.0,
            crit: 0,
            use_time: 10,
            use_animation: 15,
            pick: 0,
            axe: 0,
            hammer: 0,
            create_tile: None,
            class: ItemClass::Other,
            scale: 1.0,
            defense: 0,
            armor_slot: None,
            shoot_speed: 0.0,
            mana: 0,
            auto_reuse: false,
            value: 10,
            rarity: Rarity::White,
            tooltip: "Forged iron links used for hooks and lanterns",
            buff_secs: 0,
        },
    ),
    (
        HOOK,
        ItemData {
            name: "Hook",
            max_stack: 99,
            damage: 0,
            knock_back: 0.0,
            crit: 0,
            use_time: 10,
            use_animation: 15,
            pick: 0,
            axe: 0,
            hammer: 0,
            create_tile: None,
            class: ItemClass::Other,
            scale: 1.0,
            defense: 0,
            armor_slot: None,
            shoot_speed: 0.0,
            mana: 0,
            auto_reuse: false,
            value: 2000,
            rarity: Rarity::White,
            tooltip: "Used to craft a Grappling Hook",
            buff_secs: 0,
        },
    ),
    (
        LESSER_MANA_POTION,
        ItemData {
            ..consumable("Lesser Mana Potion", 50, 17)
        }
        .with_value(100)
        .with_rarity(Rarity::White)
        .with_tooltip("Restores 50 mana"),
    ),
    (
        MANA_POTION,
        ItemData {
            ..consumable("Mana Potion", 50, 17)
        }
        .with_value(250)
        .with_rarity(Rarity::Blue)
        .with_tooltip("Restores 100 mana"),
    ),
    (
        HEALING_POTION,
        ItemData {
            ..consumable("Healing Potion", 50, 17)
        }
        .with_value(1000)
        .with_rarity(Rarity::Blue)
        .with_tooltip("Restores 100 life"),
    ),
    (
        GLOWING_MUSHROOM,
        ItemData {
            ..consumable("Glowing Mushroom", 9999, 17)
        }
        .with_value(250)
        .with_rarity(Rarity::Blue)
        .with_tooltip("Restores 25 life and used to brew stronger potions"),
    ),
    (
        REGENERATION_POTION,
        ItemData {
            ..consumable("Regeneration Potion", 50, 17)
        }
        .with_value(200)
        .with_rarity(Rarity::Blue)
        .with_buff_secs(480)
        .with_tooltip("Provides life regeneration"),
    ),
    (
        SWIFTNESS_POTION,
        ItemData {
            ..consumable("Swiftness Potion", 50, 17)
        }
        .with_value(200)
        .with_rarity(Rarity::Blue)
        .with_buff_secs(480)
        .with_tooltip("25% increased movement speed"),
    ),
    (
        IRONSKIN_POTION,
        ItemData {
            ..consumable("Ironskin Potion", 50, 17)
        }
        .with_value(200)
        .with_rarity(Rarity::Blue)
        .with_buff_secs(480)
        .with_tooltip("Increases defense by 8"),
    ),
    (
        SHINE_POTION,
        ItemData {
            ..consumable("Shine Potion", 50, 17)
        }
        .with_value(200)
        .with_rarity(Rarity::Blue)
        .with_buff_secs(300)
        .with_tooltip("Emits an aura of light"),
    ),
    (
        WATER_WALKING_POTION,
        ItemData {
            ..consumable("Water Walking Potion", 50, 17)
        }
        .with_value(200)
        .with_rarity(Rarity::Blue)
        .with_buff_secs(600)
        .with_tooltip("Allows the ability to walk on water and lava"),
    ),
    (
        RECALL_POTION,
        ItemData {
            ..consumable("Recall Potion", 50, 17)
        }
        .with_value(200)
        .with_rarity(Rarity::Blue)
        .with_tooltip("Teleports you home to your spawn point"),
    ),
    (
        BOMB,
        ItemData {
            name: "Bomb",
            max_stack: 99,
            damage: 100,
            knock_back: 8.0,
            crit: 0,
            use_time: 25,
            use_animation: 25,
            pick: 0,
            axe: 0,
            hammer: 0,
            create_tile: None,
            class: ItemClass::Consumable,
            scale: 1.0,
            defense: 0,
            armor_slot: None,
            shoot_speed: 5.0,
            mana: 0,
            auto_reuse: false,
            value: 300,
            rarity: Rarity::White,
            tooltip: "A small explosion that will destroy most tiles",
            buff_secs: 0,
        },
    ),
    (
        DYNAMITE,
        ItemData {
            name: "Dynamite",
            max_stack: 99,
            damage: 250,
            knock_back: 10.0,
            crit: 0,
            use_time: 40,
            use_animation: 40,
            pick: 0,
            axe: 0,
            hammer: 0,
            create_tile: None,
            class: ItemClass::Consumable,
            scale: 1.0,
            defense: 0,
            armor_slot: None,
            shoot_speed: 4.0,
            mana: 0,
            auto_reuse: false,
            value: 2000,
            rarity: Rarity::Blue,
            tooltip: "A large explosion that will destroy most tiles",
            buff_secs: 0,
        },
    ),
    (
        GRENADE,
        ItemData {
            name: "Grenade",
            max_stack: 99,
            damage: 60,
            knock_back: 8.0,
            crit: 4,
            use_time: 44,
            use_animation: 44,
            pick: 0,
            axe: 0,
            hammer: 0,
            create_tile: None,
            class: ItemClass::Consumable,
            scale: 1.0,
            defense: 0,
            armor_slot: None,
            shoot_speed: 5.5,
            mana: 0,
            auto_reuse: false,
            value: 75,
            rarity: Rarity::White,
            tooltip: "A small explosion that will not destroy tiles",
            buff_secs: 0,
        },
    ),
    (
        METEORITE_ITEM,
        ItemData {
            ..block("Meteorite", METEORITE_ORE)
        }
        .with_value(200)
        .with_rarity(Rarity::Blue)
        .with_tooltip("Warm to the touch"),
    ),
    (
        CLOUD_ITEM,
        ItemData {
            ..block("Cloud", CLOUD_BLOCK)
        }
        .with_value(5)
        .with_rarity(Rarity::White)
        .with_tooltip("Soft floating sky block"),
    ),
    (
        SUNPLATE_ITEM,
        ItemData {
            ..block("Sunplate Block", SUNPLATE_BLOCK)
        }
        .with_value(10)
        .with_rarity(Rarity::White)
        .with_tooltip("Ornate block from a Floating Island"),
    ),
    (
        HIVE_ITEM,
        ItemData {
            ..block("Hive", HIVE_BLOCK)
        }
        .with_value(5)
        .with_rarity(Rarity::White)
        .with_tooltip("Sticky honeycomb wall from a Jungle Hive"),
    ),
    (
        CAMPFIRE_ITEM,
        ItemData {
            class: ItemClass::Placeable,
            ..block("Campfire", CAMPFIRE)
        }
        .with_value(100)
        .with_rarity(Rarity::White)
        .with_tooltip("Life regen is increased when near a campfire"),
    ),
    (
        HEART_LANTERN_ITEM,
        ItemData {
            class: ItemClass::Placeable,
            ..block("Heart Lantern", HEART_LANTERN)
        }
        .with_value(15000)
        .with_rarity(Rarity::Green)
        .with_tooltip("Increases life regeneration when placed nearby"),
    ),
    (
        TINKERERS_WORKSHOP_ITEM,
        ItemData {
            class: ItemClass::Placeable,
            max_stack: 99,
            ..block("Tinkerer's Workshop", TINKERERS_WORKSHOP)
        }
        .with_value(100000)
        .with_rarity(Rarity::Blue)
        .with_tooltip("Used to combine accessories"),
    ),
];

static UNKNOWN: ItemData = ItemData {
    name: "Unknown",
    max_stack: 1,
    damage: 0,
    knock_back: 0.0,
    crit: 0,
    use_time: 100,
    use_animation: 100,
    pick: 0,
    axe: 0,
    hammer: 0,
    create_tile: None,
    class: ItemClass::Other,
    scale: 1.0,
    defense: 0,
    armor_slot: None,
    shoot_speed: 0.0,
    mana: 0,
    auto_reuse: false,
    value: 0,
    rarity: Rarity::White,
    tooltip: "Unknown item",
    buff_secs: 0,
};

/// Every item id with data.
pub fn all_item_ids() -> impl Iterator<Item = ItemId> {
    ITEMS.iter().map(|(i, _)| *i)
}

/// Whether the item table has an entry for `id`.
pub fn item_exists(id: ItemId) -> bool {
    ITEMS.iter().any(|(i, _)| *i == id)
}

pub fn item_data(id: ItemId) -> &'static ItemData {
    ITEMS
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, d)| d)
        .unwrap_or(&UNKNOWN)
}

/// Deterministic SplitMix64, shared by tests and the server.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }
    pub fn next_u32(&mut self) -> u32 {
        // SplitMix64 (Steele, Lea, Flood 2014).
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (z ^ (z >> 31)) as u32
    }
    /// Uniform-ish roll in `0..n` (`n > 0`); every `rng % n` in the plans.
    pub fn below(&mut self, n: u32) -> u32 {
        self.next_u32() % n
    }
}

#[cfg(test)]
mod tests {
    use super::TORCH;
    use super::*;
    use crate::block::*;

    #[test]
    fn muramasa_auto_swings_with_fast_speed() {
        let d = item_data(MURAMASA);
        assert_eq!(MURAMASA.0, 155);
        assert_eq!(d.name, "Muramasa");
        assert_eq!(d.class, ItemClass::Melee);
        assert_eq!(d.damage, 24);
        assert_eq!(d.knock_back, 3.0);
        assert_eq!(d.use_time, 18);
        assert_eq!(d.use_animation, 18);
        assert!(d.auto_reuse);
    }

    #[test]
    fn wooden_sword_matches_spec() {
        // gameplay spec
        let d = item_data(WOODEN_SWORD);
        assert_eq!(WOODEN_SWORD.0, 24);
        assert_eq!(d.class, ItemClass::Melee);
        assert_eq!((d.damage, d.use_time, d.use_animation), (7, 20, 20));
        assert_eq!((d.knock_back, d.crit, d.max_stack), (5.0, 4, 1));
    }

    #[test]
    fn broadsword_stats_match_spec() {
        // gameplay spec: damage 9, useTime 20, useAnimation 21, knockBack 5.5, crit 4
        let b = item_data(COPPER_BROADSWORD);
        assert_eq!(b.class, ItemClass::Melee);
        assert_eq!(
            (
                b.damage,
                b.use_time,
                b.use_animation,
                b.knock_back,
                b.crit,
                b.max_stack
            ),
            (9, 20, 21, 5.5, 4, 1)
        );
    }

    #[test]
    fn gel_and_coin_ids_match_terraria() {
        assert_eq!(COPPER_BROADSWORD.0, 3508);
        assert_eq!(GEL.0, 23);
        let coins = [COPPER_COIN, SILVER_COIN, GOLD_COIN, PLATINUM_COIN];
        assert_eq!(coins.map(|c| c.0), [71, 72, 73, 74]);
        assert_eq!(item_data(GEL).max_stack, 9999);
        for c in coins {
            assert_eq!(item_data(c).max_stack, 100);
            assert_eq!(item_data(c).class, ItemClass::Other);
            assert!(item_data(c).create_tile.is_none());
        }
    }

    #[test]
    fn wall_item_matches_terraria() {
        assert_eq!(WOOD_WALL, ItemId(93));
        let d = item_data(WOOD_WALL);
        assert_eq!((d.class, d.create_tile), (ItemClass::Wall, None));
        assert_eq!((d.use_time, d.use_animation), (7, 15));
    }

    #[test]
    fn all_item_ids_includes_new_loot() {
        let ids: Vec<_> = all_item_ids().collect();
        assert_eq!(ids.len(), 172);
        for id in [
            BUG_NET,
            BUNNY_ITEM,
            BIRD_ITEM,
            SQUIRREL_ITEM,
            CAVE_CRITTER_ITEM,
            COPPER_BROADSWORD,
            GEL,
            COPPER_COIN,
            SILVER_COIN,
            GOLD_COIN,
            PLATINUM_COIN,
            ItemId(86),
            WORM_FOOD,
            DEMONITE_BAR,
            DEMON_BOW,
            WAR_AXE_OF_THE_NIGHT,
            LIGHTS_BANE,
            SHADOW_GREAVES,
            SHADOW_SCALEMAIL,
            SHADOW_HELMET,
            NIGHTMARE_PICKAXE,
            FALLEN_STAR,
            MANA_CRYSTAL,
            WAND_OF_SPARKING,
            HERMES_BOOTS,
            CLOUD_IN_A_BOTTLE,
            BAND_OF_REGENERATION,
            MAGIC_MIRROR,
            ENCHANTED_BOOMERANG,
            SPEAR,
            SHINY_RED_BALLOON,
            LUCKY_HORSESHOE,
            WOODEN_HAMMER,
            THE_BREAKER,
            VILETHORN,
            FLINTLOCK_PISTOL,
            MUSKET,
            MINISHARK,
            MUSKET_BALL,
            BAND_OF_STARPOWER,
            SHADOW_ORB_ITEM,
            BALL_O_HURT,
            BONE,
            GOLDEN_KEY,
            MAGIC_MISSILE,
            MURAMASA,
            COBALT_SHIELD,
            AQUA_SCEPTER,
            BLUE_MOON,
            HANDGUN,
            SHADOW_KEY,
            GUIDE_VOODOO_DOLL,
            VALOR,
            OBSIDIAN_SKULL,
            HELLSTONE_BAR,
            MOLTEN_HELMET,
            MOLTEN_BREASTPLATE,
            MOLTEN_GREAVES,
            MOLTEN_PICKAXE,
            MOLTEN_HAMAXE,
            FIERY_GREATSWORD,
            MOLTEN_FURY,
            PHOENIX_BLASTER,
            FLAMARANG,
        ] {
            assert!(ids.contains(&id));
        }
    }

    #[test]
    fn hammers_match_spec() {
        // gameplay spec (case 196): Wooden Hammer
        assert_eq!(WOODEN_HAMMER, ItemId(196));
        let wh = item_data(WOODEN_HAMMER);
        assert_eq!(wh.name, "Wooden Hammer");
        assert_eq!(wh.class, ItemClass::Tool);
        assert_eq!(wh.damage, 2);
        assert_eq!(wh.knock_back, 5.5);
        assert_eq!(wh.use_time, 25);
        assert_eq!(wh.use_animation, 37);
        assert_eq!(wh.hammer, 25);

        // gameplay spec (case 104): The Breaker
        assert_eq!(THE_BREAKER, ItemId(104));
        let tb = item_data(THE_BREAKER);
        assert_eq!(tb.name, "The Breaker");
        assert_eq!(tb.class, ItemClass::Tool);
        assert_eq!(tb.damage, 24);
        assert_eq!(tb.knock_back, 6.0);
        assert_eq!(tb.use_time, 19);
        assert_eq!(tb.use_animation, 45);
        assert_eq!(tb.hammer, 55);
    }

    #[test]
    fn worm_food_registered_with_demon_altar_recipe() {
        // gameplay spec. gameplay spec: consumable, useTime =
        // useAnimation = 45, no other stats. Crafted at a Demon Altar from
        // obtainable Corruption materials (Shadow Scale + Ebonstone).
        assert_eq!(WORM_FOOD, ItemId(70));
        assert!(
            all_item_ids().any(|id| id == WORM_FOOD),
            "id 70 in the table"
        );
        let d = item_data(WORM_FOOD);
        assert_eq!(d.name, "Worm Food");
        assert_eq!(d.class, ItemClass::Consumable);
        assert_eq!((d.use_time, d.use_animation), (45, 45));
        assert_eq!(d.max_stack, 9999);
        assert_eq!(d.create_tile, None);
        assert!(
            crate::crafting::recipes()
                .iter()
                .any(|r| r.output.item == WORM_FOOD
                    && r.station == Some(crate::block::DEMON_ALTAR))
        );
    }

    #[test]
    fn eater_shadow_scale_item_matches_terraria() {
        assert_eq!(SHADOW_SCALE, ItemId(86));
        assert!(
            all_item_ids().any(|id| id == SHADOW_SCALE),
            "included in client icon loading"
        );
        let d = item_data(SHADOW_SCALE);
        assert_eq!(d.name, "Shadow Scale");
        assert_eq!(d.max_stack, 9999);
        assert_eq!((d.class, d.create_tile), (ItemClass::Other, None));
    }

    #[test]
    fn copper_tools_match_spec() {
        // Copper Pickaxe (3509): knockBack 2, crit default 4
        let p = item_data(COPPER_PICKAXE);
        assert_eq!(
            (
                p.damage,
                p.use_time,
                p.use_animation,
                p.pick,
                p.axe,
                p.knock_back
            ),
            (4, 15, 23, 35, 0, 2.0)
        );
        // Copper Shortsword (3507): knockBack 4
        let s = item_data(COPPER_SHORTSWORD);
        assert_eq!(
            (
                s.damage,
                s.use_time,
                s.use_animation,
                s.pick,
                s.axe,
                s.knock_back
            ),
            (5, 13, 13, 0, 0, 4.0)
        );
        // Copper Axe (3506): knockBack 4.5
        let a = item_data(COPPER_AXE);
        assert_eq!(
            (
                a.damage,
                a.use_time,
                a.use_animation,
                a.pick,
                a.axe,
                a.knock_back
            ),
            (3, 21, 30, 0, 7, 4.5)
        );
    }

    #[test]
    fn wood_is_a_placeable_block_item() {
        // Item.item defaults case 9, gameplay spec.
        let w = item_data(WOOD);
        assert_eq!(w.name, "Wood");
        assert_eq!(w.max_stack, 9999);
        assert_eq!(w.use_time, 10);
        assert_eq!(w.use_animation, 15);
        assert_eq!(w.create_tile, Some(WOOD_BLOCK));
        assert_eq!(w.class, ItemClass::Block);
        assert_eq!(WOOD_BLOCK.drop_item(), Some(WOOD));
    }

    #[test]
    fn trunk_has_no_item() {
        assert_eq!(TRUNK.drop_item(), None);
        assert!(all_item_ids().all(|i| item_data(i).create_tile != Some(TRUNK)));
    }

    #[test]
    fn blocks_have_create_tile() {
        // KillTile_GetItemDrops: each tile drops the item whose createTile is that tile.
        for (item, tile) in [
            (DIRT_BLOCK, DIRT),
            (STONE_BLOCK, STONE),
            (IRON_ORE, IRON),
            (COPPER_ORE, COPPER),
            (GOLD_ORE, GOLD),
            (SILVER_ORE, SILVER),
            (SAND_BLOCK, SAND),
            (MUD_BLOCK, MUD),
            (SNOW_BLOCK, SNOW),
            (ICE_BLOCK, ICE),
            (SANDSTONE_BLOCK, SANDSTONE),
            (HARDENED_SAND_BLOCK, HARDENED_SAND),
            (WOOD, WOOD_BLOCK),
        ] {
            let d = item_data(item);
            assert_eq!(d.create_tile, Some(tile));
            assert_eq!(d.class, ItemClass::Block);
        }
        assert_eq!(item_data(TORCH).create_tile, Some(BlockId(4)));
    }

    #[test]
    fn tier_tools_match_spec() {
        // (id, damage, kb, useTime, useAnimation, pick, axe, scale, gameplay spec case line)
        let rows = [
            (IRON_PICKAXE, 5, 2.0, 13, 20, 40, 0, 1.0),      // :1392
            (IRON_BROADSWORD, 12, 5.5, 20, 20, 0, 0, 1.0),   // :1429
            (IRON_AXE, 5, 4.5, 19, 27, 0, 9, 1.0),           // :1515
            (SILVER_AXE, 6, 4.5, 18, 26, 0, 10, 1.0),        // :31369
            (SILVER_BROADSWORD, 14, 6.0, 20, 20, 0, 0, 1.0), // :31339
            (SILVER_PICKAXE, 6, 2.0, 11, 19, 45, 0, 1.05),   // :31329
            (GOLD_AXE, 7, 4.5, 18, 26, 0, 11, 1.0),          // :31311
            (GOLD_BROADSWORD, 15, 6.5, 18, 18, 0, 0, 1.0),   // :31280
            (GOLD_PICKAXE, 6, 2.0, 17, 20, 55, 0, 1.05),     // :31270
        ];
        for (id, dmg, kb, ut, ua, pick, axe, scale) in rows {
            let d = item_data(id);
            assert_eq!(
                (
                    d.damage,
                    d.knock_back,
                    d.use_time,
                    d.use_animation,
                    d.pick,
                    d.axe,
                    d.scale
                ),
                (dmg, kb, ut, ua, pick, axe, scale),
                "{}",
                d.name
            );
            assert_eq!((d.crit, d.max_stack), (4, 1));
        }
        assert_eq!(
            [
                IRON_PICKAXE.0,
                IRON_BROADSWORD.0,
                IRON_AXE.0,
                SILVER_AXE.0,
                SILVER_PICKAXE.0,
                SILVER_BROADSWORD.0,
                GOLD_AXE.0,
                GOLD_PICKAXE.0,
                GOLD_BROADSWORD.0
            ],
            [1, 4, 10, 3512, 3515, 3514, 3518, 3521, 3520]
        );
    }

    #[test]
    fn armor_ids_slots_and_defense_match_spec() {
        // (id, slot, defense, gameplay spec case line); order head, body, legs per metal
        use ArmorSlot::*;
        let rows = [
            (COPPER_HELMET, 89, Head, 1, 2428),
            (COPPER_CHAINMAIL, 80, Body, 2, 2344),
            (COPPER_GREAVES, 76, Legs, 1, 2316),
            (IRON_HELMET, 90, Head, 2, 2435),
            (IRON_CHAINMAIL, 81, Body, 3, 2351),
            (IRON_GREAVES, 77, Legs, 2, 2323),
            (SILVER_HELMET, 91, Head, 3, 2442),
            (SILVER_CHAINMAIL, 82, Body, 4, 2358),
            (SILVER_GREAVES, 78, Legs, 3, 2330),
            (GOLD_HELMET, 92, Head, 4, 2449),
            (GOLD_CHAINMAIL, 83, Body, 5, 2365),
            (GOLD_GREAVES, 79, Legs, 4, 2337),
            (SHADOW_HELMET, 102, Head, 6, 2456),
            (SHADOW_SCALEMAIL, 101, Body, 7, 2463),
            (SHADOW_GREAVES, 100, Legs, 6, 2470),
        ];
        for (id, n, slot, def, _line) in rows {
            assert_eq!(id.0, n);
            let d = item_data(id);
            assert_eq!(
                (d.class, d.armor_slot, d.defense),
                (ItemClass::Armor, Some(slot), def),
                "{}",
                d.name
            );
        }
    }

    #[test]
    fn armor_is_not_stackable() {
        for id in all_item_ids().filter(|i| item_data(*i).class == ItemClass::Armor) {
            assert_eq!(item_data(id).max_stack, 1);
            assert!(item_data(id).create_tile.is_none());
        }
    }

    #[test]
    fn existing_items_have_zero_defense() {
        for id in all_item_ids().filter(|i| {
            item_data(*i).class != ItemClass::Armor && *i != COBALT_SHIELD && *i != OBSIDIAN_SKULL
        }) {
            assert_eq!(item_data(id).defense, 0);
            assert_eq!(item_data(id).armor_slot, None);
        }
    }

    #[test]
    fn star_mana_ids_match_terraria() {
        // gameplay spec (75), :1764 (109), :7684 (3069).
        assert_eq!(
            [FALLEN_STAR.0, MANA_CRYSTAL.0, WAND_OF_SPARKING.0],
            [75, 109, 3069]
        );

        // gameplay spec: consumable, useStyle 4, useTime/useAnimation 17, ammo FallenStar.
        let star = item_data(FALLEN_STAR);
        assert_eq!(star.name, "Fallen Star");
        assert_eq!((star.use_time, star.use_animation), (17, 17));
        assert_eq!(star.max_stack, 9999);

        // gameplay spec: consumable, useStyle 4, useTime/useAnimation 30.
        let crystal = item_data(MANA_CRYSTAL);
        assert_eq!(crystal.name, "Mana Crystal");
        assert_eq!(crystal.class, ItemClass::Consumable);
        assert_eq!((crystal.use_time, crystal.use_animation), (30, 30));
        assert_eq!(crystal.max_stack, 9999);

        // gameplay spec: damage 14, mana 2, useTime/useAnimation 26,
        // shootSpeed 7, crit 10, magic, noMelee.
        let wand = item_data(WAND_OF_SPARKING);
        assert_eq!(wand.name, "Wand of Sparking");
        assert_eq!(wand.class, ItemClass::Magic);
        assert_eq!(
            (
                wand.damage,
                wand.mana,
                wand.use_time,
                wand.use_animation,
                wand.crit,
                wand.shoot_speed,
                wand.max_stack,
            ),
            (14, 2, 26, 26, 10, 7.0, 1)
        );
    }

    #[test]
    fn m6_item_ids_and_classes_match_spec() {
        assert_eq!(BAND_OF_REGENERATION, ItemId(49));
        assert_eq!(MAGIC_MIRROR, ItemId(50));
        assert_eq!(CLOUD_IN_A_BOTTLE, ItemId(53));
        assert_eq!(HERMES_BOOTS, ItemId(54));
        assert_eq!(ENCHANTED_BOOMERANG, ItemId(55));
        assert_eq!(LUCKY_HORSESHOE, ItemId(158));
        assert_eq!(SHINY_RED_BALLOON, ItemId(159));
        assert_eq!(SPEAR, ItemId(274));

        for id in [
            BAND_OF_REGENERATION,
            CLOUD_IN_A_BOTTLE,
            HERMES_BOOTS,
            LUCKY_HORSESHOE,
            SHINY_RED_BALLOON,
        ] {
            let d = item_data(id);
            assert_eq!(d.class, ItemClass::Accessory);
            assert_eq!(d.max_stack, 1);
        }
        assert_eq!(item_data(ENCHANTED_BOOMERANG).max_stack, 1);
        assert_eq!(item_data(SPEAR).max_stack, 1);
        assert_eq!(item_data(MAGIC_MIRROR).class, ItemClass::Other);
    }

    #[test]
    fn all_items_have_tooltip_and_non_coin_value() {
        for id in all_item_ids() {
            let data = item_data(id);
            assert!(
                !data.tooltip.is_empty(),
                "item {:?} ({}) must have a non-empty tooltip",
                id,
                data.name
            );
            if !is_coin(id) {
                assert!(
                    data.value > 0,
                    "item {:?} ({}) must have value > 0",
                    id,
                    data.name
                );
            }
        }
    }

    #[test]
    fn molten_item_ids_and_stats_match_wiki() {
        assert_eq!(item_data(MOLTEN_PICKAXE).name, "Molten Pickaxe");
        assert_eq!(
            (
                item_data(MOLTEN_PICKAXE).damage,
                item_data(MOLTEN_PICKAXE).use_time,
                item_data(MOLTEN_PICKAXE).pick
            ),
            (12, 23, 100)
        );
        assert_eq!(
            (
                item_data(MOLTEN_HAMAXE).damage,
                item_data(MOLTEN_HAMAXE).axe,
                item_data(MOLTEN_HAMAXE).hammer
            ),
            (20, 30, 70)
        );
        assert_eq!(
            (
                item_data(FIERY_GREATSWORD).damage,
                item_data(FIERY_GREATSWORD).use_time
            ),
            (40, 40)
        );
        assert_eq!(
            (
                item_data(MOLTEN_FURY).damage,
                item_data(MOLTEN_FURY).use_time
            ),
            (31, 22)
        );
        assert_eq!(
            (
                item_data(PHOENIX_BLASTER).damage,
                item_data(PHOENIX_BLASTER).use_time,
                item_data(PHOENIX_BLASTER).auto_reuse
            ),
            (33, 11, false)
        );
        assert_eq!(
            (item_data(FLAMARANG).damage, item_data(FLAMARANG).use_time),
            (49, 14)
        );
        assert_eq!(
            (
                item_data(MOLTEN_HELMET).defense,
                item_data(MOLTEN_BREASTPLATE).defense,
                item_data(MOLTEN_GREAVES).defense
            ),
            (8, 9, 8)
        );
        assert_eq!(item_data(HELLSTONE_BAR).max_stack, 9999);

        // Terraria ids from the spec table.
        assert_eq!(
            [
                HELLSTONE_BAR.0,
                MOLTEN_HELMET.0,
                MOLTEN_BREASTPLATE.0,
                MOLTEN_GREAVES.0,
                MOLTEN_PICKAXE.0,
                MOLTEN_HAMAXE.0,
                FIERY_GREATSWORD.0,
                MOLTEN_FURY.0,
                PHOENIX_BLASTER.0,
                FLAMARANG.0,
            ],
            [175, 231, 232, 233, 122, 217, 121, 120, 219, 119]
        );

        // Classes and shot speeds.
        use ItemClass::*;
        assert_eq!(
            [
                item_data(MOLTEN_PICKAXE).class,
                item_data(MOLTEN_HAMAXE).class,
                item_data(FIERY_GREATSWORD).class,
                item_data(MOLTEN_FURY).class,
                item_data(PHOENIX_BLASTER).class,
                item_data(FLAMARANG).class,
                item_data(HELLSTONE_BAR).class,
            ],
            [Tool, Tool, Melee, Ranged, Ranged, Melee, Other]
        );
        assert_eq!(item_data(MOLTEN_FURY).shoot_speed, 8.5);
        assert_eq!(item_data(PHOENIX_BLASTER).shoot_speed, 13.0);
        assert_eq!(
            (
                item_data(MOLTEN_HELMET).armor_slot,
                item_data(MOLTEN_BREASTPLATE).armor_slot,
                item_data(MOLTEN_GREAVES).armor_slot
            ),
            (
                Some(ArmorSlot::Head),
                Some(ArmorSlot::Body),
                Some(ArmorSlot::Legs)
            )
        );

        // Orange rarity and the sell values from the spec.
        for id in [
            HELLSTONE_BAR,
            MOLTEN_HELMET,
            MOLTEN_BREASTPLATE,
            MOLTEN_GREAVES,
            MOLTEN_PICKAXE,
            MOLTEN_HAMAXE,
            FIERY_GREATSWORD,
            MOLTEN_FURY,
            PHOENIX_BLASTER,
            FLAMARANG,
        ] {
            assert_eq!(
                item_data(id).rarity,
                Rarity::Orange,
                "{}",
                item_data(id).name
            );
            assert_eq!(
                all_item_ids().filter(|i| *i == id).count(),
                1,
                "{} registered once",
                item_data(id).name
            );
        }
        assert_eq!(item_data(HELLSTONE_BAR).value, 4000);
        assert_eq!(item_data(MOLTEN_HELMET).value, 9000);
        assert_eq!(item_data(MOLTEN_BREASTPLATE).value, 6000);
        assert_eq!(item_data(MOLTEN_GREAVES).value, 6000);
        for id in [
            MOLTEN_PICKAXE,
            MOLTEN_HAMAXE,
            FIERY_GREATSWORD,
            MOLTEN_FURY,
            PHOENIX_BLASTER,
            FLAMARANG,
        ] {
            assert_eq!(item_data(id).value, 27000, "{}", item_data(id).name);
        }

        // The Phoenix Blaster takes bullets; the other weapons stay arrow/melee.
        assert!(matches_ammo(PHOENIX_BLASTER, MUSKET_BALL));
        assert!(!matches_ammo(PHOENIX_BLASTER, WOODEN_ARROW));
    }
}

#[cfg(test)]
mod demonite_tests {
    use super::*;
    use crate::block::DEMONITE_ORE;
    use crate::crafting::*;
    use crate::interact::min_pick;

    #[test]
    fn demonite_ids_match_terraria() {
        assert_eq!(DEMONITE_BAR, ItemId(57));
        assert_eq!(DEMON_BOW, ItemId(44));
        assert_eq!(WAR_AXE_OF_THE_NIGHT, ItemId(45));
        assert_eq!(LIGHTS_BANE, ItemId(46));
        assert_eq!(SHADOW_GREAVES, ItemId(100));
        assert_eq!(SHADOW_SCALEMAIL, ItemId(101));
        assert_eq!(SHADOW_HELMET, ItemId(102));
        assert_eq!(NIGHTMARE_PICKAXE, ItemId(103));
    }

    #[test]
    fn demonite_bar_recipe_matches_spec() {
        let r = recipe(DEMONITE_BAR_RECIPE).expect("demonite bar recipe");
        assert_eq!(
            r.output,
            Ingredient {
                item: DEMONITE_BAR,
                count: 1
            }
        );
        assert_eq!(
            r.ingredients,
            &[Ingredient {
                item: DEMONITE_ORE_ITEM,
                count: 3
            }]
        );
        assert_eq!(r.station, Some(crate::block::FURNACE));
    }

    #[test]
    fn nightmare_pickaxe_stats_match() {
        let d = item_data(NIGHTMARE_PICKAXE);
        assert_eq!(d.class, ItemClass::Tool);
        assert_eq!((d.damage, d.use_time, d.use_animation), (9, 15, 20));
        assert_eq!((d.pick, d.axe, d.knock_back, d.scale), (65, 0, 3.0, 1.0));
    }

    #[test]
    fn nightmare_pickaxe_mines_demonite() {
        let pick = item_data(NIGHTMARE_PICKAXE).pick;
        assert_eq!(pick, 65);
        assert!(pick >= min_pick(DEMONITE_ORE));
    }

    #[test]
    fn shadow_armor_defense_sum() {
        // Shadow armor: Helmet 6, Scalemail 7, Greaves 6 = total 19.
        // Set bonus (7.5% melee speed) skipped per design ruling (defense only).
        let h = item_data(SHADOW_HELMET);
        let b = item_data(SHADOW_SCALEMAIL);
        let l = item_data(SHADOW_GREAVES);
        assert_eq!(h.armor_slot, Some(ArmorSlot::Head));
        assert_eq!(b.armor_slot, Some(ArmorSlot::Body));
        assert_eq!(l.armor_slot, Some(ArmorSlot::Legs));
        assert_eq!(h.defense, 6);
        assert_eq!(b.defense, 7);
        assert_eq!(l.defense, 6);
        assert_eq!(h.defense + b.defense + l.defense, 19);
    }

    #[test]
    fn m11_item_ids_and_stats_match_wiki() {
        assert_eq!(PWNHAMMER, ItemId(367));
        assert_eq!(BREAKER_BLADE, ItemId(426));
        assert_eq!(CLOCKWORK_ASSAULT_RIFLE, ItemId(434));
        assert_eq!(SORCERER_EMBLEM, ItemId(489));
        assert_eq!(WARRIOR_EMBLEM, ItemId(490));
        assert_eq!(RANGER_EMBLEM, ItemId(491));
        assert_eq!(LASER_RIFLE, ItemId(514));
        assert_eq!(FIRECRACKER, ItemId(4912));

        let pwn = item_data(PWNHAMMER);
        assert_eq!(pwn.class, ItemClass::Tool);
        assert_eq!(
            (pwn.damage, pwn.hammer, pwn.use_time, pwn.auto_reuse),
            (26, 80, 14, true)
        );
        assert_eq!((pwn.rarity, pwn.value), (Rarity::LightRed, 39000));

        let bb = item_data(BREAKER_BLADE);
        assert_eq!(bb.class, ItemClass::Melee);
        assert_eq!((bb.damage, bb.use_time, bb.crit), (70, 35, 4));
        assert_eq!((bb.rarity, bb.value), (Rarity::LightRed, 150000));

        let car = item_data(CLOCKWORK_ASSAULT_RIFLE);
        assert_eq!(car.class, ItemClass::Ranged);
        assert_eq!((car.damage, car.use_time, car.auto_reuse), (17, 12, true));
        assert_eq!((car.rarity, car.value), (Rarity::LightRed, 150000));
        assert!(matches_ammo(CLOCKWORK_ASSAULT_RIFLE, MUSKET_BALL));

        let lr = item_data(LASER_RIFLE);
        assert_eq!(lr.class, ItemClass::Magic);
        assert_eq!(
            (lr.damage, lr.mana, lr.use_time, lr.auto_reuse),
            (29, 8, 12, true)
        );
        assert_eq!((lr.rarity, lr.value), (Rarity::LightRed, 150000));

        let fc = item_data(FIRECRACKER);
        assert_eq!(fc.class, ItemClass::Melee);
        assert_eq!((fc.damage, fc.use_time), (40, 30));
        assert_eq!((fc.rarity, fc.value), (Rarity::LightRed, 150000));

        for emb in [WARRIOR_EMBLEM, RANGER_EMBLEM, SORCERER_EMBLEM] {
            let d = item_data(emb);
            assert_eq!(d.class, ItemClass::Accessory);
            assert_eq!((d.rarity, d.value), (Rarity::LightRed, 100000));
        }
    }
}
