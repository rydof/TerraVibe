//! Persisted client settings: window, audio and controls, stored as a plain
//! `key=value` file. Loaded before the `App` is built so every field can fall
//! back to its default individually — a missing, corrupt or out-of-range
//! config must never block launch.

use std::io;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use bevy::prelude::Resource;

use bevy::window::{
    MonitorSelection, PresentMode, VideoModeSelection, Window, WindowMode, WindowResolution,
};
use serde::{Deserialize, Serialize};

use crate::input_map::{GameAction, InputBindings, keycode_name, parse_keycode};

/// Saved resolutions below this are treated as corrupt (the window would be
/// unusable); there is no upper bound.
const MIN_DIMENSION: u32 = 640;

const MAX_VOLUME: u8 = 100;
const SENSITIVITY_RANGE: RangeInclusive<f32> = 0.25..=3.0;
const FOV_RANGE: RangeInclusive<f32> = 50.0..=100.0;
const SCREEN_SHAKE_RANGE: RangeInclusive<f32> = 0.0..=2.0;
const UI_SCALE_RANGE: RangeInclusive<f32> = 0.75..=1.5;
const HUD_OPACITY_RANGE: RangeInclusive<f32> = 0.25..=1.0;
const TOOLTIP_DELAY_RANGE: RangeInclusive<f32> = 0.0..=1.0;

const fn default_width() -> u32 {
    1920
}

const fn default_height() -> u32 {
    1080
}

const fn default_sound_volume() -> u8 {
    75
}

const fn default_music_volume() -> u8 {
    50
}

const fn default_sensitivity() -> f32 {
    1.0
}

const fn default_fov() -> f32 {
    70.0
}

const fn default_true() -> bool {
    true
}

const fn default_false() -> bool {
    false
}

const fn default_screen_shake() -> f32 {
    1.0
}

const fn default_ui_scale() -> f32 {
    1.0
}

const fn default_hud_opacity() -> f32 {
    1.0
}

const fn default_tooltip_delay() -> f32 {
    0.2
}

const fn default_ambient_volume() -> u8 {
    75
}

const fn default_ui_volume() -> u8 {
    75
}

/// Settings persistence file location. Defaults to [`config_path`] using process environment.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct SettingsPath(pub Option<PathBuf>);

impl Default for SettingsPath {
    fn default() -> Self {
        Self(config_path(|k| std::env::var(k).ok()))
    }
}

/// Window presentation the user picked in the Options screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowModeSetting {
    #[default]
    Windowed,
    Borderless,
    Fullscreen,
}

/// Graphics/effects quality preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityPreset {
    Low,
    #[default]
    Medium,
    High,
}

impl QualityPreset {
    /// Multiplier applied to particle and weather pool caps.
    pub fn particle_scale(self) -> f32 {
        match self {
            QualityPreset::Low => 0.35,
            QualityPreset::Medium => 0.7,
            QualityPreset::High => 1.0,
        }
    }
}

/// Everything the client persists between runs. Defaults are what a first
/// launch uses.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub window_mode: WindowModeSetting,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_sound_volume")]
    pub sound_volume: u8,
    #[serde(default = "default_music_volume")]
    pub music_volume: u8,
    #[serde(default = "default_sensitivity")]
    pub sensitivity: f32,
    #[serde(default = "default_fov")]
    pub fov: f32,
    #[serde(default = "default_true")]
    pub vsync: bool,
    #[serde(default = "default_false")]
    pub invert_y: bool,
    #[serde(default = "default_screen_shake")]
    pub screen_shake: f32,
    #[serde(default = "default_true")]
    pub hitstop: bool,
    #[serde(default = "default_true")]
    pub damage_numbers: bool,
    #[serde(default = "default_false")]
    pub reduced_motion: bool,
    #[serde(default = "default_true")]
    pub weather_effects: bool,
    #[serde(default = "default_true")]
    pub ambient_particles: bool,
    #[serde(default = "default_true")]
    pub atmosphere_fx: bool,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    #[serde(default = "default_hud_opacity")]
    pub hud_opacity: f32,
    #[serde(default = "default_tooltip_delay")]
    pub tooltip_delay: f32,
    #[serde(default = "default_false")]
    pub colorblind_palette: bool,
    #[serde(default = "default_false")]
    pub high_contrast_crosshair: bool,
    #[serde(default = "default_ambient_volume")]
    pub ambient_volume: u8,
    #[serde(default = "default_ui_volume")]
    pub ui_volume: u8,
    #[serde(default = "default_true")]
    pub mute_on_focus_loss: bool,
    #[serde(default)]
    pub quality_preset: QualityPreset,
    #[serde(default)]
    pub bindings: InputBindings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            window_mode: WindowModeSetting::Windowed,
            width: 1920,
            height: 1080,
            sound_volume: 75,
            music_volume: 50,
            sensitivity: 1.0,
            fov: 70.0,
            vsync: true,
            invert_y: false,
            screen_shake: 1.0,
            hitstop: true,
            damage_numbers: true,
            reduced_motion: false,
            weather_effects: true,
            ambient_particles: true,
            atmosphere_fx: true,
            ui_scale: 1.0,
            hud_opacity: 1.0,
            tooltip_delay: 0.2,
            colorblind_palette: false,
            high_contrast_crosshair: false,
            ambient_volume: 75,
            ui_volume: 75,
            mute_on_focus_loss: true,
            quality_preset: QualityPreset::Medium,
            bindings: InputBindings::default(),
        }
    }
}

/// `$XDG_CONFIG_HOME/terravibe/settings.cfg`, else `$HOME/.config/terravibe/settings.cfg`,
/// else `%APPDATA%\terravibe\settings.cfg`, else `None`. `env` is injected so
/// tests can point the lookup at a temp dir; an empty value counts as unset.
pub fn config_path(env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    for (key, dir) in [
        ("XDG_CONFIG_HOME", "terravibe"),
        ("HOME", ".config/terravibe"),
        ("APPDATA", "terravibe"),
    ] {
        if let Some(base) = env(key).filter(|b| !b.is_empty()) {
            return Some(Path::new(&base).join(dir).join("settings.cfg"));
        }
    }
    None
}

/// Loads settings from [`config_path`]. Never fails: a missing or unreadable
/// file gives defaults, a corrupt one degrades field by field via [`parse`].
pub fn load(env: impl Fn(&str) -> Option<String>) -> Settings {
    let Some(path) = config_path(env) else {
        return Settings::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => parse(&text),
        Err(_) => Settings::default(),
    }
}

/// Parses `key=value` lines. Unknown keys are ignored; each field that is
/// missing, unparsable or out of range keeps its default, so a corrupt file
/// degrades field by field instead of failing the launch.
pub fn parse(text: &str) -> Settings {
    let mut s = Settings::default();
    let mut file_minimap = None;
    let mut file_fullscreen = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue; // not a `key=value` line: ignore
        };
        let value = value.trim();
        match key.trim() {
            "window_mode" => s.window_mode = parse_mode(value).unwrap_or(s.window_mode),
            "width" => s.width = parse_dimension(value).unwrap_or(s.width),
            "height" => s.height = parse_dimension(value).unwrap_or(s.height),
            "sound_volume" => s.sound_volume = parse_volume(value).unwrap_or(s.sound_volume),
            "music_volume" => s.music_volume = parse_volume(value).unwrap_or(s.music_volume),
            "sensitivity" => {
                s.sensitivity = parse_ranged(value, SENSITIVITY_RANGE).unwrap_or(s.sensitivity)
            }
            "fov" => s.fov = parse_ranged(value, FOV_RANGE).unwrap_or(s.fov),
            "vsync" => s.vsync = value.parse().unwrap_or(s.vsync),
            "invert_y" => s.invert_y = value.parse().unwrap_or(s.invert_y),
            "screen_shake" => {
                s.screen_shake = parse_ranged(value, SCREEN_SHAKE_RANGE).unwrap_or(s.screen_shake)
            }
            "hitstop" => s.hitstop = value.parse().unwrap_or(s.hitstop),
            "damage_numbers" => s.damage_numbers = value.parse().unwrap_or(s.damage_numbers),
            "reduced_motion" => s.reduced_motion = value.parse().unwrap_or(s.reduced_motion),
            "weather_effects" => s.weather_effects = value.parse().unwrap_or(s.weather_effects),
            "ambient_particles" => {
                s.ambient_particles = value.parse().unwrap_or(s.ambient_particles)
            }
            "atmosphere_fx" => s.atmosphere_fx = value.parse().unwrap_or(s.atmosphere_fx),
            "ui_scale" => s.ui_scale = parse_ranged(value, UI_SCALE_RANGE).unwrap_or(s.ui_scale),
            "hud_opacity" => {
                s.hud_opacity = parse_ranged(value, HUD_OPACITY_RANGE).unwrap_or(s.hud_opacity)
            }
            "tooltip_delay" => {
                s.tooltip_delay =
                    parse_ranged(value, TOOLTIP_DELAY_RANGE).unwrap_or(s.tooltip_delay)
            }
            "colorblind_palette" => {
                s.colorblind_palette = value.parse().unwrap_or(s.colorblind_palette)
            }
            "high_contrast_crosshair" => {
                s.high_contrast_crosshair = value.parse().unwrap_or(s.high_contrast_crosshair)
            }
            "ambient_volume" => s.ambient_volume = parse_volume(value).unwrap_or(s.ambient_volume),
            "ui_volume" => s.ui_volume = parse_volume(value).unwrap_or(s.ui_volume),
            "mute_on_focus_loss" => {
                s.mute_on_focus_loss = value.parse().unwrap_or(s.mute_on_focus_loss)
            }
            "quality_preset" => {
                s.quality_preset = parse_quality_preset(value).unwrap_or(s.quality_preset)
            }
            other => {
                if let (Some(action), Some(code)) =
                    (GameAction::from_config_key(other), parse_keycode(value))
                {
                    match action {
                        GameAction::Minimap => file_minimap = Some(code),
                        GameAction::FullscreenMap => file_fullscreen = Some(code),
                        _ => {}
                    }
                    let _ = s.bindings.rebind(action, code);
                }
            }
        }
    }
    // Migrate pre-Terraria defaults (M=minimap, N=fullscreen) to Terraria defaults (Tab, M).
    if file_minimap == Some(bevy::prelude::KeyCode::KeyM)
        && file_fullscreen == Some(bevy::prelude::KeyCode::KeyN)
    {
        let _ = s
            .bindings
            .rebind(GameAction::Minimap, bevy::prelude::KeyCode::Tab);
        let _ = s
            .bindings
            .rebind(GameAction::FullscreenMap, bevy::prelude::KeyCode::KeyM);
    }
    s
}

fn parse_mode(value: &str) -> Option<WindowModeSetting> {
    match value {
        "windowed" => Some(WindowModeSetting::Windowed),
        "borderless" => Some(WindowModeSetting::Borderless),
        "fullscreen" => Some(WindowModeSetting::Fullscreen),
        _ => None,
    }
}

fn parse_quality_preset(value: &str) -> Option<QualityPreset> {
    match value {
        "low" => Some(QualityPreset::Low),
        "medium" => Some(QualityPreset::Medium),
        "high" => Some(QualityPreset::High),
        _ => None,
    }
}

fn parse_dimension(value: &str) -> Option<u32> {
    value.parse::<u32>().ok().filter(|v| *v >= MIN_DIMENSION)
}

fn parse_volume(value: &str) -> Option<u8> {
    value.parse::<u8>().ok().filter(|v| *v <= MAX_VOLUME)
}

fn parse_ranged(value: &str, range: RangeInclusive<f32>) -> Option<f32> {
    value.parse::<f32>().ok().filter(|v| range.contains(v))
}

/// Serializes to plain `key=value` lines (the mirror image of [`parse`]) and
/// writes atomically, so a crash mid-save can't corrupt the existing config.
pub fn save(path: &Path, s: &Settings) -> io::Result<()> {
    let mut text = format!(
        "window_mode={}\nwidth={}\nheight={}\nsound_volume={}\nmusic_volume={}\n\
         sensitivity={}\nfov={}\nvsync={}\ninvert_y={}\n\
         screen_shake={}\nhitstop={}\ndamage_numbers={}\nreduced_motion={}\n\
         weather_effects={}\nambient_particles={}\natmosphere_fx={}\n\
         ui_scale={}\nhud_opacity={}\ntooltip_delay={}\ncolorblind_palette={}\n\
         high_contrast_crosshair={}\nambient_volume={}\nui_volume={}\n\
         mute_on_focus_loss={}\nquality_preset={}\n",
        mode_key(s.window_mode),
        s.width,
        s.height,
        s.sound_volume,
        s.music_volume,
        s.sensitivity,
        s.fov,
        s.vsync,
        s.invert_y,
        s.screen_shake,
        s.hitstop,
        s.damage_numbers,
        s.reduced_motion,
        s.weather_effects,
        s.ambient_particles,
        s.atmosphere_fx,
        s.ui_scale,
        s.hud_opacity,
        s.tooltip_delay,
        s.colorblind_palette,
        s.high_contrast_crosshair,
        s.ambient_volume,
        s.ui_volume,
        s.mute_on_focus_loss,
        quality_key(s.quality_preset),
    );
    for action in GameAction::ALL {
        if let Some(name) = keycode_name(s.bindings.key(action)) {
            text.push_str(&format!("{}={}\n", action.config_key(), name));
        }
    }
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    atomic_write(path, text.as_bytes())
}

fn mode_key(mode: WindowModeSetting) -> &'static str {
    match mode {
        WindowModeSetting::Windowed => "windowed",
        WindowModeSetting::Borderless => "borderless",
        WindowModeSetting::Fullscreen => "fullscreen",
    }
}

fn quality_key(preset: QualityPreset) -> &'static str {
    match preset {
        QualityPreset::Low => "low",
        QualityPreset::Medium => "medium",
        QualityPreset::High => "high",
    }
}

/// Write `bytes` to `path` via a temp file and rename; a failed write leaves
/// the old file (if any) untouched and no partial file behind. Local copy of
/// the server's crate-private `save::atomic_write`.
fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    let res = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// Linear gain for Bevy's `GlobalVolume` (0.0..=1.0).
pub fn volume_gain(volume: u8) -> f32 {
    volume as f32 / 100.0
}

/// Applies settings to an existing Window resource/component.
pub fn apply_to_window(s: &Settings, window: &mut Window) {
    window.mode = bevy_mode(s.window_mode);
    window.present_mode = present_mode(s.vsync);
    if s.window_mode == WindowModeSetting::Windowed {
        window.resolution.set(s.width as f32, s.height as f32);
    }
}

/// `AutoVsync`/`AutoNoVsync`, letting the driver fall back gracefully.
pub fn present_mode(vsync: bool) -> PresentMode {
    if vsync {
        PresentMode::AutoVsync
    } else {
        PresentMode::AutoNoVsync
    }
}

/// The window the game normally opens with. Automated runs (bench,
/// screenshot, demo) build their own forced 1920x1080 window instead.
pub fn window_for(s: &Settings) -> Window {
    Window {
        title: "TerraVibe".into(),
        resolution: WindowResolution::new(s.width, s.height),
        present_mode: present_mode(s.vsync),
        mode: bevy_mode(s.window_mode),
        ..Default::default()
    }
}

/// Maps the saved setting onto Bevy's window mode. `Current` monitor because
/// a pure builder has no `Monitor` entity handles yet.
fn bevy_mode(mode: WindowModeSetting) -> WindowMode {
    match mode {
        WindowModeSetting::Windowed => WindowMode::Windowed,
        WindowModeSetting::Borderless => {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        }
        WindowModeSetting::Fullscreen => {
            WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// A fresh temp dir per test: parallel tests never share state, and the
    /// real user config is never touched.
    fn dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("terravibe-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d); // start clean even after a crashed run
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Where `save` (and `load` via the `env` closure below) put the config.
    fn path(name: &str) -> PathBuf {
        dir(name).join("terravibe").join("settings.cfg")
    }

    /// Points `load`'s config lookup at a temp `XDG_CONFIG_HOME`.
    fn env(k: &str) -> Option<String> {
        (k == "XDG_CONFIG_HOME").then(|| dir("load").to_string_lossy().into())
    }

    /// Every field different from `Default` yet valid, so a roundtrip proves
    /// each field individually survives the file.
    fn random_but_valid() -> Settings {
        Settings {
            window_mode: WindowModeSetting::Borderless,
            width: 1280,
            height: 720,
            sound_volume: 40,
            music_volume: 60,
            sensitivity: 1.5,
            fov: 90.0,
            vsync: false,
            invert_y: true,
            screen_shake: 0.5,
            hitstop: false,
            damage_numbers: false,
            reduced_motion: true,
            weather_effects: false,
            ambient_particles: false,
            atmosphere_fx: false,
            ui_scale: 1.25,
            hud_opacity: 0.75,
            tooltip_delay: 0.5,
            colorblind_palette: true,
            high_contrast_crosshair: true,
            ambient_volume: 35,
            ui_volume: 90,
            mute_on_focus_loss: false,
            quality_preset: QualityPreset::High,
            bindings: InputBindings::default(),
        }
    }

    #[test]
    fn roundtrip_preserves_every_field() {
        let s = random_but_valid();
        let file = path("roundtrip");
        save(&file, &s).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert_eq!(parse(&text), s);
    }

    #[test]
    fn corrupt_file_falls_back_field_by_field() {
        let text = "window_mode=Banana\nheight=1080\nsensitivity=9.0\nfov=10.0\nsound_volume=200\nscreen_shake=9.0\ngarbage line\n";
        let s = parse(text); // width missing, height ok, mode/volume bad, sensitivity/fov/shake out of range
        assert_eq!(
            (
                s.window_mode,
                s.width,
                s.height,
                s.sound_volume,
                s.sensitivity,
                s.fov,
                s.screen_shake,
                s.hitstop,
                s.damage_numbers,
                s.reduced_motion,
            ),
            (
                WindowModeSetting::Windowed,
                1920,
                1080,
                75,
                1.0,
                70.0,
                1.0,
                true,
                true,
                false
            )
        );
    }

    #[test]
    fn unknown_keys_are_ignored() {
        assert_eq!(parse("frobnicate=1\nvsync=false\n").vsync, false);
    }

    #[test]
    fn load_missing_file_gives_defaults() {
        assert_eq!(load(env), Settings::default());
    }

    #[test]
    fn volume_maps_to_linear_gain_and_vsync_to_present_mode() {
        assert_eq!(volume_gain(75), 0.75);
        assert_eq!(present_mode(true), PresentMode::AutoVsync);
        assert_eq!(present_mode(false), PresentMode::AutoNoVsync);
    }

    #[test]
    fn music_volume_setting_roundtrips_and_old_file_defaults_50() {
        // Old settings file without music_volume: defaults to 50
        let old_text = "window_mode=windowed\nwidth=1920\nheight=1080\nsound_volume=80\n";
        let s = parse(old_text);
        assert_eq!(s.music_volume, 50);

        // Roundtrip with music_volume
        let p = path("music_roundtrip");
        let mut original = Settings::default();
        original.music_volume = 40;
        save(&p, &original).unwrap();
        let loaded = parse(&std::fs::read_to_string(&p).unwrap());
        assert_eq!(loaded.music_volume, 40);
        let _ = std::fs::remove_dir_all(dir("music_roundtrip"));
    }

    #[test]
    fn window_for_uses_mode_and_size() {
        let w = window_for(&Settings {
            window_mode: WindowModeSetting::Windowed,
            width: 1280,
            height: 720,
            ..Default::default()
        });
        assert_eq!(
            (w.resolution.width(), w.resolution.height(), w.present_mode),
            (1280., 720., PresentMode::AutoVsync)
        );
    }

    #[test]
    fn settings_combat_feedback_defaults_and_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<Settings>();

        let s = Settings::default();
        assert_eq!(s.screen_shake, 1.0);
        assert!(s.hitstop);
        assert!(s.damage_numbers);
        assert!(!s.reduced_motion);

        // Roundtrip through JSON
        let serialized = serde_json::to_string(&s).unwrap();
        let deserialized: Settings = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, s);

        // Deserializing old settings JSON missing the new fields uses defaults
        let old_json = r#"{
            "window_mode": "Windowed",
            "width": 1920,
            "height": 1080,
            "sound_volume": 75,
            "music_volume": 50,
            "sensitivity": 1.0,
            "fov": 70.0,
            "vsync": true,
            "invert_y": false
        }"#;
        let loaded_old: Settings = serde_json::from_str(old_json).unwrap();
        assert_eq!(loaded_old.screen_shake, 1.0);
        assert!(loaded_old.hitstop);
        assert!(loaded_old.damage_numbers);
        assert!(!loaded_old.reduced_motion);
    }

    #[test]
    fn save_is_atomic_no_tmp_left_behind() {
        let file = path("atomic");
        save(&file, &Settings::default()).unwrap();
        assert!(file.exists());
        assert!(!file.with_extension("tmp").exists());
    }

    #[test]
    fn settings_weather_and_atmosphere_defaults_and_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<Settings>();

        let s = Settings::default();
        assert!(s.weather_effects);
        assert!(s.ambient_particles);
        assert!(s.atmosphere_fx);

        let old_json = r#"{"window_mode":"Windowed","width":1920,"height":1080}"#;
        let loaded: Settings = serde_json::from_str(old_json).unwrap();
        assert!(loaded.weather_effects);
        assert!(loaded.ambient_particles);
        assert!(loaded.atmosphere_fx);
    }

    #[test]
    fn settings_pass5_fields_default_and_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<Settings>();
        assert_copy::<QualityPreset>();

        let s = Settings::default();
        assert_eq!(s.ui_scale, 1.0);
        assert_eq!(s.hud_opacity, 1.0);
        assert_eq!(s.tooltip_delay, 0.2);
        assert!(!s.colorblind_palette);
        assert!(!s.high_contrast_crosshair);
        assert_eq!(s.ambient_volume, 75);
        assert_eq!(s.ui_volume, 75);
        assert!(s.mute_on_focus_loss);
        assert_eq!(s.quality_preset, QualityPreset::Medium);

        let old_json = r#"{"window_mode":"Windowed","width":1920,"height":1080}"#;
        let loaded: Settings = serde_json::from_str(old_json).unwrap();
        assert_eq!(loaded.ui_scale, 1.0);
        assert_eq!(loaded.hud_opacity, 1.0);
        assert_eq!(loaded.tooltip_delay, 0.2);
        assert!(!loaded.colorblind_palette);
        assert!(!loaded.high_contrast_crosshair);
        assert_eq!(loaded.ambient_volume, 75);
        assert_eq!(loaded.ui_volume, 75);
        assert!(loaded.mute_on_focus_loss);
        assert_eq!(loaded.quality_preset, QualityPreset::Medium);
    }

    #[test]
    fn legacy_map_keybinds_migrate_to_terraria_defaults() {
        use bevy::prelude::KeyCode;

        let legacy_cfg = "window_mode=windowed\nbind_minimap=KeyM\nbind_fullscreen_map=KeyN\n";
        let migrated = parse(legacy_cfg);
        assert_eq!(migrated.bindings.key(GameAction::Minimap), KeyCode::Tab);
        assert_eq!(
            migrated.bindings.key(GameAction::FullscreenMap),
            KeyCode::KeyM
        );

        // Non-legacy custom binds must be preserved without migration.
        let custom_cfg = "bind_minimap=KeyK\nbind_fullscreen_map=KeyN\n";
        let custom = parse(custom_cfg);
        assert_eq!(custom.bindings.key(GameAction::Minimap), KeyCode::KeyK);
        assert_eq!(
            custom.bindings.key(GameAction::FullscreenMap),
            KeyCode::KeyN
        );
    }
}
