//! The Terraria inventory screen, opened with `Esc`.
//!
//! Layout is authored at a 1080 px window height and scales with it: the 10x5 bag grid
//! (hotbar row on top) top-left, coin and ammo columns right of it, armor and greyed
//! accessories at the right edge, the craftable recipe list bottom-left and a Settings
//! button bottom-right. Every change is a request (`ClickSlot`, `Craft`); the server owns
//! the inventory and the stack on the cursor.

use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use terravibe_sim::BlockId;
use terravibe_sim::coin::{format_coins, total_copper};
use terravibe_sim::crafting::{RecipeId, craft, recipe, station_in_reach};
use terravibe_sim::inventory::{AMMO0, COIN0, Inventory, ItemStack, SLOTS, SortMode};
use terravibe_sim::item::{ArmorSlot, Prefix, display_name, item_data};
use terravibe_sim::net::ClientMsg;
use terravibe_sim::slots::{ClickKind, SlotRef};

use super::ExtraUiAssets;
use super::HudFonts;
use super::chest::{self, ChestPanel};
use super::crafting::{
    RowState, STATIONS, craft_feedback, filter_recipes, missing_ingredients, row_state,
    sorted_order, station_ok, title_text,
};
use super::hotbar::{
    HotbarAssets, ItemIconSource, ORIGIN, SLOT_PITCH, SLOT_SCALE, SLOT_TEX, icon_of,
};
use super::text::{HudText, StackText, measure};
use super::town::{TownPanel, sell_click};
use crate::audio::{SfxEvent, SfxKind};
use crate::net::{
    ClientVanity, CraftResults, LocalAccessories, LocalAccessoryPrefixes, LocalArmor, LocalHeld,
    LocalInventory, Net,
};
use crate::session::{AppState, GameMenu, GamePanel};
use crate::view::camera::set_grab;
use crate::view::chunks::ChunkStream;
use crate::view::player::{LoadedBlocks, LocalPlayer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GhostSilhouette {
    Coin(usize),
    Ammo,
    Trash,
    ArmorHead,
    ArmorBody,
    ArmorLegs,
    Accessory,
}

pub fn ghost_silhouette_for_cell(id: usize) -> Option<GhostSilhouette> {
    match id {
        n if (COIN0..AMMO0).contains(&n) => Some(GhostSilhouette::Coin(n - COIN0)),
        n if (AMMO0..SLOTS).contains(&n) => Some(GhostSilhouette::Ammo),
        n if n == ARMOR0 || n == VANITY_ARMOR0 => Some(GhostSilhouette::ArmorHead),
        n if n == ARMOR0 + 1 || n == VANITY_ARMOR0 + 1 => Some(GhostSilhouette::ArmorBody),
        n if n == ARMOR0 + 2 || n == VANITY_ARMOR0 + 2 => Some(GhostSilhouette::ArmorLegs),
        n if (ACC0..ACC0 + 5).contains(&n) || (VANITY_ACC0..VANITY_ACC0 + 5).contains(&n) => {
            Some(GhostSilhouette::Accessory)
        }
        n if n == TRASH_CELL => Some(GhostSilhouette::Trash),
        _ => None,
    }
}

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RulerEnabled(pub bool);

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildGridEnabled(pub bool);

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct RulerReadout(pub Option<([i32; 3], f32)>);

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct FavoriteSlots(pub [bool; BAG]);

impl Default for FavoriteSlots {
    fn default() -> Self {
        Self([false; BAG])
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EquipTab {
    #[default]
    Equip,
    Housing,
}

pub const VALUABLE_TRASH_THRESHOLD_COPPER: u32 = 100;

pub fn trash_slot_at(cursor: [f32; 2], win: [f32; 2]) -> bool {
    let [x, y, s] = cell_rect(TRASH_CELL, win);
    inside(cursor, x, y, s, s)
}

pub fn sort_btn_at(cursor: [f32; 2], win: [f32; 2]) -> bool {
    let s = scale(win);
    let sx = (special_x() + STEP + 36.0) * s;
    let sy = (GRID[1] + 4.0 * STEP) * s;
    inside(cursor, sx, sy, 32.0 * s, 32.0 * s)
}

#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryOpen(pub bool);

/// Window height the layout constants below are authored for.
pub const REF_H: f32 = 1080.0;
/// Slot texture size, pitch and scale shared with the hotbar (`Main.DrawInventory`).
const SLOT: f32 = SLOT_TEX;
const PITCH: f32 = SLOT_PITCH;
const GRID: [f32; 2] = [ORIGIN, ORIGIN];
/// On-screen slot size (52 * 0.85) and distance between slot origins (56 * 0.85).
const CELL: f32 = SLOT * SLOT_SCALE;
const STEP: f32 = PITCH * SLOT_SCALE;
const COLS: usize = 10;
/// Bag slots on screen (hotbar row included).
const BAG: usize = 50;
/// Reference-px left edge of the bag grid (and of panels laid out below it).
pub const BAG_LEFT: f32 = GRID[0];
/// Reference-px bottom edge of the 5-row bag grid.
pub const BAG_BOTTOM: f32 = GRID[1] + (BAG / COLS) as f32 * STEP;
/// Reference-px top and right margin for the armor/accessory column (below Life/Mana).
const EQUIP_TOP: f32 = 196.0;
const EQUIP_RIGHT: f32 = 56.0;
/// Clickable cells: 58 inventory slots, 3 armor, 5 accessories, 3 vanity armor, 5 vanity accessories, 1 trash.
const ARMOR0: usize = SLOTS;
const ACC0: usize = SLOTS + 3;
pub const VANITY_ARMOR0: usize = ACC0 + 5;
pub const VANITY_ACC0: usize = VANITY_ARMOR0 + 3;
pub const TRASH_CELL: usize = VANITY_ACC0 + 5;
pub const CELLS: usize = TRASH_CELL + 1;

const HOUSING_NPCS: [terravibe_sim::town_npc::TownKind; 3] = [
    terravibe_sim::town_npc::TownKind::Guide,
    terravibe_sim::town_npc::TownKind::Merchant,
    terravibe_sim::town_npc::TownKind::ArmsDealer,
];

pub const CAROUSEL_POS: [f32; 2] = [40.0, 420.0];
pub const CAROUSEL_SLOT_SIZE: f32 = 44.0;
pub const CAROUSEL_PITCH: f32 = 48.0;
pub const CAROUSEL_ROWS: usize = 6;
pub const HAMMER_COLS: usize = 7;
pub const HAMMER_ROWS: usize = 4;
pub const HAMMER_SLOTS: usize = HAMMER_COLS * HAMMER_ROWS;
pub const HAMMER_BTN_SIZE: [f32; 2] = [32.0, 32.0];

const ROWS: usize = 9;
const DETAIL_X: f32 = CAROUSEL_POS[0] + CAROUSEL_SLOT_SIZE + 24.0;
const SETTINGS_SIZE: [f32; 2] = [200.0, 56.0];
const FEEDBACK_SECS: f32 = 3.0;
const MAX_CRAFT: u16 = 999;

pub(crate) fn scale(win: [f32; 2]) -> f32 {
    win[1] / REF_H
}

pub fn carousel_slot_rect(i: usize, win: [f32; 2]) -> [f32; 4] {
    let s = scale(win);
    [
        CAROUSEL_POS[0] * s,
        (CAROUSEL_POS[1] + i as f32 * CAROUSEL_PITCH) * s,
        CAROUSEL_SLOT_SIZE * s,
        CAROUSEL_SLOT_SIZE * s,
    ]
}

pub fn grid_slot_rect(i: usize, win: [f32; 2]) -> [f32; 4] {
    let s = scale(win);
    let col = i % HAMMER_COLS;
    let row = i / HAMMER_COLS;
    [
        (CAROUSEL_POS[0] + col as f32 * CAROUSEL_PITCH) * s,
        (CAROUSEL_POS[1] + row as f32 * CAROUSEL_PITCH) * s,
        CAROUSEL_SLOT_SIZE * s,
        CAROUSEL_SLOT_SIZE * s,
    ]
}

pub fn hammer_button_rect(win: [f32; 2]) -> [f32; 4] {
    let s = scale(win);
    [
        CAROUSEL_POS[0] * s,
        (CAROUSEL_POS[1] - 36.0) * s,
        HAMMER_BTN_SIZE[0] * s,
        HAMMER_BTN_SIZE[1] * s,
    ]
}

pub fn hammer_button_at(cursor: [f32; 2], win: [f32; 2]) -> bool {
    let [x, y, w, h] = hammer_button_rect(win);
    inside(cursor, x, y, w, h)
}

pub fn carousel_slot_at(cursor: [f32; 2], win: [f32; 2]) -> Option<usize> {
    (0..CAROUSEL_ROWS).find(|&i| {
        let [x, y, w, h] = carousel_slot_rect(i, win);
        inside(cursor, x, y, w, h)
    })
}

pub fn grid_slot_at(cursor: [f32; 2], win: [f32; 2]) -> Option<usize> {
    (0..HAMMER_SLOTS).find(|&i| {
        let [x, y, w, h] = grid_slot_rect(i, win);
        inside(cursor, x, y, w, h)
    })
}

/// Which slot a cell shows; trash has no server slot to click via `ClickSlot`.
fn cell_ref(id: usize) -> Option<SlotRef> {
    match id {
        0..SLOTS => Some(SlotRef::Inv(id as u8)),
        ARMOR0 => Some(SlotRef::Armor(ArmorSlot::Head)),
        n if n == ARMOR0 + 1 => Some(SlotRef::Armor(ArmorSlot::Body)),
        n if n == ARMOR0 + 2 => Some(SlotRef::Armor(ArmorSlot::Legs)),
        n if (ACC0..ACC0 + 5).contains(&n) => Some(SlotRef::Accessory((n - ACC0) as u8)),
        n if n == VANITY_ARMOR0 => Some(SlotRef::VanityArmor(ArmorSlot::Head)),
        n if n == VANITY_ARMOR0 + 1 => Some(SlotRef::VanityArmor(ArmorSlot::Body)),
        n if n == VANITY_ARMOR0 + 2 => Some(SlotRef::VanityArmor(ArmorSlot::Legs)),
        n if (VANITY_ACC0..VANITY_ACC0 + 5).contains(&n) => {
            Some(SlotRef::VanityAccessory((n - VANITY_ACC0) as u8))
        }
        _ => None,
    }
}

/// Reference-px x of the coin/ammo columns and the trash slot, right of the bag grid.
fn special_x() -> f32 {
    GRID[0] + COLS as f32 * STEP + 30.0
}

/// `[x, y, size]` of cell `id` in window px.
pub fn cell_rect(id: usize, win: [f32; 2]) -> [f32; 3] {
    let s = scale(win);
    let (x, y) = match id {
        0..BAG => (
            GRID[0] + (id % COLS) as f32 * STEP,
            GRID[1] + (id / COLS) as f32 * STEP,
        ),
        BAG..AMMO0 => (special_x(), GRID[1] + (id - COIN0) as f32 * STEP),
        AMMO0..SLOTS => (special_x() + STEP, GRID[1] + (id - AMMO0) as f32 * STEP),
        n if n == TRASH_CELL => (special_x(), GRID[1] + 4.0 * STEP),
        n if (ARMOR0..VANITY_ARMOR0).contains(&n) => (
            win[0] / s - EQUIP_RIGHT - CELL,
            EQUIP_TOP + (id - ARMOR0) as f32 * STEP,
        ),
        _ => (
            win[0] / s - EQUIP_RIGHT - CELL - STEP,
            EQUIP_TOP + (id - VANITY_ARMOR0) as f32 * STEP,
        ),
    };
    [x * s, y * s, CELL * s]
}

/// Reference-px width of the right-side Equipment / Housing card (>= 220px for unclipped tabs & status).
pub const EQUIP_CARD_W: f32 = 236.0;

pub fn equip_card_rect(win: [f32; 2]) -> [f32; 4] {
    let s = scale(win);
    let right_margin = EQUIP_RIGHT - 12.0;
    [
        win[0] - (right_margin + EQUIP_CARD_W) * s,
        (EQUIP_TOP - 8.0) * s,
        EQUIP_CARD_W * s,
        (8.0 * STEP + 14.0) * s,
    ]
}

pub fn equip_tab_rect(tab: usize, win: [f32; 2]) -> [f32; 4] {
    let s = scale(win);
    let [cx, _, cw, _] = equip_card_rect(win);
    let gap = 6.0 * s;
    let w = (cw - gap) / 2.0;
    [
        cx + tab as f32 * (w + gap),
        (EQUIP_TOP - 38.0) * s,
        w,
        26.0 * s,
    ]
}

pub fn housing_query_rect(win: [f32; 2]) -> [f32; 4] {
    let s = scale(win);
    let [cx, cy, cw, _] = equip_card_rect(win);
    [cx + 12.0 * s, cy + 12.0 * s, cw - 24.0 * s, 32.0 * s]
}

pub fn housing_status_rect(win: [f32; 2]) -> [f32; 4] {
    let s = scale(win);
    let [cx, cy, cw, _] = equip_card_rect(win);
    [cx + 12.0 * s, cy + 52.0 * s, cw - 24.0 * s, 96.0 * s]
}

pub fn housing_npc_rect(idx: usize, win: [f32; 2]) -> [f32; 4] {
    let s = scale(win);
    let [cx, cy, cw, _] = equip_card_rect(win);
    [
        cx + 12.0 * s,
        cy + (160.0 + idx as f32 * 40.0) * s,
        cw - 24.0 * s,
        34.0 * s,
    ]
}

/// Word-wraps housing status text to at most `max_chars` per line so it never clips the card edge.
pub fn wrap_housing_text(text: &str, max_chars: usize) -> String {
    let mut lines = Vec::new();
    for raw_line in text.lines() {
        let mut cur = String::new();
        for word in raw_line.split_whitespace() {
            if cur.is_empty() {
                cur.push_str(word);
            } else if cur.chars().count() + 1 + word.chars().count() <= max_chars {
                cur.push(' ');
                cur.push_str(word);
            } else {
                lines.push(cur);
                cur = word.to_string();
            }
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
    }
    lines.join("\n")
}

fn housing_npc_swatch_color(npc: terravibe_sim::town_npc::TownKind) -> Color {
    match npc {
        terravibe_sim::town_npc::TownKind::Guide => Color::srgb(0.32, 0.68, 0.42),
        terravibe_sim::town_npc::TownKind::Merchant => Color::srgb(0.28, 0.52, 0.88),
        terravibe_sim::town_npc::TownKind::ArmsDealer => Color::srgb(0.76, 0.30, 0.28),
    }
}

pub fn ruler_btn_rect(win: [f32; 2]) -> [f32; 4] {
    let s = scale(win);
    [GRID[0] * s, (BAG_BOTTOM + 12.0) * s, 96.0 * s, 28.0 * s]
}

pub fn build_grid_btn_rect(win: [f32; 2]) -> [f32; 4] {
    let s = scale(win);
    [
        (GRID[0] + 104.0) * s,
        (BAG_BOTTOM + 12.0) * s,
        96.0 * s,
        28.0 * s,
    ]
}

fn inside(cursor: [f32; 2], x: f32, y: f32, w: f32, h: f32) -> bool {
    (x..x + w).contains(&cursor[0]) && (y..y + h).contains(&cursor[1])
}

/// Slot under `cursor` (window px) for a window of size `win`.
pub fn slot_at(cursor: [f32; 2], win: [f32; 2]) -> Option<SlotRef> {
    (0..CELLS).find_map(|id| {
        let at = cell_ref(id)?;
        let [x, y, s] = cell_rect(id, win);
        inside(cursor, x, y, s, s).then_some(at)
    })
}

/// `[x, y, w, h]` of recipe row `i` (0 = first visible row).
#[allow(dead_code)]
pub fn row_rect(i: usize, win: [f32; 2]) -> [f32; 4] {
    carousel_slot_rect(i, win)
}

/// Visible recipe row under `cursor`; add the list scroll for the list index.
#[allow(dead_code)]
pub fn recipe_at(cursor: [f32; 2], win: [f32; 2]) -> Option<usize> {
    if let Some(i) = carousel_slot_at(cursor, win) {
        return Some(i);
    }
    let s = scale(win);
    (0..CAROUSEL_ROWS).find(|&i| {
        let [x, y, _, _] = carousel_slot_rect(i, win);
        inside(cursor, x, y, 380.0 * s, CAROUSEL_PITCH * s)
    })
}

/// `[x, y, w, h]` of the Settings button, bottom-right.
pub fn settings_rect(win: [f32; 2]) -> [f32; 4] {
    let s = scale(win);
    [
        win[0] - (40.0 + SETTINGS_SIZE[0]) * s,
        (REF_H - 40.0 - SETTINGS_SIZE[1]) * s,
        SETTINGS_SIZE[0] * s,
        SETTINGS_SIZE[1] * s,
    ]
}

pub fn settings_at(cursor: [f32; 2], win: [f32; 2]) -> bool {
    let [x, y, w, h] = settings_rect(win);
    inside(cursor, x, y, w, h)
}

/// Recipes the player can make right now, craftable-first order (Terraria lists only these).
pub fn craftable_list(inv: &Inventory, near: &[BlockId]) -> Vec<RecipeId> {
    sorted_order(inv, near)
        .into_iter()
        .filter(|&id| {
            recipe(id)
                .is_some_and(|r| row_state(inv, id, station_ok(r, near)) == RowState::Craftable)
        })
        .collect()
}

/// How many times `id` can be crafted in a row (Shift-click), capped; 0 when none.
pub fn max_crafts(inv: &Inventory, id: RecipeId) -> u16 {
    let mut inv = inv.clone();
    let mut n = 0;
    while n < MAX_CRAFT && craft(&mut inv, id, 1).is_ok() {
        n += 1;
    }
    n
}

pub fn detail_text(id: RecipeId, inv: Option<&Inventory>) -> String {
    let Some(r) = recipe(id) else {
        return String::new();
    };
    let mut s = String::from("Ingredients:");
    let missing = inv.map(|i| missing_ingredients(r, i)).unwrap_or_default();
    for i in r.ingredients {
        let name = display_name(i.item, Prefix(0));
        if let Some(&(_, shortfall)) = missing.iter().find(|(item, _)| *item == i.item) {
            s.push_str(&format!("\n  {} {} (missing {})", i.count, name, shortfall));
        } else {
            s.push_str(&format!("\n  {} {}", i.count, name));
        }
    }
    let st = terravibe_sim::crafting::recipe_station_name(id).unwrap_or("By Hand");
    s.push_str(&format!("\nStation: {st}"));
    s
}

/// View state of the screen.
#[derive(Resource, Default)]
pub struct InventoryUi {
    /// First visible index into `list`.
    pub scroll: usize,
    /// Index into `list` last hovered or clicked.
    pub selected: usize,
    pub list: Vec<RecipeId>,
    /// Stations within reach (client probe; the server decides).
    pub near: Vec<BlockId>,
    pub feedback: String,
    pub feedback_ticks: f32,
    pub trash: Option<ItemStack>,
    pub pending_trash_confirm: Option<ItemStack>,
    pub sort_mode: SortMode,
    pub hammer_grid: bool,
    pub search_query: String,
    pub equip_tab: EquipTab,
}

/// Item-icon cache and the on-demand content source, bundled to keep `update_view`'s
/// parameter list inside Bevy's system-parameter limit.
#[derive(SystemParam)]
pub struct IconAssets<'w> {
    pub assets: Option<ResMut<'w, HotbarAssets>>,
    pub source: Option<Res<'w, ItemIconSource>>,
    pub images: ResMut<'w, Assets<Image>>,
}

/// Esc routing for the in-game menu handler.
#[derive(SystemParam)]
pub struct EscPanels<'w, 's> {
    inv: Option<ResMut<'w, InventoryOpen>>,
    chest: Option<ResMut<'w, ChestPanel>>,
    town: Option<ResMut<'w, TownPanel>>,
    player: Option<Res<'w, LocalPlayer>>,
    net: Option<Res<'w, Net>>,
    chest_root: Query<'w, 's, &'static mut Visibility, With<chest::PanelRoot>>,
}

impl EscPanels<'_, '_> {
    /// Esc with the game menu closed. Returns whether it was consumed; `false` means the
    /// caller opens the Game Menu (player dead, or no inventory screen in this app).
    pub fn esc(&mut self, opts: &mut CursorOptions) -> bool {
        let Some(inv) = self.inv.as_mut() else {
            return false;
        };
        if self.player.as_ref().is_some_and(|p| p.0.dead) {
            // The Game Menu opens instead; do not leave the inventory open behind it.
            if inv.0 {
                inv.0 = false;
                if let Some(net) = &self.net {
                    let _ = net.send(ClientMsg::CloseInventory);
                }
            }
            return false;
        }
        let chest_open = self.chest.as_ref().is_some_and(|c| c.open.is_some());
        let town_open = self.town.as_ref().is_some_and(|t| t.is_open());
        if !(inv.0 || chest_open || town_open) {
            inv.0 = true;
            set_grab(opts, false);
            return true;
        }
        inv.0 = false;
        if let Some(c) = self.chest.as_mut() {
            if c.open.take().is_some() {
                c.slots.clear();
                if let Some(net) = &self.net {
                    let _ = net.send(ClientMsg::CloseChest);
                }
            }
            for mut v in &mut self.chest_root {
                *v = Visibility::Hidden;
            }
        }
        if let Some(t) = self.town.as_mut() {
            **t = TownPanel::default();
        }
        if let Some(net) = &self.net {
            let _ = net.send(ClientMsg::CloseInventory);
        }
        set_grab(opts, true);
        true
    }
}

#[derive(Component, Clone, Copy)]
pub enum Part {
    Back(usize),
    Ghost(usize),
    Icon(usize),
    Count(usize),
    FavStar(usize),
    InventoryHeader,
    CraftingHeader,
    RecipeName,
    CarouselBack(usize),
    CarouselIcon(usize),
    HammerBtn,
    HammerBtnIcon,
    RulerBtn,
    RulerText,
    BuildGridBtn,
    BuildGridText,
    RulerHudText,
    QuickStackBtn,
    SortBtn,
    DefenseIcon,
    DefenseText,
    EquipTab(usize),
    EquipTabText(usize),
    HousingQueryBtn,
    HousingQueryText,
    HousingStatusBox,
    HousingStatusText,
    HousingNpcBtn(usize),
    HousingNpcSwatch(usize),
    HousingNpcText(usize),
    HousingNpcStatus(usize),
    RowBack(usize),
    RowIcon(usize),
    RowText(usize),
    SettingsBack,
    SettingsText,
    Detail,
    Feedback,
    Panel,
    InvCard,
    CoinsCard,
    CraftCard,
    EquipCard,
    HeldIcon,
    HeldCount,
}

#[derive(Component)]
pub struct InvRoot;

const ROW_BG: Color = super::style::BTN_BG;
const ROW_SEL: Color = super::style::BTN_BG_ACTIVE;

fn abs() -> Node {
    Node {
        position_type: PositionType::Absolute,
        ..default()
    }
}

fn label() -> (HudText, Node) {
    (
        HudText {
            text: String::new(),
            death: false,
        },
        abs(),
    )
}

pub fn setup(mut commands: Commands, assets: Option<Res<HotbarAssets>>) {
    commands.insert_resource(InventoryOpen(false));
    commands.insert_resource(InventoryUi::default());
    commands.insert_resource(FavoriteSlots::default());
    commands.insert_resource(RulerEnabled::default());
    commands.insert_resource(BuildGridEnabled::default());
    commands.insert_resource(RulerReadout::default());
    let Some(assets) = assets else { return };
    let back = assets.back.clone();
    commands
        .spawn((
            InvRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..abs()
            },
            BackgroundColor(Color::NONE), // Authentic semi-translucent view: 3D world fully visible!
            GlobalZIndex(500),
            Visibility::Hidden,
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|p| {
            p.spawn((Part::Panel, abs(), BackgroundColor(Color::NONE)));
            for card in [
                Part::InvCard,
                Part::CoinsCard,
                Part::CraftCard,
                Part::EquipCard,
            ] {
                p.spawn((
                    card,
                    Node {
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_PANEL)),
                        ..abs()
                    },
                    BackgroundColor(super::style::PANEL_BG),
                    BorderColor::all(super::style::PANEL_BORDER),
                ));
            }
            p.spawn((Part::InventoryHeader, label()));
            p.spawn((Part::CraftingHeader, label()));
            p.spawn((Part::RecipeName, label()));
            p.spawn((
                Part::HammerBtn,
                Node {
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                    ..abs()
                },
                BackgroundColor(super::style::BTN_BG),
                BorderColor::all(super::style::PANEL_BORDER),
            ));
            p.spawn((
                Part::HammerBtnIcon,
                abs(),
                ImageNode::new(back.clone()),
                Visibility::Hidden,
            ));
            p.spawn((
                Part::QuickStackBtn,
                abs(),
                ImageNode::new(back.clone()),
                Visibility::Hidden,
            ));
            p.spawn((
                Part::SortBtn,
                abs(),
                ImageNode::new(back.clone()),
                Visibility::Hidden,
            ));
            p.spawn((
                Part::DefenseIcon,
                abs(),
                ImageNode::new(back.clone()),
                Visibility::Hidden,
            ));
            p.spawn((Part::DefenseText, label()));

            for tab in 0..2 {
                p.spawn((
                    Part::EquipTab(tab),
                    Node {
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                        ..abs()
                    },
                    BackgroundColor(super::style::BTN_BG),
                    BorderColor::all(super::style::PANEL_BORDER),
                ));
                p.spawn((Part::EquipTabText(tab), label()));
            }
            p.spawn((
                Part::RulerBtn,
                Node {
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                    ..abs()
                },
                BackgroundColor(super::style::BTN_BG),
                BorderColor::all(super::style::PANEL_BORDER),
            ));
            p.spawn((Part::RulerText, label()));
            p.spawn((
                Part::BuildGridBtn,
                Node {
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                    ..abs()
                },
                BackgroundColor(super::style::BTN_BG),
                BorderColor::all(super::style::PANEL_BORDER),
            ));
            p.spawn((Part::BuildGridText, label()));
            p.spawn((
                Part::HousingQueryBtn,
                Node {
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                    ..abs()
                },
                BackgroundColor(super::style::BTN_BG),
                BorderColor::all(super::style::PANEL_BORDER),
            ));
            p.spawn((Part::HousingQueryText, label()));
            p.spawn((
                Part::HousingStatusBox,
                Node {
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                    ..abs()
                },
                BackgroundColor(super::style::BAR_BG),
                BorderColor::all(super::style::PANEL_BORDER),
            ));
            p.spawn((
                Part::HousingStatusText,
                label(),
                super::text::HudTextColor(Color::WHITE),
            ));
            for i in 0..HOUSING_NPCS.len() {
                p.spawn((
                    Part::HousingNpcBtn(i),
                    Node {
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                        ..abs()
                    },
                    BackgroundColor(super::style::BTN_BG),
                    BorderColor::all(super::style::PANEL_BORDER),
                ));
                p.spawn((
                    Part::HousingNpcSwatch(i),
                    Node {
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..abs()
                    },
                    BackgroundColor(housing_npc_swatch_color(HOUSING_NPCS[i])),
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.35)),
                ));
                p.spawn((Part::HousingNpcText(i), label()));
                p.spawn((
                    Part::HousingNpcStatus(i),
                    label(),
                    super::text::HudTextColor(super::style::GOLD_TEXT),
                ));
            }

            for id in 0..CELLS {
                p.spawn((Part::Back(id), abs(), ImageNode::new(back.clone())));
                p.spawn((
                    Part::Ghost(id),
                    abs(),
                    ImageNode::new(back.clone()),
                    Visibility::Hidden,
                ));
                p.spawn((
                    Part::Icon(id),
                    abs(),
                    ImageNode::new(back.clone()),
                    Visibility::Hidden,
                ));
                p.spawn((Part::Count(id), StackText, label()));
                if id < BAG {
                    p.spawn((
                        Part::FavStar(id),
                        StackText,
                        label(),
                        super::text::HudTextColor(Color::srgb(1.0, 0.85, 0.25)),
                    ));
                }
            }

            for i in 0..HAMMER_SLOTS.max(ROWS) {
                p.spawn((Part::CarouselBack(i), abs(), ImageNode::new(back.clone())));
                p.spawn((
                    Part::CarouselIcon(i),
                    abs(),
                    ImageNode::new(back.clone()),
                    Visibility::Hidden,
                ));
            }

            for i in 0..ROWS {
                p.spawn((
                    Part::RowBack(i),
                    Node {
                        border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BAR)),
                        ..abs()
                    },
                    BackgroundColor(Color::NONE),
                ));
                p.spawn((
                    Part::RowIcon(i),
                    abs(),
                    ImageNode::new(back.clone()),
                    Visibility::Hidden,
                ));
                p.spawn((Part::RowText(i), label()));
            }
            p.spawn((
                Part::SettingsBack,
                Node {
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                    ..abs()
                },
                BackgroundColor(ROW_BG),
                BorderColor::all(super::style::GOLD_BORDER),
            ));
            p.spawn((Part::SettingsText, label()));
            p.spawn((Part::Detail, label()));
            p.spawn((Part::Feedback, label()));
        });
    for part in [Part::HeldIcon, Part::HeldCount] {
        let mut e = commands.spawn((
            part,
            abs(),
            GlobalZIndex(600),
            Visibility::Hidden,
            DespawnOnExit(AppState::InGame),
        ));
        match part {
            Part::HeldIcon => e.insert(ImageNode::new(back.clone())),
            _ => e.insert((StackText, label().0)),
        };
    }
    commands.spawn((
        Part::RulerHudText,
        label(),
        super::text::HudTextColor(Color::srgb(1.0, 0.92, 0.45)),
        GlobalZIndex(120),
        Visibility::Hidden,
        DespawnOnExit(AppState::InGame),
    ));
}

fn window_size(w: &Window) -> [f32; 2] {
    [w.width(), w.height()]
}

fn cursor(w: &Window) -> Option<[f32; 2]> {
    w.cursor_position().map(|c| [c.x, c.y])
}

fn shift(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
}

fn alt(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight)
}

#[derive(SystemParam)]
pub struct ClickExtras<'w> {
    favorites: Option<ResMut<'w, FavoriteSlots>>,
    ruler: Option<ResMut<'w, RulerEnabled>>,
    build_grid: Option<ResMut<'w, BuildGridEnabled>>,
    player: Option<Res<'w, LocalPlayer>>,
    stream: Option<Res<'w, ChunkStream>>,
    chest: Option<Res<'w, ChestPanel>>,
    town: Option<ResMut<'w, TownPanel>>,
    sfx: Option<ResMut<'w, Messages<SfxEvent>>>,
}

/// Station probe and craftable list, refreshed while the screen is open.
#[allow(clippy::too_many_arguments)]
pub fn refresh_list(
    open: Res<InventoryOpen>,
    inv: Option<Res<LocalInventory>>,
    player: Option<Res<LocalPlayer>>,
    stream: Option<Res<ChunkStream>>,
    time: Res<Time>,
    mut results: MessageReader<CraftResults>,
    mut ui: ResMut<InventoryUi>,
    mut sfx: Option<ResMut<Messages<SfxEvent>>>,
) {
    for r in results.read() {
        ui.feedback = craft_feedback(r.recipe, r.crafted, r.error);
        ui.feedback_ticks = FEEDBACK_SECS;
        if r.crafted > 0
            && r.error.is_none()
            && let Some(ref mut bus) = sfx
        {
            bus.write(SfxEvent(SfxKind::Craft));
        }
    }
    if ui.feedback_ticks > 0.0 {
        ui.feedback_ticks -= time.delta_secs();
        if ui.feedback_ticks <= 0.0 {
            ui.feedback.clear();
        }
    }
    if !open.0 {
        return;
    }
    let (Some(inv), Some(player), Some(stream)) = (inv, player, stream) else {
        return;
    };
    let loaded = LoadedBlocks(stream.generated());
    let near: Vec<BlockId> = STATIONS
        .into_iter()
        .filter(|&st| station_in_reach(&loaded, player.0.body.pos, st))
        .collect();
    let list = if ui.search_query.trim().is_empty() {
        craftable_list(&inv.0, &near)
    } else {
        let all = sorted_order(&inv.0, &near);
        let indices: Vec<usize> = all.iter().map(|id| id.0 as usize).collect();
        let filtered = filter_recipes(&indices, &ui.search_query);
        filtered.into_iter().map(|i| RecipeId(i as u16)).collect()
    };
    if ui.near != near {
        ui.near = near;
    }
    if ui.list != list {
        ui.list = list;
    }
    ui.selected = ui.selected.min(ui.list.len().saturating_sub(1));
    ui.scroll = ui.scroll.min(ui.list.len().saturating_sub(ROWS));
}

/// Mouse input on the open screen: slot clicks, recipe clicks, the Settings button.
#[allow(clippy::too_many_arguments)]
pub fn click_input(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut opts: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut open: ResMut<InventoryOpen>,
    mut ui: ResMut<InventoryUi>,
    mut menu: ResMut<GameMenu>,
    mut wheel: MessageReader<MouseWheel>,
    inv: Option<Res<LocalInventory>>,
    held: Option<ResMut<LocalHeld>>,
    net: Option<Res<Net>>,
    keyboard_events: Option<MessageReader<KeyboardInput>>,
    cam: Option<Single<&crate::view::camera::FlyCamera>>,
    mut extras: ClickExtras,
) {
    if !open.0 {
        wheel.clear();
        return;
    }
    let chest_open = extras.chest.as_ref().is_some_and(|c| c.open.is_some());
    let shop_open = extras.town.as_ref().is_some_and(|t| t.shop_open());
    // The chest grid or shop panel covers the recipe list while it is up.
    let panels_open = chest_open || shop_open;

    if !panels_open {
        if let Some(mut ke) = keyboard_events {
            for ev in ke.read() {
                if !ev.state.is_pressed() {
                    continue;
                }
                if ev.key_code == KeyCode::Backspace || ev.logical_key == Key::Backspace {
                    ui.search_query.pop();
                } else if let Key::Character(ref s) = ev.logical_key {
                    for c in s.chars() {
                        if (c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_')
                            && ui.search_query.len() < 32
                        {
                            ui.search_query.push(c);
                        }
                    }
                }
            }
        } else if keys.just_pressed(KeyCode::Backspace) {
            ui.search_query.pop();
        }
    } else if let Some(mut ke) = keyboard_events {
        ke.clear();
    }
    let win = window_size(&window);
    let s = scale(win);
    let Some(c) = cursor(&window) else { return };

    if !panels_open {
        if ui.hammer_grid {
            if let Some(i) = grid_slot_at(c, win) {
                if i < ui.list.len() {
                    ui.selected = i;
                }
            }
        } else if let Some(i) = carousel_slot_at(c, win) {
            let total = ui.list.len();
            if i + ui.scroll < total {
                ui.selected = i + ui.scroll;
            }
        }
    }

    let mut delta: isize = 0;
    for e in wheel.read() {
        delta -= e.y.signum() as isize;
    }
    if delta != 0 && !panels_open && carousel_slot_at(c, win).is_some() {
        let max = ui.list.len().saturating_sub(CAROUSEL_ROWS) as isize;
        ui.scroll = (ui.scroll as isize + delta).clamp(0, max.max(0)) as usize;
    }

    let left = mouse.just_pressed(MouseButton::Left);
    let right = mouse.just_pressed(MouseButton::Right);
    if !(left || right) {
        return;
    }

    if left && settings_at(c, win) {
        open.0 = false;
        if let Some(net) = &net {
            let _ = net.send(ClientMsg::CloseInventory);
        }
        menu.panel = Some(GamePanel::Root);
        set_grab(&mut opts, false);
        return;
    }

    if left && hammer_button_at(c, win) {
        ui.hammer_grid = !ui.hammer_grid;
        if let Some(ref mut bus) = extras.sfx {
            bus.write(SfxEvent(SfxKind::UiClick));
        }
        return;
    }

    let [rx, ry, rw, rh] = ruler_btn_rect(win);
    if left && inside(c, rx, ry, rw, rh) {
        if let Some(ref mut r) = extras.ruler {
            r.0 = !r.0;
        }
        if let Some(ref mut bus) = extras.sfx {
            bus.write(SfxEvent(SfxKind::UiClick));
        }
        return;
    }

    let [gx, gy, gw, gh] = build_grid_btn_rect(win);
    if left && inside(c, gx, gy, gw, gh) {
        if let Some(ref mut g) = extras.build_grid {
            g.0 = !g.0;
        }
        if let Some(ref mut bus) = extras.sfx {
            bus.write(SfxEvent(SfxKind::UiClick));
        }
        return;
    }

    for tab in 0..2 {
        let [tx, ty, tw, th] = equip_tab_rect(tab, win);
        if left && inside(c, tx, ty, tw, th) {
            ui.equip_tab = if tab == 0 {
                EquipTab::Equip
            } else {
                EquipTab::Housing
            };
            if let Some(ref mut bus) = extras.sfx {
                bus.write(SfxEvent(SfxKind::UiClick));
            }
            return;
        }
    }

    if left && ui.equip_tab == EquipTab::Housing {
        let [qx, qy, qw, qh] = housing_query_rect(win);
        if inside(c, qx, qy, qw, qh) {
            if let (Some(player), Some(town), Some(net)) = (
                extras.player.as_deref(),
                extras.town.as_deref_mut(),
                net.as_deref(),
            ) {
                crate::hud::town::query_housing_at_player(
                    player,
                    extras.stream.as_deref(),
                    town,
                    net,
                );
            }
            if let Some(ref mut bus) = extras.sfx {
                bus.write(SfxEvent(SfxKind::UiClick));
            }
            return;
        }
        for (i, &npc) in HOUSING_NPCS.iter().enumerate() {
            let [nx, ny, nw, nh] = housing_npc_rect(i, win);
            if inside(c, nx, ny, nw, nh) {
                if let Some(ref mut t) = extras.town {
                    t.selected_npc = npc;
                    let pos = t.housing_query_pos.or_else(|| {
                        extras.player.as_ref().map(|p| {
                            [
                                p.0.body.pos[0].floor() as i32,
                                p.0.body.pos[1].floor() as i32,
                                p.0.body.pos[2].floor() as i32,
                            ]
                        })
                    });
                    if let (Some(pos), Some(net)) = (pos, net.as_deref()) {
                        let _ = net.send(ClientMsg::AssignHousing { npc, pos });
                    }
                }
                if let Some(ref mut bus) = extras.sfx {
                    bus.write(SfxEvent(SfxKind::UiClick));
                }
                return;
            }
        }
    }

    // Quick-stack button beside trash slot
    let quick_stack_rect = [
        (special_x() + STEP) * s,
        (GRID[1] + 4.0 * STEP) * s,
        32.0 * s,
        32.0 * s,
    ];
    if left
        && inside(
            c,
            quick_stack_rect[0],
            quick_stack_rect[1],
            quick_stack_rect[2],
            quick_stack_rect[3],
        )
    {
        let fav_blocked = extras.favorites.as_ref().is_some_and(|fav| {
            inv.as_ref().is_some_and(|inv_res| {
                extras.chest.as_ref().is_some_and(|ch| {
                    (0..BAG).any(|i| {
                        fav.0[i]
                            && inv_res.0.slots[i]
                                .is_some_and(|s| ch.slots.iter().flatten().any(|cs| cs.id == s.id))
                    })
                })
            })
        });
        if fav_blocked {
            ui.feedback = "Quick Stack skipped: favorited slots protected".to_string();
            ui.feedback_ticks = 2.0;
            return;
        }
        if let Some(net) = &net {
            let _ = net.send(ClientMsg::ChestQuickStack);
        }
        return;
    }

    // Sort bag button beside quick-stack button
    if left && sort_btn_at(c, win) {
        let fav_occupied = extras.favorites.as_ref().is_some_and(|fav| {
            inv.as_ref()
                .is_some_and(|inv_res| (0..BAG).any(|i| fav.0[i] && inv_res.0.slots[i].is_some()))
        });
        if fav_occupied {
            ui.feedback = "Sort skipped: favorited slots protected".to_string();
            ui.feedback_ticks = 2.0;
            return;
        }
        let mode = ui.sort_mode;
        if let Some(net) = &net {
            let _ = net.send(ClientMsg::SortInventory { mode });
        }
        ui.sort_mode = match mode {
            SortMode::ByType => SortMode::ByName,
            SortMode::ByName => SortMode::ByValue,
            SortMode::ByValue => SortMode::ByType,
        };
        let label = match mode {
            SortMode::ByType => "Type",
            SortMode::ByName => "Name",
            SortMode::ByValue => "Value",
        };
        ui.feedback = format!("Sorted by {label}");
        ui.feedback_ticks = 1.5;
        return;
    }

    // Trash can slot
    if left && trash_slot_at(c, win) {
        if let Some(mut held) = held {
            if let Some(st) = held.0 {
                let stack_value = item_data(st.id).value.saturating_mul(st.count);
                if stack_value >= VALUABLE_TRASH_THRESHOLD_COPPER
                    && ui.pending_trash_confirm != Some(st)
                {
                    ui.pending_trash_confirm = Some(st);
                    ui.feedback = format!(
                        "Click trash again to discard {} ({})",
                        item_data(st.id).name,
                        format_coins(stack_value)
                    );
                    ui.feedback_ticks = 3.0;
                    return;
                }
                ui.pending_trash_confirm = None;
                held.0 = None;
                ui.trash = Some(st);
            } else {
                ui.pending_trash_confirm = None;
                if let Some(tr) = ui.trash.take() {
                    held.0 = Some(tr);
                }
            }
        }
        return;
    }

    if let Some(at) = slot_at(c, win) {
        let is_equip_slot = matches!(
            at,
            SlotRef::Armor(_)
                | SlotRef::Accessory(_)
                | SlotRef::VanityArmor(_)
                | SlotRef::VanityAccessory(_)
        );
        if is_equip_slot && ui.equip_tab == EquipTab::Housing {
            return;
        }
        ui.pending_trash_confirm = None;
        if left
            && alt(&keys)
            && let SlotRef::Inv(slot) = at
            && (slot as usize) < BAG
        {
            if let Some(ref mut fav) = extras.favorites {
                fav.0[slot as usize] = !fav.0[slot as usize];
            }
            if let Some(ref mut bus) = extras.sfx {
                bus.write(SfxEvent(SfxKind::UiClick));
            }
            return;
        }
        if shift(&keys)
            && let SlotRef::Inv(slot) = at
            && (slot as usize) < BAG
            && extras
                .favorites
                .as_ref()
                .is_some_and(|f| f.0[slot as usize])
        {
            ui.feedback = "Favorited item is protected".to_string();
            ui.feedback_ticks = 2.0;
            return;
        }
        // While the shop is up, Shift+left on a bag slot sells it.
        if let Some(slot) = sell_click(shop_open, left, shift(&keys), at) {
            if let Some(net) = &net {
                let _ = net.send(ClientMsg::ShopSell { slot });
            }
            return;
        }
        // While a chest is open, Shift+left on a bag slot transfers it to the chest.
        if chest_open
            && left
            && shift(&keys)
            && let SlotRef::Inv(slot) = at
            && (slot as usize) < COIN0
        {
            if let Some(net) = &net {
                let _ = net.send(ClientMsg::ChestMove {
                    to_chest: true,
                    slot,
                });
            }
            return;
        }
        let kind = match (left, shift(&keys)) {
            (true, true) => ClickKind::Shift,
            (true, false) => ClickKind::Left,
            _ => ClickKind::Right,
        };
        if let Some(net) = &net {
            let _ = net.send(ClientMsg::ClickSlot { at, kind });
        }
        if let Some(ref mut bus) = extras.sfx {
            bus.write(SfxEvent(SfxKind::UiClick));
        }
        return;
    }

    if left && !panels_open {
        let clicked_recipe_id = if ui.hammer_grid {
            grid_slot_at(c, win).and_then(|i| ui.list.get(i).copied())
        } else {
            carousel_slot_at(c, win).and_then(|i| ui.list.get(ui.scroll + i).copied())
        };

        if let Some(id) = clicked_recipe_id
            && let Some(inv) = inv
            && let Some(net) = &net
        {
            let count = if shift(&keys) {
                max_crafts(&inv.0, id)
            } else {
                1
            };
            if count > 0 {
                let _ = net.send(ClientMsg::Craft { recipe: id, count });
                if let Some(ref mut bus) = extras.sfx {
                    bus.write(SfxEvent(SfxKind::UiClick));
                }
            }
            return;
        }
    }

    if left
        && !panels_open
        && held.as_ref().is_some_and(|h| h.0.is_some())
        && let Some(net) = &net
    {
        let _ = net.send(ClientMsg::DropItem {
            dir: look_dir(cam.as_deref().map(|v| &**v)),
            from_cursor: true,
        });
    }
}

fn look_dir(cam: Option<&crate::view::camera::FlyCamera>) -> [f32; 3] {
    cam.map_or([0.0, 0.0, -1.0], |c| {
        let (sy, cy) = c.yaw.sin_cos();
        let (sp, cp) = c.pitch.sin_cos();
        [-sy * cp, sp, -cy * cp]
    })
}

pub fn handle_throw_key(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    gamepads: Query<&Gamepad>,
    settings: Option<Res<crate::settings::Settings>>,
    open: Option<Res<InventoryOpen>>,
    menu: Option<Res<GameMenu>>,
    victory: Option<Res<crate::hud::victory::VictoryModalState>>,
    inv: Option<Res<LocalInventory>>,
    favorites: Option<Res<FavoriteSlots>>,
    cam: Option<Single<&crate::view::camera::FlyCamera>>,
    net: Option<Res<Net>>,
) {
    let Some(net) = net else {
        return;
    };
    if open.is_some_and(|o| o.0)
        || menu.is_some_and(|m| m.panel.is_some())
        || victory.is_some_and(|v| v.active)
    {
        return;
    }
    if let (Some(inv), Some(fav)) = (inv.as_ref(), favorites.as_ref())
        && fav.0.get(inv.0.selected as usize).copied().unwrap_or(false)
    {
        return;
    }
    let bindings = settings.map(|s| s.bindings).unwrap_or_default();
    let key_pressed = keys
        .as_ref()
        .is_some_and(|k| k.just_pressed(bindings.key(crate::input_map::GameAction::Throw)));
    let gp_pressed = gamepads
        .iter()
        .any(|g| g.just_pressed(GamepadButton::DPadLeft));
    if key_pressed || gp_pressed {
        let _ = net.send(ClientMsg::DropItem {
            dir: look_dir(cam.as_deref().map(|v| &**v)),
            from_cursor: false,
        });
    }
}

/// Terraria's `ItemSlot.Draw` icon scale: art larger than 32 px shrinks to 32 px,
/// everything else is drawn at native size, then the whole icon is multiplied by the
/// slot scale. All dimensions are rounded to whole pixels to stay pixel-perfect.
pub(super) fn fit_icon(size: Vec2, slot_scale: f32) -> Vec2 {
    let max = size.max_element();
    let scale = if max > 32.0 { 32.0 / max } else { 1.0 };
    (size * scale * slot_scale).round()
}

fn count_text(s: Option<ItemStack>) -> String {
    match s {
        Some(s) if s.count > 1 => s.count.to_string(),
        _ => String::new(),
    }
}

fn set_text(t: &mut HudText, want: &str) {
    if t.text != want {
        t.text = want.to_string();
    }
}

fn set_vis(v: &mut Visibility, show: bool) {
    let want = if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *v != want {
        *v = want;
    }
}

fn place(node: &mut Node, x: f32, y: f32, w: f32, h: f32) {
    node.left = Val::Px(x);
    node.top = Val::Px(y);
    node.width = Val::Px(w);
    node.height = Val::Px(h);
}

#[derive(SystemParam)]
pub struct ViewPanels<'w> {
    chest: Option<Res<'w, ChestPanel>>,
    town: Option<Res<'w, TownPanel>>,
    tooltip: Option<ResMut<'w, super::tooltip::TooltipOverlay>>,
    acc_prefixes: Option<Res<'w, LocalAccessoryPrefixes>>,
    vanity: Option<Res<'w, ClientVanity>>,
    favorites: Option<Res<'w, FavoriteSlots>>,
    ruler: Option<Res<'w, RulerEnabled>>,
    build_grid: Option<Res<'w, BuildGridEnabled>>,
    ruler_readout: Option<Res<'w, RulerReadout>>,
}

/// Positions and fills every node of the screen from the server-owned state.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn update_view(
    open: Res<InventoryOpen>,
    ui: Res<InventoryUi>,
    window: Single<&Window, With<PrimaryWindow>>,
    inv: Option<Res<LocalInventory>>,
    armor: Option<Res<LocalArmor>>,
    accessories: Option<Res<LocalAccessories>>,
    held: Option<Res<LocalHeld>>,
    icon_assets: IconAssets,
    extra_assets: Option<Res<ExtraUiAssets>>,
    fonts: Res<HudFonts>,
    mut panels: ViewPanels,
    mut root: Query<&mut Visibility, With<InvRoot>>,
    mut images: Query<
        (&Part, &mut Node, &mut ImageNode, &mut Visibility),
        (Without<HudText>, Without<InvRoot>),
    >,
    mut texts: Query<
        (
            &Part,
            &mut Node,
            &mut HudText,
            &mut Visibility,
            Option<&mut super::text::HudTextColor>,
        ),
        (Without<ImageNode>, Without<InvRoot>),
    >,
    mut boxes: Query<
        (
            &Part,
            &mut Node,
            Option<&mut BackgroundColor>,
            Option<&mut BorderColor>,
            Option<&mut Visibility>,
        ),
        (Without<ImageNode>, Without<HudText>, Without<InvRoot>),
    >,
) {
    let win = window_size(&window);
    let s = scale(win);
    for (p, mut node, mut t, mut vis, _) in &mut texts {
        if matches!(p, Part::RulerHudText) {
            if panels.ruler.as_ref().is_some_and(|r| r.0) {
                let text = match panels.ruler_readout.as_ref().and_then(|ro| ro.0) {
                    Some(([dx, dy, dz], dist)) => {
                        format!("Ruler: dX:{dx:+} dY:{dy:+} dZ:{dz:+} ({dist:.1}m)")
                    }
                    None => "Ruler: ON (aim at block)".to_string(),
                };
                place(
                    &mut node,
                    (win[0] - 320.0 * s) / 2.0,
                    win[1] - 42.0 * s,
                    0.0,
                    0.0,
                );
                set_text(&mut t, &text);
                set_vis(&mut vis, true);
            } else {
                set_vis(&mut vis, false);
            }
        }
    }
    for mut v in &mut root {
        set_vis(&mut v, open.0);
    }
    let held_stack = held.and_then(|h| h.0);
    if !open.0 {
        for (p, _, _, mut v) in &mut images {
            if matches!(p, Part::HeldIcon) {
                set_vis(&mut v, false);
            }
        }
        for (p, _, mut t, mut v, _) in &mut texts {
            if matches!(p, Part::HeldCount) {
                set_vis(&mut v, false);
                set_text(&mut t, "");
            }
        }
        return;
    }
    let IconAssets {
        assets,
        source: icon_source,
        images: mut image_assets,
    } = icon_assets;
    let (Some(inv), Some(mut assets)) = (inv, assets) else {
        return;
    };
    let back = assets.back.clone();
    let back_selected = assets.back_selected.clone();
    let cur = cursor(&window);
    let panels_open = panels.chest.as_ref().is_some_and(|c| c.open.is_some())
        || panels.town.as_ref().is_some_and(|t| t.shop_open());
    let armor = armor.map_or([None; 3], |a| a.0);
    let accessories = accessories.map_or([None; 5], |a| a.0);
    let acc_prefixes = panels.acc_prefixes.as_ref().map_or([Prefix(0); 5], |p| p.0);
    let vanity_armor = panels.vanity.as_ref().map_or([None; 3], |v| v.armor);
    let vanity_acc = panels.vanity.as_ref().map_or([None; 5], |v| v.accessories);
    let vanity_acc_prefixes = panels
        .vanity
        .as_ref()
        .map_or([Prefix(0); 5], |v| v.accessory_prefixes);
    let equip_mode = ui.equip_tab == EquipTab::Equip;
    let slot_stack = |id: usize| -> Option<ItemStack> {
        match id {
            0..SLOTS => inv.0.slots[id],
            n if (ARMOR0..ACC0).contains(&n) => armor[n - ARMOR0].map(|item| ItemStack {
                id: item,
                count: 1,
                prefix: Prefix(0),
            }),
            n if (ACC0..VANITY_ARMOR0).contains(&n) => {
                accessories[n - ACC0].map(|item| ItemStack {
                    id: item,
                    count: 1,
                    prefix: acc_prefixes[n - ACC0],
                })
            }
            n if (VANITY_ARMOR0..VANITY_ACC0).contains(&n) => {
                vanity_armor[n - VANITY_ARMOR0].map(|item| ItemStack {
                    id: item,
                    count: 1,
                    prefix: Prefix(0),
                })
            }
            n if (VANITY_ACC0..TRASH_CELL).contains(&n) => {
                vanity_acc[n - VANITY_ACC0].map(|item| ItemStack {
                    id: item,
                    count: 1,
                    prefix: vanity_acc_prefixes[n - VANITY_ACC0],
                })
            }
            n if n == TRASH_CELL => ui.trash,
            _ => None,
        }
    };
    let font = &fonts.mouse.0;
    let hover = cur.and_then(|c| slot_at(c, win));
    let hover_cell = (0..CELLS)
        .find(|&id| (id < ARMOR0 || equip_mode) && cell_ref(id).is_some() && cell_ref(id) == hover)
        .or_else(|| cur.and_then(|c| trash_slot_at(c, win).then_some(TRASH_CELL)));
    if held_stack.is_none()
        && let Some(overlay) = panels.tooltip.as_deref_mut()
    {
        if let Some(st) = hover_cell.and_then(slot_stack)
            && let Some(cell_id) = hover_cell
        {
            let equipped = if cell_id < ARMOR0 {
                let d = item_data(st.id);
                if let Some(slot) = d.armor_slot {
                    let idx = match slot {
                        ArmorSlot::Head => 0,
                        ArmorSlot::Body => 1,
                        ArmorSlot::Legs => 2,
                    };
                    armor[idx].filter(|&eq_id| eq_id != st.id)
                } else if d.damage > 0 {
                    inv.0
                        .selected_stack()
                        .map(|s| s.id)
                        .filter(|&eq_id| eq_id != st.id && item_data(eq_id).damage > 0)
                } else {
                    None
                }
            } else {
                None
            };
            overlay.candidate = Some(super::tooltip::TooltipCandidate {
                target_id: 1_000 + cell_id as u64,
                stack: st,
                equipped,
                price_override: None,
            });
        } else if !panels_open
            && let Some(rid) = cur.and_then(|c| {
                if ui.hammer_grid {
                    grid_slot_at(c, win).and_then(|i| ui.list.get(i).copied())
                } else {
                    recipe_at(c, win).and_then(|i| ui.list.get(ui.scroll + i).copied())
                }
            })
            && let Some(r) = recipe(rid)
        {
            overlay.candidate = Some(super::tooltip::TooltipCandidate {
                target_id: 2_000 + rid.0 as u64,
                stack: ItemStack {
                    id: r.output.item,
                    count: r.output.count,
                    prefix: Prefix(0),
                },
                equipped: None,
                price_override: None,
            });
        }
    }
    let _visible: Vec<RecipeId> = ui.list.iter().skip(ui.scroll).take(ROWS).copied().collect();
    // Loads missing icons on demand and caches them in `HotbarAssets`.
    let mut icon_for = |item: terravibe_sim::item::ItemId| {
        icon_of(&mut assets, &mut image_assets, icon_source.as_deref(), item)
    };

    for (p, mut node, mut img, mut vis) in &mut images {
        match *p {
            Part::Back(id) => {
                if (ARMOR0..TRASH_CELL).contains(&id) && !equip_mode {
                    set_vis(&mut vis, false);
                    continue;
                }
                set_vis(&mut vis, true);
                let [x, y, sz] = cell_rect(id, win);
                place(&mut node, x, y, sz, sz);
                let is_valid = cell_ref(id).is_some() || id == TRASH_CELL;
                let hovered = is_valid && hover_cell == Some(id);
                let want_img = if hovered { &back_selected } else { &back };
                if img.image != *want_img {
                    img.image = want_img.clone();
                }
                let alpha = if hovered {
                    0.98
                } else if is_valid {
                    0.85
                } else {
                    0.35
                };
                let is_fav = id < BAG && panels.favorites.as_ref().is_some_and(|f| f.0[id]);
                let is_vanity = (VANITY_ARMOR0..TRASH_CELL).contains(&id);
                img.color = if is_fav {
                    Color::srgba(1.0, 0.88, 0.35, alpha)
                } else if is_vanity {
                    Color::srgba(0.76, 0.84, 1.0, alpha)
                } else {
                    Color::srgba(1.0, 1.0, 1.0, alpha)
                };
            }
            Part::Ghost(id) => {
                if (ARMOR0..TRASH_CELL).contains(&id) && !equip_mode {
                    set_vis(&mut vis, false);
                    continue;
                }
                let [x, y, sz] = cell_rect(id, win);
                if slot_stack(id).is_none() {
                    let ghost = ghost_silhouette_for_cell(id);
                    let handle = ghost.and_then(|g| match g {
                        GhostSilhouette::Coin(c) => extra_assets
                            .as_ref()
                            .and_then(|e| e.coin_ghosts.get(c).and_then(|o| o.clone())),
                        GhostSilhouette::Ammo => {
                            extra_assets.as_ref().and_then(|e| e.ammo_ghost.clone())
                        }
                        GhostSilhouette::Trash => {
                            extra_assets.as_ref().and_then(|e| e.trash_ghost.clone())
                        }
                        GhostSilhouette::ArmorHead => extra_assets
                            .as_ref()
                            .and_then(|e| e.armor_head_ghost.clone()),
                        GhostSilhouette::ArmorBody => extra_assets
                            .as_ref()
                            .and_then(|e| e.armor_body_ghost.clone()),
                        GhostSilhouette::ArmorLegs => extra_assets
                            .as_ref()
                            .and_then(|e| e.armor_legs_ghost.clone()),
                        GhostSilhouette::Accessory => extra_assets
                            .as_ref()
                            .and_then(|e| e.accessory_ghost.clone()),
                    });
                    if let Some(handle) = handle {
                        let d = sz * 0.52;
                        place(&mut node, x + (sz - d) / 2.0, y + (sz - d) / 2.0, d, d);
                        if img.image != handle {
                            img.image = handle;
                        }
                        let ghost_alpha = if (VANITY_ARMOR0..TRASH_CELL).contains(&id) {
                            0.26
                        } else {
                            0.40
                        };
                        img.color = Color::srgba(1.0, 1.0, 1.0, ghost_alpha);
                        set_vis(&mut vis, true);
                    } else {
                        set_vis(&mut vis, false);
                    }
                } else {
                    set_vis(&mut vis, false);
                }
            }
            Part::Icon(id) => {
                if (ARMOR0..TRASH_CELL).contains(&id) && !equip_mode {
                    set_vis(&mut vis, false);
                    continue;
                }
                match slot_stack(id) {
                    Some(stack) => {
                        let [x, y, sz] = cell_rect(id, win);
                        let (handle, size) = icon_for(stack.id);
                        let d = fit_icon(size, SLOT_SCALE);
                        place(
                            &mut node,
                            (x + (sz - d.x) / 2.0).round(),
                            (y + (sz - d.y) / 2.0).round(),
                            d.x.round(),
                            d.y.round(),
                        );
                        if img.image != handle {
                            img.image = handle;
                        }
                        set_vis(&mut vis, true);
                    }
                    None => set_vis(&mut vis, false),
                }
            }
            Part::CarouselBack(i) => {
                let count_visible = if ui.hammer_grid {
                    ui.list.len().min(HAMMER_SLOTS)
                } else {
                    ui.list.len().saturating_sub(ui.scroll).min(CAROUSEL_ROWS)
                };
                let is_vis = !panels_open && i < count_visible;
                if is_vis {
                    let [x, y, w, h] = if ui.hammer_grid {
                        grid_slot_rect(i, win)
                    } else {
                        carousel_slot_rect(i, win)
                    };
                    place(&mut node, x, y, w, h);
                    let is_sel = if ui.hammer_grid {
                        i == ui.selected
                    } else {
                        ui.scroll + i == ui.selected
                    };
                    let want = if is_sel { &back_selected } else { &back };
                    if img.image != *want {
                        img.image = want.clone();
                    }
                    set_vis(&mut vis, true);
                } else {
                    set_vis(&mut vis, false);
                }
            }
            Part::CarouselIcon(i) => {
                let count_visible = if ui.hammer_grid {
                    ui.list.len().min(HAMMER_SLOTS)
                } else {
                    ui.list.len().saturating_sub(ui.scroll).min(CAROUSEL_ROWS)
                };
                let is_vis = !panels_open && i < count_visible;
                let rec_id = if ui.hammer_grid {
                    ui.list.get(i).copied()
                } else {
                    ui.list.get(ui.scroll + i).copied()
                };
                match (is_vis, rec_id.and_then(recipe)) {
                    (true, Some(recipe)) => {
                        let [x, y, w, h] = if ui.hammer_grid {
                            grid_slot_rect(i, win)
                        } else {
                            carousel_slot_rect(i, win)
                        };
                        let (handle, size) = icon_for(recipe.output.item);
                        let d = fit_icon(size, SLOT_SCALE);
                        place(
                            &mut node,
                            (x + (w - d.x) / 2.0).round(),
                            (y + (h - d.y) / 2.0).round(),
                            d.x.round(),
                            d.y.round(),
                        );
                        if img.image != handle {
                            img.image = handle;
                        }
                        set_vis(&mut vis, true);
                    }
                    _ => set_vis(&mut vis, false),
                }
            }
            Part::HammerBtnIcon => {
                let [hx, hy, hw, hh] = hammer_button_rect(win);
                place(
                    &mut node,
                    hx + 4.0 * s,
                    hy + 4.0 * s,
                    hw - 8.0 * s,
                    hh - 8.0 * s,
                );
                if let Some(ref extra) = extra_assets
                    && let Some(ref h_img) = extra.hammer_btn
                {
                    if img.image != *h_img {
                        img.image = h_img.clone();
                    }
                    set_vis(&mut vis, !panels_open);
                } else {
                    set_vis(&mut vis, false);
                }
            }
            Part::QuickStackBtn => {
                let qx = (special_x() + STEP) * s;
                let qy = (GRID[1] + 4.0 * STEP) * s;
                place(&mut node, qx + 2.0 * s, qy + 2.0 * s, 28.0 * s, 28.0 * s);
                if let Some(ref extra) = extra_assets
                    && let Some(ref qs) = extra.quick_stack_btn
                {
                    if img.image != *qs {
                        img.image = qs.clone();
                    }
                    set_vis(&mut vis, true);
                } else {
                    set_vis(&mut vis, false);
                }
            }
            Part::SortBtn => {
                let sx = (special_x() + STEP + 36.0) * s;
                let sy = (GRID[1] + 4.0 * STEP) * s;
                place(&mut node, sx + 2.0 * s, sy + 2.0 * s, 28.0 * s, 28.0 * s);
                if let Some(ref extra) = extra_assets
                    && let Some(ref sb) = extra.sort_btn
                {
                    if img.image != *sb {
                        img.image = sb.clone();
                    }
                    set_vis(&mut vis, true);
                } else {
                    set_vis(&mut vis, false);
                }
            }
            Part::DefenseIcon => {
                let def_x = win[0] - (EQUIP_RIGHT + CELL + STEP + 104.0) * s;
                let def_y = (EQUIP_TOP + 6.0 * STEP + 10.0) * s;
                place(&mut node, def_x, def_y, 24.0 * s, 26.0 * s);
                if equip_mode
                    && let Some(ref extra) = extra_assets
                    && let Some(ref sh) = extra.shield_icon
                {
                    if img.image != *sh {
                        img.image = sh.clone();
                    }
                    set_vis(&mut vis, true);
                } else {
                    set_vis(&mut vis, false);
                }
            }
            Part::RowIcon(_i) => {
                set_vis(&mut vis, false);
            }
            Part::HeldIcon => match (cur, held_stack) {
                (Some(c), Some(stack)) => {
                    let (handle, size) = icon_for(stack.id);
                    let d = fit_icon(size, SLOT_SCALE);
                    place(
                        &mut node,
                        (c[0] - d.x / 2.0).round(),
                        (c[1] - d.y / 2.0).round(),
                        d.x.round(),
                        d.y.round(),
                    );
                    if img.image != handle {
                        img.image = handle;
                    }
                    set_vis(&mut vis, true);
                }
                _ => set_vis(&mut vis, false),
            },
            _ => {}
        }
    }
    for (p, mut node, mut t, mut vis, mut color) in &mut texts {
        set_vis(&mut vis, true);
        match *p {
            Part::InventoryHeader => {
                place(&mut node, GRID[0] * s, 0.0, 0.0, 0.0);
                set_text(
                    &mut t,
                    &format!("Inventory — {}", format_coins(total_copper(&inv.0))),
                );
            }
            Part::CraftingHeader => {
                place(
                    &mut node,
                    (CAROUSEL_POS[0] + CAROUSEL_SLOT_SIZE + 14.0) * s,
                    (CAROUSEL_POS[1] - 30.0) * s,
                    0.0,
                    0.0,
                );
                let text = if panels_open {
                    String::new()
                } else if ui.search_query.is_empty() {
                    "Crafting".to_string()
                } else {
                    format!("Crafting — Search: {}", ui.search_query)
                };
                set_text(&mut t, &text);
            }
            Part::RecipeName => {
                place(
                    &mut node,
                    (CAROUSEL_POS[0] + CAROUSEL_SLOT_SIZE + 14.0) * s,
                    (CAROUSEL_POS[1] - 4.0) * s,
                    0.0,
                    0.0,
                );
                let name = if panels_open {
                    String::new()
                } else {
                    ui.list
                        .get(ui.selected)
                        .and_then(|&id| recipe(id))
                        .map(title_text)
                        .unwrap_or_default()
                };
                set_text(&mut t, &name);
            }
            Part::DefenseText => {
                if !equip_mode {
                    set_vis(&mut vis, false);
                    continue;
                }
                let acc_def: i32 = accessories
                    .iter()
                    .flatten()
                    .map(|&id| item_data(id).defense)
                    .sum();
                let def = super::armor::total_defense(&armor) + acc_def;
                let def_x = win[0] - (EQUIP_RIGHT + CELL + STEP + 104.0) * s;
                let def_y = (EQUIP_TOP + 6.0 * STEP + 10.0) * s;
                place(&mut node, def_x + 28.0 * s, def_y + 5.0 * s, 0.0, 0.0);
                set_text(&mut t, &format!("Def: {def}"));
            }
            Part::Count(id) => {
                if (ARMOR0..TRASH_CELL).contains(&id) && !equip_mode {
                    set_vis(&mut vis, false);
                    continue;
                }
                let [x, y, sz] = cell_rect(id, win);
                let count_str = count_text(slot_stack(id));
                let tw = measure(&fonts.stack.0, &count_str);
                node.left = Val::Px(x + sz - tw - 6.0 * s);
                node.top = Val::Px(y + sz - 20.0 * s);
                set_text(&mut t, &count_str);
            }
            Part::FavStar(id) => {
                let is_fav = id < BAG
                    && panels.favorites.as_ref().is_some_and(|f| f.0[id])
                    && slot_stack(id).is_some();
                if is_fav {
                    let [x, y, _] = cell_rect(id, win);
                    place(&mut node, x + 4.0 * s, y + 2.0 * s, 0.0, 0.0);
                    set_text(&mut t, "*");
                    set_vis(&mut vis, true);
                } else {
                    set_vis(&mut vis, false);
                }
            }
            Part::RulerText => {
                let [x, y, _w, _h] = ruler_btn_rect(win);
                place(&mut node, x + 8.0 * s, y + 5.0 * s, 0.0, 0.0);
                let on = panels.ruler.as_ref().is_some_and(|r| r.0);
                set_text(&mut t, if on { "Ruler: ON" } else { "Ruler: OFF" });
            }
            Part::BuildGridText => {
                let [x, y, _w, _h] = build_grid_btn_rect(win);
                place(&mut node, x + 8.0 * s, y + 5.0 * s, 0.0, 0.0);
                let on = panels.build_grid.as_ref().is_some_and(|g| g.0);
                set_text(&mut t, if on { "Grid: ON" } else { "Grid: OFF" });
            }
            Part::EquipTabText(tab) => {
                let [x, y, w, h] = equip_tab_rect(tab, win);
                let label_str = if tab == 0 { "Equip" } else { "Housing" };
                let tw = measure(font, label_str);
                let lh = font.line_spacing as f32;
                node.left = Val::Px(x + ((w - tw) / 2.0).max(4.0 * s));
                node.top = Val::Px(y + ((h - lh) / 2.0).max(2.0 * s));
                set_text(&mut t, label_str);
            }
            Part::HousingQueryText => {
                if equip_mode {
                    set_vis(&mut vis, false);
                    continue;
                }
                let [x, y, w, h] = housing_query_rect(win);
                let label_str = "? Query Housing";
                let tw = measure(font, label_str);
                let lh = font.line_spacing as f32;
                node.left = Val::Px(x + ((w - tw) / 2.0).max(6.0 * s));
                node.top = Val::Px(y + ((h - lh) / 2.0).max(2.0 * s));
                set_text(&mut t, label_str);
            }
            Part::HousingStatusText => {
                if equip_mode {
                    set_vis(&mut vis, false);
                    continue;
                }
                let [sx, sy, _, _] = housing_status_rect(win);
                place(&mut node, sx + 10.0 * s, sy + 8.0 * s, 0.0, 0.0);
                let housing_status = panels.town.as_ref().and_then(|tp| tp.housing_status);
                let raw = housing_status
                    .map(|(ok, reason)| super::town::house_text(ok, reason))
                    .unwrap_or("Click ? Query Housing to check this room.");
                let wrapped = wrap_housing_text(raw, 24);
                set_text(&mut t, &wrapped);
                if let Some(ref mut c) = color {
                    c.0 = match housing_status {
                        Some((true, _)) => Color::srgb(0.45, 0.95, 0.55),
                        Some((false, _)) => Color::srgb(1.0, 0.78, 0.35),
                        None => Color::srgb(0.84, 0.88, 0.96),
                    };
                }
            }
            Part::HousingNpcText(i) => {
                if equip_mode {
                    set_vis(&mut vis, false);
                    continue;
                }
                let [nx, ny, _nw, nh] = housing_npc_rect(i, win);
                let lh = font.line_spacing as f32;
                place(
                    &mut node,
                    nx + 36.0 * s,
                    ny + ((nh - lh) / 2.0).max(2.0 * s),
                    0.0,
                    0.0,
                );
                set_text(&mut t, HOUSING_NPCS[i].name());
            }
            Part::HousingNpcStatus(i) => {
                if equip_mode {
                    set_vis(&mut vis, false);
                    continue;
                }
                let [nx, ny, nw, nh] = housing_npc_rect(i, win);
                let selected = panels
                    .town
                    .as_ref()
                    .is_some_and(|tp| tp.selected_npc == HOUSING_NPCS[i]);
                let housing_ok = panels
                    .town
                    .as_ref()
                    .is_some_and(|tp| tp.housing_status.is_some_and(|(ok, _)| ok));
                let badge = if selected && housing_ok {
                    "Home"
                } else if selected {
                    "Active"
                } else {
                    "Set"
                };
                let tw = measure(font, badge);
                let lh = font.line_spacing as f32;
                place(
                    &mut node,
                    nx + nw - tw - 10.0 * s,
                    ny + ((nh - lh) / 2.0).max(2.0 * s),
                    0.0,
                    0.0,
                );
                set_text(&mut t, badge);
                if let Some(ref mut c) = color {
                    c.0 = if selected && housing_ok {
                        Color::srgb(0.45, 0.95, 0.55)
                    } else if selected {
                        super::style::GOLD_TEXT
                    } else {
                        Color::srgb(0.62, 0.68, 0.82)
                    };
                }
            }
            Part::RowText(_i) => {
                set_vis(&mut vis, false);
                set_text(&mut t, "");
            }
            Part::SettingsText => {
                let [x, y, w, h] = settings_rect(win);
                let tw = measure(font, "Settings");
                node.left = Val::Px(x + (w - tw) / 2.0);
                node.top = Val::Px(y + (h - font.line_spacing as f32) / 2.0);
                set_text(&mut t, "Settings");
            }
            Part::Detail => {
                node.left = Val::Px((CAROUSEL_POS[0] + CAROUSEL_SLOT_SIZE + 14.0) * s);
                node.top = Val::Px((CAROUSEL_POS[1] + 24.0) * s);
                let want = if panels_open {
                    String::new()
                } else {
                    ui.list
                        .get(ui.selected)
                        .map(|&id| detail_text(id, Some(&inv.0)))
                        .unwrap_or_default()
                };
                set_text(&mut t, &want);
            }
            Part::Feedback => {
                node.left = Val::Px(DETAIL_X * s);
                node.top = Val::Px((REF_H - 100.0) * s);
                set_text(&mut t, &ui.feedback);
            }
            Part::HeldCount => match (cur, held_stack) {
                (Some(c), Some(h)) => {
                    node.left = Val::Px(c[0] - SLOT * s * 0.3);
                    node.top = Val::Px(c[1] + SLOT * s * 0.1);
                    set_text(&mut t, &count_text(Some(h)));
                }
                _ => {
                    set_vis(&mut vis, false);
                    set_text(&mut t, "");
                }
            },
            _ => {}
        }
    }
    for (p, mut node, bg, border, mut vis) in &mut boxes {
        match *p {
            Part::Panel => place(&mut node, 0.0, 0.0, 0.0, 0.0),
            Part::InvCard => place(
                &mut node,
                (GRID[0] - 10.0) * s,
                2.0 * s,
                (COLS as f32 * STEP + 16.0) * s,
                (GRID[1] + 5.0 * STEP + 8.0) * s,
            ),
            Part::CoinsCard => place(
                &mut node,
                (special_x() - 8.0) * s,
                (GRID[1] - 8.0) * s,
                (2.0 * STEP + 24.0) * s,
                (5.0 * STEP + 14.0) * s,
            ),
            Part::CraftCard => {
                if panels_open {
                    place(&mut node, 0.0, 0.0, 0.0, 0.0);
                    if let Some(mut bg) = bg {
                        bg.0 = Color::NONE;
                    }
                    if let Some(mut border) = border {
                        *border = BorderColor::all(Color::NONE);
                    }
                } else {
                    let h = if ui.hammer_grid {
                        (HAMMER_ROWS as f32 * CAROUSEL_PITCH + 50.0) * s
                    } else {
                        (CAROUSEL_ROWS as f32 * CAROUSEL_PITCH + 50.0) * s
                    };
                    place(
                        &mut node,
                        (CAROUSEL_POS[0] - 10.0) * s,
                        (CAROUSEL_POS[1] - 42.0) * s,
                        420.0 * s,
                        h,
                    );
                    if let Some(mut bg) = bg {
                        bg.0 = super::style::PANEL_BG;
                    }
                    if let Some(mut border) = border {
                        *border = BorderColor::all(super::style::PANEL_BORDER);
                    }
                }
            }
            Part::EquipCard => {
                let [ex, ey, ew, eh] = equip_card_rect(win);
                place(&mut node, ex, ey, ew, eh);
            }
            Part::HammerBtn => {
                let [hx, hy, hw, hh] = hammer_button_rect(win);
                place(&mut node, hx, hy, hw, hh);
                if let Some(mut bg) = bg {
                    let want = if panels_open {
                        Color::NONE
                    } else if ui.hammer_grid {
                        ROW_SEL
                    } else {
                        ROW_BG
                    };
                    if bg.0 != want {
                        bg.0 = want;
                    }
                }
            }
            Part::EquipTab(tab) => {
                let [tx, ty, tw, th] = equip_tab_rect(tab, win);
                place(&mut node, tx, ty, tw, th);
                let active = (tab == 0 && equip_mode) || (tab == 1 && !equip_mode);
                if let Some(mut bg) = bg {
                    bg.0 = if active { ROW_SEL } else { ROW_BG };
                }
                if let Some(mut border) = border {
                    *border = BorderColor::all(if active {
                        super::style::GOLD_BORDER
                    } else {
                        super::style::PANEL_BORDER
                    });
                }
            }
            Part::RulerBtn => {
                let [rx, ry, rw, rh] = ruler_btn_rect(win);
                place(&mut node, rx, ry, rw, rh);
                let on = panels.ruler.as_ref().is_some_and(|r| r.0);
                if let Some(mut bg) = bg {
                    bg.0 = if on { ROW_SEL } else { ROW_BG };
                }
                if let Some(mut border) = border {
                    *border = BorderColor::all(if on {
                        super::style::GOLD_BORDER
                    } else {
                        super::style::PANEL_BORDER
                    });
                }
            }
            Part::BuildGridBtn => {
                let [gx, gy, gw, gh] = build_grid_btn_rect(win);
                place(&mut node, gx, gy, gw, gh);
                let on = panels.build_grid.as_ref().is_some_and(|g| g.0);
                if let Some(mut bg) = bg {
                    bg.0 = if on { ROW_SEL } else { ROW_BG };
                }
                if let Some(mut border) = border {
                    *border = BorderColor::all(if on {
                        super::style::GOLD_BORDER
                    } else {
                        super::style::PANEL_BORDER
                    });
                }
            }
            Part::HousingQueryBtn => {
                if let Some(ref mut v) = vis {
                    set_vis(v, !equip_mode);
                }
                if equip_mode {
                    place(&mut node, 0.0, 0.0, 0.0, 0.0);
                } else {
                    let [qx, qy, qw, qh] = housing_query_rect(win);
                    place(&mut node, qx, qy, qw, qh);
                    let hovered = cur.is_some_and(|c| inside(c, qx, qy, qw, qh));
                    if let Some(mut bg) = bg {
                        bg.0 = if hovered { ROW_SEL } else { ROW_BG };
                    }
                    if let Some(mut border) = border {
                        *border = BorderColor::all(if hovered {
                            super::style::GOLD_BORDER
                        } else {
                            super::style::PANEL_BORDER
                        });
                    }
                }
            }
            Part::HousingStatusBox => {
                if let Some(ref mut v) = vis {
                    set_vis(v, !equip_mode);
                }
                if equip_mode {
                    place(&mut node, 0.0, 0.0, 0.0, 0.0);
                } else {
                    let [sx, sy, sw, sh] = housing_status_rect(win);
                    place(&mut node, sx, sy, sw, sh);
                    let housing_status = panels.town.as_ref().and_then(|tp| tp.housing_status);
                    if let Some(mut bg) = bg {
                        bg.0 = match housing_status {
                            Some((true, _)) => Color::srgba(0.08, 0.22, 0.14, 0.90),
                            Some((false, _)) => Color::srgba(0.22, 0.15, 0.06, 0.90),
                            None => super::style::BAR_BG,
                        };
                    }
                    if let Some(mut border) = border {
                        *border = BorderColor::all(match housing_status {
                            Some((true, _)) => Color::srgba(0.28, 0.82, 0.42, 0.95),
                            Some((false, _)) => Color::srgba(0.92, 0.65, 0.22, 0.95),
                            None => super::style::PANEL_BORDER,
                        });
                    }
                }
            }
            Part::HousingNpcBtn(i) => {
                if let Some(ref mut v) = vis {
                    set_vis(v, !equip_mode);
                }
                if equip_mode {
                    place(&mut node, 0.0, 0.0, 0.0, 0.0);
                } else {
                    let [nx, ny, nw, nh] = housing_npc_rect(i, win);
                    place(&mut node, nx, ny, nw, nh);
                    let selected = panels
                        .town
                        .as_ref()
                        .is_some_and(|tp| tp.selected_npc == HOUSING_NPCS[i]);
                    let hovered = cur.is_some_and(|c| inside(c, nx, ny, nw, nh));
                    if let Some(mut bg) = bg {
                        bg.0 = if selected {
                            ROW_SEL
                        } else if hovered {
                            Color::srgba(0.22, 0.29, 0.56, 0.94)
                        } else {
                            ROW_BG
                        };
                    }
                    if let Some(mut border) = border {
                        *border = BorderColor::all(if selected || hovered {
                            super::style::GOLD_BORDER
                        } else {
                            super::style::PANEL_BORDER
                        });
                    }
                }
            }
            Part::HousingNpcSwatch(i) => {
                if let Some(ref mut v) = vis {
                    set_vis(v, !equip_mode);
                }
                if equip_mode {
                    place(&mut node, 0.0, 0.0, 0.0, 0.0);
                } else {
                    let [nx, ny, _nw, nh] = housing_npc_rect(i, win);
                    let sz = 20.0 * s;
                    place(&mut node, nx + 8.0 * s, ny + (nh - sz) / 2.0, sz, sz);
                    if let Some(mut bg) = bg {
                        bg.0 = housing_npc_swatch_color(HOUSING_NPCS[i]);
                    }
                }
            }
            Part::RowBack(_i) => {
                place(&mut node, 0.0, 0.0, 0.0, 0.0);
            }
            Part::SettingsBack => {
                let [x, y, w, h] = settings_rect(win);
                place(&mut node, x, y, w, h);
            }
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::chest::ChestPanel;
    use crate::hud::crafting::{RowState, row_state};
    use crate::hud::town::TownPanel;
    use crate::menu::Action;
    use crate::menu::game::handle_game_menu_input;
    use crate::net::{LocalInventory, Net};
    use crate::session::{AppState, GameMenu, GamePanel, SessionError};
    use crate::view::camera::{CameraMode, FlyCamera};
    use crate::view::player::{LocalPlayer, SampledInput, sample_input};
    use bevy::input::mouse::MouseWheel;
    use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
    use std::sync::{Arc, Mutex};
    use terravibe_sim::crafting::{
        FURNACE_RECIPE, TORCH_RECIPE, WOODEN_SWORD_RECIPE, WORKBENCH_RECIPE,
    };
    use terravibe_sim::inventory::{Inventory, ItemStack};
    use terravibe_sim::item::{ANVIL_ITEM, DIRT_BLOCK, GEL, Prefix, WOOD, item_data};
    use terravibe_sim::net::ClientMsg;
    use terravibe_sim::slots::SlotRef;

    fn stack(id: terravibe_sim::item::ItemId, count: u32) -> ItemStack {
        ItemStack {
            id,
            count,
            prefix: Prefix(0),
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_message::<Action>()
            .add_message::<MouseWheel>()
            .add_message::<KeyboardInput>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<InventoryOpen>()
            .init_resource::<GameMenu>()
            .init_resource::<ChestPanel>()
            .init_resource::<TownPanel>()
            .init_resource::<InventoryUi>()
            .init_resource::<LocalHeld>()
            .insert_resource(SessionError(None))
            .insert_resource(LocalPlayer(terravibe_sim::player::Player::new([0.0; 3])))
            .insert_resource(LocalInventory(Inventory::empty()))
            .insert_resource(CameraMode::First)
            .init_resource::<SampledInput>()
            .add_systems(Update, (handle_game_menu_input, click_input));
        let mut window = Window::default();
        window.resolution.set(1920.0, 1080.0);
        let opts = CursorOptions {
            grab_mode: CursorGrabMode::Locked,
            visible: false,
            ..default()
        };
        app.world_mut().spawn((window, PrimaryWindow, opts));
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app
    }

    fn press(app: &mut App, key: KeyCode) {
        let mut k = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        k.clear();
        k.press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(key);
    }

    fn grab(app: &mut App) -> CursorGrabMode {
        app.world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap()
            .grab_mode
    }

    fn click_at(app: &mut App, pos: [f32; 2]) {
        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .unwrap()
            .set_cursor_position(Some(Vec2::new(pos[0], pos[1])));
        let mut m = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        m.clear();
        m.press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
    }

    #[test]
    fn esc_opens_inventory_and_releases_cursor() {
        let mut app = app();
        press(&mut app, KeyCode::Escape);
        assert!(app.world().resource::<InventoryOpen>().0);
        assert_eq!(app.world().resource::<GameMenu>().panel, None);
        assert_eq!(grab(&mut app), CursorGrabMode::None);
    }

    #[test]
    fn esc_closes_inventory_chest_and_shop_in_one_press() {
        let mut app = app();
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        app.world_mut().resource_mut::<ChestPanel>().open = Some([1, 2, 3]);
        app.world_mut().resource_mut::<TownPanel>().npc = Some(terravibe_sim::enemy::EnemyId(7));
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<InventoryOpen>().0);
        assert!(app.world().resource::<ChestPanel>().open.is_none());
        assert!(app.world().resource::<TownPanel>().npc.is_none());
        assert_eq!(app.world().resource::<GameMenu>().panel, None);
        assert_eq!(grab(&mut app), CursorGrabMode::Locked);
    }

    #[test]
    fn esc_closes_housing_panel_when_opened_without_npc() {
        let mut app = app();
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        {
            let mut town = app.world_mut().resource_mut::<TownPanel>();
            town.tab = crate::hud::town::TownTab::Housing;
            town.npc = None;
        }
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<InventoryOpen>().0);
        assert!(!app.world().resource::<TownPanel>().is_open());
        assert_eq!(app.world().resource::<GameMenu>().panel, None);
        assert_eq!(grab(&mut app), CursorGrabMode::Locked);
    }

    #[test]
    fn esc_while_dead_opens_game_menu() {
        let mut app = app();
        app.world_mut().resource_mut::<LocalPlayer>().0.dead = true;
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<InventoryOpen>().0);
        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Root)
        );
        assert_eq!(grab(&mut app), CursorGrabMode::None);
    }

    #[test]
    fn settings_button_opens_game_menu() {
        let mut app = app();
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        let [x, y, w, h] = settings_rect([1920.0, 1080.0]);
        click_at(&mut app, [x + w / 2.0, y + h / 2.0]);
        assert!(!app.world().resource::<InventoryOpen>().0);
        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Root)
        );
        assert!(settings_at([x + 1.0, y + 1.0], [1920.0, 1080.0]));
        assert!(!settings_at([x - 1.0, y + 1.0], [1920.0, 1080.0]));
    }

    #[test]
    fn slot_at_maps_grid_coin_ammo_armor_at_1080_and_720() {
        use terravibe_sim::item::ArmorSlot;
        for win in [[1920.0, 1080.0], [1280.0, 720.0]] {
            let center = |id: usize| {
                let [x, y, sz] = cell_rect(id, win);
                [x + sz / 2.0, y + sz / 2.0]
            };
            assert_eq!(slot_at(center(0), win), Some(SlotRef::Inv(0)));
            assert_eq!(slot_at(center(9), win), Some(SlotRef::Inv(9)));
            assert_eq!(slot_at(center(49), win), Some(SlotRef::Inv(49)));
            assert_eq!(slot_at(center(COIN0), win), Some(SlotRef::Inv(50)));
            assert_eq!(slot_at(center(COIN0 + 3), win), Some(SlotRef::Inv(53)));
            assert_eq!(slot_at(center(AMMO0), win), Some(SlotRef::Inv(54)));
            assert_eq!(slot_at(center(AMMO0 + 3), win), Some(SlotRef::Inv(57)));
            assert_eq!(
                slot_at(center(ARMOR0), win),
                Some(SlotRef::Armor(ArmorSlot::Head))
            );
            assert_eq!(
                slot_at(center(ARMOR0 + 1), win),
                Some(SlotRef::Armor(ArmorSlot::Body))
            );
            assert_eq!(
                slot_at(center(ARMOR0 + 2), win),
                Some(SlotRef::Armor(ArmorSlot::Legs))
            );
            // Accessories are clickable slots (SlotRef::Accessory).
            assert_eq!(slot_at(center(ACC0), win), Some(SlotRef::Accessory(0)));
            assert_eq!(slot_at(center(ACC0 + 4), win), Some(SlotRef::Accessory(4)));
            // The gap between two slots is not a slot, and neither is empty space.
            let [x0, y0, sz] = cell_rect(0, win);
            let [x1, _, _] = cell_rect(1, win);
            assert_eq!(slot_at([(x0 + sz + x1) / 2.0, y0 + sz / 2.0], win), None);
            assert_eq!(slot_at([5.0, 5.0], win), None);
        }
        // Bag row 0 is aligned with the hotbar row.
        let [hx, hy, hs] = super::super::hotbar::slot_rect(0);
        let [x, y, sz] = cell_rect(0, [1920.0, 1080.0]);
        assert!((x - hx).abs() < 1e-3 && (y - hy).abs() < 1e-3 && (sz - hs).abs() < 1e-3);
        // The crafting list sits below the grid.
        assert_eq!(recipe_at([100.0, 440.0], [1920.0, 1080.0]), Some(0));
        assert_eq!(recipe_at([100.0, 440.0 + 60.0], [1920.0, 1080.0]), Some(1));
        assert_eq!(recipe_at([100.0, 100.0], [1920.0, 1080.0]), None);
        assert_eq!(
            recipe_at([100.0 * 2.0 / 3.0, 440.0 * 2.0 / 3.0], [1280.0, 720.0]),
            Some(0)
        );
    }

    #[test]
    fn fit_icon_follows_terraria_scale_rule() {
        // Native pixel art stays native, then gets the slot scale.
        assert_eq!(fit_icon(Vec2::new(16.0, 16.0), 0.85), Vec2::new(14.0, 14.0));
        // Oversized art shrinks to 32 px on its longest side, then the slot scale.
        let d = fit_icon(Vec2::new(64.0, 32.0), 0.85);
        assert_eq!(d, Vec2::new(27.0, 14.0));
        // Everything is rounded to whole pixels.
        let d = fit_icon(Vec2::new(20.0, 20.0), 0.85);
        assert_eq!(d, Vec2::new(17.0, 17.0));
    }

    #[test]
    fn recipe_list_shows_only_craftable() {
        let mut inv = Inventory::empty();
        assert!(inv.add(stack(WOOD, 10)).is_none());
        assert!(inv.add(stack(GEL, 1)).is_none());
        let list = craftable_list(&inv, &[]);
        assert!(list.contains(&WORKBENCH_RECIPE) && list.contains(&TORCH_RECIPE));
        assert!(
            list.iter()
                .all(|&id| row_state(&inv, id, true) == RowState::Craftable
                    || row_state(&inv, id, false) == RowState::Craftable)
        );
        assert!(list.len() < terravibe_sim::crafting::recipes().len());
        assert!(craftable_list(&Inventory::empty(), &[]).is_empty());
    }

    #[test]
    fn use_held_false_while_inventory_open() {
        let mut app = app();
        app.world_mut().spawn(FlyCamera {
            yaw: 0.0,
            pitch: 0.0,
        });
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.add_systems(Update, sample_input);
        app.update();
        assert!(app.world().resource::<SampledInput>().input.use_held);
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        app.update();
        assert!(!app.world().resource::<SampledInput>().input.use_held);
    }

    #[test]
    fn c_key_does_nothing() {
        let mut app = app();
        press(&mut app, KeyCode::KeyC);
        assert!(!app.world().resource::<InventoryOpen>().0);
        assert_eq!(app.world().resource::<GameMenu>().panel, None);
        assert_eq!(grab(&mut app), CursorGrabMode::Locked);
    }

    #[test]
    fn max_crafts_counts_repeats() {
        // Workbench: 10 Wood each.
        let mut inv = Inventory::empty();
        assert!(inv.add(stack(WOOD, 35)).is_none());
        assert_eq!(max_crafts(&inv, WORKBENCH_RECIPE), 3);
        // Torch needs Wood and Gel; the scarcer one limits.
        let mut inv = Inventory::empty();
        assert!(inv.add(stack(WOOD, 10)).is_none());
        assert!(inv.add(stack(GEL, 2)).is_none());
        assert_eq!(max_crafts(&inv, TORCH_RECIPE), 2);
        assert_eq!(max_crafts(&Inventory::empty(), TORCH_RECIPE), 0);
    }

    #[test]
    fn dead_esc_closes_open_inventory() {
        let mut app = app();
        app.world_mut().resource_mut::<LocalPlayer>().0.dead = true;
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<InventoryOpen>().0);
        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Root)
        );
    }

    struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<ClientMsg>>>);
    impl terravibe_sim::net::ClientTransport for Recorder {
        fn send(&self, m: ClientMsg) -> Result<(), terravibe_sim::net::Disconnected> {
            self.0.lock().unwrap().push(m);
            Ok(())
        }
        fn try_recv(
            &self,
        ) -> Result<Option<terravibe_sim::net::ServerMsg>, terravibe_sim::net::Disconnected>
        {
            Ok(None)
        }
    }

    fn craft_click(shift_held: bool) -> Vec<ClientMsg> {
        let mut app = app();
        let sent = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        app.insert_resource(Net::new(Recorder(sent.clone())));
        let mut inv = Inventory::empty();
        assert!(inv.add(stack(WOOD, 35)).is_none());
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        app.world_mut().resource_mut::<InventoryUi>().list = craftable_list(&inv, &[]);
        assert_eq!(
            app.world().resource::<InventoryUi>().list[0],
            WORKBENCH_RECIPE
        );
        app.world_mut().insert_resource(LocalInventory(inv));
        if shift_held {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::ShiftLeft);
        }
        let [x, y, w, h] = row_rect(0, [1920.0, 1080.0]);
        click_at(&mut app, [x + w / 2.0, y + h / 2.0]);
        let out = sent.lock().unwrap().clone();
        out
    }

    #[test]
    fn shift_click_recipe_crafts_max_and_plain_click_crafts_one() {
        let msgs = craft_click(true);
        assert!(
            matches!(msgs[..], [ClientMsg::Craft { recipe, count: 3 }] if recipe == WORKBENCH_RECIPE),
            "{msgs:?}"
        );
        let msgs = craft_click(false);
        assert!(
            matches!(msgs[..], [ClientMsg::Craft { recipe, count: 1 }] if recipe == WORKBENCH_RECIPE),
            "{msgs:?}"
        );
    }

    #[test]
    fn shift_click_bag_sells_while_shop_open() {
        let mut app = app();
        let sent = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        app.insert_resource(Net::new(Recorder(sent.clone())));
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        {
            let mut town = app.world_mut().resource_mut::<TownPanel>();
            town.npc = Some(terravibe_sim::enemy::EnemyId(7));
            town.shop = vec![(GEL, 5)];
        }
        let [x, y, sz] = cell_rect(13, [1920.0, 1080.0]);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        click_at(&mut app, [x + sz / 2.0, y + sz / 2.0]);
        let msgs = sent.lock().unwrap().clone();
        assert!(
            matches!(msgs[..], [ClientMsg::ShopSell { slot: 13 }]),
            "{msgs:?}"
        );
    }

    #[test]
    fn shift_click_bag_moves_to_chest_while_chest_open() {
        let mut app = app();
        let sent = Arc::new(Mutex::new(Vec::new()));
        app.insert_resource(Net::new(Recorder(sent.clone())));
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        app.world_mut().resource_mut::<ChestPanel>().open = Some([1, 2, 3]);
        let [x, y, sz] = cell_rect(13, [1920.0, 1080.0]);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        click_at(&mut app, [x + sz / 2.0, y + sz / 2.0]);
        let msgs = sent.lock().unwrap().clone();
        assert!(
            matches!(
                msgs[..],
                [ClientMsg::ChestMove {
                    to_chest: true,
                    slot: 13
                }]
            ),
            "{msgs:?}"
        );
    }

    #[test]
    fn slot_ghost_icons_defined_for_special_cells() {
        assert_eq!(
            ghost_silhouette_for_cell(COIN0),
            Some(GhostSilhouette::Coin(0))
        );
        assert_eq!(
            ghost_silhouette_for_cell(COIN0 + 1),
            Some(GhostSilhouette::Coin(1))
        );
        assert_eq!(
            ghost_silhouette_for_cell(COIN0 + 2),
            Some(GhostSilhouette::Coin(2))
        );
        assert_eq!(
            ghost_silhouette_for_cell(COIN0 + 3),
            Some(GhostSilhouette::Coin(3))
        );
        assert_eq!(
            ghost_silhouette_for_cell(AMMO0),
            Some(GhostSilhouette::Ammo)
        );
        assert_eq!(
            ghost_silhouette_for_cell(AMMO0 + 3),
            Some(GhostSilhouette::Ammo)
        );
        assert_eq!(
            ghost_silhouette_for_cell(ARMOR0),
            Some(GhostSilhouette::ArmorHead)
        );
        assert_eq!(
            ghost_silhouette_for_cell(ARMOR0 + 1),
            Some(GhostSilhouette::ArmorBody)
        );
        assert_eq!(
            ghost_silhouette_for_cell(ARMOR0 + 2),
            Some(GhostSilhouette::ArmorLegs)
        );
        assert_eq!(
            ghost_silhouette_for_cell(TRASH_CELL),
            Some(GhostSilhouette::Trash)
        );
        assert_eq!(ghost_silhouette_for_cell(0), None);
        assert_eq!(ghost_silhouette_for_cell(49), None);
    }

    #[test]
    fn trash_slot_mechanics_stores_and_retrieves_item() {
        let mut app = app();
        app.init_resource::<LocalHeld>();
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        let [win_w, win_h] = [1920.0, 1080.0];
        let [tx, ty, ts] = cell_rect(TRASH_CELL, [win_w, win_h]);
        let trash_center = [tx + ts / 2.0, ty + ts / 2.0];

        assert!(trash_slot_at(trash_center, [win_w, win_h]));

        // Place held item into trash
        app.world_mut()
            .insert_resource(LocalHeld(Some(stack(DIRT_BLOCK, 25))));
        click_at(&mut app, trash_center);
        assert_eq!(app.world().resource::<LocalHeld>().0, None);
        assert_eq!(
            app.world().resource::<InventoryUi>().trash,
            Some(stack(DIRT_BLOCK, 25))
        );

        // Click again with empty cursor retrieves the item
        click_at(&mut app, trash_center);
        assert_eq!(
            app.world().resource::<LocalHeld>().0,
            Some(stack(DIRT_BLOCK, 25))
        );
        assert_eq!(app.world().resource::<InventoryUi>().trash, None);
    }

    #[test]
    fn crafting_carousel_hit_testing_and_hammer_toggle() {
        let win = [1920.0, 1080.0];
        // Carousel slots are compact (~40-44px) icon slots below the inventory
        let slot0_pos = carousel_slot_rect(0, win);
        assert!(
            carousel_slot_at(
                [
                    slot0_pos[0] + slot0_pos[2] / 2.0,
                    slot0_pos[1] + slot0_pos[3] / 2.0
                ],
                win
            )
            .is_some()
        );

        let hammer_pos = hammer_button_rect(win);
        assert!(hammer_button_at(
            [
                hammer_pos[0] + hammer_pos[2] / 2.0,
                hammer_pos[1] + hammer_pos[3] / 2.0
            ],
            win
        ));
    }

    #[test]
    fn header_displays_carried_coin_total() {
        use bevy::ecs::system::RunSystemOnce;
        use std::collections::HashMap;
        use std::sync::Arc;
        use terravibe_assets::{RgbaImage, SpriteFont};

        let mut app = app();
        let font = Arc::new(SpriteFont {
            atlas: RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![[0; 4]],
            },
            glyphs: vec![],
            line_spacing: 10,
            spacing: 0.0,
            default_char: None,
        });
        app.init_resource::<Assets<Image>>();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        app.insert_resource(HotbarAssets {
            back: handle.clone(),
            back_selected: handle.clone(),
            icons: HashMap::new(),
        });
        app.insert_resource(HudFonts {
            mouse: (font.clone(), handle.clone()),
            death: (font.clone(), handle.clone()),
            stack: (font, handle),
        });

        let mut inv = Inventory::empty();
        terravibe_sim::coin::give_copper(&mut inv, 150);
        assert_eq!(terravibe_sim::coin::total_copper(&inv), 150);

        app.world_mut().run_system_once(setup).unwrap();
        app.world_mut().insert_resource(LocalInventory(inv));
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        app.world_mut().run_system_once(update_view).unwrap();

        let mut header_text = None;
        for (part, text) in app
            .world_mut()
            .query::<(&Part, &HudText)>()
            .iter(app.world())
        {
            if matches!(part, Part::InventoryHeader) {
                header_text = Some(text.text.clone());
            }
        }
        assert_eq!(header_text, Some("Inventory — 1s 50c".to_string()));
    }

    #[test]
    fn trash_slot_and_hammer_grid_hover_submit_tooltips() {
        use bevy::ecs::system::RunSystemOnce;
        use std::collections::HashMap;
        use std::sync::Arc;
        use terravibe_assets::{RgbaImage, SpriteFont};
        use terravibe_sim::item::DIRT_BLOCK;

        let mut app = app();
        app.init_resource::<super::super::tooltip::TooltipOverlay>();
        let font = Arc::new(SpriteFont {
            atlas: RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![[0; 4]],
            },
            glyphs: vec![],
            line_spacing: 20,
            spacing: 0.0,
            default_char: None,
        });
        app.init_resource::<Assets<Image>>();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        app.insert_resource(HotbarAssets {
            back: handle.clone(),
            back_selected: handle.clone(),
            icons: HashMap::new(),
        });
        app.insert_resource(HudFonts {
            mouse: (font.clone(), handle.clone()),
            death: (font.clone(), handle.clone()),
            stack: (font, handle),
        });

        app.world_mut().run_system_once(setup).unwrap();
        app.world_mut()
            .insert_resource(LocalInventory(Inventory::empty()));
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;

        // 1. Trash slot hover
        let trash_stack = ItemStack {
            id: DIRT_BLOCK,
            count: 3,
            prefix: Prefix(0),
        };
        app.world_mut().resource_mut::<InventoryUi>().trash = Some(trash_stack);
        let [tx, ty, ts] = cell_rect(TRASH_CELL, [1920.0, 1080.0]);
        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .unwrap()
            .set_cursor_position(Some(Vec2::new(tx + ts * 0.5, ty + ts * 0.5)));
        app.world_mut().run_system_once(update_view).unwrap();
        let cand = app
            .world()
            .resource::<super::super::tooltip::TooltipOverlay>()
            .candidate
            .clone()
            .expect("trash slot hover should submit tooltip");
        assert_eq!(cand.target_id, 1_000 + TRASH_CELL as u64);
        assert_eq!(cand.stack.id, DIRT_BLOCK);

        // 2. Hammer grid recipe hover
        let rid = RecipeId(0);
        {
            let mut ui = app.world_mut().resource_mut::<InventoryUi>();
            ui.hammer_grid = true;
            ui.list = vec![rid];
        }
        let [gx, gy, gw, gh] = grid_slot_rect(0, [1920.0, 1080.0]);
        app.world_mut()
            .resource_mut::<super::super::tooltip::TooltipOverlay>()
            .candidate = None;
        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .unwrap()
            .set_cursor_position(Some(Vec2::new(gx + gw * 0.5, gy + gh * 0.5)));
        app.world_mut().run_system_once(update_view).unwrap();
        let cand = app
            .world()
            .resource::<super::super::tooltip::TooltipOverlay>()
            .candidate
            .clone()
            .expect("hammer grid recipe hover should submit tooltip");
        assert_eq!(cand.target_id, 2_000 + rid.0 as u64);
        assert_eq!(cand.stack.id, recipe(rid).unwrap().output.item);
    }

    #[test]
    fn sort_does_not_touch_held_stack() {
        let mut app = app();
        let sent = Arc::new(Mutex::new(Vec::new()));
        app.insert_resource(Net::new(Recorder(sent.clone())));
        app.insert_resource(LocalHeld(Some(stack(DIRT_BLOCK, 5))));
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;

        let win = [1920.0, 1080.0];
        let s = scale(win);
        let sort_center = [
            (special_x() + STEP + 36.0 + 16.0) * s,
            (GRID[1] + 4.0 * STEP + 16.0) * s,
        ];
        assert!(sort_btn_at(sort_center, win));
        click_at(&mut app, sort_center);

        assert_eq!(
            app.world().resource::<LocalHeld>().0,
            Some(stack(DIRT_BLOCK, 5))
        );
        let msgs = sent.lock().unwrap();
        assert!(
            msgs.iter()
                .any(|m| matches!(m, ClientMsg::SortInventory { .. }))
        );
    }

    #[test]
    fn trashing_valuable_item_requires_confirmation_low_value_does_not() {
        let mut app = app();
        app.init_resource::<LocalHeld>();
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        let win = [1920.0, 1080.0];
        let [tx, ty, ts] = cell_rect(TRASH_CELL, win);
        let trash_center = [tx + ts / 2.0, ty + ts / 2.0];

        // Low-value item (< 100c) trashes immediately.
        assert!(item_data(DIRT_BLOCK).value < VALUABLE_TRASH_THRESHOLD_COPPER);
        app.world_mut()
            .insert_resource(LocalHeld(Some(stack(DIRT_BLOCK, 1))));
        click_at(&mut app, trash_center);
        assert_eq!(app.world().resource::<LocalHeld>().0, None);
        assert_eq!(
            app.world().resource::<InventoryUi>().trash,
            Some(stack(DIRT_BLOCK, 1))
        );
        assert_eq!(
            app.world().resource::<InventoryUi>().pending_trash_confirm,
            None
        );

        // High-value item (>= 100c) requires a second click to confirm.
        assert!(item_data(ANVIL_ITEM).value >= VALUABLE_TRASH_THRESHOLD_COPPER);
        app.world_mut()
            .insert_resource(LocalHeld(Some(stack(ANVIL_ITEM, 1))));
        click_at(&mut app, trash_center);
        assert_eq!(
            app.world().resource::<LocalHeld>().0,
            Some(stack(ANVIL_ITEM, 1))
        );
        assert_eq!(
            app.world().resource::<InventoryUi>().pending_trash_confirm,
            Some(stack(ANVIL_ITEM, 1))
        );
        assert_eq!(
            app.world().resource::<InventoryUi>().trash,
            Some(stack(DIRT_BLOCK, 1))
        );

        // Second click confirms and moves to trash.
        click_at(&mut app, trash_center);
        assert_eq!(app.world().resource::<LocalHeld>().0, None);
        assert_eq!(
            app.world().resource::<InventoryUi>().pending_trash_confirm,
            None
        );
        assert_eq!(
            app.world().resource::<InventoryUi>().trash,
            Some(stack(ANVIL_ITEM, 1))
        );
    }

    #[test]
    fn crafting_panel_shows_required_station_and_missing_materials() {
        let mut inv = Inventory::empty();
        inv.add(stack(WOOD, 3));
        // Workbench needs 10 Wood, By Hand. Shortfall: 7 Wood.
        let dt = detail_text(WORKBENCH_RECIPE, Some(&inv));
        assert!(dt.contains("10 Wood (missing 7)"), "actual: {dt}");
        assert!(dt.contains("Station: By Hand"), "actual: {dt}");

        // Wooden Sword needs 7 Wood and Workbench station. Shortfall: 4 Wood.
        let dt_sword = detail_text(WOODEN_SWORD_RECIPE, Some(&inv));
        assert!(
            dt_sword.contains("7 Wood (missing 4)"),
            "actual: {dt_sword}"
        );
        assert!(
            dt_sword.contains("Station: Workbench"),
            "actual: {dt_sword}"
        );

        // With sufficient materials, no missing notice is displayed.
        inv.add(stack(WOOD, 7));
        let dt_full = detail_text(WORKBENCH_RECIPE, Some(&inv));
        assert!(dt_full.contains("10 Wood"), "actual: {dt_full}");
        assert!(!dt_full.contains("missing"), "actual: {dt_full}");
    }

    #[test]
    fn recipe_search_filters_list_by_name() {
        let mut app = app();
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        let mut inv = Inventory::empty();
        inv.add(stack(WOOD, 10));
        inv.add(stack(GEL, 5));
        app.world_mut().insert_resource(LocalInventory(inv));

        // Unfiltered: shows craftable items (Workbench, Torch, etc.)
        let all = sorted_order(&app.world().resource::<LocalInventory>().0, &[]);
        let indices: Vec<usize> = all.iter().map(|id| id.0 as usize).collect();
        let matched = filter_recipes(&indices, "torch");
        assert!(matched.contains(&(TORCH_RECIPE.0 as usize)));
        assert!(!matched.contains(&(FURNACE_RECIPE.0 as usize)));

        // Setting search_query filters recipes
        app.world_mut().resource_mut::<InventoryUi>().search_query = "torch".to_string();
        let q = app.world().resource::<InventoryUi>().search_query.clone();
        let filtered = filter_recipes(&indices, &q);
        let list: Vec<RecipeId> = filtered.into_iter().map(|i| RecipeId(i as u16)).collect();
        assert!(list.contains(&TORCH_RECIPE));
        assert!(!list.contains(&FURNACE_RECIPE));
    }

    #[test]
    fn search_query_not_mutated_when_chest_or_shop_open() {
        let mut app = app();
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;
        app.world_mut().resource_mut::<InventoryUi>().search_query = "torch".to_string();

        let char_input = |c: char| KeyboardInput {
            logical_key: Key::Character(c.to_string().into()),
            key_code: KeyCode::KeyA,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };
        let backspace_input = || KeyboardInput {
            logical_key: Key::Backspace,
            key_code: KeyCode::Backspace,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };

        // 1. When chest is open, typing and backspace do not mutate search_query
        app.world_mut().resource_mut::<ChestPanel>().open = Some([1, 2, 3]);
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(char_input('x'));
        app.update();
        assert_eq!(app.world().resource::<InventoryUi>().search_query, "torch");

        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(backspace_input());
        app.update();
        assert_eq!(app.world().resource::<InventoryUi>().search_query, "torch");

        // 2. When chest is closed but shop is open, typing and backspace do not mutate search_query
        app.world_mut().resource_mut::<ChestPanel>().open = None;
        {
            let mut town = app.world_mut().resource_mut::<TownPanel>();
            town.npc = Some(terravibe_sim::enemy::EnemyId(7));
            town.shop = vec![(GEL, 5)];
        }
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(char_input('y'));
        app.update();
        assert_eq!(app.world().resource::<InventoryUi>().search_query, "torch");

        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(backspace_input());
        app.update();
        assert_eq!(app.world().resource::<InventoryUi>().search_query, "torch");

        // 3. When neither is open, typing mutates search_query
        app.world_mut().resource_mut::<TownPanel>().npc = None;
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(char_input('x'));
        app.update();
        assert_eq!(app.world().resource::<InventoryUi>().search_query, "torchx");

        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(backspace_input());
        app.update();
        assert_eq!(app.world().resource::<InventoryUi>().search_query, "torch");
    }

    #[test]
    fn vanity_slots_housing_tab_builder_toggles_and_alt_favorite() {
        use terravibe_sim::item::ArmorSlot;

        let win = [1920.0, 1080.0];
        let center_of = |r: [f32; 4]| [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
        let cell_center = |id: usize| {
            let [x, y, sz] = cell_rect(id, win);
            [x + sz / 2.0, y + sz / 2.0]
        };

        // Vanity armor & accessories sit directly left of Armor/Accessory slots.
        assert_eq!(
            slot_at(cell_center(VANITY_ARMOR0), win),
            Some(SlotRef::VanityArmor(ArmorSlot::Head))
        );
        assert_eq!(
            slot_at(cell_center(VANITY_ARMOR0 + 2), win),
            Some(SlotRef::VanityArmor(ArmorSlot::Legs))
        );
        assert_eq!(
            slot_at(cell_center(VANITY_ACC0), win),
            Some(SlotRef::VanityAccessory(0))
        );
        assert_eq!(
            slot_at(cell_center(VANITY_ACC0 + 4), win),
            Some(SlotRef::VanityAccessory(4))
        );
        assert!(cell_rect(VANITY_ARMOR0, win)[0] < cell_rect(ARMOR0, win)[0]);

        let mut app = app();
        app.init_resource::<FavoriteSlots>();
        app.init_resource::<RulerEnabled>();
        app.init_resource::<BuildGridEnabled>();
        app.world_mut().resource_mut::<InventoryOpen>().0 = true;

        // Toggle Ruler & Build Grid via their buttons.
        click_at(&mut app, center_of(ruler_btn_rect(win)));
        assert!(app.world().resource::<RulerEnabled>().0);
        click_at(&mut app, center_of(build_grid_btn_rect(win)));
        assert!(app.world().resource::<BuildGridEnabled>().0);

        // Switch Equip -> Housing tab -> Equip tab.
        click_at(&mut app, center_of(equip_tab_rect(1, win)));
        assert_eq!(
            app.world().resource::<InventoryUi>().equip_tab,
            EquipTab::Housing
        );
        click_at(&mut app, center_of(equip_tab_rect(0, win)));
        assert_eq!(
            app.world().resource::<InventoryUi>().equip_tab,
            EquipTab::Equip
        );

        // Alt + Left Click on bag slot 0 toggles FavoriteSlots.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::AltLeft);
        click_at(&mut app, cell_center(0));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::AltLeft);
        assert!(app.world().resource::<FavoriteSlots>().0[0]);

        // Favorited slot 0 blocks Sort when occupied.
        app.world_mut().resource_mut::<LocalInventory>().0.slots[0] = Some(stack(DIRT_BLOCK, 1));
        let [sx, sy] = [
            (special_x() + STEP + 36.0 + 16.0) * scale(win),
            (GRID[1] + 4.0 * STEP + 16.0) * scale(win),
        ];
        click_at(&mut app, [sx, sy]);
        assert!(
            app.world()
                .resource::<InventoryUi>()
                .feedback
                .contains("favorited slots protected")
        );
    }

    #[test]
    fn housing_panel_layout_fits_without_clipping_and_wraps_status() {
        let win = [1920.0, 1080.0];
        let card = equip_card_rect(win);
        assert!(
            card[2] >= 220.0,
            "equip_card_rect width {} must be >= 220px",
            card[2]
        );
        assert!(card[0] > 0.0 && card[0] + card[2] < win[0]);

        let t0 = equip_tab_rect(0, win);
        let t1 = equip_tab_rect(1, win);
        assert!(t0[0] >= card[0] - 0.01);
        assert!(t0[0] + t0[2] < t1[0]);
        assert!(t1[0] + t1[2] <= card[0] + card[2] + 0.01);

        let q = housing_query_rect(win);
        let st = housing_status_rect(win);
        assert!(q[0] >= card[0] && q[0] + q[2] <= card[0] + card[2]);
        assert!(st[0] >= card[0] && st[0] + st[2] <= card[0] + card[2]);
        assert!(
            q[1] + q[3] < st[1],
            "query button must not overlap status box"
        );

        let mut prev_bottom = st[1] + st[3];
        for i in 0..HOUSING_NPCS.len() {
            let r = housing_npc_rect(i, win);
            assert!(r[0] >= card[0] && r[0] + r[2] <= card[0] + card[2]);
            assert!(
                r[1] > prev_bottom,
                "npc row {i} must sit below previous element"
            );
            assert!(
                r[1] + r[3] <= card[1] + card[3],
                "npc row {i} must fit inside card"
            );
            prev_bottom = r[1] + r[3];
        }

        for code in 0..=8 {
            let raw = crate::hud::town::house_text(code == 0, code);
            let wrapped = wrap_housing_text(raw, 24);
            for line in wrapped.lines() {
                assert!(
                    line.chars().count() <= 24,
                    "wrapped line exceeded 24 chars: '{line}'"
                );
            }
            assert_eq!(
                wrapped.split_whitespace().collect::<Vec<_>>(),
                raw.split_whitespace().collect::<Vec<_>>()
            );
        }
    }
}
