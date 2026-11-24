//! Create World screen: world name, seed, evil/difficulty notices, and validation.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use terravibe_server::worlds::{NameError, create_world, validate_name};
use terravibe_sim::WorldSize;

use super::style::{
    BORDER_W, BUTTON_HEIGHT, COLOR_BACKDROP, COLOR_HEADER_DIVIDER, COLOR_INPUT_ACTIVE_BORDER,
    COLOR_INPUT_BG, COLOR_INPUT_IDLE_BORDER, COLOR_PANEL_BG, COLOR_PANEL_BORDER, COLOR_PANEL_INNER,
    COLOR_TEXT_GOLD, MenuSelected, RADIUS_BTN, RADIUS_PANEL, spawn_button_sized, viewport_size,
};
use super::text::{MenuText, MenuTextColor};
use super::{Action, MenuFocus};
use crate::session::{AppState, MenuScreen, SaveRoot, SelectedWorld};

/// Which text field is currently focused for typing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Field {
    #[default]
    Name,
    Seed,
}

/// In-progress drafts for world creation.
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct CreateDrafts {
    pub name: String,
    pub seed: String,
    pub field: Field,
    pub size: WorldSize,
}

/// Inline validation error messages.
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct CreateErrors {
    pub name: Option<String>,
    pub seed: Option<String>,
}

/// Pure deterministic pseudo-random seed generator from nanos and process ID.
pub fn random_seed(nanos: u128, pid: u32) -> u32 {
    let mut h = (nanos ^ ((pid as u128) << 32)).wrapping_mul(0x517cc1b727220a95);
    h ^= h >> 32;
    h = h.wrapping_mul(0x6c62272e07bb0142);
    h ^= h >> 32;
    let seed = ((h as u32) ^ ((h >> 32) as u32)) & 0x7fff_ffff;
    if seed == 0 { 1 } else { seed }
}

/// Feeds a character into the active text draft, filtering for printable ASCII.
pub fn apply_char(drafts: &mut CreateDrafts, errors: &mut CreateErrors, ch: char) {
    if !(ch.is_ascii_graphic() || ch == ' ') {
        match drafts.field {
            Field::Name => {
                errors.name = Some("Only printable ASCII characters are allowed".into());
            }
            Field::Seed => {
                errors.seed = Some("Only printable ASCII characters are allowed".into());
            }
        }
        return;
    }
    match drafts.field {
        Field::Name => {
            if drafts.name.chars().count() < 32 {
                drafts.name.push(ch);
                errors.name = None;
            }
        }
        Field::Seed => {
            if drafts.seed.len() < 10 {
                drafts.seed.push(ch);
                errors.seed = None;
            }
        }
    }
}

/// Handles Backspace on the active text draft.
pub fn apply_backspace(drafts: &mut CreateDrafts, errors: &mut CreateErrors) {
    match drafts.field {
        Field::Name => {
            drafts.name.pop();
            errors.name = None;
        }
        Field::Seed => {
            drafts.seed.pop();
            errors.seed = None;
        }
    }
}

/// Pure action handler for world creation.
pub fn execute_create_world(
    save_root: &Path,
    drafts: &CreateDrafts,
    errors: &mut CreateErrors,
    selected_world: &mut SelectedWorld,
    next_state: &mut NextState<AppState>,
) -> bool {
    let valid_name = match validate_name(&drafts.name) {
        Ok(n) => n,
        Err(NameError::Empty) => {
            errors.name = Some("World name cannot be empty".into());
            return false;
        }
        Err(NameError::TooLong) => {
            errors.name = Some("World name cannot exceed 32 characters".into());
            return false;
        }
        Err(NameError::Illegal(c)) => {
            errors.name = Some(format!("Illegal character in world name: '{c}'"));
            return false;
        }
    };

    let seed = if drafts.seed.trim().is_empty() {
        random_seed(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            std::process::id(),
        )
    } else {
        match drafts.seed.trim().parse::<u32>() {
            Ok(s) => s,
            Err(_) => {
                errors.seed = Some("Seed must be a valid 32-bit unsigned number".into());
                return false;
            }
        }
    };

    match create_world(save_root, valid_name, seed, drafts.size) {
        Ok(info) => {
            errors.name = None;
            errors.seed = None;
            selected_world.name = info.name;
            selected_world.seed = info.seed;
            selected_world.size = info.size;
            selected_world.persist = true;
            next_state.set(AppState::InGame);
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            errors.name = Some(format!("World \"{valid_name}\" already exists"));
            false
        }
        Err(e) => {
            errors.name = Some(format!("Failed to create world: {e}"));
            false
        }
    }
}

#[derive(Component)]
pub struct NameValueText;

#[derive(Component)]
pub struct SeedValueText;

#[derive(Component)]
pub struct NameErrorText;

#[derive(Component)]
pub struct SeedErrorText;

#[derive(Component)]
pub struct SizeValueText;

#[derive(Component)]
pub struct DifficultyDescriptionText;

#[derive(Component)]
pub struct NameInputBox;

#[derive(Component)]
pub struct SeedInputBox;

pub fn setup_create_screen(
    mut commands: Commands,
    mut drafts: ResMut<CreateDrafts>,
    mut errors: ResMut<CreateErrors>,
    mut focus: ResMut<MenuFocus>,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
) {
    *drafts = CreateDrafts::default();
    *errors = CreateErrors::default();
    focus.rank = Some(0);

    let (win_w, win_h) = viewport_size(primary_window.as_ref());
    let center_x = win_w * 0.5;
    let panel_w = 740.0;
    let panel_h = 620.0;
    let panel_top = ((win_h - panel_h) * 0.5).max(24.0);

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
        DespawnOnExit(MenuScreen::CreateWorld),
    ));

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(center_x - panel_w * 0.5),
                top: Val::Px(panel_top),
                width: Val::Px(panel_w),
                height: Val::Px(panel_h),
                border: UiRect::all(Val::Px(BORDER_W)),
                border_radius: BorderRadius::all(Val::Px(RADIUS_PANEL)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(22.0)),
                ..default()
            },
            BackgroundColor(COLOR_PANEL_BG),
            BorderColor::all(COLOR_PANEL_BORDER),
            DespawnOnExit(MenuScreen::CreateWorld),
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
                MenuText::new("Create World", 1.45),
                MenuTextColor(COLOR_TEXT_GOLD),
            ));

            p.spawn((
                Node {
                    width: Val::Px(panel_w - 56.0),
                    height: Val::Px(2.0),
                    margin: UiRect::bottom(Val::Px(14.0)),
                    ..default()
                },
                BackgroundColor(COLOR_HEADER_DIVIDER),
            ));

            // World Name input box
            p.spawn((
                Node {
                    width: Val::Px(640.0),
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                },
                MenuText::new("World Name  [Tab / Up / Down to switch field]", 0.9),
            ));
            p.spawn((
                Node {
                    width: Val::Px(640.0),
                    height: Val::Px(38.0),
                    border: UiRect::all(Val::Px(BORDER_W)),
                    border_radius: BorderRadius::all(Val::Px(RADIUS_BTN)),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(12.0)),
                    margin: UiRect::bottom(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(COLOR_INPUT_BG),
                BorderColor::all(COLOR_INPUT_ACTIVE_BORDER),
                NameInputBox,
            ))
            .with_children(|box_p| {
                box_p.spawn((Node::default(), MenuText::new("> ", 1.05), NameValueText));
            });
            p.spawn((
                Node {
                    width: Val::Px(640.0),
                    height: Val::Px(18.0),
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
                MenuText::new("", 0.8),
                NameErrorText,
            ));

            // Seed input box (left 460px; Random Seed button sits beside it at right)
            p.spawn((
                Node {
                    width: Val::Px(640.0),
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                },
                MenuText::new("Seed  (decimal, or blank for random)", 0.9),
            ));
            p.spawn(Node {
                width: Val::Px(640.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(2.0)),
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    Node {
                        width: Val::Px(460.0),
                        height: Val::Px(38.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(5.0)),
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(COLOR_INPUT_BG),
                    BorderColor::all(COLOR_INPUT_IDLE_BORDER),
                    SeedInputBox,
                ))
                .with_children(|box_p| {
                    box_p.spawn((Node::default(), MenuText::new("> ", 1.05), SeedValueText));
                });
            });
            p.spawn((
                Node {
                    width: Val::Px(640.0),
                    height: Val::Px(18.0),
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                },
                MenuText::new("", 0.8),
                SeedErrorText,
            ));

            // Summary badge bar
            p.spawn((
                Node {
                    width: Val::Px(640.0),
                    height: Val::Px(34.0),
                    border: UiRect::all(Val::Px(BORDER_W)),
                    border_radius: BorderRadius::all(Val::Px(RADIUS_BTN)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::horizontal(Val::Px(14.0)),
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(COLOR_PANEL_INNER),
                BorderColor::all(COLOR_HEADER_DIVIDER),
            ))
            .with_children(|bar| {
                bar.spawn((
                    Node::default(),
                    MenuText::new("World size: Small", 0.9),
                    SizeValueText,
                ));
                bar.spawn((
                    Node::default(),
                    MenuText::new("Evil: Corruption  |  Classic", 0.85),
                ));
            });
        });

    // Random Seed button beside the Seed input box
    let b1 = spawn_button_sized(
        &mut commands,
        "Random Seed",
        Action::RandomSeed,
        1,
        (center_x + 152.0, panel_top + 190.0),
        (168.0, 38.0),
        false,
    );
    commands
        .entity(b1)
        .insert(DespawnOnExit(MenuScreen::CreateWorld));

    // World size selector cards (Small / Medium / Large)
    let size_btn_w = 200.0;
    let size_gap = 18.0;
    let total_size_w = size_btn_w * 3.0 + size_gap * 2.0;
    let size_start_x = center_x - total_size_w * 0.5;
    for (idx, (label, sz)) in [
        ("Small (4200x1200)", WorldSize::Small),
        ("Medium (6400x1800)", WorldSize::Medium),
        ("Large (8400x2400)", WorldSize::Large),
    ]
    .into_iter()
    .enumerate()
    {
        let b_sz = spawn_button_sized(
            &mut commands,
            label,
            Action::SelectWorldSize(sz),
            (3 + idx) as u16,
            (
                size_start_x + (idx as f32) * (size_btn_w + size_gap),
                panel_top + 322.0,
            ),
            (size_btn_w, BUTTON_HEIGHT),
            false,
        );
        commands.entity(b_sz).insert((
            DespawnOnExit(MenuScreen::CreateWorld),
            MenuSelected(drafts.size == sz),
        ));
    }

    // Difficulty buttons: Classic (active), Expert (disabled), Master (disabled)
    let diff_btn_w = 200.0;
    let diff_gap = 18.0;
    let total_diff_w = diff_btn_w * 3.0 + diff_gap * 2.0;
    let diff_start_x = center_x - total_diff_w * 0.5;
    for (idx, (label, disabled)) in [
        ("Classic", false),
        ("Expert (Later)", true),
        ("Master (Later)", true),
    ]
    .into_iter()
    .enumerate()
    {
        let b_diff = spawn_button_sized(
            &mut commands,
            label,
            Action::Noop,
            (6 + idx) as u16,
            (
                diff_start_x + (idx as f32) * (diff_btn_w + diff_gap),
                panel_top + 382.0,
            ),
            (diff_btn_w, BUTTON_HEIGHT),
            disabled,
        );
        commands.entity(b_diff).insert((
            DespawnOnExit(MenuScreen::CreateWorld),
            MenuSelected(!disabled),
        ));
    }

    // Description of the selected difficulty
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(panel_top + 440.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            DespawnOnExit(MenuScreen::CreateWorld),
        ))
        .with_children(|p| {
            p.spawn((
                Node::default(),
                MenuText::new(
                    "Classic: Standard Terraria experience. Drop half your coins on death.",
                    0.88,
                ),
                DifficultyDescriptionText,
            ));
        });

    // Footer buttons cleanly separated
    let foot_w = 220.0;
    let foot_y = panel_top + panel_h - 68.0;
    let b0 = spawn_button_sized(
        &mut commands,
        "Create & Play",
        Action::CreateWorld,
        0,
        (center_x - foot_w - 18.0, foot_y),
        (foot_w, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b0)
        .insert(DespawnOnExit(MenuScreen::CreateWorld));

    let b2 = spawn_button_sized(
        &mut commands,
        "Back",
        Action::Back,
        2,
        (center_x + 18.0, foot_y),
        (foot_w, BUTTON_HEIGHT),
        false,
    );
    commands
        .entity(b2)
        .insert(DespawnOnExit(MenuScreen::CreateWorld));
}

/// Reads typing and field selection for CreateWorld screen.
pub fn handle_create_input(
    current_screen: Option<Res<State<MenuScreen>>>,
    keyboard_events: Option<MessageReader<KeyboardInput>>,
    mut drafts: ResMut<CreateDrafts>,
    mut errors: ResMut<CreateErrors>,
    mut action_writer: Option<MessageWriter<Action>>,
) {
    if current_screen.as_ref().map(|s| s.get()) != Some(&MenuScreen::CreateWorld) {
        return;
    }

    let Some(mut keyboard_events) = keyboard_events else {
        return;
    };

    for ev in keyboard_events.read() {
        if !ev.state.is_pressed() {
            continue;
        }

        if ev.key_code == KeyCode::Backspace || ev.logical_key == Key::Backspace {
            apply_backspace(&mut drafts, &mut errors);
            continue;
        }

        if ev.key_code == KeyCode::Tab
            || ev.logical_key == Key::Tab
            || ev.key_code == KeyCode::ArrowUp
            || ev.key_code == KeyCode::ArrowDown
        {
            drafts.field = match drafts.field {
                Field::Name => Field::Seed,
                Field::Seed => Field::Name,
            };
            continue;
        }

        if ev.key_code == KeyCode::Enter
            || ev.key_code == KeyCode::NumpadEnter
            || ev.logical_key == Key::Enter
        {
            if let Some(ref mut writer) = action_writer {
                writer.write(Action::CreateWorld);
            }
            continue;
        }

        if ev.logical_key == Key::Space || ev.key_code == KeyCode::Space {
            apply_char(&mut drafts, &mut errors, ' ');
            continue;
        }

        if let Key::Character(ref s) = ev.logical_key {
            for ch in s.chars() {
                apply_char(&mut drafts, &mut errors, ch);
            }
        }
    }
}

/// Handles CreateWorld, RandomSeed, and world size selection actions.
pub fn handle_create_actions(
    current_screen: Option<Res<State<MenuScreen>>>,
    actions: Option<MessageReader<Action>>,
    save_root: Option<Res<SaveRoot>>,
    drafts: Option<ResMut<CreateDrafts>>,
    errors: Option<ResMut<CreateErrors>>,
    selected_world: Option<ResMut<SelectedWorld>>,
    next_state: Option<ResMut<NextState<AppState>>>,
) {
    if current_screen.as_ref().map(|s| s.get()) != Some(&MenuScreen::CreateWorld) {
        return;
    }

    let Some(mut actions) = actions else {
        return;
    };

    let (Some(mut drafts), Some(mut errors)) = (drafts, errors) else {
        return;
    };

    let save_root = save_root;
    let mut selected_world = selected_world;
    let mut next_state = next_state;

    for action in actions.read() {
        match action {
            Action::CycleWorldSize => {
                drafts.size = drafts.size.next();
            }
            Action::SelectWorldSize(sz) => {
                drafts.size = *sz;
            }
            Action::RandomSeed => {
                let s = random_seed(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos(),
                    std::process::id(),
                );
                drafts.seed = s.to_string();
                errors.seed = None;
            }
            Action::CreateWorld => {
                let (Some(save_root), Some(selected_world), Some(next_state)) = (
                    save_root.as_ref(),
                    selected_world.as_mut(),
                    next_state.as_mut(),
                ) else {
                    continue;
                };
                execute_create_world(
                    &save_root.0,
                    &drafts,
                    &mut errors,
                    selected_world,
                    next_state,
                );
            }
            _ => {}
        }
    }
}

/// Synchronizes UI text elements when drafts or errors change.
pub fn sync_create_ui(
    drafts: Res<CreateDrafts>,
    errors: Res<CreateErrors>,
    mut name_val: Query<
        &mut MenuText,
        (
            With<NameValueText>,
            Without<SeedValueText>,
            Without<NameErrorText>,
            Without<SeedErrorText>,
            Without<SizeValueText>,
        ),
    >,
    mut seed_val: Query<
        &mut MenuText,
        (
            With<SeedValueText>,
            Without<NameValueText>,
            Without<NameErrorText>,
            Without<SeedErrorText>,
            Without<SizeValueText>,
        ),
    >,
    mut name_err: Query<
        &mut MenuText,
        (
            With<NameErrorText>,
            Without<NameValueText>,
            Without<SeedValueText>,
            Without<SeedErrorText>,
            Without<SizeValueText>,
        ),
    >,
    mut seed_err: Query<
        &mut MenuText,
        (
            With<SeedErrorText>,
            Without<NameValueText>,
            Without<SeedValueText>,
            Without<NameErrorText>,
            Without<SizeValueText>,
        ),
    >,
    mut size_val: Query<
        &mut MenuText,
        (
            With<SizeValueText>,
            Without<NameValueText>,
            Without<SeedValueText>,
            Without<NameErrorText>,
            Without<SeedErrorText>,
        ),
    >,
    mut name_box: Query<&mut BorderColor, (With<NameInputBox>, Without<SeedInputBox>)>,
    mut seed_box: Query<&mut BorderColor, (With<SeedInputBox>, Without<NameInputBox>)>,
    mut size_btns: Query<(&super::MenuButton, &mut MenuSelected)>,
) {
    if !drafts.is_changed() && !errors.is_changed() {
        return;
    }

    let cursor = |f: Field| if drafts.field == f { "_" } else { "" };

    for mut t in &mut name_val {
        let display = if drafts.name.is_empty() {
            format!("[Type world name]{}", cursor(Field::Name))
        } else {
            format!("{}{}", drafts.name, cursor(Field::Name))
        };
        if t.text != display {
            t.text = display;
        }
    }

    for mut t in &mut seed_val {
        let display = if drafts.seed.is_empty() {
            format!("[Random seed]{}", cursor(Field::Seed))
        } else {
            format!("{}{}", drafts.seed, cursor(Field::Seed))
        };
        if t.text != display {
            t.text = display;
        }
    }

    for mut t in &mut name_err {
        let err = errors.name.as_deref().unwrap_or("");
        if t.text != err {
            t.text = err.to_string();
        }
    }

    for mut t in &mut seed_err {
        let err = errors.seed.as_deref().unwrap_or("");
        if t.text != err {
            t.text = err.to_string();
        }
    }

    for mut t in &mut size_val {
        let display = format!("World size: {}", drafts.size.label());
        if t.text != display {
            t.text = display;
        }
    }

    for mut border in &mut name_box {
        *border = BorderColor::all(if drafts.field == Field::Name {
            COLOR_INPUT_ACTIVE_BORDER
        } else {
            COLOR_INPUT_IDLE_BORDER
        });
    }
    for mut border in &mut seed_box {
        *border = BorderColor::all(if drafts.field == Field::Seed {
            COLOR_INPUT_ACTIVE_BORDER
        } else {
            COLOR_INPUT_IDLE_BORDER
        });
    }
    for (btn, mut sel) in &mut size_btns {
        if let Action::SelectWorldSize(sz) = btn.action {
            sel.0 = drafts.size == sz;
        }
    }
}

pub struct CreatePlugin;

impl Plugin for CreatePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<KeyboardInput>()
            .add_message::<Action>()
            .init_resource::<CreateDrafts>()
            .init_resource::<CreateErrors>()
            .add_systems(OnEnter(MenuScreen::CreateWorld), setup_create_screen)
            .add_systems(
                Update,
                (handle_create_input, handle_create_actions, sync_create_ui)
                    .run_if(in_state(MenuScreen::CreateWorld)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn tmp() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "tv_create_test_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn random_seed_uses_nanos_and_pid() {
        assert_ne!(random_seed(1, 1), 0, "seed must not be zero");
        assert_eq!(
            random_seed(42, 7),
            random_seed(42, 7),
            "seed generation must be deterministic"
        );
        assert_ne!(
            random_seed(42, 7),
            random_seed(43, 7),
            "different nanos must produce different seeds"
        );
        assert_ne!(
            random_seed(42, 7),
            random_seed(42, 8),
            "different pids must produce different seeds"
        );
    }

    #[test]
    fn typing_filters_to_printable_ascii() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .add_plugins(CreatePlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::CreateWorld);
        app.update();

        let send_key = |app: &mut App, key: KeyboardInput| {
            app.world_mut()
                .resource_mut::<Messages<KeyboardInput>>()
                .write(key);
            app.update();
        };

        let char_input = |c: char| KeyboardInput {
            logical_key: Key::Character(c.to_string().into()),
            key_code: KeyCode::KeyA,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };

        let backspace_input = || KeyboardInput {
            logical_key: Key::Backspace,
            key_code: KeyCode::Backspace,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };

        // Feed non-ASCII 'é' -> draft unchanged, inline error set
        send_key(&mut app, char_input('é'));
        assert_eq!(app.world().resource::<CreateDrafts>().name, "");
        assert!(
            app.world().resource::<CreateErrors>().name.is_some(),
            "non-ASCII must produce inline error"
        );

        // Feed 'a' -> "a", error cleared
        send_key(&mut app, char_input('a'));
        assert_eq!(app.world().resource::<CreateDrafts>().name, "a");
        assert!(
            app.world().resource::<CreateErrors>().name.is_none(),
            "valid ASCII clears error"
        );

        // Backspace pops
        send_key(&mut app, backspace_input());
        assert_eq!(app.world().resource::<CreateDrafts>().name, "");
    }

    #[test]
    fn create_flow_rejects_case_duplicate_message() {
        let temp = tmp();
        terravibe_server::worlds::create_world(&temp, "My World", 42, WorldSize::Small).unwrap();

        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .add_message::<Action>()
            .insert_resource(SaveRoot(temp.clone()))
            .insert_resource(SelectedWorld {
                name: String::new(),
                seed: 0,
                size: WorldSize::Small,
                persist: false,
            })
            .add_plugins(CreatePlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::CreateWorld);
        app.update();

        // Type "MY WORLD" into draft
        app.world_mut().resource_mut::<CreateDrafts>().name = "MY WORLD".into();

        // Activate CreateWorld
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::CreateWorld);
        app.update();

        // Must show inline duplicate error
        let errors = app.world().resource::<CreateErrors>();
        assert!(
            errors
                .name
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains("already exists"),
            "expected duplicate error message, got: {:?}",
            errors.name
        );

        // App state must remain MenuScreen::CreateWorld, NOT InGame
        let screen = app.world().resource::<State<MenuScreen>>();
        assert_eq!(*screen.get(), MenuScreen::CreateWorld);

        // No second directory created on disk
        let entries: Vec<_> = std::fs::read_dir(&temp)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["My World"]);

        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn create_flow_rejects_illegal_and_nonascii() {
        let temp = tmp();
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .add_message::<Action>()
            .insert_resource(SaveRoot(temp.clone()))
            .insert_resource(SelectedWorld {
                name: String::new(),
                seed: 0,
                size: WorldSize::Small,
                persist: false,
            })
            .add_plugins(CreatePlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::CreateWorld);
        app.update();

        // "a/b"
        app.world_mut().resource_mut::<CreateDrafts>().name = "a/b".into();
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::CreateWorld);
        app.update();
        let errors = app.world().resource::<CreateErrors>();
        assert!(errors.name.is_some(), "expected error for 'a/b'");

        // "café"
        app.world_mut().resource_mut::<CreateDrafts>().name = "café".into();
        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::CreateWorld);
        app.update();
        let errors = app.world().resource::<CreateErrors>();
        assert!(errors.name.is_some(), "expected error for 'café'");

        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn arrow_keys_switch_field_without_moving_focus_on_create_world() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_plugins((CreatePlugin, crate::menu::nav::NavPlugin));

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::CreateWorld);
        app.update();
        app.update();

        assert_eq!(app.world().resource::<MenuFocus>().rank, Some(0));
        assert_eq!(app.world().resource::<CreateDrafts>().field, Field::Name);

        // Press ArrowDown via ButtonInput and KeyboardInput
        let arrow_down = KeyboardInput {
            logical_key: Key::ArrowDown,
            key_code: KeyCode::ArrowDown,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(arrow_down);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowDown);
        app.update();

        // Field switched to Seed, but MenuFocus rank remained unchanged at Some(0)
        assert_eq!(app.world().resource::<CreateDrafts>().field, Field::Seed);
        assert_eq!(
            app.world().resource::<MenuFocus>().rank,
            Some(0),
            "MenuFocus must not change via ArrowDown on CreateWorld"
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ArrowDown);
        app.update();

        // Press ArrowUp
        let arrow_up = KeyboardInput {
            logical_key: Key::ArrowUp,
            key_code: KeyCode::ArrowUp,
            state: bevy::input::ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
            repeat: false,
            text: None,
        };
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(arrow_up);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        app.update();

        // Field switched back to Name, MenuFocus rank still Some(0)
        assert_eq!(app.world().resource::<CreateDrafts>().field, Field::Name);
        assert_eq!(
            app.world().resource::<MenuFocus>().rank,
            Some(0),
            "MenuFocus must not change via ArrowUp on CreateWorld"
        );
    }

    #[test]
    fn seed_field_nonascii_error_routing() {
        let mut drafts = CreateDrafts {
            name: String::new(),
            seed: String::new(),
            field: Field::Seed,
            size: WorldSize::Small,
        };
        let mut errors = CreateErrors::default();

        apply_char(&mut drafts, &mut errors, 'é');

        assert!(
            errors.seed.is_some(),
            "non-ASCII on seed field must route to errors.seed"
        );
        assert!(
            errors.name.is_none(),
            "non-ASCII on seed field must leave errors.name empty"
        );
        assert_eq!(drafts.seed, "");
    }

    fn test_create_world_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .add_plugins(CreatePlugin);
        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::CreateWorld);
        app
    }

    fn collect_menu_texts(app: &mut App) -> Vec<String> {
        app.world_mut()
            .query::<&MenuText>()
            .iter(app.world())
            .map(|t| t.text.clone())
            .collect()
    }

    #[test]
    fn world_creation_shows_difficulty_description() {
        let mut app = test_create_world_app();
        app.update();
        let texts = collect_menu_texts(&mut app);
        assert!(
            texts
                .iter()
                .any(|t| t
                    == "Classic: Standard Terraria experience. Drop half your coins on death."),
            "Classic difficulty description missing; texts: {texts:?}"
        );
    }

    #[test]
    fn create_screen_offers_small_medium_large() {
        use crate::menu::MenuButton;
        use terravibe_sim::WorldSize;
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .add_sub_state::<MenuScreen>()
            .init_resource::<MenuFocus>()
            .add_plugins(CreatePlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::CreateWorld);
        app.update();

        let actions: Vec<Action> = app
            .world_mut()
            .query::<&MenuButton>()
            .iter(app.world())
            .map(|b| b.action.clone())
            .collect();
        for sz in [WorldSize::Small, WorldSize::Medium, WorldSize::Large] {
            assert!(
                actions.contains(&Action::SelectWorldSize(sz)),
                "Create screen missing button for {sz:?}"
            );
        }

        assert_eq!(
            app.world().resource::<CreateDrafts>().size,
            WorldSize::Small
        );

        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::CycleWorldSize);
        app.update();
        assert_eq!(
            app.world().resource::<CreateDrafts>().size,
            WorldSize::Medium
        );

        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::CycleWorldSize);
        app.update();
        assert_eq!(
            app.world().resource::<CreateDrafts>().size,
            WorldSize::Large
        );

        app.world_mut()
            .resource_mut::<Messages<Action>>()
            .write(Action::CycleWorldSize);
        app.update();
        assert_eq!(
            app.world().resource::<CreateDrafts>().size,
            WorldSize::Small
        );
    }
}
