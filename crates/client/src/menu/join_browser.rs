//! Join / Server Browser menu screen: 6-char join code or raw iroh ticket,
//! combined Public + [LAN] room list, background discovery, and password modal.

use std::sync::{Mutex, mpsc};
use std::time::Duration;

use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};

use super::style::{
    BORDER_W, BUTTON_HEIGHT, COLOR_BACKDROP, COLOR_HEADER_DIVIDER, COLOR_INPUT_ACTIVE_BORDER,
    COLOR_INPUT_BG, COLOR_PANEL_BG, COLOR_PANEL_BORDER, COLOR_PANEL_INNER, COLOR_TEXT_ERROR,
    COLOR_TEXT_GOLD, COLOR_TEXT_MUTED, RADIUS_BTN, RADIUS_PANEL, spawn_button_sized, viewport_size,
};
use super::text::{MenuText, MenuTextColor};
use super::{Action, MenuFocus};
use crate::session::{AppState, MenuScreen, SessionMode};

const LAN_DISCOVERY_TIMEOUT: Duration = Duration::from_millis(600);
const MAX_CODE_INPUT_CHARS: usize = 512;
const MAX_PASSWORD_CHARS: usize = 32;

/// Where a discovered room originated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoomSource {
    Public { code: String },
    Lan { ticket: String },
}

/// Unified entry displayed in the server browser list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserRoom {
    pub world_name: String,
    pub host_name: String,
    pub players: u8,
    pub max_players: u8,
    pub has_password: bool,
    pub source: RoomSource,
}

/// Pending room awaiting password input in the modal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingPasswordJoin {
    pub world_name: String,
    pub source: RoomSource,
}

struct RefreshResult {
    rooms: Vec<BrowserRoom>,
    status: Option<String>,
}

enum ResolveOutcome {
    Ready {
        ticket: String,
        password: Option<String>,
    },
    NeedsPassword {
        world_name: String,
        ticket: String,
    },
    Error(String),
}

/// State of the Join / Server Browser screen.
#[derive(Resource, Default)]
pub struct JoinBrowserState {
    pub code_input: String,
    pub rooms: Vec<BrowserRoom>,
    pub status: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
    pub password_modal: Option<PendingPasswordJoin>,
    pub password_input: String,
    refresh_rx: Option<Mutex<mpsc::Receiver<RefreshResult>>>,
    resolve_rx: Option<Mutex<mpsc::Receiver<ResolveOutcome>>>,
}

#[derive(Resource, Default)]
pub struct JoinBrowserRebuild(pub bool);

#[derive(Component)]
pub struct JoinBrowserRoot;

#[derive(Component)]
pub struct JoinCodeValueText;

#[derive(Component)]
pub struct JoinPasswordValueText;

#[derive(Component)]
pub struct JoinRoomRow;

/// Returns true if `input` looks like a 6-character lobby code rather than a raw iroh ticket.
pub fn is_lobby_code(input: &str) -> bool {
    let trimmed = input.trim();
    trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_alphanumeric())
}

fn spawn_refresh_worker() -> mpsc::Receiver<RefreshResult> {
    let (tx, rx) = mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("terravibe-browser-refresh".into())
        .spawn(move || {
            let mut rooms = Vec::new();
            let mut lobby_note: Option<String> = None;

            for lan in terravibe_net::discover_lan_rooms(LAN_DISCOVERY_TIMEOUT) {
                rooms.push(BrowserRoom {
                    world_name: lan.world_name,
                    host_name: lan.host_name,
                    players: lan.players,
                    max_players: lan.max_players,
                    has_password: lan.has_password,
                    source: RoomSource::Lan { ticket: lan.ticket },
                });
            }

            match terravibe_net::LobbyClient::from_env() {
                Some(client) => match client.list_rooms() {
                    Ok(public_rooms) => {
                        for r in public_rooms {
                            rooms.push(BrowserRoom {
                                world_name: r.world_name,
                                host_name: r.host_name,
                                players: r.players,
                                max_players: r.max_players,
                                has_password: r.has_password,
                                source: RoomSource::Public { code: r.code },
                            });
                        }
                    }
                    Err(_) => {
                        lobby_note = Some("Lobby unreachable; showing LAN rooms only.".into());
                    }
                },
                None => {
                    lobby_note = Some("LAN discovery active (set TERRAVIBE_LOBBY_URL for public rooms).".into());
                }
            }

            let _ = tx.send(RefreshResult {
                rooms,
                status: lobby_note,
            });
        });
    rx
}

fn spawn_resolve_worker(code: String, password: Option<String>) -> mpsc::Receiver<ResolveOutcome> {
    let (tx, rx) = mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("terravibe-code-resolve".into())
        .spawn(move || {
            let Some(client) = terravibe_net::LobbyClient::from_env() else {
                let _ = tx.send(ResolveOutcome::Error(
                    "Lobby URL not configured; paste a raw host ticket instead.".into(),
                ));
                return;
            };
            match client.resolve_room(&code) {
                Ok(resp) => {
                    if resp.has_password && password.is_none() {
                        let _ = tx.send(ResolveOutcome::NeedsPassword {
                            world_name: resp.world_name,
                            ticket: resp.ticket,
                        });
                    } else {
                        let _ = tx.send(ResolveOutcome::Ready {
                            ticket: resp.ticket,
                            password,
                        });
                    }
                }
                Err(e) => {
                    let _ = tx.send(ResolveOutcome::Error(format!(
                        "Could not resolve code '{code}': {e}"
                    )));
                }
            }
        });
    rx
}

fn format_code_display(input: &str) -> String {
    if input.is_empty() {
        "> Enter 6-char code or ticket...".to_string()
    } else if input.chars().count() > 36 {
        let tail: String = input
            .chars()
            .rev()
            .take(33)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("> ...{tail}")
    } else {
        format!("> {input}")
    }
}

fn format_modal_password(password: &str) -> String {
    if password.is_empty() {
        "> ".to_string()
    } else {
        format!("> {}", "*".repeat(password.chars().count()))
    }
}

fn format_room_title(room: &BrowserRoom) -> String {
    let prefix = match room.source {
        RoomSource::Lan { .. } => "[LAN] ",
        RoomSource::Public { .. } => "",
    };
    let lock = if room.has_password { "  [LOCK]" } else { "" };
    format!("{prefix}{}{lock}", room.world_name)
}

pub fn rebuild_join_browser_ui(
    commands: &mut Commands,
    state: &JoinBrowserState,
    focus: &mut MenuFocus,
    viewport: (f32, f32),
) {
    let (win_w, win_h) = viewport;
    let center_x = win_w * 0.5;
    let panel_w = 780.0;
    let panel_h = 620.0;
    let panel_top = ((win_h - panel_h) * 0.5).max(24.0);
    let modal_open = state.password_modal.is_some();

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(win_w),
            height: Val::Px(win_h),
            ..default()
        },
        BackgroundColor(COLOR_BACKDROP),
        DespawnOnExit(MenuScreen::JoinBrowser),
        JoinBrowserRoot,
    ));

    // Main panel
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(center_x - panel_w * 0.5),
                top: Val::Px(panel_top),
                width: Val::Px(panel_w),
                height: Val::Px(panel_h),
                border: UiRect::all(Val::Px(BORDER_W)),
                border_radius: BorderRadius::all(Val::Px(RADIUS_PANEL)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(22.0)),
                ..default()
            },
            BackgroundColor(COLOR_PANEL_BG),
            BorderColor::all(COLOR_PANEL_BORDER),
            GlobalZIndex(0),
            DespawnOnExit(MenuScreen::JoinBrowser),
            JoinBrowserRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
                MenuText::new("Join Game", 1.45),
                MenuTextColor(COLOR_TEXT_GOLD),
            ));
            p.spawn((
                Node {
                    width: Val::Px(panel_w - 56.0),
                    height: Val::Px(2.0),
                    margin: UiRect::bottom(Val::Px(14.0)),
                    ..default()
                },
                BackgroundColor(COLOR_HEADER_DIVIDER),
            ));

            // Code input box (left side; Join + Refresh buttons sit beside it)
            p.spawn(Node {
                width: Val::Px(710.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Node {
                        width: Val::Px(436.0),
                        height: Val::Px(38.0),
                        border: UiRect::all(Val::Px(BORDER_W)),
                        border_radius: BorderRadius::all(Val::Px(RADIUS_BTN)),
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(COLOR_INPUT_BG),
                    BorderColor::all(COLOR_INPUT_ACTIVE_BORDER),
                ))
                .with_children(|box_p| {
                    box_p.spawn((
                        Node::default(),
                        MenuText::new(format_code_display(&state.code_input), 0.95),
                        JoinCodeValueText,
                    ));
                });
            });

            if let Some(ref err) = state.error {
                p.spawn((
                    Node {
                        width: Val::Px(710.0),
                        margin: UiRect::bottom(Val::Px(6.0)),
                        ..default()
                    },
                    MenuText::new(err, 0.85),
                    MenuTextColor(COLOR_TEXT_ERROR),
                ));
            } else if let Some(ref status) = state.status {
                p.spawn((
                    Node {
                        width: Val::Px(710.0),
                        margin: UiRect::bottom(Val::Px(6.0)),
                        ..default()
                    },
                    MenuText::new(status, 0.85),
                    MenuTextColor(COLOR_TEXT_MUTED),
                ));
            }
        });

    // Top row buttons: Join (by code) and Refresh
    let top_btn_y = panel_top + 88.0;
    let b_join_code = spawn_button_sized(
        commands,
        "Join",
        Action::JoinByCode,
        0,
        (center_x + 92.0, top_btn_y),
        (116.0, 38.0),
        modal_open,
    );
    commands.entity(b_join_code).insert((
        DespawnOnExit(MenuScreen::JoinBrowser),
        JoinBrowserRoot,
        GlobalZIndex(1),
    ));

    let b_refresh = spawn_button_sized(
        commands,
        "Refresh",
        Action::RefreshServers,
        1,
        (center_x + 218.0, top_btn_y),
        (132.0, 38.0),
        modal_open,
    );
    commands.entity(b_refresh).insert((
        DespawnOnExit(MenuScreen::JoinBrowser),
        JoinBrowserRoot,
        GlobalZIndex(1),
    ));

    let mut current_rank = 2u16;

    if state.rooms.is_empty() {
        let msg = if state.loading {
            "Searching for LAN and public games..."
        } else {
            "No public or LAN games found. Enter a code above or click Refresh."
        };
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(center_x - 320.0),
                top: Val::Px(panel_top + 220.0),
                width: Val::Px(640.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            MenuText::new(msg, 0.95),
            MenuTextColor(COLOR_TEXT_MUTED),
            DespawnOnExit(MenuScreen::JoinBrowser),
            JoinBrowserRoot,
            GlobalZIndex(1),
        ));
    } else {
        let start_y = panel_top + 168.0;
        let card_w = 720.0;
        for (idx, room) in state.rooms.iter().take(5).enumerate() {
            let row_y = start_y + (idx as f32) * 72.0;
            commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(center_x - card_w * 0.5),
                        top: Val::Px(row_y),
                        width: Val::Px(card_w),
                        height: Val::Px(64.0),
                        border: UiRect::all(Val::Px(BORDER_W)),
                        border_radius: BorderRadius::all(Val::Px(RADIUS_PANEL)),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::horizontal(Val::Px(16.0)),
                        ..default()
                    },
                    BackgroundColor(COLOR_PANEL_INNER),
                    BorderColor::all(COLOR_PANEL_BORDER),
                    DespawnOnExit(MenuScreen::JoinBrowser),
                    JoinBrowserRoot,
                    JoinRoomRow,
                    GlobalZIndex(1),
                ))
                .with_children(|p| {
                    p.spawn((
                        Node {
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        },
                        MenuText::new(format_room_title(room), 1.0),
                    ));
                    p.spawn((
                        Node::default(),
                        MenuText::new(
                            format!(
                                "Host: {}  |  Players: {}/{}",
                                room.host_name, room.players, room.max_players
                            ),
                            0.82,
                        ),
                        MenuTextColor(COLOR_TEXT_MUTED),
                    ));
                });

            let b_join = spawn_button_sized(
                commands,
                "Join",
                Action::JoinRoom(idx),
                current_rank,
                (center_x + 248.0, row_y + 13.0),
                (96.0, 38.0),
                modal_open,
            );
            commands.entity(b_join).insert((
                DespawnOnExit(MenuScreen::JoinBrowser),
                JoinBrowserRoot,
                GlobalZIndex(2),
            ));
            current_rank += 1;
        }
    }

    // Footer Back button
    let footer_y = panel_top + panel_h - 66.0;
    let foot_w = 220.0;
    let b_back = spawn_button_sized(
        commands,
        "Back",
        Action::Back,
        current_rank,
        (center_x - foot_w * 0.5, footer_y),
        (foot_w, BUTTON_HEIGHT),
        modal_open,
    );
    commands.entity(b_back).insert((
        DespawnOnExit(MenuScreen::JoinBrowser),
        JoinBrowserRoot,
        GlobalZIndex(1),
    ));

    if !modal_open && focus.rank.is_none() {
        focus.rank = Some(0);
    }

    // Password prompt modal
    if let Some(ref pending) = state.password_modal {
        let modal_top = (win_h - 240.0) * 0.5;
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(center_x - 260.0),
                    top: Val::Px(modal_top),
                    width: Val::Px(520.0),
                    height: Val::Px(240.0),
                    border: UiRect::all(Val::Px(BORDER_W)),
                    border_radius: BorderRadius::all(Val::Px(RADIUS_PANEL)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(22.0)),
                    ..default()
                },
                BackgroundColor(COLOR_PANEL_BG),
                BorderColor::all(COLOR_INPUT_ACTIVE_BORDER),
                GlobalZIndex(100),
                DespawnOnExit(MenuScreen::JoinBrowser),
                JoinBrowserRoot,
            ))
            .with_children(|p| {
                p.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                    MenuText::new("Room Password", 1.3),
                    MenuTextColor(COLOR_TEXT_GOLD),
                ));
                p.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(12.0)),
                        ..default()
                    },
                    MenuText::new(
                        format!("Enter password for \"{}\":", pending.world_name),
                        0.9,
                    ),
                ));
                p.spawn((
                    Node {
                        width: Val::Px(420.0),
                        height: Val::Px(38.0),
                        border: UiRect::all(Val::Px(BORDER_W)),
                        border_radius: BorderRadius::all(Val::Px(RADIUS_BTN)),
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(COLOR_INPUT_BG),
                    BorderColor::all(COLOR_INPUT_ACTIVE_BORDER),
                ))
                .with_children(|box_p| {
                    box_p.spawn((
                        Node::default(),
                        MenuText::new(format_modal_password(&state.password_input), 1.0),
                        JoinPasswordValueText,
                    ));
                });
            });

        let modal_btn_w = 180.0;
        let b_confirm = spawn_button_sized(
            commands,
            "Join",
            Action::ConfirmRoomPassword,
            1000,
            (center_x - modal_btn_w - 14.0, modal_top + 166.0),
            (modal_btn_w, BUTTON_HEIGHT),
            false,
        );
        commands.entity(b_confirm).insert((
            DespawnOnExit(MenuScreen::JoinBrowser),
            JoinBrowserRoot,
            GlobalZIndex(101),
        ));

        let b_cancel = spawn_button_sized(
            commands,
            "Cancel",
            Action::CancelRoomPassword,
            1001,
            (center_x + 14.0, modal_top + 166.0),
            (modal_btn_w, BUTTON_HEIGHT),
            false,
        );
        commands.entity(b_cancel).insert((
            DespawnOnExit(MenuScreen::JoinBrowser),
            JoinBrowserRoot,
            GlobalZIndex(101),
        ));

        focus.rank = Some(1000);
    }
}

pub fn setup_join_browser(
    mut commands: Commands,
    mut state: ResMut<JoinBrowserState>,
    mut focus: ResMut<MenuFocus>,
    mut rebuild: ResMut<JoinBrowserRebuild>,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
) {
    *state = JoinBrowserState::default();
    state.loading = true;
    state.refresh_rx = Some(Mutex::new(spawn_refresh_worker()));
    focus.rank = Some(0);
    rebuild.0 = false;

    rebuild_join_browser_ui(
        &mut commands,
        &state,
        &mut focus,
        viewport_size(primary_window.as_ref()),
    );
}

pub fn handle_join_browser_input(
    keyboard_events: Option<MessageReader<KeyboardInput>>,
    mut state: ResMut<JoinBrowserState>,
) {
    let Some(mut keyboard_events) = keyboard_events else {
        return;
    };
    let modal_open = state.password_modal.is_some();

    for ev in keyboard_events.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        if ev.key_code == KeyCode::Backspace || ev.logical_key == Key::Backspace {
            if modal_open {
                state.password_input.pop();
            } else {
                state.code_input.pop();
                state.error = None;
            }
            continue;
        }
        if ev.logical_key == Key::Space || ev.key_code == KeyCode::Space {
            if modal_open && state.password_input.chars().count() < MAX_PASSWORD_CHARS {
                state.password_input.push(' ');
            }
            continue;
        }
        if let Key::Character(ref s) = ev.logical_key {
            for ch in s.chars() {
                if !ch.is_ascii_graphic() {
                    continue;
                }
                if modal_open {
                    if state.password_input.chars().count() < MAX_PASSWORD_CHARS {
                        state.password_input.push(ch);
                    }
                } else if state.code_input.chars().count() < MAX_CODE_INPUT_CHARS {
                    state.code_input.push(ch);
                    state.error = None;
                }
            }
        }
    }
}

pub fn poll_join_browser_workers(
    mut state: ResMut<JoinBrowserState>,
    mut rebuild: ResMut<JoinBrowserRebuild>,
    mut session_mode: Option<ResMut<SessionMode>>,
    mut next_app_state: Option<ResMut<NextState<AppState>>>,
) {
    let refresh_done = state
        .refresh_rx
        .as_ref()
        .and_then(|rx| rx.lock().ok().and_then(|g| g.try_recv().ok()));
    if let Some(res) = refresh_done {
        state.refresh_rx = None;
        state.loading = false;
        state.rooms = res.rooms;
        state.status = res.status;
        rebuild.0 = true;
    }

    let resolve_done = state
        .resolve_rx
        .as_ref()
        .and_then(|rx| rx.lock().ok().and_then(|g| g.try_recv().ok()));
    if let Some(outcome) = resolve_done {
        state.resolve_rx = None;
        match outcome {
            ResolveOutcome::Ready { ticket, password } => {
                if let Some(ref mut mode) = session_mode {
                    **mode = SessionMode::Join { ticket, password };
                }
                if let Some(ref mut next) = next_app_state {
                    next.set(AppState::InGame);
                }
            }
            ResolveOutcome::NeedsPassword { world_name, ticket } => {
                state.password_input.clear();
                state.password_modal = Some(PendingPasswordJoin {
                    world_name,
                    source: RoomSource::Lan { ticket },
                });
                rebuild.0 = true;
            }
            ResolveOutcome::Error(msg) => {
                state.error = Some(msg);
                rebuild.0 = true;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_join_browser_actions(
    mut commands: Commands,
    actions: Option<MessageReader<Action>>,
    mut state: ResMut<JoinBrowserState>,
    mut session_mode: Option<ResMut<SessionMode>>,
    mut next_app_state: Option<ResMut<NextState<AppState>>>,
    mut next_screen: Option<ResMut<NextState<MenuScreen>>>,
    mut focus: ResMut<MenuFocus>,
    mut rebuild: ResMut<JoinBrowserRebuild>,
    existing_roots: Query<Entity, With<JoinBrowserRoot>>,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
) {
    if let Some(mut actions) = actions {
        for action in actions.read() {
            match action {
                Action::RefreshServers => {
                    state.loading = true;
                    state.error = None;
                    state.refresh_rx = Some(Mutex::new(spawn_refresh_worker()));
                    rebuild.0 = true;
                }
                Action::JoinByCode => {
                    let trimmed = state.code_input.trim().to_string();
                    if trimmed.is_empty() {
                        state.error =
                            Some("Enter a 6-character join code or host ticket.".into());
                        rebuild.0 = true;
                    } else if is_lobby_code(&trimmed) {
                        state.error = None;
                        state.status = Some(format!("Resolving code {}...", trimmed.to_ascii_uppercase()));
                        state.resolve_rx = Some(Mutex::new(spawn_resolve_worker(
                            trimmed.to_ascii_uppercase(),
                            None,
                        )));
                        rebuild.0 = true;
                    } else {
                        state.error = None;
                        if let Some(ref mut mode) = session_mode {
                            **mode = SessionMode::Join {
                                ticket: trimmed,
                                password: None,
                            };
                        }
                        if let Some(ref mut next) = next_app_state {
                            next.set(AppState::InGame);
                        }
                    }
                }
                Action::JoinRoom(idx) => {
                    if let Some(room) = state.rooms.get(*idx).cloned() {
                        if room.has_password {
                            state.password_input.clear();
                            state.password_modal = Some(PendingPasswordJoin {
                                world_name: room.world_name,
                                source: room.source,
                            });
                            rebuild.0 = true;
                        } else {
                            match room.source {
                                RoomSource::Lan { ticket } => {
                                    if let Some(ref mut mode) = session_mode {
                                        **mode = SessionMode::Join {
                                            ticket,
                                            password: None,
                                        };
                                    }
                                    if let Some(ref mut next) = next_app_state {
                                        next.set(AppState::InGame);
                                    }
                                }
                                RoomSource::Public { code } => {
                                    state.error = None;
                                    state.status = Some(format!("Joining {code}..."));
                                    state.resolve_rx =
                                        Some(Mutex::new(spawn_resolve_worker(code, None)));
                                    rebuild.0 = true;
                                }
                            }
                        }
                    }
                }
                Action::ConfirmRoomPassword => {
                    if let Some(pending) = state.password_modal.take() {
                        let pw = if state.password_input.is_empty() {
                            None
                        } else {
                            Some(state.password_input.clone())
                        };
                        state.password_input.clear();
                        match pending.source {
                            RoomSource::Lan { ticket } => {
                                if let Some(ref mut mode) = session_mode {
                                    **mode = SessionMode::Join {
                                        ticket,
                                        password: pw,
                                    };
                                }
                                if let Some(ref mut next) = next_app_state {
                                    next.set(AppState::InGame);
                                }
                            }
                            RoomSource::Public { code } => {
                                state.error = None;
                                state.status = Some(format!("Joining {code}..."));
                                state.resolve_rx =
                                    Some(Mutex::new(spawn_resolve_worker(code, pw)));
                                rebuild.0 = true;
                            }
                        }
                    }
                }
                Action::CancelRoomPassword => {
                    state.password_modal = None;
                    state.password_input.clear();
                    focus.rank = Some(0);
                    rebuild.0 = true;
                }
                Action::Back => {
                    if let Some(ref mut next) = next_screen {
                        next.set(MenuScreen::Characters);
                    }
                }
                _ => {}
            }
        }
    }

    if rebuild.0 {
        rebuild.0 = false;
        for e in &existing_roots {
            commands.entity(e).despawn();
        }
        rebuild_join_browser_ui(
            &mut commands,
            &state,
            &mut focus,
            viewport_size(primary_window.as_ref()),
        );
    }
}

pub fn sync_join_browser_text(
    state: Res<JoinBrowserState>,
    mut code_q: Query<&mut MenuText, (With<JoinCodeValueText>, Without<JoinPasswordValueText>)>,
    mut pw_q: Query<&mut MenuText, (With<JoinPasswordValueText>, Without<JoinCodeValueText>)>,
) {
    if !state.is_changed() {
        return;
    }
    let code_disp = format_code_display(&state.code_input);
    for mut t in &mut code_q {
        t.text = code_disp.clone();
    }
    let pw_disp = format_modal_password(&state.password_input);
    for mut t in &mut pw_q {
        t.text = pw_disp.clone();
    }
}

pub struct JoinBrowserPlugin;

impl Plugin for JoinBrowserPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<JoinBrowserState>()
            .init_resource::<JoinBrowserRebuild>()
            .add_message::<Action>()
            .add_systems(OnEnter(MenuScreen::JoinBrowser), setup_join_browser)
            .add_systems(
                Update,
                (
                    handle_join_browser_input,
                    poll_join_browser_workers,
                    handle_join_browser_actions,
                    sync_join_browser_text,
                )
                    .chain()
                    .run_if(in_state(MenuScreen::JoinBrowser)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::MenuButton;

    #[test]
    fn join_browser_renders_code_input_refresh_and_room_list() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .init_resource::<SessionMode>()
            .add_plugins(JoinBrowserPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::JoinBrowser);
        app.update();
        app.update();

        // Populate deterministic LAN + public rooms (and cancel background refresh)
        {
            let mut st = app.world_mut().resource_mut::<JoinBrowserState>();
            st.refresh_rx = None;
            st.loading = false;
            st.rooms = vec![
                BrowserRoom {
                    world_name: "Oakvale".into(),
                    host_name: "Alice".into(),
                    players: 2,
                    max_players: 8,
                    has_password: false,
                    source: RoomSource::Lan {
                        ticket: "tv1:lan-ticket".into(),
                    },
                },
                BrowserRoom {
                    world_name: "Ironhold".into(),
                    host_name: "Bob".into(),
                    players: 3,
                    max_players: 4,
                    has_password: true,
                    source: RoomSource::Public {
                        code: "TV4K9P".into(),
                    },
                },
            ];
        }
        app.world_mut().resource_mut::<JoinBrowserRebuild>().0 = true;
        app.update();

        // Verify code input text node, Join, Refresh, Back, and room Join buttons
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<JoinCodeValueText>>()
                .iter(app.world())
                .count(),
            1,
            "code input text must exist"
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<JoinRoomRow>>()
                .iter(app.world())
                .count(),
            2,
            "two room rows must be rendered"
        );

        let buttons: Vec<MenuButton> = app
            .world_mut()
            .query::<&MenuButton>()
            .iter(app.world())
            .cloned()
            .collect();
        assert!(buttons.iter().any(|b| b.action == Action::JoinByCode));
        assert!(buttons.iter().any(|b| b.action == Action::RefreshServers));
        assert!(buttons.iter().any(|b| b.action == Action::JoinRoom(0)));
        assert!(buttons.iter().any(|b| b.action == Action::JoinRoom(1)));
        assert!(buttons.iter().any(|b| b.action == Action::Back));

        // Verify [LAN] and [LOCK] badges appear in room text
        let all_texts: Vec<String> = app
            .world_mut()
            .query::<&MenuText>()
            .iter(app.world())
            .map(|t| t.text.clone())
            .collect();
        assert!(
            all_texts.iter().any(|t| t.contains("[LAN] Oakvale")),
            "LAN room must show [LAN] prefix; got {all_texts:?}"
        );
        assert!(
            all_texts
                .iter()
                .any(|t| t.contains("Ironhold") && t.contains("[LOCK]")),
            "Locked room must show [LOCK]; got {all_texts:?}"
        );

        // Clicking a password-protected room opens the password modal
        app.world_mut().write_message(Action::JoinRoom(1));
        app.update();
        assert!(
            app.world()
                .resource::<JoinBrowserState>()
                .password_modal
                .is_some(),
            "password modal must open for locked room"
        );

        // Cancel closes the modal
        app.world_mut().write_message(Action::CancelRoomPassword);
        app.update();
        assert!(
            app.world()
                .resource::<JoinBrowserState>()
                .password_modal
                .is_none()
        );

        // Clicking an unlocked LAN room sets SessionMode::Join and transitions to InGame
        app.world_mut().write_message(Action::JoinRoom(0));
        app.update();
        app.update();
        assert_eq!(
            *app.world().resource::<SessionMode>(),
            SessionMode::Join {
                ticket: "tv1:lan-ticket".into(),
                password: None,
            }
        );
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );
    }
}
