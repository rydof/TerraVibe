//! Chest panel: the open chest's 10x4 grid, drawn in the inventory screen just below the
//! bag grid (as in Terraria). Opening a chest also opens the inventory. Every click is a
//! `ClickSlot` request on `SlotRef::Chest`; the server owns the inventories and answers with
//! `ChestContents` / `ChestClosed`. Right click on a chest block opens, `Esc`/`E` close,
//! `Q` quick-stacks matching items.

use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use terravibe_sim::chest::CHEST_SLOTS;
use terravibe_sim::chunk::BlockQuery;
use terravibe_sim::interact::{RAY_MAX, in_reach, raycast};
use terravibe_sim::inventory::{BAG, ItemStack};
use terravibe_sim::net::ClientMsg;
use terravibe_sim::slots::{ClickKind, SlotRef};
use terravibe_sim::{CHEST, DOOR_CLOSED, DOOR_OPEN};

use super::hotbar::{SLOT_SCALE, icon_of};
use super::inventory::{self, BAG_BOTTOM, BAG_LEFT, IconAssets, InventoryOpen, fit_icon};
use super::text::{HudText, StackText, measure};
use crate::net::{ChestMsg, Net};
use crate::session::AppState;
use crate::view::camera::{FlyCamera, set_grab};
use crate::view::chunks::ChunkStream;
use crate::view::player::{LoadedBlocks, LocalPlayer};

pub const COLS: usize = 10;
pub const GRID_ROWS: usize = 4;
/// Reference-px cell size (scaled with the inventory's 1080-px layout).
pub const CELL: f32 = 38.0;
/// Reference-px cell pitch (scaled with the inventory's 1080-px layout).
pub const PITCH: f32 = 40.0;
/// Chest grid offset inside the panel, in reference px.
pub const CHEST_GRID: [f32; 2] = [0.0, 24.0];
/// Gap between the bag grid and the chest panel, in reference px.
const PANEL_GAP: f32 = 10.0;

/// Panel top-left for a window: directly under the inventory's 5-row bag grid.
pub fn panel_pos(win: [f32; 2]) -> [f32; 2] {
    let s = inventory::scale(win);
    [BAG_LEFT * s, (BAG_BOTTOM + PANEL_GAP) * s]
}

/// Panel size for a window, scaled like the inventory layout.
pub fn panel_size(win: [f32; 2]) -> [f32; 2] {
    let s = inventory::scale(win);
    [
        (COLS as f32 * PITCH + 120.0) * s,
        (CHEST_GRID[1] + GRID_ROWS as f32 * PITCH + 12.0) * s,
    ]
}

pub const UTILITY_BUTTONS: [&str; 4] = ["Loot All", "Deposit All", "Quick Stack", "Restock"];
pub const BTN_X: f32 = CHEST_GRID[0] + COLS as f32 * PITCH + 10.0;
pub const BTN_W: f32 = 100.0;
pub const BTN_H: f32 = 30.0;
pub const BTN_PITCH: f32 = 36.0;

pub fn utility_button_rect(idx: usize, win: [f32; 2]) -> [f32; 4] {
    let s = inventory::scale(win);
    let p = panel_pos(win);
    [
        p[0] + BTN_X * s,
        p[1] + (CHEST_GRID[1] + idx as f32 * BTN_PITCH) * s,
        BTN_W * s,
        BTN_H * s,
    ]
}

pub fn utility_button_at(cursor: [f32; 2], win: [f32; 2]) -> Option<usize> {
    (0..4).find(|&i| {
        let [x, y, w, h] = utility_button_rect(i, win);
        (x..x + w).contains(&cursor[0]) && (y..y + h).contains(&cursor[1])
    })
}

fn cell_at_pitch(cursor: [f32; 2], grid: [f32; 2], pitch: f32) -> Option<usize> {
    let (x, y) = (cursor[0] - grid[0], cursor[1] - grid[1]);
    if x < 0.0 || y < 0.0 {
        return None;
    }
    let (c, r) = ((x / pitch) as usize, (y / pitch) as usize);
    (c < COLS && r < GRID_ROWS).then_some(r * COLS + c)
}

/// Grid cell (0..40) under `cursor` for a grid whose top-left is `grid` (reference px).
#[cfg(test)]
fn cell_at(cursor: [f32; 2], grid: [f32; 2]) -> Option<usize> {
    cell_at_pitch(cursor, grid, PITCH)
}

/// `[x, y, size]` of chest cell `i` relative to the panel, in window px.
pub fn cell_rect(i: usize, win: [f32; 2]) -> [f32; 3] {
    let s = inventory::scale(win);
    [
        (CHEST_GRID[0] + (i % COLS) as f32 * PITCH) * s,
        (CHEST_GRID[1] + (i / COLS) as f32 * PITCH) * s,
        CELL * s,
    ]
}

/// Chest slot under `cursor` (window px).
pub fn slot_at(cursor: [f32; 2], win: [f32; 2]) -> Option<usize> {
    let s = inventory::scale(win);
    let p = panel_pos(win);
    cell_at_pitch(
        cursor,
        [p[0] + CHEST_GRID[0] * s, p[1] + CHEST_GRID[1] * s],
        PITCH * s,
    )
}

fn shift(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
}

#[derive(Resource, Default)]
pub struct ChestPanel {
    pub open: Option<[i32; 3]>,
    pub slots: Vec<Option<ItemStack>>,
}

pub fn is_open(p: Res<ChestPanel>) -> bool {
    p.open.is_some()
}

#[derive(Component)]
pub struct PanelRoot;
#[derive(Component)]
pub struct CellBg(usize);
#[derive(Component)]
pub struct CellIcon(usize);
#[derive(Component)]
pub struct CellCount(usize);
#[derive(Component)]
pub struct UtilityButtonBg(pub usize);
#[derive(Component)]
pub struct UtilityButtonText(pub usize);

const BG: Color = super::style::PANEL_BG_SOLID;
const CELL_BG: Color = Color::srgba(0.17, 0.23, 0.48, 0.85);
const CELL_FULL: Color = Color::srgba(0.35, 0.42, 0.8, 0.9);

fn abs(x: f32, y: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(x),
        top: Val::Px(y),
        width: Val::Px(w),
        height: Val::Px(h),
        ..default()
    }
}

fn place(node: &mut Node, x: f32, y: f32, w: f32, h: f32) {
    node.left = Val::Px(x);
    node.top = Val::Px(y);
    node.width = Val::Px(w);
    node.height = Val::Px(h);
}

fn label(text: &str) -> HudText {
    HudText {
        text: text.into(),
        death: false,
    }
}

pub fn setup(mut commands: Commands, window: Single<&Window, With<PrimaryWindow>>) {
    let win = [window.width(), window.height()];
    let s = inventory::scale(win);
    let pos = panel_pos(win);
    let size = panel_size(win);
    commands
        .spawn((
            PanelRoot,
            Node {
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_PANEL)),
                ..abs(pos[0], pos[1], size[0], size[1])
            },
            BackgroundColor(BG),
            BorderColor::all(super::style::PANEL_BORDER),
            Visibility::Hidden,
            GlobalZIndex(510),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|p| {
            p.spawn((
                abs(10.0 * s, 6.0 * s, 0.0, 0.0),
                label("Chest   [Q] quick stack  [E/Esc] close"),
            ));
            for i in 0..CHEST_SLOTS {
                let [x, y, cell] = cell_rect(i, win);
                p.spawn((
                    CellBg(i),
                    Node {
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                        ..abs(x, y, cell, cell)
                    },
                    BackgroundColor(CELL_BG),
                    BorderColor::all(super::style::PANEL_BORDER),
                ));
                p.spawn((
                    CellIcon(i),
                    abs(x, y, 0.0, 0.0),
                    ImageNode::default(),
                    Visibility::Hidden,
                ));
                p.spawn((
                    CellCount(i),
                    abs(x + 2.0 * s, y + 22.0 * s, 0.0, 0.0),
                    label(""),
                    StackText,
                ));
            }
            for (idx, &name) in UTILITY_BUTTONS.iter().enumerate() {
                let [bx, by] = [(BTN_X) * s, (CHEST_GRID[1] + idx as f32 * BTN_PITCH) * s];
                p.spawn((
                    UtilityButtonBg(idx),
                    Node {
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                        ..abs(bx, by, BTN_W * s, BTN_H * s)
                    },
                    BackgroundColor(super::style::BTN_BG),
                    BorderColor::all(super::style::GOLD_BORDER),
                ));
                p.spawn((
                    UtilityButtonText(idx),
                    abs(bx + 8.0 * s, by + 6.0 * s, 0.0, 0.0),
                    label(name),
                ));
            }
        });
}

/// Right click on a chest block within reach asks the server to open it.
pub fn open_input(
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    gamepad_interact: Option<Res<crate::view::interact::GamepadInteractState>>,
    opts: Single<&CursorOptions, With<PrimaryWindow>>,
    cam: Single<&Transform, With<FlyCamera>>,
    stream: Option<Res<ChunkStream>>,
    player: Option<Res<LocalPlayer>>,
    net: Option<Res<Net>>,
) {
    let triggered = mouse.is_some_and(|m| m.just_pressed(MouseButton::Right))
        || gamepad_interact.is_some_and(|g| g.use_just_pressed);
    if !triggered || opts.grab_mode == bevy::window::CursorGrabMode::None {
        return;
    }
    let (Some(player), Some(net), Some(stream)) = (player, net, stream) else {
        return;
    };
    let q = LoadedBlocks(stream.generated());
    if let Some(h) = raycast(&q, cam.translation.into(), (*cam.forward()).into(), RAY_MAX)
        && in_reach(&player.0.body, h.block)
    {
        match q.block(h.block[0], h.block[1], h.block[2]) {
            CHEST => {
                let _ = net.send(ClientMsg::OpenChest { pos: h.block });
            }
            // The server validates reach and that nobody stands in the doorway.
            DOOR_CLOSED | DOOR_OPEN => {
                let _ = net.send(ClientMsg::UseTile { pos: h.block });
            }
            _ => {}
        }
    }
}

/// Applies server answers: show the panel and free the cursor, or hide it again.
pub fn receive(
    mut msgs: MessageReader<ChestMsg>,
    mut panel: ResMut<ChestPanel>,
    mut inv: Option<ResMut<InventoryOpen>>,
    mut opts: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut root: Single<&mut Visibility, With<PanelRoot>>,
) {
    for m in msgs.read() {
        match m {
            ChestMsg::Contents { pos, slots } => {
                if panel.open.is_none() {
                    set_grab(&mut opts, false);
                    **root = Visibility::Inherited;
                }
                panel.open = Some(*pos);
                panel.slots = slots.clone();
                // The bag is shown beside the chest, as in Terraria.
                if let Some(inv) = inv.as_mut() {
                    inv.0 = true;
                }
            }
            ChestMsg::Closed => {
                if panel.open.take().is_some() {
                    panel.slots.clear();
                    // The inventory usually stays open; keep the cursor free for it.
                    let bag_open = inv.as_ref().is_some_and(|i| i.0);
                    set_grab(&mut opts, !bag_open);
                    **root = Visibility::Hidden;
                }
            }
        }
    }
}

/// Clicks move stacks; `E` (or rebound EquipArmor key) closes (`Esc` is routed by the inventory screen), `Q` quick-stacks.
#[allow(clippy::too_many_arguments)]
pub fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    net: Res<Net>,
    inv: Option<Res<crate::net::LocalInventory>>,
    settings: Option<Res<crate::settings::Settings>>,
    mut panel: ResMut<ChestPanel>,
    mut opts: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut root: Single<&mut Visibility, With<PanelRoot>>,
) {
    if panel.open.is_none() {
        return;
    }
    let close_key = settings.as_deref().map_or(KeyCode::KeyE, |s| {
        s.bindings.key(crate::input_map::GameAction::EquipArmor)
    });
    if keys.just_pressed(close_key) || keys.just_pressed(KeyCode::KeyE) {
        let _ = net.send(ClientMsg::CloseChest);
        panel.open = None;
        panel.slots.clear();
        **root = Visibility::Hidden;
        set_grab(&mut opts, false);
        return;
    }
    if keys.just_pressed(KeyCode::KeyQ) {
        let _ = net.send(ClientMsg::ChestQuickStack);
    }
    let left = mouse.just_pressed(MouseButton::Left);
    let right = mouse.just_pressed(MouseButton::Right);
    if !(left || right) {
        return;
    }
    let Some(c) = window.cursor_position() else {
        return;
    };
    let win = [window.width(), window.height()];

    // Utility buttons: Loot All, Deposit All, Quick Stack, Restock
    if left && let Some(btn) = utility_button_at([c.x, c.y], win) {
        match btn {
            0 => {
                // Loot All: moves chest items to inventory
                for slot in 0..CHEST_SLOTS {
                    if panel.slots.get(slot).copied().flatten().is_some() {
                        let _ = net.send(ClientMsg::ChestMove {
                            to_chest: false,
                            slot: slot as u8,
                        });
                    }
                }
            }
            1 => {
                // Deposit All: moves inventory items to chest
                if let Some(inv) = inv.as_ref() {
                    for slot in 0..BAG {
                        if inv.0.slots.get(slot).copied().flatten().is_some() {
                            let _ = net.send(ClientMsg::ChestMove {
                                to_chest: true,
                                slot: slot as u8,
                            });
                        }
                    }
                }
            }
            2 => {
                // Quick Stack
                let _ = net.send(ClientMsg::ChestQuickStack);
            }
            3 => {
                // Restock: refills inventory items that have matching stacks in the chest
                if let Some(inv) = inv.as_ref() {
                    for slot in 0..CHEST_SLOTS {
                        if let Some(cs) = panel.slots.get(slot).copied().flatten() {
                            if inv.0.slots[0..BAG].iter().flatten().any(|s| s.id == cs.id) {
                                let _ = net.send(ClientMsg::ChestMove {
                                    to_chest: false,
                                    slot: slot as u8,
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        return;
    }

    let Some(slot) = slot_at([c.x, c.y], win) else {
        return;
    };
    if left && shift(&keys) {
        let _ = net.send(ClientMsg::ChestMove {
            to_chest: false,
            slot: slot as u8,
        });
        return;
    }
    let kind = if left {
        ClickKind::Left
    } else {
        ClickKind::Right
    };
    let _ = net.send(ClientMsg::ClickSlot {
        at: SlotRef::Chest(slot as u8),
        kind,
    });
}

fn count_of(s: &ItemStack) -> String {
    if s.count > 1 {
        s.count.to_string()
    } else {
        String::new()
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

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn update_cells(
    window: Single<&Window, With<PrimaryWindow>>,
    panel: Res<ChestPanel>,
    held: Option<Res<crate::net::LocalHeld>>,
    fonts: Option<Res<super::HudFonts>>,
    icon_assets: IconAssets,
    mut tooltip: Option<ResMut<super::tooltip::TooltipOverlay>>,
    mut q: ParamSet<(
        Query<&mut Node, With<PanelRoot>>,
        Query<(&CellBg, &mut BackgroundColor, &mut Node)>,
        Query<
            (&CellIcon, &mut Node, &mut ImageNode, &mut Visibility),
            (Without<CellBg>, Without<CellCount>),
        >,
        Query<(&CellCount, &mut HudText, &mut Node), (Without<CellBg>, Without<CellIcon>)>,
        Query<
            (&UtilityButtonBg, &mut Node),
            (Without<CellBg>, Without<CellIcon>, Without<CellCount>),
        >,
        Query<
            (&UtilityButtonText, &mut Node),
            (
                Without<CellBg>,
                Without<CellIcon>,
                Without<CellCount>,
                Without<UtilityButtonBg>,
            ),
        >,
    )>,
) {
    let win = [window.width(), window.height()];
    let s = inventory::scale(win);
    let pos = panel_pos(win);
    let size = panel_size(win);
    if let Ok(mut root) = q.p0().single_mut() {
        place(&mut root, pos[0], pos[1], size[0], size[1]);
    }

    let stack = |i: usize| panel.slots.get(i).copied().flatten();
    if held.and_then(|h| h.0).is_none()
        && let Some(overlay) = tooltip.as_deref_mut()
        && let Some(c) = window.cursor_position()
        && let Some(slot) = slot_at([c.x, c.y], win)
        && let Some(st) = stack(slot)
    {
        overlay.candidate = Some(super::tooltip::TooltipCandidate {
            target_id: 3_000 + slot as u64,
            stack: st,
            equipped: None,
            price_override: None,
        });
    }
    for (b, mut c, mut node) in &mut q.p1() {
        let [x, y, cell] = cell_rect(b.0, win);
        place(&mut node, x, y, cell, cell);
        let want = if stack(b.0).is_some() {
            CELL_FULL
        } else {
            CELL_BG
        };
        if c.0 != want {
            c.0 = want;
        }
    }
    let IconAssets {
        assets,
        images: mut image_assets,
    } = icon_assets;
    if let Some(mut assets) = assets {
        for (ic, mut node, mut img, mut vis) in &mut q.p2() {
            if let Some(st) = stack(ic.0) {
                let [x, y, sz] = cell_rect(ic.0, win);
                let (handle, icon_size) = icon_of(&mut assets, &mut image_assets, st.id);
                let d = fit_icon(icon_size, SLOT_SCALE);
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
            } else {
                set_vis(&mut vis, false);
            }
        }
    }
    for (n, mut t, mut node) in &mut q.p3() {
        let [x, y, sz] = cell_rect(n.0, win);
        let want = stack(n.0).as_ref().map(count_of).unwrap_or_default();
        let tw = fonts
            .as_ref()
            .map(|f| measure(&f.stack.0, &want))
            .unwrap_or(0.0);
        node.left = Val::Px(x + sz - tw - 4.0 * s);
        node.top = Val::Px(y + sz - 18.0 * s);
        if t.text != want {
            t.text = want;
        }
    }
    for (btn, mut node) in &mut q.p4() {
        let [bx, by] = [(BTN_X) * s, (CHEST_GRID[1] + btn.0 as f32 * BTN_PITCH) * s];
        place(&mut node, bx, by, BTN_W * s, BTN_H * s);
    }
    for (btn, mut node) in &mut q.p5() {
        let [bx, by] = [(BTN_X) * s, (CHEST_GRID[1] + btn.0 as f32 * BTN_PITCH) * s];
        node.left = Val::Px(bx + 8.0 * s);
        node.top = Val::Px(by + 6.0 * s);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
    use terravibe_sim::net::{ClientMsg, ClientTransport, Disconnected, ServerMsg};
    use terravibe_sim::slots::{ClickKind, SlotRef};

    use super::*;
    use crate::hud::inventory::{BAG_BOTTOM, InventoryOpen};
    use crate::net::Net;

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

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<ChestMsg>()
            .init_resource::<ChestPanel>()
            .init_resource::<InventoryOpen>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Assets<Image>>();
        app.world_mut().spawn((PanelRoot, Visibility::Hidden));
        let mut window = Window::default();
        window.resolution.set(1920.0, 1080.0);
        let opts = CursorOptions {
            grab_mode: CursorGrabMode::Locked,
            visible: false,
            ..default()
        };
        app.world_mut().spawn((window, PrimaryWindow, opts));
        app
    }

    fn set_cursor(app: &mut App, pos: [f32; 2]) {
        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .unwrap()
            .set_cursor_position(Some(Vec2::from(pos)));
    }

    fn press_mouse(app: &mut App, button: MouseButton) {
        let mut m = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        m.clear();
        m.press(button);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(button);
    }

    fn cell_center(i: usize, win: [f32; 2]) -> [f32; 2] {
        let p = panel_pos(win);
        let [x, y, cell] = cell_rect(i, win);
        [p[0] + x + cell / 2.0, p[1] + y + cell / 2.0]
    }

    #[test]
    fn chest_grid_slot_hit_testing() {
        let g = [100.0, 200.0];
        assert_eq!(cell_at([100.0, 200.0], g), Some(0));
        assert_eq!(cell_at([100.0 + PITCH * 9.5, 200.0], g), Some(9));
        assert_eq!(cell_at([100.0, 200.0 + PITCH], g), Some(10));
        assert_eq!(
            cell_at([100.0 + PITCH * 9.9, 200.0 + PITCH * 3.9], g),
            Some(39)
        );
        assert_eq!(cell_at([99.0, 200.0], g), None);
        assert_eq!(cell_at([100.0, 199.0], g), None);
        assert_eq!(cell_at([100.0 + PITCH * 10.0, 200.0], g), None);
        assert_eq!(cell_at([100.0, 200.0 + PITCH * 4.0], g), None);
    }

    #[test]
    fn opening_chest_opens_inventory_below() {
        let mut app = app();
        app.add_systems(Update, (receive, update_cells).chain());
        let pos = [4, 12, -3];
        app.world_mut().write_message(ChestMsg::Contents {
            pos,
            slots: vec![None; CHEST_SLOTS],
        });
        app.update();

        assert!(
            app.world().resource::<InventoryOpen>().0,
            "opening a chest opens the inventory alongside it"
        );
        assert_eq!(app.world().resource::<ChestPanel>().open, Some(pos));
        for win in [[1920.0, 1080.0], [1280.0, 720.0]] {
            let p = panel_pos(win);
            let s = crate::hud::inventory::scale(win);
            assert!(
                p[1] >= BAG_BOTTOM * s,
                "the chest grid sits below the bag grid at {win:?}"
            );
        }
        let grab = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap()
            .grab_mode;
        assert_eq!(grab, CursorGrabMode::None);
    }

    #[test]
    fn chest_slot_click_sends_clickslot_chest() {
        let mut app = app();
        let sent = Arc::new(Mutex::new(Vec::new()));
        app.insert_resource(Net::new(Recorder(sent.clone())));
        app.world_mut().resource_mut::<ChestPanel>().open = Some([0, 0, 0]);
        app.add_systems(Update, handle_input);

        set_cursor(&mut app, cell_center(3, [1920.0, 1080.0]));
        press_mouse(&mut app, MouseButton::Left);
        let msgs = sent.lock().unwrap().clone();
        assert!(
            matches!(
                msgs[..],
                [ClientMsg::ClickSlot {
                    at: SlotRef::Chest(3),
                    kind: ClickKind::Left
                }]
            ),
            "{msgs:?}"
        );

        sent.lock().unwrap().clear();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        set_cursor(&mut app, cell_center(17, [1920.0, 1080.0]));
        press_mouse(&mut app, MouseButton::Left);
        let msgs = sent.lock().unwrap().clone();
        assert!(
            matches!(
                msgs[..],
                [ClientMsg::ChestMove {
                    to_chest: false,
                    slot: 17
                }]
            ),
            "{msgs:?}"
        );
    }

    #[test]
    fn no_click_sent_when_chest_closed() {
        let mut app = app();
        let sent = Arc::new(Mutex::new(Vec::new()));
        app.insert_resource(Net::new(Recorder(sent.clone())));
        app.add_systems(Update, handle_input);

        set_cursor(&mut app, cell_center(3, [1920.0, 1080.0]));
        press_mouse(&mut app, MouseButton::Left);
        assert!(
            sent.lock().unwrap().is_empty(),
            "a closed chest must not send ClickSlot"
        );
    }

    #[test]
    fn chest_slot_hit_testing_scales_at_1080_and_720() {
        for win in [[1920.0, 1080.0], [1280.0, 720.0]] {
            let [_, _, cell] = cell_rect(0, win);
            let s = crate::hud::inventory::scale(win);
            assert!((cell - CELL * s).abs() < 1e-6, "{win:?}");

            for i in [0, 3, 17, 39] {
                let c = cell_center(i, win);
                assert_eq!(slot_at(c, win), Some(i), "{win:?} cell {i}");
            }
            // A point below the 10x4 grid is not a cell.
            let p = panel_pos(win);
            assert_eq!(
                slot_at(
                    [p[0] + 1.0, p[1] + (CHEST_GRID[1] + 4.0 * PITCH + 1.0) * s],
                    win
                ),
                None
            );
            // Outside the panel.
            assert_eq!(slot_at([p[0] - 1.0, p[1]], win), None);
        }
    }

    #[test]
    fn panel_fits_1280x720() {
        for win in [[1920.0, 1080.0], [1280.0, 720.0]] {
            let p = panel_pos(win);
            let size = panel_size(win);
            assert!(p[0] + size[0] <= 580.0 * crate::hud::inventory::scale(win));
            assert!(p[1] + size[1] <= win[1]);
        }
    }

    #[test]
    fn chest_utility_buttons_actions() {
        use crate::net::LocalInventory;
        use terravibe_sim::inventory::Inventory;
        use terravibe_sim::item::{COPPER_PICKAXE, Prefix, WOOD};

        let mut app = app();
        let sent = Arc::new(Mutex::new(Vec::new()));
        app.insert_resource(Net::new(Recorder(sent.clone())));
        app.world_mut().resource_mut::<ChestPanel>().open = Some([0, 0, 0]);

        let mut chest_slots = vec![None; CHEST_SLOTS];
        chest_slots[2] = Some(ItemStack {
            id: WOOD,
            count: 50,
            prefix: Prefix(0),
        });
        app.world_mut().resource_mut::<ChestPanel>().slots = chest_slots;

        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack {
            id: COPPER_PICKAXE,
            count: 1,
            prefix: Prefix(0),
        });
        inv.slots[1] = Some(ItemStack {
            id: WOOD,
            count: 20,
            prefix: Prefix(0),
        });
        app.insert_resource(LocalInventory(inv));
        app.add_systems(Update, handle_input);

        let win = [1920.0, 1080.0];

        // 1. Loot All button: index 0
        let [x0, y0, w0, h0] = utility_button_rect(0, win);
        set_cursor(&mut app, [x0 + w0 / 2.0, y0 + h0 / 2.0]);
        press_mouse(&mut app, MouseButton::Left);
        let msgs = sent.lock().unwrap().clone();
        assert!(
            msgs.iter().any(|m| matches!(
                m,
                ClientMsg::ChestMove {
                    to_chest: false,
                    slot: 2,
                }
            )),
            "Loot all moves chest slot 2 to inventory: {msgs:?}"
        );

        // 2. Deposit All button: index 1
        sent.lock().unwrap().clear();
        let [x1, y1, w1, h1] = utility_button_rect(1, win);
        set_cursor(&mut app, [x1 + w1 / 2.0, y1 + h1 / 2.0]);
        press_mouse(&mut app, MouseButton::Left);
        let msgs = sent.lock().unwrap().clone();
        assert!(
            msgs.iter().any(|m| matches!(
                m,
                ClientMsg::ChestMove {
                    to_chest: true,
                    slot: 0,
                }
            )) && msgs.iter().any(|m| matches!(
                m,
                ClientMsg::ChestMove {
                    to_chest: true,
                    slot: 1,
                }
            )),
            "Deposit all moves inventory slots to chest: {msgs:?}"
        );

        // 3. Quick Stack button: index 2
        sent.lock().unwrap().clear();
        let [x2, y2, w2, h2] = utility_button_rect(2, win);
        set_cursor(&mut app, [x2 + w2 / 2.0, y2 + h2 / 2.0]);
        press_mouse(&mut app, MouseButton::Left);
        let msgs = sent.lock().unwrap().clone();
        assert!(
            msgs.iter().any(|m| matches!(m, ClientMsg::ChestQuickStack)),
            "Quick stack sends ChestQuickStack: {msgs:?}"
        );

        // 4. Restock button: index 3
        sent.lock().unwrap().clear();
        let [x3, y3, w3, h3] = utility_button_rect(3, win);
        set_cursor(&mut app, [x3 + w3 / 2.0, y3 + h3 / 2.0]);
        press_mouse(&mut app, MouseButton::Left);
        let msgs = sent.lock().unwrap().clone();
        assert!(
            msgs.iter().any(|m| matches!(
                m,
                ClientMsg::ChestMove {
                    to_chest: false,
                    slot: 2,
                }
            )),
            "Restock moves matching chest item (WOOD slot 2) to inventory: {msgs:?}"
        );
    }
}
