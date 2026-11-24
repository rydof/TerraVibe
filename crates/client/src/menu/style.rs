//! Menu UI styling, panel constants, and button spawn helpers.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};

use super::nav::MenuFocus;
use super::text::MenuText;
use super::{Action, MenuButton};

pub use crate::ui_theme::*;

pub const BUTTON_WIDTH: f32 = 260.0;
pub const BUTTON_HEIGHT: f32 = 42.0;

// Button color aliases mapped to ui_theme tokens
pub const COLOR_BUTTON_NORMAL: Color = BTN_BG;
pub const COLOR_BUTTON_FOCUSED: Color = BTN_BG_HOVER;
pub const COLOR_BUTTON_SELECTED: Color = BTN_BG_ACTIVE;
pub const COLOR_BUTTON_DISABLED: Color = BTN_BG_DISABLED;

// Border color aliases mapped to ui_theme tokens
pub const COLOR_BORDER_NORMAL: Color = BORDER_IDLE;
pub const COLOR_BORDER_FOCUSED: Color = BORDER_GOLD;
pub const COLOR_BORDER_SELECTED: Color = BORDER_SELECTED;
pub const COLOR_BORDER_DISABLED: Color = BORDER_DISABLED;

// Panel background aliases mapped to ui_theme tokens
pub const COLOR_BACKDROP: Color = BACKDROP;
pub const COLOR_PANEL_BG: Color = PANEL_BG;
pub const COLOR_PANEL_INNER: Color = PANEL_INNER;
pub const COLOR_PANEL_BORDER: Color = BORDER_IDLE;
pub const COLOR_HEADER_DIVIDER: Color = HEADER_DIVIDER;

// Text input field aliases mapped to ui_theme tokens
pub const COLOR_INPUT_BG: Color = INPUT_BG;
pub const COLOR_INPUT_ACTIVE_BORDER: Color = BORDER_GOLD;
pub const COLOR_INPUT_IDLE_BORDER: Color = BORDER_IDLE;

// Semantic text & slider aliases mapped to ui_theme tokens
pub const COLOR_TEXT_PRIMARY: Color = TEXT_PRIMARY;
pub const COLOR_TEXT_GOLD: Color = TEXT_GOLD;
pub const COLOR_TEXT_MUTED: Color = TEXT_MUTED;
pub const COLOR_TEXT_ERROR: Color = TEXT_ERROR;
pub const COLOR_TEXT_SUCCESS: Color = TEXT_SUCCESS;
pub const COLOR_SLIDER_TRACK: Color = SLIDER_TRACK;
pub const COLOR_SLIDER_FILL: Color = SLIDER_FILL;
pub const COLOR_SLIDER_BORDER: Color = BORDER_IDLE;

/// Marks a `MenuButton` that is currently active/selected (e.g. chosen world size or tab).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct MenuSelected(pub bool);

/// Returns `(width, height)` of the primary window, falling back to `1280x720` in headless tests.
pub fn viewport_size(primary_window: Option<&Query<&Window, With<PrimaryWindow>>>) -> (f32, f32) {
    primary_window
        .and_then(|q| q.single().ok())
        .map(|w| (w.width().max(640.0), w.height().max(480.0)))
        .unwrap_or((1280.0, 720.0))
}

/// Updates button background and border colors according to focus, selected, and disabled state.
pub fn update_button_visuals(
    focus: Res<MenuFocus>,
    session_error: Option<Res<crate::session::SessionError>>,
    mut buttons: Query<(
        &MenuButton,
        &mut BackgroundColor,
        &mut BorderColor,
        Option<&super::game::ErrorModalUi>,
        Option<&MenuSelected>,
    )>,
) {
    let has_error = session_error.as_ref().is_some_and(|e| e.0.is_some());
    for (button, mut bg, mut border, modal, selected) in &mut buttons {
        let is_selected = selected.is_some_and(|s| s.0);
        if button.disabled {
            bg.0 = COLOR_BUTTON_DISABLED;
            *border = BorderColor::all(COLOR_BORDER_DISABLED);
        } else if focus.rank == Some(button.rank) && (!has_error || modal.is_some()) {
            bg.0 = COLOR_BUTTON_FOCUSED;
            *border = BorderColor::all(COLOR_BORDER_FOCUSED);
        } else if is_selected {
            bg.0 = COLOR_BUTTON_SELECTED;
            *border = BorderColor::all(COLOR_BORDER_SELECTED);
        } else {
            bg.0 = COLOR_BUTTON_NORMAL;
            *border = BorderColor::all(COLOR_BORDER_NORMAL);
        }
    }
}

/// Spawns a styled menu button with custom size and disabled state.
pub fn spawn_button_sized(
    commands: &mut Commands,
    label: &str,
    action: Action,
    rank: u16,
    pos: (f32, f32),
    size: (f32, f32),
    disabled: bool,
) -> Entity {
    let (bg, border) = if disabled {
        (COLOR_BUTTON_DISABLED, COLOR_BORDER_DISABLED)
    } else {
        (COLOR_BUTTON_NORMAL, COLOR_BORDER_NORMAL)
    };

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(pos.0),
                top: Val::Px(pos.1),
                width: Val::Px(size.0),
                height: Val::Px(size.1),
                border: UiRect::all(Val::Px(BORDER_W)),
                border_radius: BorderRadius::all(Val::Px(RADIUS_BTN)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(border),
            MenuButton {
                action,
                rank,
                disabled,
            },
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    position_type: PositionType::Relative,
                    ..default()
                },
                MenuText::new(label, 1.0),
            ));
        })
        .id()
}

/// Spawns a styled menu button with custom disabled state.
pub fn spawn_button_ext(
    commands: &mut Commands,
    label: &str,
    action: Action,
    rank: u16,
    pos: (f32, f32),
    disabled: bool,
) -> Entity {
    spawn_button_sized(
        commands,
        label,
        action,
        rank,
        pos,
        (BUTTON_WIDTH, BUTTON_HEIGHT),
        disabled,
    )
}

/// Spawns an enabled styled menu button.
pub fn spawn_button(
    commands: &mut Commands,
    label: &str,
    action: Action,
    rank: u16,
    pos: (f32, f32),
) -> Entity {
    spawn_button_ext(commands, label, action, rank, pos, false)
}

/// Spawns a visual horizontal slider/progress bar track and returns the fill bar entity.
pub fn spawn_slider_bar(
    parent: &mut ChildSpawnerCommands,
    width: f32,
    height: f32,
    fraction: f32,
    fill_color: Color,
) -> Entity {
    let clamped = fraction.clamp(0.0, 1.0);
    let mut fill_entity = Entity::PLACEHOLDER;
    parent
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                border: UiRect::all(Val::Px(BORDER_W)),
                border_radius: BorderRadius::all(Val::Px(height * 0.5)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(COLOR_SLIDER_TRACK),
            BorderColor::all(COLOR_SLIDER_BORDER),
        ))
        .with_children(|track| {
            fill_entity = track
                .spawn((
                    Node {
                        width: Val::Percent(clamped * 100.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(Val::Px(height * 0.5)),
                        ..default()
                    },
                    BackgroundColor(fill_color),
                ))
                .id();
        });
    fill_entity
}

pub struct StylePlugin;

impl Plugin for StylePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_button_visuals);
    }
}
