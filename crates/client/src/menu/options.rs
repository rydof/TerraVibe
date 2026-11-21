//! Options screen: video, audio, controls, and persistence.

use bevy::audio::GlobalVolume;
use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryMonitor, PrimaryWindow, Window};

use super::style::{
    BUTTON_HEIGHT, BUTTON_WIDTH, COLOR_BACKDROP, COLOR_PANEL_BG, COLOR_PANEL_BORDER,
    COLOR_PANEL_INNER, COLOR_SLIDER_FILL, COLOR_TEXT_GOLD, MenuSelected, spawn_button_sized,
    spawn_slider_bar,
};
use super::text::{MenuText, MenuTextColor};
use super::{Action, Setting};
use crate::input_map::{GameAction, keycode_name};
use crate::session::{GameMenu, GamePanel, MenuScreen};
use crate::settings::{self, QualityPreset, Settings, SettingsPath, WindowModeSetting};
use crate::view::camera::{FlyCamera, LookSensitivity};

pub const RESOLUTION_PRESETS: [(u32, u32); 4] =
    [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440)];

pub const BINDINGS_COPY: &str = "Controls: move WASD, jump Space, attack/use left mouse, throw item T, inventory and crafting Esc, camera right mouse, game menu via Settings in the inventory";

/// Formats a dynamic summary of current key bindings.
pub fn format_bindings_copy(bindings: &crate::input_map::InputBindings) -> String {
    let k = |a: GameAction| keycode_name(bindings.key(a)).unwrap_or("?");
    format!(
        "Controls: move {}{}{}{}, jump {}, attack/use left mouse, throw item {}, inventory and crafting {}, camera right mouse, game menu via Settings in the inventory",
        k(GameAction::MoveForward),
        k(GameAction::MoveLeft),
        k(GameAction::MoveBack),
        k(GameAction::MoveRight),
        k(GameAction::Jump),
        k(GameAction::Throw),
        k(GameAction::Inventory),
    )
}

/// Identifies whether the Options screen is hosted under Title menu or InGame pause menu.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OptionsHost {
    #[default]
    Title,
    GameMenu,
}

/// Category tabs in the grouped Options screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum OptionsTab {
    #[default]
    Display,
    Audio,
    Controls,
    Accessibility,
}

impl OptionsTab {
    pub const ALL: [OptionsTab; 4] = [
        Self::Display,
        Self::Audio,
        Self::Controls,
        Self::Accessibility,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Display => "Display",
            Self::Audio => "Audio",
            Self::Controls => "Controls",
            Self::Accessibility => "Accessibility",
        }
    }
}

/// Ephemeral UI state for the Options screen (active tab, key-rebinding listen target, conflict feedback).
#[derive(Resource, Clone, Debug, PartialEq, Eq, Default)]
pub struct OptionsUiState {
    pub active_tab: OptionsTab,
    pub listening_action: Option<GameAction>,
    pub conflict_message: Option<String>,
}

/// Cycle quality preset: Low -> Medium -> High -> Low.
pub fn cycle_quality_preset(preset: QualityPreset) -> QualityPreset {
    match preset {
        QualityPreset::Low => QualityPreset::Medium,
        QualityPreset::Medium => QualityPreset::High,
        QualityPreset::High => QualityPreset::Low,
    }
}

/// Cycle window mode: Windowed -> Borderless -> Fullscreen -> Windowed.
pub fn cycle_window_mode(mode: WindowModeSetting) -> WindowModeSetting {
    match mode {
        WindowModeSetting::Windowed => WindowModeSetting::Borderless,
        WindowModeSetting::Borderless => WindowModeSetting::Fullscreen,
        WindowModeSetting::Fullscreen => WindowModeSetting::Windowed,
    }
}

/// Cycle resolution presets (1280x720 -> 1600x900 -> 1920x1080 -> 2560x1440 -> 1280x720).
/// Unknown resolutions wrap to the first preset.
pub fn cycle_resolution(w: u32, h: u32) -> (u32, u32) {
    if let Some(pos) = RESOLUTION_PRESETS
        .iter()
        .position(|&(pw, ph)| pw == w && ph == h)
    {
        let next_idx = (pos + 1) % RESOLUTION_PRESETS.len();
        RESOLUTION_PRESETS[next_idx]
    } else {
        RESOLUTION_PRESETS[0]
    }
}

/// Nudge volume by step (step 5, clamp 0..=100).
pub fn nudge_volume(current: i32, step: i32) -> u8 {
    (current + step).clamp(0, 100) as u8
}

/// Nudge sensitivity by step (step 0.25, clamp 0.25..=3.0).
pub fn nudge_sensitivity(current: f32, step: i32) -> f32 {
    let raw = current + step as f32 * 0.25;
    // Round to nearest 0.01 to avoid floating-point drift
    let rounded = (raw * 100.0).round() / 100.0;
    rounded.clamp(0.25, 3.0)
}

/// Nudge FOV by step (step 5, clamp 50..=100).
pub fn nudge_fov(current: f32, step: i32) -> f32 {
    let raw = current + step as f32 * 5.0;
    let rounded = (raw * 10.0).round() / 10.0;
    rounded.clamp(50.0, 100.0)
}

/// Nudge UI scale by step (step 0.05, clamp 0.75..=1.5).
pub fn nudge_ui_scale(current: f32, step: i32) -> f32 {
    let raw = current + step as f32 * 0.05;
    let rounded = (raw * 100.0).round() / 100.0;
    rounded.clamp(0.75, 1.5)
}

/// Nudge HUD opacity by step (step 0.05, clamp 0.25..=1.0).
pub fn nudge_hud_opacity(current: f32, step: i32) -> f32 {
    let raw = current + step as f32 * 0.05;
    let rounded = (raw * 100.0).round() / 100.0;
    rounded.clamp(0.25, 1.0)
}

/// Nudge tooltip delay by step (step 0.1s, clamp 0.0..=1.0).
pub fn nudge_tooltip_delay(current: f32, step: i32) -> f32 {
    let raw = current + step as f32 * 0.1;
    let rounded = (raw * 100.0).round() / 100.0;
    rounded.clamp(0.0, 1.0)
}

/// Nudge screen shake intensity by step (step 0.25, clamp 0.0..=2.0).
pub fn nudge_screen_shake(current: f32, step: i32) -> f32 {
    let raw = current + step as f32 * 0.25;
    let rounded = (raw * 100.0).round() / 100.0;
    rounded.clamp(0.0, 2.0)
}

/// Options root and the window center used to position it.
#[derive(Component)]
pub struct OptionsUiRoot(f32);

fn recenter_options(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut roots: Query<(&mut Node, &mut OptionsUiRoot)>,
) {
    let Ok(window) = windows.single() else { return };
    let center = window.width() / 2.0;
    for (mut node, mut root) in &mut roots {
        let delta = center - root.0;
        if delta != 0.0 {
            if let Val::Px(ref mut left) = node.left {
                *left += delta;
            }
            root.0 = center;
        }
    }
}

/// Helper to persist settings safely to disk.
pub fn persist_settings(path: &SettingsPath, settings: &Settings) {
    if let Some(ref p) = path.0 {
        if let Err(e) = settings::save(p, settings) {
            warn!("Failed to save settings to {}: {e}", p.display());
        }
    }
}

/// Format mode display string.
fn format_mode(mode: WindowModeSetting) -> &'static str {
    match mode {
        WindowModeSetting::Windowed => "Windowed",
        WindowModeSetting::Borderless => "Borderless",
        WindowModeSetting::Fullscreen => "Fullscreen",
    }
}

fn format_quality_preset(preset: QualityPreset) -> &'static str {
    match preset {
        QualityPreset::Low => "Low",
        QualityPreset::Medium => "Medium",
        QualityPreset::High => "High",
    }
}

fn on_off(val: bool) -> &'static str {
    if val { "On" } else { "Off" }
}

struct UiSpawnCtx {
    center_x: f32,
    is_game_menu: bool,
    rank: u16,
}

impl UiSpawnCtx {
    fn tag(&self, commands: &mut Commands, entity: Entity, z: i32) {
        commands
            .entity(entity)
            .insert((OptionsUiRoot(self.center_x), GlobalZIndex(z)));
        if !self.is_game_menu {
            commands
                .entity(entity)
                .insert(DespawnOnExit(MenuScreen::Options));
        }
    }

    fn spawn_btn(
        &mut self,
        commands: &mut Commands,
        label: &str,
        action: Action,
        pos: (f32, f32),
        size: (f32, f32),
        disabled: bool,
    ) -> Entity {
        let b = spawn_button_sized(commands, label, action, self.rank, pos, size, disabled);
        self.tag(commands, b, 1);
        self.rank += 1;
        b
    }

    fn spawn_toggle(
        &mut self,
        commands: &mut Commands,
        name: &str,
        value: bool,
        action: Action,
        pos: (f32, f32),
        size: (f32, f32),
    ) {
        let text = format!("{name}: {}", on_off(value));
        let b = self.spawn_btn(commands, &text, action, pos, size, false);
        if value {
            commands.entity(b).insert(MenuSelected(true));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_adj(
        &mut self,
        commands: &mut Commands,
        label: &str,
        fraction: f32,
        dec_act: Action,
        inc_act: Action,
        pos: (f32, f32),
        total_w: f32,
        row_h: f32,
    ) {
        let arrow_w = 40.0;
        let gap = 6.0;
        let label_w = (total_w - arrow_w * 2.0 - gap * 2.0).max(80.0);
        let dec_x = pos.0;
        let lbl_x = dec_x + arrow_w + gap;
        let inc_x = lbl_x + label_w + gap;

        self.spawn_btn(
            commands,
            "<",
            dec_act,
            (dec_x, pos.1),
            (arrow_w, row_h),
            false,
        );

        let bar_w = (label_w - 20.0).max(40.0);
        let val_container = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(lbl_x),
                    top: Val::Px(pos.1),
                    width: Val::Px(label_w),
                    height: Val::Px(row_h),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    padding: UiRect::bottom(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(COLOR_PANEL_INNER),
                BorderColor::all(COLOR_PANEL_BORDER),
            ))
            .with_children(|p| {
                p.spawn((
                    Node {
                        position_type: PositionType::Relative,
                        ..default()
                    },
                    MenuText::new(label, 0.9),
                ));
                p.spawn(Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0),
                    bottom: Val::Px(4.0),
                    ..default()
                })
                .with_children(|bar_wrap| {
                    spawn_slider_bar(bar_wrap, bar_w, 5.0, fraction, COLOR_SLIDER_FILL);
                });
            })
            .id();
        self.tag(commands, val_container, 1);

        self.spawn_btn(
            commands,
            ">",
            inc_act,
            (inc_x, pos.1),
            (arrow_w, row_h),
            false,
        );
    }
}

/// Spawns the options screen elements into the world using default UI state.
pub fn setup_options(
    commands: &mut Commands,
    settings: &Settings,
    host: Option<OptionsHost>,
    primary_window: &Query<&Window, With<PrimaryWindow>>,
    primary_monitor: &Query<&Monitor, With<PrimaryMonitor>>,
    all_monitors: &Query<&Monitor>,
) {
    setup_options_with_state(
        commands,
        settings,
        &OptionsUiState::default(),
        host,
        primary_window,
        primary_monitor,
        all_monitors,
    );
}

/// Spawns the grouped options screen elements into the world for the given UI state.
pub fn setup_options_with_state(
    commands: &mut Commands,
    settings: &Settings,
    state: &OptionsUiState,
    host: Option<OptionsHost>,
    primary_window: &Query<&Window, With<PrimaryWindow>>,
    primary_monitor: &Query<&Monitor, With<PrimaryMonitor>>,
    all_monitors: &Query<&Monitor>,
) {
    let is_game_menu = host == Some(OptionsHost::GameMenu);
    let center_x = primary_window
        .iter()
        .next()
        .map_or(960.0, |w| w.width() / 2.0);
    let mut ctx = UiSpawnCtx {
        center_x,
        is_game_menu,
        rank: 0,
    };

    let subtitle = if let Some(ref msg) = state.conflict_message {
        msg.clone()
    } else if let Some(act) = state.listening_action {
        format!("Press a key to bind '{}'...", act.label())
    } else {
        "Select a category tab below".to_string()
    };

    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(COLOR_BACKDROP),
        ))
        .id();
    ctx.tag(commands, backdrop, -1);

    // Framed content panel card behind the tabs & controls (Row direction so layout test's header query stays unique)
    let content_card = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(center_x - 380.0),
                top: Val::Px(154.0),
                width: Val::Px(760.0),
                height: Val::Px(532.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(COLOR_PANEL_BG),
            BorderColor::all(COLOR_PANEL_BORDER),
        ))
        .id();
    ctx.tag(commands, content_card, 0);

    let container = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(center_x - 380.0),
                top: Val::Px(24.0),
                width: Val::Px(760.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(COLOR_PANEL_BG),
            BorderColor::all(COLOR_PANEL_BORDER),
        ))
        .id();
    ctx.tag(commands, container, 0);

    commands.entity(container).with_children(|p| {
        p.spawn((
            Node {
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
            MenuText::new("Options", 1.35),
            MenuTextColor(COLOR_TEXT_GOLD),
        ));
        p.spawn((
            Node {
                max_width: Val::Px(700.0),
                margin: UiRect::ZERO,
                ..default()
            },
            MenuText::new(&subtitle, 0.8),
        ));
    });

    // Category tab bar (4 tabs with MenuSelected styling)
    let tab_w = 160.0;
    let tab_gap = 12.0;
    let tab_total_w = tab_w * 4.0 + tab_gap * 3.0;
    let tab_start_x = center_x - tab_total_w / 2.0;
    let tab_y = 114.0;
    for (idx, tab) in OptionsTab::ALL.into_iter().enumerate() {
        let x = tab_start_x + idx as f32 * (tab_w + tab_gap);
        let b = ctx.spawn_btn(
            commands,
            tab.label(),
            Action::SelectOptionsTab(tab),
            (x, tab_y),
            (tab_w, 36.0),
            false,
        );
        commands
            .entity(b)
            .insert(MenuSelected(tab == state.active_tab));
    }

    let content_y = 168.0;
    let col_w = 320.0;
    let left_col_x = center_x - col_w - 20.0;
    let right_col_x = center_x + 20.0;
    let center_col_x = center_x - col_w / 2.0;
    let step_y = 50.0;

    match state.active_tab {
        OptionsTab::Display => {
            let res_text = if settings.window_mode == WindowModeSetting::Windowed {
                format!("Resolution: {}x{}", settings.width, settings.height)
            } else {
                let native_size = primary_monitor
                    .iter()
                    .next()
                    .or_else(|| all_monitors.iter().next())
                    .map(|m| (m.physical_width, m.physical_height))
                    .or_else(|| {
                        primary_window.iter().next().map(|w| {
                            (
                                w.resolution.physical_width(),
                                w.resolution.physical_height(),
                            )
                        })
                    })
                    .unwrap_or((settings.width, settings.height));
                format!("Resolution: {}x{} (Native)", native_size.0, native_size.1)
            };
            let res_disabled = settings.window_mode != WindowModeSetting::Windowed;

            // Left column: window & quality
            ctx.spawn_btn(
                commands,
                &format!("Window Mode: {}", format_mode(settings.window_mode)),
                Action::CycleWindowMode,
                (left_col_x, content_y),
                (col_w, BUTTON_HEIGHT),
                false,
            );
            ctx.spawn_btn(
                commands,
                &res_text,
                Action::CycleResolution,
                (left_col_x, content_y + step_y),
                (col_w, BUTTON_HEIGHT),
                res_disabled,
            );
            ctx.spawn_toggle(
                commands,
                "VSync",
                settings.vsync,
                Action::ToggleVsync,
                (left_col_x, content_y + step_y * 2.0),
                (col_w, BUTTON_HEIGHT),
            );
            ctx.spawn_btn(
                commands,
                &format!(
                    "Quality: {}",
                    format_quality_preset(settings.quality_preset)
                ),
                Action::CycleQualityPreset,
                (left_col_x, content_y + step_y * 3.0),
                (col_w, BUTTON_HEIGHT),
                false,
            );
            ctx.spawn_adj(
                commands,
                &format!("FOV: {:.0} deg", settings.fov),
                (settings.fov - 50.0) / 50.0,
                Action::Adjust(Setting::Fov, -1),
                Action::Adjust(Setting::Fov, 1),
                (left_col_x, content_y + step_y * 4.0),
                col_w,
                BUTTON_HEIGHT,
            );

            // Right column: effects & HUD scale/opacity
            ctx.spawn_toggle(
                commands,
                "Weather Effects",
                settings.weather_effects,
                Action::ToggleWeatherEffects,
                (right_col_x, content_y),
                (col_w, BUTTON_HEIGHT),
            );
            ctx.spawn_toggle(
                commands,
                "Ambient Particles",
                settings.ambient_particles,
                Action::ToggleAmbientParticles,
                (right_col_x, content_y + step_y),
                (col_w, BUTTON_HEIGHT),
            );
            ctx.spawn_toggle(
                commands,
                "Atmosphere FX",
                settings.atmosphere_fx,
                Action::ToggleAtmosphereFx,
                (right_col_x, content_y + step_y * 2.0),
                (col_w, BUTTON_HEIGHT),
            );
            ctx.spawn_adj(
                commands,
                &format!("UI Scale: {:.2}x", settings.ui_scale),
                (settings.ui_scale - 0.75) / 0.75,
                Action::Adjust(Setting::UiScale, -1),
                Action::Adjust(Setting::UiScale, 1),
                (right_col_x, content_y + step_y * 3.0),
                col_w,
                BUTTON_HEIGHT,
            );
            ctx.spawn_adj(
                commands,
                &format!("HUD Opacity: {:.0}%", settings.hud_opacity * 100.0),
                (settings.hud_opacity - 0.25) / 0.75,
                Action::Adjust(Setting::HudOpacity, -1),
                Action::Adjust(Setting::HudOpacity, 1),
                (right_col_x, content_y + step_y * 4.0),
                col_w,
                BUTTON_HEIGHT,
            );
        }
        OptionsTab::Audio => {
            ctx.spawn_adj(
                commands,
                &format!("Sound Volume: {}%", settings.sound_volume),
                settings.sound_volume as f32 / 100.0,
                Action::Adjust(Setting::Volume, -5),
                Action::Adjust(Setting::Volume, 5),
                (center_col_x, content_y),
                col_w,
                BUTTON_HEIGHT,
            );
            ctx.spawn_adj(
                commands,
                &format!("Music Volume: {}%", settings.music_volume),
                settings.music_volume as f32 / 100.0,
                Action::Adjust(Setting::MusicVolume, -5),
                Action::Adjust(Setting::MusicVolume, 5),
                (center_col_x, content_y + step_y),
                col_w,
                BUTTON_HEIGHT,
            );
            ctx.spawn_adj(
                commands,
                &format!("Ambient Volume: {}%", settings.ambient_volume),
                settings.ambient_volume as f32 / 100.0,
                Action::Adjust(Setting::AmbientVolume, -5),
                Action::Adjust(Setting::AmbientVolume, 5),
                (center_col_x, content_y + step_y * 2.0),
                col_w,
                BUTTON_HEIGHT,
            );
            ctx.spawn_adj(
                commands,
                &format!("UI Volume: {}%", settings.ui_volume),
                settings.ui_volume as f32 / 100.0,
                Action::Adjust(Setting::UiVolume, -5),
                Action::Adjust(Setting::UiVolume, 5),
                (center_col_x, content_y + step_y * 3.0),
                col_w,
                BUTTON_HEIGHT,
            );
            ctx.spawn_toggle(
                commands,
                "Mute on Focus Loss",
                settings.mute_on_focus_loss,
                Action::ToggleMuteOnFocusLoss,
                (center_col_x, content_y + step_y * 4.0),
                (col_w, BUTTON_HEIGHT),
            );
        }
        OptionsTab::Controls => {
            ctx.spawn_adj(
                commands,
                &format!("Sensitivity: {:.2}", settings.sensitivity),
                (settings.sensitivity - 0.25) / 2.75,
                Action::Adjust(Setting::Sensitivity, -1),
                Action::Adjust(Setting::Sensitivity, 1),
                (left_col_x, content_y),
                col_w,
                BUTTON_HEIGHT,
            );
            ctx.spawn_toggle(
                commands,
                "Invert Y",
                settings.invert_y,
                Action::ToggleInvertY,
                (right_col_x, content_y),
                (col_w, BUTTON_HEIGHT),
            );

            let bind_y0 = content_y + step_y + 4.0;
            let bind_step_y = 40.0;
            let bind_h = 34.0;
            let split = GameAction::COUNT.div_ceil(2);
            for (idx, action) in GameAction::ALL.into_iter().enumerate() {
                let col_x = if idx < split { left_col_x } else { right_col_x };
                let row = if idx < split { idx } else { idx - split };
                let y = bind_y0 + row as f32 * bind_step_y;
                let key_str = if state.listening_action == Some(action) {
                    "[Press Key...]".to_string()
                } else {
                    keycode_name(settings.bindings.key(action))
                        .unwrap_or("?")
                        .to_string()
                };
                let label = format!("{}: {key_str}", action.label());
                ctx.spawn_btn(
                    commands,
                    &label,
                    Action::StartRebind(action),
                    (col_x, y),
                    (col_w, bind_h),
                    false,
                );
            }
            ctx.spawn_btn(
                commands,
                "Reset Bindings",
                Action::ResetBindings,
                (right_col_x, bind_y0 + split as f32 * bind_step_y),
                (col_w, bind_h),
                false,
            );
        }
        OptionsTab::Accessibility => {
            ctx.spawn_toggle(
                commands,
                "Color-Blind Palette",
                settings.colorblind_palette,
                Action::ToggleColorblindPalette,
                (center_col_x, content_y),
                (col_w, BUTTON_HEIGHT),
            );
            ctx.spawn_toggle(
                commands,
                "High-Contrast Crosshair",
                settings.high_contrast_crosshair,
                Action::ToggleHighContrastCrosshair,
                (center_col_x, content_y + step_y),
                (col_w, BUTTON_HEIGHT),
            );
            ctx.spawn_adj(
                commands,
                &format!("Screen Shake: {:.0}%", settings.screen_shake * 100.0),
                settings.screen_shake / 2.0,
                Action::Adjust(Setting::ScreenShake, -1),
                Action::Adjust(Setting::ScreenShake, 1),
                (center_col_x, content_y + step_y * 2.0),
                col_w,
                BUTTON_HEIGHT,
            );
            ctx.spawn_toggle(
                commands,
                "Hitstop",
                settings.hitstop,
                Action::ToggleHitstop,
                (center_col_x, content_y + step_y * 3.0),
                (col_w, BUTTON_HEIGHT),
            );
            ctx.spawn_toggle(
                commands,
                "Damage Numbers",
                settings.damage_numbers,
                Action::ToggleDamageNumbers,
                (center_col_x, content_y + step_y * 4.0),
                (col_w, BUTTON_HEIGHT),
            );
            ctx.spawn_toggle(
                commands,
                "Reduced Motion",
                settings.reduced_motion,
                Action::ToggleReducedMotion,
                (center_col_x, content_y + step_y * 5.0),
                (col_w, BUTTON_HEIGHT),
            );
            ctx.spawn_adj(
                commands,
                &format!("Tooltip Delay: {:.1}s", settings.tooltip_delay),
                settings.tooltip_delay,
                Action::Adjust(Setting::TooltipDelay, -1),
                Action::Adjust(Setting::TooltipDelay, 1),
                (center_col_x, content_y + step_y * 6.0),
                col_w,
                BUTTON_HEIGHT,
            );
        }
    }

    // Footer: Reset Defaults & Back (fits inside 720p: 630 + 42 = 672 <= 720)
    let footer_y = 630.0;
    ctx.spawn_btn(
        commands,
        "Reset Defaults",
        Action::ResetDefaults,
        (center_x - BUTTON_WIDTH - 12.0, footer_y),
        (BUTTON_WIDTH, BUTTON_HEIGHT),
        false,
    );
    ctx.spawn_btn(
        commands,
        "Back",
        Action::Back,
        (center_x + 12.0, footer_y),
        (BUTTON_WIDTH, BUTTON_HEIGHT),
        false,
    );
}

fn is_options_open(
    current_menu_screen: Option<&Res<State<MenuScreen>>>,
    game_menu: Option<&ResMut<GameMenu>>,
) -> bool {
    let is_in_menu_options = current_menu_screen.map(|s| s.get()) == Some(&MenuScreen::Options);
    let is_in_game_menu_options = game_menu.and_then(|gm| gm.panel) == Some(GamePanel::Options);
    is_in_menu_options || is_in_game_menu_options
}

#[allow(clippy::too_many_arguments)]
fn rebuild_options_ui(
    commands: &mut Commands,
    options_ui_entities: &Query<Entity, With<OptionsUiRoot>>,
    settings: &Settings,
    ui_state: &OptionsUiState,
    host: Option<OptionsHost>,
    win_query: &Query<&Window, With<PrimaryWindow>>,
    primary_monitor: &Query<&Monitor, With<PrimaryMonitor>>,
    all_monitors: &Query<&Monitor>,
) {
    for e in options_ui_entities {
        commands.entity(e).despawn();
    }
    setup_options_with_state(
        commands,
        settings,
        ui_state,
        host,
        win_query,
        primary_monitor,
        all_monitors,
    );
}

/// Captures the next supported key press when listening for a control rebind.
#[allow(clippy::too_many_arguments)]
pub fn handle_options_rebind_input(
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    current_menu_screen: Option<Res<State<MenuScreen>>>,
    game_menu: Option<ResMut<GameMenu>>,
    host: Option<Res<OptionsHost>>,
    mut settings: Option<ResMut<Settings>>,
    settings_path: Option<Res<SettingsPath>>,
    mut ui_state: Option<ResMut<OptionsUiState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut commands: Commands,
    options_ui_entities: Query<Entity, With<OptionsUiRoot>>,
    primary_monitor: Query<&Monitor, With<PrimaryMonitor>>,
    all_monitors: Query<&Monitor>,
) {
    if !is_options_open(current_menu_screen.as_ref(), game_menu.as_ref()) {
        return;
    }
    let (Some(kb), Some(ref mut ui_state), Some(ref mut settings)) =
        (keyboard, ui_state.as_deref_mut(), settings.as_deref_mut())
    else {
        return;
    };
    let Some(target_action) = ui_state.listening_action else {
        return;
    };

    let pressed_key = kb
        .get_just_pressed()
        .copied()
        .find(|k| keycode_name(*k).is_some());
    let Some(key) = pressed_key else {
        return;
    };

    ui_state.listening_action = None;
    match settings.bindings.rebind(target_action, key) {
        Ok(()) => {
            ui_state.conflict_message = None;
            if let Some(ref path) = settings_path {
                persist_settings(path, settings);
            }
        }
        Err(conflict) => {
            let key_label = keycode_name(key).unwrap_or("Key");
            ui_state.conflict_message = Some(format!(
                "Conflict: {key_label} is already bound to {}",
                conflict.label()
            ));
        }
    }

    rebuild_options_ui(
        &mut commands,
        &options_ui_entities,
        settings,
        ui_state,
        host.as_deref().copied(),
        &windows,
        &primary_monitor,
        &all_monitors,
    );
}

/// Handles actions emitted from buttons or navigation when Options is open.
#[allow(clippy::too_many_arguments)]
pub fn handle_options_actions(
    actions: Option<MessageReader<Action>>,
    current_menu_screen: Option<Res<State<MenuScreen>>>,
    mut next_menu_screen: Option<ResMut<NextState<MenuScreen>>>,
    mut game_menu: Option<ResMut<GameMenu>>,
    host: Option<ResMut<OptionsHost>>,
    settings: Option<ResMut<Settings>>,
    settings_path: Option<Res<SettingsPath>>,
    mut ui_state: Option<ResMut<OptionsUiState>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut global_volume: Option<ResMut<GlobalVolume>>,
    mut look_sens: Option<ResMut<LookSensitivity>>,
    mut cameras: Query<&mut Projection, With<FlyCamera>>,
    mut commands: Commands,
    options_ui_entities: Query<Entity, With<OptionsUiRoot>>,
    primary_monitor: Query<&Monitor, With<PrimaryMonitor>>,
    all_monitors: Query<&Monitor>,
) {
    if !is_options_open(current_menu_screen.as_ref(), game_menu.as_ref()) {
        return;
    }

    let (Some(mut action_reader), Some(ref mut settings)) = (actions, settings) else {
        return;
    };

    let mut settings_changed = false;
    let mut ui_changed = false;
    let mut close_options = false;

    for action in action_reader.read() {
        match action {
            Action::SelectOptionsTab(tab) => {
                if let Some(ref mut state) = ui_state {
                    state.active_tab = *tab;
                    state.listening_action = None;
                    state.conflict_message = None;
                }
                ui_changed = true;
            }
            Action::StartRebind(game_action) => {
                if let Some(ref mut state) = ui_state {
                    state.listening_action = Some(*game_action);
                    state.conflict_message = None;
                }
                ui_changed = true;
            }
            Action::ResetBindings => {
                settings.bindings.reset_defaults();
                if let Some(ref mut state) = ui_state {
                    state.listening_action = None;
                    state.conflict_message = None;
                }
                settings_changed = true;
            }
            Action::CycleWindowMode => {
                settings.window_mode = cycle_window_mode(settings.window_mode);
                if let Ok(mut win) = windows.single_mut() {
                    settings::apply_to_window(settings, &mut win);
                }
                settings_changed = true;
            }
            Action::CycleResolution => {
                if settings.window_mode == WindowModeSetting::Windowed {
                    let (nw, nh) = cycle_resolution(settings.width, settings.height);
                    settings.width = nw;
                    settings.height = nh;
                    if let Ok(mut win) = windows.single_mut() {
                        settings::apply_to_window(settings, &mut win);
                    }
                    settings_changed = true;
                }
            }
            Action::CycleQualityPreset => {
                settings.quality_preset = cycle_quality_preset(settings.quality_preset);
                settings_changed = true;
            }
            Action::ToggleVsync => {
                settings.vsync = !settings.vsync;
                if let Ok(mut win) = windows.single_mut() {
                    settings::apply_to_window(settings, &mut win);
                }
                settings_changed = true;
            }
            Action::ToggleWeatherEffects => {
                settings.weather_effects = !settings.weather_effects;
                settings_changed = true;
            }
            Action::ToggleAmbientParticles => {
                settings.ambient_particles = !settings.ambient_particles;
                settings_changed = true;
            }
            Action::ToggleAtmosphereFx => {
                settings.atmosphere_fx = !settings.atmosphere_fx;
                settings_changed = true;
            }
            Action::ToggleMuteOnFocusLoss => {
                settings.mute_on_focus_loss = !settings.mute_on_focus_loss;
                settings_changed = true;
            }
            Action::ToggleColorblindPalette => {
                settings.colorblind_palette = !settings.colorblind_palette;
                settings_changed = true;
            }
            Action::ToggleHighContrastCrosshair => {
                settings.high_contrast_crosshair = !settings.high_contrast_crosshair;
                settings_changed = true;
            }
            Action::ToggleHitstop => {
                settings.hitstop = !settings.hitstop;
                settings_changed = true;
            }
            Action::ToggleDamageNumbers => {
                settings.damage_numbers = !settings.damage_numbers;
                settings_changed = true;
            }
            Action::ToggleReducedMotion => {
                settings.reduced_motion = !settings.reduced_motion;
                settings_changed = true;
            }
            Action::Adjust(Setting::Volume, step) => {
                settings.sound_volume = nudge_volume(settings.sound_volume as i32, *step);
                if let Some(ref mut gv) = global_volume {
                    gv.volume =
                        bevy::audio::Volume::Linear(settings::volume_gain(settings.sound_volume));
                }
                settings_changed = true;
            }
            Action::Adjust(Setting::MusicVolume, step) => {
                settings.music_volume = nudge_volume(settings.music_volume as i32, *step);
                settings_changed = true;
            }
            Action::Adjust(Setting::AmbientVolume, step) => {
                settings.ambient_volume = nudge_volume(settings.ambient_volume as i32, *step);
                settings_changed = true;
            }
            Action::Adjust(Setting::UiVolume, step) => {
                settings.ui_volume = nudge_volume(settings.ui_volume as i32, *step);
                settings_changed = true;
            }
            Action::Adjust(Setting::Sensitivity, step) => {
                settings.sensitivity = nudge_sensitivity(settings.sensitivity, *step);
                if let Some(ref mut sens) = look_sens {
                    sens.0 = settings.sensitivity;
                }
                settings_changed = true;
            }
            Action::Adjust(Setting::Fov, step) => {
                settings.fov = nudge_fov(settings.fov, *step);
                for mut proj in &mut cameras {
                    if let Projection::Perspective(ref mut pers) = *proj {
                        pers.fov = settings.fov.to_radians();
                    }
                }
                settings_changed = true;
            }
            Action::Adjust(Setting::UiScale, step) => {
                settings.ui_scale = nudge_ui_scale(settings.ui_scale, *step);
                settings_changed = true;
            }
            Action::Adjust(Setting::HudOpacity, step) => {
                settings.hud_opacity = nudge_hud_opacity(settings.hud_opacity, *step);
                settings_changed = true;
            }
            Action::Adjust(Setting::TooltipDelay, step) => {
                settings.tooltip_delay = nudge_tooltip_delay(settings.tooltip_delay, *step);
                settings_changed = true;
            }
            Action::Adjust(Setting::ScreenShake, step) => {
                settings.screen_shake = nudge_screen_shake(settings.screen_shake, *step);
                settings_changed = true;
            }
            Action::ToggleInvertY => {
                settings.invert_y = !settings.invert_y;
                settings_changed = true;
            }
            Action::ResetDefaults => {
                **settings = Settings::default();
                if let Some(ref mut state) = ui_state {
                    state.listening_action = None;
                    state.conflict_message = None;
                }
                if let Ok(mut win) = windows.single_mut() {
                    settings::apply_to_window(settings, &mut win);
                }
                if let Some(ref mut gv) = global_volume {
                    gv.volume =
                        bevy::audio::Volume::Linear(settings::volume_gain(settings.sound_volume));
                }
                if let Some(ref mut sens) = look_sens {
                    sens.0 = settings.sensitivity;
                }
                for mut proj in &mut cameras {
                    if let Projection::Perspective(ref mut pers) = *proj {
                        pers.fov = settings.fov.to_radians();
                    }
                }
                settings_changed = true;
            }
            Action::Back => {
                close_options = true;
            }
            _ => {}
        }
    }

    if settings_changed || ui_changed {
        if settings_changed {
            if let Some(ref path) = settings_path {
                persist_settings(path, settings);
            }
        }
        let win_query = windows.as_readonly();
        let current_host = host.as_deref().copied();
        let fallback_state = OptionsUiState::default();
        let state_ref = ui_state.as_deref().unwrap_or(&fallback_state);
        rebuild_options_ui(
            &mut commands,
            &options_ui_entities,
            settings,
            state_ref,
            current_host,
            &win_query,
            &primary_monitor,
            &all_monitors,
        );
    } else if close_options {
        if let Some(ref mut state) = ui_state {
            state.listening_action = None;
            state.conflict_message = None;
        }
        if let Some(ref path) = settings_path {
            persist_settings(path, settings);
        }
        let current_host = host.as_deref().copied().unwrap_or_default();
        match current_host {
            OptionsHost::Title => {
                if let Some(ref mut next) = next_menu_screen {
                    next.set(MenuScreen::Title);
                }
            }
            OptionsHost::GameMenu => {
                if let Some(ref mut gm) = game_menu {
                    gm.panel = Some(GamePanel::Root);
                }
                for e in &options_ui_entities {
                    commands.entity(e).despawn();
                }
            }
        }
    }
}

fn on_enter_menu_options(
    mut host: ResMut<OptionsHost>,
    mut ui_state: ResMut<OptionsUiState>,
    mut commands: Commands,
    settings: Res<Settings>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    primary_monitor: Query<&Monitor, With<PrimaryMonitor>>,
    all_monitors: Query<&Monitor>,
) {
    *host = OptionsHost::Title;
    *ui_state = OptionsUiState::default();
    setup_options_with_state(
        &mut commands,
        &settings,
        &ui_state,
        Some(*host),
        &primary_window,
        &primary_monitor,
        &all_monitors,
    );
}

fn sync_saved_settings_to_resources(
    settings: Res<Settings>,
    mut look_sens: ResMut<LookSensitivity>,
    mut global_volume: ResMut<GlobalVolume>,
) {
    look_sens.0 = settings.sensitivity;
    global_volume.volume =
        bevy::audio::Volume::Linear(settings::volume_gain(settings.sound_volume));
}

pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OptionsHost>()
            .init_resource::<OptionsUiState>()
            .init_resource::<Settings>()
            .init_resource::<SettingsPath>()
            .init_resource::<GlobalVolume>()
            .init_resource::<LookSensitivity>()
            .add_systems(Startup, sync_saved_settings_to_resources)
            .add_systems(OnEnter(MenuScreen::Options), on_enter_menu_options)
            .add_systems(
                Update,
                handle_options_rebind_input.before(handle_options_actions),
            )
            .add_systems(Update, handle_options_actions)
            .add_systems(Update, recenter_options);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::menu::{MenuButton, MenuFocus};
    use crate::view::camera::update_camera_angles;
    use bevy::audio::Volume;
    use bevy::window::{PresentMode, WindowMode};

    #[test]
    fn options_music_row_nudges_by_5() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .add_message::<Action>()
            .init_resource::<Settings>()
            .init_resource::<SettingsPath>()
            .init_resource::<GlobalVolume>()
            .init_resource::<LookSensitivity>()
            .add_systems(Update, handle_options_actions);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Options);
        app.world_mut().resource_mut::<Settings>().music_volume = 50;

        app.world_mut()
            .write_message(Action::Adjust(Setting::MusicVolume, 5));
        app.update();
        assert_eq!(app.world().resource::<Settings>().music_volume, 55);

        app.world_mut()
            .write_message(Action::Adjust(Setting::MusicVolume, -5));
        app.update();
        assert_eq!(app.world().resource::<Settings>().music_volume, 50);
    }

    #[test]
    fn options_recenter_after_window_resize() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .add_message::<Action>()
            .add_plugins(OptionsPlugin);
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Options);
        app.update();
        app.world_mut()
            .entity_mut(window)
            .get_mut::<Window>()
            .unwrap()
            .resolution
            .set(1280.0, 720.0);
        app.update();
        let mut roots = app
            .world_mut()
            .query_filtered::<&Node, With<OptionsUiRoot>>();
        let nodes: Vec<_> = roots.iter(app.world()).collect();
        assert!(!nodes.is_empty());
        for node in nodes {
            if let (Val::Px(left), Val::Px(width)) = (node.left, node.width) {
                assert!(
                    left >= 0.0 && left + width <= 1280.0,
                    "node outside 1280 viewport: {left}..{}",
                    left + width
                );
            }
        }
    }

    #[test]
    fn cycles_wrap() {
        assert_eq!(
            cycle_window_mode(WindowModeSetting::Fullscreen),
            WindowModeSetting::Windowed
        );
        assert_eq!(cycle_resolution(2560, 1440), (1280, 720));
        assert_eq!(cycle_resolution(800, 600), (1280, 720));
    }

    #[test]
    fn nudges_clamp_to_spec_ranges() {
        assert_eq!(nudge_volume(98, 5), 100);
        assert_eq!(nudge_volume(2, -5), 0);
        assert_eq!(nudge_sensitivity(3.0, 1), 3.0);
        assert_eq!(nudge_sensitivity(0.25, -1), 0.25);
        assert_eq!(nudge_fov(99.0, 5), 100.0);
        assert_eq!(nudge_fov(50.0, -5), 50.0);
    }

    fn test_temp_file(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "terravibe-opt-test-{}-{}",
            prefix,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        dir.join("settings.cfg")
    }

    #[test]
    fn apply_volume_updates_global_volume_and_settings_file() {
        let file = test_temp_file("vol");
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .add_message::<Action>()
            .insert_resource(Settings {
                sound_volume: 75,
                ..default()
            })
            .insert_resource(SettingsPath(Some(file.clone())))
            .insert_resource(GlobalVolume::new(Volume::Linear(settings::volume_gain(75))))
            .add_systems(Update, handle_options_actions);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Options);
        app.update();

        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::Adjust(Setting::Volume, 5));
        app.update();

        let s = app.world().resource::<Settings>();
        assert_eq!(s.sound_volume, 80);

        let gv = app.world().resource::<GlobalVolume>();
        assert_eq!(gv.volume, Volume::Linear(settings::volume_gain(80)));

        let saved = std::fs::read_to_string(&file).expect("file written");
        assert!(saved.contains("sound_volume=80"), "file content: {saved}");
    }

    #[test]
    fn apply_window_mode_updates_window_live() {
        let file = test_temp_file("win");
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .add_message::<Action>()
            .insert_resource(Settings {
                window_mode: WindowModeSetting::Windowed,
                vsync: true,
                ..default()
            })
            .insert_resource(SettingsPath(Some(file)))
            .add_systems(Update, handle_options_actions);

        // Spawn stub PrimaryWindow
        let win_entity = app
            .world_mut()
            .spawn((
                Window {
                    mode: WindowMode::Windowed,
                    present_mode: PresentMode::AutoVsync,
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Options);
        app.update();

        // Cycle WindowMode to Borderless
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::CycleWindowMode);
        app.update();

        let win = app.world().get::<Window>(win_entity).unwrap();
        assert!(matches!(win.mode, WindowMode::BorderlessFullscreen(_)));

        // Toggle VSync
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::ToggleVsync);
        app.update();

        let win = app.world().get::<Window>(win_entity).unwrap();
        assert_eq!(win.present_mode, PresentMode::AutoNoVsync);
    }

    #[test]
    fn apply_fov_and_sensitivity_update_camera_and_look() {
        let file = test_temp_file("fov_sens");
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .add_message::<Action>()
            .insert_resource(Settings {
                fov: 70.0,
                sensitivity: 1.0,
                invert_y: false,
                ..default()
            })
            .insert_resource(SettingsPath(Some(file)))
            .insert_resource(LookSensitivity(1.0))
            .add_systems(Update, handle_options_actions);

        // Spawn FlyCamera entity with Perspective projection
        let cam_entity = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Projection::Perspective(PerspectiveProjection {
                    fov: 70.0f32.to_radians(),
                    ..default()
                }),
                FlyCamera {
                    yaw: 0.0,
                    pitch: 0.0,
                },
            ))
            .id();

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Options);
        app.update();

        // Adjust FOV by 5
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::Adjust(Setting::Fov, 1));
        app.update();

        let proj = app.world().get::<Projection>(cam_entity).unwrap();
        if let Projection::Perspective(ref p) = *proj {
            assert!((p.fov - 75.0f32.to_radians()).abs() < 1e-4);
        } else {
            panic!("Expected perspective projection");
        }

        // Adjust Sensitivity
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::Adjust(Setting::Sensitivity, 2));
        app.update();

        let sens = app.world().resource::<LookSensitivity>();
        assert!((sens.0 - 1.5).abs() < 1e-4);

        // Invert Y toggle and test angle math
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::ToggleInvertY);
        app.update();

        let s = app.world().resource::<Settings>();
        assert!(s.invert_y);

        // Using update_camera_angles helper:
        let motion_delta = Vec2::new(10.0, 10.0);
        let (yaw_normal, pitch_normal) =
            update_camera_angles(0.0, 0.0, motion_delta, 1.0, false, (-1.5, 1.5));
        let (yaw_invert, pitch_invert) =
            update_camera_angles(0.0, 0.0, motion_delta, 1.0, true, (-1.5, 1.5));
        assert_eq!(yaw_normal, yaw_invert);
        assert_eq!(pitch_normal, -pitch_invert);
    }

    #[test]
    fn reset_restores_defaults_and_persists() {
        let file = test_temp_file("reset");
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .add_message::<Action>()
            .insert_resource(Settings {
                sound_volume: 20,
                music_volume: 30,
                sensitivity: 2.5,
                fov: 90.0,
                vsync: false,
                invert_y: true,
                window_mode: WindowModeSetting::Fullscreen,
                width: 1280,
                height: 720,
                ..default()
            })
            .insert_resource(SettingsPath(Some(file.clone())))
            .insert_resource(GlobalVolume::new(Volume::Linear(settings::volume_gain(20))))
            .insert_resource(LookSensitivity(2.5))
            .add_systems(Update, handle_options_actions);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Options);
        app.update();

        // Reset
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::ResetDefaults);
        app.update();

        let s = app.world().resource::<Settings>();
        assert_eq!(*s, Settings::default());

        let gv = app.world().resource::<GlobalVolume>();
        assert_eq!(
            gv.volume,
            Volume::Linear(settings::volume_gain(Settings::default().sound_volume))
        );

        let sens = app.world().resource::<LookSensitivity>();
        assert_eq!(sens.0, Settings::default().sensitivity);

        let saved = std::fs::read_to_string(&file).expect("file written");
        assert_eq!(settings::parse(&saved), Settings::default());
    }

    #[test]
    fn fullscreen_disables_resolution_row() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .add_message::<Action>()
            .insert_resource(Settings {
                window_mode: WindowModeSetting::Fullscreen,
                width: 1920,
                height: 1080,
                ..default()
            })
            .insert_resource(SettingsPath(None))
            .add_plugins(OptionsPlugin);

        // Spawn stub primary monitor with 2560x1440
        app.world_mut().spawn((
            Monitor {
                name: Some("TestMonitor".into()),
                physical_width: 2560,
                physical_height: 1440,
                physical_position: IVec2::ZERO,
                refresh_rate_millihertz: Some(60000),
                scale_factor: 1.0,
                video_modes: vec![],
            },
            PrimaryMonitor,
        ));

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Options);
        app.update();

        // Check buttons
        let mut buttons_query = app.world_mut().query::<(&MenuButton, &Children)>();
        let mut text_query = app.world_mut().query::<&MenuText>();
        let mut found_res_button = false;

        for (btn, children) in buttons_query.iter(app.world()) {
            if btn.action == Action::CycleResolution {
                found_res_button = true;
                assert!(
                    btn.disabled,
                    "Resolution button must be disabled in fullscreen"
                );
                for child in children.iter() {
                    if let Ok(txt) = text_query.get(app.world(), child) {
                        assert!(
                            txt.text.contains("2560x1440 (Native)"),
                            "Button text: {}",
                            txt.text
                        );
                    }
                }
            }
        }
        assert!(found_res_button, "Resolution button must exist");

        // Send CycleResolution: must be ignored because mode != Windowed
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::CycleResolution);
        app.update();

        let s = app.world().resource::<Settings>();
        assert_eq!(s.width, 1920);
        assert_eq!(s.height, 1080);
    }

    #[test]
    fn startup_syncs_saved_settings_to_resources() {
        let mut app = App::new();
        app.insert_resource(Settings {
            sound_volume: 20,
            sensitivity: 2.0,
            ..default()
        })
        .add_plugins(OptionsPlugin);

        app.update();

        let sens = app.world().resource::<LookSensitivity>();
        assert_eq!(sens.0, 2.0);

        let gv = app.world().resource::<GlobalVolume>();
        assert_eq!(gv.volume, Volume::Linear(settings::volume_gain(20)));
    }

    #[test]
    fn adjustable_rows_support_decrement() {
        let file = test_temp_file("dec");
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .add_message::<Action>()
            .insert_resource(Settings {
                sound_volume: 50,
                sensitivity: 1.5,
                fov: 75.0,
                ..default()
            })
            .insert_resource(SettingsPath(Some(file)))
            .insert_resource(GlobalVolume::new(Volume::Linear(settings::volume_gain(50))))
            .insert_resource(LookSensitivity(1.5))
            .add_systems(Update, handle_options_actions);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Options);
        app.update();

        // Decrement Volume by 5
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::Adjust(Setting::Volume, -5));
        app.update();

        let s = app.world().resource::<Settings>();
        assert_eq!(s.sound_volume, 45);

        let gv = app.world().resource::<GlobalVolume>();
        assert_eq!(gv.volume, Volume::Linear(settings::volume_gain(45)));

        // Decrement Sensitivity by 1 step (-0.25)
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::Adjust(Setting::Sensitivity, -1));
        app.update();

        let s = app.world().resource::<Settings>();
        assert!((s.sensitivity - 1.25).abs() < 1e-4);

        let sens = app.world().resource::<LookSensitivity>();
        assert!((sens.0 - 1.25).abs() < 1e-4);

        // Decrement FOV by 1 step (-5.0)
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::Adjust(Setting::Fov, -1));
        app.update();

        let s = app.world().resource::<Settings>();
        assert!((s.fov - 70.0).abs() < 1e-4);
    }

    #[test]
    fn options_layout_header_and_buttons_do_not_overlap() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .add_message::<Action>()
            .insert_resource(Settings::default())
            .insert_resource(SettingsPath(None))
            .add_plugins(OptionsPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Options);
        app.update();

        // Find header panel container
        let (panel_top, pad_top, pad_bot, border_top, border_bot, children_vec) = {
            let mut panel_q = app
                .world_mut()
                .query_filtered::<(&Node, &Children), (With<OptionsUiRoot>, Without<MenuButton>)>();
            let panels: Vec<_> = panel_q
                .iter(app.world())
                .filter(|(node, _)| matches!(node.flex_direction, FlexDirection::Column))
                .collect();
            assert_eq!(
                panels.len(),
                1,
                "Must have exactly 1 header panel container"
            );
            let (panel_node, children) = panels[0];

            let panel_top = match panel_node.top {
                Val::Px(v) => v,
                other => panic!("Expected Px top for panel, got {:?}", other),
            };
            let pad_top = match panel_node.padding.top {
                Val::Px(v) => v,
                _ => 0.0,
            };
            let pad_bot = match panel_node.padding.bottom {
                Val::Px(v) => v,
                _ => 0.0,
            };
            let border_top = match panel_node.border.top {
                Val::Px(v) => v,
                _ => 0.0,
            };
            let border_bot = match panel_node.border.bottom {
                Val::Px(v) => v,
                _ => 0.0,
            };
            let children_vec: Vec<Entity> = children.iter().collect();
            (
                panel_top,
                pad_top,
                pad_bot,
                border_top,
                border_bot,
                children_vec,
            )
        };

        // Sum child contents and margins
        let mut children_content_height = 0.0f32;
        let mut node_q = app.world_mut().query::<&Node>();
        let mut text_q = app.world_mut().query::<&MenuText>();
        // Mouse font typical line height is ~20px (at scale 1.0)
        let base_line_h = 20.0f32;
        for child in children_vec {
            let child_node = node_q.get(app.world(), child).unwrap();
            let margin_bot = match child_node.margin.bottom {
                Val::Px(v) => v,
                _ => 0.0,
            };
            let text_scale = text_q
                .get(app.world(), child)
                .map(|t| t.scale)
                .unwrap_or(1.0);
            children_content_height += base_line_h * text_scale + margin_bot;
        }

        let panel_bottom =
            panel_top + border_top + pad_top + children_content_height + pad_bot + border_bot;

        // Query all menu buttons and check bounds
        let mut btn_q = app
            .world_mut()
            .query_filtered::<(&MenuButton, &Node), With<OptionsUiRoot>>();
        let mut buttons: Vec<_> = btn_q
            .iter(app.world())
            .map(|(b, node)| {
                let top = match node.top {
                    Val::Px(v) => v,
                    other => panic!("Expected Px top for button, got {:?}", other),
                };
                let height = match node.height {
                    Val::Px(v) => v,
                    other => panic!("Expected Px height for button, got {:?}", other),
                };
                (b.rank, top, height)
            })
            .collect();
        buttons.sort_by_key(|&(rank, _, _)| rank);

        assert!(!buttons.is_empty(), "Must have buttons in options");
        let row0_top = buttons[0].1;

        assert!(
            panel_bottom <= row0_top,
            "Header panel bottom ({panel_bottom:.1}px) intersects Row 0 top ({row0_top:.1}px)"
        );

        // Check that all buttons fit in 1280x720 and 1920x1080 window
        for (rank, top, height) in buttons {
            let bottom = top + height;
            assert!(
                bottom <= 720.0,
                "Button rank {rank} bottom ({bottom:.1}px) exceeds 720p height limit"
            );
        }
    }

    #[test]
    fn options_tabs_switch_and_rebind_conflict_shows_message() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<Action>()
            .insert_resource(Settings::default())
            .insert_resource(SettingsPath(None))
            .add_plugins(OptionsPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Options);
        app.update();

        // Default tab is Display; switching to Audio shows ambient_volume / ui_volume controls.
        assert_eq!(
            app.world().resource::<OptionsUiState>().active_tab,
            OptionsTab::Display
        );
        app.world_mut()
            .write_message(Action::SelectOptionsTab(OptionsTab::Audio));
        app.update();
        assert_eq!(
            app.world().resource::<OptionsUiState>().active_tab,
            OptionsTab::Audio
        );
        let has_mute_btn = app
            .world_mut()
            .query::<&MenuButton>()
            .iter(app.world())
            .any(|b| b.action == Action::ToggleMuteOnFocusLoss);
        assert!(has_mute_btn, "Audio tab must expose mute-on-focus-loss");

        // Switch to Accessibility and verify colorblind toggle is present.
        app.world_mut()
            .write_message(Action::SelectOptionsTab(OptionsTab::Accessibility));
        app.update();
        let has_cb_btn = app
            .world_mut()
            .query::<&MenuButton>()
            .iter(app.world())
            .any(|b| b.action == Action::ToggleColorblindPalette);
        assert!(
            has_cb_btn,
            "Accessibility tab must expose color-blind palette toggle"
        );

        // Switch to Controls, start rebinding Jump, press KeyW (bound to MoveForward) -> conflict!
        app.world_mut()
            .write_message(Action::SelectOptionsTab(OptionsTab::Controls));
        app.update();
        app.world_mut()
            .write_message(Action::StartRebind(crate::input_map::GameAction::Jump));
        app.update();
        assert_eq!(
            app.world().resource::<OptionsUiState>().listening_action,
            Some(crate::input_map::GameAction::Jump)
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyW);

        // Binding must remain Space, and conflict message must be shown.
        assert_eq!(
            app.world()
                .resource::<Settings>()
                .bindings
                .key(crate::input_map::GameAction::Jump),
            KeyCode::Space
        );
        let msg = app
            .world()
            .resource::<OptionsUiState>()
            .conflict_message
            .clone()
            .expect("conflict message must be set");
        assert!(
            msg.contains("Move Forward"),
            "Conflict message should mention conflicting action: {msg}"
        );
        let ui_shows_msg = app
            .world_mut()
            .query::<&MenuText>()
            .iter(app.world())
            .any(|t| t.text.contains("Move Forward"));
        assert!(ui_shows_msg, "UI must render conflict message");
    }

    #[test]
    fn options_all_new_settings_update_and_reset() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<Action>()
            .insert_resource(Settings::default())
            .insert_resource(SettingsPath(None))
            .add_plugins(OptionsPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Options);
        app.update();

        for act in [
            Action::CycleQualityPreset,
            Action::ToggleWeatherEffects,
            Action::ToggleAmbientParticles,
            Action::ToggleAtmosphereFx,
            Action::ToggleMuteOnFocusLoss,
            Action::ToggleColorblindPalette,
            Action::ToggleHighContrastCrosshair,
            Action::ToggleHitstop,
            Action::ToggleDamageNumbers,
            Action::ToggleReducedMotion,
            Action::Adjust(Setting::AmbientVolume, 5),
            Action::Adjust(Setting::UiVolume, -5),
            Action::Adjust(Setting::UiScale, 2),
            Action::Adjust(Setting::HudOpacity, -2),
            Action::Adjust(Setting::TooltipDelay, 2),
            Action::Adjust(Setting::ScreenShake, -1),
        ] {
            app.world_mut().write_message(act);
        }
        app.update();

        let s = app.world().resource::<Settings>();
        assert_eq!(s.quality_preset, QualityPreset::High);
        assert!(!s.weather_effects);
        assert!(!s.ambient_particles);
        assert!(!s.atmosphere_fx);
        assert!(!s.mute_on_focus_loss);
        assert!(s.colorblind_palette);
        assert!(s.high_contrast_crosshair);
        assert!(!s.hitstop);
        assert!(!s.damage_numbers);
        assert!(s.reduced_motion);
        assert_eq!(s.ambient_volume, 80);
        assert_eq!(s.ui_volume, 70);
        assert!((s.ui_scale - 1.1).abs() < 1e-4);
        assert!((s.hud_opacity - 0.9).abs() < 1e-4);
        assert!((s.tooltip_delay - 0.4).abs() < 1e-4);
        assert!((s.screen_shake - 0.75).abs() < 1e-4);

        // Valid rebind succeeds and ResetDefaults restores everything.
        app.world_mut()
            .write_message(Action::StartRebind(GameAction::Jump));
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyU);
        app.update();
        assert_eq!(
            app.world()
                .resource::<Settings>()
                .bindings
                .key(GameAction::Jump),
            KeyCode::KeyU
        );

        app.world_mut().write_message(Action::ResetDefaults);
        app.update();
        assert_eq!(*app.world().resource::<Settings>(), Settings::default());
    }

    #[test]
    fn controls_tab_throw_and_reset_bindings_do_not_overlap() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<Action>()
            .insert_resource(Settings::default())
            .insert_resource(SettingsPath(None))
            .add_plugins(OptionsPlugin);
        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Options);
        app.update();
        app.world_mut()
            .write_message(Action::SelectOptionsTab(OptionsTab::Controls));
        app.update();

        let mut throw_pos = None;
        let mut reset_pos = None;
        for (btn, node) in app
            .world_mut()
            .query::<(&super::super::MenuButton, &Node)>()
            .iter(app.world())
        {
            if btn.action == Action::StartRebind(GameAction::Throw) {
                throw_pos = Some((node.left, node.top));
            } else if btn.action == Action::ResetBindings {
                reset_pos = Some((node.left, node.top));
            }
        }
        assert!(throw_pos.is_some(), "Throw rebind button must exist");
        assert!(reset_pos.is_some(), "Reset Bindings button must exist");
        assert_ne!(
            throw_pos, reset_pos,
            "Throw button and Reset Bindings button must not share the same position"
        );

        let copy = format_bindings_copy(&crate::input_map::InputBindings::default());
        assert!(copy.contains("KeyT"));
    }
}
