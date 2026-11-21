//! Menu UI styling, panel constants, and button spawn helpers.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};

use super::nav::MenuFocus;
use super::text::MenuText;
use super::{Action, MenuButton};

pub const BUTTON_WIDTH: f32 = 260.0;
pub const BUTTON_HEIGHT: f32 = 42.0;

// Button color constants (deep navy-slate & warm parchment/gold)
pub const COLOR_BUTTON_NORMAL: Color = Color::srgba(0.10, 0.13, 0.22, 0.96);
pub const COLOR_BUTTON_FOCUSED: Color = Color::srgba(0.20, 0.26, 0.42, 0.98);
pub const COLOR_BUTTON_SELECTED: Color = Color::srgba(0.15, 0.28, 0.27, 0.98);
pub const COLOR_BUTTON_DISABLED: Color = Color::srgba(0.07, 0.08, 0.12, 0.62);

// Border color constants (burnished bronze, warm gold & emerald accent)
pub const COLOR_BORDER_NORMAL: Color = Color::srgba(0.68, 0.56, 0.30, 0.95);
pub const COLOR_BORDER_FOCUSED: Color = Color::srgba(1.00, 0.87, 0.34, 1.00);
pub const COLOR_BORDER_SELECTED: Color = Color::srgba(0.40, 0.88, 0.62, 1.00);
pub const COLOR_BORDER_DISABLED: Color = Color::srgba(0.26, 0.26, 0.30, 0.50);

// Panel background constants (deep twilight navy-slate & warm gold/parchment frame)
pub const COLOR_BACKDROP: Color = Color::srgba(0.03, 0.05, 0.10, 0.58);
pub const COLOR_PANEL_BG: Color = Color::srgba(0.07, 0.09, 0.16, 0.94);
pub const COLOR_PANEL_INNER: Color = Color::srgba(0.11, 0.14, 0.23, 0.92);
pub const COLOR_PANEL_BORDER: Color = Color::srgba(0.82, 0.66, 0.32, 0.96);
pub const COLOR_HEADER_DIVIDER: Color = Color::srgba(0.82, 0.66, 0.32, 0.48);

// Text input field constants
pub const COLOR_INPUT_BG: Color = Color::srgba(0.05, 0.06, 0.11, 0.98);
pub const COLOR_INPUT_ACTIVE_BORDER: Color = Color::srgba(1.00, 0.87, 0.34, 1.00);
pub const COLOR_INPUT_IDLE_BORDER: Color = Color::srgba(0.40, 0.45, 0.60, 0.85);

// Semantic text & slider colors
pub const COLOR_TEXT_PRIMARY: Color = Color::srgb(0.96, 0.95, 0.91);
pub const COLOR_TEXT_GOLD: Color = Color::srgb(0.99, 0.85, 0.42);
pub const COLOR_TEXT_MUTED: Color = Color::srgb(0.72, 0.77, 0.86);
pub const COLOR_TEXT_ERROR: Color = Color::srgb(0.98, 0.44, 0.42);
pub const COLOR_TEXT_SUCCESS: Color = Color::srgb(0.46, 0.90, 0.60);
pub const COLOR_SLIDER_TRACK: Color = Color::srgba(0.04, 0.06, 0.10, 0.96);
pub const COLOR_SLIDER_FILL: Color = Color::srgba(0.88, 0.70, 0.30, 0.98);
pub const COLOR_SLIDER_BORDER: Color = Color::srgba(0.44, 0.48, 0.62, 0.85);

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
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
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
                border: UiRect::all(Val::Px(1.0)),
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
