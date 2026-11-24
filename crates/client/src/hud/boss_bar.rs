//! Boss health bar: a centred bar near the bottom of the screen, filled left to
//! right in proportion to `life / life_max`, with the boss name above it.

use bevy::prelude::*;

use super::text::HudText;
use crate::session::AppState;

/// Latest boss state from `ServerMsg::Boss`; `None` when no boss is alive.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct BossState(pub Option<BossInfo>);

/// Latest Wall of Flesh state from `ServerMsg::WofState`; `None` when inactive.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct WofBossState(pub Option<WofBossInfo>);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WofBossInfo {
    pub wall_x: f32,
    pub dir: i8,
    pub life: i32,
    pub life_max: i32,
}

impl WofBossState {
    pub fn set(&mut self, info: WofBossInfo) {
        self.0 = Some(info);
    }

    pub fn clear(&mut self) {
        self.0 = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BossInfo {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub life: i32,
    pub life_max: i32,
    pub phase: u8,
    pub state: u8,
    pub spin: f32,
}

impl BossState {
    pub fn set(&mut self, info: BossInfo) {
        self.0 = Some(info);
    }

    /// `ServerMsg::BossGone`.
    pub fn clear(&mut self) {
        self.0 = None;
    }
}

/// Bar width as a fraction of the screen.
const BAR_WIDTH_PERCENT: f32 = 40.0;
const BAR_HEIGHT: f32 = 16.0;
const BAR_BOTTOM: f32 = 48.0;
/// Duration of the boss intro title card in seconds.
pub const TITLE_CARD_DURATION_SECS: f32 = 3.0;

/// Tracks boss spawn transitions and the intro title card fade timer.
#[derive(Resource, Default, Debug, Clone)]
pub struct BossTitleCardState {
    pub active: bool,
    pub was_boss_alive: bool,
    pub timer: f32,
}

/// Alpha curve for the boss intro title card: fades in over 0.4s, holds, fades out over 0.8s.
pub fn title_card_alpha(timer: f32, duration: f32) -> f32 {
    if timer <= 0.0 || duration <= 0.0 {
        return 0.0;
    }
    let elapsed = (duration - timer).max(0.0);
    let fade_in = ((elapsed + 0.02) / 0.4).clamp(0.05, 1.0);
    let fade_out = (timer / 0.8).clamp(0.0, 1.0);
    fade_in.min(fade_out)
}

/// Vertical slide-in offset (px) for the boss bar during the first 0.4s after spawn.
pub fn boss_bar_entrance_offset(timer: f32, duration: f32, reduced_motion: bool) -> f32 {
    if reduced_motion || timer <= 0.0 || duration <= 0.0 {
        return 0.0;
    }
    let elapsed = (duration - timer).max(0.0);
    let t = (elapsed / 0.4).clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - t) * (1.0 - t);
    -36.0 * (1.0 - ease)
}

/// Fill fraction, clamped to 0..=1 (a zero `life_max` reads as empty).
pub fn bar_fill(life: i32, life_max: i32) -> f32 {
    if life_max <= 0 {
        return 0.0;
    }
    (life as f32 / life_max as f32).clamp(0.0, 1.0)
}

/// Phase 2 tints the bar darker red.
fn fill_color(phase: u8) -> Color {
    if phase >= 2 {
        Color::srgb(0.75, 0.1, 0.45)
    } else {
        Color::srgb(0.8, 0.12, 0.1)
    }
}

#[derive(Component)]
pub struct BossBarRoot;
#[derive(Component)]
pub struct BossBarFill;
#[derive(Component)]
pub struct BossBarLabel;
#[derive(Component)]
pub struct BossBarHpLabel;
#[derive(Component)]
pub struct BossTitleCard;
#[derive(Component)]
pub struct BossTitleLabel;

pub fn setup(mut commands: Commands) {
    commands
        .spawn((
            BossBarRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent((100.0 - BAR_WIDTH_PERCENT) / 2.0),
                bottom: Val::Px(BAR_BOTTOM),
                width: Val::Percent(BAR_WIDTH_PERCENT),
                height: Val::Px(BAR_HEIGHT),
                padding: UiRect::all(Val::Px(2.0)),
                border: UiRect::all(Val::Px(super::style::BORDER_W)),
                border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                ..default()
            },
            BackgroundColor(super::style::BAR_BG),
            BorderColor::all(super::style::GOLD_BORDER),
            Visibility::Hidden,
            GlobalZIndex(900),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|p| {
            p.spawn((
                BossBarFill,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BAR)),
                    ..default()
                },
                BackgroundColor(fill_color(1)),
            ));
            p.spawn((
                BossBarLabel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(-24.0),
                    ..default()
                },
                HudText {
                    text: "Eye of Cthulhu".to_string(),
                    death: false,
                },
                super::text::HudTextColor(super::style::GOLD_TEXT),
            ));
            p.spawn((
                BossBarHpLabel,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(0.0),
                    top: Val::Px(-24.0),
                    ..default()
                },
                HudText {
                    text: String::new(),
                    death: false,
                },
            ));
        });

    commands
        .spawn((
            BossTitleCard,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(30.0),
                top: Val::Percent(18.0),
                width: Val::Percent(40.0),
                padding: UiRect::axes(Val::Px(20.0), Val::Px(12.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.02, 0.04, 0.0)),
            Visibility::Hidden,
            GlobalZIndex(910),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|p| {
            p.spawn((
                BossTitleLabel,
                HudText {
                    text: "Eye of Cthulhu".to_string(),
                    death: false,
                },
            ));
        });
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    state: Res<BossState>,
    wof_state: Option<Res<WofBossState>>,
    enemies: Query<&crate::view::enemies::ClientEnemy>,
    mut sfx: Option<ResMut<Messages<crate::audio::SfxEvent>>>,
    time: Option<Res<Time<Real>>>,
    settings: Option<Res<crate::settings::Settings>>,
    mut card: Option<ResMut<BossTitleCardState>>,
    mut root: Query<(&mut Visibility, &mut Node), With<BossBarRoot>>,
    mut fill: Query<(&mut Node, &mut BackgroundColor), (With<BossBarFill>, Without<BossBarRoot>)>,
    mut bar_label: Query<
        &mut HudText,
        (
            With<BossBarLabel>,
            Without<BossTitleLabel>,
            Without<BossBarHpLabel>,
        ),
    >,
    mut hp_label: Query<
        &mut HudText,
        (
            With<BossBarHpLabel>,
            Without<BossBarLabel>,
            Without<BossTitleLabel>,
        ),
    >,
    mut title_label: Query<
        &mut HudText,
        (
            With<BossTitleLabel>,
            Without<BossBarLabel>,
            Without<BossBarHpLabel>,
        ),
    >,
    mut title_card: Query<
        (&mut Visibility, &mut BackgroundColor),
        (
            With<BossTitleCard>,
            Without<BossBarRoot>,
            Without<BossBarFill>,
        ),
    >,
) {
    use terravibe_sim::enemy::{EnemyKind, is_eater_of_worlds};
    let dt = time.as_ref().map_or(1.0 / 60.0, |t| t.delta_secs());
    let reduced_motion = settings.as_ref().is_some_and(|s| s.reduced_motion);
    let mut from_enemies = false;
    let active = if let Some(w) = wof_state.as_ref().and_then(|s| s.0) {
        Some(("Wall of Flesh", w.life, w.life_max, 2u8))
    } else if let Some(b) = state.0 {
        Some(("Eye of Cthulhu", b.life, b.life_max, b.phase))
    } else if let Some(sk) = enemies
        .iter()
        .find(|e| e.kind == EnemyKind::SkeletronHead && e.hp > 0)
    {
        from_enemies = true;
        let max = EnemyKind::SkeletronHead.stats().hp;
        let phase = if sk.hp * 2 <= max { 2 } else { 1 };
        Some(("Skeletron", sk.hp, max, phase))
    } else {
        let eow_hp: i32 = enemies
            .iter()
            .filter(|e| is_eater_of_worlds(e.kind) && e.hp > 0)
            .map(|e| e.hp)
            .sum();
        if eow_hp > 0 {
            from_enemies = true;
            let max = 1800;
            let phase = if eow_hp * 2 <= max { 2 } else { 1 };
            Some(("Eater of Worlds", eow_hp, max, phase))
        } else {
            None
        }
    };
    let boss_alive = active.is_some();

    let mut card_alpha = 0.0;
    let mut bar_offset = 0.0;
    if let Some(ref mut card) = card {
        if boss_alive && !card.was_boss_alive {
            card.active = true;
            card.timer = TITLE_CARD_DURATION_SECS;
            if from_enemies && let Some(ref mut bus) = sfx {
                bus.write(crate::audio::SfxEvent(crate::audio::SfxKind::BossRoar));
            }
        } else if !boss_alive {
            card.active = false;
            card.timer = 0.0;
        }
        card.was_boss_alive = boss_alive;
        if card.active && card.timer > 0.0 {
            card_alpha = title_card_alpha(card.timer, TITLE_CARD_DURATION_SECS);
            bar_offset =
                boss_bar_entrance_offset(card.timer, TITLE_CARD_DURATION_SECS, reduced_motion);
            card.timer = (card.timer - dt).max(0.0);
            if card.timer <= 0.0 {
                card.active = false;
            }
        }
    }

    if let Ok((mut card_vis, mut card_bg)) = title_card.single_mut() {
        if card_alpha > 0.01 {
            *card_vis = Visibility::Inherited;
            card_bg.0 = Color::srgba(0.05, 0.02, 0.04, 0.75 * card_alpha);
        } else {
            *card_vis = Visibility::Hidden;
        }
    }

    let Ok((mut vis, mut root_node)) = root.single_mut() else {
        return;
    };
    let Some((name, life, life_max, phase)) = active else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    root_node.bottom = Val::Px(BAR_BOTTOM + bar_offset);
    if let Ok(mut t) = bar_label.single_mut()
        && t.text != name
    {
        t.text = name.to_string();
    }
    let hp_str = format!("{life} / {life_max}");
    if let Ok(mut t) = hp_label.single_mut()
        && t.text != hp_str
    {
        t.text = hp_str;
    }
    if let Ok(mut t) = title_label.single_mut()
        && t.text != name
    {
        t.text = name.to_string();
    }
    if let Ok((mut node, mut bg)) = fill.single_mut() {
        node.width = Val::Percent(100.0 * bar_fill(life, life_max));
        bg.0 = fill_color(phase);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(life: i32) -> BossInfo {
        BossInfo {
            pos: [0.0; 3],
            vel: [0.0; 3],
            life,
            life_max: 2800,
            phase: 1,
            state: 0,
            spin: 0.0,
        }
    }

    #[test]
    fn bar_fill_clamps_and_scales() {
        assert_eq!(bar_fill(1400, 2800), 0.5);
        assert_eq!(bar_fill(-5, 2800), 0.0);
        assert_eq!(bar_fill(5000, 2800), 1.0);
        assert_eq!(bar_fill(10, 0), 0.0);
    }

    #[test]
    fn boss_state_clears_on_gone() {
        let mut s = BossState::default();
        s.set(info(100));
        assert!(s.0.is_some());
        s.clear();
        assert_eq!(s, BossState(None));
    }

    #[test]
    fn boss_bar_hidden_when_no_boss() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<BossState>()
            .add_systems(Startup, setup)
            .add_systems(Update, update);
        app.update();
        app.update();
        let vis = |app: &mut App| {
            *app.world_mut()
                .query_filtered::<&Visibility, With<BossBarRoot>>()
                .single(app.world())
                .unwrap()
        };
        assert_eq!(vis(&mut app), Visibility::Hidden);
        app.world_mut().resource_mut::<BossState>().set(info(2800));
        app.update();
        assert_eq!(vis(&mut app), Visibility::Inherited);
        app.world_mut().resource_mut::<BossState>().clear();
        app.update();
        assert_eq!(vis(&mut app), Visibility::Hidden);
    }

    #[test]
    fn boss_title_card_triggers_on_spawn_and_fades() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<BossState>()
            .init_resource::<BossTitleCardState>()
            .add_systems(Startup, setup)
            .add_systems(Update, update);
        app.update();

        assert!(!app.world().resource::<BossTitleCardState>().active);

        app.world_mut().resource_mut::<BossState>().set(info(2800));
        app.update();
        let card = app.world().resource::<BossTitleCardState>();
        assert!(card.active);
        assert!(card.timer > 0.0);

        let card_vis = *app
            .world_mut()
            .query_filtered::<&Visibility, With<BossTitleCard>>()
            .single(app.world())
            .unwrap();
        assert_eq!(card_vis, Visibility::Inherited);

        // Once timer reaches zero, alpha fades to 0 and card hides.
        assert_eq!(title_card_alpha(0.0, TITLE_CARD_DURATION_SECS), 0.0);
        assert!(title_card_alpha(TITLE_CARD_DURATION_SECS * 0.5, TITLE_CARD_DURATION_SECS) > 0.9);
    }

    #[test]
    fn wof_boss_bar_displays_wall_of_flesh() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<BossState>()
            .init_resource::<WofBossState>()
            .add_systems(Startup, setup)
            .add_systems(Update, update);
        app.update();

        app.world_mut()
            .resource_mut::<WofBossState>()
            .set(WofBossInfo {
                wall_x: -100.0,
                dir: 1,
                life: 4000,
                life_max: 8000,
            });
        app.update();

        let vis = *app
            .world_mut()
            .query_filtered::<&Visibility, With<BossBarRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(vis, Visibility::Inherited);

        let label = app
            .world_mut()
            .query_filtered::<&HudText, With<BossBarLabel>>()
            .single(app.world())
            .unwrap()
            .text
            .clone();
        assert_eq!(label, "Wall of Flesh");
    }

    #[test]
    fn skeletron_and_eater_of_worlds_show_boss_bar_and_title_card() {
        use crate::view::enemies::ClientEnemy;
        use terravibe_sim::enemy::{EnemyId, EnemyKind};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<BossState>()
            .init_resource::<WofBossState>()
            .init_resource::<BossTitleCardState>()
            .add_message::<crate::audio::SfxEvent>()
            .add_systems(Startup, setup)
            .add_systems(Update, update);
        app.update();

        // Spawn SkeletronHead
        let sk_entity = app
            .world_mut()
            .spawn(ClientEnemy {
                id: EnemyId(10),
                kind: EnemyKind::SkeletronHead,
                hp: 2200,
                moving: false,
                vel: Vec3::ZERO,
                hurt: 0,
                telegraph_t: 0.0,
            })
            .id();
        app.update();

        let vis = *app
            .world_mut()
            .query_filtered::<&Visibility, With<BossBarRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(vis, Visibility::Inherited);
        let label = app
            .world_mut()
            .query_filtered::<&HudText, With<BossBarLabel>>()
            .single(app.world())
            .unwrap()
            .text
            .clone();
        assert_eq!(label, "Skeletron");
        assert!(app.world().resource::<BossTitleCardState>().active);

        app.world_mut().despawn(sk_entity);
        app.update();
        assert!(!app.world().resource::<BossTitleCardState>().active);

        // Spawn Eater of Worlds segments
        app.world_mut().spawn(ClientEnemy {
            id: EnemyId(20),
            kind: EnemyKind::EaterOfWorldsHead,
            hp: 150,
            moving: false,
            vel: Vec3::ZERO,
            hurt: 0,
            telegraph_t: 0.0,
        });
        app.world_mut().spawn(ClientEnemy {
            id: EnemyId(21),
            kind: EnemyKind::EaterOfWorldsBody,
            hp: 150,
            moving: false,
            vel: Vec3::ZERO,
            hurt: 0,
            telegraph_t: 0.0,
        });
        app.update();

        let label = app
            .world_mut()
            .query_filtered::<&HudText, With<BossBarLabel>>()
            .single(app.world())
            .unwrap()
            .text
            .clone();
        assert_eq!(label, "Eater of Worlds");
        assert!(app.world().resource::<BossTitleCardState>().active);
    }
}
