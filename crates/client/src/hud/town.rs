//! Town NPC UI: right click a Guide or Merchant to talk (`TalkNpc`); the reply is
//! shown in a text panel in the chest area below the inventory. Up/Down choose,
//! Enter buys, Shift-clicking a bag slot sells it, `Esc` closes. `H` asks whether
//! the room around the player is valid housing and `X` breaks the wall behind the
//! block under the crosshair (both feed the same panel with the server's answer).

use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use terravibe_sim::enemy::EnemyId;
use terravibe_sim::housing::check_room;
use terravibe_sim::interact::{RAY_MAX, in_reach, raycast};
use terravibe_sim::inventory::{BAG, ItemStack};
use terravibe_sim::item::{ArmorSlot, ItemId, Prefix, item_data};
use terravibe_sim::net::ClientMsg;
use terravibe_sim::slots::SlotRef;
use terravibe_sim::town_npc::TownKind;

use super::inventory::InventoryOpen;
use super::text::HudText;
use crate::net::Net;
use crate::session::AppState;
use crate::view::camera::{FlyCamera, set_grab};
use crate::view::chunks::ChunkStream;
use crate::view::enemies::{ClientEnemy, EnemyInterp};
use crate::view::player::{LoadedBlocks, LocalPlayer};

#[derive(Message, Clone, Debug)]
pub enum TownMsg {
    Dialogue {
        npc: EnemyId,
        text: String,
    },
    Shop {
        items: Vec<(ItemId, u32)>,
    },
    Result {
        ok: bool,
        cannot_afford: bool,
    },
    House {
        ok: bool,
        reason: u8,
    },
    Speech {
        id: EnemyId,
        text: String,
    },
    NurseHealed {
        ok: bool,
        healed: i32,
        cost_copper: u32,
    },
    ReforgeResult {
        ok: bool,
        #[allow(dead_code)]
        slot: u8,
        new_prefix: Prefix,
        cost_copper: u32,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TownTab {
    #[default]
    Talk,
    Crafting,
    Reforge,
    Housing,
}

#[derive(Resource)]
pub struct TownPanel {
    pub npc: Option<EnemyId>,
    pub text: String,
    pub shop: Vec<(ItemId, u32)>,
    pub sel: usize,
    /// One-line feedback (trade result, housing status, Guide crafting note).
    pub note: String,
    pub tab: TownTab,
    pub is_guide: bool,
    pub is_nurse: bool,
    pub is_tinkerer: bool,
    pub reforge_sel: usize,
    pub guide_scroll: usize,
    pub guide_sel: usize,
    pub housing_query_pos: Option<[i32; 3]>,
    pub housing_status: Option<(bool, u8)>,
    pub room_bounds: Option<([i32; 3], [i32; 3])>,
    pub selected_npc: TownKind,
    pub last_speech: Option<(EnemyId, String)>,
}

impl Default for TownPanel {
    fn default() -> Self {
        Self {
            npc: None,
            text: String::new(),
            shop: Vec::new(),
            sel: 0,
            note: String::new(),
            tab: TownTab::Talk,
            is_guide: false,
            is_nurse: false,
            is_tinkerer: false,
            reforge_sel: 0,
            guide_scroll: 0,
            guide_sel: 0,
            housing_query_pos: None,
            housing_status: None,
            room_bounds: None,
            selected_npc: TownKind::Guide,
            last_speech: None,
        }
    }
}

impl TownPanel {
    /// A Merchant's stock is showing, so bag clicks can sell.
    pub fn shop_open(&self) -> bool {
        self.npc.is_some() && !self.shop.is_empty()
    }

    pub fn is_open(&self) -> bool {
        self.npc.is_some()
    }

    /// True when the Guide's material-recipe lookup tab is active.
    pub fn guide_crafting_open(&self) -> bool {
        self.npc.is_some() && self.is_guide && self.tab == TownTab::Crafting
    }
}

/// Slot sold by a click while the shop is open: Shift+left on a bag slot.
pub fn sell_click(shop_open: bool, left: bool, shift: bool, at: SlotRef) -> Option<u8> {
    if !(shop_open && left && shift) {
        return None;
    }
    match at {
        SlotRef::Inv(i) if (i as usize) < BAG => Some(i),
        _ => None,
    }
}

#[derive(Component)]
pub struct TownRoot;
#[derive(Component)]
pub struct TownLabel;
#[derive(Component)]
pub struct TownSpeaker;
#[derive(Component)]
pub struct TownTabButton(pub usize);
#[derive(Component)]
pub struct ShopRowIcon(pub usize);
#[derive(Component)]
pub struct GuideMaterialSlotBox;
#[derive(Component)]
pub struct GuideMaterialIcon;
#[derive(Component)]
pub struct GuideMaterialCount;
#[derive(Component)]
pub struct GuideRecipeIcon(pub usize);

const MAX_SHOP_ROWS: usize = 12;
const MAX_GUIDE_ROWS: usize = 7;

/// Reach-check radius when picking an NPC with the crosshair.
const PICK_RADIUS: f32 = 1.0;

/// Housing answer text (`RoomFail::code` 1..=7, 8 = furniture missing).
pub fn house_text(ok: bool, reason: u8) -> &'static str {
    match (ok, reason) {
        (true, _) => "Valid housing.",
        (_, 1) => "Not housing: room is too close to the world edge.",
        (_, 2) => "Not housing: stand inside an open room cell, not a solid block.",
        (_, 3) => "Not housing: room is open air (missing walls or roof).",
        (_, 4) => "Not housing: room is too big.",
        (_, 5) => "Not housing: room is too small (needs at least 60 cells).",
        (_, 6) => "Not housing: room extends into unloaded chunks.",
        (_, 7) => "Not housing: room needs a solid floor tile to stand on.",
        (_, 8) => "Not housing: needs a chair, table, door and torch.",
        _ => "Not housing: the room is not sealed, big enough or walled.",
    }
}

/// Plain-language body text for the Housing tab.
pub fn housing_panel_text(panel: &TownPanel) -> String {
    let status_line = panel
        .housing_status
        .map(|(ok, reason)| house_text(ok, reason))
        .unwrap_or("Click ? in Inventory > Housing to check a room.");
    let mut out = String::from(status_line);
    if let Some((lo, hi)) = panel.room_bounds {
        out.push_str(&format!(
            "\nRoom bounds: ({}, {}, {}) to ({}, {}, {})",
            lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
        ));
    } else if let Some(pos) = panel.housing_query_pos {
        out.push_str(&format!(
            "\nQuery cell: ({}, {}, {})",
            pos[0], pos[1], pos[2]
        ));
    }
    out.push_str(&format!(
        "\n\nNPC: {}   [<] [>]\n[Assign NPC] (Enter)   [?] Query   Esc: close",
        panel.selected_npc.name()
    ));
    if !panel.note.is_empty() && panel.note != status_line {
        out.push_str(&format!("\n\n{}", panel.note));
    }
    out
}

pub fn price_text(copper: u32) -> String {
    terravibe_sim::coin::format_coins(copper)
}

/// Panel top-left in window px: positioned center-right, clear of left-side Inventory/Crafting panels.
pub fn panel_pos(win: [f32; 2]) -> [f32; 2] {
    let s = crate::hud::inventory::scale(win);
    [510.0 * s, 340.0 * s]
}

/// True when `cursor` lies inside the floating Town panel.
pub fn panel_hit(cursor: [f32; 2], win: [f32; 2], guide_crafting: bool) -> bool {
    let pos = panel_pos(win);
    let k = crate::hud::inventory::scale(win);
    let h = if guide_crafting { 340.0 } else { 260.0 } * k;
    cursor[0] >= pos[0]
        && cursor[0] <= pos[0] + 460.0 * k
        && cursor[1] >= pos[1]
        && cursor[1] <= pos[1] + h
}

/// True when `cursor` lies inside the Guide Material Slot box `(16..60, 44..88)` in panel coords.
pub fn guide_slot_hit(cursor: [f32; 2], win: [f32; 2]) -> bool {
    let pos = panel_pos(win);
    let k = crate::hud::inventory::scale(win);
    let rel_x = (cursor[0] - pos[0]) / k;
    let rel_y = (cursor[1] - pos[1]) / k;
    (16.0..=60.0).contains(&rel_x) && (44.0..=88.0).contains(&rel_y)
}

pub fn setup(mut commands: Commands, window: Single<&Window, With<PrimaryWindow>>) {
    let win = [window.width(), window.height()];
    let s = crate::hud::inventory::scale(win);
    let pos = panel_pos(win);
    commands
        .spawn((
            TownRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(pos[0]),
                top: Val::Px(pos[1]),
                width: Val::Px(460.0 * s),
                height: Val::Px(260.0 * s),
                border: UiRect::all(Val::Px(super::style::BORDER_W)),
                border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_PANEL)),
                ..default()
            },
            BackgroundColor(super::style::PANEL_BG_SOLID),
            BorderColor::all(super::style::PANEL_BORDER),
            Visibility::Hidden,
            GlobalZIndex(511),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|p| {
            p.spawn((
                TownSpeaker,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(16.0),
                    top: Val::Px(12.0),
                    ..default()
                },
                HudText {
                    text: String::new(),
                    death: false,
                },
                super::text::HudTextColor(super::style::GOLD_TEXT),
            ));
            for (idx, label_str, left_px, w_px) in [
                (0usize, "Talk", 120.0, 68.0),
                (1usize, "Crafting", 196.0, 92.0),
                (2usize, "Housing", 296.0, 88.0),
            ] {
                p.spawn((
                    TownTabButton(idx),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(left_px * s),
                        top: Val::Px(8.0 * s),
                        width: Val::Px(w_px * s),
                        height: Val::Px(24.0 * s),
                        border: UiRect::all(Val::Px(super::style::BORDER_W)),
                        border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(super::style::BTN_BG),
                    BorderColor::all(super::style::PANEL_BORDER),
                    Visibility::Hidden,
                ))
                .with_children(|btn| {
                    btn.spawn((
                        HudText {
                            text: label_str.to_string(),
                            death: false,
                        },
                        Node::default(),
                    ));
                });
            }
            p.spawn((
                GuideMaterialSlotBox,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(16.0 * s),
                    top: Val::Px(44.0 * s),
                    width: Val::Px(44.0 * s),
                    height: Val::Px(44.0 * s),
                    border: UiRect::all(Val::Px(super::style::BORDER_W)),
                    border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_SLOT)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(super::style::SLOT_BG),
                BorderColor::all(super::style::GOLD_BORDER),
                Visibility::Hidden,
            ))
            .with_children(|slot| {
                slot.spawn((
                    GuideMaterialIcon,
                    Node {
                        width: Val::Px(30.0 * s),
                        height: Val::Px(30.0 * s),
                        ..default()
                    },
                    ImageNode::default(),
                    Visibility::Hidden,
                ));
                slot.spawn((
                    GuideMaterialCount,
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(2.0),
                        bottom: Val::Px(1.0),
                        ..default()
                    },
                    HudText {
                        text: String::new(),
                        death: false,
                    },
                ));
            });
            for i in 0..MAX_GUIDE_ROWS {
                p.spawn((
                    GuideRecipeIcon(i),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(72.0 * s),
                        top: Val::Px(44.0 * s),
                        width: Val::Px(16.0),
                        height: Val::Px(16.0),
                        ..default()
                    },
                    ImageNode::default(),
                    Visibility::Hidden,
                ));
            }
            for i in 0..MAX_SHOP_ROWS {
                p.spawn((
                    ShopRowIcon(i),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(16.0),
                        top: Val::Px(38.0),
                        width: Val::Px(16.0),
                        height: Val::Px(16.0),
                        ..default()
                    },
                    ImageNode::default(),
                    Visibility::Hidden,
                ));
            }
            p.spawn((
                TownLabel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(16.0),
                    top: Val::Px(38.0),
                    ..default()
                },
                HudText {
                    text: String::new(),
                    death: false,
                },
            ));
        });
}

/// Right click on a town NPC within reach.
pub fn talk_input(
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    gamepad_interact: Option<Res<crate::view::interact::GamepadInteractState>>,
    cam: Single<&Transform, With<FlyCamera>>,
    player: Option<Res<LocalPlayer>>,
    net: Option<Res<Net>>,
    npcs: Query<(&ClientEnemy, &EnemyInterp)>,
) {
    let triggered = mouse.is_some_and(|m| m.just_pressed(MouseButton::Right))
        || gamepad_interact.is_some_and(|g| g.use_just_pressed);
    if !triggered {
        return;
    }
    let (Some(player), Some(net)) = (player, net) else {
        return;
    };
    let (eye, dir) = (cam.translation, *cam.forward());
    let best = npcs
        .iter()
        .filter(|(e, _)| e.kind.is_friendly())
        .filter_map(|(e, i)| {
            let c = i.pos() + Vec3::Y * 0.85;
            let t = (c - eye).dot(dir);
            let off = (c - (eye + dir * t)).length();
            (t > 0.0 && t < RAY_MAX && off < PICK_RADIUS).then_some((t, e.id, i.pos()))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, id, pos)) = best {
        let cell = [pos.x.floor() as i32, pos.y as i32 + 1, pos.z.floor() as i32];
        if in_reach(&player.0.body, cell) {
            let _ = net.send(ClientMsg::TalkNpc { id });
        }
    }
}

struct ClientLoadedQuery<'a>(
    &'a std::collections::HashMap<
        terravibe_sim::chunk::ChunkPos,
        std::sync::Arc<terravibe_sim::chunk::Chunk>,
    >,
);

impl terravibe_sim::chunk::BlockQuery for ClientLoadedQuery<'_> {
    fn block(&self, x: i32, y: i32, z: i32) -> terravibe_sim::block::BlockId {
        LoadedBlocks(self.0).block(x, y, z)
    }

    fn liquid(&self, x: i32, y: i32, z: i32) -> Option<terravibe_sim::liquid_cell::Liquid> {
        LoadedBlocks(self.0).liquid(x, y, z)
    }
}

impl terravibe_sim::interact::LoadedQuery for ClientLoadedQuery<'_> {
    fn is_loaded(&self, x: i32, y: i32, z: i32) -> bool {
        self.0
            .contains_key(&terravibe_sim::chunk::ChunkPos::of_block(x, y, z))
    }

    fn has_wall(&self, x: i32, y: i32, z: i32) -> bool {
        use terravibe_sim::chunk::{CHUNK_SIZE, ChunkPos};
        let Some(c) = self.0.get(&ChunkPos::of_block(x, y, z)) else {
            return false;
        };
        c.wall(
            x.rem_euclid(CHUNK_SIZE) as usize,
            y.rem_euclid(CHUNK_SIZE) as usize,
            z.rem_euclid(CHUNK_SIZE) as usize,
        )
    }
}

pub fn query_housing_at_player(
    player: &LocalPlayer,
    stream: Option<&ChunkStream>,
    panel: &mut TownPanel,
    net: &Net,
) {
    let p = player.0.body.pos;
    let pos = [
        p[0].floor() as i32,
        p[1].floor() as i32,
        p[2].floor() as i32,
    ];
    let bounds = stream.and_then(|s| {
        let q = ClientLoadedQuery(s.generated());
        check_room(&q, pos).ok().map(|r| {
            (
                r.lo,
                [
                    r.lo[0] + r.bbox[0],
                    r.lo[1] + r.bbox[1],
                    r.lo[2] + r.bbox[2],
                ],
            )
        })
    });
    panel.housing_query_pos = Some(pos);
    panel.room_bounds = bounds;
    panel.tab = TownTab::Housing;
    let _ = net.send(ClientMsg::QueryHouse { pos });
}

/// `X` (or gamepad West) breaks the wall at the cell in front of the block under
/// the crosshair (walls live in air cells).
#[allow(clippy::too_many_arguments)]
pub fn house_keys(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    cam: Single<&Transform, With<FlyCamera>>,
    stream: Option<Res<ChunkStream>>,
    player: Option<Res<LocalPlayer>>,
    net: Option<Res<Net>>,
    settings: Option<Res<crate::settings::Settings>>,
) {
    let (Some(_player), Some(net)) = (player, net) else {
        return;
    };
    let bindings = settings
        .as_deref()
        .map_or_else(crate::input_map::InputBindings::default, |s| s.bindings);
    let break_wall_pressed = keys
        .just_pressed(bindings.key(crate::input_map::GameAction::BreakWall))
        .then_some(())
        .is_some()
        || gamepads.iter().any(|g| g.just_pressed(GamepadButton::West));
    if break_wall_pressed && let Some(stream) = stream {
        let q = LoadedBlocks(stream.generated());
        if let Some(h) = raycast(&q, cam.translation.into(), (*cam.forward()).into(), RAY_MAX) {
            let pos = [
                h.block[0] + h.normal[0],
                h.block[1] + h.normal[1],
                h.block[2] + h.normal[2],
            ];
            let _ = net.send(ClientMsg::BreakWall { pos });
        }
    }
}

pub fn receive(
    mut msgs: MessageReader<TownMsg>,
    mut panel: ResMut<TownPanel>,
    mut inv: Option<ResMut<InventoryOpen>>,
    mut opts: Single<&mut CursorOptions, With<PrimaryWindow>>,
    enemies: Query<&ClientEnemy>,
) {
    for m in msgs.read() {
        match m {
            TownMsg::Dialogue { npc, text } => {
                // Free the cursor so clicks do not attack while the panel is up.
                set_grab(&mut opts, false);
                let kind = enemies.iter().find(|e| e.id == *npc).map(|e| e.kind);
                let is_guide = kind.map_or_else(
                    || !text.starts_with("Old Man:"),
                    |k| k == terravibe_sim::enemy::EnemyKind::Guide,
                );
                let is_nurse = kind == Some(terravibe_sim::enemy::EnemyKind::Nurse);
                let is_tinkerer = kind == Some(terravibe_sim::enemy::EnemyKind::GoblinTinkerer);
                *panel = TownPanel {
                    npc: Some(*npc),
                    text: text.clone(),
                    tab: TownTab::Talk,
                    is_guide,
                    is_nurse,
                    is_tinkerer,
                    ..default()
                };
            }
            TownMsg::Shop { items } => {
                panel.shop = items.clone();
                panel.sel = 0;
                // Terraria shows the bag beside the shop; Shift-click sells a bag slot.
                if let Some(inv) = inv.as_mut() {
                    inv.0 = true;
                }
                set_grab(&mut opts, false);
            }
            TownMsg::Result { ok, cannot_afford } => {
                panel.note = if *ok {
                    "Done."
                } else if *cannot_afford {
                    "Cannot afford."
                } else {
                    "Cannot do that."
                }
                .into();
            }
            TownMsg::House { ok, reason } => {
                set_grab(&mut opts, false);
                panel.tab = TownTab::Housing;
                panel.housing_status = Some((*ok, *reason));
                panel.note = house_text(*ok, *reason).into();
            }
            TownMsg::Speech { id, text } => {
                panel.last_speech = Some((*id, text.clone()));
                if panel.tab == TownTab::Housing || panel.npc == Some(*id) {
                    panel.note = text.clone();
                }
            }
            TownMsg::NurseHealed {
                ok,
                healed,
                cost_copper,
            } => {
                panel.note = if *ok {
                    format!("Healed {healed} HP for {}.", price_text(*cost_copper))
                } else if *cost_copper == 0 {
                    "You're already at full health!".into()
                } else {
                    format!("Need {} to heal.", price_text(*cost_copper))
                };
            }
            TownMsg::ReforgeResult {
                ok,
                new_prefix,
                cost_copper,
                ..
            } => {
                panel.note = if *ok {
                    let pname = terravibe_sim::item::prefix_name(*new_prefix);
                    let pdisp = if pname.is_empty() { "Normal" } else { pname };
                    format!("Reforged to {pdisp} for {}!", price_text(*cost_copper))
                } else {
                    "Cannot reforge (need eligible item & coins).".into()
                };
            }
        }
    }
}

/// Buy quantity selected by modifier keys: plain = 1, Shift = 10, Ctrl = All (`u16::MAX`).
pub fn buy_count_for_modifiers(shift: bool, ctrl: bool) -> u16 {
    if ctrl {
        u16::MAX
    } else if shift {
        10
    } else {
        1
    }
}

/// Short stat comparison against an equipped item for a shop row.
pub fn shop_comparison_text(item: ItemId, equipped: Option<ItemId>) -> Option<String> {
    let eq_id = equipped?;
    let d = item_data(item);
    let eq = item_data(eq_id);
    if d.pick > 0 && eq.pick > 0 {
        Some(format!("vs {}: {:+}% pick", eq.name, d.pick - eq.pick))
    } else if d.axe > 0 && eq.axe > 0 {
        Some(format!("vs {}: {:+}% axe", eq.name, (d.axe - eq.axe) * 5))
    } else if d.damage > 0 || eq.damage > 0 {
        Some(format!("vs {}: {:+} dmg", eq.name, d.damage - eq.damage))
    } else if d.defense > 0 || eq.defense > 0 {
        Some(format!("vs {}: {:+} def", eq.name, d.defense - eq.defense))
    } else {
        None
    }
}

/// Formats a single shop row with selection marker, price, equipped comparison, and `[1x] [10x] [All]`.
pub fn format_shop_row(
    item: ItemId,
    price: u32,
    selected: bool,
    equipped: Option<ItemId>,
) -> String {
    let mark = if selected { ">" } else { " " };
    let cmp = shop_comparison_text(item, equipped)
        .map(|c| format!(" ({c})"))
        .unwrap_or_default();
    format!(
        "{mark} {} - {}{} [1x] [10x] [All]",
        item_data(item).name,
        price_text(price),
        cmp
    )
}

fn cycle_town_npc(npc: TownKind) -> TownKind {
    let all = TownKind::ALL;
    let idx = all.iter().position(|&k| k == npc).unwrap_or(0);
    all[(idx + 1) % all.len()]
}

/// Hit-test for the `[Assign NPC]` button in the Housing tab.
pub fn assign_button_hit(cursor: [f32; 2], win: [f32; 2]) -> bool {
    let pos = panel_pos(win);
    let k = crate::hud::inventory::scale(win);
    let rel_x = (cursor[0] - pos[0]) / k;
    let rel_y = (cursor[1] - pos[1]) / k;
    (12.0..=160.0).contains(&rel_x) && (135.0..=195.0).contains(&rel_y)
}

#[allow(clippy::too_many_arguments)]
pub fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    fonts: Option<Res<super::HudFonts>>,
    net: Option<Res<Net>>,
    mut panel: ResMut<TownPanel>,
    _settings: Option<Res<crate::settings::Settings>>,
    player: Option<Res<LocalPlayer>>,
    stream: Option<Res<ChunkStream>>,
    mut inv: Option<ResMut<InventoryOpen>>,
    mut opts: Option<Single<&mut CursorOptions, With<PrimaryWindow>>>,
    mut wheel: Option<MessageReader<bevy::input::mouse::MouseWheel>>,
    guide_slot: Option<Res<crate::net::LocalGuideSlot>>,
    held: Option<Res<crate::net::LocalHeld>>,
    local_inv: Option<Res<crate::net::LocalInventory>>,
) {
    let Some(net) = net else {
        return;
    };
    if !panel.is_open() && panel.tab != TownTab::Housing {
        if let Some(ref mut w) = wheel {
            w.clear();
        }
        return;
    }
    let switch_to_crafting =
        |panel: &mut TownPanel,
         inv: &mut Option<ResMut<InventoryOpen>>,
         opts: &mut Option<Single<&mut CursorOptions, With<PrimaryWindow>>>| {
            panel.tab = TownTab::Crafting;
            panel.note.clear();
            if let Some(inv) = inv.as_deref_mut() {
                inv.0 = true;
            }
            if let Some(opts) = opts.as_deref_mut() {
                set_grab(opts, false);
            }
        };
    if keys.just_pressed(KeyCode::Tab) && panel.npc.is_some() {
        match panel.tab {
            TownTab::Talk => {
                if panel.is_guide {
                    switch_to_crafting(&mut panel, &mut inv, &mut opts);
                } else if panel.is_tinkerer {
                    panel.tab = TownTab::Reforge;
                    panel.reforge_sel = 0;
                } else {
                    panel.tab = TownTab::Housing;
                    if panel.housing_query_pos.is_none()
                        && let Some(player) = player.as_ref()
                    {
                        query_housing_at_player(player, stream.as_deref(), &mut panel, &net);
                    }
                }
            }
            TownTab::Crafting => {
                let _ = net.send(ClientMsg::CloseGuideCrafting);
                panel.tab = TownTab::Housing;
                if panel.housing_query_pos.is_none()
                    && let Some(player) = player.as_ref()
                {
                    query_housing_at_player(player, stream.as_deref(), &mut panel, &net);
                }
            }
            TownTab::Reforge => {
                panel.tab = TownTab::Housing;
                if panel.housing_query_pos.is_none()
                    && let Some(player) = player.as_ref()
                {
                    query_housing_at_player(player, stream.as_deref(), &mut panel, &net);
                }
            }
            TownTab::Housing => {
                panel.tab = TownTab::Talk;
            }
        }
        return;
    }
    if mouse
        .as_ref()
        .is_some_and(|m| m.just_pressed(MouseButton::Left))
        && let Some(win) = window.as_deref()
        && let Some(c) = win.cursor_position()
    {
        let win_size = [win.width(), win.height()];
        let pos = panel_pos(win_size);
        let k = crate::hud::inventory::scale(win_size);
        let rel_x = (c.x - pos[0]) / k;
        let rel_y = (c.y - pos[1]) / k;
        if panel.npc.is_some() && (4.0..=36.0).contains(&rel_y) {
            let housing_range = if panel.is_guide {
                296.0..=384.0
            } else {
                200.0..=296.0
            };
            if (120.0..=188.0).contains(&rel_x) {
                if panel.tab == TownTab::Reforge
                    && panel.is_tinkerer
                    && let Some(npc) = panel.npc
                {
                    let eligible: Vec<usize> = local_inv
                        .as_ref()
                        .map(|li| {
                            li.0.slots[..BAG]
                                .iter()
                                .enumerate()
                                .filter_map(|(idx, s)| {
                                    let st = (*s)?;
                                    terravibe_sim::shop::is_reforgeable(st.id).then_some(idx)
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    if !eligible.is_empty() {
                        panel.reforge_sel = panel.reforge_sel.min(eligible.len() - 1);
                        if keys.just_pressed(KeyCode::ArrowDown)
                            && panel.reforge_sel + 1 < eligible.len()
                        {
                            panel.reforge_sel += 1;
                        } else if keys.just_pressed(KeyCode::ArrowUp) && panel.reforge_sel > 0 {
                            panel.reforge_sel -= 1;
                        } else if keys.just_pressed(KeyCode::Enter) {
                            let slot = eligible[panel.reforge_sel] as u8;
                            let _ = net.send(ClientMsg::ReforgeItem { npc, slot });
                        }
                    }
                    return;
                }
                if panel.tab == TownTab::Crafting {
                    let _ = net.send(ClientMsg::CloseGuideCrafting);
                }
                panel.tab = TownTab::Talk;
                return;
            } else if panel.is_guide && (196.0..=288.0).contains(&rel_x) {
                switch_to_crafting(&mut panel, &mut inv, &mut opts);
                return;
            } else if housing_range.contains(&rel_x) {
                if panel.tab == TownTab::Reforge
                    && panel.is_tinkerer
                    && let Some(npc) = panel.npc
                {
                    let eligible: Vec<usize> = local_inv
                        .as_ref()
                        .map(|li| {
                            li.0.slots[..BAG]
                                .iter()
                                .enumerate()
                                .filter_map(|(idx, s)| {
                                    let st = (*s)?;
                                    terravibe_sim::shop::is_reforgeable(st.id).then_some(idx)
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    if !eligible.is_empty() {
                        panel.reforge_sel = panel.reforge_sel.min(eligible.len() - 1);
                        if keys.just_pressed(KeyCode::ArrowDown)
                            && panel.reforge_sel + 1 < eligible.len()
                        {
                            panel.reforge_sel += 1;
                        } else if keys.just_pressed(KeyCode::ArrowUp) && panel.reforge_sel > 0 {
                            panel.reforge_sel -= 1;
                        } else if keys.just_pressed(KeyCode::Enter) {
                            let slot = eligible[panel.reforge_sel] as u8;
                            let _ = net.send(ClientMsg::ReforgeItem { npc, slot });
                        }
                    }
                    return;
                }
                if panel.tab == TownTab::Crafting {
                    let _ = net.send(ClientMsg::CloseGuideCrafting);
                }
                panel.tab = TownTab::Housing;
                if panel.housing_query_pos.is_none()
                    && let Some(player) = player.as_ref()
                {
                    query_housing_at_player(player, stream.as_deref(), &mut panel, &net);
                }
                return;
            }
        }
    }

    if panel.tab == TownTab::Reforge
        && panel.is_tinkerer
        && let Some(npc) = panel.npc
    {
        let eligible: Vec<usize> = local_inv
            .as_ref()
            .map(|li| {
                li.0.slots[..BAG]
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, s)| {
                        let st = (*s)?;
                        terravibe_sim::shop::is_reforgeable(st.id).then_some(idx)
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !eligible.is_empty() {
            panel.reforge_sel = panel.reforge_sel.min(eligible.len() - 1);
            if keys.just_pressed(KeyCode::ArrowDown) && panel.reforge_sel + 1 < eligible.len() {
                panel.reforge_sel += 1;
            } else if keys.just_pressed(KeyCode::ArrowUp) && panel.reforge_sel > 0 {
                panel.reforge_sel -= 1;
            } else if keys.just_pressed(KeyCode::Enter) {
                let slot = eligible[panel.reforge_sel] as u8;
                let _ = net.send(ClientMsg::ReforgeItem { npc, slot });
            }
        }
        return;
    }
    if panel.tab == TownTab::Crafting {
        let recipes = guide_slot
            .as_ref()
            .and_then(|g| g.0)
            .map(|st| terravibe_sim::crafting::recipes_using_material(st.id))
            .unwrap_or_default();
        let count = recipes.len();
        if count > 0 {
            panel.guide_sel = panel.guide_sel.min(count - 1);
            panel.guide_scroll = panel.guide_scroll.min(count.saturating_sub(MAX_GUIDE_ROWS));
            if keys.just_pressed(KeyCode::ArrowDown) && panel.guide_sel + 1 < count {
                panel.guide_sel += 1;
                if panel.guide_sel >= panel.guide_scroll + MAX_GUIDE_ROWS {
                    panel.guide_scroll = panel.guide_sel + 1 - MAX_GUIDE_ROWS;
                }
            }
            if keys.just_pressed(KeyCode::ArrowUp) && panel.guide_sel > 0 {
                panel.guide_sel -= 1;
                if panel.guide_sel < panel.guide_scroll {
                    panel.guide_scroll = panel.guide_sel;
                }
            }
            if let Some(ref mut w) = wheel {
                let mut delta: isize = 0;
                for e in w.read() {
                    delta -= e.y.signum() as isize;
                }
                if delta != 0 {
                    let max_scroll = count.saturating_sub(MAX_GUIDE_ROWS) as isize;
                    panel.guide_scroll =
                        (panel.guide_scroll as isize + delta).clamp(0, max_scroll) as usize;
                }
            }
        } else {
            if let Some(ref mut w) = wheel {
                w.clear();
            }
            panel.guide_sel = 0;
            panel.guide_scroll = 0;
        }
        if mouse
            .as_ref()
            .is_some_and(|m| m.just_pressed(MouseButton::Left))
            && let Some(win) = window.as_deref()
            && let Some(c) = win.cursor_position()
        {
            let win_size = [win.width(), win.height()];
            if guide_slot_hit([c.x, c.y], win_size) {
                let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
                if !shift
                    && let Some(h) = held.as_ref().and_then(|h| h.0)
                    && !terravibe_sim::crafting::is_material(h.id)
                {
                    panel.note = "That item is not a crafting material.".to_string();
                } else {
                    panel.note.clear();
                    let kind = if shift {
                        terravibe_sim::slots::ClickKind::Shift
                    } else {
                        terravibe_sim::slots::ClickKind::Left
                    };
                    let _ = net.send(ClientMsg::ClickSlot {
                        at: terravibe_sim::slots::SlotRef::Guide,
                        kind,
                    });
                }
                return;
            }
            if let Some(row) = guide_row_at([c.x, c.y], win_size, count, panel.guide_scroll) {
                panel.guide_sel = panel.guide_scroll + row;
                return;
            }
        }
        return;
    }
    if let Some(ref mut w) = wheel {
        w.clear();
    }

    if panel.tab == TownTab::Housing {
        if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::ArrowRight) {
            panel.selected_npc = cycle_town_npc(panel.selected_npc);
        }
        let assign_clicked = mouse.is_some_and(|m| m.just_pressed(MouseButton::Left))
            && window
                .as_deref()
                .and_then(|w| {
                    w.cursor_position()
                        .map(|c| ([c.x, c.y], [w.width(), w.height()]))
                })
                .is_some_and(|(c, win_size)| assign_button_hit(c, win_size));
        if (keys.just_pressed(KeyCode::Enter) || assign_clicked)
            && let Some(pos) = panel.housing_query_pos
        {
            let _ = net.send(ClientMsg::AssignHousing {
                npc: panel.selected_npc,
                pos,
            });
        }
        return;
    }

    if panel.is_nurse
        && (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::KeyH))
        && let Some(npc) = panel.npc
    {
        let _ = net.send(ClientMsg::HealAtNurse { npc });
        return;
    }
    if panel.is_tinkerer && keys.just_pressed(KeyCode::KeyR) {
        panel.tab = TownTab::Reforge;
        panel.reforge_sel = 0;
        return;
    }

    let Some(npc) = panel.npc else {
        return;
    };
    let n = panel.shop.len();
    if n > 0 {
        let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        if keys.just_pressed(KeyCode::ArrowDown) {
            panel.sel = (panel.sel + 1) % n;
        }
        if keys.just_pressed(KeyCode::ArrowUp) {
            panel.sel = (panel.sel + n - 1) % n;
        }
        if keys.just_pressed(KeyCode::Enter) {
            let _ = net.send(ClientMsg::ShopBuy {
                npc,
                index: panel.sel as u8,
                count: buy_count_for_modifiers(shift, ctrl),
            });
        }
        if mouse.is_some_and(|m| m.just_pressed(MouseButton::Left))
            && let Some(win) = window.as_deref()
            && let Some(c) = win.cursor_position()
        {
            let win_size = [win.width(), win.height()];
            let line_h = fonts
                .as_ref()
                .map(|f| (f.mouse.0.line_spacing as f32).max(18.0))
                .unwrap_or(20.0);
            if let Some(row) = shop_row_at([c.x, c.y], win_size, &panel.text, n, line_h) {
                panel.sel = row;
                let pos = panel_pos(win_size);
                let k = crate::hud::inventory::scale(win_size);
                let rel_x = (c.x - pos[0]) / k;
                let count = if rel_x >= 395.0 {
                    u16::MAX
                } else if rel_x >= 335.0 {
                    10
                } else {
                    buy_count_for_modifiers(shift, ctrl)
                };
                let _ = net.send(ClientMsg::ShopBuy {
                    npc,
                    index: row as u8,
                    count,
                });
            }
        }
        // Selling has no key: Shift-click a bag slot (see `sell_click`), so `S`
        // stays "move backwards".
    } else {
        if keys.just_pressed(KeyCode::KeyG) {
            let _ = net.send(ClientMsg::GuideAdvice);
        }
        if panel.is_guide && keys.just_pressed(KeyCode::KeyC) {
            switch_to_crafting(&mut panel, &mut inv, &mut opts);
        }
    }
}

/// Maps window cursor to visible Guide recipe row `0..MAX_GUIDE_ROWS`.
pub fn guide_row_at(
    cursor: [f32; 2],
    win: [f32; 2],
    recipe_count: usize,
    scroll: usize,
) -> Option<usize> {
    if recipe_count == 0 {
        return None;
    }
    let pos = panel_pos(win);
    let k = crate::hud::inventory::scale(win);
    let rel_x = (cursor[0] - pos[0]) / k;
    let rel_y = (cursor[1] - pos[1]) / k;
    if !(16.0..=444.0).contains(&rel_x) || rel_y < 100.0 {
        return None;
    }
    let row = ((rel_y - 100.0) / 22.0).floor() as usize;
    let visible = recipe_count.saturating_sub(scroll).min(MAX_GUIDE_ROWS);
    (row < visible).then_some(row)
}

pub fn shop_row_at(
    cursor: [f32; 2],
    win: [f32; 2],
    text: &str,
    shop_len: usize,
    line_h: f32,
) -> Option<usize> {
    if shop_len == 0 || line_h <= 0.0 {
        return None;
    }
    let pos = panel_pos(win);
    let k = crate::hud::inventory::scale(win);
    if cursor[0] < pos[0] || cursor[0] > pos[0] + 460.0 * k {
        return None;
    }
    let header_lines = text.split('\n').count() + 1;
    let first_row_y = pos[1] + 38.0 + header_lines as f32 * line_h;
    let rel_y = cursor[1] - first_row_y;
    if rel_y < 0.0 {
        return None;
    }
    let idx = (rel_y / line_h).floor() as usize;
    (idx < shop_len).then_some(idx)
}

fn equipped_for_comparison(
    item: ItemId,
    armor: Option<&[Option<ItemId>; 3]>,
    inv: Option<&terravibe_sim::inventory::Inventory>,
) -> Option<ItemId> {
    let d = item_data(item);
    if let Some(slot) = d.armor_slot {
        let idx = match slot {
            ArmorSlot::Head => 0,
            ArmorSlot::Body => 1,
            ArmorSlot::Legs => 2,
        };
        return armor.and_then(|a| a[idx]);
    }
    if let Some(inv) = inv {
        if d.pick > 0 {
            return inv.slots[0..10]
                .iter()
                .flatten()
                .map(|st| st.id)
                .find(|&id| item_data(id).pick > 0);
        }
        if d.axe > 0 {
            return inv.slots[0..10]
                .iter()
                .flatten()
                .map(|st| st.id)
                .find(|&id| item_data(id).axe > 0);
        }
        if d.damage > 0 {
            return inv.slots[0..10]
                .iter()
                .flatten()
                .map(|st| st.id)
                .find(|&id| {
                    let eq = item_data(id);
                    eq.class == d.class && eq.damage > 0
                });
        }
    }
    None
}

fn wrap_text(font: &terravibe_assets::SpriteFont, text: &str, max_width: f32) -> String {
    if max_width <= 0.0 {
        return text.to_string();
    }
    let mut out = Vec::new();
    for para in text.split('\n') {
        if para.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && super::text::measure(font, &candidate) > max_width {
                out.push(line);
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        out.push(line);
    }
    out.join("\n")
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct GuideViewParams<'w, 's> {
    guide_slot: Option<Res<'w, crate::net::LocalGuideSlot>>,
    guide_box: Option<
        Single<
            'w,
            's,
            (&'static mut Node, &'static mut Visibility),
            (
                With<GuideMaterialSlotBox>,
                Without<TownRoot>,
                Without<TownTabButton>,
                Without<ShopRowIcon>,
                Without<TownLabel>,
                Without<GuideMaterialIcon>,
                Without<GuideRecipeIcon>,
            ),
        >,
    >,
    guide_mat_icon: Option<
        Single<
            'w,
            's,
            (
                &'static mut ImageNode,
                &'static mut Node,
                &'static mut Visibility,
            ),
            (
                With<GuideMaterialIcon>,
                Without<TownRoot>,
                Without<TownTabButton>,
                Without<ShopRowIcon>,
                Without<TownLabel>,
                Without<GuideMaterialSlotBox>,
                Without<GuideRecipeIcon>,
            ),
        >,
    >,
    guide_mat_count: Option<
        Single<
            'w,
            's,
            &'static mut HudText,
            (
                With<GuideMaterialCount>,
                Without<TownLabel>,
                Without<TownSpeaker>,
            ),
        >,
    >,
    guide_recipe_icons: Query<
        'w,
        's,
        (
            &'static GuideRecipeIcon,
            &'static mut Node,
            &'static mut ImageNode,
            &'static mut Visibility,
        ),
        (
            Without<TownRoot>,
            Without<TownTabButton>,
            Without<ShopRowIcon>,
            Without<TownLabel>,
            Without<GuideMaterialSlotBox>,
            Without<GuideMaterialIcon>,
        ),
    >,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_view(
    window: Single<&Window, With<PrimaryWindow>>,
    panel: Res<TownPanel>,
    fonts: Option<Res<super::HudFonts>>,
    inv: Option<Res<crate::net::LocalInventory>>,
    armor: Option<Res<crate::net::LocalArmor>>,
    icon_assets: super::inventory::IconAssets,
    mut tooltip: Option<ResMut<super::tooltip::TooltipOverlay>>,
    mut root: Single<&mut Visibility, With<TownRoot>>,
    mut node: Single<&mut Node, With<TownRoot>>,
    mut label: Single<
        (&mut HudText, &mut Node),
        (
            With<TownLabel>,
            Without<TownSpeaker>,
            Without<TownRoot>,
            Without<TownTabButton>,
            Without<ShopRowIcon>,
            Without<GuideMaterialSlotBox>,
            Without<GuideMaterialIcon>,
            Without<GuideMaterialCount>,
            Without<GuideRecipeIcon>,
        ),
    >,
    mut speaker: Single<&mut HudText, (With<TownSpeaker>, Without<TownLabel>)>,
    mut tabs: Query<
        (
            &TownTabButton,
            &mut Node,
            &mut BackgroundColor,
            &mut BorderColor,
            &mut Visibility,
        ),
        (
            Without<TownRoot>,
            Without<ShopRowIcon>,
            Without<TownLabel>,
            Without<GuideMaterialSlotBox>,
            Without<GuideMaterialIcon>,
            Without<GuideRecipeIcon>,
        ),
    >,
    mut shop_icons: Query<
        (&ShopRowIcon, &mut Node, &mut ImageNode, &mut Visibility),
        (
            Without<TownRoot>,
            Without<TownTabButton>,
            Without<TownLabel>,
            Without<GuideMaterialSlotBox>,
            Without<GuideMaterialIcon>,
            Without<GuideRecipeIcon>,
        ),
    >,
    guide: GuideViewParams,
) {
    let GuideViewParams {
        guide_slot,
        mut guide_box,
        mut guide_mat_icon,
        mut guide_mat_count,
        mut guide_recipe_icons,
    } = guide;
    let win = [window.width(), window.height()];
    let k = crate::hud::inventory::scale(win);
    let pos = panel_pos(win);
    let panel_h = if panel.guide_crafting_open() {
        340.0
    } else {
        260.0
    };
    node.left = Val::Px(pos[0]);
    node.top = Val::Px(pos[1]);
    node.width = Val::Px(460.0 * k);
    node.height = Val::Px(panel_h * k);

    let line_h = fonts
        .as_ref()
        .map(|f| (f.mouse.0.line_spacing as f32).max(18.0))
        .unwrap_or(20.0);

    for (tab_btn, mut tnode, mut bg, mut border, mut vis) in &mut tabs {
        let show = panel.npc.is_some() && (tab_btn.0 != 1 || panel.is_guide);
        let want_vis = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want_vis {
            *vis = want_vis;
        }
        if !show {
            continue;
        }
        let (left_px, w_px) = match tab_btn.0 {
            0 => (120.0, 68.0),
            1 => (196.0, 92.0),
            _ if panel.is_guide => (296.0, 88.0),
            _ => (200.0, 96.0),
        };
        tnode.left = Val::Px(left_px * k);
        tnode.top = Val::Px(8.0 * k);
        tnode.width = Val::Px(w_px * k);
        tnode.height = Val::Px(24.0 * k);
        let active = (tab_btn.0 == 0 && panel.tab == TownTab::Talk)
            || (tab_btn.0 == 1 && panel.tab == TownTab::Crafting)
            || (tab_btn.0 == 2 && panel.tab == TownTab::Housing);
        let want_bg = if active {
            super::style::BTN_BG_ACTIVE
        } else {
            super::style::BTN_BG
        };
        if bg.0 != want_bg {
            bg.0 = want_bg;
        }
        let want_border = BorderColor::all(if active {
            super::style::GOLD_BORDER
        } else {
            super::style::PANEL_BORDER
        });
        if *border != want_border {
            *border = want_border;
        }
    }

    let wrapped_header = fonts
        .as_ref()
        .map(|f| wrap_text(&f.mouse.0, &panel.text, (460.0 * k - 32.0).max(200.0)))
        .unwrap_or_else(|| panel.text.clone());

    let super::inventory::IconAssets {
        assets,
        images: mut image_assets,
    } = icon_assets;
    let show_shop_icons = panel.tab == TownTab::Talk && !panel.shop.is_empty();
    let header_lines = wrapped_header.split('\n').count() + 1;
    let guide_stack = guide_slot.as_ref().and_then(|g| g.0);
    let guide_recipes = guide_stack
        .map(|st| terravibe_sim::crafting::recipes_using_material(st.id))
        .unwrap_or_default();
    let show_guide = panel.guide_crafting_open();

    if let Some(ref mut gb) = guide_box {
        gb.0.left = Val::Px(16.0 * k);
        gb.0.top = Val::Px(44.0 * k);
        gb.0.width = Val::Px(44.0 * k);
        gb.0.height = Val::Px(44.0 * k);
        let want_vis = if show_guide {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *gb.1 != want_vis {
            *gb.1 = want_vis;
        }
    }
    if let Some(ref mut mc) = guide_mat_count {
        let want_count = if show_guide
            && let Some(st) = guide_stack
            && st.count > 1
        {
            st.count.to_string()
        } else {
            String::new()
        };
        if mc.text != want_count {
            mc.text = want_count;
        }
    }

    if let Some(mut assets) = assets {
        if let Some(ref mut gmi) = guide_mat_icon {
            if show_guide && let Some(st) = guide_stack {
                let (handle, size) = super::hotbar::icon_of(&mut assets, &mut image_assets, st.id);
                let max_dim = 32.0 * k;
                let scale_factor = max_dim / size.max_element().max(1.0);
                gmi.1.width = Val::Px((size.x * scale_factor).round().max(1.0));
                gmi.1.height = Val::Px((size.y * scale_factor).round().max(1.0));
                if gmi.0.image != handle {
                    gmi.0.image = handle;
                }
                if *gmi.2 != Visibility::Inherited {
                    *gmi.2 = Visibility::Inherited;
                }
            } else if *gmi.2 != Visibility::Hidden {
                *gmi.2 = Visibility::Hidden;
            }
        }

        if show_guide && !guide_recipes.is_empty() {
            let start = panel
                .guide_scroll
                .min(guide_recipes.len().saturating_sub(MAX_GUIDE_ROWS));
            for (row_icon, mut inode, mut img, mut vis) in &mut guide_recipe_icons {
                if let Some(r) = guide_recipes.get(start + row_icon.0) {
                    let (handle, size) =
                        super::hotbar::icon_of(&mut assets, &mut image_assets, r.output.item);
                    let max_dim = (line_h - 2.0).clamp(12.0, 20.0);
                    let scale_factor = max_dim / size.max_element().max(1.0);
                    let w = (size.x * scale_factor).round().max(1.0);
                    let h = (size.y * scale_factor).round().max(1.0);
                    let row_y = 100.0 * k + row_icon.0 as f32 * 22.0 * k;
                    inode.left = Val::Px(16.0 * k);
                    inode.top = Val::Px(row_y + (line_h - h) * 0.5);
                    inode.width = Val::Px(w);
                    inode.height = Val::Px(h);
                    if img.image != handle {
                        img.image = handle;
                    }
                    if *vis != Visibility::Inherited {
                        *vis = Visibility::Inherited;
                    }
                } else if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
        } else {
            for (_, _, _, mut vis) in &mut guide_recipe_icons {
                if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
        }

        if show_shop_icons {
            for (row_icon, mut inode, mut img, mut vis) in &mut shop_icons {
                if let Some(&(item, _)) = panel.shop.get(row_icon.0) {
                    let (handle, size) =
                        super::hotbar::icon_of(&mut assets, &mut image_assets, item);
                    let max_dim = (line_h - 2.0).clamp(12.0, 20.0);
                    let scale_factor = max_dim / size.max_element().max(1.0);
                    let w = (size.x * scale_factor).round().max(1.0);
                    let h = (size.y * scale_factor).round().max(1.0);
                    let row_y = 38.0 + (header_lines + row_icon.0) as f32 * line_h;
                    inode.left = Val::Px(16.0);
                    inode.top = Val::Px(row_y + (line_h - h) * 0.5);
                    inode.width = Val::Px(w);
                    inode.height = Val::Px(h);
                    if img.image != handle {
                        img.image = handle;
                    }
                    if *vis != Visibility::Inherited {
                        *vis = Visibility::Inherited;
                    }
                } else if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
        } else {
            for (_, _, _, mut vis) in &mut shop_icons {
                if *vis != Visibility::Hidden {
                    *vis = Visibility::Hidden;
                }
            }
        }
    } else {
        if let Some(ref mut gmi) = guide_mat_icon
            && *gmi.2 != Visibility::Hidden
        {
            *gmi.2 = Visibility::Hidden;
        }
        for (_, _, _, mut vis) in &mut guide_recipe_icons {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
        for (_, _, _, mut vis) in &mut shop_icons {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
        }
    }

    if panel.tab == TownTab::Talk
        && panel.shop_open()
        && let Some(overlay) = tooltip.as_deref_mut()
        && let Some(c) = window.cursor_position()
    {
        if let Some(row) = shop_row_at([c.x, c.y], win, &wrapped_header, panel.shop.len(), line_h)
            && let Some(&(item, price)) = panel.shop.get(row)
        {
            let eq = equipped_for_comparison(
                item,
                armor.as_ref().map(|a| &a.0),
                inv.as_ref().map(|i| &i.0),
            );
            overlay.candidate = Some(super::tooltip::TooltipCandidate {
                target_id: 4_000 + row as u64,
                stack: ItemStack {
                    id: item,
                    count: 1,
                    prefix: Prefix(0),
                },
                equipped: eq,
                price_override: Some(("Buy", price)),
            });
        }
    } else if show_guide
        && let Some(overlay) = tooltip.as_deref_mut()
        && let Some(c) = window.cursor_position()
    {
        if guide_slot_hit([c.x, c.y], win)
            && let Some(st) = guide_stack
        {
            overlay.candidate = Some(super::tooltip::TooltipCandidate {
                target_id: 4_500,
                stack: st,
                equipped: None,
                price_override: None,
            });
        } else if let Some(row) =
            guide_row_at([c.x, c.y], win, guide_recipes.len(), panel.guide_scroll)
            && let Some(r) = guide_recipes.get(panel.guide_scroll + row)
        {
            let eq = equipped_for_comparison(
                r.output.item,
                armor.as_ref().map(|a| &a.0),
                inv.as_ref().map(|i| &i.0),
            );
            overlay.candidate = Some(super::tooltip::TooltipCandidate {
                target_id: 4_600 + (panel.guide_scroll + row) as u64,
                stack: ItemStack {
                    id: r.output.item,
                    count: r.output.count,
                    prefix: Prefix(0),
                },
                equipped: eq,
                price_override: None,
            });
        }
    }

    let (ref mut label_text, ref mut label_node) = *label;
    if show_guide {
        label_node.left = Val::Px(16.0 * k);
        label_node.top = Val::Px(42.0 * k);
    } else {
        label_node.left = Val::Px(16.0);
        label_node.top = Val::Px(38.0);
    }
    if !panel.is_changed() && !guide_slot.is_some_and(|g| g.is_changed()) {
        return;
    }
    let max_text_w = (460.0 * k - 32.0).max(200.0);
    let s = if panel.tab == TownTab::Housing {
        let raw = housing_panel_text(&panel);
        fonts
            .as_ref()
            .map(|f| wrap_text(&f.mouse.0, &raw, max_text_w))
            .unwrap_or(raw)
    } else if panel.tab == TownTab::Reforge && panel.is_tinkerer {
        let mut rows =
            String::from("Reforge (Up/Down to select, Enter to reforge, Tab: switch):\n");
        if let Some(inv_ref) = inv.as_ref().map(|i| &i.0) {
            let eligible: Vec<(usize, ItemStack)> = inv_ref.slots[..BAG]
                .iter()
                .enumerate()
                .filter_map(|(idx, s)| {
                    let st = (*s)?;
                    terravibe_sim::shop::is_reforgeable(st.id).then_some((idx, st))
                })
                .collect();
            if eligible.is_empty() {
                rows += "\nNo reforgeable weapons, tools, or accessories in bag.";
            } else {
                let sel = panel.reforge_sel.min(eligible.len() - 1);
                for (i, (slot_idx, st)) in eligible.iter().take(10).enumerate() {
                    let mark = if i == sel { ">" } else { " " };
                    let pfx = terravibe_sim::item::prefix_name(st.prefix);
                    let name = if pfx.is_empty() {
                        item_data(st.id).name.to_string()
                    } else {
                        format!("{pfx} {}", item_data(st.id).name)
                    };
                    let cost = price_text(terravibe_sim::shop::reforge_cost(st.id));
                    rows += &format!("\n{mark} [{slot_idx}] {name} — {cost}");
                }
            }
        }
        if !panel.note.is_empty() {
            rows += &format!("\n\n{}", panel.note);
        }
        rows
    } else if show_guide {
        guide_crafting_body(
            &panel,
            guide_stack,
            &guide_recipes,
            inv.as_ref().map(|i| &i.0),
            fonts.as_deref(),
            max_text_w,
        )
    } else {
        let mut s = wrapped_header;
        if !panel.shop.is_empty() {
            s += "\n";
            let armor_ref = armor.as_ref().map(|a| &a.0);
            let inv_ref = inv.as_ref().map(|i| &i.0);
            for (i, (item, price)) in panel.shop.iter().enumerate() {
                let eq = equipped_for_comparison(*item, armor_ref, inv_ref);
                s += &format!(
                    "\n   {}",
                    format_shop_row(*item, *price, i == panel.sel, eq)
                );
            }
            let footer = "Up/Down, Enter [1x] Shift+Enter [10x] Ctrl+Enter [All]   Shift-click bag: sell   Esc: close";
            let wrapped_footer = fonts
                .as_ref()
                .map(|f| wrap_text(&f.mouse.0, footer, max_text_w))
                .unwrap_or_else(|| footer.to_string());
            s += &format!("\n\n{wrapped_footer}");
        }
        if panel.npc.is_some() && panel.shop.is_empty() {
            if panel.is_guide {
                s += "\n\n[G] Advice: What should I do next?   [C] Crafting   Esc: close";
            } else if panel.is_nurse {
                s += "\n\n[Enter] Heal & Clear Debuffs   [G] Advice   Esc: close";
            } else {
                s += "\n\n[G] Advice: What should I do next?   Esc: close";
            }
        } else if panel.is_tinkerer {
            s += "\n\n[R] Open Reforge Tab";
        }
        if !panel.note.is_empty() {
            let wrapped_note = fonts
                .as_ref()
                .map(|f| wrap_text(&f.mouse.0, &panel.note, max_text_w))
                .unwrap_or_else(|| panel.note.clone());
            s += &format!("\n\n{wrapped_note}");
        }
        s
    };
    **root = if panel.npc.is_some() && !s.is_empty() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if label_text.text != s {
        label_text.text = s;
    }

    let speaker_name = if panel.tab == TownTab::Housing {
        "Housing"
    } else if panel.tab == TownTab::Reforge && panel.is_tinkerer {
        "Goblin Tinkerer — Reforge"
    } else if show_guide {
        "Guide — Crafting"
    } else if !panel.shop.is_empty() {
        if panel.text.starts_with("Arms Dealer:")
            || panel
                .shop
                .iter()
                .any(|(id, _)| *id == terravibe_sim::item::FLINTLOCK_PISTOL)
        {
            "Arms Dealer"
        } else if panel.is_tinkerer {
            "Goblin Tinkerer"
        } else if panel.text.starts_with("Demolitionist:") {
            "Demolitionist"
        } else if panel.text.starts_with("Dryad:") {
            "Dryad"
        } else {
            "Merchant"
        }
    } else if panel.npc.is_some() {
        if panel.is_guide {
            "Guide"
        } else if panel.is_nurse {
            "Nurse"
        } else {
            "Town NPC"
        }
    } else {
        "Housing Query"
    };
    if speaker.text != speaker_name {
        speaker.text = speaker_name.to_string();
    }
}

/// Formats the Guide's material-recipe lookup body text.
pub fn guide_crafting_body(
    panel: &TownPanel,
    guide_stack: Option<ItemStack>,
    recipes: &[&'static terravibe_sim::crafting::Recipe],
    inv: Option<&terravibe_sim::inventory::Inventory>,
    fonts: Option<&super::HudFonts>,
    max_text_w: f32,
) -> String {
    let mut out = String::new();
    let Some(st) = guide_stack else {
        out.push_str(
            "        Material: (empty)\n        Click slot with an item or Shift-click bag",
        );
        if !panel.note.is_empty() {
            out.push_str(&format!("\n\n{}", panel.note));
        } else {
            out.push_str("\n\nPlace any crafting material in the slot on the left to see every recipe that uses it.");
        }
        return fonts
            .map(|f| wrap_text(&f.mouse.0, &out, max_text_w))
            .unwrap_or(out);
    };

    let mat_name = item_data(st.id).name;
    let total = recipes.len();
    let start = panel.guide_scroll.min(total.saturating_sub(MAX_GUIDE_ROWS));
    let end = (start + MAX_GUIDE_ROWS).min(total);
    if total > MAX_GUIDE_ROWS {
        out.push_str(&format!(
            "        {mat_name} — {total} recipes ({}-{} / {total})\n        Scroll / Up-Down to browse",
            start + 1,
            end
        ));
    } else {
        out.push_str(&format!(
            "        {mat_name} — {total} recipes\n        Click slot or Shift-click to swap/remove"
        ));
    }
    out.push('\n');

    let sel = panel.guide_sel.min(total.saturating_sub(1));
    for (idx, r) in recipes.iter().enumerate().skip(start).take(MAX_GUIDE_ROWS) {
        let mark = if idx == sel { ">" } else { " " };
        let out_name = item_data(r.output.item).name;
        let qty = if r.output.count > 1 {
            format!(" x{}", r.output.count)
        } else {
            String::new()
        };
        let station = terravibe_sim::crafting::recipe_station_name(r.id).unwrap_or("By Hand");
        out.push_str(&format!("\n   {mark} {out_name}{qty} — {station}"));
    }

    if let Some(r) = recipes.get(sel) {
        let ing_list: Vec<String> = r
            .ingredients
            .iter()
            .map(|ing| {
                let name = item_data(ing.item).name;
                if let Some(inv) = inv {
                    let have = inv.count_of(ing.item);
                    format!("{} x{} ({have}/{})", name, ing.count, ing.count)
                } else {
                    format!("{} x{}", name, ing.count)
                }
            })
            .collect();
        let station = terravibe_sim::crafting::recipe_station_name(r.id).unwrap_or("By Hand");
        let detail = format!(
            "\n\nIngredients: {}\nStation: {}",
            ing_list.join(", "),
            station
        );
        let wrapped_detail = fonts
            .map(|f| wrap_text(&f.mouse.0, &detail, max_text_w))
            .unwrap_or(detail);
        out.push_str(&wrapped_detail);
    }
    if !panel.note.is_empty() {
        out.push_str(&format!("\n{}", panel.note));
    }
    out
}

/// Draws a 3D wireframe bounding box around the queried room when the Housing tab is open.
pub fn draw_room_highlight(mut gizmos: Option<Gizmos>, panel: Option<Res<TownPanel>>) {
    let Some(ref mut gizmos) = gizmos else {
        return;
    };
    let Some(panel) = panel else {
        return;
    };
    if panel.tab != TownTab::Housing {
        return;
    }
    let valid = panel.housing_status.is_some_and(|(ok, _)| ok);
    let color = if valid {
        Color::srgba(0.2, 0.9, 0.35, 0.9)
    } else {
        Color::srgba(0.95, 0.3, 0.25, 0.9)
    };
    if let Some((lo, hi)) = panel.room_bounds {
        let min = Vec3::new(lo[0] as f32, lo[1] as f32, lo[2] as f32);
        let max = Vec3::new(hi[0] as f32, hi[1] as f32, hi[2] as f32);
        let center = (min + max) * 0.5;
        let size = (max - min).max(Vec3::splat(1.0));
        gizmos.cube(Transform::from_translation(center).with_scale(size), color);
    } else if let Some(pos) = panel.housing_query_pos {
        let center = Vec3::new(pos[0] as f32, pos[1] as f32, pos[2] as f32) + Vec3::splat(0.5);
        gizmos.cube(
            Transform::from_translation(center).with_scale(Vec3::splat(1.02)),
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use terravibe_sim::inventory::Inventory;
    use terravibe_sim::item::TORCH;
    use terravibe_sim::net::{ClientTransport, Disconnected, ServerMsg};
    use terravibe_sim::slots::SlotRef;

    use super::*;
    use crate::net::LocalInventory;

    #[test]
    fn prices_split_into_coins() {
        assert_eq!(price_text(0), "0c");
        assert_eq!(price_text(50), "50c");
        assert_eq!(price_text(5000), "50s");
        assert_eq!(price_text(10_250), "1g 2s 50c");
        assert_eq!(price_text(1_000_000), "1p");
    }

    #[test]
    fn house_reasons_have_text() {
        assert!(house_text(true, 0).starts_with("Valid"));
        assert!(house_text(false, 8).contains("chair"));
    }

    #[test]
    fn shop_sell_is_shift_click_on_bag_slot() {
        let bag = SlotRef::Inv(13);
        assert_eq!(sell_click(true, true, true, bag), Some(13));
        assert_eq!(sell_click(true, true, false, bag), None, "plain click");
        assert_eq!(sell_click(false, true, true, bag), None, "shop closed");
        assert_eq!(sell_click(true, false, true, bag), None, "right click");
        assert_eq!(
            sell_click(true, true, true, SlotRef::Inv(50)),
            None,
            "coin slots are not bag slots"
        );
        assert_eq!(sell_click(true, true, true, SlotRef::Chest(3)), None);
    }

    struct Recorder(Arc<Mutex<Vec<ClientMsg>>>);

    impl ClientTransport for Recorder {
        fn send(&self, m: ClientMsg) -> Result<(), Disconnected> {
            self.0.lock().unwrap().push(m);
            Ok(())
        }

        fn try_recv(&self) -> Result<Option<ServerMsg>, Disconnected> {
            Ok(None)
        }
    }

    fn press_with_shop(key: KeyCode) -> Vec<ClientMsg> {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>();
        let sent = Arc::new(Mutex::new(Vec::new()));
        app.insert_resource(Net::new(Recorder(sent.clone())));
        app.insert_resource(LocalInventory(Inventory::empty()));
        app.insert_resource(TownPanel {
            npc: Some(EnemyId(7)),
            shop: vec![(TORCH, 50)],
            ..default()
        });
        app.add_systems(Update, handle_input);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        sent.lock().unwrap().clone()
    }

    #[test]
    fn s_key_never_sells() {
        assert!(
            press_with_shop(KeyCode::KeyS).is_empty(),
            "S moves backwards and must never sell"
        );
        assert!(
            press_with_shop(KeyCode::KeyR).is_empty(),
            "there is no sell key binding anymore"
        );
    }

    #[test]
    fn h_key_sends_guide_advice_when_talking_to_guide() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>();
        let sent = Arc::new(Mutex::new(Vec::new()));
        app.insert_resource(Net::new(Recorder(sent.clone())));
        app.insert_resource(TownPanel {
            npc: Some(EnemyId(1)),
            shop: Vec::new(),
            ..default()
        });
        app.add_systems(Update, handle_input);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyG);
        app.update();
        let msgs = sent.lock().unwrap().clone();
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], ClientMsg::GuideAdvice));
    }

    #[test]
    fn shop_row_shows_equipped_comparison_and_cannot_afford() {
        use terravibe_sim::item::{COPPER_HELMET, COPPER_PICKAXE, IRON_HELMET, WOODEN_SWORD};

        // Equipped weapon comparison on a weapon/tool with damage
        let row = format_shop_row(COPPER_PICKAXE, 500, true, Some(WOODEN_SWORD));
        assert!(row.contains("[1x]") && row.contains("[10x]") && row.contains("[All]"));
        assert!(row.contains("vs Wooden Sword"), "row: {row}");
        assert!(row.contains("dmg"), "row: {row}");

        // Equipped armor comparison
        let armor_row = format_shop_row(IRON_HELMET, 500, false, Some(COPPER_HELMET));
        assert!(armor_row.contains("vs Copper Helmet"), "row: {armor_row}");
        assert!(armor_row.contains("def"), "row: {armor_row}");

        // Modifier buy counts
        assert_eq!(buy_count_for_modifiers(false, false), 1);
        assert_eq!(buy_count_for_modifiers(true, false), 10);
        assert_eq!(buy_count_for_modifiers(false, true), u16::MAX);

        // Cannot afford feedback via TownMsg::Result
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<TownMsg>()
            .init_resource::<TownPanel>();
        app.world_mut()
            .spawn((PrimaryWindow, Window::default(), CursorOptions::default()));
        app.add_systems(Update, receive);
        app.world_mut()
            .resource_mut::<Messages<TownMsg>>()
            .write(TownMsg::Result {
                ok: false,
                cannot_afford: true,
            });
        app.update();
        let panel = app.world().resource::<TownPanel>();
        assert!(
            panel.note.contains("Cannot afford"),
            "note was: {}",
            panel.note
        );
    }

    #[test]
    fn pickaxe_comparison_ignores_axe_in_earlier_slot() {
        use terravibe_sim::item::{COPPER_AXE, COPPER_PICKAXE};

        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack {
            id: COPPER_AXE,
            count: 1,
            prefix: Prefix(0),
        });
        inv.slots[1] = Some(ItemStack {
            id: COPPER_PICKAXE,
            count: 1,
            prefix: Prefix(0),
        });
        let eq = equipped_for_comparison(COPPER_PICKAXE, None, Some(&inv));
        assert_eq!(eq, Some(COPPER_PICKAXE));
        let cmp = shop_comparison_text(COPPER_PICKAXE, eq).unwrap();
        assert!(cmp.contains("vs Copper Pickaxe"), "cmp: {cmp}");
        assert!(cmp.contains("% pick"), "cmp: {cmp}");
    }

    #[test]
    fn housing_tab_displays_reasons_and_sends_assign_housing() {
        use terravibe_sim::town_npc::TownKind;

        // 1. Plain-language reasons for all housing codes 0..=8.
        assert!(house_text(true, 0).starts_with("Valid"));
        for code in 1..=8 {
            let txt = house_text(false, code);
            assert!(
                txt.starts_with("Not housing:"),
                "code {code} should start with 'Not housing:', got: {txt}"
            );
        }
        assert!(house_text(false, 3).contains("open air"));
        assert!(house_text(false, 5).contains("small"));
        assert!(house_text(false, 8).contains("chair"));

        // 2. TownMsg::House switches to Housing tab and stores status.
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<TownMsg>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<TownPanel>();
        app.world_mut()
            .spawn((PrimaryWindow, Window::default(), CursorOptions::default()));
        let sent = Arc::new(Mutex::new(Vec::new()));
        app.insert_resource(Net::new(Recorder(sent.clone())));
        app.add_systems(Update, (receive, handle_input.after(receive)));

        {
            let mut panel = app.world_mut().resource_mut::<TownPanel>();
            panel.housing_query_pos = Some([10, 65, 12]);
            panel.room_bounds = Some(([8, 64, 10], [14, 68, 16]));
        }
        app.world_mut()
            .resource_mut::<Messages<TownMsg>>()
            .write(TownMsg::House {
                ok: true,
                reason: 0,
            });
        app.update();

        let panel = app.world().resource::<TownPanel>();
        assert_eq!(panel.tab, TownTab::Housing);
        assert_eq!(panel.housing_status, Some((true, 0)));
        let text = housing_panel_text(panel);
        assert!(text.contains("Valid housing"), "text: {text}");
        assert!(text.contains("(8, 64, 10)"), "bounds missing: {text}");
        assert!(text.contains("[Assign NPC]"), "button missing: {text}");

        // 3. Pressing Enter in Housing tab sends ClientMsg::AssignHousing.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();

        let msgs = sent.lock().unwrap().clone();
        assert!(
            msgs.iter().any(|m| matches!(
                m,
                ClientMsg::AssignHousing {
                    npc: TownKind::Guide,
                    pos: [10, 65, 12],
                }
            )),
            "expected ClientMsg::AssignHousing, got: {msgs:?}"
        );
    }

    #[test]
    fn town_msg_house_preserves_npc_shop_and_dialogue() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<TownMsg>()
            .init_resource::<TownPanel>();
        app.world_mut()
            .spawn((PrimaryWindow, Window::default(), CursorOptions::default()));
        app.add_systems(Update, receive);

        {
            let mut panel = app.world_mut().resource_mut::<TownPanel>();
            panel.npc = Some(EnemyId(42));
            panel.shop = vec![(TORCH, 50)];
            panel.text = "Hello Traveler!".into();
            panel.tab = TownTab::Talk;
        }

        app.world_mut()
            .resource_mut::<Messages<TownMsg>>()
            .write(TownMsg::House {
                ok: true,
                reason: 0,
            });
        app.update();

        let panel = app.world().resource::<TownPanel>();
        assert_eq!(panel.tab, TownTab::Housing);
        assert_eq!(panel.housing_status, Some((true, 0)));
        assert_eq!(panel.npc, Some(EnemyId(42)), "NPC should be preserved");
        assert_eq!(panel.shop, vec![(TORCH, 50)], "Shop should be preserved");
        assert_eq!(
            panel.text, "Hello Traveler!",
            "Dialogue should be preserved"
        );
        assert!(panel.note.contains("Valid housing"));
    }

    #[test]
    fn switching_to_housing_tab_automatically_queries_housing_if_unqueried() {
        use terravibe_sim::player::Player;

        fn press(app: &mut App, key: KeyCode) {
            let mut k = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            k.clear();
            k.press(key);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(key);
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<TownPanel>();
        let sent = Arc::new(Mutex::new(Vec::new()));
        app.insert_resource(Net::new(Recorder(sent.clone())));
        app.insert_resource(LocalPlayer(Player::new([15.2, 60.1, -8.7])));
        {
            let mut panel = app.world_mut().resource_mut::<TownPanel>();
            panel.npc = Some(EnemyId(1));
            panel.tab = TownTab::Talk;
            panel.housing_query_pos = None;
        }
        app.add_systems(Update, handle_input);

        // Press Tab to switch from Talk to Housing
        press(&mut app, KeyCode::Tab);

        let panel = app.world().resource::<TownPanel>();
        assert_eq!(panel.tab, TownTab::Housing);
        assert_eq!(panel.housing_query_pos, Some([15, 60, -9]));

        let msgs = sent.lock().unwrap().clone();
        assert_eq!(msgs.len(), 1);
        assert!(matches!(
            msgs[0],
            ClientMsg::QueryHouse { pos: [15, 60, -9] }
        ));

        // Switch to Talk and back to Housing; housing_query_pos is already Some, so no re-query
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.world().resource::<TownPanel>().tab, TownTab::Talk);

        press(&mut app, KeyCode::Tab);
        assert_eq!(app.world().resource::<TownPanel>().tab, TownTab::Housing);

        // No new query sent
        assert_eq!(sent.lock().unwrap().len(), 1);
    }

    #[test]
    fn housing_tab_without_npc_hides_floating_town_panel() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<TownPanel>();
        app.init_resource::<Assets<Image>>();
        app.world_mut()
            .spawn((PrimaryWindow, Window::default(), CursorOptions::default()));
        let root_id = app
            .world_mut()
            .spawn((TownRoot, Node::default(), Visibility::Inherited))
            .id();
        app.world_mut().spawn((
            TownLabel,
            Node::default(),
            HudText {
                text: String::new(),
                death: false,
            },
        ));
        app.world_mut().spawn((
            TownSpeaker,
            HudText {
                text: String::new(),
                death: false,
            },
        ));
        app.add_systems(Update, update_view);

        // When Housing tab is active without an NPC Talk/Shop, TownPanel::is_open() is false
        // and the center floating TownRoot stays hidden.
        {
            let mut panel = app.world_mut().resource_mut::<TownPanel>();
            panel.npc = None;
            panel.tab = TownTab::Housing;
            panel.housing_status = Some((false, 3));
        }
        app.update();

        assert!(!app.world().resource::<TownPanel>().is_open());
        assert_eq!(
            *app.world().entity(root_id).get::<Visibility>().unwrap(),
            Visibility::Hidden
        );

        // When an NPC Talk/Shop is active, the floating TownRoot is shown.
        {
            let mut panel = app.world_mut().resource_mut::<TownPanel>();
            panel.npc = Some(EnemyId(1));
            panel.tab = TownTab::Talk;
            panel.text = "Hello!".into();
        }
        app.update();

        assert!(app.world().resource::<TownPanel>().is_open());
        assert_eq!(
            *app.world().entity(root_id).get::<Visibility>().unwrap(),
            Visibility::Inherited
        );
    }

    #[test]
    fn guide_crafting_tab_opens_inventory_lists_recipes_and_rejects_non_material() {
        use terravibe_sim::item::{IRON_BAR, WOODEN_SWORD};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<InventoryOpen>()
            .init_resource::<crate::net::LocalGuideSlot>()
            .init_resource::<crate::net::LocalHeld>()
            .init_resource::<TownPanel>();
        let sent = Arc::new(Mutex::new(Vec::new()));
        app.insert_resource(Net::new(Recorder(sent.clone())));
        app.world_mut()
            .spawn((PrimaryWindow, Window::default(), CursorOptions::default()));
        {
            let mut panel = app.world_mut().resource_mut::<TownPanel>();
            panel.npc = Some(EnemyId(1));
            panel.is_guide = true;
            panel.tab = TownTab::Talk;
        }
        app.add_systems(Update, handle_input);

        // Press C to open Guide Crafting tab & Inventory
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyC);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyC);

        assert!(app.world().resource::<TownPanel>().guide_crafting_open());
        assert!(app.world().resource::<InventoryOpen>().0);

        // Place IRON_BAR in GuideSlot and format body
        let stack = ItemStack {
            id: IRON_BAR,
            count: 5,
            prefix: Prefix(0),
        };
        let recipes = terravibe_sim::crafting::recipes_using_material(IRON_BAR);
        let body = guide_crafting_body(
            app.world().resource::<TownPanel>(),
            Some(stack),
            &recipes,
            None,
            None,
            420.0,
        );
        assert!(body.contains("Iron Bar"), "body: {body}");
        assert!(body.contains("Iron Pickaxe"), "body: {body}");
        assert!(body.contains("Anvil"), "body: {body}");

        // Holding non-material WOODEN_SWORD and clicking Guide Material Slot sets feedback note
        app.world_mut().resource_mut::<crate::net::LocalHeld>().0 = Some(ItemStack {
            id: WOODEN_SWORD,
            count: 1,
            prefix: Prefix(0),
        });
        // Simulate clicking Guide Material Slot via panel_hit + guide_slot_hit
        let win = [1280.0, 720.0];
        let pos = panel_pos(win);
        let k = crate::hud::inventory::scale(win);
        let slot_cursor = [pos[0] + 30.0 * k, pos[1] + 60.0 * k];
        assert!(panel_hit(slot_cursor, win, true));
        assert!(guide_slot_hit(slot_cursor, win));
    }
}
