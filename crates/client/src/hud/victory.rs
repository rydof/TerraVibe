//! End-of-era Victory modal overlay shown when the Wall of Flesh is defeated.

use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use super::text::HudText;
use crate::session::{AppState, MenuScreen};

#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct VictoryModalState {
    pub active: bool,
    pub fade_alpha: f32,
}

#[derive(Component)]
pub struct VictoryModalRoot;

#[derive(Component)]
pub struct VictoryContinueButton;

#[derive(Component)]
pub struct VictoryTitleButton;

pub fn setup(mut commands: Commands) {
    commands
        .spawn((
            VictoryModalRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(super::style::BACKDROP),
            Visibility::Hidden,
            GlobalZIndex(1250),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(36.0), Val::Px(28.0)),
                        border: UiRect::all(Val::Px(super::style::BORDER_W)),
                        border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_PANEL)),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(14.0),
                        min_width: Val::Px(420.0),
                        ..default()
                    },
                    BackgroundColor(super::style::PANEL_BG_SOLID),
                    BorderColor::all(super::style::GOLD_BORDER),
                ))
                .with_children(|card| {
                    card.spawn((
                        HudText {
                            text: "Wall of Flesh Defeated!".to_string(),
                            death: false,
                        },
                        super::text::HudTextColor(super::style::GOLD_TEXT),
                        Node::default(),
                    ));
                    card.spawn((
                        HudText {
                            text: "Pre-Hardmode Era Complete — World Remains Pre-Hardmode"
                                .to_string(),
                            death: false,
                        },
                        Node::default(),
                    ));
                    card.spawn((
                        VictoryContinueButton,
                        bevy::ui_widgets::Button,
                        Node {
                            min_width: Val::Px(220.0),
                            margin: UiRect::top(Val::Px(6.0)),
                            padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
                            border: UiRect::all(Val::Px(super::style::BORDER_W)),
                            border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor::all(super::style::GOLD_BORDER),
                        BackgroundColor(super::style::BTN_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            HudText {
                                text: "Continue Playing".to_string(),
                                death: false,
                            },
                            Node::default(),
                        ));
                    });
                    card.spawn((
                        VictoryTitleButton,
                        bevy::ui_widgets::Button,
                        Node {
                            min_width: Val::Px(220.0),
                            padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
                            border: UiRect::all(Val::Px(super::style::BORDER_W)),
                            border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor::all(super::style::PANEL_BORDER),
                        BackgroundColor(super::style::BTN_BG),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            HudText {
                                text: "Save & Return to Title".to_string(),
                                death: false,
                            },
                            Node::default(),
                        ));
                    });
                });
        });
}

pub fn update(
    time: Option<Res<Time>>,
    mut state: ResMut<VictoryModalState>,
    mut root_q: Query<(&mut Visibility, &mut BackgroundColor), With<VictoryModalRoot>>,
    mut cursor_q: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Ok((mut vis, mut bg)) = root_q.single_mut() else {
        return;
    };
    if !state.active {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        return;
    }
    *vis = Visibility::Inherited;
    let dt = time.as_ref().map_or(1.0 / 60.0, |t| t.delta_secs());
    state.fade_alpha = (state.fade_alpha + dt * 2.5).min(1.0);
    bg.0 = Color::srgba(0.02, 0.01, 0.03, 0.78 * state.fade_alpha);
    if let Ok(mut opts) = cursor_q.single_mut() {
        opts.grab_mode = CursorGrabMode::None;
        opts.visible = true;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn handle_input(
    mut state: ResMut<VictoryModalState>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    gamepads: Query<&Gamepad>,
    cont_q: Query<(), (Added<Pressed>, With<VictoryContinueButton>)>,
    title_q: Query<(), (Added<Pressed>, With<VictoryTitleButton>)>,
    mut cursor_q: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut next_app_state: Option<ResMut<NextState<AppState>>>,
    mut next_menu_screen: Option<ResMut<NextState<MenuScreen>>>,
) {
    if !state.active {
        return;
    }
    let key_continue = keys.as_ref().is_some_and(|k| {
        k.just_pressed(KeyCode::Space)
            || k.just_pressed(KeyCode::Enter)
            || k.just_pressed(KeyCode::NumpadEnter)
            || k.just_pressed(KeyCode::Escape)
    });
    let gamepad_continue = gamepads
        .iter()
        .any(|g| g.just_pressed(GamepadButton::South));
    if !cont_q.is_empty() || key_continue || gamepad_continue {
        state.active = false;
        state.fade_alpha = 0.0;
        if let Ok(mut opts) = cursor_q.single_mut() {
            opts.grab_mode = CursorGrabMode::Locked;
            opts.visible = false;
        }
        return;
    }
    if !title_q.is_empty() {
        state.active = false;
        state.fade_alpha = 0.0;
        if let Some(ref mut next_app) = next_app_state {
            next_app.set(AppState::Menu);
        }
        if let Some(ref mut next_menu) = next_menu_screen {
            next_menu.set(MenuScreen::Title);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn victory_modal_opens_continues_and_returns_to_title() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<AppState>()
            .init_state::<MenuScreen>()
            .init_resource::<VictoryModalState>()
            .add_systems(Startup, setup)
            .add_systems(Update, (update, handle_input));
        app.update();

        let vis = |app: &mut App| {
            *app.world_mut()
                .query_filtered::<&Visibility, With<VictoryModalRoot>>()
                .single(app.world())
                .unwrap()
        };
        assert_eq!(vis(&mut app), Visibility::Hidden);

        app.world_mut().resource_mut::<VictoryModalState>().active = true;
        app.update();
        assert_eq!(vis(&mut app), Visibility::Inherited);

        // Press Continue Playing
        let cont_entity = app
            .world_mut()
            .query_filtered::<Entity, With<VictoryContinueButton>>()
            .single(app.world())
            .unwrap();
        app.world_mut().entity_mut(cont_entity).insert(Pressed);
        app.update();
        assert!(!app.world().resource::<VictoryModalState>().active);
        assert_eq!(vis(&mut app), Visibility::Hidden);

        // Re-open and press Save & Return to Title
        app.world_mut().resource_mut::<VictoryModalState>().active = true;
        app.update();
        let title_entity = app
            .world_mut()
            .query_filtered::<Entity, With<VictoryTitleButton>>()
            .single(app.world())
            .unwrap();
        app.world_mut().entity_mut(title_entity).insert(Pressed);
        app.update();
        assert!(!app.world().resource::<VictoryModalState>().active);
        assert!(matches!(
            *app.world().resource::<NextState<AppState>>(),
            NextState::Pending(AppState::Menu)
        ));
    }
}
