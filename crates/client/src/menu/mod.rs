//! Menu framework, screens, and navigation.

pub mod characters;
pub mod create;
pub mod create_character;
pub mod creative;
pub mod diorama;
pub mod game;
pub mod host_setup;
pub mod join_browser;
pub mod multiplayer;
pub mod nav;
pub mod options;
pub mod sounds;
pub mod style;
pub mod text;
pub mod title;
pub mod worlds;

use crate::session::MenuScreen;
use bevy::prelude::*;
pub use game::menu_closed;
pub use nav::{MenuFocus, next_rank};
pub use sounds::{MenuAudio, MenuSounds};
pub use style::{BUTTON_HEIGHT, BUTTON_WIDTH, spawn_button, spawn_button_ext, spawn_button_sized};
pub use text::MenuText;

/// Tracks whether the player is navigating through single-player, host, or join flows.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MenuFlow {
    #[default]
    SinglePlayer,
    Host,
    Join,
}

/// Sub-screens accessible from the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Screen {
    Characters,
    CreateCharacter,
    Worlds,
    CreateWorld,
    Options,
    Multiplayer,
    HostSetup,
    JoinBrowser,
}

impl Screen {
    pub fn to_menu_screen(self) -> MenuScreen {
        match self {
            Screen::Characters => MenuScreen::Characters,
            Screen::CreateCharacter => MenuScreen::CreateCharacter,
            Screen::Worlds => MenuScreen::Worlds,
            Screen::CreateWorld => MenuScreen::CreateWorld,
            Screen::Options => MenuScreen::Options,
            Screen::Multiplayer => MenuScreen::Multiplayer,
            Screen::HostSetup => MenuScreen::HostSetup,
            Screen::JoinBrowser => MenuScreen::JoinBrowser,
        }
    }
}

/// Settings adjustable through menu actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Setting {
    Volume,
    MusicVolume,
    AmbientVolume,
    UiVolume,
    Sensitivity,
    Fov,
    UiScale,
    HudOpacity,
    TooltipDelay,
    ScreenShake,
}

/// Actions triggered by menu buttons or keyboard shortcuts.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Quit,
    OpenScreen(Screen),
    Back,
    Noop,
    Play(String),
    LoadBackup(String),
    Delete(String),
    ConfirmDelete,
    CancelDelete,
    ContinueLastPlayed,
    PlayCharacter(String),
    DeleteCharacter(String),
    ConfirmDeleteCharacter,
    CancelDeleteCharacter,
    CycleHair,
    CycleSkin,
    CycleHairColor,
    CycleEyes,
    CycleShirt,
    CyclePants,
    RandomizeCharacter,
    RotatePreviewLeft,
    RotatePreviewRight,
    CreateCharacterConfirm,
    CreateWorld,
    RandomSeed,
    CycleWorldSize,
    SelectWorldSize(terravibe_sim::WorldSize),
    Adjust(Setting, i32),
    CycleWindowMode,
    CycleResolution,
    CycleQualityPreset,
    ToggleVsync,
    ToggleInvertY,
    ToggleWeatherEffects,
    ToggleAmbientParticles,
    ToggleAtmosphereFx,
    ToggleMuteOnFocusLoss,
    ToggleColorblindPalette,
    ToggleHighContrastCrosshair,
    ToggleHitstop,
    ToggleDamageNumbers,
    ToggleReducedMotion,
    SelectOptionsTab(options::OptionsTab),
    StartRebind(crate::input_map::GameAction),
    ResetBindings,
    ResetDefaults,
    Resume,
    OpenCreative,
    GameOptions,
    GameControls,
    ResumeRoot,
    SaveQuit,
    ConfirmQuit,
    CancelQuit,
    DismissError,
    RetryConnection,
    StartHostFlow,
    StartJoinFlow,
    HostMaxPlayersDelta(i32),
    HostTogglePublic,
    HostLaunch,
    JoinByCode,
    RefreshServers,
    JoinRoom(usize),
    ConfirmRoomPassword,
    CancelRoomPassword,
}

/// A button in the menu UI with navigation rank and action.
#[derive(Component, Clone, Debug)]
pub struct MenuButton {
    pub action: Action,
    pub rank: u16,
    pub disabled: bool,
}

/// Pure hit-test determining if `point` lies within a rectangle defined by its `center` and `size`.
pub fn point_in_rect(center: Vec2, size: Vec2, point: Vec2) -> bool {
    let half = size * 0.5;
    (point.x - center.x).abs() <= half.x && (point.y - center.y).abs() <= half.y
}

#[derive(Default, Clone)]
pub struct MenuPlugin {
    pub sounds: MenuSounds,
}

fn reset_menu_flow_on_title(mut flow: ResMut<MenuFlow>) {
    *flow = MenuFlow::SinglePlayer;
}

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Action>();
        app.init_resource::<MenuFocus>()
            .init_resource::<MenuFlow>()
            .add_systems(OnEnter(MenuScreen::Title), reset_menu_flow_on_title)
            .add_plugins((
                style::StylePlugin,
                nav::NavPlugin,
                text::TextPlugin,
                sounds::SoundsPlugin {
                    sounds: self.sounds.clone(),
                },
                title::TitlePlugin,
                characters::CharactersPlugin,
                create_character::CreateCharacterPlugin,
                multiplayer::MultiplayerPlugin,
                host_setup::HostSetupPlugin,
                join_browser::JoinBrowserPlugin,
            ))
            .add_plugins((
                worlds::WorldsPlugin,
                create::CreatePlugin,
                options::OptionsPlugin,
                diorama::DioramaPlugin,
                game::GameMenuPlugin,
                creative::CreativePlugin,
            ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::AppState;

    #[test]
    fn point_in_rect_hits_and_misses() {
        assert!(point_in_rect(
            Vec2::new(0., 0.),
            Vec2::new(10., 10.),
            Vec2::new(4., 4.)
        ));
        assert!(!point_in_rect(
            Vec2::new(0., 0.),
            Vec2::new(10., 10.),
            Vec2::new(6., 6.)
        ));
    }

    #[test]
    fn focus_moves_in_rank_order_wraps_and_skips_disabled() {
        assert_eq!(next_rank(&[0, 1, 2], &[1], 0, 1), 2);
        assert_eq!(next_rank(&[0, 1, 2], &[1], 2, 1), 0);
        assert_eq!(next_rank(&[0, 1, 2], &[1], 0, -1), 2);
    }

    #[test]
    fn activate_runs_open_screen_and_quit() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .add_message::<AppExit>()
            .add_systems(Update, nav::activate_focused_button);

        app.world_mut().spawn(MenuButton {
            action: Action::OpenScreen(Screen::Worlds),
            rank: 0,
            disabled: false,
        });
        app.world_mut().resource_mut::<MenuFocus>().rank = Some(0);

        app.update();
        app.update();
        let screen = app.world().resource::<State<MenuScreen>>();
        assert_eq!(*screen.get(), MenuScreen::Worlds);

        app.world_mut().spawn(MenuButton {
            action: Action::Quit,
            rank: 1,
            disabled: false,
        });
        app.world_mut().resource_mut::<MenuFocus>().rank = Some(1);

        app.update();
        let exits = app.world().resource::<Messages<AppExit>>();
        let mut cursor = exits.get_cursor();
        let last_exit = cursor.read(exits).last();
        assert_eq!(last_exit, Some(&AppExit::Success));
    }

    #[test]
    fn multi_screen_has_host_join_and_back_enabled() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .add_plugins(multiplayer::MultiplayerPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Multiplayer);
        app.update();

        let mut buttons = app.world_mut().query::<&MenuButton>();
        let list: Vec<MenuButton> = buttons.iter(app.world()).cloned().collect();
        assert_eq!(
            list.len(),
            3,
            "multiplayer screen must spawn Host, Join, and Back buttons"
        );

        let host_buttons: Vec<_> = list
            .iter()
            .filter(|b| b.action == Action::StartHostFlow)
            .collect();
        assert_eq!(host_buttons.len(), 1, "exactly one Host button expected");
        assert!(!host_buttons[0].disabled, "Host button must be enabled");

        let join_buttons: Vec<_> = list
            .iter()
            .filter(|b| b.action == Action::StartJoinFlow)
            .collect();
        assert_eq!(join_buttons.len(), 1, "exactly one Join button expected");
        assert!(!join_buttons[0].disabled, "Join button must be enabled");

        let back_buttons: Vec<_> = list.iter().filter(|b| b.action == Action::Back).collect();
        assert_eq!(back_buttons.len(), 1, "exactly one Back button expected");
        assert!(!back_buttons[0].disabled, "Back button must be enabled");

        assert!(
            list.iter().all(|b| !b.disabled),
            "all multiplayer buttons must be enabled"
        );
    }

    #[test]
    fn disabled_buttons_neither_focus_nor_activate() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .add_message::<AppExit>()
            .add_systems(
                Update,
                (nav::handle_nav_input, nav::activate_focused_button),
            );

        app.world_mut().spawn(MenuButton {
            action: Action::Quit,
            rank: 0,
            disabled: true,
        });

        app.update();
        assert_eq!(app.world().resource::<MenuFocus>().rank, None);

        let exits = app.world().resource::<Messages<AppExit>>();
        assert!(exits.is_empty());
    }

    #[test]
    fn sound_volume_gain_applied_on_menu_audio() {
        use crate::settings::volume_gain;
        assert_eq!(volume_gain(0), 0.0);
        assert_eq!(volume_gain(75), 0.75);
        assert_eq!(volume_gain(100), 1.0);
    }

    #[test]
    fn title_screen_has_four_buttons() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .init_resource::<Assets<Image>>()
            .add_plugins(title::TitlePlugin);

        app.update();
        app.update();

        let mut buttons = app.world_mut().query::<&MenuButton>();
        let list: Vec<MenuButton> = buttons.iter(app.world()).cloned().collect();
        assert_eq!(list.len(), 4, "Title screen should have 4 buttons");

        let ranks: Vec<u16> = list.iter().map(|b| b.rank).collect();
        assert_eq!(ranks, vec![0, 1, 2, 3]);

        assert_eq!(list[0].action, Action::OpenScreen(Screen::Characters));
        assert_eq!(list[1].action, Action::OpenScreen(Screen::Multiplayer));
        assert_eq!(list[2].action, Action::OpenScreen(Screen::Options));
        assert_eq!(list[3].action, Action::Quit);
    }

    #[test]
    fn diorama_camera_has_menu_despawn() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<Time>()
            .add_plugins(diorama::DioramaPlugin);

        app.update();

        let mut cameras =
            app.world_mut()
                .query::<(&Camera3d, &DespawnOnExit<AppState>, Has<IsDefaultUiCamera>)>();
        let cam = cameras.iter(app.world()).next();
        assert!(
            cam.is_some(),
            "Diorama camera must be spawned in AppState::Menu"
        );
        let (_, despawn, is_default_ui) = cam.unwrap();
        assert_eq!(despawn.0, AppState::Menu);
        assert!(is_default_ui, "Diorama camera must have IsDefaultUiCamera");
    }

    #[test]
    fn focused_action_not_executed_without_activation_key() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<AppExit>()
            .add_plugins(nav::NavPlugin);

        app.world_mut().spawn(MenuButton {
            action: Action::Quit,
            rank: 0,
            disabled: false,
        });
        app.world_mut().resource_mut::<MenuFocus>().rank = Some(0);

        // Multiple frames with no key input must NOT fire the focused action
        for _ in 0..5 {
            app.update();
        }

        let exits = app.world().resource::<Messages<AppExit>>();
        assert!(
            exits.is_empty(),
            "focused button action must not execute without an activation key"
        );

        // Pressing Enter activates the focused button
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();

        let exits = app.world().resource::<Messages<AppExit>>();
        let mut cursor = exits.get_cursor();
        assert_eq!(cursor.read(exits).last(), Some(&AppExit::Success));
    }

    #[test]
    fn mouse_hover_and_click_updates_focus_and_activates() {
        use bevy::ui::UiGlobalTransform;

        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<AppExit>()
            .add_systems(Update, nav::handle_mouse_input);

        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::new(100.0, 100.0)));
        app.world_mut().spawn(window);

        let mut node = ComputedNode::default();
        node.size = Vec2::new(200.0, 40.0);
        app.world_mut().spawn((
            MenuButton {
                action: Action::Quit,
                rank: 2,
                disabled: false,
            },
            node,
            UiGlobalTransform::from_xy(100.0, 100.0),
        ));

        // Press left mouse button, then update
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(app.world().resource::<MenuFocus>().rank, Some(2));
        assert!(app.world().resource::<Messages<AppExit>>().is_empty());

        // Releasing left mouse button activates the hovered button
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();

        let exits = app.world().resource::<Messages<AppExit>>();
        let mut cursor = exits.get_cursor();
        assert_eq!(cursor.read(exits).last(), Some(&AppExit::Success));
    }

    #[test]
    fn rebuild_menu_text_sizes_parent_node() {
        use std::sync::Arc;

        let mut app = App::new();
        let font = terravibe_assets::SpriteFont {
            atlas: terravibe_assets::RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![[0; 4]],
            },
            glyphs: vec![terravibe_assets::Glyph {
                ch: 'x',
                src: [0, 0, 10, 12],
                crop: [0, 0, 10, 12],
                kerning: [1.0, 10.0, 1.0],
            }],
            line_spacing: 16,
            spacing: 2.0,
            default_char: None,
        };
        let font_arc = Arc::new(font.clone());
        let image = Handle::default();
        app.insert_resource(crate::hud::HudFonts {
            mouse: (font_arc.clone(), image.clone()),
            death: (font_arc.clone(), image.clone()),
            stack: (font_arc, image),
        });
        app.add_plugins(text::TextPlugin);

        let entity = app
            .world_mut()
            .spawn((Node::default(), MenuText::new("x\nx", 1.0)))
            .id();

        app.update();

        let expected = text::measure_scaled(&font, "x\nx", 1.0);
        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(node.width, Val::Px(expected.x));
        assert_eq!(node.height, Val::Px(expected.y));
        assert!(expected.x > 0.0);
        assert!(expected.y > 0.0);
    }

    #[test]
    fn multiplayer_copy_fits_within_560px() {
        if let Ok(tc) = terravibe_assets::TerrariaContent::locate(None) {
            if let Ok(font) = tc.font("Fonts/Mouse_Text") {
                let sz = text::measure_scaled(&font, multiplayer::MULTIPLAYER_COPY, 0.9);
                assert!(
                    sz.x <= 560.0,
                    "multiplayer copy widest line ({}) exceeds 560px",
                    sz.x
                );
                for line in multiplayer::MULTIPLAYER_COPY.lines() {
                    let w = text::measure_scaled(&font, line, 0.9).x;
                    assert!(w <= 560.0, "line {:?} width {} exceeds 560px", line, w);
                }
            }
        }
    }

    fn assert_buttons_fit_without_overlap(app: &mut App, win_w: f32, win_h: f32, label: &str) {
        let mut q = app.world_mut().query::<(&MenuButton, &Node)>();
        let mut rects: Vec<(u16, f32, f32, f32, f32)> = Vec::new();
        for (btn, node) in q.iter(app.world()) {
            let (Val::Px(l), Val::Px(t), Val::Px(w), Val::Px(h)) =
                (node.left, node.top, node.width, node.height)
            else {
                continue;
            };
            assert!(
                l >= 0.0 && t >= 0.0 && l + w <= win_w && t + h <= win_h,
                "[{label}] {win_w}x{win_h}: button rank {} ({:?}) out of bounds: ({l}, {t}, {w}, {h})",
                btn.rank,
                btn.action
            );
            rects.push((btn.rank, l, t, l + w, t + h));
        }
        assert!(!rects.is_empty(), "[{label}] expected buttons");
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                let (r1, l1, t1, r1x, b1) = rects[i];
                let (r2, l2, t2, r2x, b2) = rects[j];
                let overlaps = l1 < r2x && r1x > l2 && t1 < b2 && b1 > t2;
                assert!(
                    !overlaps,
                    "[{label}] {win_w}x{win_h}: button {r1} ({l1},{t1}..{r1x},{b1}) overlaps {r2} ({l2},{t2}..{r2x},{b2})"
                );
            }
        }
    }

    #[test]
    fn all_menu_screens_fit_without_overlap_at_720p_and_1080p() {
        for (win_w, win_h) in [(1280.0, 720.0), (1920.0, 1080.0)] {
            for screen in [
                MenuScreen::Title,
                MenuScreen::Characters,
                MenuScreen::CreateCharacter,
                MenuScreen::Worlds,
                MenuScreen::CreateWorld,
                MenuScreen::Multiplayer,
                MenuScreen::HostSetup,
                MenuScreen::JoinBrowser,
            ] {
                let mut app = App::new();
                app.add_plugins(bevy::state::app::StatesPlugin)
                    .init_state::<AppState>()
                    .add_sub_state::<MenuScreen>()
                    .init_resource::<MenuFocus>()
                    .init_resource::<Assets<Image>>()
                    .add_message::<Action>()
                    .add_plugins((
                        title::TitlePlugin,
                        characters::CharactersPlugin,
                        create_character::CreateCharacterPlugin,
                        worlds::WorldsPlugin,
                        create::CreatePlugin,
                        multiplayer::MultiplayerPlugin,
                        host_setup::HostSetupPlugin,
                        join_browser::JoinBrowserPlugin,
                    ));
                app.world_mut().spawn((
                    Window {
                        resolution: bevy::window::WindowResolution::new(win_w as u32, win_h as u32),
                        ..default()
                    },
                    bevy::window::PrimaryWindow,
                ));
                app.world_mut()
                    .resource_mut::<NextState<MenuScreen>>()
                    .set(screen);
                app.update();
                app.update();
                assert_buttons_fit_without_overlap(&mut app, win_w, win_h, &format!("{screen:?}"));
            }
        }
    }
}
