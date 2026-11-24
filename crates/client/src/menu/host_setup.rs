//! Host Setup menu screen: max players (2..=16), optional password, Public/Private visibility.

use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};

use super::style::{
    BORDER_W, BUTTON_HEIGHT, COLOR_BACKDROP, COLOR_HEADER_DIVIDER, COLOR_INPUT_ACTIVE_BORDER,
    COLOR_INPUT_BG, COLOR_PANEL_BG, COLOR_PANEL_BORDER, COLOR_TEXT_GOLD, COLOR_TEXT_MUTED,
    RADIUS_BTN, RADIUS_PANEL, spawn_button_sized, viewport_size,
};
use super::text::{MenuText, MenuTextColor};
use super::{Action, MenuFocus};
use crate::session::{AppState, MenuScreen, SelectedWorld, SessionMode};

pub const MIN_PLAYERS: u8 = 2;
pub const MAX_PLAYERS: u8 = 16;
pub const DEFAULT_MAX_PLAYERS: u8 = 8;
pub const MAX_PASSWORD_CHARS: usize = 32;

/// Configuration state for hosting a multiplayer session.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct HostSetupState {
    pub max_players: u8,
    pub password: String,
    pub public: bool,
}

impl Default for HostSetupState {
    fn default() -> Self {
        Self {
            max_players: DEFAULT_MAX_PLAYERS,
            password: String::new(),
            public: true,
        }
    }
}

impl HostSetupState {
    pub fn adjust_max_players(&mut self, delta: i32) {
        let next = (self.max_players as i32 + delta)
            .clamp(MIN_PLAYERS as i32, MAX_PLAYERS as i32) as u8;
        self.max_players = next;
    }

    pub fn toggle_public(&mut self) {
        self.public = !self.public;
    }

    pub fn push_password_char(&mut self, ch: char) {
        if (ch.is_ascii_graphic() || ch == ' ')
            && self.password.chars().count() < MAX_PASSWORD_CHARS
        {
            self.password.push(ch);
        }
    }

    pub fn pop_password_char(&mut self) {
        self.password.pop();
    }

    pub fn to_session_mode(&self) -> SessionMode {
        SessionMode::Host {
            max_players: self.max_players.clamp(MIN_PLAYERS, MAX_PLAYERS),
            password: if self.password.is_empty() {
                None
            } else {
                Some(self.password.clone())
            },
            public: self.public,
        }
    }
}

#[derive(Component)]
pub struct MaxPlayersValueText;

#[derive(Component)]
pub struct HostPasswordText;

#[derive(Component)]
pub struct HostVisibilityText;

fn format_password_display(password: &str) -> String {
    if password.is_empty() {
        "> (none)".to_string()
    } else {
        format!("> {}", "*".repeat(password.chars().count()))
    }
}

fn format_visibility_label(public: bool) -> &'static str {
    if public {
        "Visibility: Public"
    } else {
        "Visibility: Private"
    }
}

pub fn setup_host_setup(
    mut commands: Commands,
    mut state: ResMut<HostSetupState>,
    mut focus: ResMut<MenuFocus>,
    selected_world: Option<Res<SelectedWorld>>,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
) {
    *state = HostSetupState::default();
    focus.rank = Some(0);

    let (win_w, win_h) = viewport_size(primary_window.as_ref());
    let center_x = win_w * 0.5;
    let panel_w = 640.0;
    let panel_h = 500.0;
    let panel_top = ((win_h - panel_h) * 0.5).max(24.0);

    let world_name = selected_world
        .as_ref()
        .map(|w| w.name.as_str())
        .filter(|n| !n.is_empty())
        .unwrap_or("World");

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
        DespawnOnExit(MenuScreen::HostSetup),
    ));

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
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            BackgroundColor(COLOR_PANEL_BG),
            BorderColor::all(COLOR_PANEL_BORDER),
            GlobalZIndex(0),
            DespawnOnExit(MenuScreen::HostSetup),
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                },
                MenuText::new("Host Setup", 1.45),
                MenuTextColor(COLOR_TEXT_GOLD),
            ));
            p.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
                MenuText::new(format!("World: {world_name}"), 0.95),
                MenuTextColor(COLOR_TEXT_MUTED),
            ));
            p.spawn((
                Node {
                    width: Val::Px(panel_w - 64.0),
                    height: Val::Px(2.0),
                    margin: UiRect::bottom(Val::Px(16.0)),
                    ..default()
                },
                BackgroundColor(COLOR_HEADER_DIVIDER),
            ));

            // Max Players section
            p.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(14.0)),
                    ..default()
                },
                MenuText::new("Max Players (2 - 16)", 0.95),
            ));
            p.spawn((
                Node {
                    height: Val::Px(38.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    margin: UiRect::bottom(Val::Px(20.0)),
                    ..default()
                },
                MenuText::new(state.max_players.to_string(), 1.2),
                MenuTextColor(COLOR_TEXT_GOLD),
                MaxPlayersValueText,
            ));

            // Optional Password section
            p.spawn((
                Node {
                    width: Val::Px(460.0),
                    margin: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                },
                MenuText::new("Password (optional - type to set)", 0.9),
            ));
            p.spawn((
                Node {
                    width: Val::Px(460.0),
                    height: Val::Px(38.0),
                    border: UiRect::all(Val::Px(BORDER_W)),
                    border_radius: BorderRadius::all(Val::Px(RADIUS_BTN)),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(12.0)),
                    margin: UiRect::bottom(Val::Px(18.0)),
                    ..default()
                },
                BackgroundColor(COLOR_INPUT_BG),
                BorderColor::all(COLOR_INPUT_ACTIVE_BORDER),
            ))
            .with_children(|box_p| {
                box_p.spawn((
                    Node::default(),
                    MenuText::new(format_password_display(&state.password), 1.0),
                    HostPasswordText,
                ));
            });

            p.spawn((
                Node {
                    margin: UiRect::top(Val::Px(58.0)),
                    ..default()
                },
                MenuText::new("Public rooms appear in the server browser.", 0.85),
                MenuTextColor(COLOR_TEXT_MUTED),
            ));
        });

    // Stepper buttons: - and +
    let stepper_y = panel_top + 148.0;
    let b_dec = spawn_button_sized(
        &mut commands,
        "-",
        Action::HostMaxPlayersDelta(-1),
        0,
        (center_x - 110.0, stepper_y),
        (56.0, 38.0),
        false,
    );
    commands
        .entity(b_dec)
        .insert((DespawnOnExit(MenuScreen::HostSetup), GlobalZIndex(1)));

    let b_inc = spawn_button_sized(
        &mut commands,
        "+",
        Action::HostMaxPlayersDelta(1),
        1,
        (center_x + 54.0, stepper_y),
        (56.0, 38.0),
        false,
    );
    commands
        .entity(b_inc)
        .insert((DespawnOnExit(MenuScreen::HostSetup), GlobalZIndex(1)));

    // Visibility toggle button
    let toggle_w = 260.0;
    let b_vis = spawn_button_sized(
        &mut commands,
        format_visibility_label(state.public),
        Action::HostTogglePublic,
        2,
        (center_x - toggle_w * 0.5, panel_top + 296.0),
        (toggle_w, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b_vis)
        .insert((DespawnOnExit(MenuScreen::HostSetup), GlobalZIndex(1)));

    // Footer buttons: Start Hosting & Back
    let foot_w = 220.0;
    let foot_y = panel_top + panel_h - 68.0;
    let b_launch = spawn_button_sized(
        &mut commands,
        "Start Hosting",
        Action::HostLaunch,
        3,
        (center_x - foot_w - 16.0, foot_y),
        (foot_w, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b_launch)
        .insert((DespawnOnExit(MenuScreen::HostSetup), GlobalZIndex(1)));

    let b_back = spawn_button_sized(
        &mut commands,
        "Back",
        Action::Back,
        4,
        (center_x + 16.0, foot_y),
        (foot_w, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b_back)
        .insert((DespawnOnExit(MenuScreen::HostSetup), GlobalZIndex(1)));
}

pub fn handle_host_setup_input(
    keyboard_events: Option<MessageReader<KeyboardInput>>,
    mut state: ResMut<HostSetupState>,
) {
    let Some(mut keyboard_events) = keyboard_events else {
        return;
    };
    for ev in keyboard_events.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        if ev.key_code == KeyCode::Backspace || ev.logical_key == Key::Backspace {
            state.pop_password_char();
            continue;
        }
        if ev.logical_key == Key::Space || ev.key_code == KeyCode::Space {
            state.push_password_char(' ');
            continue;
        }
        if let Key::Character(ref s) = ev.logical_key {
            for ch in s.chars() {
                state.push_password_char(ch);
            }
        }
    }
}

pub fn handle_host_setup_actions(
    actions: Option<MessageReader<Action>>,
    mut state: ResMut<HostSetupState>,
    mut session_mode: Option<ResMut<SessionMode>>,
    mut next_app_state: Option<ResMut<NextState<AppState>>>,
    mut next_screen: Option<ResMut<NextState<MenuScreen>>>,
) {
    let Some(mut actions) = actions else {
        return;
    };
    for action in actions.read() {
        match action {
            Action::HostMaxPlayersDelta(delta) => {
                state.adjust_max_players(*delta);
            }
            Action::HostTogglePublic => {
                state.toggle_public();
            }
            Action::HostLaunch => {
                if let Some(ref mut mode) = session_mode {
                    **mode = state.to_session_mode();
                }
                if let Some(ref mut next) = next_app_state {
                    next.set(AppState::InGame);
                }
            }
            Action::Back => {
                if let Some(ref mut next) = next_screen {
                    next.set(MenuScreen::Worlds);
                }
            }
            _ => {}
        }
    }
}

pub fn sync_host_setup_ui(
    state: Res<HostSetupState>,
    mut max_q: Query<&mut MenuText, (With<MaxPlayersValueText>, Without<HostPasswordText>)>,
    mut pw_q: Query<&mut MenuText, (With<HostPasswordText>, Without<MaxPlayersValueText>)>,
    buttons: Query<(&super::MenuButton, &Children)>,
    mut text_q: Query<
        &mut MenuText,
        (Without<MaxPlayersValueText>, Without<HostPasswordText>),
    >,
) {
    if !state.is_changed() {
        return;
    }
    for mut t in &mut max_q {
        t.text = state.max_players.to_string();
    }
    let pw_str = format_password_display(&state.password);
    for mut t in &mut pw_q {
        t.text = pw_str.clone();
    }
    let vis_label = format_visibility_label(state.public);
    for (btn, children) in &buttons {
        if btn.action == Action::HostTogglePublic {
            for child in children.iter() {
                if let Ok(mut t) = text_q.get_mut(child) {
                    t.text = vis_label.to_string();
                }
            }
        }
    }
}

pub struct HostSetupPlugin;

impl Plugin for HostSetupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HostSetupState>()
            .add_message::<Action>()
            .add_systems(OnEnter(MenuScreen::HostSetup), setup_host_setup)
            .add_systems(
                Update,
                (
                    handle_host_setup_input,
                    handle_host_setup_actions,
                    sync_host_setup_ui,
                )
                    .chain()
                    .run_if(in_state(MenuScreen::HostSetup)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::MenuButton;

    #[test]
    fn host_setup_clamps_max_players_2_to_16_and_launches() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .init_resource::<SessionMode>()
            .init_resource::<SelectedWorld>()
            .add_plugins(HostSetupPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::HostSetup);
        app.update();
        app.update();

        // Verify all expected controls are spawned
        let buttons: Vec<MenuButton> = app
            .world_mut()
            .query::<&MenuButton>()
            .iter(app.world())
            .cloned()
            .collect();
        assert!(buttons.iter().any(|b| b.action == Action::HostMaxPlayersDelta(-1)));
        assert!(buttons.iter().any(|b| b.action == Action::HostMaxPlayersDelta(1)));
        assert!(buttons.iter().any(|b| b.action == Action::HostTogglePublic));
        assert!(buttons.iter().any(|b| b.action == Action::HostLaunch));
        assert!(buttons.iter().any(|b| b.action == Action::Back));

        // Default is 8 and Public
        {
            let st = app.world().resource::<HostSetupState>();
            assert_eq!(st.max_players, 8);
            assert!(st.public);
        }

        // Decrement below 2 clamps at 2
        for _ in 0..10 {
            app.world_mut().write_message(Action::HostMaxPlayersDelta(-1));
        }
        app.update();
        assert_eq!(app.world().resource::<HostSetupState>().max_players, 2);

        // Increment above 16 clamps at 16
        for _ in 0..20 {
            app.world_mut().write_message(Action::HostMaxPlayersDelta(1));
        }
        app.update();
        assert_eq!(app.world().resource::<HostSetupState>().max_players, 16);

        // Toggle visibility to Private and set a password
        app.world_mut().write_message(Action::HostTogglePublic);
        app.world_mut()
            .resource_mut::<HostSetupState>()
            .password = "secret".into();
        app.update();
        assert!(!app.world().resource::<HostSetupState>().public);

        // Launch sets SessionMode::Host and transitions to AppState::InGame
        app.world_mut().write_message(Action::HostLaunch);
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<SessionMode>(),
            SessionMode::Host {
                max_players: 16,
                password: Some("secret".into()),
                public: false,
            }
        );
        assert_eq!(*app.world().resource::<State<AppState>>().get(), AppState::InGame);
    }
}
