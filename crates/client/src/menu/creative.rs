//! Creative Mode item spawner and quick-action overlay (`F2` or Pause -> Creative Mode).

use std::collections::HashMap;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::mouse::{MouseButton, MouseWheel};
use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;
use bevy::window::{CursorOptions, PrimaryWindow, Window};
use terravibe_sim::item::{
    BUG_NET, GUIDE_VOODOO_DOLL, ItemClass, ItemId, SUSPICIOUS_LOOKING_EYE, WORM_FOOD, all_item_ids,
    item_data,
};
use terravibe_sim::net::{ClientMsg, CreativeCmd};

use super::style::{
    COLOR_BACKDROP, COLOR_BORDER_NORMAL, COLOR_BORDER_SELECTED, COLOR_BUTTON_NORMAL,
    COLOR_BUTTON_SELECTED, COLOR_HEADER_DIVIDER, COLOR_INPUT_ACTIVE_BORDER, COLOR_INPUT_BG,
    COLOR_PANEL_BG, COLOR_PANEL_BORDER, COLOR_PANEL_INNER, COLOR_TEXT_GOLD, COLOR_TEXT_MUTED,
    COLOR_TEXT_PRIMARY, COLOR_TEXT_SUCCESS, viewport_size,
};
use super::text::{MenuText, MenuTextColor};
use crate::hud::inventory::IconAssets;
use crate::net::{ClientGodMode, Net};
use crate::session::{AppState, SessionError};
use crate::view::camera::set_grab;

const GRID_COLS: usize = 5;
const GRID_ROWS: usize = 5;
const ITEMS_PER_PAGE: usize = GRID_COLS * GRID_ROWS;
const DAY_TIME_TICKS: u32 = 16_200;
const NIGHT_TIME_TICKS: u32 = terravibe_sim::time::DAY_TICKS + 5_400;
const SEARCH_MAX_CHARS: usize = 32;

/// Whether the Creative Mode spawner overlay is currently open.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreativeMenuOpen(pub bool);

/// Item filter category in the Creative Mode browser.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CreativeCategory {
    #[default]
    All,
    Weapons,
    Tools,
    Armor,
    Accessories,
    Potions,
    Blocks,
    BossSummon,
    Misc,
}

impl CreativeCategory {
    pub const ALL: [Self; 9] = [
        Self::All,
        Self::Weapons,
        Self::Tools,
        Self::Armor,
        Self::Accessories,
        Self::Potions,
        Self::Blocks,
        Self::BossSummon,
        Self::Misc,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Weapons => "Weapons",
            Self::Tools => "Tools",
            Self::Armor => "Armor",
            Self::Accessories => "Accessories",
            Self::Potions => "Potions",
            Self::Blocks => "Blocks",
            Self::BossSummon => "Boss/Summon",
            Self::Misc => "Misc",
        }
    }
}

/// Returns true if `id` belongs to `category`.
pub fn item_matches_category(id: ItemId, category: CreativeCategory) -> bool {
    let d = item_data(id);
    let is_boss_summon = matches!(id, SUSPICIOUS_LOOKING_EYE | WORM_FOOD | GUIDE_VOODOO_DOLL);
    let is_potion = d.class == ItemClass::Consumable
        && (d.buff_secs > 0 || d.name.contains("Potion") || d.name == "Mushroom");
    let is_tool = d.class == ItemClass::Tool || id == BUG_NET;

    match category {
        CreativeCategory::All => true,
        CreativeCategory::Weapons => {
            matches!(
                d.class,
                ItemClass::Melee | ItemClass::Ranged | ItemClass::Magic
            ) || (d.class == ItemClass::Consumable && d.damage > 0)
        }
        CreativeCategory::Tools => is_tool,
        CreativeCategory::Armor => d.class == ItemClass::Armor,
        CreativeCategory::Accessories => d.class == ItemClass::Accessory,
        CreativeCategory::Potions => is_potion,
        CreativeCategory::Blocks => {
            matches!(
                d.class,
                ItemClass::Block | ItemClass::Placeable | ItemClass::Wall
            )
        }
        CreativeCategory::BossSummon => is_boss_summon,
        CreativeCategory::Misc => {
            !(matches!(
                d.class,
                ItemClass::Melee
                    | ItemClass::Ranged
                    | ItemClass::Magic
                    | ItemClass::Armor
                    | ItemClass::Accessory
                    | ItemClass::Block
                    | ItemClass::Placeable
                    | ItemClass::Wall
            ) || (d.class == ItemClass::Consumable && d.damage > 0))
                && !is_tool
                && !is_potion
                && !is_boss_summon
        }
    }
}

/// Filters `all_item_ids()` by `category` and case-insensitive substring match on item name.
pub fn filter_creative_items(search: &str, category: CreativeCategory) -> Vec<ItemId> {
    let q = search.trim().to_ascii_lowercase();
    all_item_ids()
        .filter(|&id| item_matches_category(id, category))
        .filter(|&id| q.is_empty() || item_data(id).name.to_ascii_lowercase().contains(&q))
        .collect()
}

/// Sensible test count for left-click, or `max_stack` for right-click.
pub fn creative_give_count(id: ItemId, right_click: bool) -> u32 {
    let d = item_data(id);
    let max_stack = d.max_stack.max(1);
    if right_click {
        return max_stack;
    }
    if matches!(
        id,
        SUSPICIOUS_LOOKING_EYE | WORM_FOOD | GUIDE_VOODOO_DOLL | BUG_NET
    ) {
        return 1;
    }
    let desired = match d.class {
        ItemClass::Melee
        | ItemClass::Ranged
        | ItemClass::Magic
        | ItemClass::Tool
        | ItemClass::Armor
        | ItemClass::Accessory => 1,
        ItemClass::Consumable => 10,
        ItemClass::Block
        | ItemClass::Placeable
        | ItemClass::Wall
        | ItemClass::Ammo
        | ItemClass::Other => 99,
    };
    desired.min(max_stack)
}

/// UI state for the Creative Mode spawner.
#[derive(Resource, Default, Debug, Clone)]
pub struct CreativeState {
    pub search: String,
    pub category: CreativeCategory,
    pub page: usize,
    pub status: String,
}

#[derive(Resource, Default)]
struct CreativeIconCache(HashMap<u16, Handle<Image>>);

#[derive(Component)]
pub struct CreativeUiRoot;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreativeControl {
    FullHeal,
    ToggleGodMode,
    SetDay,
    SetNight,
    Close,
    Category(CreativeCategory),
    PrevPage,
    NextPage,
    ItemCell(usize),
}

#[derive(Component)]
struct CreativeSearchText;

#[derive(Component)]
struct CreativeGodModeText;

#[derive(Component)]
struct CreativePageText;

#[derive(Component)]
struct CreativeStatusText;

#[derive(Component)]
struct CreativeCellIcon(usize);

#[derive(Component)]
struct CreativeCellLabel(usize);

fn total_pages(item_count: usize) -> usize {
    item_count.max(1).div_ceil(ITEMS_PER_PAGE)
}

fn truncate_label(name: &str, max_chars: usize) -> String {
    let mut chars = name.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}..", &head[..head.len().saturating_sub(1)])
    } else {
        head
    }
}

fn load_item_icon(
    id: ItemId,
    cache: &mut CreativeIconCache,
    icon_assets: &mut IconAssets,
) -> Option<Handle<Image>> {
    if let Some(handle) = cache.0.get(&id.0) {
        return Some(handle.clone());
    }
    let assets = icon_assets.assets.as_mut()?;
    let (handle, _) = crate::hud::hotbar::icon_of(assets, &mut icon_assets.images, id);
    cache.0.insert(id.0, handle.clone());
    Some(handle)
}

fn handle_creative_input(
    open: Res<CreativeMenuOpen>,
    mut state: ResMut<CreativeState>,
    mut key_events: Option<MessageReader<KeyboardInput>>,
    mut wheel_events: Option<MessageReader<MouseWheel>>,
) {
    if !open.0 {
        return;
    }
    if let Some(ref mut reader) = key_events {
        for ev in reader.read() {
            if ev.state != ButtonState::Pressed {
                continue;
            }
            if matches!(ev.key_code, KeyCode::Escape | KeyCode::F2) {
                continue;
            }
            if ev.key_code == KeyCode::Backspace {
                if state.search.pop().is_some() {
                    state.page = 0;
                }
                continue;
            }
            match &ev.logical_key {
                Key::Character(s) => {
                    for ch in s.chars() {
                        if ch.is_ascii()
                            && !ch.is_control()
                            && state.search.chars().count() < SEARCH_MAX_CHARS
                        {
                            state.search.push(ch);
                            state.page = 0;
                        }
                    }
                }
                Key::Space => {
                    if state.search.chars().count() < SEARCH_MAX_CHARS {
                        state.search.push(' ');
                        state.page = 0;
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(ref mut wheel) = wheel_events {
        let items = filter_creative_items(&state.search, state.category);
        let pages = total_pages(items.len());
        for ev in wheel.read() {
            if ev.y < 0.0 && state.page + 1 < pages {
                state.page += 1;
            } else if ev.y > 0.0 && state.page > 0 {
                state.page -= 1;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_creative_clicks(
    mut open: ResMut<CreativeMenuOpen>,
    mut state: ResMut<CreativeState>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    controls: Query<(
        &CreativeControl,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&Visibility>,
    )>,
    net: Option<Res<Net>>,
    mut clock: Option<ResMut<crate::net::ClientClock>>,
) {
    if !open.0 {
        return;
    }
    let Some(mouse) = mouse else { return };
    let left = mouse.just_pressed(MouseButton::Left);
    let right = mouse.just_pressed(MouseButton::Right);
    if !left && !right {
        return;
    }
    let Ok(win) = windows.single() else { return };
    let Some(cursor_phys) = win.cursor_position() else {
        return;
    };
    let cursor = cursor_phys * win.scale_factor();

    let mut hit = None;
    for (ctrl, node, xform, vis) in &controls {
        if matches!(vis, Some(Visibility::Hidden)) {
            continue;
        }
        let center = xform.translation;
        let half = node.size() * 0.5;
        if (cursor.x - center.x).abs() <= half.x && (cursor.y - center.y).abs() <= half.y {
            hit = Some(*ctrl);
            break;
        }
    }
    let Some(ctrl) = hit else { return };

    match ctrl {
        CreativeControl::Close if left => {
            open.0 = false;
            if let Ok(mut opts) = cursor_query.single_mut() {
                set_grab(&mut opts, true);
            }
        }
        CreativeControl::FullHeal if left => {
            if let Some(ref net) = net {
                let _ = net.send(ClientMsg::CreativeAction(CreativeCmd::FullHeal));
            }
            state.status = "Full Heal sent".into();
        }
        CreativeControl::ToggleGodMode if left => {
            if let Some(ref net) = net {
                let _ = net.send(ClientMsg::CreativeAction(CreativeCmd::ToggleGodMode));
            }
            state.status = "Toggled God Mode".into();
        }
        CreativeControl::SetDay if left => {
            if let Some(ref net) = net {
                let _ = net.send(ClientMsg::CreativeAction(CreativeCmd::SetTime(
                    DAY_TIME_TICKS,
                )));
            }
            if let Some(ref mut clock) = clock {
                clock.tick = DAY_TIME_TICKS;
                clock.override_tick = None;
            }
            state.status = "Time set to Day (09:00)".into();
        }
        CreativeControl::SetNight if left => {
            if let Some(ref net) = net {
                let _ = net.send(ClientMsg::CreativeAction(CreativeCmd::SetTime(
                    NIGHT_TIME_TICKS,
                )));
            }
            if let Some(ref mut clock) = clock {
                clock.tick = NIGHT_TIME_TICKS;
                clock.override_tick = None;
            }
            state.status = "Time set to Night (21:00)".into();
        }
        CreativeControl::Category(cat) if left => {
            state.category = cat;
            state.page = 0;
        }
        CreativeControl::PrevPage if left => {
            state.page = state.page.saturating_sub(1);
        }
        CreativeControl::NextPage if left => {
            let items = filter_creative_items(&state.search, state.category);
            let pages = total_pages(items.len());
            if state.page + 1 < pages {
                state.page += 1;
            }
        }
        CreativeControl::ItemCell(slot_idx) => {
            let items = filter_creative_items(&state.search, state.category);
            let idx = state.page * ITEMS_PER_PAGE + slot_idx;
            if let Some(&item) = items.get(idx) {
                let count = creative_give_count(item, right);
                if let Some(ref net) = net {
                    let _ = net.send(ClientMsg::CreativeGive { item, count });
                }
                state.status = format!("Gave {}x {}", count, item_data(item).name);
            }
        }
        _ => {}
    }
}

fn spawn_creative_ui(commands: &mut Commands, viewport: (f32, f32)) {
    let (win_w, win_h) = viewport;
    let panel_w = (win_w - 80.0).clamp(640.0, 920.0);
    let panel_h = (win_h - 60.0).clamp(480.0, 660.0);
    let panel_x = (win_w - panel_w) * 0.5;
    let panel_y = (win_h - panel_h) * 0.5;

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(COLOR_BACKDROP),
        GlobalZIndex(1299),
        CreativeUiRoot,
    ));

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x),
                top: Val::Px(panel_y),
                width: Val::Px(panel_w),
                height: Val::Px(panel_h),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(COLOR_PANEL_BG),
            BorderColor::all(COLOR_PANEL_BORDER),
            GlobalZIndex(1300),
            CreativeUiRoot,
        ))
        .with_children(|root| {
            // Header + Quick Actions
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                },
                CreativeUiRoot,
            ))
            .with_children(|hdr| {
                hdr.spawn((
                    MenuText::new("Creative Mode", 1.25),
                    MenuTextColor(COLOR_TEXT_GOLD),
                    CreativeUiRoot,
                ));
                hdr.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },
                    CreativeUiRoot,
                ))
                .with_children(|bar| {
                    for (label, ctrl) in [
                        ("Full Heal", CreativeControl::FullHeal),
                        ("God Mode: OFF", CreativeControl::ToggleGodMode),
                        ("Set Day", CreativeControl::SetDay),
                        ("Set Night", CreativeControl::SetNight),
                        ("Close", CreativeControl::Close),
                    ] {
                        bar.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(COLOR_BUTTON_NORMAL),
                            BorderColor::all(COLOR_BORDER_NORMAL),
                            GlobalZIndex(1301),
                            ctrl,
                            CreativeUiRoot,
                        ))
                        .with_children(|b| {
                            let mut text = b.spawn((MenuText::new(label, 0.8), CreativeUiRoot));
                            if ctrl == CreativeControl::ToggleGodMode {
                                text.insert(CreativeGodModeText);
                            }
                        });
                    }
                });
            });

            // Divider
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(COLOR_HEADER_DIVIDER),
                CreativeUiRoot,
            ));

            // Search bar
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(32.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(COLOR_INPUT_BG),
                BorderColor::all(COLOR_INPUT_ACTIVE_BORDER),
                CreativeUiRoot,
            ))
            .with_children(|s| {
                s.spawn((
                    MenuText::new("Search: (type to filter items...)", 0.85),
                    MenuTextColor(COLOR_TEXT_PRIMARY),
                    CreativeSearchText,
                    CreativeUiRoot,
                ));
            });

            // Category tabs
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(5.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                CreativeUiRoot,
            ))
            .with_children(|tabs| {
                for cat in CreativeCategory::ALL {
                    tabs.spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(9.0), Val::Px(5.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(COLOR_BUTTON_NORMAL),
                        BorderColor::all(COLOR_BORDER_NORMAL),
                        GlobalZIndex(1301),
                        CreativeControl::Category(cat),
                        CreativeUiRoot,
                    ))
                    .with_children(|t| {
                        t.spawn((MenuText::new(cat.label(), 0.75), CreativeUiRoot));
                    });
                }
            });

            // 5x5 Item Grid
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    display: Display::Grid,
                    grid_template_columns: RepeatedGridTrack::flex(GRID_COLS as u16, 1.0),
                    grid_template_rows: RepeatedGridTrack::flex(GRID_ROWS as u16, 1.0),
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                CreativeUiRoot,
            ))
            .with_children(|grid| {
                for idx in 0..ITEMS_PER_PAGE {
                    grid.spawn((
                        Node {
                            padding: UiRect::all(Val::Px(6.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0),
                            ..default()
                        },
                        BackgroundColor(COLOR_PANEL_INNER),
                        BorderColor::all(COLOR_BORDER_NORMAL),
                        GlobalZIndex(1301),
                        CreativeControl::ItemCell(idx),
                        CreativeUiRoot,
                    ))
                    .with_children(|cell| {
                        cell.spawn((
                            Node {
                                width: Val::Px(24.0),
                                height: Val::Px(24.0),
                                flex_shrink: 0.0,
                                ..default()
                            },
                            ImageNode::default(),
                            CreativeCellIcon(idx),
                            CreativeUiRoot,
                        ));
                        cell.spawn((
                            MenuText::new("", 0.72),
                            MenuTextColor(COLOR_TEXT_PRIMARY),
                            CreativeCellLabel(idx),
                            CreativeUiRoot,
                        ));
                    });
                }
            });

            // Footer: Pagination + Status
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                },
                CreativeUiRoot,
            ))
            .with_children(|ftr| {
                ftr.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    CreativeUiRoot,
                ))
                .with_children(|pg| {
                    pg.spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(COLOR_BUTTON_NORMAL),
                        BorderColor::all(COLOR_BORDER_NORMAL),
                        GlobalZIndex(1301),
                        CreativeControl::PrevPage,
                        CreativeUiRoot,
                    ))
                    .with_children(|b| {
                        b.spawn((MenuText::new("Prev", 0.78), CreativeUiRoot));
                    });

                    pg.spawn((
                        MenuText::new("Page 1 / 1", 0.8),
                        MenuTextColor(COLOR_TEXT_MUTED),
                        CreativePageText,
                        CreativeUiRoot,
                    ));

                    pg.spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(COLOR_BUTTON_NORMAL),
                        BorderColor::all(COLOR_BORDER_NORMAL),
                        GlobalZIndex(1301),
                        CreativeControl::NextPage,
                        CreativeUiRoot,
                    ))
                    .with_children(|b| {
                        b.spawn((MenuText::new("Next", 0.78), CreativeUiRoot));
                    });
                });

                ftr.spawn((
                    MenuText::new("Left-Click: Give  |  Right-Click: Max Stack", 0.78),
                    MenuTextColor(COLOR_TEXT_SUCCESS),
                    CreativeStatusText,
                    CreativeUiRoot,
                ));
            });
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_creative_ui(
    mut commands: Commands,
    open: Res<CreativeMenuOpen>,
    session_error: Option<Res<SessionError>>,
    state: Res<CreativeState>,
    god_mode: Option<Res<ClientGodMode>>,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
    roots: Query<Entity, With<CreativeUiRoot>>,
    mut cache: ResMut<CreativeIconCache>,
    mut icon_assets: IconAssets,
    mut search_q: Query<
        &mut MenuText,
        (
            With<CreativeSearchText>,
            Without<CreativeGodModeText>,
            Without<CreativePageText>,
            Without<CreativeStatusText>,
            Without<CreativeCellLabel>,
        ),
    >,
    mut god_q: Query<
        &mut MenuText,
        (
            With<CreativeGodModeText>,
            Without<CreativeSearchText>,
            Without<CreativePageText>,
            Without<CreativeStatusText>,
            Without<CreativeCellLabel>,
        ),
    >,
    mut page_q: Query<
        &mut MenuText,
        (
            With<CreativePageText>,
            Without<CreativeSearchText>,
            Without<CreativeGodModeText>,
            Without<CreativeStatusText>,
            Without<CreativeCellLabel>,
        ),
    >,
    mut status_q: Query<
        &mut MenuText,
        (
            With<CreativeStatusText>,
            Without<CreativeSearchText>,
            Without<CreativeGodModeText>,
            Without<CreativePageText>,
            Without<CreativeCellLabel>,
        ),
    >,
    mut ctrl_q: Query<(
        &CreativeControl,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut Visibility,
    )>,
    mut icon_q: Query<
        (&CreativeCellIcon, &mut ImageNode, &mut Visibility),
        Without<CreativeControl>,
    >,
    mut label_q: Query<
        (&CreativeCellLabel, &mut MenuText),
        (
            Without<CreativeSearchText>,
            Without<CreativeGodModeText>,
            Without<CreativePageText>,
            Without<CreativeStatusText>,
        ),
    >,
) {
    let should_show = open.0 && session_error.as_ref().is_none_or(|e| e.0.is_none());
    let exists = roots.iter().next().is_some();

    if should_show && !exists {
        spawn_creative_ui(&mut commands, viewport_size(primary_window.as_ref()));
        return;
    }
    if !should_show {
        if exists {
            for e in &roots {
                commands.entity(e).queue_silenced(|entity: EntityWorldMut| {
                    entity.despawn();
                });
            }
        }
        return;
    }

    let items = filter_creative_items(&state.search, state.category);
    let pages = total_pages(items.len());
    let page = state.page.min(pages - 1);
    let start = page * ITEMS_PER_PAGE;

    for mut t in &mut search_q {
        t.text = if state.search.is_empty() {
            "Search: (type to filter items...)".into()
        } else {
            format!("Search: {}_", state.search)
        };
    }

    let god_on = god_mode.as_ref().is_some_and(|g| g.0);
    for mut t in &mut god_q {
        t.text = if god_on {
            "God Mode: ON".into()
        } else {
            "God Mode: OFF".into()
        };
    }

    for mut t in &mut page_q {
        t.text = format!("Page {} / {} ({} items)", page + 1, pages, items.len());
    }

    for mut t in &mut status_q {
        t.text = if state.status.is_empty() {
            "Left-Click: Give  |  Right-Click: Max Stack".into()
        } else {
            state.status.clone()
        };
    }

    for (ctrl, mut bg, mut border, mut vis) in &mut ctrl_q {
        match *ctrl {
            CreativeControl::ToggleGodMode => {
                bg.0 = if god_on {
                    COLOR_BUTTON_SELECTED
                } else {
                    COLOR_BUTTON_NORMAL
                };
                *border = BorderColor::all(if god_on {
                    COLOR_BORDER_SELECTED
                } else {
                    COLOR_BORDER_NORMAL
                });
            }
            CreativeControl::Category(cat) => {
                let active = cat == state.category;
                bg.0 = if active {
                    COLOR_BUTTON_SELECTED
                } else {
                    COLOR_BUTTON_NORMAL
                };
                *border = BorderColor::all(if active {
                    COLOR_BORDER_SELECTED
                } else {
                    COLOR_BORDER_NORMAL
                });
            }
            CreativeControl::ItemCell(slot_idx) => {
                let has_item = start + slot_idx < items.len();
                *vis = if has_item {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            _ => {}
        }
    }

    for (icon_slot, mut img_node, mut vis) in &mut icon_q {
        if let Some(&id) = items.get(start + icon_slot.0) {
            if let Some(handle) = load_item_icon(id, &mut cache, &mut icon_assets) {
                img_node.image = handle;
                *vis = Visibility::Inherited;
            } else {
                *vis = Visibility::Hidden;
            }
        } else {
            *vis = Visibility::Hidden;
        }
    }

    for (lbl_slot, mut text) in &mut label_q {
        text.text = if let Some(&id) = items.get(start + lbl_slot.0) {
            truncate_label(item_data(id).name, 16)
        } else {
            String::new()
        };
    }
}

fn despawn_creative_ui(
    mut commands: Commands,
    mut open: ResMut<CreativeMenuOpen>,
    roots: Query<Entity, With<CreativeUiRoot>>,
) {
    open.0 = false;
    for e in &roots {
        commands.entity(e).queue_silenced(|entity: EntityWorldMut| {
            entity.despawn();
        });
    }
}

pub struct CreativePlugin;

impl Plugin for CreativePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CreativeMenuOpen>()
            .init_resource::<CreativeState>()
            .init_resource::<CreativeIconCache>()
            .add_systems(
                Update,
                (
                    handle_creative_input,
                    handle_creative_clicks,
                    sync_creative_ui,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(OnExit(AppState::InGame), despawn_creative_ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::item::{
        COPPER_PICKAXE, DIRT_BLOCK, FIERY_GREATSWORD, IRON_HELMET, LAVA_CHARM,
        LESSER_HEALING_POTION, WOODEN_ARROW,
    };

    #[test]
    fn creative_filter_by_category_and_search() {
        let all = filter_creative_items("", CreativeCategory::All);
        assert_eq!(all.len(), all_item_ids().count());

        let weapons = filter_creative_items("Fiery", CreativeCategory::Weapons);
        assert!(weapons.contains(&FIERY_GREATSWORD));
        assert!(!weapons.contains(&DIRT_BLOCK));

        let tools = filter_creative_items("", CreativeCategory::Tools);
        assert!(tools.contains(&COPPER_PICKAXE));
        assert!(tools.contains(&BUG_NET));

        let armor = filter_creative_items("", CreativeCategory::Armor);
        assert!(armor.contains(&IRON_HELMET));

        let acc = filter_creative_items("", CreativeCategory::Accessories);
        assert!(acc.contains(&LAVA_CHARM));

        let potions = filter_creative_items("", CreativeCategory::Potions);
        assert!(potions.contains(&LESSER_HEALING_POTION));

        let blocks = filter_creative_items("", CreativeCategory::Blocks);
        assert!(blocks.contains(&DIRT_BLOCK));

        let summons = filter_creative_items("", CreativeCategory::BossSummon);
        assert!(summons.contains(&SUSPICIOUS_LOOKING_EYE));
        assert!(summons.contains(&WORM_FOOD));
        assert!(summons.contains(&GUIDE_VOODOO_DOLL));
    }

    #[test]
    fn creative_give_counts_left_and_right_click() {
        assert_eq!(creative_give_count(FIERY_GREATSWORD, false), 1);
        assert_eq!(creative_give_count(COPPER_PICKAXE, false), 1);
        assert_eq!(creative_give_count(IRON_HELMET, false), 1);
        assert_eq!(creative_give_count(LAVA_CHARM, false), 1);
        assert_eq!(creative_give_count(SUSPICIOUS_LOOKING_EYE, false), 1);
        assert_eq!(creative_give_count(LESSER_HEALING_POTION, false), 10);
        assert_eq!(creative_give_count(DIRT_BLOCK, false), 99);
        assert_eq!(creative_give_count(WOODEN_ARROW, false), 99);

        assert_eq!(
            creative_give_count(DIRT_BLOCK, true),
            item_data(DIRT_BLOCK).max_stack
        );
        assert_eq!(
            creative_give_count(LESSER_HEALING_POTION, true),
            item_data(LESSER_HEALING_POTION).max_stack
        );
    }

    #[test]
    fn creative_ui_buttons_have_explicit_z_above_panel() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_resource::<Assets<Image>>()
            .init_resource::<CreativeMenuOpen>()
            .init_resource::<CreativeState>()
            .init_resource::<CreativeIconCache>()
            .add_systems(Update, sync_creative_ui.run_if(in_state(AppState::InGame)));

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        app.world_mut().resource_mut::<CreativeMenuOpen>().0 = true;
        app.update();

        let mut panel_q = app
            .world_mut()
            .query_filtered::<(&BackgroundColor, Option<&GlobalZIndex>), (With<CreativeUiRoot>, Without<CreativeControl>)>();
        let panels: Vec<_> = panel_q
            .iter(app.world())
            .filter(|(bg, _)| bg.0 == COLOR_PANEL_BG)
            .collect();
        assert_eq!(panels.len(), 1, "expected 1 CreativeUiRoot panel");
        let panel_z = panels[0]
            .1
            .map(|z| z.0)
            .expect("Creative panel must have explicit GlobalZIndex");

        let mut ctrl_q = app
            .world_mut()
            .query_filtered::<(&CreativeControl, Option<&GlobalZIndex>), With<CreativeUiRoot>>();
        let controls: Vec<_> = ctrl_q.iter(app.world()).collect();
        // 5 quick actions + 9 categories + 2 pagination + 25 item cells = 41 controls
        assert_eq!(controls.len(), 41);
        for (ctrl, maybe_z) in controls {
            let z = maybe_z
                .map(|z| z.0)
                .unwrap_or_else(|| panic!("Creative control {ctrl:?} missing GlobalZIndex"));
            assert!(
                z > panel_z,
                "Creative control {ctrl:?} (z={z}) must be > panel (z={panel_z})"
            );
        }
    }

    #[test]
    fn creative_day_and_night_ticks_match_world_clock_and_sky_brightness() {
        use terravibe_sim::time::{NIGHT_FLOOR, WorldClock, sky_brightness};

        let day_clock = WorldClock {
            tick: DAY_TIME_TICKS,
        };
        assert!(day_clock.is_day());
        assert!((sky_brightness(DAY_TIME_TICKS) - 1.0).abs() < 1e-6);

        let night_clock = WorldClock {
            tick: NIGHT_TIME_TICKS,
        };
        assert!(!night_clock.is_day());
        assert!((sky_brightness(NIGHT_TIME_TICKS) - NIGHT_FLOOR).abs() < 1e-6);
    }
}
