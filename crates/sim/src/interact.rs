//! Item use: voxel raycast, mining and placing. Ports the relevant parts of
//! `Player.ItemCheck`, `Player.PickTile`/`GetPickaxeDamage` and `tile damage tracking`.

use std::collections::HashMap;

use crate::block::{
    AIR, BLUE_DUNGEON_BRICK, BlockId, CHAIR, DEMON_ALTAR, DEMONITE_ORE, DOOR_CLOSED, DOOR_OPEN,
    EBONSTONE, GREEN_DUNGEON_BRICK, HELLFORGE, HELLSTONE, MOSS, OBSIDIAN, PINK_DUNGEON_BRICK, POT,
    SHADOW_ORB, SHELF_MUSHROOM, SMALL_ROCK, STALACTITE, TABLE, TORCH, TRUNK,
};
use crate::chunk::BlockQuery;
use crate::inventory::{Inventory, ItemStack};
use crate::item::{
    EMPTY_BUCKET, GOLDEN_KEY, ItemClass, ItemId, LAVA_BUCKET, LESSER_HEALING_POTION,
    LIFE_CRYSTAL_ITEM, MANA_CRYSTAL, MINISHARK, MUSHROOM, OBSIDIAN_SKIN_POTION, Prefix, Rng,
    SHADOW_KEY, WATER_BUCKET, WOOD, WOOD_WALL, item_data, prefix_stats,
};
use crate::liquid_cell::{Liquid, LiquidKind};
use crate::player::consts::{HALF_WIDTH, HEIGHT, OBSIDIAN_SKIN_TICKS, TILE_BLOCKS};
use crate::player::vitals::LESSER_HEALING_HEAL;
use crate::player::{PlayerBody, PlayerInput, Vitals};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub block: [i32; 3],
    pub normal: [i32; 3],
}

/// Interaction view of the world. Physics treats unloaded chunks as solid
/// (`SOLID_UNLOADED`), but a missing chunk is not a usable target: it is air
/// for interaction purposes.
pub trait LoadedQuery: BlockQuery {
    /// Whether the chunk containing (x, y, z) is loaded.
    fn is_loaded(&self, x: i32, y: i32, z: i32) -> bool;
    /// Background wall bit at (x, y, z); `false` where unloaded or unsupported.
    fn has_wall(&self, _x: i32, _y: i32, _z: i32) -> bool {
        false
    }
}

/// Interaction edits are fallible. `false` means the chunk was not loaded and
/// nothing changed, so callers must not consume items, drop loot or emit events.
pub trait EditQuery: LoadedQuery {
    fn try_set_block(&mut self, x: i32, y: i32, z: i32, b: BlockId) -> bool;
    /// Sets or clears the wall bit. `false` = chunk not loaded, nothing changed.
    fn try_set_wall(&mut self, x: i32, y: i32, z: i32, on: bool) -> bool;
    /// Sets or clears liquid. `false` = chunk not loaded, nothing changed.
    fn set_liquid(&mut self, _x: i32, _y: i32, _z: i32, _l: Option<Liquid>) -> bool {
        false
    }
    /// Wakes liquid simulation around `pos`.
    fn wake_liquid(&mut self, _pos: [i32; 3]) {}
}

/// Amanatides-Woo voxel DDA from `eye` along `dir` (need not be normalised) for
/// `max` blocks. Hits the first cell `hit` accepts.
fn raycast_where(
    eye: [f32; 3],
    dir: [f32; 3],
    max: f32,
    mut hit: impl FnMut([i32; 3]) -> bool,
) -> Option<Hit> {
    let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    if len < 1e-9 {
        return None;
    }
    let d = [dir[0] / len, dir[1] / len, dir[2] / len];
    let mut cell = [
        eye[0].floor() as i32,
        eye[1].floor() as i32,
        eye[2].floor() as i32,
    ];
    let mut step = [0i32; 3];
    let mut t_max = [f32::INFINITY; 3];
    let mut t_delta = [f32::INFINITY; 3];
    for a in 0..3 {
        if d[a] > 0.0 {
            step[a] = 1;
            t_max[a] = ((cell[a] + 1) as f32 - eye[a]) / d[a];
            t_delta[a] = 1.0 / d[a];
        } else if d[a] < 0.0 {
            step[a] = -1;
            t_max[a] = (cell[a] as f32 - eye[a]) / d[a];
            t_delta[a] = -1.0 / d[a];
        }
    }
    loop {
        let a = if t_max[0] <= t_max[1] && t_max[0] <= t_max[2] {
            0
        } else if t_max[1] <= t_max[2] {
            1
        } else {
            2
        };
        if t_max[a] > max {
            return None;
        }
        cell[a] += step[a];
        t_max[a] += t_delta[a];
        if hit(cell) {
            let mut normal = [0; 3];
            normal[a] = -step[a];
            return Some(Hit {
                block: cell,
                normal,
            });
        }
    }
}

/// Amanatides-Woo voxel DDA for rendering: hits the first non-AIR block
/// (torches included). Interaction uses [`raycast_loaded`] instead, so it can
/// reject chunks the caller hasn't loaded.
pub fn raycast(q: &impl BlockQuery, eye: [f32; 3], dir: [f32; 3], max: f32) -> Option<Hit> {
    raycast_pick(q, eye, dir, max, false)
}

/// Like [`raycast`], but optionally considers liquid cells as hits.
pub fn raycast_pick(
    q: &impl BlockQuery,
    eye: [f32; 3],
    dir: [f32; 3],
    max: f32,
    include_liquid: bool,
) -> Option<Hit> {
    raycast_where(eye, dir, max, |c| {
        q.block(c[0], c[1], c[2]) != AIR || (include_liquid && q.liquid(c[0], c[1], c[2]).is_some())
    })
}

/// Like [`raycast`], but unloaded chunks are air: no phantom targets.
fn raycast_loaded(q: &impl LoadedQuery, eye: [f32; 3], dir: [f32; 3], max: f32) -> Option<Hit> {
    raycast_where(eye, dir, max, |c| {
        q.is_loaded(c[0], c[1], c[2]) && q.block(c[0], c[1], c[2]) != AIR
    })
}

/// Eye height above the feet.
pub const EYE_HEIGHT: f32 = 1.62;
/// Player.tileRangeX / tileRangeY defaults (Player.DefaultTileRangeX/Y), in tiles.
const TILE_RANGE_X: f32 = 5.0;
const TILE_RANGE_Y: f32 = 4.0;
/// Longest ray cast; anything farther is out of reach anyway.
pub const RAY_MAX: f32 = 8.0;

/// Player.ItemCheck: the tile must lie within tileRangeX/Y of the hitbox.
pub fn in_reach(body: &PlayerBody, c: [i32; 3]) -> bool {
    let gap = |lo: f32, hi: f32, cell: i32| (cell as f32 - hi).max(lo - (cell + 1) as f32).max(0.0);
    let dx = gap(body.pos[0] - HALF_WIDTH, body.pos[0] + HALF_WIDTH, c[0]);
    let dz = gap(body.pos[2] - HALF_WIDTH, body.pos[2] + HALF_WIDTH, c[2]);
    let dy = gap(body.pos[1], body.pos[1] + HEIGHT, c[1]);
    dx.hypot(dz) <= TILE_RANGE_X * TILE_BLOCKS && dy <= TILE_RANGE_Y * TILE_BLOCKS
}

/// Whether the block cell `c` intersects the player's box.
pub fn overlaps_player(body: &PlayerBody, c: [i32; 3]) -> bool {
    let (p, x, y, z) = (body.pos, c[0] as f32, c[1] as f32, c[2] as f32);
    x < p[0] + HALF_WIDTH
        && x + 1.0 > p[0] - HALF_WIDTH
        && y < p[1] + HEIGHT
        && y + 1.0 > p[1]
        && z < p[2] + HALF_WIDTH
        && z + 1.0 > p[2] - HALF_WIDTH
}

/// Player.GetPickaxeDamage for the tiles that exist in this game.
fn pick_damage(tile: BlockId, pick: i32) -> i32 {
    // Main.tileNoFail (torches, pots, dressing) starts at 100.
    let mut dmg = if matches!(
        tile,
        TORCH | POT | STALACTITE | MOSS | SMALL_ROCK | SHELF_MUSHROOM
    ) {
        100
    } else {
        0
    };
    dmg += pick;
    // Tiles 147, 0, 53, 59 (snow, dirt, sand, mud) get pickPower added a second time.
    if matches!(tile.0, 147 | 0 | 53 | 59) {
        dmg += pick;
    }
    dmg
}

/// Minimum pick power per tile. Vanilla gates only the tiles listed at
/// gameplay spec (Meteorite/Demonite 55, Hellstone 65, Dungeon 100,
/// Lihzahrd 210, ...). Of those that exist here, Demonite Ore and Obsidian
/// demand pick 55 (gold tier), Ebonstone, Hellstone and the Hellforge demand
/// pick 65 (nightmare tier), and the Dungeon bricks demand 100 (molten tier).
/// Copper, iron, silver, gold and stone mine with any pickaxe.
const MIN_PICK: &[(BlockId, i32)] = &[
    (DEMONITE_ORE, 55),
    (OBSIDIAN, 55),
    (EBONSTONE, 65),
    (HELLSTONE, 65),
    (HELLFORGE, 65),
    (BLUE_DUNGEON_BRICK, 100),
    (GREEN_DUNGEON_BRICK, 100),
    (PINK_DUNGEON_BRICK, 100),
];

/// Pick power a tile demands (0 = anything mines it).
pub fn min_pick(tile: BlockId) -> i32 {
    min_pick_in(MIN_PICK, tile)
}

fn min_pick_in(table: &[(BlockId, i32)], tile: BlockId) -> i32 {
    table
        .iter()
        .find(|(b, _)| *b == tile)
        .map_or(0, |(_, p)| *p)
}

/// gameplay spec: axe power applies only to tree tiles, at 1.2x.
fn axe_damage(tile: BlockId, axe: i32) -> i32 {
    if tile == TRUNK {
        (axe as f32 * 1.2) as i32
    } else {
        0
    }
}

/// gameplay spec: one bonus wood when `Next(35) <= axe` or the
/// second roll is 0. The short-circuit is Terraria's: the second roll is only
/// drawn when the first fails.
fn bonus_wood_roll(rng: &mut Rng, axe: i32) -> bool {
    rng.next_u32() % 35 <= axe as u32 || rng.next_u32() % 3 == 0
}

/// Remove the trunk from `at` upward, leaving any lower trunk blocks intact as a stump, dropping Wood.
fn chop_tree(
    w: &mut impl EditQuery,
    at: [i32; 3],
    axe: i32,
    rng: &mut Rng,
    ev: &mut Vec<UseEvent>,
) {
    let [x, mut y, z] = at;
    while w.block(x, y, z) == TRUNK {
        if !w.try_set_block(x, y, z, AIR) {
            break;
        }
        ev.push(UseEvent::TileBroken {
            pos: [x, y, z],
            tile: TRUNK,
        });
        let pos = [x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5];
        let wood = ItemStack {
            id: WOOD,
            count: 1,
            prefix: crate::item::Prefix(0),
        };
        ev.push(UseEvent::ItemDrop { stack: wood, pos });
        if bonus_wood_roll(rng, axe) {
            ev.push(UseEvent::ItemDrop { stack: wood, pos });
        }
        y += 1;
    }
}

/// tile damage tracking.Prune: ages every entry, decaying damage once it gets old.
fn prune(w: &impl LoadedQuery, st: &mut UseState) {
    st.tile_damage.retain(|p, (dmg, ttl)| {
        if *ttl <= 1 {
            return false;
        }
        *ttl -= 1;
        *dmg -= match *ttl {
            ..12 => 10,
            12..24 => 7,
            24..36 => 5,
            36..48 => 2,
            _ => 0,
        };
        *dmg >= 0 && w.is_loaded(p[0], p[1], p[2]) && w.block(p[0], p[1], p[2]) != AIR
    });
}

fn round_ties_even(v: f32) -> i32 {
    v.round_ties_even() as i32
}

#[derive(Clone, Debug, Default)]
pub struct UseState {
    pub anim: i32,
    pub time: i32,
    pub tile_damage: HashMap<[i32; 3], (i32, i32)>,
    /// Whether the use button was held during the previous `tick_use`, so a
    /// non-`auto_reuse` item only starts on a fresh press.
    pub last_use_held: bool,
    /// Hotbar slot requested during a swing; applied when the swing ends.
    pending_slot: Option<usize>,
    /// Remaining follow-up shots in a Clockwork Assault Rifle 3-round burst.
    burst_left: u8,
    burst_delay: u8,
    burst_weapon: Option<ItemId>,
    burst_ammo: Option<ItemId>,
    burst_prefix: Option<Prefix>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UseEvent {
    Swing {
        item: ItemId,
        prefix: Prefix,
    },
    TileHit {
        pos: [i32; 3],
        damage: i32,
    },
    TileBroken {
        pos: [i32; 3],
        tile: BlockId,
    },
    Placed {
        pos: [i32; 3],
        tile: BlockId,
    },
    /// A wall was placed at `pos` (the item is already taken).
    WallPlaced {
        pos: [i32; 3],
    },
    /// The selected consumable was used. Server-internal: the server applies the
    /// effect with `apply_consumable` and does not forward it.
    Consume {
        item: ItemId,
    },
    /// An item spawned in the world at `pos` (block centre). Server-internal:
    /// the server turns it into a `Drop` and does not forward it.
    ItemDrop {
        stack: ItemStack,
        pos: [f32; 3],
    },
    /// A bow fired one arrow (already taken from the inventory). Server-internal:
    /// the server spawns the projectile and does not forward this.
    Shoot {
        bow: ItemId,
        ammo: ItemId,
        origin: [f32; 3],
        dir: [f32; 3],
        prefix: Prefix,
    },
    /// A magic wand fired a spell. Server-internal: the server checks mana, spends it,
    /// and spawns the spell's projectile.
    Cast {
        wand: ItemId,
        origin: [f32; 3],
        dir: [f32; 3],
        prefix: Prefix,
    },
    /// A shadow orb tile was broken with a hammer.
    ShadowOrbBroken {
        pos: [i32; 3],
    },
}

/// Right click on a door: swaps `DOOR_CLOSED` <-> `DOOR_OPEN` (WorldGen.OpenDoor /
/// CloseDoor). Refused when `user` cannot reach it, or when closing it would
/// trap any body in `bodies`. Returns the new block when the door changed.
pub fn toggle_door(
    w: &mut impl EditQuery,
    user: &PlayerBody,
    bodies: &[&PlayerBody],
    c: [i32; 3],
) -> Option<BlockId> {
    if !w.is_loaded(c[0], c[1], c[2]) || !in_reach(user, c) {
        return None;
    }
    let new = match w.block(c[0], c[1], c[2]) {
        DOOR_CLOSED => DOOR_OPEN,
        DOOR_OPEN if bodies.iter().any(|b| overlaps_player(b, c)) => return None,
        DOOR_OPEN => DOOR_CLOSED,
        _ => return None,
    };
    w.try_set_block(c[0], c[1], c[2], new).then_some(new)
}

/// Whether a wall may go into cell `c`: loaded, empty air, not already walled,
/// and touching a solid block or another wall (WorldGen.PlaceWall needs a
/// neighbouring tile or wall; here 6-connected). Unloaded neighbours do not count.
pub fn wall_placeable(w: &impl LoadedQuery, c: [i32; 3]) -> bool {
    if !w.is_loaded(c[0], c[1], c[2])
        || w.block(c[0], c[1], c[2]) != AIR
        || w.has_wall(c[0], c[1], c[2])
    {
        return false;
    }
    [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ]
    .iter()
    .any(|d| {
        let (x, y, z) = (c[0] + d[0], c[1] + d[1], c[2] + d[2]);
        w.is_loaded(x, y, z) && (w.block(x, y, z).is_solid() || w.has_wall(x, y, z))
    })
}

/// Removes the wall at `c` when `user` can reach it; returns the item to drop.
pub fn break_wall(w: &mut impl EditQuery, user: &PlayerBody, c: [i32; 3]) -> Option<ItemId> {
    if !w.is_loaded(c[0], c[1], c[2]) || !w.has_wall(c[0], c[1], c[2]) || !in_reach(user, c) {
        return None;
    }
    w.try_set_wall(c[0], c[1], c[2], false).then_some(WOOD_WALL)
}

/// Applies a `UseEvent::Consume` for the selected stack: the life crystal raises max
/// life (Player.ItemCheck_UseLifeCrystal) and one item is taken only on success.
pub fn apply_consumable(item: ItemId, v: &mut Vitals, inv: &mut Inventory) -> bool {
    if inv.selected_stack().is_none_or(|s| s.id != item) {
        return false;
    }
    let ok = match item {
        LIFE_CRYSTAL_ITEM => v.use_crystal(),
        MANA_CRYSTAL => v.use_mana_crystal(),
        LESSER_HEALING_POTION => v.use_potion(LESSER_HEALING_HEAL),
        MUSHROOM => {
            if v.use_potion(15) {
                v.potion_delay_ticks = 1800; // 30s Potion Sickness
                true
            } else {
                false
            }
        }
        OBSIDIAN_SKIN_POTION => {
            v.obsidian_skin_ticks = OBSIDIAN_SKIN_TICKS;
            true
        }
        _ => false,
    };
    if ok {
        inv.take_one_selected();
    }
    ok
}

fn healing_item(id: ItemId) -> Option<(i32, i32)> {
    match id {
        LESSER_HEALING_POTION => Some((
            LESSER_HEALING_HEAL,
            crate::player::vitals::POTION_DELAY_TICKS,
        )),
        MUSHROOM => Some((15, 1800)),
        _ => None,
    }
}

fn mana_restore_amount(id: ItemId) -> Option<i32> {
    match id {
        _ => None,
    }
}

fn consume_bag_slot(inv: &mut Inventory, idx: usize) {
    if let Some(slot) = inv.slots.get_mut(idx)
        && let Some(st) = slot.as_mut()
    {
        st.count -= 1;
        if st.count == 0 {
            *slot = None;
        }
    }
}

/// Quick-heal (`H`): consumes the best healing potion/mushroom in `inv.slots[0..BAG]`
/// when `v.life < v.life_max` and Potion Sickness is inactive.
pub fn quick_heal(v: &mut Vitals, inv: &mut Inventory) -> bool {
    if v.life <= 0 || v.life >= v.life_max || v.potion_delay_ticks > 0 {
        return false;
    }
    let best = inv.slots[..crate::inventory::BAG]
        .iter()
        .enumerate()
        .filter_map(|(idx, slot)| {
            let id = slot.as_ref()?.id;
            let (heal, delay) = healing_item(id)?;
            Some((idx, heal, delay))
        })
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)));
    let Some((idx, heal, delay)) = best else {
        return false;
    };
    if !v.use_potion(heal) {
        return false;
    }
    v.potion_delay_ticks = delay;
    consume_bag_slot(inv, idx);
    true
}

/// Quick-mana (`J`): consumes the best mana potion in `inv.slots[0..BAG]`
/// when `v.mana < v.mana_max`.
pub fn quick_mana(v: &mut Vitals, inv: &mut Inventory) -> bool {
    quick_mana_with_max(v, inv, v.mana_max)
}

/// Quick-mana (`J`) using an explicit effective mana cap (including accessories).
pub fn quick_mana_with_max(v: &mut Vitals, inv: &mut Inventory, mana_max: i32) -> bool {
    if v.life <= 0 || v.mana >= mana_max {
        return false;
    }
    let best = inv.slots[..crate::inventory::BAG]
        .iter()
        .enumerate()
        .filter_map(|(idx, slot)| {
            let id = slot.as_ref()?.id;
            let restore = mana_restore_amount(id)?;
            Some((idx, restore))
        })
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)));
    let Some((idx, restore)) = best else {
        return false;
    };
    v.mana = (v.mana + restore).min(mana_max);
    consume_bag_slot(inv, idx);
    true
}

/// Quick-buff (`B`): consumes one of each distinct unapplied buff potion in `inv.slots[0..BAG]`.
pub fn quick_buff(v: &mut Vitals, inv: &mut Inventory) -> bool {
    if v.life <= 0 {
        return false;
    }
    let mut applied = false;
    for idx in 0..crate::inventory::BAG {
        let Some(id) = inv.slots[idx].map(|s| s.id) else {
            continue;
        };
        if id == OBSIDIAN_SKIN_POTION && v.obsidian_skin_ticks == 0 {
            v.obsidian_skin_ticks = OBSIDIAN_SKIN_TICKS;
            consume_bag_slot(inv, idx);
            applied = true;
        }
    }
    applied
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChestLockError {
    RequiresGoldenKey,
}

/// Attempts to unlock a locked chest using a Golden Key from the player's inventory.
/// If `is_locked` is false, returns `Ok(false)` (chest already unlocked).
/// If `is_locked` is true and a Golden Key is present, consumes 1 key and returns `Ok(true)` (unlocked).
/// If `is_locked` is true and no Golden Key is present, returns `Err(ChestLockError::RequiresGoldenKey)`.
pub fn try_unlock_chest(inv: &mut Inventory, is_locked: bool) -> Result<bool, ChestLockError> {
    if !is_locked {
        return Ok(false);
    }
    if inv.remove(GOLDEN_KEY, 1) {
        Ok(true)
    } else {
        Err(ChestLockError::RequiresGoldenKey)
    }
}

/// Returns true if the inventory contains at least one Shadow Key.
pub fn has_shadow_key(inv: &Inventory) -> bool {
    inv.count_of(SHADOW_KEY) > 0
}

pub fn tick_use(
    w: &mut impl EditQuery,
    body: &PlayerBody,
    inv: &mut Inventory,
    st: &mut UseState,
    input: &PlayerInput,
    rng: &mut Rng,
) -> Vec<UseEvent> {
    tick_use_with_mana(w, body, inv, st, input, rng, None)
}

pub fn tick_use_with_mana(
    w: &mut impl EditQuery,
    body: &PlayerBody,
    inv: &mut Inventory,
    st: &mut UseState,
    input: &PlayerInput,
    rng: &mut Rng,
    mana: Option<i32>,
) -> Vec<UseEvent> {
    let mut ev = Vec::new();
    // Terraria's Player.ItemCheck: a hotbar selection takes effect at once
    // unless a use animation is running. While one runs, latch the request and
    // apply it when the swing ends, so the swing always finishes with the item
    // it started with (and a held use button does not switch mid-swing).
    let want = input.slot as usize;
    if want < 10 {
        if want == inv.selected {
            st.pending_slot = None;
        } else if st.anim == 0 {
            inv.selected = want;
            st.time = 0;
            st.pending_slot = None;
        } else {
            st.pending_slot = Some(want);
        }
    }
    let sel: Option<ItemStack> = inv.selected_stack().copied();

    // Tick active multi-shot burst (e.g. Clockwork Assault Rifle) without extra ammo cost.
    if st.burst_left > 0 {
        if st.burst_delay > 1 {
            st.burst_delay -= 1;
        } else if let (Some(bow), Some(ammo)) = (st.burst_weapon, st.burst_ammo) {
            let eye = [body.pos[0], body.pos[1] + EYE_HEIGHT, body.pos[2]];
            let (sy, cy) = input.yaw.sin_cos();
            let (sp, cp) = input.pitch.sin_cos();
            let dir = [-sy * cp, sp, -cy * cp];
            ev.push(UseEvent::Shoot {
                bow,
                ammo,
                prefix: st.burst_prefix.unwrap_or(Prefix(0)),
                origin: eye,
                dir,
            });
            st.burst_left -= 1;
            if st.burst_left > 0 {
                st.burst_delay = 4;
            } else {
                st.burst_weapon = None;
                st.burst_ammo = None;
                st.burst_prefix = None;
            }
        } else {
            st.burst_left = 0;
        }
    }

    // Auto-use: start a new use when idle and the button is held. Terraria's
    // `autoReuse` items repeat while held; everything else needs a fresh press.
    if st.anim == 0 && input.use_held {
        if let Some(s) = sel {
            let data = item_data(s.id);
            let fresh_press = !st.last_use_held;
            // Player.PickAmmo: a gun or bow with no ammo neither animates nor
            // emits a swing (gameplay spec). Same for magic weapons without enough mana.
            let no_ammo =
                data.class == ItemClass::Ranged && inv.first_ammo_slot_for(s.id).is_none();
            let no_mana = data.class == ItemClass::Magic
                && mana.is_some_and(|m| m < crate::item::mana_cost(data.mana, s.prefix));
            if (data.auto_reuse || fresh_press) && !no_ammo && !no_mana {
                // Item.Prefix: useAnimation = Round(useAnimation * spd).
                let molten = if s.id == crate::item::FIRECRACKER {
                    1.0
                } else {
                    body.molten_melee_use_time_mult(data.class)
                };
                let spd = prefix_stats(s.prefix).use_time * molten;
                st.anim = round_ties_even(data.use_animation as f32 * spd).max(1);
                ev.push(UseEvent::Swing {
                    item: s.id,
                    prefix: s.prefix,
                });
            }
        }
    }

    // ItemCheck: the effect fires when itemTime is 0 during an animation.
    if st.anim > 0 && st.time == 0 {
        if let Some(s) = sel {
            let data = item_data(s.id);
            // Molten Armor's +7% melee speed shortens the swing of melee and
            // tool items (excluding Firecracker whip); guns, bows and magic keep their own use time.
            let mut use_time_mult = prefix_stats(s.prefix).use_time;
            if s.id != crate::item::FIRECRACKER {
                use_time_mult *= body.molten_melee_use_time_mult(data.class);
            }
            let use_time = round_ties_even(data.use_time as f32 * use_time_mult);
            let eye = [body.pos[0], body.pos[1] + EYE_HEIGHT, body.pos[2]];
            let (sy, cy) = input.yaw.sin_cos();
            let (sp, cp) = input.pitch.sin_cos();
            let dir = [-sy * cp, sp, -cy * cp];
            let is_bucket = s.id == EMPTY_BUCKET || s.id == WATER_BUCKET || s.id == LAVA_BUCKET;
            let hit = if is_bucket {
                raycast_where(eye, dir, RAY_MAX, |c| {
                    w.is_loaded(c[0], c[1], c[2])
                        && (w.block(c[0], c[1], c[2]) != AIR
                            || w.liquid(c[0], c[1], c[2]).is_some())
                })
            } else {
                raycast_loaded(&*w, eye, dir, RAY_MAX)
            };
            if data.class == ItemClass::Ranged {
                // Player.PickAmmo (gameplay spec): ammo slots first, then the
                // bag, in slot order; with none, the bow/gun does not fire.
                let slot = inv.first_ammo_slot_for(s.id);
                if let Some(i) = slot {
                    // Minishark's 33% ammo save (Player.PickAmmo ammoUseChance).
                    let saved = s.id == MINISHARK && rng.next_u32() % 3 == 0;
                    let a = inv.slots[i].as_mut().unwrap();
                    let ammo = a.id;
                    if !saved {
                        a.count -= 1;
                        if a.count == 0 {
                            inv.slots[i] = None;
                        }
                    }
                    ev.push(UseEvent::Shoot {
                        bow: s.id,
                        ammo,
                        prefix: s.prefix,
                        origin: eye,
                        dir,
                    });
                    if s.id == crate::item::CLOCKWORK_ASSAULT_RIFLE {
                        st.burst_left = 2;
                        st.burst_delay = 4;
                        st.burst_weapon = Some(s.id);
                        st.burst_ammo = Some(ammo);
                        st.burst_prefix = Some(s.prefix);
                        // Player.ItemCheck: reuseDelay = 14 after the burst animation.
                        st.time = use_time + 14;
                        st.anim = st.anim.max(st.time);
                    } else {
                        st.time = use_time;
                    }
                }
            } else if data.class == ItemClass::Magic {
                ev.push(UseEvent::Cast {
                    wand: s.id,
                    prefix: s.prefix,
                    origin: eye,
                    dir,
                });
                st.time = use_time;
            } else if data.pick > 0 || data.axe > 0 || data.hammer > 0 {
                let target = hit.filter(|h| in_reach(body, h.block)).and_then(|h| {
                    let [x, y, z] = h.block;
                    let tile = w.block(x, y, z);
                    // Player.ItemCheck_UseMiningTools: trees take only axe
                    // damage, every other tile only pickaxe damage.
                    // Demon altars yield only to hammers with >= 80 hammer power (Pwnhammer).
                    let dmg = if tile == TRUNK {
                        axe_damage(tile, data.axe)
                    } else if tile == DEMON_ALTAR {
                        if data.hammer >= 80 { data.hammer } else { 0 }
                    } else if tile == SHADOW_ORB {
                        if data.hammer > 0 { data.hammer } else { 0 }
                    } else if data.pick > 0 && data.pick >= min_pick(tile) {
                        pick_damage(tile, data.pick)
                    } else {
                        0
                    };
                    (dmg > 0).then_some((h, tile, dmg))
                });
                if let Some((h, tile, dmg)) = target {
                    let [x, y, z] = h.block;
                    // Player.PickTile: AddDamage, break at >= 100.
                    let e = st.tile_damage.entry(h.block).or_insert((0, 0));
                    e.0 += dmg;
                    e.1 = 60;
                    let total = e.0;
                    if total >= 100 && tile == TRUNK {
                        st.tile_damage.remove(&h.block);
                        chop_tree(w, h.block, data.axe, rng, &mut ev);
                        st.time = use_time;
                    } else if total >= 100 {
                        // Only a successful edit breaks the tile or drops loot.
                        let broke = w.try_set_block(x, y, z, AIR);
                        st.tile_damage.remove(&h.block);
                        if broke {
                            ev.push(UseEvent::TileBroken { pos: h.block, tile });
                            if tile == SHADOW_ORB {
                                ev.push(UseEvent::ShadowOrbBroken { pos: h.block });
                            }
                            if let Some(item) = tile.drop_item() {
                                ev.push(UseEvent::ItemDrop {
                                    stack: ItemStack {
                                        id: item,
                                        count: 1,
                                        prefix: crate::item::Prefix(0),
                                    },
                                    pos: [x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5],
                                });
                            }
                            st.time = use_time;
                        }
                    } else {
                        ev.push(UseEvent::TileHit {
                            pos: h.block,
                            damage: total,
                        });
                        prune(&*w, st);
                        st.time = use_time;
                    }
                }
            } else if data.class == ItemClass::Consumable {
                ev.push(UseEvent::Consume { item: s.id });
                st.time = use_time;
            } else if data.class == ItemClass::Wall {
                if let Some(h) = hit {
                    let c = [
                        h.block[0] + h.normal[0],
                        h.block[1] + h.normal[1],
                        h.block[2] + h.normal[2],
                    ];
                    if wall_placeable(&*w, c)
                        && in_reach(body, c)
                        && w.try_set_wall(c[0], c[1], c[2], true)
                    {
                        inv.take_one_selected();
                        ev.push(UseEvent::WallPlaced { pos: c });
                        st.time = st.anim.max(use_time);
                    }
                }
            } else if let Some(tile) = data.create_tile {
                if let Some(h) = hit {
                    let c = [
                        h.block[0] + h.normal[0],
                        h.block[1] + h.normal[1],
                        h.block[2] + h.normal[2],
                    ];
                    // Unloaded space is not air: never place into, or support a
                    // placement on, a chunk the server hasn't got.
                    let free = w.is_loaded(c[0], c[1], c[2]) && w.block(c[0], c[1], c[2]) == AIR;
                    let blocks_player = tile.is_solid() && overlaps_player(body, c);
                    let supported = tile != TORCH
                        || [[0, -1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]
                            .iter()
                            .any(|n| {
                                let (nx, ny, nz) = (c[0] + n[0], c[1] + n[1], c[2] + n[2]);
                                w.is_loaded(nx, ny, nz) && w.block(nx, ny, nz).is_solid()
                            });
                    // Tables and chairs stand on a solid block or a platform.
                    let floor_ok = !matches!(tile, TABLE | CHAIR) || {
                        let (x, y, z) = (c[0], c[1] - 1, c[2]);
                        w.is_loaded(x, y, z) && {
                            let b = w.block(x, y, z);
                            b.is_solid() || b.is_platform()
                        }
                    };
                    if free && !blocks_player && supported && floor_ok && in_reach(body, c) {
                        // Consume the stack only if the edit really happened.
                        if w.try_set_block(c[0], c[1], c[2], tile) {
                            inv.take_one_selected();
                            ev.push(UseEvent::Placed { pos: c, tile });
                            st.time = st.anim.max(use_time);
                        }
                    }
                }
            } else if s.id == EMPTY_BUCKET {
                if let Some(h) = hit {
                    let [x, y, z] = h.block;
                    if in_reach(body, [x, y, z]) {
                        if let Some(liq) = w.liquid(x, y, z) {
                            if liq.amount >= 128 {
                                if w.set_liquid(x, y, z, None) {
                                    w.wake_liquid([x, y, z]);
                                    inv.take_one_selected();
                                    let filled_id = if liq.kind == LiquidKind::Water {
                                        WATER_BUCKET
                                    } else {
                                        LAVA_BUCKET
                                    };
                                    let filled = ItemStack {
                                        id: filled_id,
                                        count: 1,
                                        prefix: crate::item::Prefix(0),
                                    };
                                    if let Some(rem) = inv.add(filled) {
                                        ev.push(UseEvent::ItemDrop {
                                            stack: rem,
                                            pos: body.pos,
                                        });
                                    }
                                    st.time = use_time;
                                }
                            }
                        }
                    }
                }
            } else if s.id == WATER_BUCKET || s.id == LAVA_BUCKET {
                if let Some(h) = hit {
                    let c = [
                        h.block[0] + h.normal[0],
                        h.block[1] + h.normal[1],
                        h.block[2] + h.normal[2],
                    ];
                    let kind = if s.id == WATER_BUCKET {
                        LiquidKind::Water
                    } else {
                        LiquidKind::Lava
                    };
                    let free = w.is_loaded(c[0], c[1], c[2])
                        && !w.block(c[0], c[1], c[2]).is_solid()
                        && match w.liquid(c[0], c[1], c[2]) {
                            None => true,
                            Some(existing) => existing.kind == kind && existing.amount < 255,
                        };
                    if free && in_reach(body, c) {
                        if w.set_liquid(c[0], c[1], c[2], Some(Liquid::new(kind, 255))) {
                            w.wake_liquid(c);
                            inv.take_one_selected();
                            let empty = ItemStack {
                                id: EMPTY_BUCKET,
                                count: 1,
                                prefix: crate::item::Prefix(0),
                            };
                            if let Some(rem) = inv.add(empty) {
                                ev.push(UseEvent::ItemDrop {
                                    stack: rem,
                                    pos: body.pos,
                                });
                            }
                            st.time = use_time;
                        }
                    }
                }
            } else if s.id == crate::item::MAGIC_MIRROR {
                st.time = use_time;
            }
        }
    }

    // Player.Update: itemAnimation and itemTime count down after ItemCheck.
    if st.anim > 0 {
        st.anim -= 1;
    }
    if st.time > 0 {
        st.time -= 1;
    }
    if st.anim == 0 {
        if let Some(want) = st.pending_slot.take() {
            inv.selected = want;
            st.time = 0;
        }
    }
    st.last_use_held = input.use_held;
    ev
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{
        DEMON_ALTAR, DEMONITE_ORE, DIRT, EBONSTONE, OBSIDIAN, SOLID_UNLOADED, SPIKES, STONE,
    };
    use crate::chunk::ChunkPos;
    use crate::inventory::AMMO0;
    use crate::item::*;

    #[derive(Default)]
    struct TW(
        HashMap<[i32; 3], BlockId>,
        std::collections::HashSet<[i32; 3]>,
        HashMap<[i32; 3], Liquid>,
        Vec<[i32; 3]>,
    );
    impl BlockQuery for TW {
        fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
            *self.0.get(&[x, y, z]).unwrap_or(&AIR)
        }
        fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid> {
            self.2.get(&[x, y, z]).copied()
        }
    }
    impl LoadedQuery for TW {
        fn is_loaded(&self, _x: i32, _y: i32, _z: i32) -> bool {
            true
        }
        fn has_wall(&self, x: i32, y: i32, z: i32) -> bool {
            self.1.contains(&[x, y, z])
        }
    }
    impl EditQuery for TW {
        fn try_set_block(&mut self, x: i32, y: i32, z: i32, b: BlockId) -> bool {
            self.0.insert([x, y, z], b);
            if b != AIR {
                self.2.remove(&[x, y, z]);
            }
            true
        }
        fn try_set_wall(&mut self, x: i32, y: i32, z: i32, on: bool) -> bool {
            if on {
                self.1.insert([x, y, z]);
            } else {
                self.1.remove(&[x, y, z]);
            }
            true
        }
        fn set_liquid(&mut self, x: i32, y: i32, z: i32, l: Option<Liquid>) -> bool {
            if let Some(liq) = l {
                self.2.insert([x, y, z], liq);
            } else {
                self.2.remove(&[x, y, z]);
            }
            true
        }
        fn wake_liquid(&mut self, pos: [i32; 3]) {
            self.3.push(pos);
        }
    }

    type TestWorld = TW;
    impl TW {
        fn new() -> Self {
            Self::default()
        }
        fn set_block(&mut self, x: i32, y: i32, z: i32, b: BlockId) {
            self.0.insert([x, y, z], b);
        }
    }

    /// Only chunk (0, 0, 0) is loaded. Missing chunks mimic a physics query:
    /// either `SOLID_UNLOADED` (server) or `AIR` (sim `World`). Interaction must
    /// ignore them either way.
    struct Missing {
        blocks: HashMap<[i32; 3], BlockId>,
        solid_unloaded: bool,
    }
    impl Missing {
        fn new(solid_unloaded: bool) -> Self {
            Missing {
                blocks: HashMap::new(),
                solid_unloaded,
            }
        }
        fn loaded(x: i32, y: i32, z: i32) -> bool {
            ChunkPos::of_block(x, y, z) == ChunkPos { x: 0, y: 0, z: 0 }
        }
    }
    impl BlockQuery for Missing {
        fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
            if Self::loaded(x, y, z) {
                *self.blocks.get(&[x, y, z]).unwrap_or(&AIR)
            } else if self.solid_unloaded {
                SOLID_UNLOADED
            } else {
                AIR
            }
        }
    }
    impl LoadedQuery for Missing {
        fn is_loaded(&self, x: i32, y: i32, z: i32) -> bool {
            Self::loaded(x, y, z)
        }
    }
    impl EditQuery for Missing {
        fn try_set_block(&mut self, x: i32, y: i32, z: i32, b: BlockId) -> bool {
            if !Self::loaded(x, y, z) {
                return false;
            }
            self.blocks.insert([x, y, z], b);
            true
        }
        fn try_set_wall(&mut self, x: i32, y: i32, z: i32, _on: bool) -> bool {
            Self::loaded(x, y, z)
        }
    }

    fn body() -> PlayerBody {
        PlayerBody::new([0.5, 0.0, 0.5])
    }
    fn body_at(pos: [f32; 3]) -> PlayerBody {
        PlayerBody::new(pos)
    }
    fn held() -> PlayerInput {
        PlayerInput {
            use_held: true,
            ..Default::default()
        }
    }
    fn inv_with(id: ItemId, count: u32, prefix: Prefix) -> Inventory {
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack { id, count, prefix });
        inv
    }
    /// Runs ticks; returns every event.
    fn run<W: EditQuery>(
        w: &mut W,
        inv: &mut Inventory,
        st: &mut UseState,
        i: &PlayerInput,
        n: usize,
    ) -> Vec<UseEvent> {
        (0..n)
            .flat_map(|_| tick_use(w, &body(), inv, st, i, &mut Rng::new(7)))
            .collect()
    }
    /// Pick strikes and ticks until the block at `at` is gone.
    fn strikes_to_break(tile: BlockId, prefix: Prefix) -> (usize, usize) {
        strikes_with(COPPER_PICKAXE, tile, prefix)
    }
    fn strikes_with(tool: ItemId, tile: BlockId, prefix: Prefix) -> (usize, usize) {
        let at = [0, 1, -2];
        let mut w = TW::default();
        w.0.insert(at, tile);
        let mut inv = inv_with(tool, 1, prefix);
        let mut st = UseState::default();
        let (mut strikes, mut ticks) = (0, 0);
        while w.block(at[0], at[1], at[2]) != AIR && ticks < 1000 {
            for e in tick_use(
                &mut w,
                &body(),
                &mut inv,
                &mut st,
                &held(),
                &mut Rng::new(7),
            ) {
                if matches!(e, UseEvent::TileHit { .. } | UseEvent::TileBroken { .. }) {
                    strikes += 1;
                }
            }
            ticks += 1;
        }
        (strikes, ticks)
    }

    #[test]
    fn potion_not_consumed_when_refused() {
        let mut inv = inv_with(LESSER_HEALING_POTION, 2, Prefix(0));
        let mut v = Vitals::new();
        assert!(!apply_consumable(LESSER_HEALING_POTION, &mut v, &mut inv)); // full life
        assert_eq!(inv.count_of(LESSER_HEALING_POTION), 2);
        v.hurt(60);
        assert!(apply_consumable(LESSER_HEALING_POTION, &mut v, &mut inv));
        assert_eq!(inv.count_of(LESSER_HEALING_POTION), 1);
        v.hurt(10);
        assert!(!apply_consumable(LESSER_HEALING_POTION, &mut v, &mut inv)); // sick
        assert_eq!(inv.count_of(LESSER_HEALING_POTION), 1);
    }

    #[test]
    fn obsidian_skin_potion_is_consumed_on_use() {
        use crate::item::OBSIDIAN_SKIN_POTION;
        use crate::player::consts::OBSIDIAN_SKIN_TICKS;

        let mut inv = inv_with(OBSIDIAN_SKIN_POTION, 2, Prefix(0));
        let mut v = Vitals::new();
        assert_eq!(v.obsidian_skin_ticks, 0);
        assert!(apply_consumable(OBSIDIAN_SKIN_POTION, &mut v, &mut inv));
        assert_eq!(inv.count_of(OBSIDIAN_SKIN_POTION), 1);
        assert_eq!(v.obsidian_skin_ticks, OBSIDIAN_SKIN_TICKS);
        assert_eq!(v.potion_delay_ticks, 0);
    }

    #[test]
    fn quick_heal_picks_best_potion_and_respects_sickness_and_full_hp() {
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack {
            id: MUSHROOM,
            count: 3,
            prefix: Prefix(0),
        });
        inv.slots[5] = Some(ItemStack {
            id: LESSER_HEALING_POTION,
            count: 2,
            prefix: Prefix(0),
        });

        let mut v = Vitals::new();
        // Full life -> does nothing
        assert!(!quick_heal(&mut v, &mut inv));
        assert_eq!(inv.count_of(LESSER_HEALING_POTION), 2);
        assert_eq!(inv.count_of(MUSHROOM), 3);

        // Hurt -> picks Lesser Healing Potion (50) over Mushroom (15)
        v.hurt(70);
        assert_eq!(v.life, 30);
        assert!(quick_heal(&mut v, &mut inv));
        assert_eq!(v.life, 80);
        assert_eq!(
            v.potion_delay_ticks,
            crate::player::vitals::POTION_DELAY_TICKS
        );
        assert_eq!(inv.count_of(LESSER_HEALING_POTION), 1);
        assert_eq!(inv.count_of(MUSHROOM), 3);

        // Potion Sickness active -> refused
        v.hurt(10);
        assert!(!quick_heal(&mut v, &mut inv));
        assert_eq!(inv.count_of(LESSER_HEALING_POTION), 1);
        assert_eq!(inv.count_of(MUSHROOM), 3);

        // Clear sickness and remove healing potions -> falls back to Mushroom (30s sickness)
        v.potion_delay_ticks = 0;
        inv.slots[5] = None;
        assert!(quick_heal(&mut v, &mut inv));
        assert_eq!(v.life, 85);
        assert_eq!(v.potion_delay_ticks, 1800);
        assert_eq!(inv.count_of(MUSHROOM), 2);
    }

    #[test]
    fn quick_mana_returns_false_at_full_or_without_potions() {
        let mut inv = Inventory::empty();
        let mut v = Vitals::new();
        assert!(!quick_mana(&mut v, &mut inv));
        v.mana = 5;
        assert!(!quick_mana(&mut v, &mut inv));
    }

    #[test]
    fn quick_buff_applies_once_per_unapplied_buff_potion() {
        use crate::item::OBSIDIAN_SKIN_POTION;
        use crate::player::consts::OBSIDIAN_SKIN_TICKS;

        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack {
            id: OBSIDIAN_SKIN_POTION,
            count: 2,
            prefix: Prefix(0),
        });
        inv.slots[1] = Some(ItemStack {
            id: OBSIDIAN_SKIN_POTION,
            count: 1,
            prefix: Prefix(0),
        });
        let mut v = Vitals::new();
        assert!(quick_buff(&mut v, &mut inv));
        assert_eq!(v.obsidian_skin_ticks, OBSIDIAN_SKIN_TICKS);
        assert_eq!(inv.count_of(OBSIDIAN_SKIN_POTION), 2);

        // Already active -> does not consume another
        assert!(!quick_buff(&mut v, &mut inv));
        assert_eq!(inv.count_of(OBSIDIAN_SKIN_POTION), 2);
    }

    #[test]
    fn door_cannot_close_on_a_player() {
        let mut w = TW::default();
        w.0.insert([0, 0, 0], DOOR_OPEN);
        let inside = body_at([0.5, 0.0, 0.5]);
        assert_eq!(toggle_door(&mut w, &inside, &[&inside], [0, 0, 0]), None);
        assert_eq!(w.block(0, 0, 0), DOOR_OPEN);
        // Clear of the door it closes, and a closed door always opens.
        let away = body_at([0.5, 0.0, 3.5]);
        assert_eq!(
            toggle_door(&mut w, &away, &[&away], [0, 0, 0]),
            Some(DOOR_CLOSED)
        );
        assert_eq!(
            toggle_door(&mut w, &away, &[&inside], [0, 0, 0]),
            Some(DOOR_OPEN)
        );
    }

    #[test]
    fn door_toggle_refuses_out_of_reach_and_other_blocks() {
        let mut w = TW::default();
        w.0.insert([40, 0, 0], DOOR_CLOSED);
        w.0.insert([1, 0, 0], STONE);
        let b = body();
        assert_eq!(toggle_door(&mut w, &b, &[&b], [40, 0, 0]), None);
        assert_eq!(toggle_door(&mut w, &b, &[&b], [1, 0, 0]), None);
    }

    #[test]
    fn place_wall_consumes_item() {
        let mut w = TW::default();
        w.0.insert([0, 1, -3], STONE); // backing solid; target cell is [0,1,-2]
        let mut inv = inv_with(WOOD_WALL, 2, Prefix(0));
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &held(), 1);
        assert!(w.has_wall(0, 1, -2));
        assert_eq!(w.block(0, 1, -2), AIR);
        assert_eq!(inv.count_of(WOOD_WALL), 1);
        assert!(
            ev.iter()
                .any(|e| matches!(e, UseEvent::WallPlaced { pos: [0, 1, -2] }))
        );
        // Breaking returns the item and clears the bit; out of reach is refused.
        assert_eq!(break_wall(&mut w, &body(), [0, 1, -2]), Some(WOOD_WALL));
        assert!(!w.has_wall(0, 1, -2));
        w.1.insert([40, 0, 0]);
        assert_eq!(break_wall(&mut w, &body(), [40, 0, 0]), None);
    }

    #[test]
    fn place_wall_on_solid_is_refused() {
        let mut w = TW::default();
        w.0.insert([0, 1, -2], STONE); // the cell in front is solid? hit is the stone itself:
        let mut inv = inv_with(WOOD_WALL, 2, Prefix(0));
        // Ray hits the stone at z=-2, so the target cell is z=-1 (air, touching stone): allowed.
        // The stone cell itself must never take a wall.
        assert!(!wall_placeable(&w, [0, 1, -2]));
        run(&mut w, &mut inv, &mut UseState::default(), &held(), 1);
        assert!(!w.has_wall(0, 1, -2));
        // A floating cell with no solid or wall neighbour is refused too.
        assert!(!wall_placeable(&TW::default(), [5, 5, 5]));
        // Unloaded space is refused.
        assert!(!wall_placeable(&Missing::new(true), [0, 0, 40]));
    }

    #[test]
    fn table_and_chair_need_a_floor() {
        use crate::block::{CHAIR, TABLE};
        for (item, tile) in [(WOODEN_TABLE, TABLE), (WOODEN_CHAIR, CHAIR)] {
            // Looking at the side of a stone block with nothing below the target cell.
            let mut w = TW::default();
            w.0.insert([0, 1, -3], STONE);
            let mut inv = inv_with(item, 1, Prefix(0));
            run(&mut w, &mut inv, &mut UseState::default(), &held(), 60);
            assert_eq!(w.block(0, 1, -2), AIR, "floating {tile:?}");
            // With a floor under the target cell it is placed.
            w.0.insert([0, 0, -2], STONE);
            run(&mut w, &mut inv, &mut UseState::default(), &held(), 60);
            assert_eq!(w.block(0, 1, -2), tile);
        }
    }

    #[test]
    fn shooting_consumes_one_ammo() {
        let mut w = TW::default();
        let mut inv = inv_with(WOODEN_BOW, 1, Prefix(0));
        inv.slots[3] = Some(ItemStack {
            id: WOODEN_ARROW,
            count: 3,
            prefix: Prefix(0),
        });
        // One bow use (useTime 30) in 20 ticks: exactly one arrow leaves.
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &held(), 20);
        let shots: Vec<_> = ev
            .iter()
            .filter(|e| matches!(e, UseEvent::Shoot { .. }))
            .collect();
        assert_eq!(shots.len(), 1);
        assert_eq!(inv.slots[3].unwrap().count, 2);
    }

    #[test]
    fn bow_uses_ammo_slots_before_bag() {
        let mut w = TW::default();
        let mut inv = inv_with(WOODEN_BOW, 1, Prefix(0));
        let arrows = |count| {
            Some(ItemStack {
                id: WOODEN_ARROW,
                count,
                prefix: Prefix(0),
            })
        };
        inv.slots[3] = arrows(3);
        inv.slots[AMMO0] = arrows(2);
        run(&mut w, &mut inv, &mut UseState::default(), &held(), 20);
        assert_eq!(inv.slots[AMMO0].unwrap().count, 1);
        assert_eq!(inv.slots[3].unwrap().count, 3);
    }

    #[test]
    fn no_ammo_means_no_shot() {
        let mut w = TW::default();
        let mut inv = inv_with(WOODEN_BOW, 1, Prefix(0));
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &held(), 100);
        assert!(!ev.iter().any(|e| matches!(e, UseEvent::Shoot { .. })));
    }

    #[test]
    fn raycast_hits_face_and_normal() {
        let mut w = TW::default();
        w.0.insert([0, 0, 3], STONE);
        let h = raycast(&w, [0.5, 0.5, 0.5], [0.0, 0.0, 1.0], 10.0).unwrap();
        assert_eq!((h.block, h.normal), ([0, 0, 3], [0, 0, -1]));
        let h = raycast(&w, [0.5, 5.5, 3.5], [0.0, -1.0, 0.0], 10.0).unwrap();
        assert_eq!((h.block, h.normal), ([0, 0, 3], [0, 1, 0]));
    }

    #[test]
    fn raycast_respects_max() {
        let mut w = TW::default();
        w.0.insert([0, 0, 3], STONE);
        assert!(raycast(&w, [0.5, 0.5, 0.5], [0.0, 0.0, 1.0], 2.0).is_none());
        assert!(raycast(&w, [0.5, 0.5, 0.5], [0.0, 0.0, 1.0], 2.6).is_some());
    }

    #[test]
    fn pickaxe_breaks_dirt_in_terraria_hits() {
        // Player.GetPickaxeDamage: dirt (tile 0) gets pickPower twice (35 + 35 = 70 per
        // strike), other tiles once; Player.PickTile breaks at >= 100 (tile damage tracking.AddDamage).
        // Copper pickaxe pick 35: dirt 2 strikes, stone 3 (35 * 3 = 105). tile damage tracking.Prune only
        // decays after 12+ prunes, so nothing decays here.
        assert_eq!(strikes_to_break(DIRT, Prefix(0)).0, 2);
        assert_eq!(strikes_to_break(STONE, Prefix(0)).0, 3);
    }

    #[test]
    fn all_current_tiles_mineable_with_copper_pick() {
        // Vanilla does not gate ores or stone (gameplay spec lists none of them).
        let pick = item_data(COPPER_PICKAXE).pick;
        for t in [
            DIRT,
            STONE,
            crate::block::IRON,
            crate::block::COPPER,
            crate::block::GOLD,
            crate::block::SILVER,
            crate::block::SAND,
            crate::block::MUD,
            crate::block::SNOW,
        ] {
            assert_eq!(min_pick(t), 0);
            assert!(pick >= min_pick(t));
            assert!(strikes_to_break(t, Prefix(0)).0 > 0, "{t:?}");
        }
    }

    #[test]
    fn min_pick_gate_blocks_weak_pick() {
        let table = [(STONE, 55)];
        assert_eq!(min_pick_in(&table, STONE), 55);
        assert_eq!(min_pick_in(&table, DIRT), 0);
        assert!(item_data(COPPER_PICKAXE).pick < min_pick_in(&table, STONE));
        assert!(item_data(crate::item::GOLD_PICKAXE).pick >= min_pick_in(&table, STONE));
    }

    #[test]
    fn demonite_requires_pick_tier() {
        // gameplay spec: Demonite Ore demands pick power 55 (gold tier).
        assert_eq!(min_pick(DEMONITE_ORE), 55);
        assert!(item_data(IRON_PICKAXE).pick < 55); // 40
        assert!(item_data(SILVER_PICKAXE).pick < 55); // 45
        assert!(item_data(GOLD_PICKAXE).pick >= 55); // 55 meets it
        assert_eq!(min_pick(EBONSTONE), 65);

        // Exercise the gate through real swing/tick code, not just the table:
        // weaker picks must never damage it; gold must actually break it.
        for pick in [IRON_PICKAXE, SILVER_PICKAXE] {
            let (strikes, ticks) = strikes_with(pick, DEMONITE_ORE, Prefix(0));
            assert_eq!((strikes, ticks), (0, 1000), "{pick:?} damaged demonite");
        }
        let (strikes, ticks) = strikes_with(GOLD_PICKAXE, DEMONITE_ORE, Prefix(0));
        assert!(strikes > 0 && ticks < 1000, "gold did not break demonite");
        for pick in [COPPER_PICKAXE, GOLD_PICKAXE] {
            let (strikes, ticks) = strikes_with(pick, EBONSTONE, Prefix(0));
            assert_eq!((strikes, ticks), (0, 1000), "{pick:?} damaged ebonstone");
        }
        let (strikes, ticks) = strikes_with(NIGHTMARE_PICKAXE, EBONSTONE, Prefix(0));
        assert!(
            strikes > 0 && ticks < 1000,
            "nightmare did not break ebonstone"
        );
    }

    #[test]
    fn nightmare_pickaxe_meets_hellstone_mining_tier() {
        use crate::block::{DEMONITE_ORE, HELLSTONE};
        use crate::item::{GOLD_PICKAXE, NIGHTMARE_PICKAXE};

        assert_eq!(min_pick(DEMONITE_ORE), 55);
        assert_eq!(min_pick(HELLSTONE), 65);

        let nightmare_data = item_data(NIGHTMARE_PICKAXE);
        assert_eq!(nightmare_data.pick, 65);
        assert!(
            nightmare_data.pick >= min_pick(DEMONITE_ORE),
            "Nightmare Pickaxe must be able to mine Demonite Ore (pick 65 >= 55)"
        );
        assert!(
            nightmare_data.pick >= min_pick(HELLSTONE),
            "Nightmare Pickaxe must be able to mine Hellstone (pick 65 >= 65)"
        );

        let gold_data = item_data(GOLD_PICKAXE);
        assert_eq!(gold_data.pick, 55);
        assert!(gold_data.pick >= min_pick(DEMONITE_ORE));
        assert!(
            gold_data.pick < min_pick(HELLSTONE),
            "Gold Pickaxe cannot mine Hellstone (pick 55 < 65)"
        );

        // Interaction test: Gold Pickaxe fails to break Hellstone, Nightmare Pickaxe succeeds.
        let at = [0, 1, -2];
        let mut w_gold = TW::default();
        w_gold.0.insert(at, HELLSTONE);
        let mut inv_gold = inv_with(GOLD_PICKAXE, 1, Prefix(0));
        let mut st_gold = UseState::default();
        for _ in 0..100 {
            tick_use(
                &mut w_gold,
                &body(),
                &mut inv_gold,
                &mut st_gold,
                &held(),
                &mut Rng::new(7),
            );
        }
        assert_eq!(
            w_gold.block(at[0], at[1], at[2]),
            HELLSTONE,
            "Gold pickaxe must not break Hellstone"
        );

        let mut w_night = TW::default();
        w_night.0.insert(at, HELLSTONE);
        let mut inv_night = inv_with(NIGHTMARE_PICKAXE, 1, Prefix(0));
        let mut st_night = UseState::default();
        let mut broken = false;
        for _ in 0..200 {
            let evs = tick_use(
                &mut w_night,
                &body(),
                &mut inv_night,
                &mut st_night,
                &held(),
                &mut Rng::new(7),
            );
            if evs.iter().any(|e| matches!(e, UseEvent::TileBroken { .. })) {
                broken = true;
                break;
            }
        }
        assert!(broken, "Nightmare Pickaxe must break Hellstone");
    }

    #[test]
    fn molten_hellforge_requires_pick_65() {
        // The Hellforge is Nightmare/Deathbringer-tier furniture.
        assert_eq!(min_pick(crate::block::HELLFORGE), 65);
        assert!(item_data(GOLD_PICKAXE).pick < min_pick(crate::block::HELLFORGE));
        assert!(item_data(NIGHTMARE_PICKAXE).pick >= min_pick(crate::block::HELLFORGE));
    }

    #[test]
    fn dungeon_bricks_require_nightmare_pickaxe() {
        assert_eq!(min_pick(BLUE_DUNGEON_BRICK), 100);
        assert_eq!(min_pick(GREEN_DUNGEON_BRICK), 100);
        assert_eq!(min_pick(PINK_DUNGEON_BRICK), 100);
        assert_eq!(min_pick(SPIKES), 0);

        let nightmare_pick = item_data(NIGHTMARE_PICKAXE).pick;
        let molten_pick = item_data(MOLTEN_PICKAXE).pick;
        assert!(nightmare_pick < min_pick(BLUE_DUNGEON_BRICK));
        assert!(molten_pick >= min_pick(BLUE_DUNGEON_BRICK));
    }

    #[test]
    fn altar_unbreakable_without_hammer() {
        // gameplay spec: damage to an altar comes from a Hammer
        // (WorldGen canHammer), and no hammer item exists here yet
        // (TODO(hammer)), so every tool is refused: no damages, no break, no
        // drop, no hardmode-style side effects.
        for tool in [GOLD_PICKAXE, GOLD_AXE, WOODEN_SWORD] {
            let at = [0, 1, -2];
            let mut w = TW::default();
            w.0.insert(at, DEMON_ALTAR);
            let mut inv = inv_with(tool, 1, Prefix(0));
            let mut st = UseState::default();
            let mut rng = Rng::new(7);
            for _ in 0..400 {
                for e in tick_use(&mut w, &body(), &mut inv, &mut st, &held(), &mut rng) {
                    assert!(
                        !matches!(
                            e,
                            UseEvent::TileHit { .. }
                                | UseEvent::TileBroken { .. }
                                | UseEvent::ItemDrop { .. }
                        ),
                        "{tool:?} harmed the altar: {e:?}"
                    );
                }
            }
            assert_eq!(w.block(at[0], at[1], at[2]), DEMON_ALTAR);
            assert!(st.tile_damage.is_empty(), "{tool:?} accumulated damage");
        }
    }

    #[test]
    fn better_pick_breaks_stone_faster() {
        let (_, copper) = strikes_with(COPPER_PICKAXE, STONE, Prefix(0));
        let (_, gold) = strikes_with(crate::item::GOLD_PICKAXE, STONE, Prefix(0));
        assert!(gold < copper, "gold {gold} copper {copper}");
    }

    #[test]
    fn quick_prefix_mines_faster() {
        let none = strikes_to_break(STONE, Prefix(0)).1;
        let quick = strikes_to_break(STONE, Prefix(42)).1;
        assert!(quick < none, "{quick} < {none}");
    }

    #[test]
    fn out_of_reach_not_mined() {
        let mut w = TW::default();
        w.0.insert([0, 1, -8], STONE);
        let mut inv = inv_with(COPPER_PICKAXE, 1, Prefix(0));
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &held(), 200);
        assert_eq!(w.block(0, 1, -8), STONE);
        assert!(
            !ev.iter()
                .any(|e| matches!(e, UseEvent::TileHit { .. } | UseEvent::TileBroken { .. }))
        );
    }

    #[test]
    fn place_refused_inside_player() {
        let mut w = TW::default();
        w.0.insert([0, -1, 0], STONE);
        let mut inv = inv_with(DIRT_BLOCK, 5, Prefix(0));
        let i = PlayerInput {
            pitch: -std::f32::consts::FRAC_PI_2,
            ..held()
        };
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &i, 60);
        assert!(!ev.iter().any(|e| matches!(e, UseEvent::Placed { .. })));
        assert_eq!(w.block(0, 0, 0), AIR);
        assert_eq!(inv.slots[0].unwrap().count, 5);
    }

    #[test]
    fn placing_consumes_stack_to_empty() {
        let mut w = TW::default();
        w.0.insert([0, 1, -3], STONE);
        let mut inv = inv_with(DIRT_BLOCK, 2, Prefix(0));
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &held(), 100);
        let placed = ev
            .iter()
            .filter(|e| matches!(e, UseEvent::Placed { .. }))
            .count();
        assert_eq!(placed, 2);
        assert!(inv.slots[0].is_none());
        assert_eq!(w.block(0, 1, -2), DIRT);
        assert_eq!(w.block(0, 1, -1), DIRT);
    }

    #[test]
    fn workbench_item_places_tile_18() {
        let mut w = TW::default();
        w.0.insert([0, 1, -3], STONE);
        let mut inv = inv_with(WORKBENCH_ITEM, 1, Prefix(0));
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &held(), 100);
        let placed = ev
            .iter()
            .filter(|e| matches!(e, UseEvent::Placed { .. }))
            .count();
        assert_eq!(placed, 1);
        assert_eq!(w.block(0, 1, -2), crate::block::WORKBENCH);
    }

    #[test]
    fn furnace_item_places_tile_17() {
        let mut w = TW::default();
        w.0.insert([0, 1, -3], STONE);
        let mut inv = inv_with(crate::item::FURNACE_ITEM, 1, Prefix(0));
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &held(), 100);
        assert_eq!(
            ev.iter()
                .filter(|e| matches!(e, UseEvent::Placed { .. }))
                .count(),
            1
        );
        assert_eq!(w.block(0, 1, -2), crate::block::FURNACE);
    }

    #[test]
    fn torch_needs_support() {
        let mut w = TW::default();
        w.0.insert([0, 1, -3], STONE);
        let mut inv = inv_with(crate::item::TORCH, 10, Prefix(0));
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &held(), 100);
        let placed = ev
            .iter()
            .filter(|e| matches!(e, UseEvent::Placed { .. }))
            .count();
        // The first torch hangs on the stone; the second would float next to a torch.
        assert_eq!(placed, 1);
        assert_eq!(w.block(0, 1, -2), crate::block::TORCH);
        assert_eq!(w.block(0, 1, -1), AIR);
        assert_eq!(inv.slots[0].unwrap().count, 9);
    }

    #[test]
    fn broken_block_does_not_touch_inventory() {
        let mut w = TW::default();
        w.0.insert([0, 1, -2], DIRT);
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack {
            id: COPPER_PICKAXE,
            count: 1,
            prefix: Prefix(0),
        });
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &held(), 60);
        assert_eq!(w.block(0, 1, -2), AIR);
        // Exactly one break event and one drop.
        let broken: Vec<_> = ev
            .iter()
            .filter_map(|e| match e {
                UseEvent::TileBroken { pos, tile } => Some((*pos, *tile)),
                _ => None,
            })
            .collect();
        assert_eq!(broken, vec![([0, 1, -2], DIRT)]);
        assert!(inv.slots.iter().skip(1).all(Option::is_none));
    }

    #[test]
    fn tile_break_emits_item_drop_event() {
        let mut w = TW::default();
        w.0.insert([0, 1, -2], DIRT);
        let mut inv = inv_with(COPPER_PICKAXE, 1, Prefix(0));
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &held(), 60);
        let drops: Vec<_> = ev
            .iter()
            .filter_map(|e| match e {
                UseEvent::ItemDrop { stack, pos } => Some((stack.id, stack.count, *pos)),
                _ => None,
            })
            .collect();
        assert_eq!(drops, vec![(DIRT_BLOCK, 1, [0.5, 1.5, -1.5])]);
    }

    #[test]
    fn unloaded_chunks_are_not_targets() {
        // A pickaxe or sword aimed into a missing chunk must not damage,
        // break, message or drop anything, even though a physics query calls
        // unloaded space SOLID_UNLOADED.
        for item in [COPPER_PICKAXE, COPPER_SHORTSWORD] {
            let mut w = Missing::new(true);
            let mut inv = inv_with(item, 1, Prefix(0));
            let mut st = UseState::default();
            let ev = run(&mut w, &mut inv, &mut st, &held(), 200);
            let targeted: Vec<_> = ev
                .iter()
                .filter(|e| !matches!(e, UseEvent::Swing { .. }))
                .collect();
            assert!(targeted.is_empty(), "{item:?} emitted {targeted:?}");
            assert!(st.tile_damage.is_empty());
            assert_eq!(inv.slots[0].unwrap().count, 1);
            assert!(inv.slots.iter().skip(1).all(Option::is_none), "no drop");
        }
    }

    #[test]
    fn placing_into_unloaded_chunk_consumes_nothing() {
        // Chunk (0,0,0) is loaded; z=31 is its last column, so the placement
        // cell z=32 is in the missing chunk (0,0,1).
        let mut w = Missing::new(false);
        w.blocks.insert([0, 1, 31], STONE);
        let mut inv = inv_with(DIRT_BLOCK, 5, Prefix(0));
        let i = PlayerInput {
            yaw: std::f32::consts::PI,
            ..held()
        };
        let mut st = UseState::default();
        let ev: Vec<_> = (0..60)
            .flat_map(|_| {
                tick_use(
                    &mut w,
                    &body_at([0.5, 0.0, 30.5]),
                    &mut inv,
                    &mut st,
                    &i,
                    &mut Rng::new(7),
                )
            })
            .collect();
        let targeted: Vec<_> = ev
            .iter()
            .filter(|e| !matches!(e, UseEvent::Swing { .. }))
            .collect();
        assert!(targeted.is_empty(), "emitted {targeted:?}");
        assert_eq!(inv.slots[0].unwrap().count, 5);
        assert!(!w.blocks.contains_key(&[0, 1, 32]));
    }

    #[test]
    fn slot_key_latches_until_swing_ends() {
        let mut w = TW::default();
        let mut inv = inv_with(COPPER_PICKAXE, 1, Prefix(0));
        inv.slots[2] = Some(ItemStack {
            id: COPPER_PICKAXE,
            count: 1,
            prefix: Prefix(0),
        });
        let mut st = UseState::default();
        run(&mut w, &mut inv, &mut st, &held(), 5);
        assert!(st.anim > 0, "a swing is in progress");
        let press = PlayerInput { slot: 2, ..held() };
        tick_use(&mut w, &body(), &mut inv, &mut st, &press, &mut Rng::new(7));
        assert_eq!(inv.selected, 0, "the swing keeps its item");
        // The request applies as soon as the animation reaches 0.
        for _ in 0..100 {
            tick_use(&mut w, &body(), &mut inv, &mut st, &press, &mut Rng::new(7));
            if inv.selected == 2 {
                break;
            }
        }
        assert_eq!(inv.selected, 2);
    }

    fn wood_dropped(ev: &[UseEvent]) -> u32 {
        ev.iter()
            .map(|e| match e {
                UseEvent::ItemDrop { stack, .. } if stack.id == WOOD => stack.count,
                _ => 0,
            })
            .sum()
    }
    /// A trunk of `h` blocks from y = 0 at (0, z = -2), in front of the player.
    fn tree_world(h: i32) -> TW {
        let mut w = TW::default();
        for y in 0..h {
            w.0.insert([0, y, -2], TRUNK);
        }
        w
    }

    #[test]
    fn axe_breaks_tree_in_terraria_hits() {
        // gameplay spec: copper axe 7 * 1.2 = 8 per hit, break at >= 100: 13 hits.
        assert_eq!(strikes_with(COPPER_AXE, TRUNK, Prefix(0)).0, 13);
    }

    #[test]
    fn pickaxe_does_not_chop_tree() {
        let mut w = tree_world(1);
        let mut inv = inv_with(COPPER_PICKAXE, 1, Prefix(0));
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &held(), 120);
        assert_eq!(w.block(0, 0, -2), TRUNK);
        assert!(ev.iter().any(|e| matches!(e, UseEvent::Swing { .. })));
        assert!(
            !ev.iter()
                .any(|e| matches!(e, UseEvent::TileHit { .. } | UseEvent::TileBroken { .. }))
        );
    }

    #[test]
    fn axe_does_not_mine_stone() {
        let mut w = TW::default();
        w.0.insert([0, 1, -2], STONE);
        let mut inv = inv_with(COPPER_AXE, 1, Prefix(0));
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &held(), 120);
        assert_eq!(w.block(0, 1, -2), STONE);
        assert!(ev.iter().any(|e| matches!(e, UseEvent::Swing { .. })));
    }

    #[test]
    fn shadow_orb_only_damaged_by_hammer() {
        let at = [0, 1, -2];
        let mut w = TW::default();
        w.0.insert(at, SHADOW_ORB);
        let mut st = UseState::default();

        // Pickaxe does 0 damage to shadow orb
        let mut pick_inv = inv_with(COPPER_PICKAXE, 1, Prefix(0));
        let ev = run(&mut w, &mut pick_inv, &mut st, &held(), 100);
        assert_eq!(w.block(at[0], at[1], at[2]), SHADOW_ORB);
        assert!(
            !ev.iter()
                .any(|e| matches!(e, UseEvent::TileHit { .. } | UseEvent::TileBroken { .. }))
        );

        // Axe does 0 damage to shadow orb
        let mut axe_inv = inv_with(COPPER_AXE, 1, Prefix(0));
        let ev = run(&mut w, &mut axe_inv, &mut st, &held(), 100);
        assert_eq!(w.block(at[0], at[1], at[2]), SHADOW_ORB);
        assert!(
            !ev.iter()
                .any(|e| matches!(e, UseEvent::TileHit { .. } | UseEvent::TileBroken { .. }))
        );
    }

    #[test]
    fn wooden_hammer_breaks_shadow_orb() {
        let at = [0, 1, -2];
        let mut w = TW::default();
        w.0.insert(at, SHADOW_ORB);
        let mut hammer_inv = inv_with(WOODEN_HAMMER, 1, Prefix(0));
        let mut st = UseState::default();

        // Wooden Hammer has hammer power 25. Takes 4 hits (25 * 4 = 100) to break.
        let ev = run(&mut w, &mut hammer_inv, &mut st, &held(), 200);
        assert_eq!(w.block(at[0], at[1], at[2]), AIR);
        assert!(
            ev.iter()
                .any(|e| matches!(e, UseEvent::TileBroken { pos, tile: SHADOW_ORB } if *pos == at))
        );
        assert!(
            ev.iter()
                .any(|e| matches!(e, UseEvent::ShadowOrbBroken { pos } if *pos == at))
        );
    }

    #[test]
    fn tree_chopped_from_middle_leaves_stump() {
        let mut w = TestWorld::new();
        // 5-block trunk: y = 1..=5
        for y in 1..=5 {
            w.set_block(0, y, 0, TRUNK);
        }
        let mut ev = Vec::new();
        let mut rng = Rng::new(42);
        // Chop at y = 3
        chop_tree(&mut w, [0, 3, 0], 10, &mut rng, &mut ev);

        // Stumps below y = 3 remain
        assert_eq!(w.block(0, 1, 0), TRUNK);
        assert_eq!(w.block(0, 2, 0), TRUNK);
        // y = 3, 4, 5 were cleared
        assert_eq!(w.block(0, 3, 0), AIR);
        assert_eq!(w.block(0, 4, 0), AIR);
        assert_eq!(w.block(0, 5, 0), AIR);
        // At least 3 wood drops produced
        assert!(
            ev.iter()
                .filter(|e| matches!(e, UseEvent::TileBroken { tile: TRUNK, .. }))
                .count()
                == 3
        );
    }

    #[test]
    fn tree_chopped_from_base_removes_whole_tree() {
        let mut w = TestWorld::new();
        for y in 1..=4 {
            w.set_block(0, y, 0, TRUNK);
        }
        let mut ev = Vec::new();
        let mut rng = Rng::new(42);
        chop_tree(&mut w, [0, 1, 0], 10, &mut rng, &mut ev);

        for y in 1..=4 {
            assert_eq!(w.block(0, y, 0), AIR);
        }
    }

    #[test]
    fn chop_emits_wood_drop_events_not_inventory() {
        for seed in 0..50 {
            let mut w = tree_world(5);
            let mut ev = Vec::new();
            chop_tree(&mut w, [0, 0, -2], 7, &mut Rng::new(seed), &mut ev);
            let n = wood_dropped(&ev);
            assert!((5..=10).contains(&n), "seed {seed}: {n}");
        }
    }

    #[test]
    fn axe_chops_tree_through_tick_use() {
        let mut w = tree_world(5);
        let mut inv = inv_with(COPPER_AXE, 1, Prefix(0));
        let input = PlayerInput {
            pitch: -0.5,
            ..held()
        };
        let ev = run(&mut w, &mut inv, &mut UseState::default(), &input, 400);
        assert!((0..5).all(|y| w.block(0, y, -2) == AIR));
        assert!(wood_dropped(&ev) >= 5);
        assert!(inv.slots.iter().skip(1).all(Option::is_none));
    }

    #[test]
    fn bonus_wood_roll_matches_spec() {
        // gameplay spec: P = 8/35 + (27/35)(1/3) = 17/35 = 48.6%.
        let mut rng = Rng::new(99);
        let n = (0..10_000).filter(|_| bonus_wood_roll(&mut rng, 7)).count();
        assert!((4560..=5160).contains(&n), "{n}");
    }

    #[test]
    fn crystal_item_consumed_on_success() {
        let mut w = TW::default();
        let mut inv = inv_with(LIFE_CRYSTAL_ITEM, 2, Prefix(0));
        let mut st = UseState::default();
        let ev = tick_use(
            &mut w,
            &body(),
            &mut inv,
            &mut st,
            &held(),
            &mut Rng::new(1),
        );
        assert!(ev.contains(&UseEvent::Consume {
            item: LIFE_CRYSTAL_ITEM
        }));
        let mut v = Vitals::new();
        assert!(apply_consumable(LIFE_CRYSTAL_ITEM, &mut v, &mut inv));
        assert_eq!((v.life_max, inv.slots[0].unwrap().count), (120, 1));
    }

    #[test]
    fn crystal_use_stops_at_400_and_keeps_item() {
        let mut inv = inv_with(LIFE_CRYSTAL_ITEM, 1, Prefix(0));
        let mut v = Vitals::new();
        v.life_max = crate::player::vitals::LIFE_CAP;
        assert!(!apply_consumable(LIFE_CRYSTAL_ITEM, &mut v, &mut inv));
        assert_eq!(inv.slots[0].unwrap().count, 1);
        // Using something other than the selected stack does nothing.
        assert!(!apply_consumable(
            COPPER_PICKAXE,
            &mut Vitals::new(),
            &mut inv
        ));
    }

    #[test]
    fn mining_crystal_drops_item_29() {
        let at = [0, 1, -2];
        let mut w = TW::default();
        w.0.insert(at, crate::block::LIFE_CRYSTAL);
        let mut inv = inv_with(COPPER_PICKAXE, 1, Prefix(0));
        let mut st = UseState::default();
        let mut drops = Vec::new();
        for _ in 0..400 {
            for e in tick_use(
                &mut w,
                &body(),
                &mut inv,
                &mut st,
                &held(),
                &mut Rng::new(7),
            ) {
                if let UseEvent::ItemDrop { stack, .. } = e {
                    drops.push(stack.id);
                }
            }
        }
        assert_eq!(w.block(at[0], at[1], at[2]), AIR);
        assert_eq!(drops, [LIFE_CRYSTAL_ITEM]);
    }

    #[test]
    fn empty_bucket_picks_up_full_water_cell() {
        let at = [0, 1, -2];
        let mut w = TW::default();
        w.set_liquid(at[0], at[1], at[2], Some(Liquid::water(255)));
        let mut inv = inv_with(EMPTY_BUCKET, 1, Prefix(0));
        let mut st = UseState::default();
        let _ = tick_use(
            &mut w,
            &body(),
            &mut inv,
            &mut st,
            &held(),
            &mut Rng::new(7),
        );
        assert_eq!(w.liquid(at[0], at[1], at[2]), None);
        assert_eq!(inv.slots[0].as_ref().map(|s| s.id), Some(WATER_BUCKET));
        assert!(w.3.contains(&at), "neighbour waking emitted");
    }

    #[test]
    fn empty_bucket_ignores_thin_water() {
        let at = [0, 1, -2];
        let mut w = TW::default();
        w.set_liquid(at[0], at[1], at[2], Some(Liquid::water(100)));
        let mut inv = inv_with(EMPTY_BUCKET, 1, Prefix(0));
        let mut st = UseState::default();
        let _ = tick_use(
            &mut w,
            &body(),
            &mut inv,
            &mut st,
            &held(),
            &mut Rng::new(7),
        );
        assert_eq!(w.liquid(at[0], at[1], at[2]), Some(Liquid::water(100)));
        assert_eq!(inv.slots[0].as_ref().map(|s| s.id), Some(EMPTY_BUCKET));
    }

    #[test]
    fn water_bucket_pours_one_full_block() {
        let target_wall = [0, 1, -2];
        let mut w = TW::default();
        w.0.insert(target_wall, STONE);
        let mut inv = inv_with(WATER_BUCKET, 1, Prefix(0));
        let mut st = UseState::default();
        let _ = tick_use(
            &mut w,
            &body(),
            &mut inv,
            &mut st,
            &held(),
            &mut Rng::new(7),
        );
        assert_eq!(w.liquid(0, 1, -1), Some(Liquid::water(255)));
        assert_eq!(inv.slots[0].as_ref().map(|s| s.id), Some(EMPTY_BUCKET));
        assert!(w.3.contains(&[0, 1, -1]), "cell waking emitted");
    }

    #[test]
    fn lava_bucket_pours_and_returns_empty_bucket() {
        let target_wall = [0, 1, -2];
        let mut w = TW::default();
        w.0.insert(target_wall, STONE);
        let mut inv = inv_with(LAVA_BUCKET, 1, Prefix(0));
        let mut st = UseState::default();
        let _ = tick_use(
            &mut w,
            &body(),
            &mut inv,
            &mut st,
            &held(),
            &mut Rng::new(7),
        );
        assert_eq!(w.liquid(0, 1, -1), Some(Liquid::lava(255)));
        assert_eq!(inv.slots[0].as_ref().map(|s| s.id), Some(EMPTY_BUCKET));
        assert!(w.3.contains(&[0, 1, -1]), "cell waking emitted");
    }

    #[test]
    fn bucket_use_out_of_reach_refused() {
        let far = [0, 1, -6];
        let mut w = TW::default();
        w.set_liquid(far[0], far[1], far[2], Some(Liquid::water(255)));
        let mut inv = inv_with(EMPTY_BUCKET, 1, Prefix(0));
        let mut st = UseState::default();
        let _ = tick_use(
            &mut w,
            &body(),
            &mut inv,
            &mut st,
            &held(),
            &mut Rng::new(7),
        );
        assert_eq!(w.liquid(far[0], far[1], far[2]), Some(Liquid::water(255)));
        assert_eq!(inv.slots[0].as_ref().map(|s| s.id), Some(EMPTY_BUCKET));

        w.0.insert(far, STONE);
        let mut inv_water = inv_with(WATER_BUCKET, 1, Prefix(0));
        let mut st2 = UseState::default();
        let _ = tick_use(
            &mut w,
            &body(),
            &mut inv_water,
            &mut st2,
            &held(),
            &mut Rng::new(7),
        );
        assert_eq!(w.liquid(0, 1, -5), None);
        assert_eq!(
            inv_water.slots[0].as_ref().map(|s| s.id),
            Some(WATER_BUCKET)
        );
    }

    #[test]
    fn obsidian_mines_with_iron_pickaxe_drops_item() {
        let at = [0, 1, -2];
        let mut w = TW::default();
        w.0.insert(at, OBSIDIAN);

        // Iron/Silver pickaxes cannot mine obsidian (pick 40/45 < 55)
        for pick in [IRON_PICKAXE, SILVER_PICKAXE] {
            let (strikes, ticks) = strikes_with(pick, OBSIDIAN, Prefix(0));
            assert_eq!((strikes, ticks), (0, 1000), "{pick:?} cannot mine obsidian");
        }

        let mut inv = inv_with(GOLD_PICKAXE, 1, Prefix(0));
        let mut st = UseState::default();
        let mut dropped = None;
        for _ in 0..1000 {
            for e in tick_use(
                &mut w,
                &body(),
                &mut inv,
                &mut st,
                &held(),
                &mut Rng::new(7),
            ) {
                if let UseEvent::ItemDrop { stack, .. } = e {
                    dropped = Some(stack.id);
                }
            }
            if w.block(at[0], at[1], at[2]) == AIR {
                break;
            }
        }
        assert_eq!(w.block(at[0], at[1], at[2]), AIR);
        assert_eq!(dropped, Some(OBSIDIAN_ITEM));
    }

    #[test]
    fn crystal_raises_max_by_20_to_cap_200() {
        let mut v = Vitals::new();
        let mut inv = inv_with(MANA_CRYSTAL, 1, Prefix(0));
        assert!(apply_consumable(MANA_CRYSTAL, &mut v, &mut inv));
        assert_eq!((v.mana_max, v.mana), (40, 40));

        for _ in 0..8 {
            inv.slots[0] = Some(ItemStack {
                id: MANA_CRYSTAL,
                count: 1,
                prefix: Prefix(0),
            });
            assert!(apply_consumable(MANA_CRYSTAL, &mut v, &mut inv));
        }
        // 20 + 9 * 20 = 200: the cap.
        assert_eq!((v.mana_max, v.mana), (crate::mana::MANA_CAP, 200));
    }

    #[test]
    fn crystal_refused_at_cap() {
        let mut v = Vitals::new();
        v.mana_max = crate::mana::MANA_CAP;
        v.mana = crate::mana::MANA_CAP;
        let mut inv = inv_with(MANA_CRYSTAL, 1, Prefix(0));
        assert!(!apply_consumable(MANA_CRYSTAL, &mut v, &mut inv));
        assert_eq!((v.mana_max, v.mana), (200, 200));
        assert_eq!(inv.slots[0].unwrap().count, 1, "item kept at the cap");
    }

    #[test]
    fn wand_use_emits_cast_event() {
        let mut w = TW::default();
        let mut inv = inv_with(WAND_OF_SPARKING, 1, Prefix(0));
        let mut st = UseState::default();
        let ev = tick_use(
            &mut w,
            &body(),
            &mut inv,
            &mut st,
            &held(),
            &mut Rng::new(1),
        );
        assert!(ev.iter().any(|e| matches!(
            e,
            UseEvent::Cast { wand, .. } if *wand == WAND_OF_SPARKING
        )));
    }

    #[test]
    fn ammo_count_matches_consumed_stack() {
        use crate::inventory::AMMO0;
        let mut w = TW::default();
        let mut inv = inv_with(WOODEN_BOW, 1, Prefix(0));
        inv.slots[3] = Some(ItemStack {
            id: FLAMING_ARROW,
            count: 50,
            prefix: Prefix(0),
        });
        inv.slots[AMMO0] = Some(ItemStack {
            id: WOODEN_ARROW,
            count: 5,
            prefix: Prefix(0),
        });

        assert_eq!(inv.ammo_count(), 5);

        run(&mut w, &mut inv, &mut UseState::default(), &held(), 20);
        assert_eq!(inv.slots[AMMO0].unwrap().count, 4);
        assert_eq!(inv.ammo_count(), 4);

        inv.slots[AMMO0] = None;
        assert_eq!(inv.ammo_count(), 50);

        run(&mut w, &mut inv, &mut UseState::default(), &held(), 20);
        assert_eq!(inv.slots[3].unwrap().count, 49);
        assert_eq!(inv.ammo_count(), 49);

        inv.slots[3] = None;
        assert_eq!(inv.ammo_count(), 0);
    }

    #[test]
    fn try_unlock_chest_requires_golden_key() {
        let mut inv = Inventory::empty();
        // Unlocked chest succeeds without key
        assert_eq!(try_unlock_chest(&mut inv, false), Ok(false));

        // Locked chest fails without key
        assert_eq!(
            try_unlock_chest(&mut inv, true),
            Err(ChestLockError::RequiresGoldenKey)
        );

        // Put a golden key in inventory
        inv.slots[0] = Some(ItemStack {
            id: GOLDEN_KEY,
            count: 2,
            prefix: Prefix(0),
        });

        // Locked chest succeeds and consumes 1 key
        assert_eq!(try_unlock_chest(&mut inv, true), Ok(true));
        assert_eq!(inv.count_of(GOLDEN_KEY), 1);

        // Consume the second key
        assert_eq!(try_unlock_chest(&mut inv, true), Ok(true));
        assert_eq!(inv.count_of(GOLDEN_KEY), 0);

        // Now without key it fails again
        assert_eq!(
            try_unlock_chest(&mut inv, true),
            Err(ChestLockError::RequiresGoldenKey)
        );
    }

    #[test]
    fn molten_set_shortens_melee_and_tool_use_time() {
        let mut molten = body();
        molten.has_molten_set = true;
        let bare = body();

        // Copper Broadsword swing animation 21: round(21 * 0.93) = 20, less the
        // tick already spent after itemCheck.
        for (b, want) in [(&molten, 19), (&bare, 20)] {
            let mut w = TW::new();
            let mut inv = inv_with(COPPER_BROADSWORD, 1, Prefix(0));
            let mut st = UseState::default();
            tick_use(&mut w, b, &mut inv, &mut st, &held(), &mut Rng::new(7));
            assert_eq!(st.anim, want, "melee animation with molten = {b:?}");
        }

        // Copper Pickaxe use 15: round(15 * 0.93) = 14, less the same tick.
        for (b, want) in [(&molten, 13), (&bare, 14)] {
            let mut w = TW::new();
            w.set_block(0, 1, -2, STONE);
            let mut inv = inv_with(COPPER_PICKAXE, 1, Prefix(0));
            let mut st = UseState::default();
            tick_use(&mut w, b, &mut inv, &mut st, &held(), &mut Rng::new(7));
            assert_eq!(st.time, want, "tool cooldown with molten = {b:?}");
        }
    }

    #[test]
    fn no_fail_tiles_break_with_any_pickaxe() {
        // `crate::item` also exports a TORCH *item*; this test means the tile.
        use crate::block::TORCH;
        // Main.tileNoFail: a base of 100, so even a bare hand (pick 0) breaks
        // them on the first strike.
        for tile in [TORCH, POT, STALACTITE, MOSS, SMALL_ROCK, SHELF_MUSHROOM] {
            assert!(
                pick_damage(tile, 0) >= 100,
                "{tile:?} should be no-fail, got {}",
                pick_damage(tile, 0)
            );
        }

        // A normal tile starts at 0. STONE is BlockId(1), outside the
        // dirt-like double-add set (147/0/53/59), so the pick power is added
        // exactly once; DIRT (0) is inside it and gets it twice.
        assert!(pick_damage(STONE, 0) < 100);
        assert_eq!(pick_damage(STONE, 35), 35);
        assert_eq!(pick_damage(DIRT, 35), 70);
    }

    #[test]
    fn molten_set_does_not_speed_up_guns_or_magic() {
        let mut molten = body();
        molten.has_molten_set = true;
        let bare = body();

        // Same setup as `molten_set_shortens_melee_and_tool_use_time`, but for
        // a gun (with bullets, so the shot — and its use time — registers) and
        // the magic wand: the started animation and cooldown must be the ones
        // the item gets without the set.
        for id in [MUSKET, WAND_OF_SPARKING] {
            let mut got = [(0, 0); 2];
            for (i, b) in [&molten, &bare].into_iter().enumerate() {
                let mut w = TW::new();
                let mut inv = inv_with(id, 1, Prefix(0));
                inv.slots[AMMO0] = Some(ItemStack {
                    id: MUSKET_BALL,
                    count: 10,
                    prefix: Prefix(0),
                });
                let mut st = UseState::default();
                tick_use(&mut w, b, &mut inv, &mut st, &held(), &mut Rng::new(7));
                got[i] = (st.anim, st.time);
            }
            assert!(got[0].0 > 0, "{id:?} should start a swing");
            assert!(
                got[0].1 > 0,
                "{id:?} should start its cooldown, anim = {}",
                got[0].0
            );
            assert_eq!(got[0], got[1], "{id:?} must ignore the molten set");
        }
    }

    #[test]
    fn m11_clockwork_assault_rifle_fires_three_round_burst_for_one_bullet() {
        use crate::item::CLOCKWORK_ASSAULT_RIFLE;
        let mut w = TW::new();
        let b = body();
        let mut inv = inv_with(CLOCKWORK_ASSAULT_RIFLE, 1, Prefix(0));
        inv.slots[AMMO0] = Some(ItemStack {
            id: MUSKET_BALL,
            count: 10,
            prefix: Prefix(0),
        });
        let mut st = UseState::default();
        let mut rng = Rng::new(7);
        let mut shots = 0;
        for tick in 0..12 {
            let input = PlayerInput {
                use_held: tick == 0,
                ..Default::default()
            };
            let evs = tick_use(&mut w, &b, &mut inv, &mut st, &input, &mut rng);
            shots += evs
                .iter()
                .filter(
                    |e| matches!(e, UseEvent::Shoot { bow, .. } if *bow == CLOCKWORK_ASSAULT_RIFLE),
                )
                .count();
        }
        assert_eq!(shots, 3, "three-round burst over 12 ticks");
        assert_eq!(
            inv.slots[AMMO0].unwrap().count,
            9,
            "only the first shot in the burst consumes ammo"
        );
    }

    #[test]
    fn single_click_places_exactly_one_block_wall_and_torch() {
        for (item, press_ticks) in [
            (DIRT_BLOCK, 1),
            (DIRT_BLOCK, 8),
            (WOOD_WALL, 1),
            (WOOD_WALL, 8),
            (crate::item::TORCH, 1),
            (crate::item::TORCH, 8),
        ] {
            let mut w = TW::default();
            w.0.insert([0, 1, -3], STONE);
            let mut inv = inv_with(item, 5, Prefix(0));
            let mut st = UseState::default();
            let mut rng = Rng::new(7);
            let mut placed_events = 0;
            for tick in 0..40 {
                let input = PlayerInput {
                    use_held: tick < press_ticks,
                    ..Default::default()
                };
                for e in tick_use(&mut w, &body(), &mut inv, &mut st, &input, &mut rng) {
                    if matches!(e, UseEvent::Placed { .. } | UseEvent::WallPlaced { .. }) {
                        placed_events += 1;
                    }
                }
            }
            assert_eq!(
                placed_events, 1,
                "{item:?} with {press_ticks}-tick press must place 1, placed {placed_events}"
            );
            assert_eq!(
                inv.slots[0].unwrap().count,
                4,
                "{item:?} must consume 1 item"
            );
        }
    }

    #[test]
    fn minishark_saves_about_one_third_ammo() {
        let mut w = TW::default();
        let b = body();
        let mut inv = inv_with(MINISHARK, 1, Prefix(0));
        inv.slots[AMMO0] = Some(ItemStack {
            id: MUSKET_BALL,
            count: 9999,
            prefix: Prefix(0),
        });
        let mut st = UseState::default();
        let mut rng = Rng::new(123);
        let mut shots = 0;
        for _ in 0..2400 {
            for e in tick_use(&mut w, &b, &mut inv, &mut st, &held(), &mut rng) {
                if matches!(e, UseEvent::Shoot { .. }) {
                    shots += 1;
                }
            }
        }
        let consumed = 9999 - inv.slots[AMMO0].unwrap().count;
        assert!(shots >= 250, "minishark fired {shots} shots");
        let saved_ratio = 1.0 - (consumed as f64 / shots as f64);
        assert!(
            (0.25..=0.42).contains(&saved_ratio),
            "expected ~33% ammo save, got {saved_ratio} ({consumed}/{shots})"
        );
    }

    #[test]
    fn out_of_mana_magic_does_not_swing() {
        let mut w = TW::default();
        let b = body();
        let mut inv = inv_with(WAND_OF_SPARKING, 1, Prefix(0));
        let mut st = UseState::default();
        let mut rng = Rng::new(7);
        let ev = tick_use_with_mana(&mut w, &b, &mut inv, &mut st, &held(), &mut rng, Some(1));
        assert!(ev.is_empty(), "insufficient mana must not swing or cast");
        assert_eq!(st.anim, 0);

        let mut released = held();
        released.use_held = false;
        tick_use_with_mana(&mut w, &b, &mut inv, &mut st, &released, &mut rng, Some(2));

        let ev_ok = tick_use_with_mana(&mut w, &b, &mut inv, &mut st, &held(), &mut rng, Some(2));
        assert!(ev_ok.iter().any(|e| matches!(e, UseEvent::Swing { .. })));
        assert!(ev_ok.iter().any(|e| matches!(e, UseEvent::Cast { .. })));
    }

    #[test]
    fn non_auto_reuse_requires_fresh_press() {
        let mut w = TW::default();
        let mut inv = inv_with(PHOENIX_BLASTER, 1, Prefix(0));
        inv.slots[AMMO0] = Some(ItemStack {
            id: MUSKET_BALL,
            count: 10,
            prefix: Prefix(0),
        });
        let mut st = UseState::default();
        let ev = run(&mut w, &mut inv, &mut st, &held(), 60);
        let shots = ev
            .iter()
            .filter(|e| matches!(e, UseEvent::Shoot { .. }))
            .count();
        assert_eq!(shots, 1, "Phoenix Blaster is non-auto and must fire once");
        assert_eq!(inv.slots[AMMO0].unwrap().count, 9);
    }

    #[test]
    fn pwnhammer_breaks_demon_altar_but_weaker_hammers_cannot() {
        let at = [0, 1, -2];
        let mut w = TW::default();
        w.0.insert(at, DEMON_ALTAR);
        let mut weak = inv_with(THE_BREAKER, 1, Prefix(0));
        let mut st = UseState::default();
        let ev_weak = run(&mut w, &mut weak, &mut st, &held(), 120);
        assert_eq!(w.block(at[0], at[1], at[2]), DEMON_ALTAR);
        assert!(
            !ev_weak
                .iter()
                .any(|e| matches!(e, UseEvent::TileHit { .. }))
        );

        let mut pwn = inv_with(PWNHAMMER, 1, Prefix(0));
        let ev_pwn = run(&mut w, &mut pwn, &mut st, &held(), 120);
        assert_eq!(w.block(at[0], at[1], at[2]), AIR);
        assert!(ev_pwn.iter().any(|e| matches!(
            e,
            UseEvent::TileBroken {
                tile: DEMON_ALTAR,
                ..
            }
        )));
    }
}
