//! Multiplayer menu screen: Host & Play, Join Game, and Back.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};

use super::style::{
    BORDER_W, BUTTON_WIDTH, COLOR_BACKDROP, COLOR_HEADER_DIVIDER, COLOR_PANEL_BG,
    COLOR_PANEL_BORDER, COLOR_PANEL_INNER, COLOR_TEXT_GOLD, COLOR_TEXT_MUTED, RADIUS_PANEL,
    spawn_button_ext, viewport_size,
};
use super::text::{MenuText, MenuTextColor};
use super::{Action, MenuFlow};
use crate::session::MenuScreen;

pub const MULTIPLAYER_COPY: &str = "Host a co-op world with a join code or LAN,\nor join a friend's game from the server browser.\nNo port forwarding required.";

pub fn setup_multiplayer(
    mut commands: Commands,
    primary_window: Option<Query<&Window, With<PrimaryWindow>>>,
) {
    let (win_w, win_h) = viewport_size(primary_window.as_ref());
    let center_x = win_w * 0.5;
    let panel_w = 640.0;
    let panel_h = 452.0;
    let panel_top = ((win_h - panel_h) * 0.5).max(40.0);

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
        DespawnOnExit(MenuScreen::Multiplayer),
    ));

    // Root panel container
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
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            BackgroundColor(COLOR_PANEL_BG),
            BorderColor::all(COLOR_PANEL_BORDER),
            DespawnOnExit(MenuScreen::Multiplayer),
            GlobalZIndex(0),
        ))
        .with_children(|p| {
            // Screen title
            p.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(10.0)),
                    ..default()
                },
                MenuText::new("Multiplayer", 1.5),
                MenuTextColor(COLOR_TEXT_GOLD),
            ));

            p.spawn((
                Node {
                    width: Val::Px(panel_w - 64.0),
                    height: Val::Px(2.0),
                    margin: UiRect::bottom(Val::Px(16.0)),
                    ..default()
                },
                BackgroundColor(COLOR_HEADER_DIVIDER),
            ));

            // Informational notice card
            p.spawn((
                Node {
                    width: Val::Px(560.0),
                    border: UiRect::all(Val::Px(BORDER_W)),
                    border_radius: BorderRadius::all(Val::Px(RADIUS_PANEL)),
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(12.0)),
                    margin: UiRect::bottom(Val::Px(20.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(COLOR_PANEL_INNER),
                BorderColor::all(COLOR_PANEL_BORDER),
            ))
            .with_children(|notice| {
                notice.spawn((
                    Node {
                        max_width: Val::Px(520.0),
                        ..default()
                    },
                    MenuText::new(MULTIPLAYER_COPY, 0.9),
                    MenuTextColor(COLOR_TEXT_MUTED),
                ));
            });
        });

    // Buttons
    let btn_x = center_x - BUTTON_WIDTH / 2.0;
    let b0 = spawn_button_ext(
        &mut commands,
        "Host & Play",
        Action::StartHostFlow,
        0,
        (btn_x, panel_top + 218.0),
        false,
    );
    commands
        .entity(b0)
        .insert((DespawnOnExit(MenuScreen::Multiplayer), GlobalZIndex(1)));

    let b1 = spawn_button_ext(
        &mut commands,
        "Join Game",
        Action::StartJoinFlow,
        1,
        (btn_x, panel_top + 276.0),
        false,
    );
    commands
        .entity(b1)
        .insert((DespawnOnExit(MenuScreen::Multiplayer), GlobalZIndex(1)));

    let b2 = spawn_button_ext(
        &mut commands,
        "Back",
        Action::Back,
        2,
        (btn_x, panel_top + 356.0),
        false,
    );
    commands
        .entity(b2)
        .insert((DespawnOnExit(MenuScreen::Multiplayer), GlobalZIndex(1)));
}

pub fn handle_multiplayer_actions(
    actions: Option<MessageReader<Action>>,
    mut flow: Option<ResMut<MenuFlow>>,
    mut next_screen: Option<ResMut<NextState<MenuScreen>>>,
) {
    let Some(mut actions) = actions else {
        return;
    };
    for action in actions.read() {
        match action {
            Action::StartHostFlow => {
                if let Some(ref mut f) = flow {
                    **f = MenuFlow::Host;
                }
                if let Some(ref mut ns) = next_screen {
                    ns.set(MenuScreen::Characters);
                }
            }
            Action::StartJoinFlow => {
                if let Some(ref mut f) = flow {
                    **f = MenuFlow::Join;
                }
                if let Some(ref mut ns) = next_screen {
                    ns.set(MenuScreen::Characters);
                }
            }
            _ => {}
        }
    }
}

pub struct MultiplayerPlugin;

impl Plugin for MultiplayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuFlow>()
            .add_message::<Action>()
            .add_systems(OnEnter(MenuScreen::Multiplayer), setup_multiplayer)
            .add_systems(
                Update,
                handle_multiplayer_actions.run_if(in_state(MenuScreen::Multiplayer)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::MenuButton;

    #[test]
    fn multiplayer_buttons_render_above_panel() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::session::AppState>()
            .add_sub_state::<MenuScreen>()
            .add_plugins(MultiplayerPlugin);

        app.world_mut()
            .resource_mut::<NextState<MenuScreen>>()
            .set(MenuScreen::Multiplayer);
        app.update();

        // Find the panel entity (has BackgroundColor COLOR_PANEL_BG, Without<MenuButton>)
        let mut panel_query = app
            .world_mut()
            .query_filtered::<(Entity, &BackgroundColor, Option<&GlobalZIndex>), Without<MenuButton>>();
        let panel_matches: Vec<_> = panel_query
            .iter(app.world())
            .filter(|(_, bg, _)| bg.0 == COLOR_PANEL_BG)
            .collect();
        assert_eq!(
            panel_matches.len(),
            1,
            "Must have exactly 1 panel container"
        );
        let (_panel_ent, _panel_bg, panel_z) = panel_matches[0];
        let panel_global_z = panel_z.map(|z| z.0).unwrap_or(0);

        // Find buttons (With<MenuButton>)
        let mut button_query = app
            .world_mut()
            .query_filtered::<(Entity, &MenuButton, Option<&GlobalZIndex>), With<MenuButton>>();
        let buttons: Vec<_> = button_query.iter(app.world()).collect();
        assert_eq!(buttons.len(), 3, "Must have 3 buttons: Host, Join, Back");

        for (_btn_ent, btn, maybe_z) in buttons {
            let btn_z = maybe_z.map(|z| z.0).expect(&format!(
                "Button rank {} must have explicit GlobalZIndex to stack above panel",
                btn.rank
            ));
            assert!(
                btn_z > panel_global_z,
                "Button rank {} (z={}) must have GlobalZIndex strictly greater than panel (z={})",
                btn.rank,
                btn_z,
                panel_global_z
            );
        }
    }
}
