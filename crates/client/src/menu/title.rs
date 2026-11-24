//! Title screen layout, original procedural wordmark, and buttons.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::style::{
    BORDER_W, BUTTON_HEIGHT, COLOR_BACKDROP, COLOR_HEADER_DIVIDER, COLOR_PANEL_BG,
    COLOR_PANEL_BORDER, COLOR_TEXT_GOLD, COLOR_TEXT_MUTED, RADIUS_PANEL, spawn_button_sized,
    viewport_size,
};
use super::text::{MenuText, MenuTextColor};
use super::{Action, Screen};
use crate::last_played::LastPlayed;
use crate::session::{AppState, MenuScreen, SaveRoot, SelectedCharacter, SelectedWorld};
use bevy::window::{PrimaryWindow, Window};
use terravibe_server::characters::{characters_dir, load_character};
use terravibe_server::worlds::{WorldInfo, list_worlds};
use terravibe_sim::Character;

pub fn generate_wordmark_image() -> Image {
    const LETTERS: [&str; 9] = [
        "11111\n00100\n00100\n00100\n00100\n00100\n00100", // T
        "11111\n10000\n11110\n10000\n10000\n10000\n11111", // E
        "11110\n10001\n10001\n11110\n10100\n10010\n10001", // R
        "11110\n10001\n10001\n11110\n10100\n10010\n10001", // R
        "01110\n10001\n10001\n11111\n10001\n10001\n10001", // A
        "10001\n10001\n10001\n10001\n10001\n01010\n00100", // V
        "11111\n00100\n00100\n00100\n00100\n00100\n11111", // I
        "11110\n10001\n10001\n11110\n10001\n10001\n11110", // B
        "11111\n10000\n11110\n10000\n10000\n10000\n11111", // E
    ];

    // Grid: 9 letters * 5 cols + 8 spaces * 1 col = 53 cols; 7 rows
    let mut grid = [[false; 53]; 7];
    let mut x_offset = 0;
    for letter in LETTERS {
        for (r, line) in letter.lines().enumerate() {
            for (c, ch) in line.chars().enumerate() {
                if ch == '1' {
                    grid[r][x_offset + c] = true;
                }
            }
        }
        x_offset += 6; // 5 + 1 spacing
    }

    let scale = 8usize;
    let pad_px = 4usize;
    let vine_extra = 2usize;
    let img_w = 53 * scale + pad_px;
    let img_h = (7 + vine_extra) * scale + pad_px;
    let mut data = vec![0u8; img_w * img_h * 4];

    let set_px = |buf: &mut [u8], x: usize, y: usize, rgba: [u8; 4]| {
        if x < img_w && y < img_h {
            let idx = (y * img_w + x) * 4;
            buf[idx..idx + 4].copy_from_slice(&rgba);
        }
    };

    // 1. Crisp 3px pixel-level drop shadow (no giant cell smear)
    let shadow_color = [10, 12, 18, 200];
    for r in 0..7 {
        for c in 0..53 {
            if grid[r][c] {
                let px = c * scale;
                let py = r * scale;
                for off in 1..=3 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            set_px(&mut data, px + sx + off, py + sy + off, shadow_color);
                        }
                    }
                }
            }
        }
    }

    // 2. Sculpted stone-and-timber front face with crisp 1px dark outline
    for r in 0..7 {
        let base_color = match r {
            0 => [212, 208, 202, 255],
            1 => [174, 170, 166, 255],
            2 => [140, 136, 132, 255],
            3 => [110, 106, 102, 255],
            4 => [194, 130, 72, 255],
            5 => [154, 96, 48, 255],
            _ => [112, 68, 34, 255],
        };
        for c in 0..53 {
            if grid[r][c] {
                let px = c * scale;
                let py = r * scale;
                for sy in 0..scale {
                    for sx in 0..scale {
                        let edge_l = sx == 0 && (c == 0 || !grid[r][c - 1]);
                        let edge_r = sx == scale - 1 && (c + 1 == 53 || !grid[r][c + 1]);
                        let edge_t = sy == 0 && (r == 0 || !grid[r - 1][c]);
                        let edge_b = sy == scale - 1 && (r + 1 == 7 || !grid[r + 1][c]);
                        let outline = edge_l || edge_r || edge_t || edge_b;
                        let inner_bevel = (sx <= 1 && !edge_l) || (sy <= 1 && !edge_t);

                        let mut px_color = if outline {
                            [24, 20, 18, 255]
                        } else if inner_bevel && r <= 3 {
                            [236, 232, 224, 255]
                        } else if inner_bevel {
                            [222, 158, 96, 255]
                        } else {
                            base_color
                        };

                        // Moss creeping on top edges of stone
                        if r == 0 && !outline && sy <= 3 {
                            px_color = if (c + sx) % 2 == 0 {
                                [128, 218, 68, 255]
                            } else {
                                [88, 178, 48, 255]
                            };
                        }

                        set_px(&mut data, px + sx, py + sy, px_color);
                    }
                }
            }
        }
    }

    // 3. Overgrown leafy vines hanging down below letters
    for c in 0..53 {
        if grid[6][c] && ((c * 13 + 7) % 6 == 0 || c % 8 == 2) {
            let px = c * scale;
            let py = 7 * scale;
            for sy in 0..scale {
                for sx in 2..5 {
                    set_px(&mut data, px + sx, py + sy, [45, 125, 35, 255]);
                }
            }
            let py2 = 8 * scale;
            for sy in 0..5 {
                for sx in 1..6 {
                    set_px(&mut data, px + sx, py2 + sy, [95, 220, 65, 255]);
                }
            }
        }
    }

    Image::new(
        Extent3d {
            width: img_w as u32,
            height: img_h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

#[derive(Resource)]
pub struct TitleAssets {
    pub wordmark: Handle<Image>,
}

pub fn setup_title_assets(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let wordmark = images.add(generate_wordmark_image());
    commands.insert_resource(TitleAssets { wordmark });
}

/// Applies a resolved Continue target to the session resources and requests gameplay.
///
/// Kept pure so activation can be tested without writing to the real character
/// directory or starting a server session.
pub fn apply_continue_target(
    character: Character,
    world: WorldInfo,
    selected_char: &mut SelectedCharacter,
    selected_world: &mut SelectedWorld,
    next_state: &mut NextState<AppState>,
) {
    selected_char.0 = Some(character);
    selected_world.name = world.name;
    selected_world.seed = world.seed;
    selected_world.size = world.size;
    selected_world.persist = true;
    next_state.set(AppState::InGame);
}

const MAX_CONTINUE_LABEL_CHARS: usize = 26;

/// Formats the title-screen Continue button label, truncating cleanly with `...`
/// so long character or world names never overflow the button border.
pub fn format_continue_label(character: &str, world_name: &str) -> String {
    let full = format!("Continue: {character} ({world_name})");
    if full.chars().count() <= MAX_CONTINUE_LABEL_CHARS {
        full
    } else {
        let mut truncated: String = full.chars().take(MAX_CONTINUE_LABEL_CHARS - 3).collect();
        truncated.push_str("...");
        truncated
    }
}

pub fn setup_title(
    mut commands: Commands,
    assets: Option<Res<TitleAssets>>,
    save_root: Option<Res<SaveRoot>>,
    last_played: Option<Res<LastPlayed>>,
    mut images: ResMut<Assets<Image>>,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
) {
    let (win_w, win_h) = viewport_size(primary_window.as_ref());
    let center_x = win_w * 0.5;
    let card_w = 360.0;
    let btn_w = 300.0;
    let btn_x = center_x - btn_w * 0.5;

    // Wordmark logo
    let wordmark = if let Some(assets) = assets {
        assets.wordmark.clone()
    } else {
        let handle = images.add(generate_wordmark_image());
        commands.insert_resource(TitleAssets {
            wordmark: handle.clone(),
        });
        handle
    };

    let base_y = ((win_h - 520.0) * 0.42).max(36.0);

    // Subtle twilight atmospheric backdrop scrim
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            top: Val::Px(0.0),
            width: Val::Px(win_w),
            height: Val::Px(win_h),
            ..default()
        },
        BackgroundColor(COLOR_BACKDROP),
        DespawnOnExit(MenuScreen::Title),
    ));

    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(center_x - 228.0),
            top: Val::Px(base_y),
            width: Val::Px(456.0),
            height: Val::Px(88.0),
            ..default()
        },
        ImageNode::new(wordmark),
        DespawnOnExit(MenuScreen::Title),
    ));

    // Subtitle & version label below the wordmark
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(center_x - 220.0),
            top: Val::Px(base_y + 92.0),
            ..default()
        },
        MenuText::new("Dig. Build. Explore. Survive.", 0.9),
        MenuTextColor(COLOR_TEXT_MUTED),
        DespawnOnExit(MenuScreen::Title),
    ));

    let version_str = format!("v{}", env!("CARGO_PKG_VERSION"));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(center_x + 150.0),
            top: Val::Px(base_y + 92.0),
            ..default()
        },
        MenuText::new(version_str, 0.9),
        MenuTextColor(COLOR_TEXT_GOLD),
        DespawnOnExit(MenuScreen::Title),
    ));

    // Continue is offered only when the last-played world still exists on disk.
    let continue_target = save_root.as_ref().and_then(|root| {
        let last = last_played
            .as_ref()
            .map(|lp| (**lp).clone())
            .unwrap_or_else(|| LastPlayed::load(&root.0));
        let character = last.last_character?;
        let world_name = last.last_world?;
        let world = list_worlds(&root.0)
            .ok()?
            .into_iter()
            .find(|w| w.name == world_name && !w.damaged)?;
        Some((character, world))
    });

    let has_continue = continue_target.is_some();
    let button_count = if has_continue { 5 } else { 4 };
    let top_pad = 26.0;
    let step = 56.0;
    let bottom_pad = 26.0;
    let card_h = top_pad + (button_count - 1) as f32 * step + BUTTON_HEIGHT + bottom_pad;
    let card_top = base_y + 124.0;

    // Gold-framed action card
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(center_x - card_w * 0.5),
                top: Val::Px(card_top),
                width: Val::Px(card_w),
                height: Val::Px(card_h),
                border: UiRect::all(Val::Px(BORDER_W)),
                border_radius: BorderRadius::all(Val::Px(RADIUS_PANEL)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::top(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(COLOR_PANEL_BG),
            BorderColor::all(COLOR_PANEL_BORDER),
            GlobalZIndex(10),
            DespawnOnExit(MenuScreen::Title),
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    width: Val::Px(card_w - 36.0),
                    height: Val::Px(2.0),
                    margin: UiRect::top(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(COLOR_HEADER_DIVIDER),
            ));
        });

    let btn_start_y = card_top + top_pad;

    let (rank_offset, y_offset) = if let Some((character, world)) = &continue_target {
        let label = format_continue_label(character, &world.name);
        let b = spawn_button_sized(
            &mut commands,
            &label,
            Action::ContinueLastPlayed,
            0,
            (btn_x, btn_start_y),
            (btn_w, BUTTON_HEIGHT),
            false,
        );
        commands
            .entity(b)
            .insert((DespawnOnExit(MenuScreen::Title), GlobalZIndex(11)));
        (1u16, step)
    } else {
        (0u16, 0.0f32)
    };

    // Main menu buttons
    let b0 = spawn_button_sized(
        &mut commands,
        "Single Player",
        Action::OpenScreen(Screen::Characters),
        rank_offset,
        (btn_x, btn_start_y + y_offset),
        (btn_w, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b0)
        .insert((DespawnOnExit(MenuScreen::Title), GlobalZIndex(11)));

    let b1 = spawn_button_sized(
        &mut commands,
        "Multiplayer",
        Action::OpenScreen(Screen::Multiplayer),
        rank_offset + 1,
        (btn_x, btn_start_y + step + y_offset),
        (btn_w, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b1)
        .insert((DespawnOnExit(MenuScreen::Title), GlobalZIndex(11)));

    let b2 = spawn_button_sized(
        &mut commands,
        "Options",
        Action::OpenScreen(Screen::Options),
        rank_offset + 2,
        (btn_x, btn_start_y + step * 2.0 + y_offset),
        (btn_w, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b2)
        .insert((DespawnOnExit(MenuScreen::Title), GlobalZIndex(11)));

    let b3 = spawn_button_sized(
        &mut commands,
        "Quit",
        Action::Quit,
        rank_offset + 3,
        (btn_x, btn_start_y + step * 3.0 + y_offset),
        (btn_w, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b3)
        .insert((DespawnOnExit(MenuScreen::Title), GlobalZIndex(11)));

    // Footer with save root only when TERRAVIBE_DEBUG is set
    if std::env::var_os("TERRAVIBE_DEBUG").is_some() {
        let footer_text = match save_root {
            Some(r) => format!("Save root: {}", r.0.display()),
            None => "Save root: (default)".to_string(),
        };
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                bottom: Val::Px(24.0),
                ..default()
            },
            MenuText::new(footer_text, 0.8),
            DespawnOnExit(MenuScreen::Title),
        ));
    }
}

/// Loads the last-played character and world, then requests a game session.
///
/// If either file is missing or unreadable, nothing happens; the player can
/// still pick another character or world from the menus.
pub fn handle_continue_action(
    actions: Option<MessageReader<Action>>,
    save_root: Option<Res<SaveRoot>>,
    last_played: Option<Res<LastPlayed>>,
    selected_char: Option<ResMut<SelectedCharacter>>,
    selected_world: Option<ResMut<SelectedWorld>>,
    next_state: Option<ResMut<NextState<AppState>>>,
) {
    let Some(mut actions) = actions else {
        return;
    };
    if !actions
        .read()
        .any(|action| matches!(action, Action::ContinueLastPlayed))
    {
        return;
    }

    let Some(save_root) = save_root else {
        return;
    };
    let last = last_played
        .as_ref()
        .map(|lp| (**lp).clone())
        .unwrap_or_else(|| LastPlayed::load(&save_root.0));
    let (Some(char_name), Some(world_name)) =
        (last.last_character.as_deref(), last.last_world.as_deref())
    else {
        return;
    };

    let Ok(character) = load_character(&characters_dir(), char_name) else {
        return;
    };
    let Ok(worlds) = list_worlds(&save_root.0) else {
        return;
    };
    let Some(world) = worlds
        .into_iter()
        .find(|w| w.name == world_name && !w.damaged)
    else {
        return;
    };

    let (Some(mut selected_char), Some(mut selected_world), Some(mut next_state)) =
        (selected_char, selected_world, next_state)
    else {
        return;
    };
    apply_continue_target(
        character,
        world,
        &mut selected_char,
        &mut selected_world,
        &mut next_state,
    );
}

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_title_assets)
            .add_systems(OnEnter(MenuScreen::Title), setup_title)
            .add_systems(
                Update,
                handle_continue_action.run_if(in_state(MenuScreen::Title)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::MenuButton;
    use crate::session::{AppState, SelectedCharacter, SelectedWorld};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::SystemTime;
    use terravibe_server::worlds::{WorldInfo, create_world};
    use terravibe_sim::WorldSize;

    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_root() -> (PathBuf, TempDir) {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "tv_title_test_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (dir.clone(), TempDir(dir))
    }

    fn test_title_app(root: PathBuf) -> App {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<Assets<Image>>()
            .insert_resource(SaveRoot(root))
            .add_plugins(TitlePlugin);
        app
    }

    fn create_dummy_world(root: &Path, name: &str) {
        create_world(root, name, 42, WorldSize::Small).expect("create dummy world");
    }

    fn find_buttons_with_action(app: &mut App, action: Action) -> Vec<Entity> {
        app.world_mut()
            .query::<(&MenuButton, Entity)>()
            .iter(app.world())
            .filter(|(button, _)| button.action == action)
            .map(|(_, entity)| entity)
            .collect()
    }

    #[test]
    fn continue_button_spawned_when_last_played_valid() {
        let (root, _guard) = temp_root();
        let mut app = test_title_app(root.clone());
        create_dummy_world(&root, "Terra");
        app.insert_resource(LastPlayed {
            last_character: Some("Alice".into()),
            last_world: Some("Terra".into()),
        });

        app.update();
        let buttons = find_buttons_with_action(&mut app, Action::ContinueLastPlayed);
        assert_eq!(buttons.len(), 1);
    }

    #[test]
    fn continue_button_hidden_when_no_last_played() {
        let (root, _guard) = temp_root();
        let mut app = test_title_app(root);
        app.insert_resource(LastPlayed::default());
        app.update();
        let buttons = find_buttons_with_action(&mut app, Action::ContinueLastPlayed);
        assert!(buttons.is_empty());
    }

    #[test]
    fn continue_button_shifts_title_ranks() {
        let (root, _guard) = temp_root();
        let mut app = test_title_app(root.clone());
        create_dummy_world(&root, "Terra");
        app.insert_resource(LastPlayed {
            last_character: Some("Alice".into()),
            last_world: Some("Terra".into()),
        });

        app.update();
        let mut buttons: Vec<MenuButton> = app
            .world_mut()
            .query::<&MenuButton>()
            .iter(app.world())
            .cloned()
            .collect();
        buttons.sort_by_key(|b| b.rank);

        assert_eq!(buttons.len(), 5);
        assert_eq!(
            buttons.iter().map(|b| b.rank).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
        assert_eq!(buttons[0].action, Action::ContinueLastPlayed);
        assert_eq!(buttons[1].action, Action::OpenScreen(Screen::Characters));
        assert_eq!(buttons[2].action, Action::OpenScreen(Screen::Multiplayer));
        assert_eq!(buttons[3].action, Action::OpenScreen(Screen::Options));
        assert_eq!(buttons[4].action, Action::Quit);
    }

    #[test]
    fn continue_activation_sets_selection_and_requests_ingame() {
        let mut selected_char = SelectedCharacter(None);
        let mut selected_world = SelectedWorld::default();
        let mut next_state = NextState::<AppState>::default();
        let mut character = terravibe_sim::Character::default();
        character.name = "Alice".into();
        let world = WorldInfo {
            name: "Terra".into(),
            seed: 1234,
            clock_tick: 13_500,
            size: WorldSize::Medium,
            last_played: SystemTime::now(),
            damaged: false,
        };

        apply_continue_target(
            character,
            world,
            &mut selected_char,
            &mut selected_world,
            &mut next_state,
        );

        assert_eq!(
            selected_char.0.as_ref().map(|c| c.name.as_str()),
            Some("Alice")
        );
        assert_eq!(selected_world.name, "Terra");
        assert_eq!(selected_world.seed, 1234);
        assert_eq!(selected_world.size, WorldSize::Medium);
        assert!(selected_world.persist);
        assert!(matches!(next_state, NextState::Pending(AppState::InGame)));
    }

    #[test]
    fn continue_button_hidden_when_only_character_set() {
        let (root, _guard) = temp_root();
        let mut app = test_title_app(root.clone());
        create_dummy_world(&root, "Terra");
        app.insert_resource(LastPlayed {
            last_character: Some("Alice".into()),
            last_world: None,
        });

        app.update();
        let buttons = find_buttons_with_action(&mut app, Action::ContinueLastPlayed);
        assert!(buttons.is_empty());
    }

    #[test]
    fn continue_button_hidden_when_only_world_set() {
        let (root, _guard) = temp_root();
        let mut app = test_title_app(root.clone());
        create_dummy_world(&root, "Terra");
        app.insert_resource(LastPlayed {
            last_character: None,
            last_world: Some("Terra".into()),
        });

        app.update();
        let buttons = find_buttons_with_action(&mut app, Action::ContinueLastPlayed);
        assert!(buttons.is_empty());
    }

    #[test]
    fn continue_button_hidden_when_world_deleted() {
        let (root, _guard) = temp_root();
        let mut app = test_title_app(root.clone());
        create_dummy_world(&root, "Terra");
        std::fs::remove_dir_all(root.join("Terra")).unwrap();
        app.insert_resource(LastPlayed {
            last_character: Some("Alice".into()),
            last_world: Some("Terra".into()),
        });

        app.update();
        let buttons = find_buttons_with_action(&mut app, Action::ContinueLastPlayed);
        assert!(buttons.is_empty());
    }

    #[test]
    fn continue_button_hidden_when_world_damaged() {
        let (root, _guard) = temp_root();
        let mut app = test_title_app(root.clone());
        create_dummy_world(&root, "Terra");
        std::fs::write(root.join("Terra").join("world.bin"), b"not a world header").unwrap();
        app.insert_resource(LastPlayed {
            last_character: Some("Alice".into()),
            last_world: Some("Terra".into()),
        });

        app.update();
        let buttons = find_buttons_with_action(&mut app, Action::ContinueLastPlayed);
        assert!(buttons.is_empty());
    }

    #[test]
    fn continue_label_truncates_long_names_cleanly() {
        assert_eq!(
            format_continue_label("Alice", "Terra"),
            "Continue: Alice (Terra)"
        );
        let long = format_continue_label("VeryLongCharacterName", "ExtremelyLongWorldName");
        assert!(
            long.chars().count() <= MAX_CONTINUE_LABEL_CHARS,
            "truncated label '{}' exceeded {} chars",
            long,
            MAX_CONTINUE_LABEL_CHARS
        );
        assert!(
            long.ends_with("..."),
            "truncated label '{long}' must end with '...'"
        );
    }

    #[test]
    fn title_buttons_sit_inside_card_with_z_above_panel() {
        for with_continue in [false, true] {
            let (root, _guard) = temp_root();
            let mut app = test_title_app(root.clone());
            if with_continue {
                create_dummy_world(&root, "VeryLongWorldNameForBoundsTest");
                app.insert_resource(LastPlayed {
                    last_character: Some("VeryLongCharacterName".into()),
                    last_world: Some("VeryLongWorldNameForBoundsTest".into()),
                });
            } else {
                app.insert_resource(LastPlayed::default());
            }
            app.update();

            let mut panel_query = app.world_mut().query_filtered::<(
                &Node,
                &BackgroundColor,
                Option<&GlobalZIndex>,
            ), Without<MenuButton>>();
            let panels: Vec<_> = panel_query
                .iter(app.world())
                .filter(|(_, bg, _)| bg.0 == COLOR_PANEL_BG)
                .collect();
            assert_eq!(panels.len(), 1, "expected exactly 1 title panel");
            let (panel_node, _, panel_z) = panels[0];
            let panel_z = panel_z
                .map(|z| z.0)
                .expect("title panel must have GlobalZIndex");
            assert_eq!(panel_z, 10);

            let (Val::Px(p_left), Val::Px(p_top), Val::Px(p_w), Val::Px(p_h)) = (
                panel_node.left,
                panel_node.top,
                panel_node.width,
                panel_node.height,
            ) else {
                panic!("title panel must use Px geometry");
            };

            let mut btn_query = app
                .world_mut()
                .query_filtered::<(&MenuButton, &Node, Option<&GlobalZIndex>), With<MenuButton>>();
            let buttons: Vec<_> = btn_query.iter(app.world()).collect();
            assert_eq!(buttons.len(), if with_continue { 5 } else { 4 });

            for (btn, node, maybe_z) in buttons {
                let btn_z = maybe_z
                    .map(|z| z.0)
                    .unwrap_or_else(|| panic!("button rank {} must have GlobalZIndex", btn.rank));
                assert!(
                    btn_z > panel_z,
                    "button rank {} (z={btn_z}) must stack above panel (z={panel_z})",
                    btn.rank
                );
                let (Val::Px(b_left), Val::Px(b_top), Val::Px(b_w), Val::Px(b_h)) =
                    (node.left, node.top, node.width, node.height)
                else {
                    panic!("title button must use Px geometry");
                };
                assert!(b_left >= p_left && b_left + b_w <= p_left + p_w);
                assert!(b_top >= p_top && b_top + b_h <= p_top + p_h);
            }
        }
    }
}
