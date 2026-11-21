//! Core gamepad support: stick deadzone, move/look reading, button glyphs, and plugin.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

pub use crate::input_map::GameAction;
use crate::session::AppState;
use crate::settings::Settings;
use crate::view::camera::{CameraMode, ControlMode, FlyCamera, update_camera_angles};

/// Third-person pitch limits, radians (positive looks up).
const THIRD_PITCH: (f32, f32) = (-std::f32::consts::FRAC_PI_3, 1.2217);

/// Default radial deadzone for analog thumbsticks.
pub const STICK_DEADZONE: f32 = 0.15;

/// Right-stick look speed multiplier (in virtual mouse-pixels per second).
const GAMEPAD_LOOK_SCALE: f32 = 900.0;

/// Returns the short prompt glyph label for `action` on a standard gamepad.
pub fn gamepad_prompt_glyph(action: GameAction) -> &'static str {
    match action {
        GameAction::MoveForward
        | GameAction::MoveBack
        | GameAction::MoveLeft
        | GameAction::MoveRight => "[LS]",
        GameAction::Jump => "[A]",
        GameAction::Inventory => "[Y]",
        GameAction::Minimap => "[D-Up]",
        GameAction::FullscreenMap => "[View]",
        GameAction::QuickHeal => "[LB+Y]",
        GameAction::QuickMana => "[LB+X]",
        GameAction::QuickBuff => "[LB+B]",
        GameAction::BreakWall => "[X]",
        GameAction::EquipArmor => "[D-Down]",
        GameAction::Throw => "[D-Left]",
    }
}

/// Applies a radial deadzone to a 2D thumbstick reading, rescaling `[deadzone, 1.0]` to `[0.0, 1.0]`.
pub fn apply_stick_deadzone(raw: Vec2, deadzone: f32) -> Vec2 {
    let len = raw.length();
    if len <= deadzone || len == 0.0 || deadzone >= 1.0 {
        return Vec2::ZERO;
    }
    let scaled = ((len - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0);
    (raw / len) * scaled
}

/// Reads `(move_vec, look_vec)` from `gamepad` with `deadzone` applied.
///
/// `move_vec` is `(strafe, forward)` from the left stick; `look_vec` is `(x, y)`
/// from the right stick.
pub fn read_gamepad_move_look(gamepad: &Gamepad, deadzone: f32) -> (Vec2, Vec2) {
    let left_raw = Vec2::new(
        gamepad.get(GamepadAxis::LeftStickX).unwrap_or(0.0),
        gamepad.get(GamepadAxis::LeftStickY).unwrap_or(0.0),
    );
    let right_raw = Vec2::new(
        gamepad.get(GamepadAxis::RightStickX).unwrap_or(0.0),
        gamepad.get(GamepadAxis::RightStickY).unwrap_or(0.0),
    );
    (
        apply_stick_deadzone(left_raw, deadzone),
        apply_stick_deadzone(right_raw, deadzone),
    )
}

/// Applies right-stick camera look when in game with cursor grabbed.
fn apply_gamepad_look(
    gamepads: Query<&Gamepad>,
    time: Res<Time>,
    opts: Query<&CursorOptions, With<PrimaryWindow>>,
    mut q: Query<(&mut FlyCamera, &mut Transform)>,
    mode: Option<Res<ControlMode>>,
    cmode: Option<Res<CameraMode>>,
    settings: Option<Res<Settings>>,
) {
    if opts
        .iter()
        .next()
        .is_some_and(|o| o.grab_mode == CursorGrabMode::None)
    {
        return;
    }
    let Some(gp) = gamepads.iter().next() else {
        return;
    };
    let (_, look) = read_gamepad_move_look(gp, STICK_DEADZONE);
    if look == Vec2::ZERO {
        return;
    }
    // Invert Y so pushing stick up pitches camera up (matching negative mouse delta.y).
    let delta = Vec2::new(look.x, -look.y) * GAMEPAD_LOOK_SCALE * time.delta_secs();
    let limits = if mode.as_deref() == Some(&ControlMode::Player)
        && cmode.as_deref() == Some(&CameraMode::Third)
    {
        THIRD_PITCH
    } else {
        (-1.54, 1.54)
    };
    let sens = settings.as_ref().map_or(1.0, |s| s.sensitivity);
    let invert_y = settings.as_ref().is_some_and(|s| s.invert_y);
    for (mut cam, mut tf) in &mut q {
        let (new_yaw, new_pitch) =
            update_camera_angles(cam.yaw, cam.pitch, delta, sens, invert_y, limits);
        cam.yaw = new_yaw;
        cam.pitch = new_pitch;
        tf.rotation = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);
    }
}

/// Toggles the inventory panel when the North gamepad button is pressed in-game.
fn toggle_inventory_gamepad(
    gamepads: Query<&Gamepad>,
    mut open: Option<ResMut<crate::hud::inventory::InventoryOpen>>,
    net: Option<Res<crate::net::Net>>,
    mut opts: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Some(ref mut open) = open else {
        return;
    };
    if gamepads
        .iter()
        .any(|g| g.just_pressed(GamepadButton::North))
    {
        open.0 = !open.0;
        if !open.0
            && let Some(net) = &net
        {
            let _ = net.send(terravibe_sim::net::ClientMsg::CloseInventory);
        }
        let grab_mode = if open.0 {
            CursorGrabMode::None
        } else {
            CursorGrabMode::Locked
        };
        for mut opt in &mut opts {
            opt.grab_mode = grab_mode;
            opt.visible = open.0;
        }
    }
}

/// Registers gamepad input systems (right-stick look and North inventory toggle).
pub struct GamepadPlugin;

impl Plugin for GamepadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (apply_gamepad_look, toggle_inventory_gamepad)
                .run_if(in_state(AppState::InGame))
                .run_if(crate::menu::menu_closed),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamepad_stick_deadzone_and_prompt_glyphs() {
        assert_eq!(gamepad_prompt_glyph(GameAction::Jump), "[A]");
        assert_eq!(gamepad_prompt_glyph(GameAction::Inventory), "[Y]");
        assert_eq!(gamepad_prompt_glyph(GameAction::MoveForward), "[LS]");
        assert_eq!(gamepad_prompt_glyph(GameAction::BreakWall), "[X]");
        assert_eq!(gamepad_prompt_glyph(GameAction::Minimap), "[D-Up]");
        assert_eq!(gamepad_prompt_glyph(GameAction::FullscreenMap), "[View]");
        assert_eq!(gamepad_prompt_glyph(GameAction::QuickHeal), "[LB+Y]");
        assert_eq!(gamepad_prompt_glyph(GameAction::QuickMana), "[LB+X]");
        assert_eq!(gamepad_prompt_glyph(GameAction::QuickBuff), "[LB+B]");
        assert_eq!(gamepad_prompt_glyph(GameAction::EquipArmor), "[D-Down]");
        assert_eq!(gamepad_prompt_glyph(GameAction::Throw), "[D-Left]");

        // Values inside deadzone snap to zero.
        assert_eq!(
            apply_stick_deadzone(Vec2::new(0.05, -0.05), STICK_DEADZONE),
            Vec2::ZERO
        );
        // Values outside deadzone rescale smoothly toward 1.0.
        let out = apply_stick_deadzone(Vec2::new(1.0, 0.0), STICK_DEADZONE);
        assert!((out.x - 1.0).abs() < 1e-4);
        assert!(out.y.abs() < 1e-4);

        let mid = apply_stick_deadzone(Vec2::new(0.575, 0.0), STICK_DEADZONE);
        assert!((mid.x - 0.5).abs() < 1e-3);
    }
}
