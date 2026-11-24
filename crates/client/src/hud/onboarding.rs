//! First-spawn onboarding card and contextual interaction prompts.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use terravibe_sim::BlockQuery;
use terravibe_sim::block::{CHEST, WORKBENCH};
use terravibe_sim::crafting::station_in_reach;
use terravibe_sim::enemy::EnemyKind;
use terravibe_sim::interact::{RAY_MAX, in_reach, raycast};
use terravibe_sim::item::SUSPICIOUS_LOOKING_EYE;

use crate::hud::text::HudText;
pub use crate::net::CharacterFacts;
use crate::session::AppState;
use crate::view::camera::FlyCamera;
use crate::view::chunks::ChunkStream;
use crate::view::enemies::{ClientEnemy, EnemyInterp};
use crate::view::player::{LoadedBlocks, LocalPlayer};

const PICK_RADIUS: f32 = 0.65;

/// Marker component for the onboarding controls card.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnboardingCardUi;

/// Marker component for the dismiss button on the onboarding card.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnboardingDismissButton;

/// Marker component to request immediate onboarding card dismissal.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DismissRequested;

/// Marker component for the contextual interaction prompt UI.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextPromptUi;

/// Contextual prompts state resource.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextualPromptsState {
    pub current_prompt: Option<&'static str>,
}

/// Sets up persistent HUD nodes for contextual prompts.
pub fn setup(mut commands: Commands) {
    // Root container for contextual prompt at bottom-center of the HUD
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(50.0),
                width: Val::Percent(100.0),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(1050),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|parent| {
            parent.spawn((
                ContextPromptUi,
                HudText {
                    text: "".into(),
                    death: false,
                },
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(super::style::BORDER_W)),
                    border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(super::style::PANEL_BG_SOLID),
                BorderColor::all(super::style::PANEL_BORDER),
                Visibility::Hidden,
            ));
        });
}

/// Formats the onboarding controls list from `bindings`.
pub fn format_onboarding_controls(bindings: &crate::input_map::InputBindings) -> Vec<String> {
    use crate::input_map::{GameAction, keycode_name};
    let k = |a: GameAction| {
        let name = keycode_name(bindings.key(a)).unwrap_or("?");
        name.strip_prefix("Key").unwrap_or(name)
    };
    vec![
        format!(
            "Movement: {}{}{}{}",
            k(GameAction::MoveForward),
            k(GameAction::MoveLeft),
            k(GameAction::MoveBack),
            k(GameAction::MoveRight)
        ),
        format!("Jump: {}", k(GameAction::Jump)),
        "Look / Aim: Mouse".to_string(),
        "Mine / Attack: Left Click".to_string(),
        "Place / Interact: Right Click".to_string(),
        format!("Throw Item: {}", k(GameAction::Throw)),
        format!("Inventory: {}", k(GameAction::Inventory)),
        "Tip: Talk to the Guide for advice on what to do next.".to_string(),
    ]
}

/// Syncs the first-spawn onboarding card: visible until dismissed, then persisted.
#[allow(clippy::too_many_arguments)]
pub fn sync_onboarding_ui(
    mut commands: Commands,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    settings: Option<Res<crate::settings::Settings>>,
    mut char_facts: Option<ResMut<CharacterFacts>>,
    mut local_facts: Option<ResMut<crate::net::LocalCharacterFacts>>,
    mut selected_char: Option<ResMut<crate::session::SelectedCharacter>>,
    net: Option<Res<crate::net::Net>>,
    inv_open: Option<Res<crate::hud::inventory::InventoryOpen>>,
    town_panel: Option<Res<crate::hud::town::TownPanel>>,
    chest_panel: Option<Res<crate::hud::chest::ChestPanel>>,
    mut card_q: Query<(Entity, &mut Visibility), With<OnboardingCardUi>>,
    btn_q: Query<(&ComputedNode, &GlobalTransform), With<OnboardingDismissButton>>,
    dismiss_req_q: Query<Entity, With<DismissRequested>>,
) {
    let has_dismiss_request = !dismiss_req_q.is_empty();
    for req in &dismiss_req_q {
        commands.entity(req).despawn();
    }

    let is_dismissed = if let Some(cf) = char_facts.as_ref() {
        cf.onboarding_dismissed
    } else if let Some(lf) = local_facts.as_ref() {
        lf.0.onboarding_dismissed
    } else {
        false
    };

    if is_dismissed {
        for (card, _) in &card_q {
            commands.entity(card).despawn();
        }
        return;
    }

    let panel_open = inv_open.as_ref().is_some_and(|i| i.0)
        || town_panel.as_ref().is_some_and(|t| t.is_open())
        || chest_panel.as_ref().is_some_and(|c| c.open.is_some());

    // Check for dismissal input (only when the card is visible, not hidden behind panels)
    let key_dismiss = !panel_open
        && keys.as_ref().is_some_and(|k| {
            k.just_pressed(KeyCode::Space)
                || k.just_pressed(KeyCode::Escape)
                || k.just_pressed(KeyCode::Enter)
                || k.just_pressed(KeyCode::NumpadEnter)
                || k.pressed(KeyCode::Space)
                || k.pressed(KeyCode::Escape)
                || k.pressed(KeyCode::Enter)
                || k.pressed(KeyCode::NumpadEnter)
        });

    let mut click_dismiss = false;
    if !panel_open {
        if let Some(mouse) = mouse.as_ref() {
            if mouse.just_pressed(MouseButton::Left) {
                if let Ok(window) = windows.single() {
                    if let Some(cursor) = window.cursor_position() {
                        for (node, transform) in &btn_q {
                            let center = transform.translation().truncate();
                            let size = node.size();
                            let min = center - size * 0.5;
                            let max = center + size * 0.5;
                            if cursor.x >= min.x
                                && cursor.x <= max.x
                                && cursor.y >= min.y
                                && cursor.y <= max.y
                            {
                                click_dismiss = true;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    if key_dismiss || click_dismiss || has_dismiss_request {
        if let Some(net) = net.as_ref() {
            let _ = net.send(terravibe_sim::net::ClientMsg::DismissOnboarding);
        }
        if let Some(ref mut cf) = char_facts {
            cf.onboarding_dismissed = true;
        }
        if let Some(ref mut lf) = local_facts {
            lf.0.onboarding_dismissed = true;
        }
        if let Some(ref mut sc) = selected_char {
            if let Some(ref mut c) = sc.0 {
                c.facts.onboarding_dismissed = true;
                let root = terravibe_server::characters::characters_dir();
                let _ = terravibe_server::characters::save_character(&root, c);
            }
        }
        for (card, _) in &card_q {
            commands.entity(card).despawn();
        }
        return;
    }

    let want_vis = if panel_open {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };

    for (_, mut vis) in &mut card_q {
        if *vis != want_vis {
            *vis = want_vis;
        }
    }

    // If not dismissed and card entity does not exist yet, spawn it.
    if card_q.iter().next().is_none() {
        commands
            .spawn((
                OnboardingCardUi,
                want_vis,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(24.0),
                    top: Val::Px(96.0),
                    width: Val::Auto,
                    min_width: Val::Px(360.0),
                    max_width: Val::Px(520.0),
                    padding: UiRect::all(Val::Px(16.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    border: UiRect::all(Val::Px(super::style::BORDER_W)),
                    border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_PANEL)),
                    ..default()
                },
                BackgroundColor(super::style::PANEL_BG_SOLID),
                BorderColor::all(super::style::GOLD_BORDER),
                GlobalZIndex(1100),
                DespawnOnExit(AppState::InGame),
            ))
            .with_children(|parent| {
                // Header
                parent.spawn((
                    HudText {
                        text: "Welcome to TerraVibe".into(),
                        death: true,
                    },
                    Node {
                        margin: UiRect::bottom(Val::Px(4.0)),
                        ..default()
                    },
                ));

                // Controls list
                let bindings = settings
                    .as_deref()
                    .map_or_else(crate::input_map::InputBindings::default, |s| s.bindings);
                let controls = format_onboarding_controls(&bindings);

                for item in controls {
                    parent.spawn((
                        HudText {
                            text: item,
                            death: false,
                        },
                        Node::default(),
                    ));
                }

                // Dismiss button
                parent
                    .spawn((
                        OnboardingDismissButton,
                        Node {
                            margin: UiRect::top(Val::Px(8.0)),
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                            border: UiRect::all(Val::Px(super::style::BORDER_W)),
                            border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            align_self: AlignSelf::FlexStart,
                            ..default()
                        },
                        BackgroundColor(super::style::BTN_BG),
                        BorderColor::all(super::style::GOLD_BORDER),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            HudText {
                                text: "[Dismiss] (Space / Esc / Enter)".into(),
                                death: false,
                            },
                            Node::default(),
                        ));
                    });
            });
    }
}

#[derive(Default)]
pub struct PromptThrottle {
    last_pos: Option<[f32; 3]>,
    last_held: Option<terravibe_sim::item::ItemId>,
    timer: f32,
}

/// Evaluates player context within interaction reach and updates contextual prompts.
#[allow(clippy::too_many_arguments)]
pub fn update_contextual_prompts(
    mut state: ResMut<ContextualPromptsState>,
    player: Option<Res<LocalPlayer>>,
    stream: Option<Res<ChunkStream>>,
    cam: Option<Single<&Transform, With<FlyCamera>>>,
    inv: Option<Res<crate::net::LocalInventory>>,
    held: Option<Res<crate::net::LocalHeld>>,
    time: Option<Res<Time>>,
    npcs: Query<(&ClientEnemy, Option<&EnemyInterp>, Option<&Transform>)>,
    mut prompt_ui: Query<(&mut Visibility, &mut HudText), With<ContextPromptUi>>,
    mut throttle: Local<PromptThrottle>,
) {
    let mut prompt: Option<&'static str> = None;

    if let Some(player) = player.as_ref() {
        let body = &player.0.body;
        let feet = body.pos;
        let cur_held = inv
            .as_ref()
            .and_then(|i| i.0.selected_stack().map(|s| s.id))
            .or_else(|| held.as_ref().and_then(|h| h.0.map(|s| s.id)));
        let moved = throttle.last_pos.is_none_or(|lp| {
            let dx = feet[0] - lp[0];
            let dy = feet[1] - lp[1];
            let dz = feet[2] - lp[2];
            dx * dx + dy * dy + dz * dz >= 1.0
        });
        let held_changed = cur_held != throttle.last_held;
        throttle.timer += time.as_ref().map_or(0.25, |t| t.delta_secs());
        if !moved && !held_changed && throttle.timer < 0.25 {
            return;
        }
        throttle.last_pos = Some(feet);
        throttle.last_held = cur_held;
        throttle.timer = 0.0;

        let mut ray_hit_guide = false;
        let mut prox_guide = false;

        // Check Guide NPC
        for (enemy, interp, xform) in &npcs {
            if enemy.kind == EnemyKind::Guide {
                let pos = if let Some(i) = interp {
                    i.pos()
                } else if let Some(x) = xform {
                    x.translation
                } else {
                    continue;
                };

                if let Some(cam) = cam.as_ref() {
                    let eye = cam.translation;
                    let dir = *cam.forward();
                    let c = pos + Vec3::Y * 0.85;
                    let t = (c - eye).dot(dir);
                    let off = (c - (eye + dir * t)).length();
                    if t > 0.0 && t < RAY_MAX && off < PICK_RADIUS {
                        let cell = [pos.x.floor() as i32, pos.y as i32 + 1, pos.z.floor() as i32];
                        if in_reach(body, cell) {
                            ray_hit_guide = true;
                        }
                    }
                }

                let cell = [
                    pos.x.floor() as i32,
                    pos.y.floor() as i32 + 1,
                    pos.z.floor() as i32,
                ];
                let alt_cell = [
                    pos.x.floor() as i32,
                    pos.y.floor() as i32,
                    pos.z.floor() as i32,
                ];
                let dist = (pos - Vec3::from(feet)).length();
                if in_reach(body, cell) || in_reach(body, alt_cell) || dist <= 5.0 {
                    prox_guide = true;
                }
            }
        }

        // Check Chest
        let mut ray_hit_chest = false;
        let mut prox_chest = false;

        if let Some(stream) = stream.as_ref() {
            let q = LoadedBlocks(stream.generated());

            if let Some(cam) = cam.as_ref() {
                if let Some(h) =
                    raycast(&q, cam.translation.into(), (*cam.forward()).into(), RAY_MAX)
                {
                    if in_reach(body, h.block)
                        && q.block(h.block[0], h.block[1], h.block[2]) == CHEST
                    {
                        ray_hit_chest = true;
                    }
                }
            }

            let (bx, by, bz) = (
                feet[0].floor() as i32,
                feet[1].floor() as i32,
                feet[2].floor() as i32,
            );
            for dx in -3..=3 {
                for dy in -2..=3 {
                    for dz in -3..=3 {
                        let c = [bx + dx, by + dy, bz + dz];
                        if q.block(c[0], c[1], c[2]) == CHEST && in_reach(body, c) {
                            prox_chest = true;
                            break;
                        }
                    }
                    if prox_chest {
                        break;
                    }
                }
                if prox_chest {
                    break;
                }
            }
        }

        // Evaluate prompt in priority order
        if ray_hit_guide || (prox_guide && !ray_hit_chest) {
            prompt = Some("[Right Click] Talk to Guide");
        } else if ray_hit_chest || prox_chest {
            prompt = Some("[Right Click] Open Chest");
        } else if let Some(stream) = stream.as_ref() {
            let q = LoadedBlocks(stream.generated());
            if station_in_reach(&q, feet, WORKBENCH) {
                prompt = Some("[Work Bench] Crafting Station Nearby");
            }
        }

        // Check holding Suspicious Looking Eye
        if prompt.is_none() {
            let holding_eye = inv.as_ref().is_some_and(|i| {
                i.0.selected_stack()
                    .is_some_and(|s| s.id == SUSPICIOUS_LOOKING_EYE)
            }) || held
                .as_ref()
                .is_some_and(|h| h.0.as_ref().is_some_and(|s| s.id == SUSPICIOUS_LOOKING_EYE));

            if holding_eye {
                prompt = Some("Use at night to summon the Eye of Cthulhu");
            }
        }
    }

    state.current_prompt = prompt;

    for (mut vis, mut text) in &mut prompt_ui {
        if let Some(p) = prompt {
            *vis = Visibility::Inherited;
            if text.text != p {
                text.text = p.to_string();
            }
        } else {
            *vis = Visibility::Hidden;
            if !text.text.is_empty() {
                text.text.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use terravibe_sim::ChunkPos;
    use terravibe_sim::block::{AIR, CHEST, WORKBENCH};
    use terravibe_sim::chunk::Chunk;
    use terravibe_sim::enemy::{EnemyId, EnemyKind};
    use terravibe_sim::inventory::{Inventory, ItemStack};
    use terravibe_sim::item::{Prefix, SUSPICIOUS_LOOKING_EYE};
    use terravibe_sim::player::Player;

    use crate::net::{LocalHeld, LocalInventory};
    use crate::session::AppState;
    use crate::view::chunks::ChunkStream;
    use crate::view::enemies::{ClientEnemy, EnemyInterp};
    use crate::view::player::LocalPlayer;

    pub fn test_hud_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ContextualPromptsState>()
            .init_resource::<CharacterFacts>()
            .init_resource::<crate::net::LocalCharacterFacts>()
            .init_resource::<LocalHeld>()
            .insert_resource(LocalPlayer(Player::new([0.0, 50.0, 0.0])))
            .insert_resource(LocalInventory(Inventory::empty()))
            .init_resource::<ChunkStream>()
            .add_systems(Startup, setup)
            .add_systems(Update, (sync_onboarding_ui, update_contextual_prompts));
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app
    }

    pub fn set_player_pos(app: &mut App, pos: [f32; 3]) {
        if let Some(mut p) = app.world_mut().get_resource_mut::<LocalPlayer>() {
            p.0.body.pos = pos;
        } else {
            app.insert_resource(LocalPlayer(Player::new(pos)));
        }
    }

    pub fn spawn_guide_npc(app: &mut App, pos: [f32; 3]) -> Entity {
        app.world_mut()
            .spawn((
                ClientEnemy::new(EnemyId(1), EnemyKind::Guide),
                Transform::from_xyz(pos[0], pos[1], pos[2]),
                EnemyInterp {
                    from: Vec3::from(pos),
                    to: Vec3::from(pos),
                    t: 1.0,
                },
            ))
            .id()
    }

    pub fn press_key(app: &mut App, key: KeyCode) {
        if let Some(mut keys) = app.world_mut().get_resource_mut::<ButtonInput<KeyCode>>() {
            keys.press(key);
        }
    }

    #[test]
    fn onboarding_card_visible_only_when_not_dismissed() {
        let mut app = test_hud_app();
        app.insert_resource(CharacterFacts {
            onboarding_dismissed: false,
            ..default()
        });
        app.update();
        assert!(
            app.world_mut()
                .query::<&OnboardingCardUi>()
                .iter(app.world())
                .next()
                .is_some(),
            "Card should be visible when onboarding is not dismissed"
        );

        // Dismiss via Space
        press_key(&mut app, KeyCode::Space);
        app.update();
        assert!(
            app.world()
                .resource::<CharacterFacts>()
                .onboarding_dismissed
        );
        assert!(
            app.world_mut()
                .query::<&OnboardingCardUi>()
                .iter(app.world())
                .next()
                .is_none(),
            "Card should be despawned after dismissal"
        );
    }

    #[test]
    fn onboarding_card_dismiss_by_esc_and_enter() {
        // Esc dismisses
        let mut app = test_hud_app();
        app.insert_resource(CharacterFacts {
            onboarding_dismissed: false,
            ..default()
        });
        app.update();
        assert!(
            app.world_mut()
                .query::<&OnboardingCardUi>()
                .iter(app.world())
                .next()
                .is_some()
        );
        press_key(&mut app, KeyCode::Escape);
        app.update();
        assert!(
            app.world()
                .resource::<CharacterFacts>()
                .onboarding_dismissed
        );
        assert!(
            app.world_mut()
                .query::<&OnboardingCardUi>()
                .iter(app.world())
                .next()
                .is_none()
        );

        // Enter dismisses
        let mut app = test_hud_app();
        app.insert_resource(CharacterFacts {
            onboarding_dismissed: false,
            ..default()
        });
        app.update();
        assert!(
            app.world_mut()
                .query::<&OnboardingCardUi>()
                .iter(app.world())
                .next()
                .is_some()
        );
        press_key(&mut app, KeyCode::Enter);
        app.update();
        assert!(
            app.world()
                .resource::<CharacterFacts>()
                .onboarding_dismissed
        );
        assert!(
            app.world_mut()
                .query::<&OnboardingCardUi>()
                .iter(app.world())
                .next()
                .is_none()
        );
    }

    #[test]
    fn onboarding_dismiss_via_request() {
        let mut app = test_hud_app();
        app.insert_resource(CharacterFacts {
            onboarding_dismissed: false,
            ..default()
        });
        app.update();
        assert!(
            app.world_mut()
                .query::<&OnboardingCardUi>()
                .iter(app.world())
                .next()
                .is_some()
        );

        app.world_mut().spawn(DismissRequested);
        app.update();
        assert!(
            app.world()
                .resource::<CharacterFacts>()
                .onboarding_dismissed
        );
        assert!(
            app.world_mut()
                .query::<&OnboardingCardUi>()
                .iter(app.world())
                .next()
                .is_none()
        );
    }

    #[test]
    fn onboarding_card_not_spawned_if_already_dismissed() {
        let mut app = test_hud_app();
        app.insert_resource(CharacterFacts {
            onboarding_dismissed: true,
            ..default()
        });
        app.update();
        assert!(
            app.world_mut()
                .query::<&OnboardingCardUi>()
                .iter(app.world())
                .next()
                .is_none(),
            "Card should not spawn if already dismissed"
        );
    }

    #[test]
    fn onboarding_card_contains_all_required_controls() {
        let mut app = test_hud_app();
        app.insert_resource(CharacterFacts {
            onboarding_dismissed: false,
            ..default()
        });
        app.update();

        let texts: Vec<String> = app
            .world_mut()
            .query::<&HudText>()
            .iter(app.world())
            .map(|t| t.text.clone())
            .collect();

        assert!(texts.iter().any(|t| t.contains("Movement: WASD")));
        assert!(texts.iter().any(|t| t.contains("Jump: Space")));
        assert!(texts.iter().any(|t| t.contains("Look / Aim: Mouse")));
        assert!(
            texts
                .iter()
                .any(|t| t.contains("Mine / Attack: Left Click"))
        );
        assert!(
            texts
                .iter()
                .any(|t| t.contains("Place / Interact: Right Click"))
        );
        assert!(texts.iter().any(|t| t.contains("Inventory: Esc")));
        assert!(texts.iter().any(|t| t.contains("Talk to the Guide")));
        assert!(texts.iter().any(|t| t.contains("[Dismiss]")));
    }

    #[test]
    fn prompt_appears_near_guide_and_chests() {
        let mut app = test_hud_app();
        set_player_pos(&mut app, [100.0, 50.0, 100.0]);
        spawn_guide_npc(&mut app, [101.0, 50.0, 100.0]);
        app.update();

        let state = app.world().resource::<ContextualPromptsState>();
        assert_eq!(state.current_prompt, Some("[Right Click] Talk to Guide"));

        // Prompt UI node is updated with the text
        let ui_text = app
            .world_mut()
            .query_filtered::<&HudText, With<ContextPromptUi>>()
            .iter(app.world())
            .next()
            .map(|t| t.text.clone());
        assert_eq!(ui_text, Some("[Right Click] Talk to Guide".into()));
    }

    #[test]
    fn prompt_appears_near_chest() {
        let mut app = test_hud_app();
        set_player_pos(&mut app, [50.0, 50.0, 50.0]);

        {
            let mut stream = app.world_mut().resource_mut::<ChunkStream>();
            let cpos = ChunkPos::of_block(51, 50, 50);
            stream.insert_chunk(cpos, Arc::new(Chunk::filled(AIR)));
            stream.set_block([51, 50, 50], CHEST);
        }
        app.update();

        let state = app.world().resource::<ContextualPromptsState>();
        assert_eq!(state.current_prompt, Some("[Right Click] Open Chest"));

        let ui_text = app
            .world_mut()
            .query_filtered::<&HudText, With<ContextPromptUi>>()
            .iter(app.world())
            .next()
            .map(|t| t.text.clone());
        assert_eq!(ui_text, Some("[Right Click] Open Chest".into()));
    }

    #[test]
    fn prompt_appears_near_workbench() {
        let mut app = test_hud_app();
        set_player_pos(&mut app, [10.0, 10.0, 10.0]);

        {
            let mut stream = app.world_mut().resource_mut::<ChunkStream>();
            let cpos = ChunkPos::of_block(11, 10, 10);
            stream.insert_chunk(cpos, Arc::new(Chunk::filled(AIR)));
            stream.set_block([11, 10, 10], WORKBENCH);
        }
        app.update();

        let state = app.world().resource::<ContextualPromptsState>();
        assert_eq!(
            state.current_prompt,
            Some("[Work Bench] Crafting Station Nearby")
        );

        let ui_text = app
            .world_mut()
            .query_filtered::<&HudText, With<ContextPromptUi>>()
            .iter(app.world())
            .next()
            .map(|t| t.text.clone());
        assert_eq!(ui_text, Some("[Work Bench] Crafting Station Nearby".into()));
    }

    #[test]
    fn prompt_appears_holding_suspicious_eye() {
        let mut app = test_hud_app();
        set_player_pos(&mut app, [200.0, 50.0, 200.0]);

        let mut inv = Inventory::empty();
        inv.slots[0] = Some(ItemStack {
            id: SUSPICIOUS_LOOKING_EYE,
            count: 1,
            prefix: Prefix(0),
        });
        inv.selected = 0;
        app.insert_resource(LocalInventory(inv));
        app.update();

        let state = app.world().resource::<ContextualPromptsState>();
        assert_eq!(
            state.current_prompt,
            Some("Use at night to summon the Eye of Cthulhu")
        );

        let ui_text = app
            .world_mut()
            .query_filtered::<&HudText, With<ContextPromptUi>>()
            .iter(app.world())
            .next()
            .map(|t| t.text.clone());
        assert_eq!(
            ui_text,
            Some("Use at night to summon the Eye of Cthulhu".into())
        );
    }
}
