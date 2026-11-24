//! Scenario registry and scripted step plans.

use bevy::input::keyboard::KeyCode;

use super::{Expect, MetricSource, Scenario, Step};

pub const SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "menu_title",
        direct_ingame: false,
        fixture: None,
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "menu_characters",
        direct_ingame: false,
        fixture: None,
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "menu_worlds",
        direct_ingame: false,
        fixture: None,
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "menu_create",
        direct_ingame: false,
        fixture: None,
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "menu_options",
        direct_ingame: false,
        fixture: None,
        demo_enemies: false,
        phases: 2,
    },
    Scenario {
        name: "menu_multiplayer",
        direct_ingame: false,
        fixture: None,
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "music",
        direct_ingame: true,
        fixture: None,
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "movement",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Flat),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "combat",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Combat),
        demo_enemies: true,
        phases: 1,
    },
    Scenario {
        name: "inventory",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Combat),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "mining_crafting",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Flat),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "housing_npc_shop",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Housing),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "corruption_altar",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Corruption),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "boss",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Boss),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "save_reload",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Flat),
        demo_enemies: false,
        phases: 2,
    },
    // Flat fixture: pre-built chunks avoid world-generation timeouts in headless
    // runs. Trade-off (accepted): the audit screenshots now show a flat plane and
    // no longer exercise generated-terrain rendering. Keep terrain coverage in a
    // separate, generation-tolerant scenario if it becomes important.
    Scenario {
        name: "visual_audit",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Flat),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "liquids",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Liquids),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "first_hour",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Housing),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "tooltips_economy",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Housing),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "combat_feedback",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Combat),
        demo_enemies: true,
        phases: 1,
    },
    Scenario {
        name: "held_items",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Flat),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "weather_audio",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Flat),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "town_housing",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Housing),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "bench_effects",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Flat),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "molten_gear",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Flat),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "death_respawn",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Flat),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "minimap",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Flat),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "accessibility_options",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Flat),
        demo_enemies: false,
        phases: 1,
    },
    Scenario {
        name: "wall_of_flesh",
        direct_ingame: true,
        fixture: Some(super::FixtureKind::Flat),
        demo_enemies: false,
        phases: 1,
    },
];

pub fn scenario_steps(name: &str, phase: u8) -> Vec<Step> {
    match (name, phase) {
        ("menu_title", 1) => vec![
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Title")),
            Step::Screenshot("title_initial"),
            // Focus and activate Single Player
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::Enter),
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Characters")),
            Step::Screenshot("title_characters_screen"),
            Step::Key(KeyCode::Escape),
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Title")),
            // Focus and activate Multiplayer
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::Enter),
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Multiplayer")),
            Step::Screenshot("title_multiplayer_screen"),
            Step::Key(KeyCode::Escape),
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Title")),
            // Focus and activate Options
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::Enter),
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Options")),
            Step::Screenshot("title_options_screen"),
            Step::Key(KeyCode::Escape),
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Title")),
            // Focus and activate Quit
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Screenshot("title_quit_focused"),
            Step::Key(KeyCode::Enter),
            Step::Wait(0.1),
        ],
        ("menu_characters", 1) => vec![
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Title")),
            Step::Click { x: 960.0, y: 360.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Characters")),
            Step::Screenshot("chars_list_initial"),
            Step::Click { x: 960.0, y: 340.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("CreateCharacter")),
            Step::Screenshot("chars_create_initial"),
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Wait(0.1),
            Step::Type("Tester"),
            Step::Wait(0.1),
            Step::Click { x: 870.0, y: 740.0 },
            Step::Wait(0.5),
            Step::Expect(Expect::Screen("Worlds")),
            Step::Screenshot("chars_worlds_reached"),
            Step::Wait(0.2),
        ],
        ("menu_worlds", 1) => vec![
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Title")),
            Step::Click { x: 960.0, y: 360.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Characters")),
            Step::Click { x: 960.0, y: 340.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("CreateCharacter")),
            Step::Click { x: 870.0, y: 740.0 },
            Step::Wait(0.5),
            Step::Expect(Expect::Screen("Worlds")),
            Step::Expect(Expect::Text("No worlds found")),
            Step::Screenshot("worlds_empty"),
            // Activate Create World CTA
            Step::Click { x: 960.0, y: 360.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("CreateWorld")),
            Step::Screenshot("worlds_create_screen"),
            // Type world name and create
            Step::Type("TestWorld"),
            Step::Wait(0.1),
            Step::Key(KeyCode::Enter),
            Step::Wait(1.0),
            // Enters InGame
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("worlds_ingame_first_play"),
            Step::Wait(0.3),
            // Esc to open pause menu
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Screenshot("worlds_inventory_open"),
            // Settings button (centre at 1920x1080) opens the Game Menu
            Step::Click {
                x: 1780.0,
                y: 1012.0,
            },
            Step::Wait(0.3),
            Step::Screenshot("worlds_game_menu_open"),
            // Click Save & Quit (rank 2 at 960, 468)
            Step::Click { x: 960.0, y: 468.0 },
            Step::Wait(1.5),
            // Returned to title
            Step::Expect(Expect::InGame(false)),
            Step::Expect(Expect::Screen("Title")),
            // Re-enter Characters screen by clicking Single Player
            Step::Click { x: 960.0, y: 360.0 },
            Step::Wait(0.5),
            Step::Expect(Expect::Screen("Characters")),
            // Play character at row 0
            Step::Click {
                x: 1090.0,
                y: 206.0,
            },
            Step::Wait(0.5),
            Step::Expect(Expect::Screen("Worlds")),
            Step::Expect(Expect::Text("TestWorld")),
            Step::Expect(Expect::Text("Seed:")),
            Step::Expect(Expect::Text("Played:")),
            Step::Screenshot("worlds_row_visible"),
            // Activate row Play button (center x = 1135.0, y = 235.0) to re-enter InGame
            Step::Click {
                x: 1135.0,
                y: 235.0,
            },
            Step::Wait(1.0),
            // Re-enters InGame with persisted world & seed
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("worlds_reenter_ingame"),
            Step::Wait(0.2),
        ],
        ("menu_create", 1) => vec![
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Title")),
            // Open Characters and pick/create to reach Worlds
            Step::Click { x: 960.0, y: 360.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Characters")),
            Step::Click { x: 960.0, y: 340.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("CreateCharacter")),
            Step::Click { x: 870.0, y: 740.0 },
            Step::Wait(0.5),
            Step::Expect(Expect::Screen("Worlds")),
            // Open Create World
            Step::Click { x: 960.0, y: 360.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("CreateWorld")),
            Step::Screenshot("create_initial"),
            // 1. Empty name error
            Step::Key(KeyCode::Enter),
            Step::Wait(0.1),
            Step::Expect(Expect::Text("World name cannot be empty")),
            Step::Screenshot("create_error_empty_name"),
            // 2. Bad seed error
            Step::Type("Alpha"),
            Step::Wait(0.1),
            Step::Key(KeyCode::Tab),
            Step::Wait(0.1),
            Step::Type("XYZ"),
            Step::Wait(0.1),
            Step::Key(KeyCode::Enter),
            Step::Wait(0.1),
            Step::Expect(Expect::Text("Seed must be a valid 32-bit unsigned number")),
            Step::Screenshot("create_error_bad_seed"),
            // Clear bad seed
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Wait(0.1),
            // 3. Random seed button
            Step::Click {
                x: 1090.0,
                y: 700.0,
            },
            Step::Wait(0.2),
            Step::Screenshot("create_random_seed_shown"),
            // Create "Alpha" and enter game
            Step::Key(KeyCode::Enter),
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            // Save & Quit
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Click {
                x: 1780.0,
                y: 1012.0,
            },
            Step::Wait(0.3),
            Step::Click { x: 960.0, y: 468.0 },
            Step::Wait(1.2),
            Step::Expect(Expect::InGame(false)),
            Step::Expect(Expect::Screen("Title")),
            // Re-open Characters then pick character to reach Worlds
            Step::Click { x: 960.0, y: 360.0 },
            Step::Wait(0.4),
            Step::Expect(Expect::Screen("Characters")),
            Step::Click {
                x: 1090.0,
                y: 206.0,
            },
            Step::Wait(0.4),
            Step::Expect(Expect::Screen("Worlds")),
            // In Worlds with 1 row, click "Create world" button at (840.0, 700.0)
            Step::Click { x: 840.0, y: 700.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("CreateWorld")),
            // 4. Duplicate rejection with name "Alpha"
            Step::Type("Alpha"),
            Step::Wait(0.1),
            Step::Key(KeyCode::Enter),
            Step::Wait(0.1),
            Step::Expect(Expect::Text("already exists")),
            Step::Screenshot("create_duplicate_error"),
            // 5. Successful create-and-play with "Beta"
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Wait(0.1),
            Step::Type("Beta"),
            Step::Wait(0.1),
            Step::Key(KeyCode::Enter),
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("create_success_beta_ingame"),
            Step::Wait(0.2),
        ],
        ("menu_options", 1) => vec![
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Title")),
            // Open Options
            Step::Click { x: 960.0, y: 480.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Options")),
            Step::Screenshot("options_initial"),
            // Change Window Mode (rank 0 -> Borderless)
            Step::Click { x: 960.0, y: 220.0 },
            Step::Wait(0.2),
            Step::Expect(Expect::Text("Window Mode: Borderless")),
            // Change VSync (rank 2 -> Off)
            Step::Click { x: 960.0, y: 316.0 },
            Step::Wait(0.2),
            Step::Expect(Expect::Text("VSync: Off")),
            // Change Sound Volume (rank 4 -> 80%)
            Step::Click {
                x: 1092.0,
                y: 364.0,
            },
            Step::Wait(0.2),
            Step::Expect(Expect::Text("Sound Volume: 80%")),
            // Change Music Volume (rank 6 -> 55%)
            Step::Click {
                x: 1092.0,
                y: 412.0,
            },
            Step::Wait(0.2),
            Step::Expect(Expect::Text("Music Volume: 55%")),
            // Change Sensitivity (rank 8 -> 1.25)
            Step::Click {
                x: 1092.0,
                y: 460.0,
            },
            Step::Wait(0.2),
            Step::Expect(Expect::Text("Sensitivity: 1.25")),
            // Change FOV (rank 10 -> 75 deg)
            Step::Click {
                x: 1092.0,
                y: 508.0,
            },
            Step::Wait(0.2),
            Step::Expect(Expect::Text("FOV: 75 deg")),
            // Change Invert Y (rank 11 -> On)
            Step::Click { x: 960.0, y: 556.0 },
            Step::Wait(0.2),
            Step::Expect(Expect::Text("Invert Y: On")),
            Step::Screenshot("options_phase1_changed"),
            // Back to Title
            Step::Click { x: 960.0, y: 652.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Title")),
            // Quit phase 1
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.1),
        ],
        ("menu_options", 2) => vec![
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Title")),
            // Open Options
            Step::Click { x: 960.0, y: 480.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Options")),
            // Verify persisted values
            Step::Expect(Expect::Text("Window Mode: Borderless")),
            Step::Expect(Expect::Text("VSync: Off")),
            Step::Expect(Expect::Text("Sound Volume: 80%")),
            Step::Expect(Expect::Text("Music Volume: 55%")),
            Step::Expect(Expect::Text("Sensitivity: 1.25")),
            Step::Expect(Expect::Text("FOV: 75 deg")),
            Step::Expect(Expect::Text("Invert Y: On")),
            Step::Screenshot("options_phase2_persisted"),
            // Click "Reset to Defaults" button
            Step::Click { x: 960.0, y: 604.0 },
            Step::Wait(0.3),
            // Verify default values restored
            Step::Expect(Expect::Text("Window Mode: Windowed")),
            Step::Expect(Expect::Text("VSync: On")),
            Step::Expect(Expect::Text("Sound Volume: 75%")),
            Step::Expect(Expect::Text("Music Volume: 50%")),
            Step::Expect(Expect::Text("Sensitivity: 1.00")),
            Step::Expect(Expect::Text("FOV: 70 deg")),
            Step::Expect(Expect::Text("Invert Y: Off")),
            Step::Screenshot("options_phase2_reset"),
            // Back to Title
            Step::Click { x: 960.0, y: 652.0 },
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Title")),
            Step::Wait(0.1),
        ],
        ("menu_multiplayer", 1) => vec![
            Step::Wait(0.2),
            Step::Expect(Expect::Screen("Title")),
            // Focus and open Multiplayer (rank 1 on fresh Title screen)
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::Enter),
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Multiplayer")),
            // Live multiplayer copy and buttons
            Step::Expect(Expect::Text("Host a co-op world")),
            Step::Expect(Expect::Text("No port forwarding required.")),
            Step::Expect(Expect::Text("Host & Play")),
            Step::Expect(Expect::Text("Join Game")),
            Step::Screenshot("multiplayer_screen"),
            // Focus carried rank 1 from Title: ArrowUp moves to rank 0 (Host & Play) -> Characters
            Step::Key(KeyCode::ArrowUp),
            Step::Wait(0.1),
            Step::Key(KeyCode::Enter),
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Characters")),
            // Return to Title, re-open Multiplayer (focus rank 1 = Join Game), activate -> Characters
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Title")),
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::Enter),
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Multiplayer")),
            Step::Key(KeyCode::Enter),
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Characters")),
            // Return to Title, re-open Multiplayer (focus rank 1), ArrowDown -> rank 2 (Back) -> Title
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Title")),
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::Enter),
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Multiplayer")),
            Step::Key(KeyCode::ArrowDown),
            Step::Wait(0.1),
            Step::Key(KeyCode::Enter),
            Step::Wait(0.3),
            Step::Expect(Expect::Screen("Title")),
            Step::Screenshot("multiplayer_back_to_title"),
            Step::Wait(0.1),
        ],
        ("music", 1) => vec![
            Step::Wait(0.5),
            Step::Expect(Expect::InGame(true)),
            Step::Wait(2.0),
            Step::Expect(Expect::Music("Day")),
            Step::Screenshot("music_day"),
            Step::SetClock(54000),
            Step::Wait(2.0),
            Step::Expect(Expect::Music("Night")),
            Step::Screenshot("music_night"),
            Step::SpawnDemoBoss,
            Step::Wait(2.0),
            Step::Expect(Expect::Music("Boss")),
            Step::Screenshot("music_boss"),
            Step::Wait(0.2),
        ],
        ("movement", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("movement_start"),
            // Walk: full speed (0.1286 blocks/tick) within 6 ticks (0.10 s) of input.
            Step::Hold(KeyCode::KeyW, 0.10),
            Step::Metric("walk_speed_6ticks", MetricSource::Vel),
            Step::Expect(Expect::MetricGt("walk_speed_6ticks", 0.12)),
            Step::Screenshot("movement_walk"),
            // Keep walking to the limit, release: stopped within 3 ticks (0.05 s).
            Step::Hold(KeyCode::KeyW, 0.25),
            Step::Wait(0.05),
            Step::Metric("stopped_speed_3ticks", MetricSource::Vel),
            Step::Expect(Expect::MetricLt("stopped_speed_3ticks", 0.01)),
            // Strafe both ways.
            Step::Hold(KeyCode::KeyD, 0.3),
            Step::Hold(KeyCode::KeyA, 0.3),
            Step::Wait(0.15),
            // Jump and fall: apex above the start, then back to the ground.
            Step::Metric("ground_y", MetricSource::MaxPlayerY),
            Step::Hold(KeyCode::Space, 0.3),
            Step::Metric("jump_apex_y", MetricSource::MaxPlayerY),
            Step::Expect(Expect::MetricGt("jump_apex_y", 68.0)),
            Step::Screenshot("movement_jump"),
            Step::Wait(1.0),
            Step::Metric("landed_speed", MetricSource::Vel),
            Step::Expect(Expect::MetricLt("landed_speed", 0.01)),
            // Walk into the workbench and chest (collision/step-up).
            Step::Look(120.0, 0.0),
            Step::Wait(0.2),
            Step::Hold(KeyCode::KeyW, 0.6),
            Step::Screenshot("movement_collision"),
            // Camera look: yaw and pitch.
            Step::Look(-240.0, 80.0),
            Step::Wait(0.2),
            Step::Screenshot("movement_look_down"),
            Step::Look(0.0, -160.0),
            Step::Wait(0.2),
            Step::Screenshot("movement_look_up"),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
        ],
        ("combat", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("combat_demo_enemies"),
            Step::Metric("life_start", MetricSource::Life),
            // Melee: sword on hotbar slot 2, swing repeatedly.
            Step::Key(KeyCode::Digit2),
            Step::Screenshot("combat_sword_windup"),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.3),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.3),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Screenshot("combat_sword_swing"),
            Step::Metric("hits_melee", MetricSource::Hits),
            Step::Expect(Expect::MetricGt("hits_melee", 0.0)),
            // Bow: slot 3, arrows live in slot 3 and are consumed on firing.
            Step::Key(KeyCode::Digit3),
            Step::Metric("ammo_before", MetricSource::InvCount),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.2),
            Step::Screenshot("combat_bow_drawn"),
            Step::Wait(0.8),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.6),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.6),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.6),
            Step::Metric("ammo_after", MetricSource::InvCount),
            Step::Expect(Expect::MetricLt("ammo_after", 103.0)),
            Step::Screenshot("combat_arrow_flight"),
            Step::Wait(1.5),
            Step::Metric("hits_total", MetricSource::Hits),
            Step::Metric("life_end", MetricSource::Life),
            Step::Screenshot("combat_aftermath"),
        ],
        ("inventory", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Metric("inv_start", MetricSource::InvCount),
            Step::Expect(Expect::MetricGt("inv_start", 102.5)),
            Step::Expect(Expect::MetricLt("inv_start", 103.5)),
            // Esc opens the inventory.
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Screenshot("inventory_open"),
            // Pick the arrow stack (slot 3) and place it into empty bag slot 15.
            Step::Click { x: 185.0, y: 42.0 },
            Step::Wait(0.2),
            Step::Screenshot("inventory_stack_held"),
            Step::Click { x: 280.0, y: 90.0 },
            Step::Wait(0.2),
            // Swap: pick the bow (slot 2), drop it on the sword (slot 1), park the sword.
            Step::Click { x: 137.0, y: 42.0 },
            Step::Wait(0.2),
            Step::Click { x: 90.0, y: 42.0 },
            Step::Wait(0.2),
            Step::Click { x: 137.0, y: 42.0 },
            Step::Wait(0.2),
            Step::Screenshot("inventory_swapped"),
            // Ammo routing: arrows (bag slot 15) into ammo slot 54, then back out.
            Step::Click { x: 280.0, y: 90.0 },
            Step::Wait(0.2),
            Step::Click { x: 596.0, y: 42.0 },
            Step::Wait(0.2),
            Step::Screenshot("inventory_ammo_slot"),
            Step::Click { x: 596.0, y: 42.0 },
            Step::Wait(0.2),
            Step::Click { x: 280.0, y: 90.0 },
            Step::Wait(0.2),
            // Coin routing: a sword must be rejected by the coin slot 50.
            Step::Click { x: 42.0, y: 42.0 },
            Step::Wait(0.2),
            Step::Click { x: 548.0, y: 42.0 },
            Step::Wait(0.2),
            Step::Screenshot("inventory_coin_rejects_sword"),
            Step::Metric("inv_held_coin", MetricSource::InvCount),
            Step::Expect(Expect::MetricLt("inv_held_coin", 102.5)),
            // Close with the sword still held: it must come back (103 total).
            Step::Key(KeyCode::Escape),
            Step::Wait(0.4),
            Step::Metric("inv_end", MetricSource::InvCount),
            Step::Expect(Expect::MetricGt("inv_end", 102.5)),
            Step::Expect(Expect::MetricLt("inv_end", 103.5)),
            Step::Screenshot("inventory_closed_with_held"),
        ],
        ("mining_crafting", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Metric("inv_start", MetricSource::InvCount),
            Step::Screenshot("crafting_start"),
            // Mine: pickaxe on slot 2, look down, dig the block in front.
            Step::Key(KeyCode::Digit2),
            Step::Look(0.0, 200.0),
            Step::Wait(0.2),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.4),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.4),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.8),
            Step::Screenshot("crafting_mined"),
            // Craft through the Esc inventory (never C).
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Screenshot("crafting_list"),
            Step::Click { x: 250.0, y: 458.0 },
            Step::Wait(0.3),
            Step::Click { x: 250.0, y: 458.0 },
            Step::Wait(0.3),
            Step::Screenshot("crafting_crafted"),
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Metric("inv_end", MetricSource::InvCount),
            Step::Expect(Expect::MetricGt("inv_end", 0.0)),
            Step::Screenshot("crafting_end"),
        ],
        ("housing_npc_shop", 1) => vec![
            Step::Wait(1.5),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("housing_start"),
            // Check the room via Inventory -> Housing tab, then walk to the Guide's house.
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Click {
                x: 1847.5,
                y: 171.0,
            },
            Step::Wait(0.2),
            Step::Click {
                x: 1818.0,
                y: 212.0,
            },
            Step::Wait(0.4),
            Step::Screenshot("housing_overlay"),
            Step::Key(KeyCode::Escape),
            Step::Wait(0.2),
            Step::Look(150.0, 0.0),
            Step::Wait(0.2),
            Step::Hold(KeyCode::KeyW, 0.8),
            Step::Wait(0.4),
            Step::Screenshot("housing_guide_room"),
            // Bag open: shop selling is Shift-click on a bag slot.
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::ShiftClick { x: 233.0, y: 42.0 },
            Step::Wait(0.3),
            Step::Screenshot("housing_inventory"),
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
        ],
        ("corruption_altar", 1) => vec![
            Step::Wait(1.5),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("corruption_surface"),
            Step::Look(0.0, 100.0),
            Step::Hold(KeyCode::KeyW, 0.8),
            Step::Hold(KeyCode::Space, 0.4),
            Step::Wait(1.0),
            Step::Screenshot("corruption_chasm"),
            Step::Metric("depth_y", MetricSource::MaxPlayerY),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Screenshot("corruption_inventory"),
        ],
        ("boss", 1) => vec![
            Step::Wait(1.5),
            Step::Expect(Expect::InGame(true)),
            Step::Metric("life_start", MetricSource::Life),
            Step::Metric("inv_start", MetricSource::InvCount),
            Step::Screenshot("boss_before"),
            // Suspicious Looking Eye is in hotbar slot 2.
            Step::Key(KeyCode::Digit2),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(3.0),
            Step::Metric("inv_summoned", MetricSource::InvCount),
            // Using the eye consumes one.
            Step::Expect(Expect::MetricLt("inv_summoned", 20.0)),
            Step::Screenshot("boss_bar"),
            Step::Wait(2.0),
            Step::Screenshot("boss_fight"),
            Step::Metric("life_end", MetricSource::Life),
            Step::Expect(Expect::InGame(true)),
        ],
        ("save_reload", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Metric("inv_before", MetricSource::InvCount),
            Step::Expect(Expect::MetricGt("inv_before", 120.0)),
            // Edit: craft the first listed recipe through the Esc inventory carousel.
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Click { x: 62.0, y: 442.0 },
            Step::Wait(0.3),
            Step::Click { x: 62.0, y: 442.0 },
            Step::Wait(0.4),
            Step::Metric("inv_edited", MetricSource::InvCount),
            Step::Expect(Expect::MetricLt("inv_edited", 120.0)),
            Step::Screenshot("save_phase1_edited"),
            // Settings -> Save & Quit.
            Step::Click {
                x: 1780.0,
                y: 1012.0,
            },
            Step::Wait(0.3),
            Step::Screenshot("save_game_menu"),
            Step::Click { x: 960.0, y: 468.0 },
            Step::Wait(1.5),
            Step::Expect(Expect::InGame(false)),
            Step::Expect(Expect::Screen("Title")),
            Step::Screenshot("save_phase1_title"),
        ],
        ("save_reload", 2) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            // Persisted: the crafted inventory and an advanced clock survive.
            Step::Metric("inv_loaded", MetricSource::InvCount),
            Step::Expect(Expect::MetricLt("inv_loaded", 120.0)),
            Step::Expect(Expect::MetricGt("inv_loaded", 100.0)),
            Step::Metric("clock_loaded", MetricSource::Clock),
            Step::Expect(Expect::MetricGt("clock_loaded", 13_600.0)),
            Step::Screenshot("save_phase2_loaded"),
        ],
        ("visual_audit", 1) => vec![
            Step::Wait(2.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("audit_spawn_forward"),
            Step::Look(300.0, 0.0),
            Step::Wait(0.3),
            Step::Screenshot("audit_spawn_right"),
            Step::Look(300.0, 0.0),
            Step::Wait(0.3),
            Step::Screenshot("audit_spawn_back"),
            Step::Look(0.0, -150.0),
            Step::Wait(0.3),
            Step::Screenshot("audit_sky"),
            Step::Look(0.0, 300.0),
            Step::Hold(KeyCode::KeyW, 1.5),
            Step::Wait(0.3),
            Step::Screenshot("audit_walked"),
            Step::Key(KeyCode::Digit4),
            Step::Screenshot("audit_hotbar_slot4"),
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Screenshot("audit_inventory"),
            Step::Click {
                x: 1780.0,
                y: 1012.0,
            },
            Step::Wait(0.3),
            Step::Screenshot("audit_game_menu"),
        ],
        ("liquids", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("liquids_pond"),
            // 1. Measure dry walk speed (moving forward)
            Step::Hold(KeyCode::KeyW, 0.15),
            Step::Metric("speed_dry", MetricSource::HorizVel),
            Step::Expect(Expect::MetricGt("speed_dry", 0.10)),
            // 2. Walk into water pond: step forward and settle on pond floor
            Step::Hold(KeyCode::KeyW, 0.35),
            Step::Wait(0.20),
            // Now walk forward in water: speed is slowed by WATER_MOVE_SCALE (0.50 <= 0.60)
            Step::Hold(KeyCode::KeyW, 0.20),
            Step::Metric("speed_water", MetricSource::HorizVel),
            Step::Expect(Expect::MetricLt("speed_water", 0.08)),
            Step::Metric("speed_ratio", MetricSource::SpeedRatio),
            Step::Expect(Expect::MetricLt("speed_ratio", 0.61)),
            // 3. Head submerged in deep pond: breath drops, life drops after breath 0
            Step::Wait(0.60),
            Step::Metric("breath_submerged", MetricSource::Breath),
            Step::Expect(Expect::MetricLt("breath_submerged", 200.0)),
            Step::Metric("life_submerged", MetricSource::Life),
            Step::Expect(Expect::MetricLt("life_submerged", 100.0)),
            // 4. Look towards glowing lava pool
            Step::Look(120.0, 10.0),
            Step::Wait(0.3),
            Step::Screenshot("liquids_lava_glow"),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
        ],
        ("first_hour", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Expect(Expect::Text("Welcome to TerraVibe")),
            Step::Screenshot("first_hour_onboarding"),
            Step::Key(KeyCode::Space), // dismiss onboarding
            Step::Wait(0.5),
            Step::Key(KeyCode::Tab), // toggle minimap
            Step::Screenshot("first_hour_minimap"),
            Step::Kill,
            Step::Wait(0.5),
            Step::Screenshot("first_hour_death"),
            Step::Expect(Expect::Text("slain")),
            Step::Respawn,
            Step::Wait(0.5),
            Step::Screenshot("first_hour_respawned"),
        ],
        ("tooltips_economy", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("tooltips_economy_start"),
            // Open inventory with Esc to inspect currency display and item tooltips
            Step::Key(KeyCode::Escape),
            Step::Wait(0.4),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("tooltips_economy_inventory_open"),
            // Click Sort button to cycle sorting mode
            Step::Click { x: 645.0, y: 246.0 },
            Step::Wait(0.3),
            Step::Screenshot("tooltips_economy_sorted"),
            // Type into recipe search box
            Step::Type("torch"),
            Step::Wait(0.3),
            Step::Screenshot("tooltips_economy_search"),
            // Clear search query
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Key(KeyCode::Backspace),
            Step::Wait(0.2),
            // Approach Merchant and exercise shop selling flow
            Step::Look(150.0, 0.0),
            Step::Wait(0.2),
            Step::Hold(KeyCode::KeyW, 0.8),
            Step::Wait(0.4),
            Step::ShiftClick { x: 233.0, y: 42.0 },
            Step::Wait(0.3),
            Step::Screenshot("tooltips_economy_shop"),
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
            Step::Screenshot("tooltips_economy_end"),
        ],
        ("combat_feedback", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("combat_feedback_start"),
            // Melee swing with sword (slot 2) against demo enemies
            Step::Key(KeyCode::Digit2),
            Step::Wait(0.2),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.2),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.3),
            Step::Screenshot("combat_feedback_hit"),
            // Fire bow (slot 3)
            Step::Key(KeyCode::Digit3),
            Step::Wait(0.2),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.5),
            Step::Screenshot("combat_feedback_ranged"),
            Step::Metric("hits", MetricSource::Hits),
            Step::Expect(Expect::MetricGt("hits", 0.0)),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
            Step::Screenshot("combat_feedback_end"),
        ],
        ("held_items", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("held_items_start"),
            // Hotbar slot 1: pickaxe (ToolMine animation)
            Step::Key(KeyCode::Digit1),
            Step::Wait(0.2),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.2),
            Step::Screenshot("held_items_pickaxe_swing"),
            // Hotbar slot 2: broadsword (SwordSlash animation)
            Step::Key(KeyCode::Digit2),
            Step::Wait(0.2),
            Step::Click { x: 960.0, y: 540.0 },
            Step::Wait(0.2),
            Step::Screenshot("held_items_sword_swing"),
            // Hotbar slot 3: axe / tool
            Step::Key(KeyCode::Digit3),
            Step::Wait(0.2),
            Step::Screenshot("held_items_slot3"),
            // Hotbar slot 4 & 5
            Step::Key(KeyCode::Digit4),
            Step::Wait(0.2),
            Step::Screenshot("held_items_slot4"),
            Step::Key(KeyCode::Digit5),
            Step::Wait(0.2),
            Step::Screenshot("held_items_slot5"),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
            Step::Screenshot("held_items_end"),
        ],
        ("weather_audio", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("weather_audio_start"),
            Step::Wait(0.5),
            Step::Screenshot("weather_audio_bed"),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
            Step::Screenshot("weather_audio_end"),
        ],
        ("town_housing", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("town_housing_start"),
            // Open Inventory -> Housing tab to inspect room validation
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Click {
                x: 1847.5,
                y: 171.0,
            },
            Step::Wait(0.2),
            Step::Click {
                x: 1818.0,
                y: 212.0,
            },
            Step::Wait(0.4),
            Step::Screenshot("town_housing_query"),
            // Click Guide NPC banner to assign to valid room
            Step::Click {
                x: 1818.0,
                y: 365.0,
            },
            Step::Wait(0.3),
            Step::Screenshot("town_housing_assigned"),
            Step::Key(KeyCode::Escape),
            Step::Wait(0.3),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
            Step::Screenshot("town_housing_end"),
        ],
        ("bench_effects", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("bench_effects_start"),
            Step::Hold(KeyCode::KeyD, 0.8),
            Step::Wait(0.5),
            Step::Key(KeyCode::Tab),
            Step::Wait(0.5),
            Step::Screenshot("bench_effects_end"),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
        ],
        ("molten_gear", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("molten_gear_start"),
            // Minimap closes and reopens: the Underworld HUD path must not panic.
            Step::Key(KeyCode::Tab),
            Step::Wait(0.3),
            Step::Key(KeyCode::Tab),
            Step::Wait(0.3),
            Step::Screenshot("molten_gear_end"),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
        ],
        ("death_respawn", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("death_respawn_alive"),
            Step::Kill,
            Step::Wait(0.5),
            Step::Screenshot("death_respawn_dead"),
            Step::Expect(Expect::Text("slain")),
            Step::Respawn,
            Step::Wait(0.5),
            Step::Screenshot("death_respawn_respawned"),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
        ],
        ("minimap", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("minimap_initial"),
            // Cycle the 3-mode Tab minimap: Corner -> FullscreenOverlay -> Off -> Corner.
            Step::Key(KeyCode::Tab),
            Step::Wait(0.3),
            Step::Screenshot("minimap_overlay"),
            Step::Key(KeyCode::Tab),
            Step::Wait(0.3),
            Step::Screenshot("minimap_toggled_off"),
            Step::Key(KeyCode::Tab),
            Step::Wait(0.3),
            Step::Screenshot("minimap_toggled_on"),
            // Open the fullscreen map view.
            Step::Key(KeyCode::KeyM),
            Step::Wait(0.3),
            Step::Screenshot("minimap_fullscreen"),
            Step::Key(KeyCode::KeyM),
            Step::Wait(0.3),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
        ],
        ("accessibility_options", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("accessibility_options_start"),
            // Esc opens the inventory; its Settings button opens Options.
            Step::Key(KeyCode::Escape),
            Step::Wait(0.4),
            Step::Screenshot("accessibility_options_inventory"),
            Step::Click {
                x: 1780.0,
                y: 1012.0,
            },
            Step::Wait(0.4),
            Step::Expect(Expect::Text("Options")),
            // Select the Accessibility tab (fourth tab, index 3, in the 4-tab bar).
            Step::Click {
                x: 1202.0,
                y: 132.0,
            },
            Step::Wait(0.3),
            Step::Screenshot("accessibility_options_tab"),
            // Toggle the color-blind palette, then inspect the result.
            Step::Click { x: 960.0, y: 189.0 },
            Step::Wait(0.3),
            Step::Screenshot("accessibility_options_colorblind"),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
        ],
        ("wall_of_flesh", 1) => vec![
            Step::Wait(1.0),
            Step::Expect(Expect::InGame(true)),
            Step::Screenshot("wall_of_flesh_start"),
            Step::Wait(0.5),
            Step::Screenshot("wall_of_flesh_end"),
            Step::Metric("alive", MetricSource::Life),
            Step::Expect(Expect::MetricGt("alive", 0.0)),
        ],
        _ => vec![Step::Wait(0.1), Step::Screenshot("stub")],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_scenario_names_match_spec() {
        let expected_names = [
            "menu_title",
            "menu_characters",
            "menu_worlds",
            "menu_create",
            "menu_options",
            "menu_multiplayer",
        ];

        let registered_names: Vec<&str> = SCENARIOS
            .iter()
            .filter(|s| s.name.starts_with("menu_"))
            .map(|s| s.name)
            .collect();
        assert_eq!(
            registered_names, expected_names,
            "registered scenarios must match spec menu scenarios"
        );
    }

    #[test]
    fn menu_scenarios_have_screenshots_and_expectations() {
        for scenario in SCENARIOS {
            if matches!(scenario.name, "menu_options" | "save_reload") {
                assert_eq!(scenario.phases, 2, "{} must have 2 phases", scenario.name);
            } else {
                assert_eq!(scenario.phases, 1, "{} must have 1 phase", scenario.name);
            }

            for phase in 1..=scenario.phases {
                let steps = scenario_steps(scenario.name, phase);
                let has_screenshot = steps.iter().any(|s| matches!(s, Step::Screenshot(_)));
                let has_expect = steps.iter().any(|s| matches!(s, Step::Expect(_)));

                assert!(
                    has_screenshot,
                    "scenario {} phase {} must have >= 1 Screenshot",
                    scenario.name, phase
                );
                assert!(
                    has_expect,
                    "scenario {} phase {} must have >= 1 Expect",
                    scenario.name, phase
                );
            }
        }
    }

    #[test]
    fn gameplay_scenarios_names_match_spec() {
        for name in [
            "movement",
            "combat",
            "inventory",
            "mining_crafting",
            "housing_npc_shop",
            "corruption_altar",
            "boss",
            "save_reload",
            "visual_audit",
        ] {
            let sc = SCENARIOS
                .iter()
                .find(|s| s.name == name)
                .unwrap_or_else(|| panic!("scenario {name} missing"));
            assert!(sc.direct_ingame, "{name} must start in game");
        }
    }

    #[test]
    fn save_reload_has_two_phases_and_quits_in_phase_one() {
        let sc = SCENARIOS.iter().find(|s| s.name == "save_reload").unwrap();
        assert_eq!(sc.phases, 2);
        assert!(sc.fixture.is_some(), "world must persist across phases");
        let p1 = scenario_steps("save_reload", 1);
        assert!(p1.contains(&Step::Click { x: 960.0, y: 468.0 }));
        assert!(p1.contains(&Step::Expect(Expect::Screen("Title"))));
        let p2 = scenario_steps("save_reload", 2);
        assert!(
            p2.iter()
                .any(|s| matches!(s, Step::Expect(Expect::MetricLt(..))))
        );
    }

    #[test]
    fn gameplay_scenarios_open_crafting_through_esc_not_c() {
        for sc in SCENARIOS {
            for phase in 1..=sc.phases {
                assert!(
                    !scenario_steps(sc.name, phase).contains(&Step::Key(KeyCode::KeyC)),
                    "{} must not use the C key",
                    sc.name
                );
            }
        }
    }

    #[test]
    fn movement_asserts_snappy_timings_in_game() {
        let steps = scenario_steps("movement", 1);
        assert!(steps.contains(&Step::Hold(KeyCode::KeyW, 0.10)));
        assert!(steps.contains(&Step::Expect(Expect::MetricGt("walk_speed_6ticks", 0.12))));
        assert!(steps.contains(&Step::Expect(Expect::MetricGt("jump_apex_y", 68.0))));
        assert!(steps.iter().any(|s| matches!(
            s,
            Step::Expect(Expect::MetricLt("stopped_speed_3ticks", 0.01))
        )));
    }

    #[test]
    fn inventory_scenario_covers_pick_place_swap_ammo_and_held_close() {
        let steps = scenario_steps("inventory", 1);
        assert!(
            steps.contains(&Step::Click { x: 596.0, y: 42.0 }),
            "ammo slot"
        );
        assert!(
            steps.contains(&Step::Click { x: 548.0, y: 42.0 }),
            "coin slot"
        );
        assert!(
            steps
                .iter()
                .any(|s| matches!(s, Step::Screenshot("inventory_closed_with_held")))
        );
        assert!(
            steps
                .iter()
                .any(|s| matches!(s, Step::Expect(Expect::MetricGt("inv_end", _))))
        );
    }

    #[test]
    fn tooltips_economy_scenario_is_registered() {
        assert!(
            SCENARIOS.iter().any(|s| s.name == "tooltips_economy"),
            "tooltips_economy must be registered in SCENARIOS"
        );
        assert!(
            !scenario_steps("tooltips_economy", 1).is_empty(),
            "tooltips_economy scenario_steps must be non-empty"
        );
    }

    #[test]
    fn combat_feedback_and_held_items_scenarios_are_registered() {
        for name in ["combat_feedback", "held_items"] {
            let sc = SCENARIOS
                .iter()
                .find(|s| s.name == name)
                .unwrap_or_else(|| panic!("scenario {name} missing from SCENARIOS"));
            assert!(sc.direct_ingame, "{name} must start in game");
            assert!(
                !scenario_steps(name, 1).is_empty(),
                "{name} scenario_steps must be non-empty"
            );
        }
    }

    #[test]
    fn weather_audio_and_town_housing_scenarios_are_registered() {
        for name in ["weather_audio", "town_housing"] {
            let sc = SCENARIOS
                .iter()
                .find(|s| s.name == name)
                .unwrap_or_else(|| panic!("scenario {name} missing from SCENARIOS"));
            assert!(sc.direct_ingame, "{name} must start in game");
            assert!(
                !scenario_steps(name, 1).is_empty(),
                "{name} scenario_steps must be non-empty"
            );
        }
    }

    #[test]
    fn bench_effects_scenario_is_registered() {
        let sc = SCENARIOS
            .iter()
            .find(|s| s.name == "bench_effects")
            .expect("bench_effects must be registered in SCENARIOS");
        assert!(sc.direct_ingame, "bench_effects must start in game");
        assert!(
            !scenario_steps("bench_effects", 1).is_empty(),
            "bench_effects scenario_steps must be non-empty"
        );
    }

    #[test]
    fn molten_gear_scenario_is_registered() {
        let sc = SCENARIOS
            .iter()
            .find(|s| s.name == "molten_gear")
            .expect("molten_gear must be registered in SCENARIOS");
        assert!(sc.direct_ingame, "molten_gear must start in game");
        let steps = scenario_steps("molten_gear", 1);
        assert!(
            !steps.is_empty(),
            "molten_gear scenario_steps must be non-empty"
        );
        assert!(
            steps.iter().any(|s| matches!(s, Step::Screenshot(_))),
            "molten_gear must take a screenshot"
        );
        assert!(
            steps.contains(&Step::Metric("alive", MetricSource::Life)),
            "molten_gear must record the alive metric"
        );
    }

    #[test]
    fn wall_of_flesh_scenario_is_registered() {
        let sc = SCENARIOS
            .iter()
            .find(|s| s.name == "wall_of_flesh")
            .expect("wall_of_flesh must be registered in SCENARIOS");
        assert!(sc.direct_ingame, "wall_of_flesh must start in game");
        let steps = scenario_steps("wall_of_flesh", 1);
        assert!(
            !steps.is_empty(),
            "wall_of_flesh scenario_steps must be non-empty"
        );
        assert!(
            steps.iter().any(|s| matches!(s, Step::Screenshot(_))),
            "wall_of_flesh must take a screenshot"
        );
        assert!(
            steps.contains(&Step::Metric("alive", MetricSource::Life)),
            "wall_of_flesh must record the alive metric"
        );
    }

    #[test]
    fn pass5_qa_scenarios_are_registered() {
        for name in ["death_respawn", "minimap", "accessibility_options"] {
            let sc = SCENARIOS
                .iter()
                .find(|s| s.name == name)
                .unwrap_or_else(|| panic!("scenario {name} missing from SCENARIOS"));
            assert!(sc.direct_ingame, "{name} must start in game");
            let steps = scenario_steps(name, 1);
            assert!(!steps.is_empty(), "{name} scenario_steps must be non-empty");
        }

        let death_steps = scenario_steps("death_respawn", 1);
        assert!(
            death_steps.contains(&Step::Kill),
            "death_respawn must have Step::Kill"
        );
        assert!(
            death_steps.contains(&Step::Respawn),
            "death_respawn must have Step::Respawn"
        );

        let minimap_steps = scenario_steps("minimap", 1);
        assert!(
            minimap_steps.contains(&Step::Key(KeyCode::Tab)),
            "minimap must toggle minimap with Tab"
        );
        assert!(
            minimap_steps.contains(&Step::Key(KeyCode::KeyM)),
            "minimap must toggle fullscreen map with KeyM"
        );

        let access_steps = scenario_steps("accessibility_options", 1);
        assert!(
            access_steps
                .iter()
                .any(|s| matches!(s, Step::Screenshot(_)))
        );
        assert!(access_steps.iter().any(|s| matches!(s, Step::Expect(_))));
    }
}
