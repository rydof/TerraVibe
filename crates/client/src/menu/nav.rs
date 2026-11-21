//! Keyboard, mouse, and directional menu navigation.

use bevy::prelude::*;
use bevy::ui::UiGlobalTransform;

use super::game::ErrorModalUi;
use super::sounds::MenuSoundEvent;
use super::{Action, MenuButton, point_in_rect};
use crate::session::{AppState, GameMenu, MenuScreen, SessionError, SessionStopping};

/// Tracks which button rank currently has focus in the menu.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuFocus {
    pub rank: Option<u16>,
}

/// Computes the next enabled button rank in rank order, wrapping around.
pub fn next_rank(ranks: &[u16], disabled: &[u16], from: u16, dir: i32) -> u16 {
    let mut enabled: Vec<u16> = ranks
        .iter()
        .copied()
        .filter(|r| !disabled.contains(r))
        .collect();
    if enabled.is_empty() {
        return from;
    }
    enabled.sort_unstable();
    enabled.dedup();

    if let Some(pos) = enabled.iter().position(|r| *r == from) {
        let len = enabled.len() as i32;
        let next_pos = ((pos as i32 + dir).rem_euclid(len)) as usize;
        enabled[next_pos]
    } else if dir > 0 {
        enabled
            .iter()
            .copied()
            .find(|&r| r > from)
            .unwrap_or(enabled[0])
    } else {
        enabled
            .iter()
            .copied()
            .rfind(|&r| r < from)
            .unwrap_or(*enabled.last().unwrap())
    }
}

/// Executes an action triggered by keyboard or mouse.
pub fn execute_action(
    action: &Action,
    current_screen: Option<&State<MenuScreen>>,
    next_screen: Option<&mut ResMut<NextState<MenuScreen>>>,
    app_exit: Option<&mut MessageWriter<AppExit>>,
    sound_events: Option<&mut MessageWriter<MenuSoundEvent>>,
    mut action_writer: Option<&mut MessageWriter<Action>>,
) {
    if let Some(ref mut writer) = action_writer {
        writer.write(action.clone());
    }
    match action {
        Action::OpenScreen(screen) => {
            if let Some(events) = sound_events {
                events.write(MenuSoundEvent::Open);
            }
            if let Some(next) = next_screen {
                next.set(screen.to_menu_screen());
            }
        }
        Action::Quit => {
            if let Some(events) = sound_events {
                events.write(MenuSoundEvent::Close);
            }
            if let Some(exit) = app_exit {
                exit.write(AppExit::Success);
            }
        }
        Action::Back => {
            if let Some(events) = sound_events {
                events.write(MenuSoundEvent::Close);
            }
            if let Some(next) = next_screen {
                match current_screen.map(|s| s.get()) {
                    Some(&MenuScreen::CreateWorld) => next.set(MenuScreen::Worlds),
                    Some(&MenuScreen::Worlds) => next.set(MenuScreen::Characters),
                    Some(&MenuScreen::CreateCharacter) => next.set(MenuScreen::Characters),
                    Some(&MenuScreen::Characters) => next.set(MenuScreen::Title),
                    _ => next.set(MenuScreen::Title),
                }
            }
        }
        _ => {
            // Screen-specific actions handled via Action messages
        }
    }
}

/// Handles directional keyboard and gamepad input (arrows, WASD, Tab, D-pad) and activation (Enter, Space, Esc, South, East).
pub fn handle_nav_input(
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    gamepads: Query<&Gamepad>,
    buttons: Query<(&MenuButton, Option<&ErrorModalUi>)>,
    current_screen: Option<Res<State<MenuScreen>>>,
    app_state: Option<Res<State<AppState>>>,
    stopping: Option<Res<SessionStopping>>,
    game_menu: Option<Res<GameMenu>>,
    session_error: Option<Res<SessionError>>,
    options_state: Option<Res<super::options::OptionsUiState>>,
    mut focus: ResMut<MenuFocus>,
    mut next_screen: Option<ResMut<NextState<MenuScreen>>>,
    mut app_exit: Option<MessageWriter<AppExit>>,
    mut sound_events: Option<MessageWriter<MenuSoundEvent>>,
    mut action_writer: Option<MessageWriter<Action>>,
) {
    if stopping.is_some() {
        return;
    }
    if options_state.is_some_and(|s| s.listening_action.is_some()) {
        return;
    }
    let is_ingame = app_state.as_ref().map(|s| s.get()) == Some(&AppState::InGame);
    let has_error = session_error.as_ref().is_some_and(|e| e.0.is_some());
    let menu_open = game_menu.as_ref().is_some_and(|gm| gm.panel.is_some());

    if is_ingame && !menu_open && !has_error {
        return;
    }

    if keyboard.is_none() && gamepads.is_empty() {
        return;
    }

    let in_create = current_screen.as_ref().map(|s| s.get()) == Some(&MenuScreen::CreateWorld);

    let target_buttons: Vec<&MenuButton> = if has_error {
        buttons
            .iter()
            .filter_map(|(b, modal)| modal.map(|_| b))
            .collect()
    } else {
        buttons.iter().map(|(b, _)| b).collect()
    };

    let mut ranks = Vec::new();
    let mut disabled = Vec::new();
    for b in &target_buttons {
        ranks.push(b.rank);
        if b.disabled {
            disabled.push(b.rank);
        }
    }

    let mut enabled: Vec<u16> = ranks
        .iter()
        .copied()
        .filter(|r| !disabled.contains(r))
        .collect();
    enabled.sort_unstable();
    enabled.dedup();

    let mut dir = 0;
    if let Some(ref kb) = keyboard {
        let shift = kb.pressed(KeyCode::ShiftLeft) || kb.pressed(KeyCode::ShiftRight);
        if !in_create {
            if kb.just_pressed(KeyCode::ArrowDown) || kb.just_pressed(KeyCode::KeyS) {
                dir += 1;
            }
            if kb.just_pressed(KeyCode::ArrowUp) || kb.just_pressed(KeyCode::KeyW) {
                dir -= 1;
            }
            if kb.just_pressed(KeyCode::Tab) {
                dir += if shift { -1 } else { 1 };
            }
        }
    }
    if !in_create {
        for gp in &gamepads {
            if gp.just_pressed(GamepadButton::DPadDown) {
                dir += 1;
            }
            if gp.just_pressed(GamepadButton::DPadUp) {
                dir -= 1;
            }
        }
    }

    if dir != 0 && !enabled.is_empty() {
        let new_rank = match focus.rank {
            Some(cur) => next_rank(&ranks, &disabled, cur, dir),
            None => {
                if dir > 0 {
                    enabled[0]
                } else {
                    *enabled.last().unwrap()
                }
            }
        };
        if focus.rank != Some(new_rank) {
            focus.rank = Some(new_rank);
            if let Some(ref mut events) = sound_events {
                events.write(MenuSoundEvent::Tick);
            }
        }
    }

    let back_pressed = keyboard
        .as_ref()
        .is_some_and(|kb| kb.just_pressed(KeyCode::Escape))
        || gamepads
            .iter()
            .any(|gp| gp.just_pressed(GamepadButton::East));
    let confirm_pressed = keyboard.as_ref().is_some_and(|kb| {
        kb.just_pressed(KeyCode::Enter)
            || kb.just_pressed(KeyCode::NumpadEnter)
            || kb.just_pressed(KeyCode::Space)
    }) || gamepads
        .iter()
        .any(|gp| gp.just_pressed(GamepadButton::South));

    if back_pressed {
        if !is_ingame && !has_error {
            execute_action(
                &Action::Back,
                current_screen.as_ref().map(|s| s.as_ref()),
                next_screen.as_mut(),
                app_exit.as_mut(),
                sound_events.as_mut(),
                action_writer.as_mut(),
            );
            focus.rank = None;
        }
    } else if !in_create && confirm_pressed {
        if let Some(rank) = focus.rank {
            if let Some(button) = target_buttons
                .iter()
                .find(|b| b.rank == rank && !b.disabled)
            {
                execute_action(
                    &button.action,
                    current_screen.as_ref().map(|s| s.as_ref()),
                    next_screen.as_mut(),
                    app_exit.as_mut(),
                    sound_events.as_mut(),
                    action_writer.as_mut(),
                );
            }
        }
    }
}

/// Reads mouse position for hover and button releases for activation.
pub fn handle_mouse_input(
    windows: Query<&Window>,
    mouse_button: Option<Res<ButtonInput<MouseButton>>>,
    buttons: Query<(
        &MenuButton,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&ErrorModalUi>,
    )>,
    current_screen: Option<Res<State<MenuScreen>>>,
    app_state: Option<Res<State<AppState>>>,
    stopping: Option<Res<SessionStopping>>,
    game_menu: Option<Res<GameMenu>>,
    session_error: Option<Res<SessionError>>,
    options_state: Option<Res<super::options::OptionsUiState>>,
    mut focus: ResMut<MenuFocus>,
    mut next_screen: Option<ResMut<NextState<MenuScreen>>>,
    mut app_exit: Option<MessageWriter<AppExit>>,
    mut sound_events: Option<MessageWriter<MenuSoundEvent>>,
    mut action_writer: Option<MessageWriter<Action>>,
) {
    if stopping.is_some() {
        return;
    }
    if options_state.is_some_and(|s| s.listening_action.is_some()) {
        return;
    }
    let is_ingame = app_state.as_ref().map(|s| s.get()) == Some(&AppState::InGame);
    let has_error = session_error.as_ref().is_some_and(|e| e.0.is_some());
    let menu_open = game_menu.as_ref().is_some_and(|gm| gm.panel.is_some());

    if is_ingame && !menu_open && !has_error {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    let mut hovered = None;
    for (button, node, transform, modal) in &buttons {
        if has_error && modal.is_none() {
            continue;
        }
        let center = transform.translation;
        let size = node.size();
        if point_in_rect(center, size, cursor) {
            hovered = Some(button);
            break;
        }
    }

    if let Some(button) = hovered {
        if !button.disabled {
            if focus.rank != Some(button.rank) {
                focus.rank = Some(button.rank);
                if let Some(ref mut events) = sound_events {
                    events.write(MenuSoundEvent::Tick);
                }
            }
            if let Some(mb) = mouse_button {
                if mb.just_released(MouseButton::Left) {
                    execute_action(
                        &button.action,
                        current_screen.as_ref().map(|s| s.as_ref()),
                        next_screen.as_mut(),
                        app_exit.as_mut(),
                        sound_events.as_mut(),
                        action_writer.as_mut(),
                    );
                }
            }
        }
    }
}

/// Directly activates the focused button (used by tests or programmatic activation).
pub fn activate_focused_button(
    buttons: Query<(&MenuButton, Option<&ErrorModalUi>)>,
    focus: Res<MenuFocus>,
    session_error: Option<Res<SessionError>>,
    current_screen: Option<Res<State<MenuScreen>>>,
    mut next_screen: Option<ResMut<NextState<MenuScreen>>>,
    mut app_exit: Option<MessageWriter<AppExit>>,
    mut sound_events: Option<MessageWriter<MenuSoundEvent>>,
    mut action_writer: Option<MessageWriter<Action>>,
) {
    let has_error = session_error.as_ref().is_some_and(|e| e.0.is_some());
    if let Some(rank) = focus.rank {
        if let Some((b, _)) = buttons
            .iter()
            .find(|(b, modal)| b.rank == rank && !b.disabled && (!has_error || modal.is_some()))
        {
            execute_action(
                &b.action,
                current_screen.as_ref().map(|s| s.as_ref()),
                next_screen.as_mut(),
                app_exit.as_mut(),
                sound_events.as_mut(),
                action_writer.as_mut(),
            );
        }
    }
}

pub struct NavPlugin;

impl Plugin for NavPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (handle_nav_input, handle_mouse_input));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamepad_dpad_south_and_east_navigate_menu() {
        let mut app = App::new();
        app.init_resource::<MenuFocus>()
            .add_message::<Action>()
            .add_message::<MenuSoundEvent>()
            .add_message::<AppExit>()
            .add_systems(Update, handle_nav_input);

        app.world_mut().spawn(MenuButton {
            rank: 0,
            action: Action::Play("world".into()),
            disabled: false,
        });
        app.world_mut().spawn(MenuButton {
            rank: 1,
            action: Action::Back,
            disabled: false,
        });

        let mut gp = Gamepad::default();
        gp.digital_mut().press(GamepadButton::DPadDown);
        let gp_entity = app.world_mut().spawn(gp).id();
        app.update();
        assert_eq!(app.world().resource::<MenuFocus>().rank, Some(0));

        // Simulate a new frame where the button was released and pressed again:
        // `clear` only drops just_pressed, so `press` on an already-held button
        // would not register another just_pressed edge.
        app.world_mut()
            .get_mut::<Gamepad>(gp_entity)
            .unwrap()
            .digital_mut()
            .reset_all();
        app.world_mut()
            .get_mut::<Gamepad>(gp_entity)
            .unwrap()
            .digital_mut()
            .press(GamepadButton::DPadDown);
        app.update();
        assert_eq!(app.world().resource::<MenuFocus>().rank, Some(1));

        // DPadUp moves back to rank 0.
        app.world_mut()
            .get_mut::<Gamepad>(gp_entity)
            .unwrap()
            .digital_mut()
            .reset_all();
        app.world_mut()
            .get_mut::<Gamepad>(gp_entity)
            .unwrap()
            .digital_mut()
            .press(GamepadButton::DPadUp);
        app.update();
        assert_eq!(app.world().resource::<MenuFocus>().rank, Some(0));

        // South confirms focused button (Action::Play).
        app.world_mut()
            .get_mut::<Gamepad>(gp_entity)
            .unwrap()
            .digital_mut()
            .reset_all();
        app.world_mut()
            .get_mut::<Gamepad>(gp_entity)
            .unwrap()
            .digital_mut()
            .press(GamepadButton::South);
        app.update();
        let mut reader = app
            .world()
            .resource::<bevy::ecs::message::Messages<Action>>()
            .get_cursor();
        let actions: Vec<Action> = reader
            .read(
                app.world()
                    .resource::<bevy::ecs::message::Messages<Action>>(),
            )
            .cloned()
            .collect();
        assert!(actions.contains(&Action::Play("world".into())));

        // East triggers Back and clears focus.
        app.world_mut()
            .get_mut::<Gamepad>(gp_entity)
            .unwrap()
            .digital_mut()
            .reset_all();
        app.world_mut()
            .get_mut::<Gamepad>(gp_entity)
            .unwrap()
            .digital_mut()
            .press(GamepadButton::East);
        app.update();
        assert_eq!(app.world().resource::<MenuFocus>().rank, None);
    }
}
