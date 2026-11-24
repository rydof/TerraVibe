//! Shared UI design system tokens: obsidian-slate translucent surfaces,
//! 1px hairline borders, restrained bronze/gold focus accents, and typography colors.

use bevy::prelude::*;

// Surfaces
pub const PANEL_BG: Color = Color::srgba(0.06, 0.08, 0.13, 0.90);
pub const PANEL_BG_SOLID: Color = Color::srgba(0.05, 0.07, 0.12, 0.95);
pub const PANEL_INNER: Color = Color::srgba(0.09, 0.12, 0.19, 0.92);
pub const SLOT_BG: Color = Color::srgba(0.04, 0.06, 0.10, 0.88);
pub const BAR_BG: Color = Color::srgba(0.04, 0.06, 0.10, 0.90);
pub const BACKDROP: Color = Color::srgba(0.02, 0.03, 0.06, 0.62);
pub const INPUT_BG: Color = Color::srgba(0.04, 0.05, 0.09, 0.96);
pub const HEADER_DIVIDER: Color = Color::srgba(0.86, 0.70, 0.34, 0.32);

// Borders
pub const BORDER_IDLE: Color = Color::srgba(0.36, 0.43, 0.58, 0.38);
pub const BORDER_ACTIVE: Color = Color::srgba(0.56, 0.65, 0.82, 0.72);
pub const BORDER_GOLD: Color = Color::srgba(0.86, 0.70, 0.34, 0.90);
pub const BORDER_SELECTED: Color = Color::srgba(0.32, 0.82, 0.58, 0.88);
pub const BORDER_DISABLED: Color = Color::srgba(0.24, 0.27, 0.34, 0.36);

// Buttons & Sliders
pub const BTN_BG: Color = Color::srgba(0.09, 0.12, 0.19, 0.92);
pub const BTN_BG_HOVER: Color = Color::srgba(0.15, 0.20, 0.31, 0.96);
pub const BTN_BG_ACTIVE: Color = Color::srgba(0.11, 0.23, 0.22, 0.96);
pub const BTN_BG_DISABLED: Color = Color::srgba(0.06, 0.07, 0.10, 0.58);
pub const SLIDER_TRACK: Color = Color::srgba(0.04, 0.06, 0.10, 0.95);
pub const SLIDER_FILL: Color = Color::srgba(0.86, 0.70, 0.34, 0.95);

// Typography
pub const TEXT_PRIMARY: Color = Color::srgb(0.95, 0.94, 0.91);
pub const TEXT_MUTED: Color = Color::srgb(0.64, 0.70, 0.80);
pub const TEXT_GOLD: Color = Color::srgb(0.95, 0.80, 0.40);
pub const TEXT_SUCCESS: Color = Color::srgb(0.42, 0.88, 0.58);
pub const TEXT_ERROR: Color = Color::srgb(0.96, 0.42, 0.40);

// Geometry
pub const RADIUS_PANEL: f32 = 6.0;
pub const RADIUS_BTN: f32 = 4.0;
pub const RADIUS_SLOT: f32 = 4.0;
pub const RADIUS_BAR: f32 = 2.0;
pub const BORDER_W: f32 = 1.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_tokens_match_spec() {
        assert_eq!(RADIUS_PANEL, 6.0);
        assert_eq!(RADIUS_BTN, 4.0);
        assert_eq!(RADIUS_SLOT, 4.0);
        assert_eq!(RADIUS_BAR, 2.0);
        assert_eq!(BORDER_W, 1.0);
    }

    #[test]
    fn surface_and_border_alphas_are_translucent_and_subtle() {
        let panel = PANEL_BG.to_srgba();
        assert!(
            (0.8..=0.95).contains(&panel.alpha),
            "PANEL_BG alpha {} not in 0.8..=0.95",
            panel.alpha
        );
        let border = BORDER_IDLE.to_srgba();
        assert!(
            border.alpha <= 0.5,
            "BORDER_IDLE alpha {} should be <= 0.5",
            border.alpha
        );
    }

    #[test]
    fn text_primary_has_high_contrast_against_panel_bg() {
        let fg = TEXT_PRIMARY.to_srgba();
        let bg = PANEL_BG.to_srgba();
        let lum = |c: bevy::color::Srgba| 0.2126 * c.red + 0.7152 * c.green + 0.0722 * c.blue;
        let contrast = (lum(fg) + 0.05) / (lum(bg) + 0.05);
        assert!(
            contrast > 7.0,
            "expected high luminance contrast (> 7.0), got {contrast}"
        );
    }
}
