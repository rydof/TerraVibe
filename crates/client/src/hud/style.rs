//! Shared Terraria-inspired UI theme constants: deep indigo translucent panels,
//! gold/slate borders, rounded corners, and crisp button states.

use bevy::prelude::*;

/// Standard Terraria translucent indigo-navy panel background.
pub const PANEL_BG: Color = Color::srgba(0.10, 0.13, 0.28, 0.90);
/// Slightly denser panel background for tooltips, modals, and dialogue cards.
pub const PANEL_BG_SOLID: Color = Color::srgba(0.09, 0.12, 0.26, 0.94);
/// Subtle dark backdrop for meter bars (boss bar, breath bar).
pub const BAR_BG: Color = Color::srgba(0.06, 0.08, 0.16, 0.88);

/// Standard steel-indigo panel border.
pub const PANEL_BORDER: Color = Color::srgba(0.38, 0.47, 0.78, 0.92);
/// Warm Terraria gold border for highlighted panels, boss bars, and primary buttons.
pub const GOLD_BORDER: Color = Color::srgba(0.88, 0.74, 0.32, 0.95);

/// Standard button fill.
pub const BTN_BG: Color = Color::srgba(0.16, 0.22, 0.45, 0.92);
/// Highlighted / hovered / selected button or row fill.
pub const BTN_BG_ACTIVE: Color = Color::srgba(0.28, 0.36, 0.68, 0.96);

/// Gold accent text color (headers, NPC speaker names, highlights).
pub const GOLD_TEXT: Color = Color::srgb(0.98, 0.86, 0.42);

/// Corner radii for panels, buttons/tooltips, and meter bars.
pub const RADIUS_PANEL: f32 = 8.0;
pub const RADIUS_BTN: f32 = 6.0;
pub const RADIUS_BAR: f32 = 4.0;
