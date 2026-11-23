//! Player physics: Terraria movement constants scaled to blocks, 60 ticks/s.

pub mod collide;
pub mod consts;
pub mod vitals;

use crate::chunk::BlockQuery;
use crate::enemy::EnemyKind;
use crate::inventory::{Inventory, ItemStack};
use crate::item::{
    AccessoryPrefixBonus, ArmorSlot, ItemClass, ItemId, Prefix, accessory_prefix_bonus, item_data,
};
use crate::liquid_cell::LiquidKind;
use crate::projectile::ProjKind;
use crate::world_bounds::WorldBounds;
pub use vitals::{Vitals, fall_damage};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DeathCause {
    Contact(EnemyKind),
    Projectile(ProjKind),
    Fall,
    Lava,
    Drown,
    Suffocation,
    Fire,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub body: PlayerBody,
    pub vitals: Vitals,
    pub dead: bool,
    /// Ticks left until respawn while dead.
    pub respawn_ticks: i32,
    pub spawn: [f32; 3],
    /// Hurt immunity ticks left (combat::hurt_player).
    pub i_frames: i32,
    /// Worn armor by `armor_index` (head, body, legs). Session-only: the server keeps no saves.
    pub armor: [Option<ItemId>; 3],
    pub accessories: [Option<ItemId>; 5],
    pub accessory_prefixes: [Prefix; 5],
    /// Vanity / social armor (head, body, legs). Grants 0 defense/stats.
    pub vanity_armor: [Option<ItemId>; 3],
    /// Vanity / social accessories (5 slots). Grants 0 defense/stats.
    pub vanity_accessories: [Option<ItemId>; 5],
    pub vanity_accessory_prefixes: [Prefix; 5],
    pub last_damage_cause: Option<DeathCause>,
    pub god_mode: bool,
}

/// Index into `Player.armor` for a slot.
pub fn armor_index(s: ArmorSlot) -> usize {
    match s {
        ArmorSlot::Head => 0,
        ArmorSlot::Body => 1,
        ArmorSlot::Legs => 2,
    }
}

/// Metal armor set defense bonuses (Copper +2, Iron +2, Silver +3, Gold +3).
pub fn armor_set_defense_bonus(armor: &[Option<ItemId>; 3]) -> i32 {
    use crate::item::*;
    match (
        armor[armor_index(ArmorSlot::Head)],
        armor[armor_index(ArmorSlot::Body)],
        armor[armor_index(ArmorSlot::Legs)],
    ) {
        (Some(COPPER_HELMET), Some(COPPER_CHAINMAIL), Some(COPPER_GREAVES)) => 2,
        (Some(IRON_HELMET), Some(IRON_CHAINMAIL), Some(IRON_GREAVES)) => 2,
        (Some(SILVER_HELMET), Some(SILVER_CHAINMAIL), Some(SILVER_GREAVES)) => 3,
        (Some(GOLD_HELMET), Some(GOLD_CHAINMAIL), Some(GOLD_GREAVES)) => 3,
        _ => 0,
    }
}

impl Player {
    /// Combined bonuses from prefixes on currently equipped accessories.
    pub fn accessory_prefix_totals(&self) -> AccessoryPrefixBonus {
        let mut total = AccessoryPrefixBonus::default();
        for (item, &prefix) in self.accessories.iter().zip(self.accessory_prefixes.iter()) {
            if item.is_some() {
                let b = accessory_prefix_bonus(prefix);
                total.defense += b.defense;
                total.mana += b.mana;
                total.crit += b.crit;
                total.damage_pct += b.damage_pct;
                total.move_speed_pct += b.move_speed_pct;
                total.melee_speed_pct += b.melee_speed_pct;
            }
        }
        total
    }

    /// `Player.statDefense`: the sum of `Item.defense` over worn pieces plus set bonuses.
    pub fn defense(&self) -> i32 {
        let armor_def: i32 = self
            .armor
            .iter()
            .flatten()
            .map(|i| item_data(*i).defense)
            .sum();
        let acc_def: i32 = self
            .accessories
            .iter()
            .flatten()
            .map(|i| item_data(*i).defense)
            .sum();
        armor_def
            + acc_def
            + armor_set_defense_bonus(&self.armor)
            + self.accessory_prefix_totals().defense
            + if self.vitals.ironskin_ticks > 0 { 8 } else { 0 }
    }

    pub fn effective_mana_max(&self) -> i32 {
        self.vitals.mana_max
            + if self.has_accessory(crate::item::BAND_OF_STARPOWER) {
                20
            } else {
                0
            }
            + self.accessory_prefix_totals().mana
    }

    pub fn max_mana(&self) -> i32 {
        self.effective_mana_max()
    }

    pub fn no_knockback(&self) -> bool {
        self.body.no_knockback
    }

    pub fn damage_knockback(&self, knockback: f32) -> f32 {
        if self.body.no_knockback {
            0.0
        } else {
            knockback
        }
    }

    pub fn apply_knockback(&mut self, knockback: f32, dir: [f32; 3]) {
        let eff = self.damage_knockback(knockback);
        self.body.vel[0] += dir[0] * eff;
        self.body.vel[1] += dir[1] * eff;
        self.body.vel[2] += dir[2] * eff;
    }

    pub fn has_accessory(&self, id: ItemId) -> bool {
        self.accessories.iter().flatten().any(|&item| item == id)
    }

    pub fn lava_max(&self) -> i32 {
        if self.has_accessory(crate::item::LAVA_CHARM) {
            consts::LAVA_CHARM_TICKS
        } else {
            0
        }
    }

    pub fn fire_walk(&self) -> bool {
        self.has_accessory(crate::item::OBSIDIAN_SKULL)
            || self.vitals.obsidian_skin_ticks > 0
            || self.has_molten_set()
    }

    /// Applies the On Fire! debuff unless the player is immune (full Molten set or Obsidian Skin).
    pub fn ignite(&mut self) {
        if !self.has_molten_set() && self.vitals.obsidian_skin_ticks == 0 {
            self.vitals.ignite();
        }
    }

    /// Full set bonus for Shadow Armor: Helmet (102), Scalemail (101), Greaves (100).
    pub fn has_shadow_set(&self) -> bool {
        self.armor[armor_index(ArmorSlot::Head)] == Some(crate::item::SHADOW_HELMET)
            && self.armor[armor_index(ArmorSlot::Body)] == Some(crate::item::SHADOW_SCALEMAIL)
            && self.armor[armor_index(ArmorSlot::Legs)] == Some(crate::item::SHADOW_GREAVES)
    }

    /// Full set bonus for Molten Armor: Helmet (231), Breastplate (232), Greaves (233).
    pub fn has_molten_set(&self) -> bool {
        self.armor[armor_index(ArmorSlot::Head)] == Some(crate::item::MOLTEN_HELMET)
            && self.armor[armor_index(ArmorSlot::Body)] == Some(crate::item::MOLTEN_BREASTPLATE)
            && self.armor[armor_index(ArmorSlot::Legs)] == Some(crate::item::MOLTEN_GREAVES)
    }

    /// Total melee damage multiplier (Molten Breastplate +7%, Molten full set +10% extra = +17%, Warrior Emblem +15%, accessory prefixes).
    pub fn melee_damage_mult(&self) -> f32 {
        let molten_chest =
            self.armor[armor_index(ArmorSlot::Body)] == Some(crate::item::MOLTEN_BREASTPLATE);
        1.0 + if molten_chest { 0.07 } else { 0.0 }
            + if self.has_molten_set() { 0.10 } else { 0.0 }
            + if self.has_accessory(crate::item::WARRIOR_EMBLEM) {
                0.15
            } else {
                0.0
            }
            + self.accessory_prefix_totals().damage_pct as f32 / 100.0
    }

    /// Total ranged damage multiplier (Ranger Emblem +15%, accessory prefixes).
    pub fn ranged_damage_mult(&self) -> f32 {
        1.0 + if self.has_accessory(crate::item::RANGER_EMBLEM) {
            0.15
        } else {
            0.0
        } + self.accessory_prefix_totals().damage_pct as f32 / 100.0
    }

    /// Total magic damage multiplier (Sorcerer Emblem +15%, accessory prefixes).
    pub fn magic_damage_mult(&self) -> f32 {
        1.0 + if self.has_accessory(crate::item::SORCERER_EMBLEM) {
            0.15
        } else {
            0.0
        } + self.accessory_prefix_totals().damage_pct as f32 / 100.0
    }

    /// Critical strike chance bonus from equipped accessory prefixes (`Precise`/`Lucky`).
    pub fn accessory_crit_bonus(&self) -> i32 {
        self.accessory_prefix_totals().crit
    }

    /// Melee critical chance bonus: Molten Helmet (+7%) + Shadow Armor pieces (+5% each) + accessories.
    pub fn melee_crit_bonus(&self) -> i32 {
        let molten_helmet =
            if self.armor[armor_index(ArmorSlot::Head)] == Some(crate::item::MOLTEN_HELMET) {
                7
            } else {
                0
            };
        let shadow_pieces = [
            (ArmorSlot::Head, crate::item::SHADOW_HELMET),
            (ArmorSlot::Body, crate::item::SHADOW_SCALEMAIL),
            (ArmorSlot::Legs, crate::item::SHADOW_GREAVES),
        ]
        .into_iter()
        .filter(|&(slot, id)| self.armor[armor_index(slot)] == Some(id))
        .count() as i32;
        molten_helmet + shadow_pieces * 5 + self.accessory_crit_bonus()
    }

    /// Multiplier on melee and tool use time from the greaves' 7% melee speed.
    pub fn melee_speed_mult(&self) -> f32 {
        self.body.molten_melee_use_time_mult(ItemClass::Melee)
    }

    pub fn sync_accessories(&mut self) {
        for i in 0..5 {
            if self.accessories[i].is_none() {
                self.accessory_prefixes[i] = Prefix(0);
            }
        }
        self.body.has_hermes_boots = self.has_accessory(crate::item::HERMES_BOOTS);
        self.body.has_cloud_in_a_bottle = self.has_accessory(crate::item::CLOUD_IN_A_BOTTLE);
        self.body.has_shiny_red_balloon = self.has_accessory(crate::item::SHINY_RED_BALLOON);
        self.body.has_shadow_set = self.has_shadow_set();
        self.body.has_molten_set =
            self.armor[armor_index(ArmorSlot::Legs)] == Some(crate::item::MOLTEN_GREAVES);
        self.body.no_knockback = self.has_accessory(crate::item::COBALT_SHIELD);
        let b = self.accessory_prefix_totals();
        self.body.move_speed_bonus = b.move_speed_pct as f32 / 100.0;
        self.body.melee_speed_bonus = b.melee_speed_pct as f32 / 100.0;
        self.vitals.mana = self.vitals.mana.min(self.effective_mana_max());
    }

    /// Equips the armor or accessory item in inventory slot `from`, swapping any worn piece back into
    /// that slot. False (nothing changed) for a dead player, a bad slot or non-equippable item.
    pub fn equip(&mut self, inv: &mut Inventory, from: usize) -> bool {
        if self.dead {
            return false;
        }
        let Some(st) = inv.slots.get(from).copied().flatten() else {
            return false;
        };
        let data = item_data(st.id);
        if let Some(slot) = data.armor_slot {
            let i = armor_index(slot);
            let old = self.armor[i].replace(st.id);
            inv.slots[from] = old.map(|id| ItemStack {
                id,
                count: 1,
                prefix: Prefix(0),
            });
            self.sync_accessories();
            true
        } else if data.class == crate::item::ItemClass::Accessory {
            if self.has_accessory(st.id) {
                return false;
            }
            let slot = self
                .accessories
                .iter()
                .position(|a| a.is_none())
                .unwrap_or(0);
            let old = self.accessories[slot].replace(st.id);
            let old_prefix = std::mem::replace(&mut self.accessory_prefixes[slot], st.prefix);
            inv.slots[from] = old.map(|id| ItemStack {
                id,
                count: 1,
                prefix: old_prefix,
            });
            self.sync_accessories();
            true
        } else {
            false
        }
    }

    /// Moves a worn piece back to the inventory. False when empty or the inventory is full.
    pub fn unequip(&mut self, inv: &mut Inventory, slot: ArmorSlot) -> bool {
        let i = armor_index(slot);
        let Some(id) = self.armor[i] else {
            return false;
        };
        let back = ItemStack {
            id,
            count: 1,
            prefix: Prefix(0),
        };
        if inv.add(back).is_some() {
            return false;
        }
        self.armor[i] = None;
        self.sync_accessories();
        true
    }

    /// Moves a worn accessory back to the inventory. False when empty or the inventory is full.
    pub fn unequip_accessory(&mut self, inv: &mut Inventory, slot: usize) -> bool {
        if slot >= 5 {
            return false;
        }
        let Some(id) = self.accessories[slot] else {
            return false;
        };
        let back = ItemStack {
            id,
            count: 1,
            prefix: self.accessory_prefixes[slot],
        };
        if inv.add(back).is_some() {
            return false;
        }
        self.accessories[slot] = None;
        self.accessory_prefixes[slot] = Prefix(0);
        self.sync_accessories();
        true
    }
}

impl Player {
    pub fn new(spawn: [f32; 3]) -> Self {
        Self {
            body: PlayerBody::new(spawn),
            vitals: Vitals::new(),
            dead: false,
            respawn_ticks: 0,
            spawn,
            i_frames: 0,
            armor: [None; 3],
            accessories: [None; 5],
            accessory_prefixes: [Prefix(0); 5],
            vanity_armor: [None; 3],
            vanity_accessories: [None; 5],
            vanity_accessory_prefixes: [Prefix(0); 5],
            last_damage_cause: None,
            god_mode: false,
        }
    }

    pub fn kill(&mut self, cause: DeathCause, ev: &mut Vec<PlayerEvent>) {
        self.dead = true;
        self.respawn_ticks = vitals::RESPAWN_TICKS;
        self.body.vel = [0.0; 3];
        self.last_damage_cause = Some(cause);
        ev.push(PlayerEvent::Died { cause });
    }

    pub fn respawn(&mut self, ev: &mut Vec<PlayerEvent>) {
        let old_life_max = self.vitals.life_max;
        let old_mana_max = self.vitals.mana_max;
        self.body = PlayerBody::new(self.spawn);
        self.vitals = Vitals::new();
        self.vitals.life_max = old_life_max;
        self.vitals.mana_max = old_mana_max;
        self.vitals.life = old_life_max;
        self.vitals.mana = self.effective_mana_max();
        self.dead = false;
        self.respawn_ticks = 0;
        self.i_frames = crate::combat::RESPAWN_IFRAMES;
        self.sync_accessories();
        ev.push(PlayerEvent::Respawned);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerEvent {
    Hurt { amount: i32 },
    Died { cause: DeathCause },
    Respawned,
}

/// One tick: physics (frozen while dead), fall damage, regen, respawn timer.
pub fn tick_player(q: &impl BlockQuery, p: &mut Player, input: &PlayerInput) -> Vec<PlayerEvent> {
    tick_player_bounded(q, p, input, None)
}

/// [`tick_player`] with optional world-edge bounds clamping.
pub fn tick_player_bounded(
    q: &impl BlockQuery,
    p: &mut Player,
    input: &PlayerInput,
    bounds: Option<&WorldBounds>,
) -> Vec<PlayerEvent> {
    let mut ev = Vec::new();
    if p.dead {
        // Player.Update: respawnTimer counts down, then the player respawns.
        p.respawn_ticks -= 1;
        if p.respawn_ticks <= 0 {
            p.respawn(&mut ev);
        }
        return ev;
    }
    p.sync_accessories();
    p.i_frames = (p.i_frames - 1).max(0);
    p.vitals.lava_cd = p.vitals.lava_cd.saturating_sub(1);
    p.vitals.tick_obsidian_skin();
    p.vitals.tick_buffs();
    p.body.swiftness = p.vitals.swiftness_ticks > 0;
    p.body.water_walking = p.vitals.water_walking_ticks > 0;
    p.vitals.lava_max = p.lava_max();
    let step = step_body_bounded(q, &mut p.body, input, bounds);
    if !p.god_mode {
        if let Some(blocks) = step.landed_fall_blocks {
            let raw_dmg = if p.has_accessory(crate::item::LUCKY_HORSESHOE) {
                0
            } else {
                fall_damage(blocks)
            };
            if raw_dmg > 0 {
                let dmg = crate::combat::damage_taken(raw_dmg, p.defense());
                p.i_frames = crate::combat::HURT_IFRAMES;
                ev.push(PlayerEvent::Hurt { amount: dmg });
                p.last_damage_cause = Some(DeathCause::Fall);
                if p.vitals.hurt(dmg) {
                    p.kill(DeathCause::Fall, &mut ev);
                    return ev;
                }
            }
        }
    }

    let head_l = {
        let x = p.body.pos[0].floor() as i32;
        let y = (p.body.pos[1] + 1.6).floor() as i32;
        let z = p.body.pos[2].floor() as i32;
        q.liquid(x, y, z).filter(|l| l.amount >= 1)
    };
    let in_water = p.body.wet == Some(LiquidKind::Water)
        || head_l.is_some_and(|l| l.kind == LiquidKind::Water);
    let in_lava =
        p.body.wet == Some(LiquidKind::Lava) || head_l.is_some_and(|l| l.kind == LiquidKind::Lava);

    if in_water {
        p.vitals.extinguish();
    }
    p.vitals.tick_lava(in_lava);
    if !p.god_mode
        && in_lava
        && p.vitals.obsidian_skin_ticks == 0
        && p.vitals.lava_time == 0
        && p.vitals.lava_cd == 0
    {
        p.ignite();
        p.vitals.lava_cd = consts::LAVA_IMMUNITY_TICKS;
        let dmg = crate::combat::damage_taken(consts::LAVA_DAMAGE, p.defense());
        ev.push(PlayerEvent::Hurt { amount: dmg });
        p.last_damage_cause = Some(DeathCause::Lava);
        if p.vitals.hurt(dmg) {
            p.kill(DeathCause::Lava, &mut ev);
            return ev;
        }
    }

    let (on_spikes, on_hot_block) = {
        let (x0, x1) = (
            (p.body.pos[0] - consts::HALF_WIDTH - 0.05).floor() as i32,
            (p.body.pos[0] + consts::HALF_WIDTH + 0.05).floor() as i32,
        );
        let (y0, y1) = (
            (p.body.pos[1] - 0.05).floor() as i32,
            (p.body.pos[1] + consts::HEIGHT + 0.05).floor() as i32,
        );
        let (z0, z1) = (
            (p.body.pos[2] - consts::HALF_WIDTH - 0.05).floor() as i32,
            (p.body.pos[2] + consts::HALF_WIDTH + 0.05).floor() as i32,
        );
        let mut spikes = false;
        let mut hot = false;
        for x in x0..=x1 {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    let b = q.block(x, y, z);
                    if b == crate::block::SPIKES {
                        spikes = true;
                    }
                    if b == crate::block::HELLSTONE
                        || b == crate::block::HELLSTONE_BRICK
                        || b == crate::block::METEORITE_ORE
                    {
                        hot = true;
                    }
                }
            }
        }
        (spikes, hot)
    };
    if !p.god_mode && on_spikes && p.i_frames == 0 {
        p.i_frames = 40;
        let dmg = crate::combat::damage_taken(40, p.defense());
        ev.push(PlayerEvent::Hurt { amount: dmg });
        p.last_damage_cause = Some(DeathCause::Suffocation);
        if p.vitals.hurt(dmg) {
            p.kill(DeathCause::Suffocation, &mut ev);
            return ev;
        }
    }
    if on_hot_block && !p.fire_walk() {
        p.vitals.burning_ticks = p.vitals.burning_ticks.max(consts::HOT_BURN_TICKS);
    }
    if p.god_mode || p.has_molten_set() || p.vitals.obsidian_skin_ticks > 0 {
        p.vitals.extinguish();
        p.vitals.burning_ticks = 0;
    }

    if !p.god_mode && (p.vitals.tick_burning() || p.vitals.tick_burn()) {
        p.last_damage_cause = Some(DeathCause::Fire);
        if p.vitals.life == 0 {
            p.kill(DeathCause::Fire, &mut ev);
            return ev;
        }
    }

    let submerged = head_submerged(q, &p.body);
    if !p.god_mode && p.vitals.tick_breath(submerged) {
        ev.push(PlayerEvent::Hurt {
            amount: consts::DROWN_DAMAGE,
        });
        p.last_damage_cause = Some(DeathCause::Drown);
        if p.vitals.life == 0 {
            p.kill(DeathCause::Drown, &mut ev);
            return ev;
        }
    }

    let moving = p.body.vel[0] != 0.0 || p.body.vel[1] != 0.0 || p.body.vel[2] != 0.0;
    p.vitals.tick_potion();
    let bonus_regen = if p.has_accessory(crate::item::BAND_OF_REGENERATION) {
        2
    } else {
        0
    } + if p.vitals.regen_ticks > 0 { 4 } else { 0 }
        + if p.vitals.campfire_active { 2 } else { 0 }
        + if p.vitals.heart_lantern_active { 2 } else { 0 };
    p.vitals.regen_life_ext(moving, bonus_regen);
    let eff_mana_max = p.effective_mana_max();
    p.vitals.regen_mana_with_max(moving, eff_mana_max);
    ev
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PlayerInput {
    /// -1..=1, +1 = forward.
    pub forward: f32,
    /// -1..=1, +1 = right.
    pub strafe: f32,
    pub jump: bool,
    /// Down held: fall through one-way platforms.
    pub drop: bool,
    /// Radians; 0 faces -z (Bevy camera default).
    pub yaw: f32,
    /// Radians; positive looks up.
    pub pitch: f32,
    /// Use-item button held.
    pub use_held: bool,
    /// Hotbar slot the player wants selected, 0..10.
    pub slot: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerBody {
    /// Feet centre.
    pub pos: [f32; 3],
    /// Blocks per tick.
    pub vel: [f32; 3],
    pub on_ground: bool,
    pub jump_ticks: i32,
    pub jump_held_prev: bool,
    /// Highest y since the body last stood or rose (for fall distance).
    pub fall_peak_y: f32,
    pub wet: Option<LiquidKind>,
    pub run_ticks: u32,
    pub double_jump_ready: bool,
    pub has_hermes_boots: bool,
    pub has_cloud_in_a_bottle: bool,
    pub has_shiny_red_balloon: bool,
    pub has_shadow_set: bool,
    pub has_molten_set: bool,
    pub no_knockback: bool,
    pub move_speed_bonus: f32,
    pub melee_speed_bonus: f32,
    pub swiftness: bool,
    pub water_walking: bool,
}

impl PlayerBody {
    pub fn new(pos: [f32; 3]) -> Self {
        Self {
            pos,
            vel: [0.0; 3],
            on_ground: false,
            jump_ticks: 0,
            jump_held_prev: false,
            fall_peak_y: pos[1],
            wet: None,
            run_ticks: 0,
            double_jump_ready: true,
            has_hermes_boots: false,
            has_cloud_in_a_bottle: false,
            has_shiny_red_balloon: false,
            has_shadow_set: false,
            has_molten_set: false,
            no_knockback: false,
            move_speed_bonus: 0.0,
            melee_speed_bonus: 0.0,
            swiftness: false,
            water_walking: false,
        }
    }

    /// Molten Armor's +7% melee speed plus accessory melee-speed prefixes: only melee
    /// and tool items benefit; guns, bows and magic keep their own timings.
    pub fn molten_melee_use_time_mult(&self, class: ItemClass) -> f32 {
        if matches!(class, ItemClass::Melee | ItemClass::Tool) {
            (1.0 - if self.has_molten_set { 0.07 } else { 0.0 } - self.melee_speed_bonus).max(0.5)
        } else {
            1.0
        }
    }
}

pub fn head_submerged(q: &impl BlockQuery, b: &PlayerBody) -> bool {
    let head_y = b.pos[1] + 1.6;
    let x = b.pos[0].floor() as i32;
    let y = head_y.floor() as i32;
    let z = b.pos[2].floor() as i32;
    q.liquid(x, y, z).is_some_and(|l| {
        l.kind == LiquidKind::Water
            && l.amount >= 1
            && (y as f32 + l.amount as f32 / 255.0) >= head_y
    })
}

fn query_body_liquid(q: &impl BlockQuery, b: &PlayerBody) -> Option<LiquidKind> {
    let x = b.pos[0].floor() as i32;
    let z = b.pos[2].floor() as i32;
    let feet_y = (b.pos[1] + 0.1).floor() as i32;
    let torso_y = (b.pos[1] + 0.9).floor() as i32;
    let feet_l = q.liquid(x, feet_y, z).filter(|l| l.amount >= 1);
    let torso_l = q.liquid(x, torso_y, z).filter(|l| l.amount >= 1);
    if feet_l.is_some_and(|l| l.kind == LiquidKind::Lava)
        || torso_l.is_some_and(|l| l.kind == LiquidKind::Lava)
    {
        Some(LiquidKind::Lava)
    } else if feet_l.is_some_and(|l| l.kind == LiquidKind::Water)
        || torso_l.is_some_and(|l| l.kind == LiquidKind::Water)
    {
        Some(LiquidKind::Water)
    } else {
        None
    }
}

#[derive(Default, Debug, PartialEq)]
pub struct StepEvents {
    /// Set on every tick the body lands, including zero-distance landings
    /// (e.g. stepping off a ledge edge): blocks fallen from the peak. Callers
    /// apply any damage threshold.
    pub landed_fall_blocks: Option<f32>,
}

/// True when the block directly under the feet centre is ice.
fn on_ice(q: &impl BlockQuery, body: &PlayerBody) -> bool {
    q.block(
        body.pos[0].floor() as i32,
        (body.pos[1] - 0.05).floor() as i32,
        body.pos[2].floor() as i32,
    ) == crate::block::ICE
}

/// Advance one 60 Hz tick.
///
/// Ports Player.HorizontalMovement (run acceleration / slowdown) in 2D and
/// Player.JumpMovement (jump counter, releaseJump) plus the gravity and
/// max-fall-speed lines of Player.Update, in blocks and +y up.
pub fn step_body(q: &impl BlockQuery, body: &mut PlayerBody, input: &PlayerInput) -> StepEvents {
    step_body_bounded(q, body, input, None)
}

/// [`step_body`] with optional world-edge bounds clamping.
pub fn step_body_bounded(
    q: &impl BlockQuery,
    body: &mut PlayerBody,
    input: &PlayerInput,
    bounds: Option<&WorldBounds>,
) -> StepEvents {
    let mut ev = StepEvents::default();

    body.wet = query_body_liquid(q, body);
    if body.wet.is_some() {
        body.fall_peak_y = body.pos[1];
    }

    let move_scale = match body.wet {
        Some(LiquidKind::Water) => consts::WATER_MOVE_SCALE,
        Some(LiquidKind::Lava) => consts::LAVA_MOVE_SCALE,
        None => 1.0,
    };

    // Horizontal: wish direction from input rotated by yaw (yaw 0 faces -z).
    let (s, c) = input.yaw.sin_cos();
    let (f, r) = (input.forward, input.strafe);
    let mut wish = [-s * f + c * r, -c * f - s * r];
    let wl = wish[0].hypot(wish[1]);
    if wl > 1.0 {
        wish = [wish[0] / wl, wish[1] / wl];
    }
    let has_input = wl > 1e-6;

    if has_input {
        if body.on_ground {
            body.run_ticks = body.run_ticks.saturating_add(1);
        }
    } else {
        body.run_ticks = 0;
    }
    let mut speed_mult = if body.has_hermes_boots && body.run_ticks >= 20 {
        2.0
    } else {
        1.0
    };
    if body.has_shadow_set {
        speed_mult *= 1.15;
    }
    speed_mult *= 1.0 + body.move_speed_bonus + if body.swiftness { 0.25 } else { 0.0 };
    let max_run_speed = consts::MAX_RUN_SPEED * move_scale * speed_mult;
    let (vx, vz) = (body.vel[0], body.vel[2]);
    let snappy = body.on_ground && !on_ice(q, body);
    if snappy {
        let m = vx.hypot(vz);
        let (mut cx, mut cz) = (vx, vz);
        let (tx, tz, rate) = if has_input {
            // Carry the current speed onto the new heading.
            // `wl` is the pre-clamp length, so measure the (possibly normalized) wish itself.
            let dl = wish[0].hypot(wish[1]);
            if m > 1e-6 {
                cx = wish[0] / dl * m;
                cz = wish[1] / dl * m;
            }
            (
                wish[0] * max_run_speed,
                wish[1] * max_run_speed,
                consts::GROUND_ACCEL * move_scale * speed_mult,
            )
        } else {
            (0.0, 0.0, consts::GROUND_STOP * move_scale)
        };
        let (dx, dz) = (tx - cx, tz - cz);
        let d = dx.hypot(dz);
        // Small epsilon absorbs float drift so the last step lands exactly on target.
        if d <= rate + 1e-6 {
            body.vel[0] = tx;
            body.vel[2] = tz;
        } else {
            body.vel[0] = cx + dx / d * rate;
            body.vel[2] = cz + dz / d * rate;
        }
    } else if has_input {
        let accel = (if body.on_ground {
            consts::RUN_ACCELERATION
        } else {
            consts::AIR_ACCEL
        }) * move_scale
            * speed_mult;
        let mut nx = vx + wish[0] * accel;
        let mut nz = vz + wish[1] * accel;
        let m = nx.hypot(nz);
        if m > max_run_speed {
            let k = max_run_speed / m;
            nx *= k;
            nz *= k;
        }
        body.vel[0] = nx;
        body.vel[2] = nz;
    } else {
        let m = vx.hypot(vz);
        let slowdown = consts::RUN_SLOWDOWN * move_scale;
        if m <= slowdown {
            body.vel[0] = 0.0;
            body.vel[2] = 0.0;
        } else {
            let k = (m - slowdown) / m;
            body.vel[0] *= k;
            body.vel[2] *= k;
        }
    }

    // Vertical: sustain an active jump, start one on a fresh press, else gravity.
    let pressed = input.jump && !body.jump_held_prev;
    if !input.jump {
        body.jump_ticks = 0;
    }
    let jump_speed = if body.has_shiny_red_balloon {
        consts::JUMP_SPEED * 1.33
    } else {
        consts::JUMP_SPEED
    };
    let jump_height_ticks = if body.has_shiny_red_balloon {
        (consts::JUMP_HEIGHT_TICKS as f32 * 1.33).round() as i32
    } else {
        consts::JUMP_HEIGHT_TICKS
    };
    if body.wet == Some(LiquidKind::Water) {
        if input.jump {
            body.vel[1] = consts::SWIM_UP_SPEED;
            body.jump_ticks = 0;
        } else {
            let grav = consts::GRAVITY * consts::WATER_GRAVITY_SCALE;
            let max_fall = consts::MAX_FALL_SPEED * consts::WATER_GRAVITY_SCALE;
            body.vel[1] = (body.vel[1] - grav).max(-max_fall);
        }
    } else if body.wet == Some(LiquidKind::Lava) {
        if input.jump && pressed && body.on_ground {
            body.vel[1] = jump_speed;
            body.jump_ticks = jump_height_ticks;
        } else if input.jump
            && pressed
            && !body.on_ground
            && body.has_cloud_in_a_bottle
            && body.double_jump_ready
        {
            body.double_jump_ready = false;
            body.fall_peak_y = body.pos[1];
            body.vel[1] = jump_speed;
            body.jump_ticks = ((jump_height_ticks as f32) * 0.75).round() as i32;
        } else if body.jump_ticks > 0 {
            body.vel[1] = jump_speed;
            body.jump_ticks -= 1;
        } else {
            let grav = consts::GRAVITY * consts::WATER_GRAVITY_SCALE;
            let max_fall = consts::MAX_FALL_SPEED * consts::WATER_GRAVITY_SCALE;
            body.vel[1] = (body.vel[1] - grav).max(-max_fall);
        }
    } else if body.jump_ticks > 0 {
        body.vel[1] = jump_speed;
        body.jump_ticks -= 1;
    } else if input.jump && pressed && body.on_ground {
        body.vel[1] = jump_speed;
        body.jump_ticks = jump_height_ticks;
    } else if input.jump
        && pressed
        && !body.on_ground
        && body.has_cloud_in_a_bottle
        && body.double_jump_ready
    {
        body.double_jump_ready = false;
        body.fall_peak_y = body.pos[1];
        body.vel[1] = jump_speed;
        body.jump_ticks = ((jump_height_ticks as f32) * 0.75).round() as i32;
    } else {
        body.vel[1] = (body.vel[1] - consts::GRAVITY).max(-consts::MAX_FALL_SPEED);
    }
    body.jump_held_prev = input.jump;

    // Collide: y, then x, then z.
    let was_on_ground = body.on_ground;
    let dy = body.vel[1];
    let blocked_y =
        collide::move_axis_drop_walk(q, &mut body.pos, 1, dy, input.drop, body.water_walking);
    body.on_ground = blocked_y && dy < 0.0;
    if body.on_ground {
        body.double_jump_ready = true;
    }
    if blocked_y {
        body.vel[1] = 0.0;
        if dy > 0.0 {
            body.jump_ticks = 0;
        }
    }
    // Auto-step only while standing and not rising, so a jump press beats a ledge snap.
    let grounded = (body.on_ground || was_on_ground) && body.vel[1] <= 0.0;
    if collide::move_horizontal(q, &mut body.pos, 0, body.vel[0], grounded, input.drop) {
        body.vel[0] = 0.0;
    }
    if collide::move_horizontal(q, &mut body.pos, 2, body.vel[2], grounded, input.drop) {
        body.vel[2] = 0.0;
    }
    if let Some(b) = bounds {
        b.clamp_body(&mut body.pos, &mut body.vel, consts::HALF_WIDTH);
    }

    // Fall tracking.
    if body.wet.is_some() {
        body.fall_peak_y = body.pos[1];
    } else if body.on_ground {
        if !was_on_ground {
            ev.landed_fall_blocks = Some((body.fall_peak_y - body.pos[1]).max(0.0));
        }
        body.fall_peak_y = body.pos[1];
    } else {
        body.fall_peak_y = body.fall_peak_y.max(body.pos[1]);
    }

    body.wet = query_body_liquid(q, body);
    if body.wet.is_some() {
        body.fall_peak_y = body.pos[1];
    }
    ev
}

#[cfg(test)]
mod tests {
    use super::consts::*;
    use super::*;
    use crate::block::{AIR, BlockId, SOLID_UNLOADED, STONE};
    use crate::liquid_cell::Liquid;

    struct Q<F: Fn(i32, i32, i32) -> bool>(F);
    impl<F: Fn(i32, i32, i32) -> bool> BlockQuery for Q<F> {
        fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
            if (self.0)(x, y, z) { STONE } else { AIR }
        }
    }
    fn floor() -> Q<impl Fn(i32, i32, i32) -> bool> {
        Q(|_, y, _| y < 0)
    }
    fn run(q: &impl BlockQuery, b: &mut PlayerBody, i: &PlayerInput, n: usize) {
        for _ in 0..n {
            step_body(q, b, i);
        }
    }
    fn hspeed(b: &PlayerBody) -> f32 {
        b.vel[0].hypot(b.vel[2])
    }
    fn fwd() -> PlayerInput {
        PlayerInput {
            forward: 1.0,
            ..Default::default()
        }
    }
    fn jump() -> PlayerInput {
        PlayerInput {
            jump: true,
            ..Default::default()
        }
    }
    /// Settled on the floor at the origin.
    fn grounded() -> (Q<impl Fn(i32, i32, i32) -> bool>, PlayerBody) {
        let q = floor();
        let mut b = PlayerBody::new([0.5, 0.0, 0.5]);
        run(&q, &mut b, &PlayerInput::default(), 5);
        (q, b)
    }
    fn apex(hold_ticks: usize) -> f32 {
        let (q, mut b) = grounded();
        let mut max = 0.0f32;
        for t in 0..120 {
            let inp = if t < hold_ticks {
                jump()
            } else {
                PlayerInput::default()
            };
            step_body(&q, &mut b, &inp);
            max = max.max(b.pos[1]);
        }
        max
    }

    #[test]
    fn standing_stays_on_floor() {
        let q = floor();
        let mut b = PlayerBody::new([0.5, 0.0, 0.5]);
        run(&q, &mut b, &PlayerInput::default(), 60);
        assert!(b.pos[1].abs() < 1e-4);
        assert!(b.on_ground);
    }

    #[test]
    fn run_reaches_max_speed() {
        let (q, mut b) = grounded();
        run(&q, &mut b, &fwd(), 6);
        assert!((hspeed(&b) - MAX_RUN_SPEED).abs() < 1e-4);
        run(&q, &mut b, &fwd(), 114);
        assert!((hspeed(&b) - MAX_RUN_SPEED).abs() < 1e-4);
    }

    fn diag() -> PlayerInput {
        PlayerInput {
            forward: 1.0,
            strafe: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn ground_diagonal_reaches_max_within_6_ticks() {
        let (q, mut b) = grounded();
        run(&q, &mut b, &diag(), 6);
        assert!(hspeed(&b) >= MAX_RUN_SPEED - 1e-4, "{}", hspeed(&b));
    }

    #[test]
    fn ground_diagonal_turn_maintains_speed() {
        let (q, mut b) = grounded();
        b.vel[2] = -MAX_RUN_SPEED;
        step_body(&q, &mut b, &diag());
        assert!(hspeed(&b) >= MAX_RUN_SPEED - 1e-4, "{}", hspeed(&b));
    }

    /// Settled on an ice floor (ICE at y = -1).
    fn on_ice_floor() -> (Bq, PlayerBody) {
        let q = Bq((-3..4)
            .flat_map(|x| (-300..4).map(move |z| ([x, -1, z], crate::block::ICE)))
            .collect());
        let mut b = PlayerBody::new([0.5, 0.0, 0.5]);
        run(&q, &mut b, &PlayerInput::default(), 5);
        (q, b)
    }

    fn at_max(b: &mut PlayerBody) {
        b.vel[2] = -MAX_RUN_SPEED;
    }

    #[test]
    fn ground_reaches_max_within_6_ticks() {
        let (q, mut b) = grounded();
        run(&q, &mut b, &fwd(), 6);
        assert!(hspeed(&b) >= MAX_RUN_SPEED - 1e-4, "{}", hspeed(&b));
    }

    #[test]
    fn ground_stops_within_3_ticks() {
        let (q, mut b) = grounded();
        at_max(&mut b);
        run(&q, &mut b, &PlayerInput::default(), 3);
        assert_eq!(hspeed(&b), 0.0);
    }

    #[test]
    fn ground_turn_redirects_momentum() {
        let (q, mut b) = grounded();
        at_max(&mut b);
        let inp = PlayerInput {
            forward: 1.0,
            yaw: std::f32::consts::FRAC_PI_2,
            ..Default::default()
        };
        step_body(&q, &mut b, &inp);
        // yaw pi/2 faces -x.
        let ang = b.vel[2].atan2(b.vel[0]) - 0.0f32.atan2(-1.0);
        let ang = ang.rem_euclid(std::f32::consts::TAU);
        let ang = ang.min(std::f32::consts::TAU - ang);
        assert!(ang < 1e-3, "angle {ang}, vel {:?}", b.vel);
    }

    #[test]
    fn ground_reverse_within_3_ticks() {
        let (q, mut b) = grounded();
        at_max(&mut b);
        let back = PlayerInput {
            forward: -1.0,
            ..Default::default()
        };
        run(&q, &mut b, &back, 3);
        assert!(b.vel[2] > 0.0, "{:?}", b.vel);
    }

    #[test]
    fn air_steering_is_softer() {
        let (q, mut b) = grounded();
        let inp = PlayerInput {
            forward: 1.0,
            jump: true,
            ..Default::default()
        };
        step_body(&q, &mut b, &inp);
        assert!(!b.on_ground);
        run(&q, &mut b, &inp, 6);
        assert!(hspeed(&b) < 0.5 * MAX_RUN_SPEED, "{}", hspeed(&b));
    }

    #[test]
    fn ice_keeps_terraria_slide() {
        let (q, mut b) = on_ice_floor();
        run(&q, &mut b, &fwd(), 6);
        assert!(hspeed(&b) < 0.5 * MAX_RUN_SPEED, "{}", hspeed(&b));
        let (q, mut b) = on_ice_floor();
        at_max(&mut b);
        run(&q, &mut b, &PlayerInput::default(), 3);
        assert!(hspeed(&b) > 0.0);
    }

    #[test]
    fn run_stops_with_slowdown() {
        let (q, mut b) = grounded();
        run(&q, &mut b, &fwd(), 120);
        let n = (MAX_RUN_SPEED / RUN_SLOWDOWN).ceil() as usize + 1;
        run(&q, &mut b, &PlayerInput::default(), n);
        assert!(hspeed(&b) < 1e-6);
    }

    #[test]
    fn held_jump_apex() {
        let a = apex(120);
        assert!((4.3..=4.8).contains(&a), "apex {a}");
    }

    #[test]
    fn tap_jump_is_lower() {
        assert!(apex(1) < 0.6 * apex(120));
    }

    #[test]
    fn no_double_jump_in_air() {
        let (q, mut b) = grounded();
        let mut landed_at = None;
        let mut left = false;
        for t in 0..200 {
            step_body(&q, &mut b, &jump());
            if !b.on_ground {
                left = true;
            } else if left && landed_at.is_none() {
                landed_at = Some(t);
            }
            if landed_at.is_some() {
                // still holding: must not have re-jumped
                assert!(b.pos[1] < 1e-4 && b.on_ground, "re-jumped at {t}");
                if t > landed_at.unwrap_or(t) + 20 {
                    break;
                }
            }
        }
        assert!(landed_at.is_some());
        // release then press again: jumps
        step_body(&q, &mut b, &PlayerInput::default());
        step_body(&q, &mut b, &jump());
        assert!(b.vel[1] > 0.0);
    }

    #[test]
    fn fall_speed_capped() {
        let q = Q(|_, _, _| false);
        let mut b = PlayerBody::new([0.5, 200.0, 0.5]);
        for _ in 0..300 {
            step_body(&q, &mut b, &PlayerInput::default());
            assert!(b.vel[1] >= -MAX_FALL_SPEED - 1e-6);
        }
    }

    #[test]
    fn fast_fall_does_not_tunnel() {
        let q = Q(|_, y, _| y == -1);
        let mut b = PlayerBody::new([0.5, 100.0, 0.5]);
        run(&q, &mut b, &PlayerInput::default(), 600);
        assert!((b.pos[1]).abs() < 1e-4, "y {}", b.pos[1]);
    }

    #[test]
    fn wall_blocks_movement() {
        let q = Q(|x, y, _| y < 0 || x >= 2);
        let mut b = PlayerBody::new([0.5, 0.0, 0.5]);
        for _ in 0..200 {
            step_body(
                &q,
                &mut b,
                &PlayerInput {
                    forward: 0.0,
                    strafe: 1.0,
                    ..Default::default()
                },
            );
            assert!(b.pos[0] + 0.3 <= 2.0 + 1e-4);
        }
        assert!(b.pos[0] > 1.6);
    }

    /// Run +x (strafe right at yaw 0) from x=0.5.
    fn run_x(q: &impl BlockQuery) -> PlayerBody {
        let mut b = PlayerBody::new([0.5, 0.0, 0.5]);
        run(
            q,
            &mut b,
            &PlayerInput {
                strafe: 1.0,
                ..Default::default()
            },
            200,
        );
        b
    }

    #[test]
    fn auto_step_one_block() {
        let b = run_x(&Q(|x, y, _| y < 0 || (x >= 2 && y == 0)));
        assert!((b.pos[1] - 1.0).abs() < 1e-3, "y {}", b.pos[1]);
        assert!(b.pos[0] > 2.5);
    }

    #[test]
    fn jump_against_ledge_jumps() {
        let q = Q(|x, y, _| y < 0 || (x >= 1 && y == 0));
        let mut b = PlayerBody::new([0.69, 0.0, 0.5]);
        run(&q, &mut b, &PlayerInput::default(), 5);
        // Pressed against the ledge (body edge at x=1.0), press jump + push into it.
        b.pos[0] = 0.7;
        let inp = PlayerInput {
            strafe: 1.0,
            jump: true,
            ..Default::default()
        };
        step_body(&q, &mut b, &inp);
        assert!(b.pos[1] < 0.6, "snapped up: y {}", b.pos[1]);
        assert!(b.vel[1] > 0.0);
    }

    #[test]
    fn no_auto_step_two_blocks() {
        let b = run_x(&Q(|x, y, _| y < 0 || (x >= 2 && y <= 1)));
        assert!(b.pos[0] <= 1.7 + 1e-4 && b.pos[1] < 1e-3);
    }

    #[test]
    fn no_auto_step_into_low_ceiling() {
        let b = run_x(&Q(|x, y, _| y < 0 || (x >= 2 && (y == 0 || y == 2))));
        assert!(b.pos[0] <= 1.7 + 1e-4 && b.pos[1] < 1e-3);
    }

    #[test]
    fn head_bump_ends_jump() {
        let q = Q(|_, y, _| !(0..3).contains(&y));
        let mut b = PlayerBody::new([0.5, 0.0, 0.5]);
        run(&q, &mut b, &PlayerInput::default(), 5);
        let mut bumped = false;
        for _ in 0..40 {
            step_body(&q, &mut b, &jump());
            if bumped {
                assert!(b.vel[1] <= 0.0);
                break;
            }
            if b.pos[1] + 1.8 > 3.0 - 1e-3 {
                bumped = true;
            }
        }
        assert!(bumped);
    }

    #[test]
    fn unloaded_is_solid() {
        let q = Q(|_, y, _| y < 0);
        struct U<'a, T: BlockQuery>(&'a T);
        impl<T: BlockQuery> BlockQuery for U<'_, T> {
            fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
                if y < 0 {
                    SOLID_UNLOADED
                } else {
                    self.0.block(x, y, z)
                }
            }
        }
        let u = U(&q);
        let mut b = PlayerBody::new([0.5, 0.0, 0.5]);
        run(&u, &mut b, &PlayerInput::default(), 60);
        assert!(b.pos[1] > -1e-4);
    }

    #[test]
    fn landed_reports_fall() {
        let q = floor();
        let mut b = PlayerBody::new([0.5, 10.0, 0.5]);
        let mut reports = vec![];
        for _ in 0..300 {
            if let Some(f) = step_body(&q, &mut b, &PlayerInput::default()).landed_fall_blocks {
                reports.push(f);
            }
        }
        assert_eq!(reports.len(), 1);
        assert!((reports[0] - 10.0).abs() < 0.5, "{reports:?}");
    }

    fn stack(id: ItemId) -> ItemStack {
        ItemStack {
            id,
            count: 1,
            prefix: Prefix(0),
        }
    }

    #[test]
    fn armor_defense_sums_equipped_pieces() {
        use crate::item::{IRON_CHAINMAIL, IRON_HELMET};
        let mut p = Player::new([0.5, 10.0, 0.5]);
        let mut inv = Inventory::empty();
        assert_eq!(p.defense(), 0);
        inv.slots[3] = Some(stack(IRON_HELMET));
        inv.slots[4] = Some(stack(IRON_CHAINMAIL));
        assert!(p.equip(&mut inv, 3) && p.equip(&mut inv, 4));
        assert_eq!(p.defense(), 2 + 3);
    }

    #[test]
    fn armor_equip_swaps_and_returns_old_piece() {
        use crate::item::{COPPER_HELMET, IRON_HELMET};
        let mut p = Player::new([0.5, 10.0, 0.5]);
        let mut inv = Inventory::empty();
        inv.slots[7] = Some(stack(COPPER_HELMET));
        assert!(p.equip(&mut inv, 7));
        assert_eq!((p.armor[0], inv.slots[7]), (Some(COPPER_HELMET), None));
        inv.slots[9] = Some(stack(IRON_HELMET));
        assert!(p.equip(&mut inv, 9));
        assert_eq!(p.armor[0], Some(IRON_HELMET));
        assert_eq!(inv.slots[9].unwrap().id, COPPER_HELMET);
        assert!(p.unequip(&mut inv, ArmorSlot::Head));
        assert_eq!(p.armor[0], None);
        assert_eq!(inv.count_of(IRON_HELMET), 1);
    }

    #[test]
    fn armor_equip_rejects_non_armor_bad_slot_and_dead() {
        use crate::item::{COPPER_BROADSWORD, IRON_HELMET};
        let mut p = Player::new([0.5, 10.0, 0.5]);
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(stack(COPPER_BROADSWORD));
        inv.slots[1] = Some(stack(IRON_HELMET));
        let before = inv.clone();
        assert!(!p.equip(&mut inv, 0));
        assert!(!p.equip(&mut inv, 2)); // empty
        assert!(!p.equip(&mut inv, 99)); // out of range
        p.dead = true;
        assert!(!p.equip(&mut inv, 1));
        assert_eq!((inv, p.armor), (before, [None; 3]));
    }

    /// Stone floor below y = 0 plus explicit blocks.
    struct Bq(Vec<([i32; 3], BlockId)>);
    impl BlockQuery for Bq {
        fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
            if let Some((_, b)) = self.0.iter().find(|(p, _)| *p == [x, y, z]) {
                *b
            } else if y < 0 {
                STONE
            } else {
                AIR
            }
        }
    }
    fn settle(q: &Bq, pos: [f32; 3]) -> PlayerBody {
        let mut b = PlayerBody::new(pos);
        run(q, &mut b, &PlayerInput::default(), 30);
        b
    }

    #[test]
    fn closed_door_blocks_walking_open_door_does_not() {
        use crate::block::{DOOR_CLOSED, DOOR_OPEN};
        for (door, blocked) in [(DOOR_CLOSED, true), (DOOR_OPEN, false)] {
            // Door column two blocks tall at x = 2.
            let q = Bq(vec![([2, 0, 0], door), ([2, 1, 0], door)]);
            let mut b = settle(&q, [0.5, 0.0, 0.5]);
            let right = PlayerInput {
                strafe: 1.0,
                yaw: 0.0,
                ..Default::default()
            };
            run(&q, &mut b, &right, 200);
            assert_eq!(b.pos[0] < 2.0, blocked, "{door:?} x={}", b.pos[0]);
        }
    }

    #[test]
    fn table_and_chair_do_not_block() {
        use crate::block::{CHAIR, TABLE};
        let q = Bq(vec![([2, 0, 0], TABLE), ([3, 0, 0], CHAIR)]);
        let mut b = settle(&q, [0.5, 0.0, 0.5]);
        let right = PlayerInput {
            strafe: 1.0,
            ..Default::default()
        };
        run(&q, &mut b, &right, 200);
        assert!(b.pos[0] > 4.0, "x={}", b.pos[0]);
    }

    #[test]
    fn platform_lets_player_jump_up_through() {
        use crate::block::PLATFORM;
        // Platform two blocks above the floor: head hits it unless it is one-way.
        let q = Bq(vec![([0, 2, 0], PLATFORM)]);
        let mut b = settle(&q, [0.5, 0.0, 0.5]);
        let mut peak = 0.0f32;
        for _ in 0..40 {
            step_body(&q, &mut b, &jump());
            peak = peak.max(b.pos[1]);
        }
        assert!(peak > 2.0, "peak {peak}");
        // Falls back and stands on the platform top.
        run(&q, &mut b, &PlayerInput::default(), 120);
        assert!(
            (b.pos[1] - 3.0).abs() < 1e-3 && b.on_ground,
            "y={}",
            b.pos[1]
        );
    }

    #[test]
    fn platform_holds_player_when_falling_onto_it() {
        use crate::block::PLATFORM;
        let q = Bq(vec![([0, 2, 0], PLATFORM)]);
        let mut b = settle(&q, [0.5, 6.0, 0.5]);
        assert!(
            (b.pos[1] - 3.0).abs() < 1e-3 && b.on_ground,
            "y={}",
            b.pos[1]
        );
        run(&q, &mut b, &PlayerInput::default(), 60);
        assert!((b.pos[1] - 3.0).abs() < 1e-3);
    }

    #[test]
    fn down_drops_through_platform() {
        use crate::block::PLATFORM;
        let q = Bq(vec![([0, 2, 0], PLATFORM)]);
        let mut b = settle(&q, [0.5, 3.0, 0.5]);
        assert!((b.pos[1] - 3.0).abs() < 1e-3);
        let down = PlayerInput {
            drop: true,
            ..Default::default()
        };
        run(&q, &mut b, &down, 30);
        run(&q, &mut b, &PlayerInput::default(), 60);
        assert!(b.pos[1] < 0.01, "y={}", b.pos[1]);
    }

    #[test]
    fn walking_into_platform_side_steps_onto_it() {
        use crate::block::PLATFORM;
        let q = Bq((2..40).map(|x| ([x, 0, 0], PLATFORM)).collect());
        let mut b = settle(&q, [0.5, 0.0, 0.5]);
        let right = PlayerInput {
            strafe: 1.0,
            ..Default::default()
        };
        run(&q, &mut b, &right, 120);
        assert!(b.pos[1] >= 0.99, "y={}", b.pos[1]);
    }

    struct LiquidQ<B, L> {
        block_fn: B,
        liquid_fn: L,
    }
    impl<B: Fn(i32, i32, i32) -> BlockId, L: Fn(i32, i32, i32) -> Option<Liquid>> BlockQuery
        for LiquidQ<B, L>
    {
        fn block(&self, x: i32, y: i32, z: i32) -> BlockId {
            (self.block_fn)(x, y, z)
        }
        fn liquid(&self, x: i32, y: i32, z: i32) -> Option<Liquid> {
            (self.liquid_fn)(x, y, z)
        }
    }

    #[test]
    fn swimming_slows_horizontal_speed_to_half() {
        let q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if (0..=10).contains(&y) {
                    Some(Liquid::water(255))
                } else {
                    None
                }
            },
        };
        let mut b = PlayerBody::new([0.5, 2.0, 0.5]);
        run(&q, &mut b, &fwd(), 30);
        let expected = MAX_RUN_SPEED * WATER_MOVE_SCALE;
        assert!(
            (hspeed(&b) - expected).abs() < 1e-4,
            "got {}, expected {}",
            hspeed(&b),
            expected
        );
    }

    #[test]
    fn jump_held_in_water_rises_slowly() {
        let q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if (0..=10).contains(&y) {
                    Some(Liquid::water(255))
                } else {
                    None
                }
            },
        };
        let mut b = PlayerBody::new([0.5, 2.0, 0.5]);
        step_body(&q, &mut b, &jump());
        assert!(
            (b.vel[1] - SWIM_UP_SPEED).abs() < 1e-4,
            "vel[1] was {}",
            b.vel[1]
        );
        let initial_y = b.pos[1];
        step_body(&q, &mut b, &jump());
        assert!(b.pos[1] > initial_y);
        assert!(SWIM_UP_SPEED < JUMP_SPEED);
    }

    #[test]
    fn sinks_when_no_input() {
        let q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if (0..=10).contains(&y) {
                    Some(Liquid::water(255))
                } else {
                    None
                }
            },
        };
        let mut b = PlayerBody::new([0.5, 5.0, 0.5]);
        step_body(&q, &mut b, &PlayerInput::default());
        assert!(b.vel[1] < 0.0);
        assert!(b.pos[1] < 5.0);
    }

    #[test]
    fn fall_damage_cancelled_by_water() {
        let q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if (0..=4).contains(&y) {
                    Some(Liquid::water(255))
                } else {
                    None
                }
            },
        };
        let mut p = Player::new([0.5, 30.0, 0.5]);
        let mut took_fall_damage = false;
        for _ in 0..120 {
            let events = tick_player(&q, &mut p, &PlayerInput::default());
            for ev in events {
                if let PlayerEvent::Hurt { .. } = ev {
                    took_fall_damage = true;
                }
            }
            if p.body.on_ground {
                break;
            }
        }
        assert!(p.body.on_ground);
        assert!(!took_fall_damage);
        assert_eq!(p.vitals.life, 100);
    }

    #[test]
    fn feet_only_in_water_does_not_drain_breath() {
        let q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if y == 0 {
                    Some(Liquid::water(255))
                } else {
                    None
                }
            },
        };
        let mut p = Player::new([0.5, 0.0, 0.5]);
        assert_eq!(p.vitals.breath, BREATH_MAX);
        for _ in 0..20 {
            tick_player(&q, &mut p, &PlayerInput::default());
        }
        assert_eq!(p.vitals.breath, BREATH_MAX);
    }

    #[test]
    fn spikes_hurt_player_on_contact() {
        let q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { crate::block::SPIKES } else { AIR },
            liquid_fn: |_, _, _| None,
        };
        let mut p = Player::new([0.5, 0.0, 0.5]);
        p.vitals.life = 100;
        let evs = tick_player(&q, &mut p, &PlayerInput::default());
        let hurt = evs.iter().any(|e| matches!(e, PlayerEvent::Hurt { .. }));
        assert!(hurt, "player on spikes should take damage");
        assert_eq!(p.i_frames, 40);
    }

    #[test]
    fn lava_hurts_80_once_per_30_ticks_and_ignites() {
        let q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if (0..=5).contains(&y) {
                    Some(Liquid::lava(255))
                } else {
                    None
                }
            },
        };
        let mut p = Player::new([0.5, 0.0, 0.5]);
        p.vitals.life = 200;
        p.vitals.life_max = 200;
        let mut lava_hurts = 0;
        for _ in 0..30 {
            let evs = tick_player(&q, &mut p, &PlayerInput::default());
            for ev in evs {
                if let PlayerEvent::Hurt { amount } = ev {
                    if amount == LAVA_DAMAGE {
                        lava_hurts += 1;
                    }
                }
            }
        }
        assert_eq!(lava_hurts, 1);
        assert_eq!(p.vitals.burn_ticks, BURN_TICKS - 30);
    }

    #[test]
    fn lava_charm_delays_first_lava_hit_by_420_ticks() {
        let q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if (0..=5).contains(&y) {
                    Some(Liquid::lava(255))
                } else {
                    None
                }
            },
        };
        let mut p = Player::new([0.5, 0.0, 0.5]);
        p.vitals.life = 200;
        p.vitals.life_max = 200;
        p.accessories[0] = Some(crate::item::LAVA_CHARM);
        p.vitals.lava_time = consts::LAVA_CHARM_TICKS;

        let mut lava_hurts = 0;
        for _ in 1..=419 {
            let evs = tick_player(&q, &mut p, &PlayerInput::default());
            for ev in evs {
                if let PlayerEvent::Hurt { amount } = ev {
                    if amount == LAVA_DAMAGE {
                        lava_hurts += 1;
                    }
                }
            }
        }
        assert_eq!(lava_hurts, 0, "no lava damage during the first 419 ticks");

        let evs = tick_player(&q, &mut p, &PlayerInput::default());
        let hit_on_420 = evs
            .iter()
            .any(|e| matches!(e, PlayerEvent::Hurt { amount } if *amount == LAVA_DAMAGE));
        assert!(hit_on_420, "first lava damage hits on tick 420");
    }

    #[test]
    fn obsidian_skin_grants_lava_immunity_and_no_burn() {
        let q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if y == 0 {
                    Some(Liquid::lava(255))
                } else {
                    None
                }
            },
        };
        let mut p = Player::new([0.5, 0.0, 0.5]);
        p.vitals.life = 200;
        p.vitals.life_max = 200;
        p.vitals.obsidian_skin_ticks = 3;

        for _ in 0..2 {
            let evs = tick_player(&q, &mut p, &PlayerInput::default());
            let hurt = evs.iter().any(|e| matches!(e, PlayerEvent::Hurt { .. }));
            assert!(
                !hurt,
                "obsidian skin should prevent all damage while active"
            );
            assert_eq!(p.vitals.burn_ticks, 0, "obsidian skin suppresses burning");
        }

        let evs = tick_player(&q, &mut p, &PlayerInput::default());
        let hurt = evs
            .iter()
            .any(|e| matches!(e, PlayerEvent::Hurt { amount } if *amount == LAVA_DAMAGE));
        assert!(hurt, "lava hurts once obsidian skin expires");
        assert!(
            p.vitals.burn_ticks > 0,
            "lava ignites once obsidian skin expires"
        );
    }

    #[test]
    fn obsidian_skull_prevents_hot_hellstone_burn() {
        struct HellstoneFloor;
        impl BlockQuery for HellstoneFloor {
            fn block(&self, _: i32, y: i32, _: i32) -> BlockId {
                if y < 0 { crate::block::HELLSTONE } else { AIR }
            }
        }

        let mut p_no_skull = Player::new([0.5, 0.0, 0.5]);
        tick_player(&HellstoneFloor, &mut p_no_skull, &PlayerInput::default());
        assert!(p_no_skull.vitals.burning_ticks > 0);

        let mut p_skull = Player::new([0.5, 0.0, 0.5]);
        p_skull.accessories[0] = Some(crate::item::OBSIDIAN_SKULL);
        tick_player(&HellstoneFloor, &mut p_skull, &PlayerInput::default());
        assert_eq!(p_skull.vitals.burning_ticks, 0);
    }

    #[test]
    fn obsidian_skull_has_one_defense() {
        use crate::item::item_data;
        assert_eq!(item_data(crate::item::OBSIDIAN_SKULL).defense, 1);

        let mut p = Player::new([0.5, 0.0, 0.5]);
        assert_eq!(p.defense(), 0);
        p.accessories[0] = Some(crate::item::OBSIDIAN_SKULL);
        assert_eq!(p.defense(), 1);
    }

    #[test]
    fn fully_submerged_in_water_extinguishes_burning() {
        let q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if (0..=3).contains(&y) {
                    Some(Liquid::water(255))
                } else {
                    None
                }
            },
        };
        let mut p = Player::new([0.5, 0.0, 0.5]);
        p.vitals.ignite();
        assert_eq!(p.vitals.burn_ticks, BURN_TICKS);
        tick_player(&q, &mut p, &PlayerInput::default());
        assert_eq!(p.vitals.burn_ticks, 0);
    }

    #[test]
    fn player_cannot_cross_edge() {
        let q = floor();
        let bounds = crate::world_bounds::WorldBounds::new(crate::world_bounds::WorldSize::Small);
        let mut b = PlayerBody::new([(bounds.half - 20) as f32, 0.0, 0.5]);
        let right = PlayerInput {
            strafe: 1.0,
            yaw: 0.0,
            ..Default::default()
        };
        for _ in 0..10_000 {
            super::step_body_bounded(&q, &mut b, &right, Some(&bounds));
        }
        let max_x = bounds.half as f32 - HALF_WIDTH;
        assert!(
            (b.pos[0] - max_x).abs() < 1e-4,
            "expected player stopped at {max_x}, got {}",
            b.pos[0]
        );
        assert_eq!(b.vel[0], 0.0);
    }

    #[test]
    fn shiny_red_balloon_increases_jump_height() {
        let q = floor();
        let mut p_normal = Player::new([0.5, 0.0, 0.5]);
        p_normal.body.on_ground = true;
        let mut p_balloon = Player::new([0.5, 0.0, 0.5]);
        p_balloon.body.on_ground = true;
        p_balloon.accessories[0] = Some(crate::item::SHINY_RED_BALLOON);
        let jump = PlayerInput {
            jump: true,
            ..Default::default()
        };
        let mut max_y_normal = 0.0f32;
        let mut max_y_balloon = 0.0f32;
        for _ in 0..60 {
            tick_player(&q, &mut p_normal, &jump);
            tick_player(&q, &mut p_balloon, &jump);
            max_y_normal = max_y_normal.max(p_normal.body.pos[1]);
            max_y_balloon = max_y_balloon.max(p_balloon.body.pos[1]);
        }
        assert!(
            max_y_balloon > max_y_normal * 1.25,
            "max_y_balloon ({max_y_balloon}) must be at least 25% higher than max_y_normal ({max_y_normal})"
        );
    }

    #[test]
    fn hermes_boots_increase_speed() {
        let q = floor();
        let mut p_normal = Player::new([0.5, 0.0, 0.5]);
        let mut p_boots = Player::new([0.5, 0.0, 0.5]);
        p_boots.accessories[0] = Some(crate::item::HERMES_BOOTS);
        let input = PlayerInput {
            forward: 1.0,
            yaw: 0.0,
            ..Default::default()
        };
        for _ in 0..40 {
            tick_player(&q, &mut p_normal, &input);
            tick_player(&q, &mut p_boots, &input);
        }
        let speed_normal = p_normal.body.vel[0].hypot(p_normal.body.vel[2]);
        let speed_boots = p_boots.body.vel[0].hypot(p_boots.body.vel[2]);
        assert!(
            speed_boots > speed_normal,
            "speed_boots ({speed_boots}) must exceed speed_normal ({speed_normal})"
        );
    }

    #[test]
    fn cloud_in_a_bottle_allows_double_jump() {
        let q = floor();
        let mut p = Player::new([0.5, 10.0, 0.5]);
        p.accessories[0] = Some(crate::item::CLOUD_IN_A_BOTTLE);
        p.body.double_jump_ready = true;
        // One tick falling without jump
        tick_player(&q, &mut p, &PlayerInput::default());
        assert!(p.body.vel[1] < 0.0, "falling");
        // Press jump mid-air
        let jump_input = PlayerInput {
            jump: true,
            ..Default::default()
        };
        tick_player(&q, &mut p, &jump_input);
        assert!(
            p.body.vel[1] > 0.0,
            "double jump should grant positive vertical velocity"
        );
    }

    #[test]
    fn lucky_horseshoe_prevents_fall_damage() {
        let q = floor();
        let mut p_normal = Player::new([0.5, 50.0, 0.5]);
        let mut p_shoe = Player::new([0.5, 50.0, 0.5]);
        p_shoe.accessories[0] = Some(crate::item::LUCKY_HORSESHOE);
        let input = PlayerInput::default();
        for _ in 0..200 {
            if !p_normal.body.on_ground {
                tick_player(&q, &mut p_normal, &input);
            }
            if !p_shoe.body.on_ground {
                tick_player(&q, &mut p_shoe, &input);
            }
        }
        assert!(p_normal.body.on_ground, "normal player must have landed");
        assert!(p_shoe.body.on_ground, "horseshoe player must have landed");
        assert!(
            p_normal.vitals.life < 100 || p_normal.dead,
            "normal player should take fall damage"
        );
        assert_eq!(
            p_shoe.vitals.life, 100,
            "horseshoe player should take no fall damage"
        );
        assert!(!p_shoe.dead);
    }

    #[test]
    fn band_of_regeneration_heals_player() {
        let q = floor();
        let mut p_normal = Player::new([0.5, 0.0, 0.5]);
        p_normal.vitals.life = 50;
        let mut p_band = Player::new([0.5, 0.0, 0.5]);
        p_band.vitals.life = 50;
        p_band.accessories[0] = Some(crate::item::BAND_OF_REGENERATION);
        let input = PlayerInput::default();
        for _ in 0..60 {
            tick_player(&q, &mut p_normal, &input);
            tick_player(&q, &mut p_band, &input);
        }
        assert_eq!(
            p_normal.vitals.life, 50,
            "normal player does not regenerate in first 60 ticks"
        );
        assert_eq!(
            p_band.vitals.life, 51,
            "band of regeneration grants +1 HP per second"
        );
    }

    #[test]
    fn shadow_armor_grants_15_percent_speed_set_bonus() {
        let q = floor();
        let mut p_normal = Player::new([0.5, 0.0, 0.5]);
        let mut p_shadow = Player::new([0.5, 0.0, 0.5]);
        let mut p_partial = Player::new([0.5, 0.0, 0.5]);
        let mut p_hermes = Player::new([0.5, 0.0, 0.5]);
        let mut p_shadow_hermes = Player::new([0.5, 0.0, 0.5]);

        // Full shadow set: Helmet (102), Scalemail (101), Greaves (100)
        p_shadow.armor[0] = Some(crate::item::SHADOW_HELMET);
        p_shadow.armor[1] = Some(crate::item::SHADOW_SCALEMAIL);
        p_shadow.armor[2] = Some(crate::item::SHADOW_GREAVES);

        // Partial set (missing greaves)
        p_partial.armor[0] = Some(crate::item::SHADOW_HELMET);
        p_partial.armor[1] = Some(crate::item::SHADOW_SCALEMAIL);

        // Hermes only
        p_hermes.accessories[0] = Some(crate::item::HERMES_BOOTS);

        // Shadow + Hermes
        p_shadow_hermes.armor[0] = Some(crate::item::SHADOW_HELMET);
        p_shadow_hermes.armor[1] = Some(crate::item::SHADOW_SCALEMAIL);
        p_shadow_hermes.armor[2] = Some(crate::item::SHADOW_GREAVES);
        p_shadow_hermes.accessories[0] = Some(crate::item::HERMES_BOOTS);

        assert!(p_shadow.has_shadow_set());
        assert!(!p_partial.has_shadow_set());
        assert_eq!(p_shadow.defense(), 19);

        let input = PlayerInput {
            forward: 1.0,
            yaw: 0.0,
            ..Default::default()
        };

        // Tick normal, partial, and shadow players until steady state (10 ticks)
        for _ in 0..10 {
            tick_player(&q, &mut p_normal, &input);
            tick_player(&q, &mut p_partial, &input);
            tick_player(&q, &mut p_shadow, &input);
        }

        let speed_normal = p_normal.body.vel[0].hypot(p_normal.body.vel[2]);
        let speed_partial = p_partial.body.vel[0].hypot(p_partial.body.vel[2]);
        let speed_shadow = p_shadow.body.vel[0].hypot(p_shadow.body.vel[2]);

        assert!(
            (speed_partial - speed_normal).abs() < 1e-4,
            "partial armor should not grant set bonus"
        );
        let ratio = speed_shadow / speed_normal;
        assert!(
            (ratio - 1.15).abs() < 1e-3,
            "full shadow armor must give +15% movement speed, got ratio {ratio}"
        );

        // Stacking with Hermes Boots (>= 20 ticks)
        for _ in 0..40 {
            tick_player(&q, &mut p_hermes, &input);
            tick_player(&q, &mut p_shadow_hermes, &input);
        }
        let speed_hermes = p_hermes.body.vel[0].hypot(p_hermes.body.vel[2]);
        let speed_shadow_hermes = p_shadow_hermes.body.vel[0].hypot(p_shadow_hermes.body.vel[2]);
        let hermes_stack_ratio = speed_shadow_hermes / speed_hermes;
        assert!(
            (hermes_stack_ratio - 1.15).abs() < 1e-3,
            "shadow armor set bonus must stack with hermes boots (+15% on top of hermes speed), got {hermes_stack_ratio}"
        );
    }

    #[test]
    fn molten_armor_set_bonus_and_fire_immunity() {
        let mut p = Player::new([0.0; 3]);
        assert_eq!(p.defense(), 0);
        p.armor = [
            Some(crate::item::MOLTEN_HELMET),
            Some(crate::item::MOLTEN_BREASTPLATE),
            Some(crate::item::MOLTEN_GREAVES),
        ];
        p.sync_accessories();
        assert_eq!(p.defense(), 25);
        assert!(p.has_molten_set());
        assert!((p.melee_damage_mult() - 1.17).abs() < 1e-6);
        assert_eq!(p.melee_crit_bonus(), 7);
        assert!((p.melee_speed_mult() - 0.93).abs() < 1e-6);
        assert!(p.fire_walk());
        assert!(p.body.has_molten_set);
    }

    #[test]
    fn molten_armor_does_not_burn_on_hot_blocks() {
        struct HellstoneFloor;
        impl BlockQuery for HellstoneFloor {
            fn block(&self, _: i32, y: i32, _: i32) -> BlockId {
                if y < 0 { crate::block::HELLSTONE } else { AIR }
            }
        }

        let mut p_bare = Player::new([0.5, 0.0, 0.5]);
        tick_player(&HellstoneFloor, &mut p_bare, &PlayerInput::default());
        assert!(p_bare.vitals.burning_ticks > 0);

        let mut p_molten = Player::new([0.5, 0.0, 0.5]);
        p_molten.armor = [
            Some(crate::item::MOLTEN_HELMET),
            Some(crate::item::MOLTEN_BREASTPLATE),
            Some(crate::item::MOLTEN_GREAVES),
        ];
        p_molten.sync_accessories();
        tick_player(&HellstoneFloor, &mut p_molten, &PlayerInput::default());
        assert_eq!(p_molten.vitals.burning_ticks, 0);
    }

    #[test]
    fn molten_armor_refuses_and_clears_on_fire() {
        let lava_q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if y == 0 {
                    Some(Liquid::lava(255))
                } else {
                    None
                }
            },
        };

        // Bare player ignites via Player::ignite and via lava contact.
        let mut p_bare = Player::new([0.5, 0.0, 0.5]);
        p_bare.ignite();
        assert_eq!(p_bare.vitals.burn_ticks, BURN_TICKS);

        let mut p_bare_lava = Player::new([0.5, 0.0, 0.5]);
        tick_player(&lava_q, &mut p_bare_lava, &PlayerInput::default());
        assert!(p_bare_lava.vitals.burn_ticks > 0);

        // Full Molten set refuses Player::ignite, refuses lava ignition, and
        // extinguishes pre-existing burn_ticks on tick.
        let mut p_molten = Player::new([0.5, 0.0, 0.5]);
        p_molten.vitals.ignite();
        assert_eq!(p_molten.vitals.burn_ticks, BURN_TICKS);
        p_molten.armor = [
            Some(crate::item::MOLTEN_HELMET),
            Some(crate::item::MOLTEN_BREASTPLATE),
            Some(crate::item::MOLTEN_GREAVES),
        ];
        p_molten.sync_accessories();
        tick_player(&lava_q, &mut p_molten, &PlayerInput::default());
        assert_eq!(
            p_molten.vitals.burn_ticks, 0,
            "Molten set must extinguish existing burn and refuse lava On Fire"
        );
        p_molten.ignite();
        assert_eq!(
            p_molten.vitals.burn_ticks, 0,
            "Player::ignite must refuse On Fire while wearing Molten set"
        );
    }

    #[test]
    fn two_piece_molten_armor_grants_no_set_bonus() {
        struct HellstoneFloor;
        impl BlockQuery for HellstoneFloor {
            fn block(&self, _: i32, y: i32, _: i32) -> BlockId {
                if y < 0 { crate::block::HELLSTONE } else { AIR }
            }
        }

        let mut p = Player::new([0.5, 0.0, 0.5]);
        p.armor[0] = Some(crate::item::MOLTEN_HELMET);
        p.armor[1] = Some(crate::item::MOLTEN_BREASTPLATE);
        p.sync_accessories();
        assert!(!p.has_molten_set());
        assert!(!p.body.has_molten_set);
        assert!((p.melee_damage_mult() - 1.07).abs() < 1e-6);
        assert_eq!(p.melee_crit_bonus(), 7);
        assert_eq!(p.melee_speed_mult(), 1.0);
        tick_player(&HellstoneFloor, &mut p, &PlayerInput::default());
        assert!(p.vitals.burning_ticks > 0);
    }

    #[test]
    fn cobalt_shield_negates_damage_knockback() {
        let mut p = Player::new([0.0, 0.0, 0.0]);
        assert!(!p.no_knockback());
        assert_eq!(p.damage_knockback(5.0), 5.0);
        p.apply_knockback(5.0, [1.0, 0.0, 0.0]);
        assert_eq!(p.body.vel[0], 5.0);

        // Equip Cobalt Shield
        p.body.vel = [0.0, 0.0, 0.0];
        p.accessories[0] = Some(crate::item::COBALT_SHIELD);
        p.sync_accessories();

        assert!(p.no_knockback());
        assert!(p.body.no_knockback);
        assert_eq!(p.damage_knockback(5.0), 0.0);
        p.apply_knockback(5.0, [1.0, 0.0, 0.0]);
        assert_eq!(p.body.vel[0], 0.0);
        assert_eq!(p.defense(), 1);
    }

    #[test]
    fn m11_emblems_boost_class_damage_multipliers() {
        use crate::item::{
            MOLTEN_BREASTPLATE, MOLTEN_GREAVES, MOLTEN_HELMET, RANGER_EMBLEM, SORCERER_EMBLEM,
            WARRIOR_EMBLEM,
        };
        let mut p = Player::new([0.0; 3]);
        assert!((p.melee_damage_mult() - 1.0).abs() < 1e-6);
        assert!((p.ranged_damage_mult() - 1.0).abs() < 1e-6);
        assert!((p.magic_damage_mult() - 1.0).abs() < 1e-6);

        p.accessories[0] = Some(WARRIOR_EMBLEM);
        p.accessories[1] = Some(RANGER_EMBLEM);
        p.accessories[2] = Some(SORCERER_EMBLEM);
        assert!((p.melee_damage_mult() - 1.15).abs() < 1e-6);
        assert!((p.ranged_damage_mult() - 1.15).abs() < 1e-6);
        assert!((p.magic_damage_mult() - 1.15).abs() < 1e-6);

        // Stacks additively with Molten set bonus (+17% + 15% = 1.32)
        p.armor = [
            Some(MOLTEN_HELMET),
            Some(MOLTEN_BREASTPLATE),
            Some(MOLTEN_GREAVES),
        ];
        assert!((p.melee_damage_mult() - 1.32).abs() < 1e-6);
    }

    #[test]
    fn band_of_starpower_and_respawn_preserve_base_mana_max() {
        let mut p = Player::new([0.0, 0.0, 0.0]);
        p.vitals.life_max = 200;
        p.vitals.mana_max = 180;
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack {
            id: crate::item::BAND_OF_STARPOWER,
            count: 1,
            prefix: Prefix(0),
        });
        assert!(p.equip(&mut inv, 0));
        assert_eq!(p.vitals.mana_max, 180, "base mana_max must not mutate");
        assert_eq!(p.effective_mana_max(), 200);
        assert!(
            p.vitals.use_mana_crystal(),
            "10th Mana Crystal must work at 180 base mana"
        );
        assert_eq!(p.vitals.mana_max, 200);
        assert_eq!(p.effective_mana_max(), 220);

        let mut ev = Vec::new();
        p.kill(DeathCause::Unknown, &mut ev);
        p.respawn(&mut ev);
        assert_eq!(p.vitals.life, 200);
        assert_eq!(p.vitals.mana, 220);
    }

    #[test]
    fn metal_armor_set_bonuses_and_shadow_crit() {
        use crate::item::*;
        let mut p = Player::new([0.0, 0.0, 0.0]);
        p.armor = [Some(GOLD_HELMET), Some(GOLD_CHAINMAIL), Some(GOLD_GREAVES)];
        assert_eq!(
            p.defense(),
            item_data(GOLD_HELMET).defense
                + item_data(GOLD_CHAINMAIL).defense
                + item_data(GOLD_GREAVES).defense
                + 3
        );
        p.armor = [Some(SHADOW_HELMET), Some(SHADOW_SCALEMAIL), None];
        assert_eq!(p.melee_crit_bonus(), 10);
    }

    #[test]
    fn lava_and_shallow_water_do_not_drown_and_obsidian_skin_clears_fire() {
        let shallow_water = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if y == 1 {
                    Some(Liquid::water(10))
                } else {
                    None
                }
            },
        };
        let b = PlayerBody::new([0.5, 0.0, 0.5]);
        assert!(!head_submerged(&shallow_water, &b));

        let lava_head = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if y == 1 {
                    Some(Liquid::lava(255))
                } else {
                    None
                }
            },
        };
        assert!(!head_submerged(&lava_head, &b));

        let mut p = Player::new([0.5, 0.0, 0.5]);
        p.vitals.burn_ticks = 100;
        p.vitals.obsidian_skin_ticks = 60;
        p.ignite();
        tick_player(&lava_head, &mut p, &PlayerInput::default());
        assert_eq!(p.vitals.burn_ticks, 0);
    }

    #[test]
    fn duplicate_accessory_equip_refused() {
        let mut p = Player::new([0.0, 0.0, 0.0]);
        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack {
            id: crate::item::COBALT_SHIELD,
            count: 1,
            prefix: Prefix(0),
        });
        inv.slots[1] = Some(ItemStack {
            id: crate::item::COBALT_SHIELD,
            count: 1,
            prefix: Prefix(0),
        });
        assert!(p.equip(&mut inv, 0));
        assert!(!p.equip(&mut inv, 1));
    }

    #[test]
    fn accessory_prefixes_apply_stats_and_survive_equip_unequip() {
        use crate::item::*;
        let mut p = Player::new([0.0, 0.0, 0.0]);
        let mut inv = Inventory::empty();
        // 65=Warding (+4 def), 66=Arcane (+20 mana), 72=Menacing (+4% dmg), 68=Lucky (+4% crit),
        // 76=Quick (+4% move speed), 80=Violent (+4% melee speed)
        inv.slots[0] = Some(ItemStack {
            id: COBALT_SHIELD,
            count: 1,
            prefix: Prefix(65),
        });
        inv.slots[1] = Some(ItemStack {
            id: HERMES_BOOTS,
            count: 1,
            prefix: Prefix(66),
        });
        inv.slots[2] = Some(ItemStack {
            id: CLOUD_IN_A_BOTTLE,
            count: 1,
            prefix: Prefix(72),
        });
        inv.slots[3] = Some(ItemStack {
            id: LUCKY_HORSESHOE,
            count: 1,
            prefix: Prefix(68),
        });
        inv.slots[4] = Some(ItemStack {
            id: SHINY_RED_BALLOON,
            count: 1,
            prefix: Prefix(76),
        });

        for slot in 0..5 {
            assert!(p.equip(&mut inv, slot));
            assert!(inv.slots[slot].is_none());
        }
        assert_eq!(p.defense(), 1 + 4);
        assert_eq!(p.effective_mana_max(), 20 + 20);
        assert!((p.melee_damage_mult() - 1.04).abs() < 1e-6);
        assert!((p.ranged_damage_mult() - 1.04).abs() < 1e-6);
        assert!((p.magic_damage_mult() - 1.04).abs() < 1e-6);
        assert_eq!(p.accessory_crit_bonus(), 4);
        assert_eq!(p.melee_crit_bonus(), 4);
        assert!((p.body.move_speed_bonus - 0.04).abs() < 1e-6);

        // Equip a Violent (80) Band of Regeneration into full accessory array by swapping via unequip/equip
        assert!(p.unequip_accessory(&mut inv, 4));
        assert_eq!(
            inv.slots[0],
            Some(ItemStack {
                id: SHINY_RED_BALLOON,
                count: 1,
                prefix: Prefix(76),
            })
        );
        inv.slots[1] = Some(ItemStack {
            id: BAND_OF_REGENERATION,
            count: 1,
            prefix: Prefix(80),
        });
        assert!(p.equip(&mut inv, 1));
        assert!((p.body.melee_speed_bonus - 0.04).abs() < 1e-6);
        assert!((p.melee_speed_mult() - 0.96).abs() < 1e-6);
    }

    #[test]
    fn water_walking_supports_player_on_water_and_lava() {
        let water_q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if (0..=4).contains(&y) {
                    Some(Liquid::water(255))
                } else {
                    None
                }
            },
        };
        let mut b_walk = PlayerBody::new([0.5, 5.0, 0.5]);
        b_walk.water_walking = true;
        for _ in 0..30 {
            step_body(&water_q, &mut b_walk, &PlayerInput::default());
        }
        assert!(
            b_walk.pos[1] >= 4.99 && b_walk.on_ground,
            "water walking should stand on water surface, got y={} on_ground={}",
            b_walk.pos[1],
            b_walk.on_ground
        );

        let mut b_sink = PlayerBody::new([0.5, 5.0, 0.5]);
        for _ in 0..30 {
            step_body(&water_q, &mut b_sink, &PlayerInput::default());
        }
        assert!(
            b_sink.pos[1] < 4.5,
            "without water walking player should sink, got y={}",
            b_sink.pos[1]
        );

        let lava_q = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, y, _| {
                if (0..=4).contains(&y) {
                    Some(Liquid::lava(255))
                } else {
                    None
                }
            },
        };
        let mut b_lava = PlayerBody::new([0.5, 5.0, 0.5]);
        b_lava.water_walking = true;
        for _ in 0..30 {
            step_body(&lava_q, &mut b_lava, &PlayerInput::default());
        }
        assert!(
            b_lava.pos[1] >= 4.99 && b_lava.on_ground,
            "water walking should also support standing on lava surface, got y={}",
            b_lava.pos[1]
        );
    }

    #[test]
    fn ironskin_and_swiftness_buffs_apply() {
        let mut p = Player::new([0.0, 5.0, 0.0]);
        let base_def = p.defense();
        p.vitals.ironskin_ticks = crate::player::consts::IRONSKIN_TICKS;
        assert_eq!(p.defense(), base_def + 8);

        let flat = LiquidQ {
            block_fn: |_, y, _| if y < 0 { STONE } else { AIR },
            liquid_fn: |_, _, _| None,
        };
        let mut p_base = Player::new([0.0, 0.0, 0.0]);
        let mut p_swift = Player::new([0.0, 0.0, 0.0]);
        p_swift.vitals.swiftness_ticks = crate::player::consts::SWIFTNESS_TICKS;
        let forward = PlayerInput {
            forward: 1.0,
            ..Default::default()
        };
        for _ in 0..20 {
            let _ = tick_player(&flat, &mut p_base, &forward);
            let _ = tick_player(&flat, &mut p_swift, &forward);
        }
        let d_base = (p_base.body.pos[0].powi(2) + p_base.body.pos[2].powi(2)).sqrt();
        let d_swift = (p_swift.body.pos[0].powi(2) + p_swift.body.pos[2].powi(2)).sqrt();
        assert!(
            d_swift > d_base * 1.15,
            "swiftness should boost speed: base={d_base}, swift={d_swift}",
        );
    }
}
