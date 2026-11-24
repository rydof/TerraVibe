//! World list, play, and delete confirmation screen.

use std::cmp::Reverse;
use std::collections::HashSet;
use std::path::Path;
use std::time::SystemTime;

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use terravibe_server::save::{has_world_backup, restore_world_backup};
use terravibe_server::worlds::{WorldInfo, delete_world, list_worlds};

use super::style::{
    BORDER_W, BUTTON_HEIGHT, BUTTON_WIDTH, COLOR_BACKDROP, COLOR_HEADER_DIVIDER, COLOR_PANEL_BG,
    COLOR_PANEL_BORDER, COLOR_PANEL_INNER, COLOR_TEXT_ERROR, COLOR_TEXT_GOLD, COLOR_TEXT_MUTED,
    RADIUS_PANEL, spawn_button_ext, spawn_button_sized, viewport_size,
};
use super::text::{MenuText, MenuTextColor};
use super::{Action, MenuFlow, MenuFocus, Screen};
use crate::session::{AppState, MenuScreen, SaveRoot, SelectedWorld, SessionMode};

/// Catalogue rows currently known to the client.
#[derive(Resource, Default, Debug)]
pub struct WorldsList {
    pub rows: Vec<WorldInfo>,
    pub backups: HashSet<String>,
    pub error: Option<String>,
}

fn refresh_worlds_list(root: &Path, worlds_list: &mut WorldsList) {
    if let Ok(rows) = list_worlds(root) {
        worlds_list.backups = rows
            .iter()
            .filter(|w| w.damaged && has_world_backup(root, &w.name))
            .map(|w| w.name.clone())
            .collect();
        worlds_list.rows = rows;
    }
}

/// State of the delete confirmation modal.
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct DeleteModal {
    pub pending_world: Option<String>,
    pub error: Option<String>,
}

/// Description of a row to be rendered in the world list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowSpec {
    pub name: String,
    pub seed: u32,
    pub clock_tick: u32,
    pub playable: bool,
    pub delete: bool,
    pub damaged: bool,
    pub action: Option<Action>,
}

/// Pure helper turning catalogue `WorldInfo` into UI row specifications.
///
/// In an empty catalogue, produces a single call-to-action row directing the player
/// to Create World. Otherwise, partitions healthy (sorted by `last_played` descending)
/// before damaged (sorted by name ascending).
pub fn rows_to_build(info: &[WorldInfo]) -> Vec<RowSpec> {
    if info.is_empty() {
        return vec![RowSpec {
            name: "No worlds found".to_string(),
            seed: 0,
            clock_tick: 0,
            playable: false,
            delete: false,
            damaged: false,
            action: Some(Action::OpenScreen(Screen::CreateWorld)),
        }];
    }

    let mut healthy: Vec<&WorldInfo> = info.iter().filter(|w| !w.damaged).collect();
    let mut damaged: Vec<&WorldInfo> = info.iter().filter(|w| w.damaged).collect();

    healthy.sort_by_key(|r| Reverse(r.last_played));
    damaged.sort_by(|a, b| a.name.cmp(&b.name));

    let mut specs = Vec::with_capacity(info.len());
    for w in healthy {
        specs.push(RowSpec {
            name: w.name.clone(),
            seed: w.seed,
            clock_tick: w.clock_tick,
            playable: true,
            delete: true,
            damaged: false,
            action: Some(Action::Play(w.name.clone())),
        });
    }
    for w in damaged {
        specs.push(RowSpec {
            name: w.name.clone(),
            seed: 0,
            clock_tick: 0,
            playable: false,
            delete: true,
            damaged: true,
            action: Some(Action::Delete(w.name.clone())),
        });
    }

    specs
}

/// Marker for all UI entities belonging to the worlds screen.
#[derive(Component)]
pub struct WorldsScreenRoot;

/// Internal trigger to rebuild the worlds UI without polling.
#[derive(Resource, Default)]
pub struct WorldsRebuildRequested(pub bool);

fn format_last_played(time: SystemTime) -> String {
    let now = SystemTime::now();
    match now.duration_since(time) {
        Ok(d) => {
            let secs = d.as_secs();
            if secs < 60 {
                "just now".to_string()
            } else if secs < 3600 {
                format!("{}m ago", secs / 60)
            } else if secs < 86400 {
                format!("{}h ago", secs / 3600)
            } else {
                format!("{}d ago", secs / 86400)
            }
        }
        Err(_) => "recently".to_string(),
    }
}

pub fn rebuild_worlds_ui(
    commands: &mut Commands,
    worlds_list: &WorldsList,
    delete_modal: &DeleteModal,
    focus: &mut MenuFocus,
    selected_char: Option<&str>,
    viewport: (f32, f32),
) {
    let (win_w, win_h) = viewport;
    let center_x = win_w * 0.5;
    let panel_w = 780.0;
    let panel_h = 620.0;
    let panel_top = ((win_h - panel_h) * 0.5).max(24.0);
    let specs = rows_to_build(&worlds_list.rows);

    let modal_open = delete_modal.pending_world.is_some();

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
        DespawnOnExit(MenuScreen::Worlds),
        WorldsScreenRoot,
    ));

    // Main worlds panel
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
            DespawnOnExit(MenuScreen::Worlds),
            WorldsScreenRoot,
        ))
        .with_children(|p| {
            // Screen title
            p.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                },
                MenuText::new("Select World", 1.45),
                MenuTextColor(COLOR_TEXT_GOLD),
            ));
            if let Some(cname) = selected_char {
                p.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                    MenuText::new(&format!("Character: {cname}"), 0.95),
                    MenuTextColor(COLOR_TEXT_MUTED),
                ));
            }
            p.spawn((
                Node {
                    width: Val::Px(panel_w - 56.0),
                    height: Val::Px(2.0),
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(COLOR_HEADER_DIVIDER),
            ));
            if let Some(ref err) = worlds_list.error {
                p.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                    MenuText::new(err, 0.9),
                    MenuTextColor(COLOR_TEXT_ERROR),
                ));
            }
        });

    let mut current_rank = 0;

    if worlds_list.rows.is_empty() {
        // Empty state explanation text
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(center_x - 300.0),
                top: Val::Px(panel_top + 160.0),
                width: Val::Px(600.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            MenuText::new("No worlds found. Create one to begin playing!", 1.0),
            DespawnOnExit(MenuScreen::Worlds),
            WorldsScreenRoot,
        ));

        // Empty state: CTA button
        let cta_x = center_x - BUTTON_WIDTH / 2.0;
        let b_cta = spawn_button_ext(
            commands,
            "Create World",
            Action::OpenScreen(Screen::CreateWorld),
            0,
            (cta_x, panel_top + 230.0),
            modal_open,
        );
        commands
            .entity(b_cta)
            .insert((DespawnOnExit(MenuScreen::Worlds), WorldsScreenRoot));

        current_rank = 1;
        if !modal_open {
            focus.rank = Some(0);
        }
    } else {
        // Render up to 5 world rows
        let start_y = panel_top + 116.0;
        let card_w = 720.0;
        for (idx, spec) in specs.iter().take(5).enumerate() {
            let row_y = start_y + (idx as f32) * 76.0;

            commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(center_x - card_w * 0.5),
                        top: Val::Px(row_y),
                        width: Val::Px(card_w),
                        height: Val::Px(68.0),
                        border: UiRect::all(Val::Px(BORDER_W)),
                        border_radius: BorderRadius::all(Val::Px(RADIUS_PANEL)),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::horizontal(Val::Px(16.0)),
                        ..default()
                    },
                    BackgroundColor(COLOR_PANEL_INNER),
                    BorderColor::all(COLOR_PANEL_BORDER),
                    DespawnOnExit(MenuScreen::Worlds),
                    WorldsScreenRoot,
                ))
                .with_children(|p| {
                    let has_backup = spec.damaged && worlds_list.backups.contains(&spec.name);
                    let label = if spec.damaged {
                        if has_backup {
                            format!("{}  [DAMAGED - Backup available]", spec.name)
                        } else {
                            format!("{}  [DAMAGED]", spec.name)
                        }
                    } else {
                        format!("{}  (Seed: {})", spec.name, spec.seed)
                    };
                    p.spawn((
                        Node {
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        },
                        MenuText::new(&label, 1.0),
                    ));

                    if let Some(w) = worlds_list.rows.iter().find(|r| r.name == spec.name) {
                        p.spawn((
                            Node::default(),
                            MenuText::new(
                                &format!(
                                    "{:?}  |  Played: {}",
                                    w.size,
                                    format_last_played(w.last_played)
                                ),
                                0.8,
                            ),
                        ));
                    }
                });

            // Action buttons on the right side of the row
            let btn_w = 90.0;
            let btn_h = 38.0;
            if spec.playable {
                let b_play = spawn_button_sized(
                    commands,
                    "Play",
                    Action::Play(spec.name.clone()),
                    current_rank,
                    (center_x + 140.0, row_y + 15.0),
                    (btn_w, btn_h),
                    modal_open,
                );
                commands
                    .entity(b_play)
                    .insert((DespawnOnExit(MenuScreen::Worlds), WorldsScreenRoot));
                current_rank += 1;
            } else if spec.damaged && worlds_list.backups.contains(&spec.name) {
                let b_bak = spawn_button_sized(
                    commands,
                    "Load backup",
                    Action::LoadBackup(spec.name.clone()),
                    current_rank,
                    (center_x + 115.0, row_y + 15.0),
                    (115.0, btn_h),
                    modal_open,
                );
                commands
                    .entity(b_bak)
                    .insert((DespawnOnExit(MenuScreen::Worlds), WorldsScreenRoot));
                current_rank += 1;
            }

            let b_del = spawn_button_sized(
                commands,
                "Delete",
                Action::Delete(spec.name.clone()),
                current_rank,
                (center_x + 245.0, row_y + 15.0),
                (btn_w, btn_h),
                modal_open,
            );
            commands
                .entity(b_del)
                .insert((DespawnOnExit(MenuScreen::Worlds), WorldsScreenRoot));
            current_rank += 1;
        }

        if !modal_open && focus.rank.is_none() {
            focus.rank = Some(0);
        }
    }

    // Bottom navigation buttons
    let footer_y = panel_top + panel_h - 66.0;

    if !worlds_list.rows.is_empty() {
        let foot_w = 220.0;
        let b_create = spawn_button_sized(
            commands,
            "Create World",
            Action::OpenScreen(Screen::CreateWorld),
            current_rank,
            (center_x - foot_w - 16.0, footer_y),
            (foot_w, BUTTON_HEIGHT),
            modal_open,
        );
        commands
            .entity(b_create)
            .insert((DespawnOnExit(MenuScreen::Worlds), WorldsScreenRoot));
        current_rank += 1;

        let b_back = spawn_button_sized(
            commands,
            "Back",
            Action::Back,
            current_rank,
            (center_x + 16.0, footer_y),
            (foot_w, BUTTON_HEIGHT),
            modal_open,
        );
        commands
            .entity(b_back)
            .insert((DespawnOnExit(MenuScreen::Worlds), WorldsScreenRoot));
    } else {
        let btn_x = center_x - BUTTON_WIDTH / 2.0;
        let b_back = spawn_button_ext(
            commands,
            "Back",
            Action::Back,
            current_rank,
            (btn_x, footer_y),
            modal_open,
        );
        commands
            .entity(b_back)
            .insert((DespawnOnExit(MenuScreen::Worlds), WorldsScreenRoot));
    }

    // Delete confirmation modal overlay
    if let Some(ref pending) = delete_modal.pending_world {
        let modal_top = (win_h - 240.0) * 0.5;
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(center_x - 260.0),
                    top: Val::Px(modal_top),
                    width: Val::Px(520.0),
                    border: UiRect::all(Val::Px(BORDER_W)),
                    border_radius: BorderRadius::all(Val::Px(RADIUS_PANEL)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(24.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.14, 0.06, 0.08)),
                BorderColor::all(Color::srgb(0.82, 0.25, 0.28)),
                GlobalZIndex(100),
                DespawnOnExit(MenuScreen::Worlds),
                WorldsScreenRoot,
            ))
            .with_children(|p| {
                p.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(12.0)),
                        ..default()
                    },
                    MenuText::new("Delete World?", 1.35),
                ));

                p.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(8.0)),
                        ..default()
                    },
                    MenuText::new(&format!("Delete \"{pending}\" forever?"), 1.0),
                ));

                p.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(16.0)),
                        ..default()
                    },
                    MenuText::new("This cannot be undone.", 0.85),
                ));

                if let Some(ref err) = delete_modal.error {
                    p.spawn((
                        Node {
                            margin: UiRect::bottom(Val::Px(12.0)),
                            ..default()
                        },
                        MenuText::new(err, 0.85),
                    ));
                }
            });

        // Cancel (rank 1000) and ConfirmDelete (rank 1001)
        let modal_btn_w = 180.0;
        let b_cancel = spawn_button_sized(
            commands,
            "Cancel",
            Action::CancelDelete,
            1000,
            (center_x - modal_btn_w - 14.0, modal_top + 160.0),
            (modal_btn_w, BUTTON_HEIGHT),
            false,
        );
        commands
            .entity(b_cancel)
            .insert((DespawnOnExit(MenuScreen::Worlds), WorldsScreenRoot));

        let b_confirm = spawn_button_sized(
            commands,
            "Delete forever",
            Action::ConfirmDelete,
            1001,
            (center_x + 14.0, modal_top + 160.0),
            (modal_btn_w, BUTTON_HEIGHT),
            false,
        );
        commands
            .entity(b_confirm)
            .insert((DespawnOnExit(MenuScreen::Worlds), WorldsScreenRoot));

        // Default focus is explicitly on Cancel
        focus.rank = Some(1000);
    }
}

pub fn setup_worlds_screen(
    mut commands: Commands,
    save_root: Option<Res<SaveRoot>>,
    selected_char: Option<Res<crate::session::SelectedCharacter>>,
    mut worlds_list: ResMut<WorldsList>,
    mut delete_modal: ResMut<DeleteModal>,
    mut focus: ResMut<MenuFocus>,
    mut rebuild: ResMut<WorldsRebuildRequested>,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
) {
    *delete_modal = DeleteModal::default();
    worlds_list.error = None;
    focus.rank = None;
    rebuild.0 = false;

    if let Some(ref root) = save_root {
        refresh_worlds_list(&root.0, &mut worlds_list);
    }

    let char_name = selected_char
        .as_ref()
        .and_then(|sc| sc.0.as_ref().map(|c| c.name.as_str()));
    rebuild_worlds_ui(
        &mut commands,
        &worlds_list,
        &delete_modal,
        &mut focus,
        char_name,
        viewport_size(primary_window.as_ref()),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn handle_worlds_actions(
    mut commands: Commands,
    current_screen: Option<Res<State<MenuScreen>>>,
    actions: Option<MessageReader<Action>>,
    save_root: Option<Res<SaveRoot>>,
    selected_char: Option<Res<crate::session::SelectedCharacter>>,
    menu_flow: Option<Res<MenuFlow>>,
    mut session_mode: Option<ResMut<SessionMode>>,
    mut worlds_list: ResMut<WorldsList>,
    mut delete_modal: ResMut<DeleteModal>,
    mut selected_world: Option<ResMut<SelectedWorld>>,
    mut next_app_state: Option<ResMut<NextState<AppState>>>,
    mut next_menu_screen: Option<ResMut<NextState<MenuScreen>>>,
    mut focus: ResMut<MenuFocus>,
    mut rebuild: ResMut<WorldsRebuildRequested>,
    existing_roots: Query<Entity, With<WorldsScreenRoot>>,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
) {
    if current_screen.as_ref().map(|s| s.get()) != Some(&MenuScreen::Worlds) {
        return;
    }

    let Some(mut actions) = actions else {
        return;
    };

    let flow = menu_flow.as_deref().copied().unwrap_or_default();
    let mut should_rebuild = false;

    for action in actions.read() {
        match action {
            Action::Play(name) => {
                let found = worlds_list
                    .rows
                    .iter()
                    .find(|r| r.name == *name)
                    .map(|r| (r.name.clone(), r.seed, r.size, r.damaged));
                if let Some((row_name, seed, size, damaged)) = found {
                    if !damaged {
                        worlds_list.error = None;
                        if let Some(ref mut sel) = selected_world {
                            sel.name = row_name;
                            sel.seed = seed;
                            sel.size = size;
                            sel.persist = true;
                        }
                        match flow {
                            MenuFlow::Host => {
                                if let Some(ref mut next_screen) = next_menu_screen {
                                    next_screen.set(MenuScreen::HostSetup);
                                }
                            }
                            MenuFlow::SinglePlayer | MenuFlow::Join => {
                                if let Some(ref mut mode) = session_mode {
                                    **mode = SessionMode::SinglePlayer;
                                }
                                if let Some(ref mut next) = next_app_state {
                                    next.set(AppState::InGame);
                                }
                            }
                        }
                    } else {
                        worlds_list.error = Some(if worlds_list.backups.contains(name) {
                            format!(
                                "Primary save for '{name}' is damaged. Use 'Load backup' to recover."
                            )
                        } else {
                            format!("World '{name}' is damaged and has no backup.")
                        });
                        should_rebuild = true;
                    }
                }
            }
            Action::LoadBackup(name) => {
                if let Some(ref root) = save_root {
                    match restore_world_backup(&root.0, name) {
                        Ok(snap) => {
                            worlds_list.error = None;
                            refresh_worlds_list(&root.0, &mut worlds_list);
                            if let Some(ref mut sel) = selected_world {
                                sel.name = name.clone();
                                sel.seed = snap.seed;
                                sel.size = snap.size;
                                sel.persist = true;
                            }
                            match flow {
                                MenuFlow::Host => {
                                    if let Some(ref mut next_screen) = next_menu_screen {
                                        next_screen.set(MenuScreen::HostSetup);
                                    }
                                }
                                MenuFlow::SinglePlayer | MenuFlow::Join => {
                                    if let Some(ref mut mode) = session_mode {
                                        **mode = SessionMode::SinglePlayer;
                                    }
                                    if let Some(ref mut next) = next_app_state {
                                        next.set(AppState::InGame);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            worlds_list.error =
                                Some(format!("Failed to load backup for '{name}': {e}"));
                            should_rebuild = true;
                        }
                    }
                }
            }
            Action::Delete(name) => {
                delete_modal.pending_world = Some(name.clone());
                delete_modal.error = None;
                should_rebuild = true;
            }
            Action::CancelDelete => {
                delete_modal.pending_world = None;
                delete_modal.error = None;
                focus.rank = Some(0);
                should_rebuild = true;
            }
            Action::ConfirmDelete => {
                if let Some(ref name) = delete_modal.pending_world.clone() {
                    let res = if let Some(ref root) = save_root {
                        delete_world(&root.0, name)
                    } else {
                        Ok(())
                    };

                    match res {
                        Ok(()) => {
                            delete_modal.pending_world = None;
                            delete_modal.error = None;
                            if let Some(ref root) = save_root {
                                refresh_worlds_list(&root.0, &mut worlds_list);
                            }
                            focus.rank = Some(0);
                            should_rebuild = true;
                        }
                        Err(e) => {
                            delete_modal.error = Some(format!("Delete failed: {e}"));
                            should_rebuild = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if should_rebuild || rebuild.0 {
        rebuild.0 = false;
        for e in &existing_roots {
            commands.entity(e).despawn();
        }
        let char_name = selected_char
            .as_ref()
            .and_then(|sc| sc.0.as_ref().map(|c| c.name.as_str()));
        rebuild_worlds_ui(
            &mut commands,
            &worlds_list,
            &delete_modal,
            &mut focus,
            char_name,
            viewport_size(primary_window.as_ref()),
        );
    }
}

pub struct WorldsPlugin;

impl Plugin for WorldsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldsList>()
            .init_resource::<DeleteModal>()
            .init_resource::<WorldsRebuildRequested>()
            .add_message::<Action>()
            .add_systems(OnEnter(MenuScreen::Worlds), setup_worlds_screen)
            .add_systems(
                Update,
                handle_worlds_actions.run_if(in_state(MenuScreen::Worlds)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::MenuButton;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{Duration, UNIX_EPOCH};

    fn tmp() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "tv_worlds_test_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn healthy(name: &str, seed: u32) -> WorldInfo {
        WorldInfo {
            name: name.into(),
            seed,
            clock_tick: 13500,
            size: terravibe_sim::WorldSize::Small,
            last_played: UNIX_EPOCH + Duration::from_secs(100),
            damaged: false,
        }
    }

    fn healthy_with_mtime(name: &str, seed: u32, secs: u64) -> WorldInfo {
        WorldInfo {
            name: name.into(),
            seed,
            clock_tick: 13500,
            size: terravibe_sim::WorldSize::Small,
            last_played: UNIX_EPOCH + Duration::from_secs(secs),
            damaged: false,
        }
    }

    fn damaged(name: &str) -> WorldInfo {
        WorldInfo {
            name: name.into(),
            seed: 0,
            clock_tick: 0,
            size: terravibe_sim::WorldSize::Small,
            last_played: UNIX_EPOCH + Duration::from_secs(50),
            damaged: true,
        }
    }

    #[test]
    fn damaged_row_delete_only() {
        let spec = rows_to_build(&[damaged("broken"), healthy("ok", 1)]);
        let broken = spec
            .iter()
            .find(|s| s.name == "broken")
            .expect("broken row exists");
        let ok = spec.iter().find(|s| s.name == "ok").expect("ok row exists");
        assert!(!broken.playable && broken.delete && ok.playable);
    }

    #[test]
    fn empty_list_focuses_create_world() {
        let spec = rows_to_build(&[]);
        assert_eq!(spec.len(), 1, "empty list should produce single CTA row");
        assert!(!spec[0].playable && !spec[0].delete);
        assert_eq!(
            spec[0].action,
            Some(Action::OpenScreen(Screen::CreateWorld))
        );

        let temp = tmp();
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .insert_resource(SaveRoot(temp.clone()))
            .add_plugins(WorldsPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Worlds);
        app.update();
        app.update();

        let focus = app.world().resource::<MenuFocus>();
        assert_eq!(
            focus.rank,
            Some(0),
            "empty worlds list must focus CTA button"
        );
        let has_empty_notice = app
            .world_mut()
            .query::<&MenuText>()
            .iter(app.world())
            .any(|t| t.text.contains("No worlds found"));
        assert!(
            has_empty_notice,
            "empty worlds list must display explanation notice"
        );
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn delete_confirmation_defaults_cancel() {
        let temp = tmp();
        terravibe_server::worlds::create_world(&temp, "ok", 42, terravibe_sim::WorldSize::Small)
            .unwrap();

        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .insert_resource(SaveRoot(temp.clone()))
            .add_plugins(WorldsPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Worlds);
        app.update();
        app.update();

        // Activate Delete("ok")
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::Delete("ok".into()));
        app.update();

        let modal = app.world().resource::<DeleteModal>();
        assert_eq!(modal.pending_world.as_deref(), Some("ok"));

        let focus = app.world().resource::<MenuFocus>();
        assert_eq!(
            focus.rank,
            Some(1000),
            "focus must be set on Cancel (rank 1000)"
        );

        // Activating ConfirmDelete calls delete_world and refreshes WorldsList
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::ConfirmDelete);
        app.update();

        let modal = app.world().resource::<DeleteModal>();
        assert_eq!(modal.pending_world, None, "modal closed after confirm");

        let worlds = app.world().resource::<WorldsList>();
        assert!(
            worlds.rows.is_empty(),
            "WorldsList should be refreshed after delete"
        );
        let disk_worlds = terravibe_server::worlds::list_worlds(&temp).unwrap();
        assert!(
            disk_worlds.is_empty(),
            "World directory should be deleted on disk"
        );

        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn rows_sort_by_last_played_desc() {
        let old = healthy_with_mtime("old", 1, 100);
        let new = healthy_with_mtime("new", 2, 200);
        let spec = rows_to_build(&[old, new]);
        assert_eq!(spec.len(), 2);
        assert_eq!(spec[0].name, "new");
        assert_eq!(spec[1].name, "old");
    }

    #[test]
    fn play_writes_selected_world() {
        let temp = tmp();
        terravibe_server::worlds::create_world(&temp, "ok", 1, terravibe_sim::WorldSize::Small)
            .unwrap();

        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .insert_resource(SaveRoot(temp.clone()))
            .insert_resource(SelectedWorld {
                name: "initial".into(),
                seed: 0,
                size: terravibe_sim::WorldSize::Small,
                persist: false,
            })
            .add_plugins(WorldsPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Worlds);
        app.update();
        app.update();

        // Activate Play("ok")
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::Play("ok".into()));
        app.update();

        let sel = app.world().resource::<SelectedWorld>();
        assert_eq!(sel.name, "ok");
        assert_eq!(sel.seed, 1);
        assert!(sel.persist);

        let next_app = app.world().resource::<NextState<AppState>>();
        assert!(matches!(*next_app, NextState::Pending(AppState::InGame)));

        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn row_action_buttons_do_not_overlap() {
        let temp = tmp();
        terravibe_server::worlds::create_world(&temp, "ok", 1, terravibe_sim::WorldSize::Small)
            .unwrap();

        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .insert_resource(SaveRoot(temp.clone()))
            .add_plugins(WorldsPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Worlds);
        app.update();
        app.update();

        let mut buttons = Vec::new();
        for (btn, node) in app
            .world_mut()
            .query::<(&MenuButton, &Node)>()
            .iter(app.world())
        {
            if matches!(btn.action, Action::Play(_) | Action::Delete(_)) {
                let left = match node.left {
                    Val::Px(p) => p,
                    _ => 0.0,
                };
                let width = match node.width {
                    Val::Px(w) => w,
                    _ => 0.0,
                };
                buttons.push((btn.action.clone(), left, width));
            }
        }

        assert_eq!(
            buttons.len(),
            2,
            "must have Play and Delete buttons for row"
        );
        let play = buttons
            .iter()
            .find(|(a, _, _)| matches!(a, Action::Play(_)))
            .unwrap();
        let del = buttons
            .iter()
            .find(|(a, _, _)| matches!(a, Action::Delete(_)))
            .unwrap();

        assert!(
            play.2 <= 100.0,
            "Play button width must be compact (<= 100), got {}",
            play.2
        );
        assert!(
            del.2 <= 100.0,
            "Delete button width must be compact (<= 100), got {}",
            del.2
        );
        assert!(
            play.1 + play.2 <= del.1,
            "Play button right edge ({}) must be <= Delete button left edge ({})",
            play.1 + play.2,
            del.1
        );
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn damaged_world_with_backup_shows_load_backup_and_recovers() {
        let temp = tmp();
        let dir = terravibe_server::save::SaveDir::open(&temp, "broken").unwrap();
        let snap1 = terravibe_server::save::WorldSnapshot {
            seed: 99,
            clock_tick: 120,
            size: terravibe_sim::WorldSize::Small,
            chunks: vec![],
            players: vec![],
            chests: vec![],
            towns: Default::default(),
            facts: Default::default(),
            weather: Default::default(),
            locked_chests: vec![],
            shadow_chests: vec![],
        };
        dir.save_world(&snap1).unwrap();
        dir.save_world(&snap1).unwrap();
        std::fs::write(dir.path().join("world.bin"), b"corrupt!").unwrap();

        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .insert_resource(SaveRoot(temp.clone()))
            .insert_resource(SelectedWorld {
                name: "initial".into(),
                seed: 0,
                size: terravibe_sim::WorldSize::Small,
                persist: false,
            })
            .add_plugins(WorldsPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Worlds);
        app.update();
        app.update();

        let has_load_backup_btn = app
            .world_mut()
            .query::<&MenuButton>()
            .iter(app.world())
            .any(|b| matches!(&b.action, Action::LoadBackup(n) if n == "broken"));
        assert!(
            has_load_backup_btn,
            "damaged world with .bak must show Load backup button"
        );

        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::LoadBackup("broken".into()));
        app.update();

        let sel = app.world().resource::<SelectedWorld>();
        assert_eq!(sel.name, "broken");
        assert_eq!(sel.seed, 99);
        assert!(matches!(
            *app.world().resource::<NextState<AppState>>(),
            NextState::Pending(AppState::InGame)
        ));
        let _ = std::fs::remove_dir_all(temp);
    }
}
