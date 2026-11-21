//! Block identifiers. The numeric value is Terraria's tile id.

/// A block type; the inner value is Terraria's tile id (`u16::MAX` = air).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct BlockId(pub u16);

pub const AIR: BlockId = BlockId(u16::MAX);
pub const DIRT: BlockId = BlockId(0);
pub const STONE: BlockId = BlockId(1);
pub const GRASS: BlockId = BlockId(2);
pub const IRON: BlockId = BlockId(6);
pub const COPPER: BlockId = BlockId(7);
pub const GOLD: BlockId = BlockId(8);
pub const SILVER: BlockId = BlockId(9);
pub const TORCH: BlockId = BlockId(4);
/// Tree trunk, `TileID.Tree` (gameplay spec).
pub const TRUNK: BlockId = BlockId(5);
pub const SAND: BlockId = BlockId(53);
pub const MUD: BlockId = BlockId(59);
pub const JUNGLE_GRASS: BlockId = BlockId(60);
/// Corruption set: `TileID.Demonite` (gameplay spec),
/// `TileID.CorruptGrass` (`:483`), `TileID.Ebonstone` (`:487`).
pub const DEMONITE_ORE: BlockId = BlockId(22);
pub const CORRUPT_GRASS: BlockId = BlockId(23);
pub const EBONSTONE: BlockId = BlockId(25);
/// `TileID.DemonAltar`, gameplay spec. Solid and opaque; every
/// six cells of one altar carry this tile. `drop_item` is `None` (nothing is
/// ever mined from it) and no item places it: altars are worldgen only.
pub const DEMON_ALTAR: BlockId = BlockId(26);
/// `TileID.ShadowOrbs`, gameplay spec. Solid and opaque cube.
/// Mined only with a hammer. Worldgen only.
pub const SHADOW_ORB: BlockId = BlockId(31);
/// TileID.Hellstone, gameplay spec.
pub const HELLSTONE: BlockId = BlockId(58);
pub const SNOW: BlockId = BlockId(147);
pub const ICE: BlockId = BlockId(161);
/// Wood block placed by the Wood item (`Item.item defaults` case 9,
/// gameplay spec).
pub const WOOD_BLOCK: BlockId = BlockId(30);
pub const SANDSTONE: BlockId = BlockId(396);
pub const HARDENED_SAND: BlockId = BlockId(397);
/// TileID.WorkBenches, gameplay spec.
pub const WORKBENCH: BlockId = BlockId(18);
/// TileID.Anvils, gameplay spec.
pub const ANVIL: BlockId = BlockId(16);
/// TileID.Furnaces, gameplay spec.
pub const FURNACE: BlockId = BlockId(17);

/// TileID.Containers, gameplay spec. Solid cube here; contents live in the server.
pub const CHEST: BlockId = BlockId(21);

/// TileID.Heart (life crystal), gameplay spec. A solid cube here (vanilla is 2x2).
pub const LIFE_CRYSTAL: BlockId = BlockId(12);

// Housing furniture set (future housing checks use these ids). Single-cell blocks here.
/// TileID.ClosedDoor, gameplay spec. Solid.
pub const DOOR_CLOSED: BlockId = BlockId(10);
/// TileID.OpenDoor, gameplay spec. Passable.
pub const DOOR_OPEN: BlockId = BlockId(11);
/// TileID.Tables, gameplay spec. Decor, no collision.
pub const TABLE: BlockId = BlockId(14);
/// TileID.Chairs, gameplay spec. Decor, no collision.
pub const CHAIR: BlockId = BlockId(15);
/// TileID.Platforms, gameplay spec. One-way (see `player::collide`).
pub const PLATFORM: BlockId = BlockId(19);

/// TileID.Plants (surface mushroom is style 6), gameplay spec. Non-solid decor.
pub const MUSHROOM_PLANT: BlockId = BlockId(3);
/// TileID.Bottles, gameplay spec. Non-solid decor; the alchemy station.
pub const BOTTLE: BlockId = BlockId(13);
/// TileID.Glass, gameplay spec. Solid.
pub const GLASS_BLOCK: BlockId = BlockId(54);
/// TileID.Obsidian, gameplay spec. Solid.
pub const OBSIDIAN: BlockId = BlockId(56);

/// Dungeon bricks: TileID.BlueDungeonBrick (41), TileID.GreenDungeonBrick (43), TileID.PinkDungeonBrick (44).
pub const BLUE_DUNGEON_BRICK: BlockId = BlockId(41);
pub const GREEN_DUNGEON_BRICK: BlockId = BlockId(43);
pub const PINK_DUNGEON_BRICK: BlockId = BlockId(44);
/// TileID.Spikes, gameplay spec. Solid.
pub const SPIKES: BlockId = BlockId(48);

/// TileID.Ash, gameplay spec.
pub const ASH: BlockId = BlockId(57);
/// TileID.ObsidianBrick, gameplay spec.
pub const OBSIDIAN_BRICK: BlockId = BlockId(75);
/// TileID.HellstoneBrick, gameplay spec.
pub const HELLSTONE_BRICK: BlockId = BlockId(76);
/// TileID.Hellforge, gameplay spec.
pub const HELLFORGE: BlockId = BlockId(77);

/// TileID.Pots (28). Breakable container; drops coins/potions via server loot.
pub const POT: BlockId = BlockId(28);
/// TileID.Stalactite (165). Non-solid cave ceiling dressing.
pub const STALACTITE: BlockId = BlockId(165);
/// TileID.GreenMoss (179). Non-solid cave floor dressing.
pub const MOSS: BlockId = BlockId(179);
/// TileID.SmallPiles (185). Non-solid surface/cave rock dressing.
pub const SMALL_ROCK: BlockId = BlockId(185);
/// Cave shelf mushroom dressing (tile 71). Non-solid.
pub const SHELF_MUSHROOM: BlockId = BlockId(71);

/// What a physics `BlockQuery` returns for chunks that aren't loaded, so the
/// player never falls through unloaded terrain.
pub const SOLID_UNLOADED: BlockId = STONE;

impl BlockId {
    /// Every non-air block except torches and furniture is solid.
    /// Tree trunks (`TRUNK`, tile 5) are solid in 3D so players cannot walk through trees.
    /// Open doors, tables and chairs have no collision; platforms are one-way and handled
    /// by `player::collide` rather than here.
    pub fn is_solid(self) -> bool {
        !matches!(
            self,
            AIR | TORCH
                | DOOR_OPEN
                | TABLE
                | CHAIR
                | PLATFORM
                | MUSHROOM_PLANT
                | BOTTLE
                | POT
                | STALACTITE
                | MOSS
                | SMALL_ROCK
                | SHELF_MUSHROOM
        )
    }

    /// One-way platform (`TileID.Sets.Platforms`).
    pub fn is_platform(self) -> bool {
        self == PLATFORM
    }

    /// Blocks that hide the face of a neighbour: everything except air,
    /// torches, plants, bottles, and non-solid cave dressing/pots.
    /// A trunk is opaque (so it meshes and occludes) but non-solid.
    pub fn is_opaque(self) -> bool {
        !matches!(
            self,
            AIR | TORCH
                | MUSHROOM_PLANT
                | BOTTLE
                | POT
                | STALACTITE
                | MOSS
                | SMALL_ROCK
                | SHELF_MUSHROOM
        )
    }

    /// The item this tile drops when broken.
    pub fn drop_item(self) -> Option<crate::item::ItemId> {
        use crate::item as i;
        // WorldGen.KillTile_GetItemDrops
        Some(match self {
            DIRT | GRASS => i::DIRT_BLOCK,              // case 0, 2
            STONE => i::STONE_BLOCK,                    // case 1
            TORCH => i::TORCH,                          // tile 4 drops its torch item
            IRON => i::IRON_ORE,                        // case 6
            COPPER => i::COPPER_ORE,                    // case 7
            GOLD => i::GOLD_ORE,                        // case 8
            SILVER => i::SILVER_ORE,                    // case 9
            SAND => i::SAND_BLOCK,                      // case 53
            MUD | JUNGLE_GRASS => i::MUD_BLOCK,         // case 59, 60
            DEMONITE_ORE => i::DEMONITE_ORE_ITEM,       // case 22 (gameplay spec)
            CORRUPT_GRASS => i::DIRT_BLOCK,             // case 23 (gameplay spec)
            EBONSTONE => i::EBONSTONE_BLOCK,            // case 25 (gameplay spec)
            SNOW => i::SNOW_BLOCK,                      // case 147
            ICE => i::ICE_BLOCK,                        // createTile 161 (ItemID 664)
            SANDSTONE => i::SANDSTONE_BLOCK,            // createTile 396 (ItemID 3271)
            HARDENED_SAND => i::HARDENED_SAND_BLOCK,    // createTile 397 (ItemID 3272)
            WOOD_BLOCK => i::WOOD,                      // createTile 30 (ItemID 9)
            WORKBENCH => i::WORKBENCH_ITEM,             // createTile 18 (ItemID 36)
            ANVIL => i::ANVIL_ITEM,                     // createTile 16 (ItemID 35, gameplay spec)
            LIFE_CRYSTAL => i::LIFE_CRYSTAL_ITEM,       // case 12 (ItemID 29, gameplay spec)
            CHEST => i::CHEST_ITEM,                     // createTile 21 (ItemID 48, gameplay spec)
            DOOR_CLOSED | DOOR_OPEN => i::WOODEN_DOOR,  // createTile 10 (gameplay spec)
            TABLE => i::WOODEN_TABLE,                   // createTile 14 (gameplay spec)
            CHAIR => i::WOODEN_CHAIR,                   // createTile 15 (gameplay spec)
            PLATFORM => i::WOOD_PLATFORM,               // createTile 19 (gameplay spec)
            MUSHROOM_PLANT => i::MUSHROOM,              // surface mushroom plant (ItemID 5)
            BOTTLE => i::BOTTLE_ITEM,                   // createTile 13 (gameplay spec case 31)
            GLASS_BLOCK => i::GLASS,                    // createTile 54 (gameplay spec case 170)
            FURNACE => i::FURNACE_ITEM,                 // createTile 17 (ItemID 33, gameplay spec)
            OBSIDIAN => i::OBSIDIAN_ITEM,               // case 56 (ItemID 173)
            ASH => i::ASH_BLOCK,                        // case 57 (ItemID 172)
            HELLSTONE => i::HELLSTONE_ITEM,             // case 58 (ItemID 174)
            OBSIDIAN_BRICK => i::OBSIDIAN_BRICK_ITEM,   // case 75 (ItemID 192)
            HELLSTONE_BRICK => i::HELLSTONE_BRICK_ITEM, // case 76 (ItemID 214)
            HELLFORGE => i::HELLFORGE_ITEM,             // case 77 (ItemID 221)
            // TRUNK (tile 5) drops nothing; the axe grants Wood per block.
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::TORCH;
    use super::*;
    use crate::item::*;

    #[test]
    fn torch_not_solid() {
        assert!(!TORCH.is_solid());
        assert!(!AIR.is_solid());
        assert!(STONE.is_solid());
    }

    #[test]
    fn trunk_is_not_solid_but_opaque() {
        // Terraria's trees have no collision (`TileID.Sets.HasOutlines` aside).
        assert!(TRUNK.is_solid());
        assert!(TRUNK.is_opaque());
        assert!(STONE.is_solid());
        assert!(STONE.is_opaque());
        assert!(!TORCH.is_solid());
        assert!(!TORCH.is_opaque());
    }

    #[test]
    fn tree_tile_ids_match_terraria() {
        assert_eq!(TRUNK.0, 5); // TileID.Tree, Terraria/ID/gameplay spec
        assert_eq!(WOOD_BLOCK.0, 30); // gameplay spec createTile for item 9
    }

    #[test]
    fn drops_match_spec() {
        // WorldGen.KillTile_GetItemDrops
        assert_eq!(DIRT.drop_item(), Some(DIRT_BLOCK)); // case 0, 2
        assert_eq!(GRASS.drop_item(), Some(DIRT_BLOCK));
        assert_eq!(STONE.drop_item(), Some(STONE_BLOCK)); // case 1
        assert_eq!(IRON.drop_item(), Some(IRON_ORE));
        assert_eq!(COPPER.drop_item(), Some(COPPER_ORE));
        assert_eq!(GOLD.drop_item(), Some(GOLD_ORE));
        assert_eq!(SILVER.drop_item(), Some(SILVER_ORE));
        assert_eq!(SAND.drop_item(), Some(SAND_BLOCK)); // case 53
        assert_eq!(MUD.drop_item(), Some(MUD_BLOCK)); // case 59, 60
        assert_eq!(JUNGLE_GRASS.drop_item(), Some(MUD_BLOCK));
        assert_eq!(SNOW.drop_item(), Some(SNOW_BLOCK)); // case 147
        assert_eq!(ICE.drop_item(), Some(ICE_BLOCK));
        assert_eq!(SANDSTONE.drop_item(), Some(SANDSTONE_BLOCK));
        assert_eq!(HARDENED_SAND.drop_item(), Some(HARDENED_SAND_BLOCK));
        assert_eq!(TORCH.drop_item(), Some(crate::item::TORCH));
        assert_eq!(TRUNK.drop_item(), None);
        assert_eq!(WOOD_BLOCK.drop_item(), Some(crate::item::WOOD));
        assert_eq!(AIR.drop_item(), None);
    }

    #[test]
    fn altar_tile_matches_terraria() {
        // TileID.DemonAltar, gameplay spec. Vanilla builds it as a
        // 3x2 with WorldGen.Place3x2 (gameplay spec, altar pass
        // :15849-15910); here one group of six cells carries the tile.
        assert_eq!(DEMON_ALTAR.0, 26);
        assert!(DEMON_ALTAR.is_solid()); // collision like vanilla's altar tiles
        assert!(DEMON_ALTAR.is_opaque());
        // Worldgen-only: no item places tile 26 (DemonAltar item 5532 is
        // skipped, gameplay spec) and nothing is ever mined from it.
        assert_eq!(DEMON_ALTAR.drop_item(), None);
        for id in all_item_ids() {
            assert!(
                item_data(id).create_tile != Some(DEMON_ALTAR),
                "{id:?} places an altar"
            );
        }
    }

    #[test]
    fn corruption_ids_match_terraria() {
        // gameplay spec (Demonite Ore), :483 (Corrupt Grass), :487 (Ebonstone).
        assert_eq!([DEMONITE_ORE.0, CORRUPT_GRASS.0, EBONSTONE.0], [22, 23, 25]);
        // gameplay spec (Demonite Ore), :1668 (Ebonstone Block).
        assert_eq!([DEMONITE_ORE_ITEM.0, EBONSTONE_BLOCK.0], [56, 61]);
        for b in [DEMONITE_ORE, CORRUPT_GRASS, EBONSTONE] {
            assert!(b.is_solid() && b.is_opaque(), "{b:?}");
        }
        // gameplay spec (demonite, corrupt grass), :65267-65268 (ebonstone)
        assert_eq!(DEMONITE_ORE.drop_item(), Some(DEMONITE_ORE_ITEM));
        assert_eq!(CORRUPT_GRASS.drop_item(), Some(DIRT_BLOCK));
        assert_eq!(EBONSTONE.drop_item(), Some(EBONSTONE_BLOCK));
        // gameplay spec (case 56, createTile 22), :2100-2107 (case 61, createTile 25)
        assert_eq!(item_data(DEMONITE_ORE_ITEM).create_tile, Some(DEMONITE_ORE));
        assert_eq!(item_data(EBONSTONE_BLOCK).create_tile, Some(EBONSTONE));
    }

    #[test]
    fn shadow_orb_tile_matches_terraria() {
        // TileID.ShadowOrbs = 31 (gameplay spec)
        assert_eq!(SHADOW_ORB.0, 31);
        assert!(SHADOW_ORB.is_solid());
        assert!(SHADOW_ORB.is_opaque());
        assert_eq!(SHADOW_ORB.drop_item(), None);
        for id in all_item_ids() {
            assert!(
                item_data(id).create_tile != Some(SHADOW_ORB),
                "{id:?} places a shadow orb"
            );
        }
    }

    #[test]
    fn life_crystal_ids_match_terraria() {
        assert_eq!((LIFE_CRYSTAL.0, LIFE_CRYSTAL_ITEM.0), (12, 29)); // gameplay spec, gameplay spec
        assert!(LIFE_CRYSTAL.is_solid());
        assert_eq!(LIFE_CRYSTAL.drop_item(), Some(LIFE_CRYSTAL_ITEM));
        // Not player-placeable in this stage: no create_tile.
        assert_eq!(item_data(LIFE_CRYSTAL_ITEM).create_tile, None);
    }

    #[test]
    fn workbench_ids_match_terraria() {
        assert_eq!(WORKBENCH.0, 18); // TileID.WorkBenches, gameplay spec
        assert_eq!(WORKBENCH_ITEM.0, 36); // gameplay spec
        assert!(WORKBENCH.is_solid());
        assert_eq!(WORKBENCH.drop_item(), Some(WORKBENCH_ITEM));
        assert_eq!(item_data(WORKBENCH_ITEM).create_tile, Some(WORKBENCH));
    }

    #[test]
    fn chest_ids_match_terraria() {
        assert_eq!((CHEST.0, CHEST_ITEM.0), (21, 48)); // gameplay spec, gameplay spec
        assert!(CHEST.is_solid());
        assert_eq!(CHEST.drop_item(), Some(CHEST_ITEM));
        assert_eq!(item_data(CHEST_ITEM).create_tile, Some(CHEST)); // gameplay spec
    }

    #[test]
    fn glass_and_bottle_ids_match_terraria() {
        // gameplay spec: Mushroom 5, Bottle 31, Glass 170; gameplay spec,:545.
        assert_eq!((MUSHROOM.0, BOTTLE_ITEM.0, GLASS.0), (5, 31, 170));
        assert_eq!((BOTTLE.0, GLASS_BLOCK.0), (13, 54));
        assert!(GLASS_BLOCK.is_solid() && !BOTTLE.is_solid() && !MUSHROOM_PLANT.is_solid());
        assert_eq!(GLASS_BLOCK.drop_item(), Some(GLASS));
        assert_eq!(BOTTLE.drop_item(), Some(BOTTLE_ITEM));
        assert_eq!(MUSHROOM_PLANT.drop_item(), Some(MUSHROOM));
        // gameplay spec case 31 / 170: createTile 13 / 54.
        assert_eq!(item_data(BOTTLE_ITEM).create_tile, Some(BOTTLE));
        assert_eq!(item_data(GLASS).create_tile, Some(GLASS_BLOCK));
    }

    #[test]
    fn mushroom_gather_gives_item() {
        // Breaking the plant yields exactly the Mushroom item (ItemID 5).
        assert_eq!(MUSHROOM_PLANT.drop_item(), Some(MUSHROOM));
        assert_eq!(item_data(MUSHROOM).name, "Mushroom");
    }

    #[test]
    fn furniture_ids_match_terraria() {
        assert_eq!(
            [DOOR_CLOSED.0, DOOR_OPEN.0, TABLE.0, CHAIR.0, PLATFORM.0],
            [10, 11, 14, 15, 19]
        ); // gameplay spec,:459,:465,:467,:475
        assert_eq!(
            [
                WOODEN_DOOR.0,
                WOODEN_TABLE.0,
                WOODEN_CHAIR.0,
                WOOD_PLATFORM.0
            ],
            [25, 32, 34, 94]
        ); // gameplay spec,:1610,:1614,:1734
        for (b, it) in [
            (DOOR_CLOSED, WOODEN_DOOR),
            (TABLE, WOODEN_TABLE),
            (CHAIR, WOODEN_CHAIR),
            (PLATFORM, WOOD_PLATFORM),
        ] {
            assert_eq!(item_data(it).create_tile, Some(b)); // gameplay spec createTile
            assert_eq!(b.drop_item(), Some(it));
        }
        assert_eq!(DOOR_OPEN.drop_item(), Some(WOODEN_DOOR));
        assert!(DOOR_CLOSED.is_solid());
        for b in [DOOR_OPEN, TABLE, CHAIR, PLATFORM] {
            assert!(!b.is_solid());
            assert!(b.is_opaque()); // still meshed as a cube
        }
    }

    #[test]
    fn furnace_and_anvil_ids_match_terraria() {
        assert_eq!((FURNACE.0, ANVIL.0), (17, 16)); // gameplay spec, :469
        assert_eq!((FURNACE_ITEM.0, ANVIL_ITEM.0), (33, 35)); // gameplay spec, :1616
        for (b, it) in [(FURNACE, FURNACE_ITEM), (ANVIL, ANVIL_ITEM)] {
            assert!(b.is_solid());
            assert_eq!(b.drop_item(), Some(it));
            assert_eq!(item_data(it).create_tile, Some(b));
            assert_eq!(item_data(it).max_stack, 9999);
        }
        assert_eq!(
            [GOLD_BAR.0, COPPER_BAR.0, SILVER_BAR.0, IRON_BAR.0],
            [19, 20, 21, 22]
        );
    }

    #[test]
    fn obsidian_id_and_solidity() {
        assert_eq!(OBSIDIAN.0, 56);
        assert!(OBSIDIAN.is_solid());
        assert!(OBSIDIAN.is_opaque());
    }

    #[test]
    fn obsidian_is_solid_for_liquid_and_player() {
        assert!(OBSIDIAN.is_solid());
        assert!(OBSIDIAN.is_opaque());
        assert_eq!(OBSIDIAN.drop_item(), Some(OBSIDIAN_ITEM));
        assert_ne!(OBSIDIAN, AIR);
    }

    #[test]
    fn dungeon_blocks_match_terraria() {
        assert_eq!(BLUE_DUNGEON_BRICK.0, 41);
        assert_eq!(GREEN_DUNGEON_BRICK.0, 43);
        assert_eq!(PINK_DUNGEON_BRICK.0, 44);
        assert_eq!(SPIKES.0, 48);
        assert!(BLUE_DUNGEON_BRICK.is_solid());
        assert!(BLUE_DUNGEON_BRICK.is_opaque());
        assert!(GREEN_DUNGEON_BRICK.is_solid());
        assert!(GREEN_DUNGEON_BRICK.is_opaque());
        assert!(PINK_DUNGEON_BRICK.is_solid());
        assert!(PINK_DUNGEON_BRICK.is_opaque());
        assert!(SPIKES.is_solid());
        assert!(SPIKES.is_opaque());
    }

    #[test]
    fn new_dressing_and_pot_blocks_have_valid_data() {
        let blocks = [POT, STALACTITE, MOSS, SMALL_ROCK, SHELF_MUSHROOM];
        let mut ids = std::collections::HashSet::new();
        for b in blocks {
            assert!(ids.insert(b.0), "duplicate BlockId {:?}", b);
            assert_ne!(b, AIR);
            assert!(!b.is_solid(), "{b:?} should be non-solid");
            assert!(
                !b.is_opaque(),
                "{b:?} should not be opaque so solid terrain beneath is not culled"
            );
        }
        assert_eq!(POT.drop_item(), None);
    }
}
