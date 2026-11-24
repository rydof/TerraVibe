//! Shared HUD UI theme constants re-exported from `crate::ui_theme`.

use bevy::prelude::*;

pub use crate::ui_theme::*;

/// Standard obsidian-slate panel border (backward-compatible alias).
pub const PANEL_BORDER: Color = BORDER_IDLE;
/// Warm gold border for highlighted panels, boss bars, and primary buttons (backward-compatible alias).
pub const GOLD_BORDER: Color = BORDER_GOLD;
/// Gold accent text color (backward-compatible alias).
pub const GOLD_TEXT: Color = TEXT_GOLD;
