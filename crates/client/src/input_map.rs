//! Rebindable gameplay action map (`Copy` so `Settings` remains `Copy`).

use std::fmt;

use bevy::input::keyboard::KeyCode;
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Every rebindable gameplay action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum GameAction {
    MoveForward = 0,
    MoveBack = 1,
    MoveLeft = 2,
    MoveRight = 3,
    Jump = 4,
    Inventory = 5,
    Minimap = 6,
    FullscreenMap = 7,
    QuickHeal = 8,
    QuickMana = 9,
    QuickBuff = 10,
    BreakWall = 11,
    EquipArmor = 12,
    Throw = 13,
    RulerPin = 14,
}

impl GameAction {
    pub const COUNT: usize = 15;

    pub const ALL: [GameAction; Self::COUNT] = [
        Self::MoveForward,
        Self::MoveBack,
        Self::MoveLeft,
        Self::MoveRight,
        Self::Jump,
        Self::Inventory,
        Self::Minimap,
        Self::FullscreenMap,
        Self::QuickHeal,
        Self::QuickMana,
        Self::QuickBuff,
        Self::BreakWall,
        Self::EquipArmor,
        Self::Throw,
        Self::RulerPin,
    ];

    /// Key name used in `settings.cfg` and serde maps.
    pub const fn config_key(self) -> &'static str {
        match self {
            Self::MoveForward => "bind_move_forward",
            Self::MoveBack => "bind_move_back",
            Self::MoveLeft => "bind_move_left",
            Self::MoveRight => "bind_move_right",
            Self::Jump => "bind_jump",
            Self::Inventory => "bind_inventory",
            Self::Minimap => "bind_minimap",
            Self::FullscreenMap => "bind_fullscreen_map",
            Self::QuickHeal => "bind_quick_heal",
            Self::QuickMana => "bind_quick_mana",
            Self::QuickBuff => "bind_quick_buff",
            Self::BreakWall => "bind_break_wall",
            Self::EquipArmor => "bind_equip_armor",
            Self::Throw => "bind_throw",
            Self::RulerPin => "bind_ruler_pin",
        }
    }

    /// Human-readable label for UI display.
    pub const fn label(self) -> &'static str {
        match self {
            Self::MoveForward => "Move Forward",
            Self::MoveBack => "Move Back",
            Self::MoveLeft => "Move Left",
            Self::MoveRight => "Move Right",
            Self::Jump => "Jump",
            Self::Inventory => "Inventory",
            Self::Minimap => "Minimap",
            Self::FullscreenMap => "Fullscreen Map",
            Self::QuickHeal => "Quick Heal",
            Self::QuickMana => "Quick Mana",
            Self::QuickBuff => "Quick Buff",
            Self::BreakWall => "Break Wall",
            Self::EquipArmor => "Equip Armor",
            Self::Throw => "Throw Item",
            Self::RulerPin => "Pin Ruler Anchor",
        }
    }

    /// Default key binding for this action.
    pub const fn default_key(self) -> KeyCode {
        match self {
            Self::MoveForward => KeyCode::KeyW,
            Self::MoveBack => KeyCode::KeyS,
            Self::MoveLeft => KeyCode::KeyA,
            Self::MoveRight => KeyCode::KeyD,
            Self::Jump => KeyCode::Space,
            Self::Inventory => KeyCode::Escape,
            Self::Minimap => KeyCode::Tab,
            Self::FullscreenMap => KeyCode::KeyM,
            Self::QuickHeal => KeyCode::KeyH,
            Self::QuickMana => KeyCode::KeyJ,
            Self::QuickBuff => KeyCode::KeyB,
            Self::BreakWall => KeyCode::KeyX,
            Self::EquipArmor => KeyCode::KeyE,
            Self::Throw => KeyCode::KeyT,
            Self::RulerPin => KeyCode::KeyR,
        }
    }

    pub fn from_config_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.config_key() == key)
    }
}

/// Fixed-size `Copy` map from [`GameAction`] to [`KeyCode`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputBindings {
    keys: [KeyCode; GameAction::COUNT],
}

impl Default for InputBindings {
    fn default() -> Self {
        Self {
            keys: GameAction::ALL.map(GameAction::default_key),
        }
    }
}

impl InputBindings {
    /// Returns the currently bound key for `action`.
    pub const fn key(&self, action: GameAction) -> KeyCode {
        self.keys[action as usize]
    }

    /// Rebinds `action` to `key`. Returns `Err(conflict)` if another action is
    /// already bound to `key`.
    pub fn rebind(&mut self, action: GameAction, key: KeyCode) -> Result<(), GameAction> {
        if self.key(action) == key {
            return Ok(());
        }
        for other in GameAction::ALL {
            if other != action && self.key(other) == key {
                return Err(other);
            }
        }
        self.keys[action as usize] = key;
        Ok(())
    }

    /// Restores all bindings to their defaults.
    pub fn reset_defaults(&mut self) {
        *self = Self::default();
    }
}

impl Serialize for InputBindings {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(GameAction::COUNT))?;
        for action in GameAction::ALL {
            if let Some(name) = keycode_name(self.key(action)) {
                map.serialize_entry(action.config_key(), name)?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for InputBindings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BindingsVisitor;

        impl<'de> Visitor<'de> for BindingsVisitor {
            type Value = InputBindings;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map of input bindings")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<InputBindings, M::Error> {
                let mut bindings = InputBindings::default();
                while let Some((k, v)) = map.next_entry::<String, String>()? {
                    if let (Some(action), Some(code)) =
                        (GameAction::from_config_key(&k), parse_keycode(&v))
                    {
                        let _ = bindings.rebind(action, code);
                    }
                }
                Ok(bindings)
            }
        }

        deserializer.deserialize_map(BindingsVisitor)
    }
}

const KEY_TABLE: &[(KeyCode, &str)] = &[
    (KeyCode::KeyA, "KeyA"),
    (KeyCode::KeyB, "KeyB"),
    (KeyCode::KeyC, "KeyC"),
    (KeyCode::KeyD, "KeyD"),
    (KeyCode::KeyE, "KeyE"),
    (KeyCode::KeyF, "KeyF"),
    (KeyCode::KeyG, "KeyG"),
    (KeyCode::KeyH, "KeyH"),
    (KeyCode::KeyI, "KeyI"),
    (KeyCode::KeyJ, "KeyJ"),
    (KeyCode::KeyK, "KeyK"),
    (KeyCode::KeyL, "KeyL"),
    (KeyCode::KeyM, "KeyM"),
    (KeyCode::KeyN, "KeyN"),
    (KeyCode::KeyO, "KeyO"),
    (KeyCode::KeyP, "KeyP"),
    (KeyCode::KeyQ, "KeyQ"),
    (KeyCode::KeyR, "KeyR"),
    (KeyCode::KeyS, "KeyS"),
    (KeyCode::KeyT, "KeyT"),
    (KeyCode::KeyU, "KeyU"),
    (KeyCode::KeyV, "KeyV"),
    (KeyCode::KeyW, "KeyW"),
    (KeyCode::KeyX, "KeyX"),
    (KeyCode::KeyY, "KeyY"),
    (KeyCode::KeyZ, "KeyZ"),
    (KeyCode::Digit0, "Digit0"),
    (KeyCode::Digit1, "Digit1"),
    (KeyCode::Digit2, "Digit2"),
    (KeyCode::Digit3, "Digit3"),
    (KeyCode::Digit4, "Digit4"),
    (KeyCode::Digit5, "Digit5"),
    (KeyCode::Digit6, "Digit6"),
    (KeyCode::Digit7, "Digit7"),
    (KeyCode::Digit8, "Digit8"),
    (KeyCode::Digit9, "Digit9"),
    (KeyCode::Space, "Space"),
    (KeyCode::Escape, "Escape"),
    (KeyCode::Enter, "Enter"),
    (KeyCode::Tab, "Tab"),
    (KeyCode::Backspace, "Backspace"),
    (KeyCode::ShiftLeft, "ShiftLeft"),
    (KeyCode::ShiftRight, "ShiftRight"),
    (KeyCode::ControlLeft, "ControlLeft"),
    (KeyCode::ControlRight, "ControlRight"),
    (KeyCode::AltLeft, "AltLeft"),
    (KeyCode::AltRight, "AltRight"),
    (KeyCode::ArrowUp, "ArrowUp"),
    (KeyCode::ArrowDown, "ArrowDown"),
    (KeyCode::ArrowLeft, "ArrowLeft"),
    (KeyCode::ArrowRight, "ArrowRight"),
    (KeyCode::F1, "F1"),
    (KeyCode::F2, "F2"),
    (KeyCode::F3, "F3"),
    (KeyCode::F4, "F4"),
    (KeyCode::F5, "F5"),
    (KeyCode::F6, "F6"),
    (KeyCode::F7, "F7"),
    (KeyCode::F8, "F8"),
    (KeyCode::F9, "F9"),
    (KeyCode::F10, "F10"),
    (KeyCode::F11, "F11"),
    (KeyCode::F12, "F12"),
    (KeyCode::Minus, "Minus"),
    (KeyCode::Equal, "Equal"),
    (KeyCode::BracketLeft, "BracketLeft"),
    (KeyCode::BracketRight, "BracketRight"),
    (KeyCode::Backslash, "Backslash"),
    (KeyCode::Semicolon, "Semicolon"),
    (KeyCode::Quote, "Quote"),
    (KeyCode::Comma, "Comma"),
    (KeyCode::Period, "Period"),
    (KeyCode::Slash, "Slash"),
    (KeyCode::Backquote, "Backquote"),
    (KeyCode::Insert, "Insert"),
    (KeyCode::Delete, "Delete"),
    (KeyCode::Home, "Home"),
    (KeyCode::End, "End"),
    (KeyCode::PageUp, "PageUp"),
    (KeyCode::PageDown, "PageDown"),
];

/// Returns the canonical string name for `code` if supported.
pub fn keycode_name(code: KeyCode) -> Option<&'static str> {
    KEY_TABLE
        .iter()
        .find_map(|&(k, name)| (k == code).then_some(name))
}

/// Parses a key name produced by [`keycode_name`].
pub fn parse_keycode(name: &str) -> Option<KeyCode> {
    KEY_TABLE
        .iter()
        .find_map(|&(k, n)| (n == name).then_some(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::keyboard::KeyCode;

    #[test]
    fn input_bindings_rebind_detects_conflict_and_resets() {
        let mut bindings = InputBindings::default();
        assert_eq!(bindings.key(GameAction::MoveForward), KeyCode::KeyW);
        assert_eq!(bindings.key(GameAction::Jump), KeyCode::Space);

        // Rebinding Jump to an unused key succeeds.
        assert_eq!(bindings.rebind(GameAction::Jump, KeyCode::KeyU), Ok(()));
        assert_eq!(bindings.key(GameAction::Jump), KeyCode::KeyU);

        // Rebinding MoveForward to KeyU conflicts with Jump.
        assert_eq!(
            bindings.rebind(GameAction::MoveForward, KeyCode::KeyU),
            Err(GameAction::Jump)
        );
        // MoveForward remains unchanged after conflict.
        assert_eq!(bindings.key(GameAction::MoveForward), KeyCode::KeyW);

        // Rebinding an action to its own current key is a no-op Ok.
        assert_eq!(bindings.rebind(GameAction::Jump, KeyCode::KeyU), Ok(()));

        // Reset restores defaults.
        bindings.reset_defaults();
        assert_eq!(bindings.key(GameAction::Jump), KeyCode::Space);
    }

    #[test]
    fn settings_with_rebound_key_roundtrips() {
        let mut s = crate::settings::Settings::default();
        s.bindings.rebind(GameAction::Jump, KeyCode::KeyU).unwrap();
        s.bindings
            .rebind(GameAction::Minimap, KeyCode::KeyK)
            .unwrap();

        let dir = std::env::temp_dir().join("terravibe_test_input_map_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("settings.cfg");
        crate::settings::save(&file, &s).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        let parsed = crate::settings::parse(&text);
        assert_eq!(parsed.bindings.key(GameAction::Jump), KeyCode::KeyU);
        assert_eq!(parsed.bindings.key(GameAction::Minimap), KeyCode::KeyK);
        assert_eq!(parsed, s);

        // Serde backward-compatibility: JSON without `bindings` deserializes to defaults.
        let legacy: crate::settings::Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.bindings, InputBindings::default());

        // Serde roundtrip preserves rebound keys.
        let json = serde_json::to_string(&s).unwrap();
        let from_json: crate::settings::Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(from_json.bindings, s.bindings);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ruler_pin_default_and_no_collisions() {
        let bindings = InputBindings::default();
        assert_eq!(bindings.key(GameAction::RulerPin), KeyCode::KeyR);
        let mut seen = std::collections::HashSet::new();
        for action in GameAction::ALL {
            assert!(
                seen.insert(bindings.key(action)),
                "duplicate default binding for {:?}",
                action
            );
        }
    }
}
