//! In-game pause menu, loading card, saving overlay, and error modal.

use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, Monitor, PrimaryMonitor, PrimaryWindow, Window};

use super::nav::MenuFocus;
use super::options::{OptionsHost, OptionsUiRoot, setup_options};
use super::style::{
    BUTTON_HEIGHT, BUTTON_WIDTH, COLOR_BORDER_NORMAL, COLOR_BUTTON_NORMAL, COLOR_HEADER_DIVIDER,
    COLOR_PANEL_BG, COLOR_PANEL_BORDER, COLOR_PANEL_INNER, COLOR_SLIDER_FILL, COLOR_TEXT_GOLD,
    COLOR_TEXT_MUTED, spawn_button, spawn_slider_bar,
};
use super::text::{MenuText, MenuTextColor};
use super::{Action, MenuButton};
use crate::net::{LoadingStage, NetState};
use crate::session::{
    AppState, GameMenu, GamePanel, MenuScreen, SelectedWorld, SessionError, SessionStopping,
};
use crate::settings::Settings;
use crate::view::camera::set_grab;

/// Condition helper: returns true if the in-game pause menu and Creative Mode are closed.
pub fn menu_closed(
    gm: Option<Res<GameMenu>>,
    creative: Option<Res<super::creative::CreativeMenuOpen>>,
) -> bool {
    gm.as_ref().map_or(true, |m| m.panel.is_none()) && creative.as_ref().map_or(true, |c| !c.0)
}

/// Marker component for in-game root pause menu entities.
#[derive(Component)]
pub struct GameRootUi;

/// Marker component for the world loading card UI.
#[derive(Component)]
pub struct LoadingCardUi;

/// The loading card's stage line.
#[derive(Component)]
pub struct LoadingStageText;

/// The loading card's rotating tip line.
#[derive(Component)]
pub struct LoadingTipText;

/// The loading card's visual progress bar fill node.
#[derive(Component)]
pub struct LoadingProgressFill;

/// Normalized progress fraction (0.0..=1.0) for a loading stage.
pub fn loading_progress(stage: LoadingStage) -> f32 {
    match stage {
        LoadingStage::Connecting => 0.15,
        LoadingStage::Generating => 0.35,
        LoadingStage::ReceivingChunks { received, total } => {
            0.35 + 0.65 * (received as f32 / total.max(1) as f32).clamp(0.0, 1.0)
        }
        LoadingStage::Ready => 1.0,
    }
}

/// Marker component for the quit confirmation dialog UI.
#[derive(Component)]
pub struct ConfirmQuitUi;

/// Marker component for the controls reference panel UI.
#[derive(Component)]
pub struct ControlsUiRoot;

/// Seconds each loading tip stays on screen.
pub const LOADING_TIP_SECS: f32 = 4.0;

/// Tips shown while the world loads, rotated every [`LOADING_TIP_SECS`] seconds.
pub const LOADING_TIPS: &[&str] = &[
    "Hint: Dig, fight, and build to survive.",
    "Hint: Place torches before night falls.",
    "Hint: Talk to the Guide for advice.",
    "Hint: Chop trees to craft a workbench.",
    "Hint: Explore caves for ore and treasure.",
];

/// Text shown for a loading stage.
pub fn loading_stage_text(stage: LoadingStage) -> String {
    match stage {
        LoadingStage::Connecting => "Connecting to server...".into(),
        LoadingStage::Generating => "Generating world...".into(),
        LoadingStage::ReceivingChunks { received, total } => {
            format!("Receiving chunks... ({received}/{total})")
        }
        LoadingStage::Ready => "Ready".into(),
    }
}

/// Tip for the elapsed loading time; wraps after all tips have been shown.
pub fn loading_tip(elapsed: f32) -> &'static str {
    let index = (elapsed.max(0.0) / LOADING_TIP_SECS) as usize % LOADING_TIPS.len();
    LOADING_TIPS[index]
}

/// Tracks how long the loading card has been visible, for tip rotation.
#[derive(Resource, Default)]
pub struct LoadingTipState {
    pub elapsed: f32,
}

/// Marker component for the saving world overlay UI.
#[derive(Component)]
pub struct SavingOverlayUi;

/// Marker component for the error modal UI.
#[derive(Component)]
pub struct ErrorModalUi;

/// Marker component for the error message text in the error modal.
#[derive(Component)]
pub struct ErrorModalMessage;

/// Stores the current Guide advice synchronized from the server.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct GuideAdviceState {
    pub current_advice: String,
}

impl Default for GuideAdviceState {
    fn default() -> Self {
        Self {
            current_advice: terravibe_sim::facts::current_guide_advice(
                &terravibe_sim::facts::WorldFacts::default(),
                &terravibe_sim::facts::CharacterFacts::default(),
            )
            .to_string(),
        }
    }
}

/// Handles keyboard Esc and F2 toggling in InGame.
#[allow(clippy::too_many_arguments)]
pub fn handle_game_menu_input(
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mut game_menu: ResMut<GameMenu>,
    mut creative_open: Option<ResMut<super::creative::CreativeMenuOpen>>,
    session_error: Res<SessionError>,
    app_state: Option<Res<State<AppState>>>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut action_writer: MessageWriter<Action>,
    mut esc_panels: crate::hud::inventory::EscPanels,
    mut menu_focus: Option<ResMut<MenuFocus>>,
    settings: Option<Res<crate::settings::Settings>>,
) {
    if app_state.as_ref().map(|s| s.get()) != Some(&AppState::InGame) {
        return;
    }
    if session_error.0.is_some() {
        return;
    }
    let Some(kb) = keyboard else { return };

    if let Some(ref mut creative) = creative_open {
        if creative.0 {
            if kb.just_pressed(KeyCode::Escape) || kb.just_pressed(KeyCode::F2) {
                creative.0 = false;
                if let Ok(mut opts) = cursor_query.single_mut() {
                    set_grab(&mut opts, true);
                }
            }
            return;
        }
        if game_menu.panel.is_none() && kb.just_pressed(KeyCode::F2) {
            creative.0 = true;
            if let Ok(mut opts) = cursor_query.single_mut() {
                set_grab(&mut opts, false);
            }
            return;
        }
    }

    let inv_key = settings.as_deref().map_or(KeyCode::Escape, |s| {
        s.bindings.key(crate::input_map::GameAction::Inventory)
    });

    if game_menu.panel.is_none() && inv_key != KeyCode::Escape && kb.just_pressed(inv_key) {
        if let Ok(mut opts) = cursor_query.single_mut() {
            let _ = esc_panels.esc(&mut opts);
        }
        return;
    }

    if kb.just_pressed(KeyCode::Escape) {
        match game_menu.panel {
            None => {
                let Ok(mut opts) = cursor_query.single_mut() else {
                    return;
                };
                // Esc opens or closes the inventory screen when Inventory is bound to Esc;
                // otherwise (or when dead) Esc opens the Game Menu.
                if inv_key != KeyCode::Escape || !esc_panels.esc(&mut opts) {
                    game_menu.panel = Some(GamePanel::Root);
                    if let Some(ref mut mf) = menu_focus {
                        mf.rank = Some(0);
                    }
                    set_grab(&mut opts, false);
                }
            }
            Some(GamePanel::Root) => {
                game_menu.panel = None;
                if let Some(ref mut mf) = menu_focus {
                    mf.rank = None;
                }
                if let Ok(mut opts) = cursor_query.single_mut() {
                    set_grab(&mut opts, true);
                }
            }
            Some(GamePanel::Options) => {
                action_writer.write(Action::Back);
            }
            Some(GamePanel::Controls) => {
                game_menu.panel = Some(GamePanel::Root);
                if let Some(ref mut mf) = menu_focus {
                    mf.rank = Some(0);
                }
            }
            Some(GamePanel::ConfirmQuit) => {
                game_menu.panel = Some(GamePanel::Root);
                if let Some(ref mut mf) = menu_focus {
                    mf.rank = Some(0);
                }
            }
        }
    }
}

/// Handles game menu specific actions (Resume, GameOptions, SaveQuit, DismissError).
pub fn handle_game_menu_actions(
    actions: Option<MessageReader<Action>>,
    mut game_menu: Option<ResMut<GameMenu>>,
    mut creative_open: Option<ResMut<super::creative::CreativeMenuOpen>>,
    mut menu_focus: Option<ResMut<MenuFocus>>,
    mut session_error: ResMut<SessionError>,
    mut next_app_state: ResMut<NextState<AppState>>,
    mut next_menu_screen: Option<ResMut<NextState<MenuScreen>>>,
    mut options_host: Option<ResMut<OptionsHost>>,
    settings: Option<Res<Settings>>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    primary_monitor: Query<&Monitor, With<PrimaryMonitor>>,
    all_monitors: Query<&Monitor>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    root_entities: Query<Entity, With<GameRootUi>>,
    error_entities: Query<Entity, With<ErrorModalUi>>,
    mut commands: Commands,
) {
    let Some(mut action_reader) = actions else {
        return;
    };

    for action in action_reader.read() {
        match action {
            Action::Resume => {
                if let Some(ref mut gm) = game_menu {
                    gm.panel = None;
                }
                if let Ok(mut opts) = cursor_query.single_mut() {
                    set_grab(&mut opts, true);
                }
            }
            Action::OpenCreative => {
                if let Some(ref mut gm) = game_menu {
                    gm.panel = None;
                }
                if let Some(ref mut mf) = menu_focus {
                    mf.rank = None;
                }
                if let Some(ref mut creative) = creative_open {
                    creative.0 = true;
                }
                if let Ok(mut opts) = cursor_query.single_mut() {
                    set_grab(&mut opts, false);
                }
            }
            Action::GameOptions => {
                if let Some(ref mut gm) = game_menu {
                    gm.panel = Some(GamePanel::Options);
                }
                if let Some(ref mut host) = options_host {
                    **host = OptionsHost::GameMenu;
                }
                for e in &root_entities {
                    commands.entity(e).despawn();
                }
                if let Some(ref s) = settings {
                    setup_options(
                        &mut commands,
                        s,
                        Some(OptionsHost::GameMenu),
                        &primary_window,
                        &primary_monitor,
                        &all_monitors,
                    );
                }
            }
            Action::GameControls => {
                if let Some(ref mut gm) = game_menu {
                    gm.panel = Some(GamePanel::Controls);
                }
                if let Some(ref mut mf) = menu_focus {
                    mf.rank = Some(0);
                }
            }
            Action::ResumeRoot => {
                if let Some(ref mut gm) = game_menu {
                    gm.panel = Some(GamePanel::Root);
                }
                if let Some(ref mut mf) = menu_focus {
                    mf.rank = Some(0);
                }
            }
            Action::SaveQuit => {
                if let Some(ref mut gm) = game_menu {
                    gm.panel = Some(GamePanel::ConfirmQuit);
                }
                if let Some(ref mut mf) = menu_focus {
                    mf.rank = Some(1);
                }
            }
            Action::ConfirmQuit => {
                next_app_state.set(AppState::Menu);
                if let Some(ref mut next_menu) = next_menu_screen {
                    next_menu.set(MenuScreen::Title);
                }
            }
            Action::CancelQuit => {
                if let Some(ref mut gm) = game_menu {
                    gm.panel = Some(GamePanel::Root);
                }
                if let Some(ref mut mf) = menu_focus {
                    mf.rank = Some(0);
                }
            }
            Action::DismissError => {
                session_error.0 = None;
                commands.remove_resource::<crate::session::DisconnectState>();
                commands.remove_resource::<crate::session::SessionRetryPending>();
                if let Some(ref mut gm) = game_menu {
                    gm.panel = None;
                }
                if let Ok(mut opts) = cursor_query.single_mut() {
                    set_grab(&mut opts, false);
                }
                next_app_state.set(AppState::Menu);
                if let Some(ref mut next_menu) = next_menu_screen {
                    next_menu.set(MenuScreen::Title);
                }
                for e in &error_entities {
                    commands.entity(e).despawn();
                }
            }
            Action::RetryConnection => {
                session_error.0 = None;
                commands.remove_resource::<crate::session::DisconnectState>();
                if let Some(ref mut gm) = game_menu {
                    gm.panel = None;
                }
                if let Ok(mut opts) = cursor_query.single_mut() {
                    set_grab(&mut opts, false);
                }
                commands.insert_resource(crate::session::SessionRetryPending);
                next_app_state.set(AppState::Menu);
                for e in &error_entities {
                    commands.entity(e).despawn();
                }
            }
            _ => {}
        }
    }
}

fn spawn_game_root_ui(commands: &mut Commands, advice: &str, viewport: (f32, f32)) {
    let (win_w, win_h) = viewport;
    let center_x = win_w * 0.5;
    let panel_w = 460.0;
    let panel_h = 444.0;
    let panel_x = center_x - panel_w / 2.0;
    let panel_y = ((win_h - panel_h) * 0.5).max(36.0);

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.55)),
        GlobalZIndex(1299),
        GameRootUi,
    ));

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(panel_x),
                top: Val::Px(panel_y),
                width: Val::Px(panel_w),
                height: Val::Px(panel_h),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(20.0)),
                ..default()
            },
            BackgroundColor(COLOR_PANEL_BG),
            BorderColor::all(COLOR_PANEL_BORDER),
            GlobalZIndex(1300),
            GameRootUi,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(if advice.is_empty() { 24.0 } else { 8.0 })),
                    ..default()
                },
                MenuText::new("Game Menu", 1.5),
                GameRootUi,
            ));
            if !advice.is_empty() {
                p.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(16.0)),
                        max_width: Val::Px(420.0),
                        ..default()
                    },
                    MenuText::new(&format!("Guide's Advice: {advice}"), 0.8),
                    GameRootUi,
                ));
            }
        })
        .id();

    let btn_x = center_x - BUTTON_WIDTH / 2.0;
    let start_y = panel_y + 132.0;
    let step_y = 54.0;

    let b0 = spawn_button(commands, "Resume", Action::Resume, 0, (btn_x, start_y));
    commands.entity(b0).insert((GameRootUi, GlobalZIndex(1301)));

    let b1 = spawn_button(
        commands,
        "Creative Mode",
        Action::OpenCreative,
        1,
        (btn_x, start_y + step_y),
    );
    commands.entity(b1).insert((GameRootUi, GlobalZIndex(1301)));

    let b2 = spawn_button(
        commands,
        "Options",
        Action::GameOptions,
        2,
        (btn_x, start_y + step_y * 2.0),
    );
    commands.entity(b2).insert((GameRootUi, GlobalZIndex(1301)));

    let b3 = spawn_button(
        commands,
        "Controls",
        Action::GameControls,
        3,
        (btn_x, start_y + step_y * 3.0),
    );
    commands.entity(b3).insert((GameRootUi, GlobalZIndex(1301)));

    let b4 = spawn_button(
        commands,
        "Save & Quit",
        Action::SaveQuit,
        4,
        (btn_x, start_y + step_y * 4.0),
    );
    commands.entity(b4).insert((GameRootUi, GlobalZIndex(1301)));

    let _ = panel;
}

/// Synchronizes the root UI of the pause menu with GameMenu.panel.
#[allow(clippy::too_many_arguments)]
pub fn sync_game_root_ui(
    game_menu: Res<GameMenu>,
    session_error: Res<SessionError>,
    root_entities: Query<Entity, With<GameRootUi>>,
    confirm_entities: Query<Entity, With<ConfirmQuitUi>>,
    controls_entities: Query<Entity, With<ControlsUiRoot>>,
    advice_state: Option<Res<GuideAdviceState>>,
    settings: Option<Res<crate::settings::Settings>>,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
    mut commands: Commands,
) {
    let should_show = game_menu.panel == Some(GamePanel::Root) && session_error.0.is_none();
    let root_exists = root_entities.iter().next().is_some();

    if should_show && !root_exists {
        let default_advice = terravibe_sim::facts::current_guide_advice(
            &terravibe_sim::facts::WorldFacts::default(),
            &terravibe_sim::facts::CharacterFacts::default(),
        );
        let advice = advice_state
            .as_ref()
            .map_or(default_advice, |a| a.current_advice.as_str());
        spawn_game_root_ui(
            &mut commands,
            advice,
            super::style::viewport_size(primary_window.as_ref()),
        );
    } else if !should_show && root_exists {
        for e in &root_entities {
            commands
                .entity(e)
                .queue_silenced(move |entity: EntityWorldMut| {
                    entity.despawn();
                });
        }
    }

    let controls_should_show =
        game_menu.panel == Some(GamePanel::Controls) && session_error.0.is_none();
    let controls_exist = controls_entities.iter().next().is_some();
    if controls_should_show && !controls_exist {
        let bindings = settings
            .as_deref()
            .map_or_else(crate::input_map::InputBindings::default, |s| s.bindings);
        spawn_controls_ui(&mut commands, &bindings);
    } else if !controls_should_show && controls_exist {
        for e in &controls_entities {
            commands
                .entity(e)
                .queue_silenced(move |entity: EntityWorldMut| {
                    entity.despawn();
                });
        }
    }

    let confirm_should_show =
        game_menu.panel == Some(GamePanel::ConfirmQuit) && session_error.0.is_none();
    let confirm_exists = confirm_entities.iter().next().is_some();
    if confirm_should_show && !confirm_exists {
        spawn_confirm_quit(&mut commands);
    } else if !confirm_should_show && confirm_exists {
        for e in &confirm_entities {
            commands.entity(e).despawn();
        }
    }
}

fn spawn_confirm_quit(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.55)),
            GlobalZIndex(1299),
            ConfirmQuitUi,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Px(560.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(28.0)),
                    ..default()
                },
                BackgroundColor(COLOR_PANEL_BG),
                BorderColor::all(COLOR_PANEL_BORDER),
                GlobalZIndex(1300),
                ConfirmQuitUi,
            ))
            .with_children(|card| {
                card.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(24.0)),
                        ..default()
                    },
                    MenuText::new("Save and return to Title Screen?", 1.0),
                    ConfirmQuitUi,
                ));
                card.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    ConfirmQuitUi,
                ))
                .with_children(|row| {
                    for (i, (label, action, rank)) in [
                        ("Save & Exit", Action::ConfirmQuit, 0u16),
                        ("Cancel", Action::CancelQuit, 1u16),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        row.spawn((
                            Node {
                                width: Val::Px(220.0),
                                height: Val::Px(BUTTON_HEIGHT),
                                border: UiRect::all(Val::Px(2.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                margin: UiRect::right(Val::Px(if i == 0 { 16.0 } else { 0.0 })),
                                ..default()
                            },
                            BackgroundColor(COLOR_BUTTON_NORMAL),
                            BorderColor::all(COLOR_BORDER_NORMAL),
                            MenuButton {
                                action,
                                rank,
                                disabled: false,
                            },
                            GlobalZIndex(1301),
                            ConfirmQuitUi,
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Node {
                                    position_type: PositionType::Relative,
                                    ..default()
                                },
                                MenuText::new(label, 1.0),
                                ConfirmQuitUi,
                            ));
                        });
                    }
                });
            });
        });
}

pub fn format_controls_sections(
    bindings: &crate::input_map::InputBindings,
) -> [(&'static str, Vec<String>); 2] {
    use crate::input_map::{GameAction, keycode_name};
    let k = |a: GameAction| {
        let name = keycode_name(bindings.key(a)).unwrap_or("?");
        if name == "Escape" {
            "Esc"
        } else {
            name.strip_prefix("Key").unwrap_or(name)
        }
    };
    [
        (
            "Movement & Combat",
            vec![
                format!(
                    "Move: {}{}{}{} ({}, {}, {}, {})",
                    k(GameAction::MoveForward),
                    k(GameAction::MoveLeft),
                    k(GameAction::MoveBack),
                    k(GameAction::MoveRight),
                    k(GameAction::MoveForward),
                    k(GameAction::MoveLeft),
                    k(GameAction::MoveBack),
                    k(GameAction::MoveRight)
                ),
                format!("Jump: {}", k(GameAction::Jump)),
                format!("Drop: {}", k(GameAction::MoveBack)),
                "Mine/Attack: Left Click".to_string(),
                "Place/Interact: Right Click".to_string(),
                format!("Throw Item: {}", k(GameAction::Throw)),
            ],
        ),
        (
            "Interface",
            vec![
                format!("Inventory: {}", k(GameAction::Inventory)),
                format!("Minimap: {}", k(GameAction::Minimap)),
                format!("Fullscreen Map: {}", k(GameAction::FullscreenMap)),
                format!("Quick Heal: {}", k(GameAction::QuickHeal)),
                format!("Quick Mana: {}", k(GameAction::QuickMana)),
                format!("Quick Buff: {}", k(GameAction::QuickBuff)),
                format!("Equip Armor: {}", k(GameAction::EquipArmor)),
                format!("Break Wall: {}", k(GameAction::BreakWall)),
                "Pause: Esc".to_string(),
            ],
        ),
    ]
}

fn spawn_controls_ui(commands: &mut Commands, bindings: &crate::input_map::InputBindings) {
    let sections = format_controls_sections(bindings);

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.06, 0.62)),
            GlobalZIndex(1299),
            ControlsUiRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Px(740.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(24.0)),
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                BackgroundColor(COLOR_PANEL_BG),
                BorderColor::all(COLOR_PANEL_BORDER),
                GlobalZIndex(1300),
            ))
            .with_children(|card| {
                card.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    },
                    MenuText::new("Controls", 1.5),
                    MenuTextColor(COLOR_TEXT_GOLD),
                ));
                card.spawn((
                    Node {
                        width: Val::Px(680.0),
                        height: Val::Px(2.0),
                        margin: UiRect::bottom(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(COLOR_HEADER_DIVIDER),
                ));

                // Clean 2-column layout: left (Movement & Actions + Combat & Items), right (Interface)
                card.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(16.0),
                    align_items: AlignItems::Stretch,
                    ..default()
                })
                .with_children(|cols| {
                    let spawn_section_block =
                        |col: &mut ChildSpawnerCommands, title: &str, lines: &[String]| {
                            col.spawn((
                                Node {
                                    margin: UiRect::bottom(Val::Px(4.0)),
                                    ..default()
                                },
                                MenuText::new(title, 1.05),
                                MenuTextColor(COLOR_TEXT_GOLD),
                            ));
                            for line in lines {
                                col.spawn((Node::default(), MenuText::new(line.as_str(), 0.88)));
                            }
                        };

                    cols.spawn((
                        Node {
                            flex_grow: 1.0,
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(14.0)),
                            row_gap: Val::Px(5.0),
                            ..default()
                        },
                        BackgroundColor(COLOR_PANEL_INNER),
                        BorderColor::all(COLOR_PANEL_BORDER),
                    ))
                    .with_children(|left| {
                        for (idx, (title, lines)) in sections.iter().take(2).enumerate() {
                            if idx > 0 {
                                left.spawn(Node {
                                    height: Val::Px(8.0),
                                    ..default()
                                });
                            }
                            spawn_section_block(left, title, lines);
                        }
                    });

                    cols.spawn((
                        Node {
                            flex_grow: 1.0,
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(14.0)),
                            row_gap: Val::Px(5.0),
                            ..default()
                        },
                        BackgroundColor(COLOR_PANEL_INNER),
                        BorderColor::all(COLOR_PANEL_BORDER),
                    ))
                    .with_children(|right| {
                        if let Some((title, lines)) = sections.get(2) {
                            spawn_section_block(right, title, lines);
                        }
                    });
                });

                card.spawn((
                    Node {
                        width: Val::Px(BUTTON_WIDTH),
                        height: Val::Px(BUTTON_HEIGHT),
                        margin: UiRect::top(Val::Px(12.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(COLOR_BUTTON_NORMAL),
                    BorderColor::all(COLOR_BORDER_NORMAL),
                    MenuButton {
                        action: Action::ResumeRoot,
                        rank: 0,
                        disabled: false,
                    },
                    GlobalZIndex(1301),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Node {
                            position_type: PositionType::Relative,
                            ..default()
                        },
                        MenuText::new("Back", 1.0),
                    ));
                });
            });
        });
}

fn spawn_loading_card(
    commands: &mut Commands,
    name: &str,
    seed: u32,
    stage: &str,
    progress: f32,
    tip: &str,
) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.80)),
            LoadingCardUi,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Px(520.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(28.0)),
                    ..default()
                },
                BackgroundColor(COLOR_PANEL_BG),
                BorderColor::all(COLOR_PANEL_BORDER),
            ))
            .with_children(|card| {
                card.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(12.0)),
                        ..default()
                    },
                    MenuText::new(stage, 1.45),
                    MenuTextColor(COLOR_TEXT_GOLD),
                    LoadingStageText,
                ));
                card.spawn((
                    Node {
                        width: Val::Px(440.0),
                        height: Val::Px(2.0),
                        margin: UiRect::bottom(Val::Px(14.0)),
                        ..default()
                    },
                    BackgroundColor(COLOR_HEADER_DIVIDER),
                ));
                card.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(6.0)),
                        ..default()
                    },
                    MenuText::new(&format!("World: {name}"), 1.15),
                ));
                card.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(16.0)),
                        ..default()
                    },
                    MenuText::new(&format!("Seed: {seed}"), 0.95),
                    MenuTextColor(COLOR_TEXT_MUTED),
                ));
                let fill_entity = spawn_slider_bar(card, 400.0, 14.0, progress, COLOR_SLIDER_FILL);
                card.commands()
                    .entity(fill_entity)
                    .insert(LoadingProgressFill);
                card.spawn((
                    Node {
                        margin: UiRect::top(Val::Px(16.0)),
                        ..default()
                    },
                    MenuText::new(tip, 0.85),
                    MenuTextColor(COLOR_TEXT_MUTED),
                    LoadingTipText,
                ));
            });
        });
}

/// Synchronizes the world loading card in InGame while !NetState::is_ready().
#[allow(clippy::too_many_arguments)]
pub fn sync_loading_card(
    net_state: Option<Res<NetState>>,
    selected_world: Option<Res<SelectedWorld>>,
    session_error: Res<SessionError>,
    time: Res<Time>,
    mut tip_state: ResMut<LoadingTipState>,
    loading_entities: Query<Entity, With<LoadingCardUi>>,
    mut stage_texts: Query<&mut MenuText, (With<LoadingStageText>, Without<LoadingTipText>)>,
    mut tip_texts: Query<&mut MenuText, (With<LoadingTipText>, Without<LoadingStageText>)>,
    mut progress_fills: Query<&mut Node, With<LoadingProgressFill>>,
    mut commands: Commands,
) {
    let stage = net_state
        .as_ref()
        .map_or(LoadingStage::Connecting, |st| st.loading_stage());
    let is_ready = stage == LoadingStage::Ready;
    let has_error = session_error.0.is_some();
    let should_show = !is_ready && !has_error;
    let card_exists = loading_entities.iter().next().is_some();

    if should_show {
        tip_state.elapsed += time.delta_secs();
        let stage_text = loading_stage_text(stage);
        let progress = loading_progress(stage);
        let tip = loading_tip(tip_state.elapsed);

        if !card_exists {
            let name = selected_world
                .as_ref()
                .map_or("Default", |w| w.name.as_str());
            let seed = selected_world.as_ref().map_or(0, |w| w.seed);
            spawn_loading_card(&mut commands, name, seed, &stage_text, progress, tip);
        } else {
            for mut text in &mut stage_texts {
                if text.text != stage_text {
                    text.text = stage_text.clone();
                }
            }
            for mut text in &mut tip_texts {
                if text.text != tip {
                    text.text = tip.to_string();
                }
            }
            for mut node in &mut progress_fills {
                node.width = Val::Percent(progress.clamp(0.0, 1.0) * 100.0);
            }
        }
    } else {
        tip_state.elapsed = 0.0;
        if card_exists {
            for e in &loading_entities {
                commands.entity(e).despawn();
            }
        }
    }
}

fn spawn_saving_overlay(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.85)),
            SavingOverlayUi,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Px(420.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(24.0)),
                    ..default()
                },
                BackgroundColor(COLOR_PANEL_BG),
                BorderColor::all(COLOR_PANEL_BORDER),
            ))
            .with_children(|card| {
                card.spawn((Node::default(), MenuText::new("Saving world...", 1.5)));
            });
        });
}

/// Synchronizes the "Saving world..." overlay while SessionStopping exists in Menu.
pub fn sync_saving_overlay(
    stopping: Option<Res<SessionStopping>>,
    saving_entities: Query<Entity, With<SavingOverlayUi>>,
    mut commands: Commands,
) {
    let should_show = stopping.is_some();
    let overlay_exists = saving_entities.iter().next().is_some();

    if should_show && !overlay_exists {
        spawn_saving_overlay(&mut commands);
    } else if !should_show && overlay_exists {
        for e in &saving_entities {
            commands
                .entity(e)
                .queue_silenced(move |entity: EntityWorldMut| {
                    entity.despawn();
                });
        }
    }
}

fn spawn_error_modal_button(
    card: &mut ChildSpawnerCommands,
    label: &str,
    action: Action,
    rank: u16,
    bottom_margin: f32,
) {
    card.spawn((
        Node {
            width: Val::Px(BUTTON_WIDTH),
            height: Val::Px(BUTTON_HEIGHT),
            margin: UiRect::bottom(Val::Px(bottom_margin)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(COLOR_BUTTON_NORMAL),
        BorderColor::all(COLOR_BORDER_NORMAL),
        MenuButton {
            action,
            rank,
            disabled: false,
        },
        ErrorModalUi,
    ))
    .with_children(|btn| {
        btn.spawn((
            Node {
                position_type: PositionType::Relative,
                ..default()
            },
            MenuText::new(label, 1.0),
            ErrorModalUi,
        ));
    });
}

fn spawn_error_modal(commands: &mut Commands, message: &str, is_disconnect: bool) {
    let heading = if is_disconnect {
        "Disconnected"
    } else {
        "Error"
    };
    let overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.85)),
            ErrorModalUi,
            GlobalZIndex(2),
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Px(560.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::all(Val::Px(28.0)),
                    ..default()
                },
                BackgroundColor(COLOR_PANEL_BG),
                BorderColor::all(COLOR_PANEL_BORDER),
                ErrorModalUi,
            ))
            .with_children(|card| {
                card.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(16.0)),
                        ..default()
                    },
                    MenuText::new(heading, 1.5),
                    ErrorModalUi,
                ));
                card.spawn((
                    Node {
                        margin: UiRect::bottom(Val::Px(24.0)),
                        ..default()
                    },
                    MenuText::new(message, 1.0),
                    ErrorModalUi,
                    ErrorModalMessage,
                ));
                if is_disconnect {
                    spawn_error_modal_button(card, "Retry", Action::RetryConnection, 0, 12.0);
                    spawn_error_modal_button(card, "Back to Menu", Action::DismissError, 1, 0.0);
                } else {
                    spawn_error_modal_button(card, "Return to title", Action::DismissError, 0, 0.0);
                }
            });
        })
        .id();

    let _ = overlay;
}

/// Synchronizes the error modal when SessionError is Some.
pub fn sync_error_modal(
    session_error: Res<SessionError>,
    disconnect_state: Option<Res<crate::session::DisconnectState>>,
    error_entities: Query<Entity, With<ErrorModalUi>>,
    text_entities: Query<(Entity, &MenuText), With<ErrorModalUi>>,
    message_query: Query<&MenuText, With<ErrorModalMessage>>,
    button_query: Query<&MenuButton, With<ErrorModalUi>>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut focus: Option<ResMut<MenuFocus>>,
    mut commands: Commands,
) {
    if let Some(msg) = session_error.0.as_deref() {
        let is_disconnect = disconnect_state.is_some();
        if let Ok(mut opts) = cursor_query.single_mut() {
            if opts.grab_mode != CursorGrabMode::None || !opts.visible {
                set_grab(&mut opts, false);
            }
        }
        if let Some(ref mut f) = focus {
            if f.rank != Some(0) {
                f.rank = Some(0);
            }
        }

        if error_entities.iter().next().is_none() {
            spawn_error_modal(&mut commands, msg, is_disconnect);
        } else {
            let has_retry = button_query
                .iter()
                .any(|b| b.action == Action::RetryConnection);
            let msg_mismatch = if let Ok(current_msg) = message_query.single() {
                current_msg.text != msg
            } else {
                text_entities.iter().any(|(_, t)| {
                    t.text != "Error"
                        && t.text != "Disconnected"
                        && t.text != "Return to title"
                        && t.text != "Retry"
                        && t.text != "Back to Menu"
                        && t.text != msg
                })
            };
            if has_retry != is_disconnect || msg_mismatch {
                for e in &error_entities {
                    commands.entity(e).despawn();
                }
                spawn_error_modal(&mut commands, msg, is_disconnect);
            }
        }
    } else if error_entities.iter().next().is_some() {
        for e in &error_entities {
            commands.entity(e).despawn();
        }
    }
}

/// Despawns in-game pause and overlay entities on leaving InGame.
fn despawn_ingame_overlays(
    root: Query<Entity, With<GameRootUi>>,
    loading: Query<Entity, With<LoadingCardUi>>,
    options: Query<Entity, With<OptionsUiRoot>>,
    confirm: Query<Entity, With<ConfirmQuitUi>>,
    controls: Query<Entity, With<ControlsUiRoot>>,
    mut commands: Commands,
) {
    for e in &root {
        commands
            .entity(e)
            .queue_silenced(move |entity: EntityWorldMut| {
                entity.despawn();
            });
    }
    for e in &loading {
        commands
            .entity(e)
            .queue_silenced(move |entity: EntityWorldMut| {
                entity.despawn();
            });
    }
    for e in &options {
        commands
            .entity(e)
            .queue_silenced(move |entity: EntityWorldMut| {
                entity.despawn();
            });
    }
    for e in &confirm {
        commands
            .entity(e)
            .queue_silenced(move |entity: EntityWorldMut| {
                entity.despawn();
            });
    }
    for e in &controls {
        commands
            .entity(e)
            .queue_silenced(move |entity: EntityWorldMut| {
                entity.despawn();
            });
    }
}

pub struct GameMenuPlugin;

impl Plugin for GameMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GuideAdviceState>()
            .init_resource::<LoadingTipState>()
            .add_systems(
                Update,
                (
                    handle_game_menu_input
                        .before(crate::menu::options::handle_options_actions)
                        .run_if(in_state(AppState::InGame)),
                    handle_game_menu_actions
                        .after(crate::menu::nav::handle_nav_input)
                        .after(crate::menu::nav::handle_mouse_input),
                    sync_error_modal,
                ),
            )
            .add_systems(
                Update,
                (
                    sync_game_root_ui
                        .after(handle_game_menu_actions)
                        .after(crate::menu::options::handle_options_actions),
                    sync_loading_card,
                )
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(Update, sync_saving_overlay.run_if(in_state(AppState::Menu)))
            .add_systems(OnExit(AppState::InGame), despawn_ingame_overlays);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::style::{BUTTON_HEIGHT, BUTTON_WIDTH};
    use crate::menu::{MenuButton, MenuFocus, Screen};
    use crate::net::{ClientClock, LoadingStage, NetState};
    use crate::session::{SessionHandle, SessionPlugin};
    use crate::view::chunks::ChunkStream;
    use crate::view::player::{LocalPlayer, SampledInput};
    use bevy::input::ButtonState;
    use bevy::input::keyboard::{Key, KeyboardInput};
    use bevy::input::mouse::MouseButtonInput;
    use bevy::ui::UiGlobalTransform;
    use bevy::window::CursorGrabMode;
    use terravibe_sim::player::Player;

    fn open_pause_menu(app: &mut App) {
        app.world_mut().resource_mut::<GameMenu>().panel = Some(GamePanel::Root);
        app.update();
    }

    fn activate_button(app: &mut App, action: Action) {
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(action);
    }

    fn press_key(app: &mut App, key_code: KeyCode, logical_key: Key) {
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(KeyboardInput {
                logical_key,
                key_code,
                state: bevy::input::ButtonState::Pressed,
                window: Entity::PLACEHOLDER,
                repeat: false,
                text: None,
            });
    }

    fn press_enter(app: &mut App) {
        press_key(app, KeyCode::Enter, Key::Enter);
    }

    fn press_escape(app: &mut App) {
        press_key(app, KeyCode::Escape, Key::Escape);
    }

    #[test]
    fn loading_card_reflects_loading_stages() {
        let mut net = NetState::default();

        // Stage 1: Connecting before hello is sent.
        assert_eq!(net.loading_stage(), LoadingStage::Connecting);

        // Stage 2: Generating after hello is sent, before spawn is known.
        net.mark_hello_sent();
        assert_eq!(net.loading_stage(), LoadingStage::Generating);

        // Stage 3: Receiving chunks once spawn is known.
        net.set_spawn([100.0, 50.0, 100.0]);
        assert!(matches!(
            net.loading_stage(),
            LoadingStage::ReceivingChunks { .. }
        ));

        // Stage 4: Ready once the surrounding chunks have arrived.
        net.mark_ready();
        assert_eq!(net.loading_stage(), LoadingStage::Ready);
    }

    #[test]
    fn loading_card_updates_stage_text_and_rotates_tip() {
        let mut app = base_test_app();
        app.world_mut().insert_resource(SelectedWorld {
            name: "Alpha".into(),
            seed: 42,
            persist: false,
            ..Default::default()
        });
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.world_mut().insert_resource(NetState::default());
        app.update();

        let texts = |app: &mut App| -> Vec<String> {
            app.world_mut()
                .query::<&MenuText>()
                .iter(app.world())
                .map(|t| t.text.clone())
                .collect()
        };
        assert!(
            texts(&mut app).iter().any(|t| t.contains("Connecting")),
            "initial stage text missing"
        );

        app.world_mut().resource_mut::<NetState>().mark_hello_sent();
        app.update();
        assert!(
            texts(&mut app).iter().any(|t| t.contains("Generating")),
            "stage text must update after hello"
        );

        app.world_mut()
            .resource_mut::<NetState>()
            .set_spawn([100.0, 50.0, 100.0]);
        app.update();
        assert!(
            texts(&mut app)
                .iter()
                .any(|t| t.contains("Receiving chunks")),
            "stage text must update once spawn is known"
        );

        // Force the tip timer into the next 4-second slot.
        app.world_mut().resource_mut::<LoadingTipState>().elapsed = LOADING_TIP_SECS;
        app.update();
        assert!(
            texts(&mut app).iter().any(|t| t == LOADING_TIPS[1]),
            "tip must rotate every {LOADING_TIP_SECS}s"
        );
    }

    #[test]
    fn loading_stage_text_describes_each_stage() {
        assert!(loading_stage_text(LoadingStage::Connecting).contains("Connecting"));
        assert!(loading_stage_text(LoadingStage::Generating).contains("Generating"));
        let receiving = loading_stage_text(LoadingStage::ReceivingChunks {
            received: 3,
            total: 9,
        });
        assert!(receiving.contains("3"));
        assert!(receiving.contains("9"));
        assert!(loading_stage_text(LoadingStage::Ready).contains("Ready"));
    }

    #[test]
    fn loading_tip_rotates_every_four_seconds() {
        assert_eq!(loading_tip(0.0), LOADING_TIPS[0]);
        assert_eq!(loading_tip(LOADING_TIP_SECS - 0.01), LOADING_TIPS[0]);
        assert_eq!(loading_tip(LOADING_TIP_SECS), LOADING_TIPS[1]);
        let cycle = LOADING_TIP_SECS * LOADING_TIPS.len() as f32;
        assert_eq!(loading_tip(cycle), LOADING_TIPS[0]);
    }

    fn base_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(bevy::window::WindowPlugin::default())
            .add_plugins(SessionPlugin)
            .add_message::<Action>()
            .init_resource::<MenuFocus>()
            .add_plugins((
                crate::menu::style::StylePlugin,
                crate::menu::nav::NavPlugin,
                crate::menu::text::TextPlugin,
                crate::menu::options::OptionsPlugin,
                GameMenuPlugin,
            ));

        // Ensure primary window has CursorOptions initialized with Locked
        if let Ok(mut opts) = app
            .world_mut()
            .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
        {
            opts.grab_mode = CursorGrabMode::Locked;
            opts.visible = false;
        }
        app
    }

    #[test]
    fn game_menu_buttons_have_explicit_z_above_panel() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        open_pause_menu(&mut app);

        let mut panel_query = app.world_mut().query_filtered::<(
            &BackgroundColor,
            Option<&GlobalZIndex>,
        ), (With<GameRootUi>, Without<MenuButton>)>();
        let panels: Vec<_> = panel_query
            .iter(app.world())
            .filter(|(bg, _)| bg.0 == COLOR_PANEL_BG)
            .collect();
        assert_eq!(panels.len(), 1, "expected exactly 1 GameRootUi panel");
        let panel_z = panels[0]
            .1
            .map(|z| z.0)
            .expect("GameRootUi panel must have explicit GlobalZIndex");

        let mut btn_query = app
            .world_mut()
            .query_filtered::<(&MenuButton, Option<&GlobalZIndex>), With<GameRootUi>>();
        let buttons: Vec<_> = btn_query.iter(app.world()).collect();
        assert_eq!(buttons.len(), 5, "expected 5 Game Menu buttons");

        for (btn, maybe_z) in buttons {
            let btn_z = maybe_z.map(|z| z.0).unwrap_or_else(|| {
                panic!(
                    "Game Menu button rank {} must have explicit GlobalZIndex",
                    btn.rank
                )
            });
            assert!(
                btn_z > panel_z,
                "Game Menu button rank {} (z={btn_z}) must be strictly greater than panel (z={panel_z})",
                btn.rank
            );
        }
    }

    #[test]
    fn error_modal_layer_above_ingame_options() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::GameOptions);
        app.update();
        app.world_mut().resource_mut::<SessionError>().0 = Some("error".into());
        app.update();
        let mut options = app
            .world_mut()
            .query_filtered::<&GlobalZIndex, With<OptionsUiRoot>>();
        let max_z = options.iter(app.world()).map(|z| z.0).max().unwrap();
        let mut overlay = app
            .world_mut()
            .query_filtered::<Option<&GlobalZIndex>, (With<ErrorModalUi>, Without<ChildOf>)>();
        let modal_z = overlay.single(app.world()).unwrap().map_or(0, |z| z.0);
        assert!(
            modal_z > max_z,
            "modal z={modal_z} must exceed options z={max_z}"
        );
        let mut button = app
            .world_mut()
            .query_filtered::<(&MenuButton, &ChildOf), With<ErrorModalUi>>();
        let (button, parent) = button.single(app.world()).unwrap();
        assert!(matches!(button.action, Action::DismissError));
        assert!(app.world().get::<ErrorModalUi>(parent.parent()).is_some());
    }

    #[test]
    fn error_modal_blocks_real_options_mouse_click() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::GameOptions);
        app.update();
        let mut q = app
            .world_mut()
            .query_filtered::<(Entity, &MenuButton), With<OptionsUiRoot>>();
        let button = q
            .iter(app.world())
            .find(|(_, b)| matches!(b.action, Action::ToggleVsync))
            .unwrap()
            .0;
        let before = app.world().resource::<crate::settings::Settings>().vsync;
        let mut node = ComputedNode::default();
        node.size = Vec2::new(200.0, 40.0);
        app.world_mut()
            .entity_mut(button)
            .insert((node, UiGlobalTransform::from_xy(100.0, 100.0)));
        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .unwrap()
            .set_cursor_position(Some(Vec2::new(100.0, 100.0)));
        app.world_mut().resource_mut::<SessionError>().0 = Some("error".into());
        app.update();
        for state in [ButtonState::Pressed, ButtonState::Released] {
            app.world_mut()
                .resource_mut::<Messages<MouseButtonInput>>()
                .write(MouseButtonInput {
                    button: MouseButton::Left,
                    state,
                    window: Entity::PLACEHOLDER,
                });
            app.update();
        }
        assert_eq!(
            app.world().resource::<crate::settings::Settings>().vsync,
            before
        );
        assert!(app.world().resource::<SessionError>().0.is_some());
    }

    #[test]
    fn options_button_accepts_click_at_real_center() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::GameOptions);
        app.update();
        let mut q = app
            .world_mut()
            .query_filtered::<(Entity, &MenuButton, &Node), With<OptionsUiRoot>>();
        let (entity, _, node) = q
            .iter(app.world())
            .find(|(_, b, _)| matches!(b.action, Action::ToggleVsync))
            .unwrap();
        let (Val::Px(left), Val::Px(top), Val::Px(width), Val::Px(height)) =
            (node.left, node.top, node.width, node.height)
        else {
            panic!("expected pixel bounds")
        };
        // Display tab's left column is centered 180px left of the window center
        // (col_w 320 / 2 + 20 gap) at 1280px wide, not screen-centered.
        assert!(
            left >= 0.0 && left + width <= 1280.0,
            "button outside viewport: {left}..{}",
            left + width
        );
        assert_eq!(left + width / 2.0, 460.0);
        let center = Vec2::new(left + width / 2.0, top + height / 2.0);
        let mut computed = ComputedNode::default();
        computed.size = Vec2::new(width, height);
        app.world_mut()
            .entity_mut(entity)
            .insert((computed, UiGlobalTransform::from_xy(center.x, center.y)));
        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .unwrap()
            .set_cursor_position(Some(center));
        let before = app.world().resource::<crate::settings::Settings>().vsync;
        for state in [ButtonState::Pressed, ButtonState::Released] {
            app.world_mut()
                .resource_mut::<Messages<MouseButtonInput>>()
                .write(MouseButtonInput {
                    button: MouseButton::Left,
                    state,
                    window: Entity::PLACEHOLDER,
                });
            app.update();
        }
        assert_ne!(
            app.world().resource::<crate::settings::Settings>().vsync,
            before
        );
    }

    #[test]
    fn options_spawned_before_resize_recenter_and_accept_click() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::window::{Monitor, PrimaryMonitor};
        let mut app = base_test_app();
        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .unwrap()
            .resolution
            .set(1920.0, 1080.0);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app.world_mut().resource_mut::<GameMenu>().panel = Some(GamePanel::Options);
        *app.world_mut().resource_mut::<OptionsHost>() = OptionsHost::GameMenu;
        app.world_mut()
            .run_system_once(
                |mut commands: Commands,
                 settings: Res<crate::settings::Settings>,
                 windows: Query<&Window, With<PrimaryWindow>>,
                 primary: Query<&Monitor, With<PrimaryMonitor>>,
                 monitors: Query<&Monitor>| {
                    setup_options(
                        &mut commands,
                        &settings,
                        Some(OptionsHost::GameMenu),
                        &windows,
                        &primary,
                        &monitors,
                    );
                },
            )
            .unwrap();
        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .unwrap()
            .resolution
            .set(1280.0, 720.0);
        {
            let mut settings = app.world_mut().resource_mut::<crate::settings::Settings>();
            settings.width = 1280;
            settings.height = 720;
        }
        app.update();
        app.update(); // Ensure a skipped Added root does not get corrected later.
        let mut roots = app
            .world_mut()
            .query_filtered::<&Node, With<OptionsUiRoot>>();
        let nodes: Vec<_> = roots.iter(app.world()).collect();
        assert!(!nodes.is_empty());
        for node in nodes {
            if let (Val::Px(left), Val::Px(width)) = (node.left, node.width) {
                assert!(
                    left >= 0.0 && left + width <= 1280.0,
                    "root outside viewport: {left}..{}",
                    left + width
                );
            }
        }
        let mut q = app
            .world_mut()
            .query_filtered::<(Entity, &MenuButton, &Node), With<OptionsUiRoot>>();
        let (entity, _, node) = q
            .iter(app.world())
            .find(|(_, b, _)| matches!(b.action, Action::ToggleVsync))
            .unwrap();
        let (Val::Px(left), Val::Px(top), Val::Px(width), Val::Px(height)) =
            (node.left, node.top, node.width, node.height)
        else {
            panic!("expected pixel bounds")
        };
        // The tabbed layout puts Vsync in the Display tab's left column, which
        // recenters to 180px left of the 1280px window center.
        let center = Vec2::new(left + width / 2.0, top + height / 2.0);
        assert!(
            left >= 0.0 && left + width <= 1280.0,
            "button outside viewport: {left}..{}",
            left + width
        );
        assert_eq!(center.x, 460.0);
        let mut computed = ComputedNode::default();
        computed.size = Vec2::new(width, height);
        app.world_mut()
            .entity_mut(entity)
            .insert((computed, UiGlobalTransform::from_xy(center.x, center.y)));
        app.world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
            .unwrap()
            .set_cursor_position(Some(center));
        let before = app.world().resource::<crate::settings::Settings>().vsync;
        for state in [ButtonState::Pressed, ButtonState::Released] {
            app.world_mut()
                .resource_mut::<Messages<MouseButtonInput>>()
                .write(MouseButtonInput {
                    button: MouseButton::Left,
                    state,
                    window: Entity::PLACEHOLDER,
                });
            app.update();
        }
        assert_ne!(
            app.world().resource::<crate::settings::Settings>().vsync,
            before
        );
        app.update();
        let mut q = app
            .world_mut()
            .query_filtered::<(&MenuButton, &Node), With<OptionsUiRoot>>();
        let (_, node) = q
            .iter(app.world())
            .find(|(b, _)| matches!(b.action, Action::ToggleVsync))
            .unwrap();
        let (Val::Px(left), Val::Px(width)) = (node.left, node.width) else {
            panic!("expected pixel bounds")
        };
        assert_eq!(
            left + width / 2.0,
            460.0,
            "rebuilt button must not be shifted twice"
        );
    }

    #[test]
    fn esc_opens_and_closes_overlay() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        let esc_down = KeyboardInput {
            logical_key: Key::Escape,
            key_code: KeyCode::Escape,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };
        let esc_up = KeyboardInput {
            logical_key: Key::Escape,
            key_code: KeyCode::Escape,
            state: bevy::input::ButtonState::Released,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };

        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(esc_down.clone());
        app.update();

        let gm = app.world().resource::<GameMenu>();
        assert_eq!(gm.panel, Some(GamePanel::Root));
        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(opts.grab_mode, CursorGrabMode::None);

        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(esc_up);
        app.update();

        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(esc_down);
        app.update();

        let gm = app.world().resource::<GameMenu>();
        assert_eq!(gm.panel, None);
        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(opts.grab_mode, CursorGrabMode::Locked);
    }

    #[test]
    fn overlay_blocks_player_input_but_server_ticking() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        let initial_pos = [10.0, 20.0, 30.0];
        let p = Player::new(initial_pos);
        app.world_mut().insert_resource(LocalPlayer(p));
        app.world_mut().insert_resource(SampledInput::default());
        app.world_mut()
            .insert_resource(crate::view::camera::CameraMode::First);
        app.world_mut().insert_resource(ChunkStream::default());
        app.world_mut().insert_resource(NetState::default());
        app.world_mut().insert_resource(ClientClock::default());
        app.world_mut().spawn(crate::view::camera::FlyCamera {
            yaw: 0.0,
            pitch: 0.0,
        });

        // System representing player movement gated by menu_closed
        app.add_systems(
            Update,
            (|keys: Res<ButtonInput<KeyCode>>, mut p: ResMut<LocalPlayer>| {
                if keys.pressed(KeyCode::KeyW) {
                    p.0.body.pos[0] += 1.0;
                }
            })
            .run_if(menu_closed),
        );

        app.world_mut().resource_mut::<GameMenu>().panel = Some(GamePanel::Root);

        // Inject movement key
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);

        // Run updates
        for _ in 0..5 {
            app.update();
            app.world_mut().resource_mut::<ClientClock>().tick += 1;
        }

        let player = app.world().resource::<LocalPlayer>();
        assert_eq!(player.0.body.pos, initial_pos);
        let clock = app.world().resource::<ClientClock>();
        assert_eq!(clock.tick, 5);
    }

    #[test]
    fn resume_and_back_restore_gameplay() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        app.world_mut().resource_mut::<GameMenu>().panel = Some(GamePanel::Root);
        if let Ok(mut opts) = app
            .world_mut()
            .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
        {
            opts.grab_mode = CursorGrabMode::None;
        }

        // Action::Resume -> panel None + cursor re-grabbed
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::Resume);
        app.update();

        assert_eq!(app.world().resource::<GameMenu>().panel, None);
        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(opts.grab_mode, CursorGrabMode::Locked);

        // GameOptions -> panel Some(Options) with OptionsHost::GameMenu
        app.world_mut().resource_mut::<GameMenu>().panel = Some(GamePanel::Root);
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::GameOptions);
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Options)
        );
        assert_eq!(
            *app.world().resource::<OptionsHost>(),
            OptionsHost::GameMenu
        );

        // Back from options -> panel Some(Root), not title
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::Back);
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Root)
        );
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );
    }

    #[test]
    fn save_quit_opens_confirmation_dialog_before_exit() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        open_pause_menu(&mut app);

        // Save & Quit opens the confirmation panel and does not leave InGame.
        activate_button(&mut app, Action::SaveQuit);
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::ConfirmQuit)
        );
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame
        );

        let texts: Vec<String> = app
            .world_mut()
            .query::<&MenuText>()
            .iter(app.world())
            .map(|t| t.text.clone())
            .collect();
        assert!(
            texts
                .iter()
                .any(|t| t.contains("Save and return to Title Screen?")),
            "confirmation prompt missing, found: {texts:?}"
        );
        let mut buttons = app.world_mut().query::<&MenuButton>();
        let actions: Vec<Action> = buttons
            .iter(app.world())
            .map(|b| b.action.clone())
            .collect();
        assert!(actions.contains(&Action::ConfirmQuit));
        assert!(actions.contains(&Action::CancelQuit));

        // Confirm & Exit transitions back to the title screen.
        activate_button(&mut app, Action::ConfirmQuit);
        app.update();
        assert!(matches!(
            app.world().resource::<NextState<AppState>>(),
            NextState::Pending(AppState::Menu)
        ));
        app.update();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Menu
        );
    }

    #[test]
    fn cancel_quit_returns_to_pause_menu() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        open_pause_menu(&mut app);

        activate_button(&mut app, Action::SaveQuit);
        app.update();
        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::ConfirmQuit)
        );

        activate_button(&mut app, Action::CancelQuit);
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Root)
        );
        let mut confirm = app.world_mut().query::<&ConfirmQuitUi>();
        assert!(
            confirm.iter(app.world()).next().is_none(),
            "confirmation UI must be despawned on cancel"
        );
    }

    #[test]
    fn escape_from_confirm_quit_returns_to_root() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app.world_mut().resource_mut::<GameMenu>().panel = Some(GamePanel::ConfirmQuit);
        app.update();

        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(KeyboardInput {
                logical_key: Key::Escape,
                key_code: KeyCode::Escape,
                state: bevy::input::ButtonState::Pressed,
                window: Entity::PLACEHOLDER,
                repeat: false,
                text: None,
            });
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Root)
        );
    }

    #[test]
    fn saving_overlay_shown_in_menu_and_blocks_navigation() {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let _ = rx.recv();
        });

        let mut app = base_test_app();
        app.world_mut()
            .insert_resource(SessionStopping::new(handle));

        // Spawn a button in Menu
        app.world_mut().spawn((
            Node::default(),
            MenuButton {
                action: Action::Quit,
                rank: 0,
                disabled: false,
            },
        ));
        app.update();

        // SavingOverlayUi is present with text "Saving world..."
        let saving_query = app
            .world_mut()
            .query_filtered::<Entity, With<SavingOverlayUi>>()
            .iter(app.world())
            .next();
        assert!(saving_query.is_some(), "SavingOverlayUi must be present");

        let texts: Vec<String> = app
            .world_mut()
            .query::<&MenuText>()
            .iter(app.world())
            .map(|t| t.text.clone())
            .collect();
        assert!(texts.iter().any(|t| t.contains("Saving world")));

        // Attempt keyboard activation (Enter) while stopping exists -> Action::Quit must NOT be emitted
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();

        let actions = app.world().resource::<Messages<Action>>();
        let mut cursor = actions.get_cursor();
        assert!(
            cursor.read(actions).next().is_none(),
            "Actions must not be emitted while SessionStopping is active"
        );

        // Allow stopping thread to finish and remove SessionStopping
        let _ = tx.send(());
        app.world_mut().remove_resource::<SessionStopping>();
        app.update();

        let saving_query_after = app
            .world_mut()
            .query_filtered::<Entity, With<SavingOverlayUi>>()
            .iter(app.world())
            .next();
        assert!(
            saving_query_after.is_none(),
            "SavingOverlayUi must be removed after stopping finishes"
        );
    }

    #[test]
    fn esc_does_nothing_when_error_modal_is_up() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        app.world_mut()
            .insert_resource(SessionError(Some("fatal connection error".into())));
        app.update();

        // Inject Esc
        let esc = KeyboardInput {
            logical_key: Key::Escape,
            key_code: KeyCode::Escape,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(esc);
        app.update();

        // GameMenu panel remains None, error modal stays
        assert_eq!(app.world().resource::<GameMenu>().panel, None);
        assert_eq!(
            app.world().resource::<SessionError>().0,
            Some("fatal connection error".into())
        );
        let error_query = app
            .world_mut()
            .query_filtered::<Entity, With<ErrorModalUi>>()
            .iter(app.world())
            .next();
        assert!(error_query.is_some(), "ErrorModalUi must stay open");
    }

    #[test]
    fn mouse_click_while_overlay_open_does_not_regrab_cursor() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        // Open menu
        app.world_mut().resource_mut::<GameMenu>().panel = Some(GamePanel::Root);
        if let Ok(mut opts) = app
            .world_mut()
            .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
        {
            opts.grab_mode = CursorGrabMode::None;
        }

        // Add camera toggle_grab system from view::camera
        app.add_systems(Update, crate::view::camera::look);

        // Inject mouse left click
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            opts.grab_mode,
            CursorGrabMode::None,
            "cursor must not be re-grabbed while overlay is open"
        );
    }

    #[test]
    fn esc_in_options_panel_returns_to_root() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        // Put in Options panel
        app.world_mut().resource_mut::<GameMenu>().panel = Some(GamePanel::Options);
        *app.world_mut().resource_mut::<OptionsHost>() = OptionsHost::GameMenu;
        app.update();

        // Press Esc
        let esc = KeyboardInput {
            logical_key: Key::Escape,
            key_code: KeyCode::Escape,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(esc);
        app.update();

        // GameMenu panel is Some(Root)
        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Root)
        );
        let root_query = app
            .world_mut()
            .query_filtered::<Entity, With<GameRootUi>>()
            .iter(app.world())
            .next();
        assert!(root_query.is_some(), "GameRootUi must be recreated");
    }

    #[test]
    fn error_modal_return_to_title_clears_session() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        app.world_mut()
            .insert_resource(LocalPlayer(Player::new([0.0, 0.0, 0.0])));
        let handle = std::thread::spawn(|| {});
        app.world_mut().insert_resource(SessionHandle::new(handle));
        app.world_mut()
            .insert_resource(SessionError(Some("boom".into())));
        app.update();

        // Modal text shows "boom"
        let texts: Vec<String> = app
            .world_mut()
            .query::<&MenuText>()
            .iter(app.world())
            .map(|t| t.text.clone())
            .collect();
        assert!(
            texts.iter().any(|t| t.contains("boom")),
            "modal text must contain error message 'boom', found: {:?}",
            texts
        );

        // DismissError -> state Menu, SessionError(None), no Net/LocalPlayer
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::DismissError);
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Menu
        );
        assert_eq!(app.world().resource::<SessionError>().0, None);
        assert!(!app.world().contains_resource::<LocalPlayer>());
    }

    #[test]
    fn loading_card_hides_when_ready() {
        let mut app = base_test_app();
        app.world_mut().insert_resource(SelectedWorld {
            name: "Alpha".into(),
            seed: 42,
            persist: false,
            ..Default::default()
        });
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.world_mut().insert_resource(NetState::default()); // ready is false
        app.update();

        // Card visible with name/seed while !is_ready
        let loading_query = app
            .world_mut()
            .query_filtered::<Entity, With<LoadingCardUi>>()
            .iter(app.world())
            .next();
        assert!(loading_query.is_some(), "LoadingCardUi must be present");

        let texts: Vec<String> = app
            .world_mut()
            .query::<&MenuText>()
            .iter(app.world())
            .map(|t| t.text.clone())
            .collect();
        assert!(texts.iter().any(|t| t.contains("Alpha")));
        assert!(texts.iter().any(|t| t.contains("42")));

        // Flip ready
        app.world_mut().resource_mut::<NetState>().ready = true;
        app.update();

        let loading_query_after = app
            .world_mut()
            .query_filtered::<Entity, With<LoadingCardUi>>()
            .iter(app.world())
            .next();
        assert!(
            loading_query_after.is_none(),
            "LoadingCardUi must be gone once ready"
        );
    }

    #[test]
    fn esc_in_menu_state_does_not_open_game_menu_or_lock_cursor() {
        let mut app = base_test_app();
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Menu
        );

        if let Ok(mut opts) = app
            .world_mut()
            .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
        {
            opts.grab_mode = CursorGrabMode::None;
            opts.visible = true;
        }

        let esc_down = KeyboardInput {
            logical_key: Key::Escape,
            key_code: KeyCode::Escape,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };
        let esc_up = KeyboardInput {
            logical_key: Key::Escape,
            key_code: KeyCode::Escape,
            state: bevy::input::ButtonState::Released,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };

        // First Esc press in Menu
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(esc_down.clone());
        app.update();

        assert_eq!(app.world().resource::<GameMenu>().panel, None);
        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(opts.grab_mode, CursorGrabMode::None);
        assert!(opts.visible);

        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(esc_up);
        app.update();

        // Second Esc press in Menu
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(esc_down);
        app.update();

        assert_eq!(app.world().resource::<GameMenu>().panel, None);
        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(opts.grab_mode, CursorGrabMode::None);
        assert!(opts.visible);
    }

    #[test]
    fn error_in_ingame_releases_cursor_and_focuses_return_to_title() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        if let Ok(mut opts) = app
            .world_mut()
            .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
        {
            opts.grab_mode = CursorGrabMode::Locked;
            opts.visible = false;
        }
        app.world_mut().resource_mut::<MenuFocus>().rank = None;

        app.world_mut().resource_mut::<SessionError>().0 = Some("server lost connection".into());
        app.update();

        let opts = app
            .world_mut()
            .query_filtered::<&CursorOptions, With<PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(opts.grab_mode, CursorGrabMode::None);
        assert!(opts.visible);

        assert_eq!(app.world().resource::<MenuFocus>().rank, Some(0));

        let enter_down = KeyboardInput {
            logical_key: Key::Enter,
            key_code: KeyCode::Enter,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(enter_down);
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Menu
        );
        assert_eq!(app.world().resource::<SessionError>().0, None);
    }

    #[test]
    fn error_modal_restricts_navigation_and_ignores_background_buttons() {
        let mut app = base_test_app();
        app.world_mut().spawn((
            Node::default(),
            MenuButton {
                action: Action::OpenScreen(Screen::Worlds),
                rank: 0,
                disabled: false,
            },
        ));
        app.world_mut().spawn((
            Node::default(),
            MenuButton {
                action: Action::OpenScreen(Screen::Options),
                rank: 1,
                disabled: false,
            },
        ));

        app.world_mut().resource_mut::<SessionError>().0 = Some("fatal error".into());
        app.update();

        assert_eq!(app.world().resource::<MenuFocus>().rank, Some(0));

        let tab_down = KeyboardInput {
            logical_key: Key::Tab,
            key_code: KeyCode::Tab,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(tab_down);
        app.update();

        assert_eq!(app.world().resource::<MenuFocus>().rank, Some(0));

        let enter_down = KeyboardInput {
            logical_key: Key::Enter,
            key_code: KeyCode::Enter,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(enter_down);
        app.update();
        app.update();

        assert_eq!(app.world().resource::<SessionError>().0, None);
        if let Some(screen) = app.world().get_resource::<State<MenuScreen>>() {
            assert_ne!(*screen.get(), MenuScreen::Worlds);
        }
    }

    #[test]
    fn error_modal_mouse_click_ignores_background_buttons_and_activates_modal_button() {
        let mut app = base_test_app();
        // Base test app already initializes WindowPlugin and has primary window
        // Set cursor position on primary window
        if let Ok(mut win) = app
            .world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
        {
            win.set_cursor_position(Some(Vec2::new(100.0, 100.0)));
        }

        // Spawn background button at (100, 100)
        let mut bg_node = ComputedNode::default();
        bg_node.size = Vec2::new(200.0, 40.0);
        app.world_mut().spawn((
            MenuButton {
                action: Action::OpenScreen(Screen::Worlds),
                rank: 0,
                disabled: false,
            },
            bg_node,
            UiGlobalTransform::from_xy(100.0, 100.0),
        ));

        // Introduce SessionError
        app.world_mut().resource_mut::<SessionError>().0 = Some("fatal mouse error".into());
        app.update();

        // Position error modal button at (400, 400)
        let modal_btn_ent = app
            .world_mut()
            .query_filtered::<Entity, (With<MenuButton>, With<ErrorModalUi>)>()
            .single(app.world())
            .unwrap();
        let mut modal_node = ComputedNode::default();
        modal_node.size = Vec2::new(BUTTON_WIDTH, BUTTON_HEIGHT);
        app.world_mut()
            .entity_mut(modal_btn_ent)
            .insert((modal_node, UiGlobalTransform::from_xy(400.0, 400.0)));

        let mouse_down = MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        };
        let mouse_up = MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Released,
            window: Entity::PLACEHOLDER,
        };

        // Click on background button position (100, 100)
        app.world_mut()
            .resource_mut::<Messages<MouseButtonInput>>()
            .write(mouse_down.clone());
        app.update();
        app.world_mut()
            .resource_mut::<Messages<MouseButtonInput>>()
            .write(mouse_up.clone());
        app.update();

        // Background button action must NOT have been executed
        assert_eq!(
            app.world().resource::<SessionError>().0,
            Some("fatal mouse error".into())
        );
        if let Some(screen) = app.world().get_resource::<State<MenuScreen>>() {
            assert_ne!(*screen.get(), MenuScreen::Worlds);
        }

        // Move cursor to error modal button position (400, 400)
        if let Ok(mut win) = app
            .world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(app.world_mut())
        {
            win.set_cursor_position(Some(Vec2::new(400.0, 400.0)));
        }

        // Click and release on modal button
        app.world_mut()
            .resource_mut::<Messages<MouseButtonInput>>()
            .write(mouse_down);
        app.update();
        app.world_mut()
            .resource_mut::<Messages<MouseButtonInput>>()
            .write(mouse_up);
        app.update();
        app.update();

        // Action::DismissError must have executed: session error cleared, in Menu
        assert_eq!(app.world().resource::<SessionError>().0, None);
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::Menu
        );
    }

    #[test]
    fn error_modal_button_is_child_of_card_and_aligned() {
        let mut app = base_test_app();
        app.world_mut().resource_mut::<SessionError>().0 = Some("test layout error".into());
        app.update();

        let mut overlay_query = app
            .world_mut()
            .query_filtered::<(Entity, &Children, &Node), With<ErrorModalUi>>();
        let (_overlay_ent, overlay_children, overlay_node) = overlay_query
            .iter(app.world())
            .find(|(_, _, n)| n.width == Val::Percent(100.0) && n.height == Val::Percent(100.0))
            .expect("overlay must exist with 100% dimensions");

        assert_eq!(overlay_node.justify_content, JustifyContent::Center);
        assert_eq!(overlay_node.align_items, AlignItems::Center);

        let card_ent = overlay_children[0];
        let card_node = app.world().get::<Node>(card_ent).expect("card node");
        assert_eq!(card_node.flex_direction, FlexDirection::Column);
        assert_eq!(card_node.align_items, AlignItems::Center);

        let card_children: Vec<Entity> = app
            .world()
            .get::<Children>(card_ent)
            .expect("card children")
            .iter()
            .collect();
        let mut btn_query = app.world_mut().query::<(&MenuButton, &Node)>();
        let mut found_btn = false;
        for &child in &card_children {
            if let Ok((b, node)) = btn_query.get(app.world(), child) {
                if b.action == Action::DismissError {
                    found_btn = true;
                    assert_eq!(node.position_type, PositionType::Relative);
                    assert_eq!(node.width, Val::Px(BUTTON_WIDTH));
                    assert_eq!(node.height, Val::Px(BUTTON_HEIGHT));
                }
            }
        }
        assert!(
            found_btn,
            "Return to title button must be a child of the card"
        );

        // Verify layout configuration holds across multiple window resolutions
        for (w, h) in [(1920.0, 1080.0), (800.0, 600.0), (2560.0, 1440.0)] {
            if let Ok(mut win) = app
                .world_mut()
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .single_mut(app.world_mut())
            {
                win.resolution.set(w, h);
            }
            app.update();

            let (_, _, overlay_node) = overlay_query
                .iter(app.world())
                .find(|(_, _, n)| n.width == Val::Percent(100.0) && n.height == Val::Percent(100.0))
                .unwrap();
            assert_eq!(overlay_node.justify_content, JustifyContent::Center);
            assert_eq!(overlay_node.align_items, AlignItems::Center);

            let card_node = app.world().get::<Node>(card_ent).unwrap();
            assert_eq!(card_node.flex_direction, FlexDirection::Column);
            assert_eq!(card_node.align_items, AlignItems::Center);
            assert_eq!(card_node.width, Val::Px(560.0));

            for &child in &card_children {
                if let Ok((b, node)) = btn_query.get(app.world(), child) {
                    if b.action == Action::DismissError {
                        assert_eq!(node.position_type, PositionType::Relative);
                        assert_eq!(node.width, Val::Px(BUTTON_WIDTH));
                        assert_eq!(node.height, Val::Px(BUTTON_HEIGHT));
                    }
                }
            }
        }
    }

    #[test]
    fn error_modal_updates_message_when_error_changes() {
        let mut app = base_test_app();
        app.world_mut().resource_mut::<SessionError>().0 = Some("error message alpha".into());
        app.update();

        let texts: Vec<String> = app
            .world_mut()
            .query::<&MenuText>()
            .iter(app.world())
            .map(|t| t.text.clone())
            .collect();
        assert!(texts.iter().any(|t| t == "error message alpha"));

        app.world_mut().resource_mut::<SessionError>().0 = Some("error message beta".into());
        app.update();

        let texts_after: Vec<String> = app
            .world_mut()
            .query::<&MenuText>()
            .iter(app.world())
            .map(|t| t.text.clone())
            .collect();
        assert!(
            texts_after.iter().any(|t| t == "error message beta"),
            "modal must display updated error message 'error message beta', found: {:?}",
            texts_after
        );
        assert!(
            !texts_after.iter().any(|t| t == "error message alpha"),
            "old error message must not be present"
        );
    }

    fn test_game_menu_app() -> App {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app
    }

    fn find_buttons_with_action(app: &mut App, action: Action) -> Vec<Entity> {
        app.world_mut()
            .query::<(Entity, &MenuButton)>()
            .iter(app.world())
            .filter(|(_, button)| button.action == action)
            .map(|(entity, _)| entity)
            .collect()
    }

    fn collect_menu_texts(app: &mut App) -> Vec<String> {
        app.world_mut()
            .query::<&MenuText>()
            .iter(app.world())
            .map(|text| text.text.clone())
            .collect()
    }

    #[test]
    fn pause_menu_opens_controls_reference_screen() {
        let mut app = test_game_menu_app();
        open_pause_menu(&mut app);

        assert_eq!(
            find_buttons_with_action(&mut app, Action::GameControls).len(),
            1,
            "pause menu must have exactly one Controls button"
        );

        activate_button(&mut app, Action::GameControls);
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Controls)
        );
        assert!(
            app.world_mut()
                .query::<&ControlsUiRoot>()
                .iter(app.world())
                .next()
                .is_some(),
            "ControlsUiRoot must be spawned"
        );

        let texts = collect_menu_texts(&mut app);
        assert!(texts.iter().any(|t| t.contains("WASD")));
        assert!(texts.iter().any(|t| t.contains("Space")));
        for line in [
            "Inventory: Esc",
            "Minimap: Tab",
            "Fullscreen Map: M",
            "Quick Heal: H",
            "Quick Mana: J",
            "Quick Buff: B",
            "Equip Armor: E",
            "Break Wall: X",
            "Pause: Esc",
        ] {
            assert!(
                texts.iter().any(|t| t == line),
                "controls reference must list {line:?}, found: {texts:?}"
            );
        }
        assert!(
            !texts.iter().any(|t| t == "Housing / Guide: H"),
            "Housing / Guide: H is removed (Housing lives in Inventory)"
        );
        assert!(
            !texts.iter().any(|t| t == "Inventory: Esc/Tab/E"),
            "Inventory: Esc/Tab/E is not a real binding"
        );

        activate_button(&mut app, Action::ResumeRoot);
        app.update();
        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Root)
        );
    }

    #[test]
    fn escape_from_controls_returns_to_root() {
        let mut app = test_game_menu_app();
        open_pause_menu(&mut app);

        activate_button(&mut app, Action::GameControls);
        app.update();
        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Controls)
        );

        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(KeyboardInput {
                logical_key: Key::Escape,
                key_code: KeyCode::Escape,
                state: bevy::input::ButtonState::Pressed,
                window: Entity::PLACEHOLDER,
                repeat: false,
                text: None,
            });
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Root)
        );
        assert!(
            app.world_mut()
                .query::<&ControlsUiRoot>()
                .iter(app.world())
                .next()
                .is_none(),
            "controls overlay must despawn when returning to root"
        );
        assert!(
            app.world_mut()
                .query::<&GameRootUi>()
                .iter(app.world())
                .next()
                .is_some(),
            "root pause menu must be restored"
        );
    }

    #[test]
    fn controls_panel_focuses_back_so_enter_works_without_arrows() {
        let mut app = test_game_menu_app();
        open_pause_menu(&mut app);
        app.world_mut().resource_mut::<MenuFocus>().rank = Some(3);

        activate_button(&mut app, Action::GameControls);
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Controls)
        );
        assert_eq!(app.world().resource::<MenuFocus>().rank, Some(0));

        press_enter(&mut app);
        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Root),
            "Enter on the Controls panel must activate Back"
        );
        assert!(
            app.world_mut()
                .query::<&GameRootUi>()
                .iter(app.world())
                .next()
                .is_some(),
            "root pause menu must be restored after Enter on Back"
        );
    }

    #[test]
    fn confirm_quit_focuses_cancel_so_enter_does_not_exit() {
        let mut app = test_game_menu_app();
        open_pause_menu(&mut app);
        app.world_mut().resource_mut::<MenuFocus>().rank = Some(0);

        activate_button(&mut app, Action::SaveQuit);
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::ConfirmQuit)
        );
        assert_eq!(app.world().resource::<MenuFocus>().rank, Some(1));

        press_enter(&mut app);
        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Root),
            "Enter on the confirmation must activate Cancel"
        );
        assert_eq!(
            *app.world().resource::<State<AppState>>().get(),
            AppState::InGame,
            "Enter must not save and exit from the confirmation"
        );
    }

    #[test]
    fn escape_from_confirm_quit_restores_resume_focus() {
        let mut app = test_game_menu_app();
        open_pause_menu(&mut app);

        activate_button(&mut app, Action::SaveQuit);
        app.update();
        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::ConfirmQuit)
        );

        press_escape(&mut app);
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            Some(GamePanel::Root)
        );
        assert_eq!(app.world().resource::<MenuFocus>().rank, Some(0));

        press_enter(&mut app);
        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            None,
            "Enter after Escape must activate Resume"
        );
    }

    #[test]
    fn pause_menu_displays_guide_advice() {
        let mut app = base_test_app();
        app.insert_resource(GuideAdviceState {
            current_advice: "You should build a shelter before monsters arrive at night.".into(),
        });
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app.world_mut().resource_mut::<GameMenu>().panel = Some(GamePanel::Root);
        app.update();

        let mut q = app
            .world_mut()
            .query_filtered::<&MenuText, With<GameRootUi>>();
        let texts = q
            .iter(app.world())
            .map(|t| t.text.clone())
            .collect::<Vec<_>>();

        assert!(
            texts
                .iter()
                .any(|t| t.contains("You should build a shelter")),
            "pause menu must display Guide advice, got: {:?}",
            texts
        );
    }

    #[test]
    fn disconnect_modal_shows_retry_and_back_to_menu() {
        let mut app = base_test_app();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();

        let reason = "Connection to server lost (server disconnected).".to_string();
        app.world_mut()
            .insert_resource(crate::session::DisconnectState {
                reason: reason.clone(),
            });
        app.world_mut().resource_mut::<SessionError>().0 = Some(reason.clone());
        app.update();

        let buttons: Vec<Action> = app
            .world_mut()
            .query_filtered::<&MenuButton, With<ErrorModalUi>>()
            .iter(app.world())
            .map(|b| b.action.clone())
            .collect();
        assert!(
            buttons.contains(&Action::RetryConnection),
            "disconnect modal must have Retry button, got {buttons:?}"
        );
        assert!(
            buttons.contains(&Action::DismissError),
            "disconnect modal must have Back to Menu button, got {buttons:?}"
        );

        let texts: Vec<String> = app
            .world_mut()
            .query_filtered::<&MenuText, With<ErrorModalUi>>()
            .iter(app.world())
            .map(|t| t.text.clone())
            .collect();
        assert!(texts.contains(&"Disconnected".to_string()));
        assert!(texts.contains(&"Retry".to_string()));
        assert!(texts.contains(&"Back to Menu".to_string()));
        assert!(texts.contains(&reason));

        // Sending RetryConnection clears DisconnectState and SessionError and schedules retry.
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::RetryConnection);
        app.update();

        assert!(
            app.world()
                .get_resource::<crate::session::DisconnectState>()
                .is_none()
        );
        assert_eq!(app.world().resource::<SessionError>().0, None);
        assert!(
            app.world()
                .get_resource::<crate::session::SessionRetryPending>()
                .is_some()
        );
    }
}
