//! Generated underground chests and their loot.
//!
//! Vanilla `WorldGen.AddBuriedChest` (gameplay spec) places the chest, then fills it
//! by depth (`Main.worldSurface + 25` picks the wooden style, `:36396-36401`). Like crystals,
//! each hashed 32^3 cell rolls at most one attempt from `(seed, cell)` alone, so any chunk
//! can regenerate the chests that land in it. The block is placed by worldgen; the contents
//! come from `chests_in` and are seeded into the server once.

use super::noise::hash;
use super::terrain::{natural_block_for_size, surface_height_for_size};
use crate::block::{AIR, CHEST, STONE};
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkPos};
use crate::inventory::ItemStack;
use crate::item::{ItemId, Prefix, Rng, item_exists};
use crate::world_bounds::WorldSize;

const SALT_CHEST: u32 = 0x3C9E_71D5;
const CELL: i32 = 32;
/// Chance (per 1000) that a cell rolls a placement attempt (~1 chest per 3 cells; most fail the pocket test).
const ATTEMPT_PERMILLE: u32 = 350;
const SEARCH: i32 = 12;
/// Minimum blocks below the column's surface.
pub const MIN_DEPTH: i32 = 12;
/// `worldSurface + 25` (gameplay spec): shallower chests use the surface table.
pub const SURFACE_BAND_DEPTH: i32 = 25;
const CLEARANCE: i32 = 2;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Band {
    /// Above `worldSurface + 25`: the `Next(11)` main-item table (`:36702-36929`).
    Surface,
    /// Below it, above `rockLayer`: `Next(6)` main items (`:36932-37230`).
    Underground,
    /// Cavern layer: M6 accessories, weapons and secondary cavern loot.
    Cavern,
    /// Subterranean dungeon chests: locked golden chests.
    Dungeon,
    /// Underworld ruined house chests: shadow chests.
    Underworld,
}

pub const CAVERN_BAND_DEPTH: i32 = 70;

pub fn band_for_depth(depth: i32) -> Band {
    if depth < SURFACE_BAND_DEPTH {
        Band::Surface
    } else if depth < CAVERN_BAND_DEPTH {
        Band::Underground
    } else {
        Band::Cavern
    }
}

struct Loot {
    out: Vec<ItemStack>,
}

impl Loot {
    /// Items this game lacks are dropped after their rolls were consumed, so the
    /// roll sequence (and every other entry) is identical to vanilla's.
    fn add(&mut self, id: u16, count: u32) {
        if item_exists(ItemId(id)) {
            self.out.push(ItemStack {
                id: ItemId(id),
                count,
                prefix: Prefix(0),
            });
        }
    }
}

/// `genRand.Next(n)`.
fn next(r: &mut Rng, n: u32) -> u32 {
    r.below(n)
}

/// `genRand.Next(lo, hi)`: `hi` exclusive.
fn range(r: &mut Rng, lo: u32, hi: u32) -> u32 {
    lo + r.below(hi - lo)
}

/// Loot for one buried chest, rolled in vanilla's order. Items missing from the item
/// table are skipped (never re-rolled); today that is every entry except torches, wood,
/// wooden arrows, copper/iron/silver bars and silver coins.
pub fn roll_chest_loot(rng: &mut Rng, band: Band) -> Vec<ItemStack> {
    let mut l = Loot { out: Vec::new() };
    let r = rng;
    match band {
        Band::Surface => {
            // gameplay spec
            const MAIN: [u16; 11] = [280, 281, 284, 285, 953, 946, 3068, 3069, 3084, 4341, 6165];
            l.add(MAIN[next(r, 11) as usize], 1);
            if next(r, 6) == 0 {
                l.add(282, range(r, 40, 76)); // :36770, glowsticks
            }
            if next(r, 6) == 0 {
                l.add(279, range(r, 150, 301)); // :36782
            }
            for id in [3093, 4345] {
                if next(r, 6) == 0 {
                    // :36789-36815
                    let mut n = 1;
                    if next(r, 5) == 0 {
                        n += next(r, 2);
                    }
                    if next(r, 10) == 0 {
                        n += next(r, 3);
                    }
                    l.add(id, n);
                }
            }
            if next(r, 3) == 0 {
                l.add(168, range(r, 3, 6)); // :36817 rope
            }
            if next(r, 2) == 0 {
                let k = next(r, 2);
                let n = next(r, 8) + 3; // :36823-36836 copper / iron bar
                l.add(if k == 0 { 20 } else { 22 }, n);
            }
            if next(r, 2) == 0 {
                l.add(965, range(r, 50, 101)); // :36838
            }
            if next(r, 3) != 0 {
                let k = next(r, 2);
                let n = next(r, 26) + 25; // :36845-36858 wooden arrow / 42
                l.add(if k == 0 { 40 } else { 42 }, n);
            }
            if next(r, 2) == 0 {
                l.add(28, next(r, 3) + 3); // :36860 lesser healing potion
            }
            if next(r, 3) != 0 {
                l.add(2350, range(r, 3, 6)); // :36867 recall potion
            }
            if next(r, 3) > 0 {
                const POTIONS: [u16; 6] = [292, 298, 299, 290, 2322, 2325]; // :36873-36901
                let k = next(r, 6) as usize;
                l.add(POTIONS[k], range(r, 1, 3));
            }
            if next(r, 2) == 0 {
                let k = next(r, 2);
                let n = next(r, 11) + 10; // :36904-36917 torch / bottle
                l.add(if k == 0 { 8 } else { 31 }, n);
            }
            if next(r, 2) == 0 {
                l.add(72, range(r, 10, 30)); // :36919 silver coin
            }
            if next(r, 2) == 0 {
                l.add(9, range(r, 50, 100)); // :36925 wood
            }
        }
        Band::Underground => {
            // gameplay spec main item (Cloud in a Bottle, Hermes Boots,
            // Magic Mirror, Band of Regeneration, Lucky Horseshoe, Shiny Red Balloon).
            const MAIN: [u16; 6] = [49, 50, 53, 54, 158, 159];
            l.add(MAIN[next(r, 6) as usize], 1);
            if next(r, 20) == 0 {
                l.add(997, 1); // :37059
            } else if next(r, 20) == 0 {
                l.add(930, 1); // :37065
                l.add(931, next(r, 26) + 25);
            }
            if next(r, 3) == 0 {
                l.add(166, range(r, 10, 20)); // :37106 bomb
            }
            if next(r, 5) == 0 {
                l.add(43, 1); // Suspicious Looking Eye
            }
            if next(r, 3) == 0 {
                l.add(965, range(r, 50, 101)); // :37117
            }
            if next(r, 2) == 0 {
                let k = next(r, 2);
                let n = next(r, 10) + 5; // :37124-37137 iron / silver bar
                l.add(if k == 0 { 22 } else { 21 }, n);
            }
            if next(r, 2) == 0 {
                let k = next(r, 2);
                let n = next(r, 25) + 25; // :37139-37152
                l.add(if k == 0 { 40 } else { 42 }, n);
            }
            if next(r, 2) == 0 {
                l.add(28, next(r, 3) + 3); // :37154
            }
            if next(r, 3) > 0 {
                const POTIONS: [u16; 9] = [289, 298, 299, 290, 303, 291, 304, 2322, 2329]; // :37161-37201
                let k = next(r, 9) as usize;
                l.add(POTIONS[k], range(r, 1, 3));
            }
            if next(r, 3) != 0 {
                l.add(2350, range(r, 2, 5)); // :37204
            }
            if next(r, 2) == 0 {
                l.add(8, next(r, 11) + 10); // :37211 torches
            }
            if next(r, 2) == 0 {
                l.add(72, range(r, 50, 90)); // :37225 silver coin
            }
        }
        Band::Cavern => {
            const MAIN: [u16; 6] = [54, 53, 49, 50, 55, 274];
            l.add(MAIN[next(r, 6) as usize], 1);
            if next(r, 2) == 0 {
                let k = next(r, 3);
                let n = next(r, 10) + 5; // iron / silver / gold bar
                let bar = match k {
                    0 => 22,
                    1 => 21,
                    _ => 19,
                };
                l.add(bar, n);
            }
            if next(r, 2) == 0 {
                let k = next(r, 2);
                let n = next(r, 26) + 25; // 25..=50 wooden / flaming arrow
                l.add(if k == 0 { 40 } else { 41 }, n);
            }
            if next(r, 2) == 0 {
                l.add(28, next(r, 3) + 3); // 3..=5 lesser healing potion
            }
            if next(r, 2) == 0 {
                l.add(8, next(r, 16) + 15); // 15..=30 torches
            }
            if next(r, 2) == 0 {
                let k = next(r, 2);
                if k == 0 {
                    l.add(73, range(r, 1, 3)); // 1..=2 gold coins
                } else {
                    l.add(72, range(r, 50, 90)); // 50..=89 silver coins
                }
            }
        }
        Band::Dungeon => {
            const MAIN: [u16; 8] = [155, 156, 157, 164, 163, 113, 3317, 329];
            l.add(MAIN[next(r, 8) as usize], 1);
            l.add(154, range(r, 20, 50));
            if next(r, 2) == 0 {
                let k = next(r, 3);
                let n = next(r, 10) + 5;
                let bar = match k {
                    0 => 22,
                    1 => 21,
                    _ => 19,
                };
                l.add(bar, n);
            }
            if next(r, 2) == 0 {
                let k = next(r, 2);
                let n = next(r, 26) + 25;
                l.add(if k == 0 { 40 } else { 97 }, n);
            }
            if next(r, 2) == 0 {
                l.add(8, next(r, 16) + 15);
            }
            if next(r, 2) == 0 {
                l.add(28, next(r, 3) + 3);
            }
            if next(r, 2) == 0 {
                l.add(110, next(r, 3) + 3);
            }
            if next(r, 2) == 0 {
                let k = next(r, 2);
                if k == 0 {
                    l.add(73, range(r, 1, 4));
                } else {
                    l.add(72, range(r, 50, 90));
                }
            }
        }
        Band::Underworld => {
            if next(r, 20) == 0 {
                l.add(crate::item::LAVA_CHARM.0, 1);
            } else {
                l.add(crate::item::OBSIDIAN_SKIN_POTION.0, 1);
            }
            l.add(crate::item::HELLSTONE_ITEM.0, range(r, 10, 26));
            l.add(crate::item::OBSIDIAN_ITEM.0, range(r, 5, 16));
            if next(r, 2) == 0 {
                let k = next(r, 2);
                if k == 0 {
                    l.add(73, range(r, 1, 4));
                } else {
                    l.add(72, range(r, 50, 90));
                }
            }
        }
    }
    for st in &mut l.out {
        if crate::item::item_data(st.id).max_stack == 1 {
            st.prefix = crate::item::roll_prefix(st.id, &mut || r.next_u32());
        }
    }
    l.out
}

fn chest_in_cell_for_size(seed: u32, cell: [i32; 3], size: WorldSize) -> Option<[i32; 3]> {
    let h = hash(seed ^ SALT_CHEST, cell[0], cell[1], cell[2]);
    if h % 1000 >= ATTEMPT_PERMILLE {
        return None;
    }
    let r = hash(seed ^ SALT_CHEST ^ 0x5bd1_e995, cell[0], cell[1], cell[2]);
    let x = cell[0] * CELL + (r % CELL as u32) as i32;
    let z = cell[2] * CELL + ((r >> 8) % CELL as u32) as i32;
    // No buried chests inside corruption patches.
    if super::corruption::corruption_at_for_size(seed, x, z, size) > 0.0 {
        return None;
    }
    let y0 = cell[1] * CELL + ((r >> 16) % CELL as u32) as i32;
    let surface = surface_height_for_size(seed, x, z, size);
    // First stone floor with an air pocket above it, scanning down.
    for y in (y0 - SEARCH..=y0).rev() {
        if y < 1 || surface - y < MIN_DEPTH {
            continue;
        }
        if natural_block_for_size(seed, x, y - 1, z, size) == STONE
            && (0..CLEARANCE).all(|d| natural_block_for_size(seed, x, y + d, z, size) == AIR)
        {
            return Some([x, y, z]);
        }
    }
    None
}

/// Chests (block position, initial contents) inside chunk `pos`. Chunk-order independent;
/// contents depend on `(seed, position)` only.
pub fn chests_in(seed: u32, pos: ChunkPos) -> Vec<([i32; 3], Vec<ItemStack>)> {
    chests_in_for_size(seed, pos, WorldSize::Small)
}

pub fn chests_in_for_size(
    seed: u32,
    pos: ChunkPos,
    size: WorldSize,
) -> Vec<([i32; 3], Vec<ItemStack>)> {
    let lo = [pos.x, pos.y, pos.z].map(|c| c * CHUNK_SIZE);
    let crystals = super::crystal::crystals_in_for_size(seed, pos, size);
    let mut out = Vec::new();
    let cy = (lo[1] / CELL)..=((lo[1] + CHUNK_SIZE - 1 + SEARCH).div_euclid(CELL));
    for cx in lo[0].div_euclid(CELL)..=(lo[0] + CHUNK_SIZE - 1).div_euclid(CELL) {
        for cz in lo[2].div_euclid(CELL)..=(lo[2] + CHUNK_SIZE - 1).div_euclid(CELL) {
            for cy in cy.clone() {
                let Some(p) = chest_in_cell_for_size(seed, [cx, cy, cz], size) else {
                    continue;
                };
                // A life crystal on the same block wins (it is applied first).
                if (0..3).all(|i| p[i] >= lo[i] && p[i] < lo[i] + CHUNK_SIZE)
                    && !crystals.contains(&p)
                {
                    let depth = surface_height_for_size(seed, p[0], p[2], size) - p[1];
                    let mut rng = Rng::new(
                        hash(seed ^ SALT_CHEST ^ 0xA5A5_1234, p[0], p[1], p[2]) as u64
                            | (hash(seed ^ SALT_CHEST ^ 0x1357_9BDF, p[2], p[0], p[1]) as u64)
                                << 32,
                    );
                    let band = if super::dungeon::is_dungeon_point(seed, p[0], p[1], p[2], size) {
                        Band::Dungeon
                    } else {
                        band_for_depth(depth)
                    };
                    out.push((p, roll_chest_loot(&mut rng, band)));
                }
            }
        }
    }
    let dungeon = super::dungeon::DungeonLayout::new(seed, size);
    let shadow_chest_idx = if dungeon.chests.is_empty() {
        None
    } else {
        Some((seed as usize) % dungeon.chests.len())
    };
    for (idx, p) in dungeon.chests.into_iter().enumerate() {
        if (0..3).all(|i| p[i] >= lo[i] && p[i] < lo[i] + CHUNK_SIZE) && !crystals.contains(&p) {
            let mut rng = Rng::new(
                hash(seed ^ SALT_CHEST ^ 0xA5A5_1234, p[0], p[1], p[2]) as u64
                    | (hash(seed ^ SALT_CHEST ^ 0x1357_9BDF, p[2], p[0], p[1]) as u64) << 32,
            );
            let mut loot = roll_chest_loot(&mut rng, Band::Dungeon);
            if shadow_chest_idx == Some(idx)
                && !loot.iter().any(|st| st.id == crate::item::SHADOW_KEY)
            {
                let prefix =
                    crate::item::roll_prefix(crate::item::SHADOW_KEY, &mut || rng.next_u32());
                loot.push(ItemStack {
                    id: crate::item::SHADOW_KEY,
                    count: 1,
                    prefix,
                });
            }
            out.push((p, loot));
        }
    }
    for house in super::underworld::houses_in_for_size(seed, pos, size) {
        let p = house.chest;
        if (0..3).all(|i| p[i] >= lo[i] && p[i] < lo[i] + CHUNK_SIZE) && !crystals.contains(&p) {
            let mut rng = Rng::new(
                hash(seed ^ SALT_CHEST ^ 0xA5A5_1234, p[0], p[1], p[2]) as u64
                    | (hash(seed ^ SALT_CHEST ^ 0x1357_9BDF, p[2], p[0], p[1]) as u64) << 32,
            );
            out.push((p, roll_chest_loot(&mut rng, Band::Underworld)));
        }
    }
    out
}

pub fn is_dungeon_chest(seed: u32, pos: [i32; 3], size: WorldSize) -> bool {
    let dungeon = super::dungeon::DungeonLayout::new(seed, size);
    dungeon.chests.contains(&pos)
        || super::dungeon::is_dungeon_point(seed, pos[0], pos[1], pos[2], size)
}

pub fn is_underworld_chest(seed: u32, pos: [i32; 3], size: WorldSize) -> bool {
    super::underworld::is_house_chest(seed, pos, size)
}

/// Places the chunk's chest blocks into air.
pub fn apply(seed: u32, pos: ChunkPos, chunk: &mut Chunk) {
    apply_for_size(seed, pos, WorldSize::Small, chunk);
}

pub fn apply_for_size(seed: u32, pos: ChunkPos, size: WorldSize, chunk: &mut Chunk) {
    let lo = [pos.x, pos.y, pos.z].map(|c| c * CHUNK_SIZE);
    for (p, _) in chests_in_for_size(seed, pos, size) {
        let l = [0, 1, 2].map(|i| (p[i] - lo[i]) as usize);
        if chunk.get(l[0], l[1], l[2]) == AIR {
            chunk.set(l[0], l[1], l[2], CHEST);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::generate_chunk;
    use crate::worldgen::terrain::{natural_block, surface_height};

    fn cp(x: i32, y: i32, z: i32) -> ChunkPos {
        ChunkPos { x, y, z }
    }

    fn all_in_area(seed: u32, chunks: i32) -> Vec<([i32; 3], Vec<ItemStack>)> {
        let mut v = Vec::new();
        for x in 0..chunks {
            for z in 0..chunks {
                for y in 0..4 {
                    v.extend(chests_in(seed, cp(x, y, z)));
                }
            }
        }
        v
    }

    fn loot(seed: u64, band: Band) -> Vec<ItemStack> {
        roll_chest_loot(&mut Rng::new(seed), band)
    }

    #[test]
    fn loot_table_is_deterministic_per_seed() {
        for band in [Band::Surface, Band::Underground, Band::Cavern] {
            assert_eq!(loot(9, band), loot(9, band));
            assert!((0..50).any(|s| loot(s, band) != loot(9, band)));
        }
        assert_eq!(all_in_area(3, 6), all_in_area(3, 6));
        assert_ne!(all_in_area(3, 6), all_in_area(4, 6));
    }

    #[test]
    fn loot_amount_ranges_match_spec() {
        use crate::item::{COPPER_BAR, IRON_BAR, SILVER_BAR, SILVER_COIN, TORCH, WOOD};
        // (item, min, max inclusive, band) from the cited gameplay spec lines.
        let want: [(ItemId, u32, u32, Band); 6] = [
            (WOOD, 50, 99, Band::Surface),            // :36927 Next(50,100)
            (SILVER_COIN, 10, 29, Band::Surface),     // :36922 Next(10,30)
            (SILVER_COIN, 50, 89, Band::Underground), // :37228 Next(50,90)
            (TORCH, 10, 20, Band::Surface),           // :36907 Next(11)+10
            (COPPER_BAR, 3, 10, Band::Surface),       // :36826 Next(8)+3
            (SILVER_BAR, 5, 14, Band::Underground),   // :37127 Next(10)+5
        ];
        let mut seen = std::collections::HashMap::new();
        for s in 0..4000u64 {
            for band in [Band::Surface, Band::Underground] {
                for st in loot(s, band) {
                    for (i, (id, lo, hi, b)) in want.iter().enumerate() {
                        if st.id == *id && *b == band {
                            assert!((*lo..=*hi).contains(&st.count), "{id:?} {}", st.count);
                            let e = seen.entry(i).or_insert((u32::MAX, 0));
                            e.0 = e.0.min(st.count);
                            e.1 = e.1.max(st.count);
                        }
                    }
                }
            }
        }
        // The full range is reachable.
        for (i, (_, lo, hi, _)) in want.iter().enumerate() {
            assert_eq!(seen[&i], (*lo, *hi), "entry {i}");
        }
        // Iron bars share the Surface/Underground slot with copper / silver.
        assert!((0..4000u64).any(|s| loot(s, Band::Surface).iter().any(|x| x.id == IRON_BAR)));
    }

    #[test]
    fn surface_chest_loot_yields_wooden_arrows() {
        use crate::item::WOODEN_ARROW;
        // :36845-36858: Next(26)+25 of arrow (40) or id 42 when the roll k == 0.
        let counts: Vec<u32> = (0..2000u64)
            .flat_map(|s| loot(s, Band::Surface))
            .filter(|st| st.id == WOODEN_ARROW)
            .map(|st| st.count)
            .collect();
        assert!(!counts.is_empty());
        assert!(counts.iter().all(|c| (25..=50).contains(c)));
    }

    #[test]
    fn chests_roll_lesser_healing_potions() {
        use crate::item::LESSER_HEALING_POTION;
        // :36860 / :37154: 3..=5 potions now that item 28 exists.
        let counts: Vec<u32> = (0..2000u64)
            .flat_map(|s| loot(s, Band::Underground))
            .filter(|st| st.id == LESSER_HEALING_POTION)
            .map(|st| st.count)
            .collect();
        assert!(!counts.is_empty());
        assert!(counts.iter().all(|c| (3..=5).contains(c)));
    }

    #[test]
    fn loot_only_contains_existing_items() {
        for s in 0..500 {
            for band in [
                Band::Surface,
                Band::Underground,
                Band::Cavern,
                Band::Dungeon,
            ] {
                for st in loot(s, band) {
                    assert!(item_exists(st.id), "{:?}", st.id);
                    assert!(st.count >= 1 && st.count <= crate::item::item_data(st.id).max_stack);
                }
            }
        }
    }

    #[test]
    fn dungeon_chest_rolls_dungeon_weapons() {
        use crate::item::{
            AQUA_SCEPTER, BLUE_MOON, COBALT_SHIELD, HANDGUN, MAGIC_MISSILE, MURAMASA, SHADOW_KEY,
            VALOR,
        };
        let weapons = [
            MURAMASA,
            COBALT_SHIELD,
            AQUA_SCEPTER,
            HANDGUN,
            BLUE_MOON,
            MAGIC_MISSILE,
            VALOR,
            SHADOW_KEY,
        ];
        let mut seen = std::collections::HashSet::new();
        for s in 0..1000 {
            let loot = roll_chest_loot(&mut Rng::new(s), Band::Dungeon);
            for st in loot {
                if weapons.contains(&st.id) {
                    seen.insert(st.id);
                }
            }
        }
        for w in weapons {
            assert!(seen.contains(&w), "dungeon chests must roll item {:?}", w);
        }
    }

    #[test]
    fn dungeon_chests_contain_bones_and_no_golden_key() {
        use crate::item::{BONE, GOLDEN_KEY};
        let mut found_bone = false;
        for s in 0..100 {
            let l = loot(s, Band::Dungeon);
            for st in l {
                if st.id == BONE {
                    assert!((20..=50).contains(&st.count));
                    found_bone = true;
                }
                assert_ne!(
                    st.id, GOLDEN_KEY,
                    "locked dungeon chests should not contain Golden Keys"
                );
            }
        }
        assert!(found_bone, "dungeon chests must contain bones");
    }

    #[test]
    fn underground_chests_roll_horseshoe_balloon_and_suspicious_eye() {
        use crate::item::{LUCKY_HORSESHOE, SHINY_RED_BALLOON, SUSPICIOUS_LOOKING_EYE};
        let (mut horseshoe, mut balloon, mut eye) = (false, false, false);
        for s in 0..500 {
            for st in loot(s, Band::Underground) {
                horseshoe |= st.id == LUCKY_HORSESHOE;
                balloon |= st.id == SHINY_RED_BALLOON;
                eye |= st.id == SUSPICIOUS_LOOKING_EYE;
            }
        }
        assert!(horseshoe, "underground chests should roll Lucky Horseshoe");
        assert!(balloon, "underground chests should roll Shiny Red Balloon");
        assert!(eye, "underground chests should roll Suspicious Looking Eye");
    }

    #[test]
    fn dungeon_world_guarantees_shadow_key_and_prefixes() {
        use crate::item::{BAND_OF_REGENERATION, ENCHANTED_BOOMERANG, SHADOW_KEY, item_data};
        let size = WorldSize::Small;
        let layout = super::super::dungeon::DungeonLayout::new(42, size);
        assert!(!layout.chests.is_empty());
        let mut found_shadow_key = false;
        for p in &layout.chests {
            let c = ChunkPos::of_block(p[0], p[1], p[2]);
            for (cp, loot) in chests_in_for_size(42, c, size) {
                if &cp == p && loot.iter().any(|st| st.id == SHADOW_KEY) {
                    found_shadow_key = true;
                }
            }
        }
        assert!(
            found_shadow_key,
            "dungeon must guarantee at least one Shadow Key"
        );

        let has_prefixed = (0..500).any(|s| {
            loot(s, Band::Underground).iter().any(|st| {
                item_data(st.id).max_stack == 1
                    && st.id != BAND_OF_REGENERATION
                    && st.id != ENCHANTED_BOOMERANG
                    && st.prefix.0 > 0
            }) || loot(s, Band::Cavern)
                .iter()
                .any(|st| item_data(st.id).max_stack == 1 && st.prefix.0 > 0)
        });
        assert!(
            has_prefixed,
            "chest max_stack==1 items should roll prefixes"
        );
    }

    #[test]
    fn chests_are_chunk_order_independent() {
        let seed = 3;
        let a = all_in_area(seed, 8);
        assert!(!a.is_empty());
        let (p, _) = &a[0];
        let c = ChunkPos::of_block(p[0], p[1], p[2]);
        let cold = generate_chunk(seed, c);
        for d in [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 1, 0)] {
            generate_chunk(seed, cp(c.x + d.0, c.y + d.1, c.z + d.2));
        }
        assert_eq!(cold, generate_chunk(seed, c));
        let l = p.map(|v| v.rem_euclid(CHUNK_SIZE) as usize);
        assert_eq!(cold.get(l[0], l[1], l[2]), CHEST);
        // The block exists in the generated chunk for every listed chest.
        for (p, _) in &a {
            let c = ChunkPos::of_block(p[0], p[1], p[2]);
            let l = p.map(|v| v.rem_euclid(CHUNK_SIZE) as usize);
            assert_eq!(
                generate_chunk(seed, c).get(l[0], l[1], l[2]),
                CHEST,
                "{p:?}"
            );
        }
    }

    #[test]
    fn chests_sit_on_solid_floor_in_air_pocket() {
        for (p, _) in all_in_area(1, 10) {
            if is_dungeon_chest(1, p, WorldSize::Small)
                || is_underworld_chest(1, p, WorldSize::Small)
            {
                continue;
            }
            assert_eq!(natural_block(1, p[0], p[1] - 1, p[2]), STONE);
            for d in 0..CLEARANCE {
                assert_eq!(natural_block(1, p[0], p[1] + d, p[2]), AIR);
            }
            assert!(surface_height(1, p[0], p[2]) - p[1] >= MIN_DEPTH);
        }
    }

    #[test]
    fn chest_density_within_tolerance() {
        // 8x8 chunk columns x 4 layers = 64 columns' worth of chunks (256 chunks).
        for seed in [1, 2, 3] {
            let n = all_in_area(seed, 8).len();
            assert!(
                (6..=60).contains(&n),
                "seed {seed}: {n} chests in 8x8 columns"
            );
        }
    }

    #[test]
    fn wand_appears_in_generated_chests_deterministically() {
        use crate::item::WAND_OF_SPARKING;
        // gameplay spec: 3069 is one of the 11 surface main-item rolls.
        let seed = (0..5_000u64)
            .find(|s| {
                loot(*s, Band::Surface)
                    .iter()
                    .any(|x| x.id == WAND_OF_SPARKING)
            })
            .expect("a surface chest rolls the Wand of Sparking");
        let a = loot(seed, Band::Surface);
        assert_eq!(a, loot(seed, Band::Surface));
        assert_eq!(
            a.iter()
                .filter(|x| x.id == WAND_OF_SPARKING && x.count == 1)
                .count(),
            1
        );
    }

    #[test]
    fn cavern_chest_rolls_m6_accessories_and_weapons() {
        use crate::item::{
            BAND_OF_REGENERATION, CLOUD_IN_A_BOTTLE, ENCHANTED_BOOMERANG, HERMES_BOOTS,
            MAGIC_MIRROR, SPEAR,
        };
        let m6_items = [
            HERMES_BOOTS,
            CLOUD_IN_A_BOTTLE,
            BAND_OF_REGENERATION,
            MAGIC_MIRROR,
            ENCHANTED_BOOMERANG,
            SPEAR,
        ];
        let mut seen = std::collections::HashSet::new();
        for s in 0..2000u64 {
            let items = roll_chest_loot(&mut Rng::new(s), Band::Cavern);
            assert!(!items.is_empty(), "cavern chest loot must not be empty");
            let primary = items[0].id;
            assert!(
                m6_items.contains(&primary),
                "cavern chest primary loot must be one of M6 items, got {primary:?}"
            );
            assert_eq!(items[0].count, 1);
            seen.insert(primary);
        }
        assert_eq!(
            seen.len(),
            6,
            "all 6 cavern accessories/weapons must be rollable"
        );
    }

    #[test]
    fn underworld_houses_contain_underworld_loot() {
        let seed = 42;
        let size = WorldSize::Small;
        let mut found_house_chest = false;
        // Surface/Underground main table IDs from reference spec / chest_gen
        const SURFACE_UNDERGROUND_MAIN_IDS: [u16; 17] = [
            280, 281, 284, 285, 953, 946, 3068, 3069, 3084, 4341, 6165, // Surface
            49, 158, 159, 160, 274, 974, // Underground
        ];

        for cx in 0..12 {
            for cz in 0..12 {
                let pos = ChunkPos { x: cx, y: 0, z: cz };
                for (p, loot) in chests_in_for_size(seed, pos, size) {
                    if is_underworld_chest(seed, p, size) {
                        found_house_chest = true;
                        let ids: Vec<u16> = loot.iter().map(|s| s.id.0).collect();
                        let has_underworld_loot = ids.contains(&crate::item::LAVA_CHARM.0)
                            || ids.contains(&crate::item::OBSIDIAN_SKIN_POTION.0)
                            || ids.contains(&crate::item::HELLSTONE_ITEM.0)
                            || ids.contains(&crate::item::OBSIDIAN_ITEM.0);
                        assert!(
                            has_underworld_loot,
                            "house chest at {p:?} missing underworld loot: {ids:?}"
                        );
                        for main_id in SURFACE_UNDERGROUND_MAIN_IDS {
                            assert!(
                                !ids.contains(&main_id),
                                "house chest contains Surface/Underground main item {main_id}"
                            );
                        }
                    }
                }
            }
        }
        assert!(
            found_house_chest,
            "expected to find at least one underworld house chest"
        );
    }
}
