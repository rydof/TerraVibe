//! Character list, select-and-play, and delete confirmation screen.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use terravibe_server::characters::{
    CharacterInfo, characters_dir, delete_character, list_characters, load_character,
    migrate_world_players,
};

use super::style::{
    BORDER_W, BUTTON_HEIGHT, BUTTON_WIDTH, COLOR_BACKDROP, COLOR_HEADER_DIVIDER, COLOR_PANEL_BG,
    COLOR_PANEL_BORDER, COLOR_PANEL_INNER, COLOR_TEXT_GOLD, RADIUS_BTN, RADIUS_PANEL,
    spawn_button_ext, spawn_button_sized, viewport_size,
};
use super::text::{MenuText, MenuTextColor};
use super::{Action, MenuFlow, MenuFocus, Screen};
use crate::session::{MenuScreen, SaveRoot, SelectedCharacter};

#[derive(Resource, Default, Debug)]
pub struct CharactersList {
    pub rows: Vec<CharacterInfo>,
}

#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct DeleteCharModal {
    pub pending: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharRowSpec {
    pub name: String,
    pub playable: bool,
    pub delete: bool,
    pub action: Option<Action>,
}

pub fn rows_to_build(info: &[CharacterInfo]) -> Vec<CharRowSpec> {
    if info.is_empty() {
        return vec![CharRowSpec {
            name: "No characters found".to_string(),
            playable: false,
            delete: false,
            action: Some(Action::OpenScreen(Screen::CreateCharacter)),
        }];
    }

    info.iter()
        .map(|c| CharRowSpec {
            name: c.name.clone(),
            playable: true,
            delete: true,
            action: Some(Action::PlayCharacter(c.name.clone())),
        })
        .collect()
}

#[derive(Component)]
pub struct CharactersScreenRoot;

#[derive(Resource, Default)]
pub struct CharactersRebuildRequested(pub bool);

pub fn setup_characters(
    mut commands: Commands,
    chars_list: Option<ResMut<CharactersList>>,
    save_root: Option<Res<SaveRoot>>,
    delete_modal: Option<Res<DeleteCharModal>>,
    mut focus: ResMut<MenuFocus>,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
) {
    let root = characters_dir();
    let _ = std::fs::create_dir_all(&root);

    // Run migration from legacy world saves once
    if let Some(sr) = &save_root {
        let _ = migrate_world_players(&sr.0, &root);
    }

    let rows = list_characters(&root).unwrap_or_default();
    if let Some(mut list) = chars_list {
        list.rows = rows.clone();
    } else {
        commands.insert_resource(CharactersList { rows: rows.clone() });
    }

    let modal_pending = delete_modal.as_ref().and_then(|m| m.pending.clone());
    let modal_open = modal_pending.is_some();
    let specs = rows_to_build(&rows);

    let (win_w, win_h) = viewport_size(primary_window.as_ref());
    let center_x = win_w * 0.5;
    let panel_w = 760.0;
    let panel_h = 620.0;
    let panel_top = ((win_h - panel_h) * 0.5).max(24.0);
    let btn_x = center_x - BUTTON_WIDTH / 2.0;

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
        DespawnOnExit(MenuScreen::Characters),
        CharactersScreenRoot,
    ));

    // Main framed panel
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
            DespawnOnExit(MenuScreen::Characters),
            CharactersScreenRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
                MenuText::new("Select Character", 1.45),
                MenuTextColor(COLOR_TEXT_GOLD),
            ));
            p.spawn((
                Node {
                    width: Val::Px(panel_w - 56.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(COLOR_HEADER_DIVIDER),
            ));
        });

    let mut current_rank = 0;
    let is_empty = rows.is_empty();

    if is_empty {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(center_x - 280.0),
                top: Val::Px(panel_top + 160.0),
                width: Val::Px(560.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            MenuText::new("No characters found. Create an adventurer to begin!", 1.0),
            DespawnOnExit(MenuScreen::Characters),
            CharactersScreenRoot,
        ));

        let b_cta = spawn_button_ext(
            &mut commands,
            "Create Character",
            Action::OpenScreen(Screen::CreateCharacter),
            0,
            (btn_x, panel_top + 230.0),
            modal_open,
        );
        commands
            .entity(b_cta)
            .insert((DespawnOnExit(MenuScreen::Characters), CharactersScreenRoot));

        current_rank = 1;
        if !modal_open {
            focus.rank = Some(0);
        }
    } else {
        let start_y = panel_top + 96.0;
        let card_w = 700.0;
        for (idx, spec) in specs.iter().take(5).enumerate() {
            let row_y = start_y + (idx as f32) * 78.0;
            let loaded_app = load_character(&root, &spec.name).ok().map(|c| c.appearance);
            let swatch_color = loaded_app
                .map(|a| {
                    Color::srgb(
                        a.shirt[0] as f32 / 255.0,
                        a.shirt[1] as f32 / 255.0,
                        a.shirt[2] as f32 / 255.0,
                    )
                })
                .unwrap_or(Color::srgb(0.45, 0.55, 0.75));
            let hair_swatch = loaded_app
                .map(|a| {
                    Color::srgb(
                        a.hair_color[0] as f32 / 255.0,
                        a.hair_color[1] as f32 / 255.0,
                        a.hair_color[2] as f32 / 255.0,
                    )
                })
                .unwrap_or(Color::srgb(0.4, 0.25, 0.15));

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
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(14.0)),
                        ..default()
                    },
                    BackgroundColor(COLOR_PANEL_INNER),
                    BorderColor::all(COLOR_PANEL_BORDER),
                    DespawnOnExit(MenuScreen::Characters),
                    CharactersScreenRoot,
                ))
                .with_children(|p| {
                    // Appearance swatch badge
                    p.spawn((
                        Node {
                            width: Val::Px(38.0),
                            height: Val::Px(38.0),
                            border: UiRect::all(Val::Px(BORDER_W)),
                            border_radius: BorderRadius::all(Val::Px(RADIUS_BTN)),
                            margin: UiRect::right(Val::Px(14.0)),
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(swatch_color),
                        BorderColor::all(COLOR_PANEL_BORDER),
                    ))
                    .with_children(|sw| {
                        sw.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(12.0),
                                ..default()
                            },
                            BackgroundColor(hair_swatch),
                        ));
                    });

                    p.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|col| {
                        col.spawn((
                            Node {
                                margin: UiRect::bottom(Val::Px(3.0)),
                                ..default()
                            },
                            MenuText::new(spec.name.clone(), 1.05),
                        ));
                        col.spawn(MenuText::new("Classic (Softcore)", 0.78));
                    });
                });

            if spec.playable {
                let play_btn = spawn_button_sized(
                    &mut commands,
                    "Play",
                    spec.action.clone().unwrap_or(Action::Back),
                    current_rank,
                    (center_x + 130.0, row_y + 15.0),
                    (96.0, 38.0),
                    modal_open,
                );
                commands
                    .entity(play_btn)
                    .insert((DespawnOnExit(MenuScreen::Characters), CharactersScreenRoot));
                current_rank += 1;
            }

            if spec.delete {
                let del_btn = spawn_button_sized(
                    &mut commands,
                    "Delete",
                    Action::DeleteCharacter(spec.name.clone()),
                    current_rank,
                    (center_x + 236.0, row_y + 15.0),
                    (96.0, 38.0),
                    modal_open,
                );
                commands
                    .entity(del_btn)
                    .insert((DespawnOnExit(MenuScreen::Characters), CharactersScreenRoot));
                current_rank += 1;
            }
        }
    }

    // Bottom buttons cleanly inside panel footer
    let footer_y = panel_top + panel_h - 66.0;
    if !is_empty {
        let foot_w = 220.0;
        let b_new = spawn_button_sized(
            &mut commands,
            "New",
            Action::OpenScreen(Screen::CreateCharacter),
            current_rank,
            (center_x - foot_w - 16.0, footer_y),
            (foot_w, BUTTON_HEIGHT),
            modal_open,
        );
        commands
            .entity(b_new)
            .insert((DespawnOnExit(MenuScreen::Characters), CharactersScreenRoot));
        current_rank += 1;

        let b_back = spawn_button_sized(
            &mut commands,
            "Back",
            Action::Back,
            current_rank,
            (center_x + 16.0, footer_y),
            (foot_w, BUTTON_HEIGHT),
            modal_open,
        );
        commands
            .entity(b_back)
            .insert((DespawnOnExit(MenuScreen::Characters), CharactersScreenRoot));
    } else {
        let b_back = spawn_button_ext(
            &mut commands,
            "Back",
            Action::Back,
            current_rank,
            (btn_x, footer_y),
            modal_open,
        );
        commands
            .entity(b_back)
            .insert((DespawnOnExit(MenuScreen::Characters), CharactersScreenRoot));
    }

    // Delete confirmation modal
    if let Some(ref pending) = modal_pending {
        let modal_top = (win_h - 220.0) * 0.5;
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
                DespawnOnExit(MenuScreen::Characters),
                CharactersScreenRoot,
            ))
            .with_children(|p| {
                p.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(12.0)),
                        ..default()
                    },
                    MenuText::new("Delete Character?", 1.3),
                ));
                p.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(20.0)),
                        ..default()
                    },
                    MenuText::new(format!("Delete \"{pending}\"? This cannot be undone."), 0.9),
                ));
            });

        let modal_btn_w = 180.0;
        let confirm_btn = spawn_button_sized(
            &mut commands,
            "Delete",
            Action::ConfirmDeleteCharacter,
            current_rank,
            (center_x - modal_btn_w - 14.0, modal_top + 145.0),
            (modal_btn_w, BUTTON_HEIGHT),
            false,
        );
        commands
            .entity(confirm_btn)
            .insert((CharactersScreenRoot, DespawnOnExit(MenuScreen::Characters)));

        let cancel_btn = spawn_button_sized(
            &mut commands,
            "Cancel",
            Action::CancelDeleteCharacter,
            current_rank + 1,
            (center_x + 14.0, modal_top + 145.0),
            (modal_btn_w, BUTTON_HEIGHT),
            false,
        );
        commands
            .entity(cancel_btn)
            .insert((CharactersScreenRoot, DespawnOnExit(MenuScreen::Characters)));
    }
}

pub fn handle_character_actions(
    mut actions: MessageReader<Action>,
    menu_flow: Option<Res<MenuFlow>>,
    mut next_screen: Option<ResMut<NextState<MenuScreen>>>,
    mut selected_char: Option<ResMut<SelectedCharacter>>,
    mut delete_modal: Option<ResMut<DeleteCharModal>>,
    mut rebuild: Option<ResMut<CharactersRebuildRequested>>,
) {
    let root = characters_dir();
    let flow = menu_flow.as_deref().copied().unwrap_or_default();
    for action in actions.read() {
        match action {
            Action::PlayCharacter(name) => {
                if let Ok(c) = load_character(&root, name) {
                    if let Some(ref mut sc) = selected_char {
                        sc.0 = Some(c);
                    }
                    if let Some(ref mut ns) = next_screen {
                        match flow {
                            MenuFlow::SinglePlayer | MenuFlow::Host => {
                                ns.set(MenuScreen::Worlds);
                            }
                            MenuFlow::Join => {
                                ns.set(MenuScreen::JoinBrowser);
                            }
                        }
                    }
                }
            }
            Action::Back => {
                if flow != MenuFlow::SinglePlayer
                    && let Some(ref mut ns) = next_screen
                {
                    ns.set(MenuScreen::Multiplayer);
                }
            }
            Action::DeleteCharacter(name) => {
                if let Some(ref mut dm) = delete_modal {
                    dm.pending = Some(name.clone());
                    if let Some(ref mut rb) = rebuild {
                        rb.0 = true;
                    }
                }
            }
            Action::ConfirmDeleteCharacter => {
                if let Some(ref mut dm) = delete_modal {
                    if let Some(name) = dm.pending.take() {
                        let _ = delete_character(&root, &name);
                        if let Some(ref mut sc) = selected_char {
                            if sc.0.as_ref().is_some_and(|c| c.name == name) {
                                sc.0 = None;
                            }
                        }
                        if let Some(ref mut rb) = rebuild {
                            rb.0 = true;
                        }
                    }
                }
            }
            Action::CancelDeleteCharacter => {
                if let Some(ref mut dm) = delete_modal {
                    dm.pending = None;
                    if let Some(ref mut rb) = rebuild {
                        rb.0 = true;
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn rebuild_characters_system(
    mut commands: Commands,
    rebuild: Option<ResMut<CharactersRebuildRequested>>,
    screen: Option<Res<State<MenuScreen>>>,
    root: Query<Entity, With<CharactersScreenRoot>>,
    chars_list: Option<ResMut<CharactersList>>,
    save_root: Option<Res<SaveRoot>>,
    delete_modal: Option<Res<DeleteCharModal>>,
    focus: ResMut<MenuFocus>,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
) {
    let Some(mut rb) = rebuild else { return };
    if !rb.0 {
        return;
    }
    rb.0 = false;
    if screen.as_ref().map(|s| s.get()) != Some(&MenuScreen::Characters) {
        return;
    }

    for e in &root {
        commands.entity(e).despawn();
    }
    setup_characters(
        commands,
        chars_list,
        save_root,
        delete_modal,
        focus,
        primary_window,
    );
}

pub struct CharactersPlugin;

impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharactersList>()
            .init_resource::<DeleteCharModal>()
            .init_resource::<CharactersRebuildRequested>()
            .add_systems(OnEnter(MenuScreen::Characters), setup_characters)
            .add_systems(
                Update,
                (handle_character_actions, rebuild_characters_system)
                    .run_if(in_state(MenuScreen::Characters)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::MenuButton;
    use terravibe_sim::character::{Appearance, Character, validate_name};

    #[test]
    fn play_opens_characters_before_worlds() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Image>()
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .add_plugins(crate::menu::title::TitlePlugin);
        app.world_mut()
            .resource_mut::<NextState<crate::session::AppState>>()
            .set(crate::session::AppState::Menu);
        app.update();
        app.update();

        let mut query = app.world_mut().query::<&MenuButton>();
        let sp_btn = query
            .iter(app.world())
            .find(|b| b.rank == 0)
            .expect("rank 0 button must exist");
        assert_eq!(sp_btn.action, Action::OpenScreen(Screen::Characters));
    }

    #[test]
    fn empty_list_prompts_creation() {
        let rows = rows_to_build(&[]);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].playable);
        assert_eq!(
            rows[0].action,
            Some(Action::OpenScreen(Screen::CreateCharacter))
        );
    }

    #[test]
    fn selecting_character_then_world_sends_character_in_hello() {
        let c = Character::new("Adventurer", Appearance::default());
        let sc = SelectedCharacter(Some(c.clone()));
        let (name, bytes) =
            sc.0.as_ref()
                .map(|c| (Some(c.name.clone()), Some(c.to_bytes())))
                .unwrap();
        assert_eq!(name, Some("Adventurer".into()));
        assert_eq!(bytes, Some(c.to_bytes()));
    }

    #[test]
    fn create_requires_valid_unique_name() {
        assert!(validate_name("").is_err());
        assert!(validate_name("   ").is_err());
        assert!(validate_name("Invalid/Char").is_err());
        assert!(validate_name("Valid_Name-1").is_ok());
    }

    #[test]
    fn mediumcore_hardcore_disabled_and_skipped_by_nav() {
        use crate::menu::nav::next_rank;
        let ranks = [0, 1, 2];
        let disabled = [1, 2]; // ranks 1 and 2 disabled
        let next = next_rank(&ranks, &disabled, 0, 1);
        assert_eq!(
            next, 0,
            "must wrap back to rank 0 skipping disabled ranks 1 and 2"
        );
    }

    #[test]
    fn expert_master_disabled_in_create_world() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .add_plugins(crate::menu::create::CreatePlugin);
        app.world_mut()
            .resource_mut::<NextState<crate::session::AppState>>()
            .set(crate::session::AppState::Menu);
        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::CreateWorld);
        app.update();
        app.update();

        let mut query = app.world_mut().query::<&MenuButton>();
        let buttons: Vec<&MenuButton> = query.iter(app.world()).collect();
        let disabled_count = buttons.iter().filter(|b| b.disabled).count();
        assert!(
            disabled_count >= 2,
            "must have at least 2 disabled buttons (Expert, Master)"
        );
    }

    #[test]
    fn delete_character_needs_confirm_and_removes_only_that_file() {
        let dir = std::env::temp_dir().join(format!("tv_del_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let c1 = Character::new("Hero1", Appearance::default());
        let c2 = Character::new("Hero2", Appearance::default());
        terravibe_server::characters::save_character(&dir, &c1).unwrap();
        terravibe_server::characters::save_character(&dir, &c2).unwrap();

        // Delete c1
        delete_character(&dir, "Hero1").unwrap();
        assert!(!dir.join("Hero1.plr").exists());
        assert!(dir.join("Hero2.plr").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn back_from_worlds_returns_to_characters() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>();
        app.world_mut()
            .resource_mut::<NextState<crate::session::AppState>>()
            .set(crate::session::AppState::Menu);
        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Worlds);
        app.add_systems(
            Update,
            |current: Res<State<MenuScreen>>, mut next: ResMut<NextState<MenuScreen>>| {
                crate::menu::nav::execute_action(
                    &Action::Back,
                    Some(&current),
                    Some(&mut next),
                    None,
                    None,
                    None,
                );
            },
        );
        app.update();
        app.update();

        let screen = app.world().resource::<State<MenuScreen>>();
        assert_eq!(*screen.get(), MenuScreen::Characters);
    }

    #[test]
    fn character_state_message_rewrites_plr() {
        let dir = std::env::temp_dir().join(format!("tv_cstate_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut c = Character::new("SyncHero", Appearance::default());
        terravibe_server::characters::save_character(&dir, &c).unwrap();

        // Server sends updated CharacterState
        c.life = 80;
        c.mana = 10;
        let bytes = c.to_bytes();
        let decoded = terravibe_sim::Character::from_bytes(&bytes).unwrap();
        terravibe_server::characters::save_character(&dir, &decoded).unwrap();

        let reloaded = terravibe_server::characters::load_character(&dir, "SyncHero").unwrap();
        assert_eq!(reloaded.life, 80);
        assert_eq!(reloaded.mana, 10);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
