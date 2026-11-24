//! In-game chat HUD: bottom-left fading chat log and `Enter` text input bar.

use std::collections::VecDeque;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::prelude::*;
use terravibe_sim::net::ClientMsg;

use super::style::{
    BORDER_GOLD, BORDER_IDLE, BORDER_W, INPUT_BG, PANEL_BG, RADIUS_BTN, RADIUS_PANEL, TEXT_GOLD,
    TEXT_PRIMARY,
};
use super::text::{HudText, HudTextColor};
use crate::net::{ChatReceived, Net};
use crate::session::AppState;

/// Maximum number of recent chat lines displayed in the bottom-left log.
pub const MAX_CHAT_LOG_LINES: usize = 8;

/// Maximum number of characters allowed in an outgoing chat message.
pub const MAX_CHAT_CHARS: usize = 200;

/// Seconds of inactivity after which the closed chat log fades out completely.
pub const CHAT_FADE_SECS: f32 = 10.0;

/// Duration of the fade-out ramp at the end of [`CHAT_FADE_SECS`].
const CHAT_FADE_RAMP_SECS: f32 = 2.0;

/// Whether the in-game chat text input bar is currently open and capturing keyboard input.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatInputOpen(pub bool);

/// Condition helper: returns `true` when the chat input bar is open.
pub fn is_open(chat: Option<Res<ChatInputOpen>>) -> bool {
    chat.is_some_and(|c| c.0)
}

/// One received chat line stored in [`ChatLog`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatEntry {
    pub sender: Option<String>,
    pub text: String,
}

impl ChatEntry {
    /// Formats the entry for display (`"Alice: hi"` or system `"hi"`).
    pub fn display_line(&self) -> String {
        match &self.sender {
            Some(name) if !name.is_empty() => format!("{name}: {}", self.text),
            _ => self.text.clone(),
        }
    }
}

/// Recent chat history, current input draft, and inactivity timer.
#[derive(Resource, Default, Debug)]
pub struct ChatLog {
    pub entries: VecDeque<ChatEntry>,
    pub draft: String,
    pub idle_secs: f32,
}

impl ChatLog {
    /// Appends a received message, keeping at most [`MAX_CHAT_LOG_LINES`] entries.
    pub fn push(&mut self, sender: Option<String>, text: String) {
        self.entries.push_back(ChatEntry { sender, text });
        while self.entries.len() > MAX_CHAT_LOG_LINES {
            self.entries.pop_front();
        }
        self.idle_secs = 0.0;
    }

    /// Computes log opacity in `0.0..=1.0` given whether the input bar is open.
    pub fn log_alpha(&self, input_open: bool) -> f32 {
        if input_open {
            return 1.0;
        }
        if self.entries.is_empty() || self.idle_secs >= CHAT_FADE_SECS {
            return 0.0;
        }
        let fade_start = (CHAT_FADE_SECS - CHAT_FADE_RAMP_SECS).max(0.0);
        if self.idle_secs <= fade_start {
            1.0
        } else {
            ((CHAT_FADE_SECS - self.idle_secs) / CHAT_FADE_RAMP_SECS).clamp(0.0, 1.0)
        }
    }
}

/// Trims whitespace, rejects empty strings, and clamps to [`MAX_CHAT_CHARS`] characters.
pub fn sanitize_chat_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(MAX_CHAT_CHARS).collect())
}

#[derive(Component)]
pub struct ChatHudRoot;

#[derive(Component)]
pub struct ChatLogContainer;

#[derive(Component)]
pub struct ChatLineUi(pub usize);

#[derive(Component)]
pub struct ChatInputBar;

#[derive(Component)]
pub struct ChatInputText;

pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatInputOpen>()
            .init_resource::<ChatLog>()
            .add_systems(OnEnter(AppState::InGame), setup_chat_ui)
            .add_systems(OnExit(AppState::InGame), reset_chat_state)
            .add_systems(
                Update,
                (
                    receive_chat_messages,
                    handle_chat_input.after(crate::menu::game::handle_game_menu_input),
                    update_chat_ui
                        .after(receive_chat_messages)
                        .after(handle_chat_input),
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

fn setup_chat_ui(mut commands: Commands) {
    commands
        .spawn((
            ChatHudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(16.0),
                bottom: Val::Px(96.0),
                width: Val::Px(460.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            GlobalZIndex(900),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|root| {
            root.spawn((
                ChatLogContainer,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(BORDER_W)),
                    border_radius: BorderRadius::all(Val::Px(RADIUS_PANEL)),
                    ..default()
                },
                BackgroundColor(PANEL_BG),
                BorderColor::all(BORDER_IDLE),
                Visibility::Hidden,
            ))
            .with_children(|log| {
                for i in 0..MAX_CHAT_LOG_LINES {
                    log.spawn((
                        ChatLineUi(i),
                        Node::default(),
                        HudText {
                            text: String::new(),
                            death: false,
                        },
                        HudTextColor(TEXT_PRIMARY),
                        Visibility::Hidden,
                    ));
                }
            });

            root.spawn((
                ChatInputBar,
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(30.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(BORDER_W)),
                    border_radius: BorderRadius::all(Val::Px(RADIUS_BTN)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(INPUT_BG),
                BorderColor::all(BORDER_GOLD),
                Visibility::Hidden,
            ))
            .with_children(|bar| {
                bar.spawn((
                    ChatInputText,
                    Node::default(),
                    HudText {
                        text: "> _".into(),
                        death: false,
                    },
                    HudTextColor(TEXT_PRIMARY),
                ));
            });
        });
}

fn reset_chat_state(mut chat_open: ResMut<ChatInputOpen>, mut chat_log: ResMut<ChatLog>) {
    chat_open.0 = false;
    *chat_log = ChatLog::default();
}

pub fn receive_chat_messages(
    mut reader: Option<MessageReader<ChatReceived>>,
    mut chat_log: ResMut<ChatLog>,
) {
    let Some(ref mut events) = reader else {
        return;
    };
    for msg in events.read() {
        chat_log.push(msg.sender.clone(), msg.text.clone());
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_chat_input(
    mut keys: Option<ResMut<ButtonInput<KeyCode>>>,
    mut key_events: Option<MessageReader<KeyboardInput>>,
    mut chat_open: ResMut<ChatInputOpen>,
    mut chat_log: ResMut<ChatLog>,
    net: Option<Res<Net>>,
    game_menu: Option<Res<crate::session::GameMenu>>,
    creative: Option<Res<crate::menu::creative::CreativeMenuOpen>>,
) {
    let menu_is_closed = crate::menu::game::menu_closed(game_menu, creative);

    if !chat_open.0 {
        // Drain keyboard events while closed so stale presses are never replayed on open.
        if let Some(ref mut events) = key_events {
            for _ in events.read() {}
        }
        if !menu_is_closed {
            return;
        }
        let open_pressed = keys.as_ref().is_some_and(|kb| {
            kb.just_pressed(KeyCode::Enter) || kb.just_pressed(KeyCode::NumpadEnter)
        });
        if open_pressed {
            chat_open.0 = true;
            chat_log.draft.clear();
            chat_log.idle_secs = 0.0;
        }
        return;
    }

    // Chat input is open: capture typed characters, Backspace, Enter, and Esc.
    let mut handled_esc = false;
    let mut handled_enter = false;
    let mut handled_backspace = false;

    if let Some(ref mut events) = key_events {
        for ev in events.read() {
            if ev.state != ButtonState::Pressed {
                continue;
            }
            if ev.key_code == KeyCode::Escape || ev.logical_key == Key::Escape {
                chat_open.0 = false;
                chat_log.draft.clear();
                chat_log.idle_secs = 0.0;
                handled_esc = true;
                if let Some(ref mut kb) = keys {
                    kb.clear_just_pressed(KeyCode::Escape);
                }
                break;
            }
            if matches!(ev.key_code, KeyCode::Enter | KeyCode::NumpadEnter)
                || ev.logical_key == Key::Enter
            {
                if let Some(text) = sanitize_chat_text(&chat_log.draft)
                    && let Some(ref net) = net
                {
                    let _ = net.send(ClientMsg::Chat { text });
                }
                chat_log.draft.clear();
                chat_open.0 = false;
                chat_log.idle_secs = 0.0;
                handled_enter = true;
                break;
            }
            if ev.key_code == KeyCode::Backspace || ev.logical_key == Key::Backspace {
                chat_log.draft.pop();
                handled_backspace = true;
                continue;
            }
            match &ev.logical_key {
                Key::Space => {
                    if chat_log.draft.chars().count() < MAX_CHAT_CHARS {
                        chat_log.draft.push(' ');
                    }
                }
                Key::Character(s) => {
                    for ch in s.chars() {
                        if !ch.is_control() && chat_log.draft.chars().count() < MAX_CHAT_CHARS {
                            chat_log.draft.push(ch);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if !chat_open.0 {
        return;
    }

    if let Some(ref mut kb) = keys {
        if !handled_esc && kb.just_pressed(KeyCode::Escape) {
            chat_open.0 = false;
            chat_log.draft.clear();
            chat_log.idle_secs = 0.0;
            kb.clear_just_pressed(KeyCode::Escape);
            return;
        }
        if !handled_enter
            && (kb.just_pressed(KeyCode::Enter) || kb.just_pressed(KeyCode::NumpadEnter))
        {
            if let Some(text) = sanitize_chat_text(&chat_log.draft)
                && let Some(ref net) = net
            {
                let _ = net.send(ClientMsg::Chat { text });
            }
            chat_log.draft.clear();
            chat_open.0 = false;
            chat_log.idle_secs = 0.0;
            return;
        }
        if !handled_backspace && kb.just_pressed(KeyCode::Backspace) {
            chat_log.draft.pop();
        }
    }
}

pub fn update_chat_ui(
    time: Option<Res<Time>>,
    chat_open: Res<ChatInputOpen>,
    mut chat_log: ResMut<ChatLog>,
    mut log_container: Query<
        (&mut Visibility, &mut BackgroundColor, &mut BorderColor),
        (With<ChatLogContainer>, Without<ChatInputBar>, Without<ChatLineUi>),
    >,
    mut lines: Query<
        (&ChatLineUi, &mut HudText, &mut HudTextColor, &mut Visibility),
        (Without<ChatLogContainer>, Without<ChatInputBar>, Without<ChatInputText>),
    >,
    mut input_bar: Query<
        &mut Visibility,
        (With<ChatInputBar>, Without<ChatLogContainer>, Without<ChatLineUi>),
    >,
    mut input_text: Query<&mut HudText, (With<ChatInputText>, Without<ChatLineUi>)>,
) {
    if !chat_open.0 && !chat_log.entries.is_empty() {
        if let Some(t) = time {
            chat_log.idle_secs = (chat_log.idle_secs + t.delta_secs()).min(CHAT_FADE_SECS + 1.0);
        }
    } else if chat_open.0 {
        chat_log.idle_secs = 0.0;
    }

    let alpha = chat_log.log_alpha(chat_open.0);
    let show_log = alpha > 0.0 && (!chat_log.entries.is_empty() || chat_open.0);

    if let Ok((mut vis, mut bg, mut border)) = log_container.single_mut() {
        *vis = if show_log {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        let bg_s = PANEL_BG.to_srgba();
        bg.0 = Color::srgba(bg_s.red, bg_s.green, bg_s.blue, bg_s.alpha * alpha);
        let bd_s = BORDER_IDLE.to_srgba();
        *border = BorderColor::all(Color::srgba(
            bd_s.red,
            bd_s.green,
            bd_s.blue,
            bd_s.alpha * alpha,
        ));
    }

    for (line, mut hud_text, mut color, mut vis) in &mut lines {
        if let Some(entry) = chat_log.entries.get(line.0) {
            let formatted = entry.display_line();
            if hud_text.text != formatted {
                hud_text.text = formatted;
            }
            let base = if entry.sender.is_some() {
                TEXT_PRIMARY
            } else {
                TEXT_GOLD
            }
            .to_srgba();
            let tinted = Color::srgba(base.red, base.green, base.blue, alpha);
            if color.0 != tinted {
                color.0 = tinted;
            }
            *vis = if show_log {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        } else {
            if !hud_text.text.is_empty() {
                hud_text.text.clear();
            }
            *vis = Visibility::Hidden;
        }
    }

    if let Ok(mut vis) = input_bar.single_mut() {
        *vis = if chat_open.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    if let Ok(mut text) = input_text.single_mut() {
        let desired = format!("> {}_", chat_log.draft);
        if text.text != desired {
            text.text = desired;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::mouse::MouseWheel;
    use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, Window};
    use terravibe_sim::net::{ServerConn, in_process_pair};

    use crate::hud::chest::ChestPanel;
    use crate::hud::inventory::InventoryOpen;
    use crate::hud::town::TownPanel;
    use crate::menu::game::handle_game_menu_input;
    use crate::session::{GameMenu, SessionError};
    use crate::view::camera::{CameraMode, FlyCamera};
    use crate::view::player::{SampledInput, sample_input};

    fn key_char(ch: &str) -> KeyboardInput {
        KeyboardInput {
            key_code: KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified),
            logical_key: Key::Character(ch.into()),
            state: ButtonState::Pressed,
            text: Some(ch.into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

    fn key_special(key_code: KeyCode, logical_key: Key) -> KeyboardInput {
        KeyboardInput {
            key_code,
            logical_key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

    #[test]
    fn chat_open_suppresses_gameplay_and_sends_message() {
        let (client, server) = in_process_pair();

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<AppState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<KeyboardInput>()
            .add_message::<MouseWheel>()
            .add_message::<ChatReceived>()
            .add_message::<crate::menu::Action>()
            .init_resource::<GameMenu>()
            .init_resource::<SessionError>()
            .init_resource::<SampledInput>()
            .init_resource::<InventoryOpen>()
            .init_resource::<ChestPanel>()
            .init_resource::<TownPanel>()
            .insert_resource(CameraMode::First)
            .insert_resource(Net::new(client))
            .add_plugins(ChatPlugin)
            .add_systems(
                Update,
                (
                    handle_game_menu_input.run_if(in_state(AppState::InGame)),
                    sample_input.run_if(in_state(AppState::InGame)),
                ),
            );

        app.world_mut().spawn(FlyCamera {
            yaw: 0.0,
            pitch: 0.0,
        });
        app.world_mut().spawn((
            Window::default(),
            PrimaryWindow,
            CursorOptions {
                grab_mode: CursorGrabMode::Locked,
                visible: false,
                ..default()
            },
        ));

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app.update();

        // Pressing T (Throw Item) must NOT open chat input.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyT);
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(key_char("t"));
        app.update();

        assert!(
            !app.world().resource::<ChatInputOpen>().0,
            "pressing T in-game must not open chat input (T is Throw Item)"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();

        // Press Enter to open chat input.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(key_special(KeyCode::Enter, Key::Enter));
        app.update();

        assert!(
            app.world().resource::<ChatInputOpen>().0,
            "pressing Enter in-game should open chat input"
        );
        assert!(
            app.world().resource::<ChatLog>().draft.is_empty(),
            "opening key Enter must not leak into chat draft"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();

        // While ChatInputOpen(true), gameplay movement/jump/hotbar/attack are suppressed.
        {
            let mut kb = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            kb.press(KeyCode::KeyW);
            kb.press(KeyCode::Space);
            kb.press(KeyCode::Digit4);
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let sampled = app.world().resource::<SampledInput>().input;
        assert_eq!(sampled.forward, 0.0, "W must not move while chat is open");
        assert!(!sampled.jump, "Space must not jump while chat is open");
        assert!(!sampled.use_held, "LMB must not attack while chat is open");
        assert_eq!(sampled.slot, 0, "Digit keys must not change hotbar slot");

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();

        // Type "  hi!x", backspace 'x', then press Enter -> sends trimmed "hi!" and closes.
        {
            let mut evs = app.world_mut().resource_mut::<Messages<KeyboardInput>>();
            evs.write(key_special(KeyCode::Space, Key::Space));
            evs.write(key_char("h"));
            evs.write(key_char("i"));
            evs.write(key_char("!"));
            evs.write(key_char("x"));
            evs.write(key_special(KeyCode::Backspace, Key::Backspace));
            evs.write(key_special(KeyCode::Enter, Key::Enter));
        }
        app.update();

        assert!(
            !app.world().resource::<ChatInputOpen>().0,
            "Enter should close chat input"
        );
        assert!(
            matches!(
                server.try_recv().unwrap(),
                Some(ClientMsg::Chat { ref text }) if text == "hi!"
            ),
            "expected ClientMsg::Chat with trimmed text 'hi!'"
        );

        // Open chat again and press Esc: closes chat WITHOUT opening pause menu.
        app.world_mut().resource_mut::<ChatInputOpen>().0 = true;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(key_special(KeyCode::Escape, Key::Escape));
        app.update();

        assert!(!app.world().resource::<ChatInputOpen>().0);
        assert_eq!(
            app.world().resource::<GameMenu>().panel,
            None,
            "Esc while chat is open must not open pause menu"
        );

        // Receiving > 8 messages keeps only the last 8, and OnExit(InGame) clears ChatLog.
        for i in 0..10 {
            app.world_mut()
                .resource_mut::<Messages<ChatReceived>>()
                .write(ChatReceived {
                    sender: Some(format!("P{i}")),
                    text: format!("msg {i}"),
                });
        }
        app.update();
        let log = app.world().resource::<ChatLog>();
        assert_eq!(log.entries.len(), MAX_CHAT_LOG_LINES);
        assert_eq!(log.entries.front().unwrap().text, "msg 2");
        assert_eq!(log.entries.back().unwrap().text, "msg 9");

        // Fade out after 10s of inactivity when closed; stays visible when open.
        app.world_mut().resource_mut::<ChatLog>().idle_secs = CHAT_FADE_SECS;
        assert_eq!(app.world().resource::<ChatLog>().log_alpha(false), 0.0);
        assert_eq!(app.world().resource::<ChatLog>().log_alpha(true), 1.0);

        // OnExit(AppState::InGame) clears chat log and closes chat input.
        app.world_mut().resource_mut::<ChatInputOpen>().0 = true;
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Menu);
        app.update();
        assert!(!app.world().resource::<ChatInputOpen>().0);
        assert!(app.world().resource::<ChatLog>().entries.is_empty());
    }

    #[test]
    fn sanitize_chat_text_trims_ignores_empty_and_clamps_200() {
        assert_eq!(sanitize_chat_text(""), None);
        assert_eq!(sanitize_chat_text("   \t  "), None);
        assert_eq!(sanitize_chat_text("  hello  "), Some("hello".into()));
        let long = "a".repeat(250);
        let sanitized = sanitize_chat_text(&long).unwrap();
        assert_eq!(sanitized.chars().count(), MAX_CHAT_CHARS);
    }
}
