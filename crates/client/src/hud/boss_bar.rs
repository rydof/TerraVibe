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
const BAR_WIDTH_PERCENT: f32 = 42.0;
const BAR_HEIGHT: f32 = 28.0;
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
pub struct BossBarGhostFill;
#[derive(Component)]
pub struct BossBarEmblemImage;
#[derive(Component)]
pub struct BossBarLabel;
#[derive(Component)]
pub struct BossBarHpLabel;
#[derive(Component)]
pub struct BossTitleCard;
#[derive(Component)]
pub struct BossTitleLabel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveBoss {
    pub name: &'static str,
    pub life: i32,
    pub life_max: i32,
    pub phase: u8,
    pub emblem: crate::art::ui_icons::BossEmblem,
    pub from_enemies: bool,
}

/// Resolves the currently active boss across WOF, Eye of Cthulhu, King Slime, Queen Bee,
/// Skeletron, and Eater of Worlds.
pub fn active_boss_info<'a>(
    wof_state: Option<&WofBossInfo>,
    boss_state: Option<&BossInfo>,
    enemies: impl Iterator<Item = &'a crate::view::enemies::ClientEnemy>,
) -> Option<ActiveBoss> {
    use crate::art::ui_icons::BossEmblem;
    use terravibe_sim::enemy::{EnemyKind, is_eater_of_worlds};

    if let Some(w) = wof_state {
        return Some(ActiveBoss {
            name: "Wall of Flesh",
            life: w.life,
            life_max: w.life_max,
            phase: 2,
            emblem: BossEmblem::WallOfFlesh,
            from_enemies: false,
        });
    }
    if let Some(b) = boss_state {
        return Some(ActiveBoss {
            name: "Eye of Cthulhu",
            life: b.life,
            life_max: b.life_max,
            phase: b.phase,
            emblem: BossEmblem::EyeOfCthulhu,
            from_enemies: false,
        });
    }

    let mut eow_hp = 0i32;
    let mut skeletron_hp = None;
    let mut king_slime_hp = None;
    let mut queen_bee_hp = None;

    for e in enemies {
        if e.hp <= 0 {
            continue;
        }
        match e.kind {
            EnemyKind::KingSlime => king_slime_hp = Some(e.hp),
            EnemyKind::QueenBee => queen_bee_hp = Some(e.hp),
            EnemyKind::SkeletronHead => skeletron_hp = Some(e.hp),
            k if is_eater_of_worlds(k) => eow_hp += e.hp,
            _ => {}
        }
    }

    if let Some(hp) = king_slime_hp {
        let max = EnemyKind::KingSlime.stats().hp;
        let phase = if hp * 2 <= max { 2 } else { 1 };
        return Some(ActiveBoss {
            name: "King Slime",
            life: hp,
            life_max: max,
            phase,
            emblem: BossEmblem::KingSlime,
            from_enemies: true,
        });
    }
    if let Some(hp) = queen_bee_hp {
        let max = EnemyKind::QueenBee.stats().hp;
        let phase = if hp * 2 <= max { 2 } else { 1 };
        return Some(ActiveBoss {
            name: "Queen Bee",
            life: hp,
            life_max: max,
            phase,
            emblem: BossEmblem::QueenBee,
            from_enemies: true,
        });
    }
    if let Some(hp) = skeletron_hp {
        let max = EnemyKind::SkeletronHead.stats().hp;
        let phase = if hp * 2 <= max { 2 } else { 1 };
        return Some(ActiveBoss {
            name: "Skeletron",
            life: hp,
            life_max: max,
            phase,
            emblem: BossEmblem::Skeletron,
            from_enemies: true,
        });
    }
    if eow_hp > 0 {
        let max = 1800;
        let phase = if eow_hp * 2 <= max { 2 } else { 1 };
        return Some(ActiveBoss {
            name: "Eater of Worlds",
            life: eow_hp,
            life_max: max,
            phase,
            emblem: BossEmblem::EaterOfWorlds,
            from_enemies: true,
        });
    }

    None
}

#[derive(Default)]
pub struct BossGhostState {
    frac: f32,
    last_emblem: Option<crate::art::ui_icons::BossEmblem>,
    emblem_handles: Vec<(crate::art::ui_icons::BossEmblem, Handle<Image>)>,
}

pub fn setup(mut commands: Commands) {
    commands
        .spawn((
            BossBarRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent((100.0 - BAR_WIDTH_PERCENT) / 2.0),
                bottom: Val::Px(BAR_BOTTOM),
                width: Val::Percent(BAR_WIDTH_PERCENT),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            Visibility::Hidden,
            GlobalZIndex(900),
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|root| {
            // Header row: Boss name on left, HP / Max (XX%) on right
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|hdr| {
                hdr.spawn((
                    BossBarLabel,
                    HudText {
                        text: "Eye of Cthulhu".to_string(),
                        death: false,
                    },
                    super::text::HudTextColor(super::style::GOLD_TEXT),
                ));
                hdr.spawn((
                    BossBarHpLabel,
                    HudText {
                        text: String::new(),
                        death: false,
                    },
                ));
            });

            // Framed horizontal flex bar: 36x36 emblem badge + recessed track with ghost & fill
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(BAR_HEIGHT),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(3.0)),
                    border: UiRect::all(Val::Px(super::style::BORDER_W)),
                    border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                    ..default()
                },
                BackgroundColor(super::style::PANEL_BG_SOLID),
                BorderColor::all(super::style::GOLD_BORDER),
            ))
            .with_children(|bar_row| {
                bar_row
                    .spawn((
                        Node {
                            width: Val::Px(36.0),
                            height: Val::Px(36.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(super::style::BAR_BG),
                        BorderColor::all(super::style::GOLD_BORDER),
                    ))
                    .with_children(|badge| {
                        badge.spawn((
                            BossBarEmblemImage,
                            ImageNode::default(),
                            Node {
                                width: Val::Px(32.0),
                                height: Val::Px(32.0),
                                ..default()
                            },
                        ));
                    });

                bar_row
                    .spawn((
                        Node {
                            flex_grow: 1.0,
                            height: Val::Percent(100.0),
                            border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BAR)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(super::style::BAR_BG),
                    ))
                    .with_children(|track| {
                        track.spawn((
                            BossBarGhostFill,
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(0.0),
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BAR)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.95, 0.72, 0.25, 0.65)),
                        ));
                        track.spawn((
                            BossBarFill,
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(0.0),
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BAR)),
                                ..default()
                            },
                            BackgroundColor(fill_color(1)),
                        ));
                    });
            });
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
                border: UiRect::all(Val::Px(super::style::BORDER_W)),
                border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_BTN)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(super::style::PANEL_BG_SOLID),
            BorderColor::all(super::style::GOLD_BORDER),
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
                super::text::HudTextColor(super::style::GOLD_TEXT),
            ));
        });
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct BossLabels<'w, 's> {
    bar_label: Query<
        'w,
        's,
        &'static mut HudText,
        (
            With<BossBarLabel>,
            Without<BossTitleLabel>,
            Without<BossBarHpLabel>,
        ),
    >,
    hp_label: Query<
        'w,
        's,
        &'static mut HudText,
        (
            With<BossBarHpLabel>,
            Without<BossBarLabel>,
            Without<BossTitleLabel>,
        ),
    >,
    title_label: Query<
        'w,
        's,
        &'static mut HudText,
        (
            With<BossTitleLabel>,
            Without<BossBarLabel>,
            Without<BossBarHpLabel>,
        ),
    >,
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
    mut ghost: Local<BossGhostState>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut root: Query<(&mut Visibility, &mut Node), With<BossBarRoot>>,
    mut fill: Query<
        (&mut Node, &mut BackgroundColor),
        (
            With<BossBarFill>,
            Without<BossBarRoot>,
            Without<BossBarGhostFill>,
            Without<BossTitleCard>,
        ),
    >,
    mut ghost_fill: Query<
        &mut Node,
        (
            With<BossBarGhostFill>,
            Without<BossBarRoot>,
            Without<BossBarFill>,
        ),
    >,
    mut emblem_img: Query<&mut ImageNode, With<BossBarEmblemImage>>,
    mut labels: BossLabels,
    mut title_card: Query<
        (&mut Visibility, &mut BackgroundColor, &mut BorderColor),
        (
            With<BossTitleCard>,
            Without<BossBarRoot>,
            Without<BossBarFill>,
        ),
    >,
) {
    let dt = time.as_ref().map_or(1.0 / 60.0, |t| t.delta_secs());
    let reduced_motion = settings.as_ref().is_some_and(|s| s.reduced_motion);
    let active = active_boss_info(
        wof_state.as_ref().and_then(|s| s.0.as_ref()),
        state.0.as_ref(),
        enemies.iter(),
    );
    let boss_alive = active.is_some();
    let from_enemies = active.is_some_and(|a| a.from_enemies);

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

    if let Ok((mut card_vis, mut card_bg, mut card_border)) = title_card.single_mut() {
        if card_alpha > 0.01 {
            *card_vis = Visibility::Inherited;
            let bg = super::style::PANEL_BG_SOLID.to_srgba();
            card_bg.0 = Color::srgba(bg.red, bg.green, bg.blue, bg.alpha * card_alpha);
            let bd = super::style::GOLD_BORDER.to_srgba();
            *card_border = BorderColor::all(Color::srgba(
                bd.red,
                bd.green,
                bd.blue,
                bd.alpha * card_alpha,
            ));
        } else {
            *card_vis = Visibility::Hidden;
        }
    }

    let Ok((mut vis, mut root_node)) = root.single_mut() else {
        return;
    };
    let Some(boss) = active else {
        *vis = Visibility::Hidden;
        ghost.last_emblem = None;
        ghost.frac = 0.0;
        return;
    };
    *vis = Visibility::Inherited;
    root_node.bottom = Val::Px(BAR_BOTTOM + bar_offset);

    if let Ok(mut t) = labels.bar_label.single_mut()
        && t.text != boss.name
    {
        t.text = boss.name.to_string();
    }
    let frac = bar_fill(boss.life, boss.life_max);
    let pct = (frac * 100.0).round() as i32;
    let phase_suffix = if boss.phase >= 2 { " · P2" } else { "" };
    let hp_str = format!("{} / {} ({pct}%){phase_suffix}", boss.life, boss.life_max);
    if let Ok(mut t) = labels.hp_label.single_mut()
        && t.text != hp_str
    {
        t.text = hp_str;
    }
    if let Ok(mut t) = labels.title_label.single_mut()
        && t.text != boss.name
    {
        t.text = boss.name.to_string();
    }

    if ghost.last_emblem != Some(boss.emblem) || frac > ghost.frac {
        ghost.frac = frac;
    } else {
        ghost.frac = (ghost.frac - 0.35 * dt).max(frac);
    }
    ghost.last_emblem = Some(boss.emblem);

    if let Some(ref mut img_assets) = images
        && let Ok(mut img_node) = emblem_img.single_mut()
    {
        let handle =
            if let Some((_, h)) = ghost.emblem_handles.iter().find(|(e, _)| *e == boss.emblem) {
                h.clone()
            } else {
                let rgba = crate::art::ui_icons::boss_emblem_icon(boss.emblem);
                let bevy_img = Image::new(
                    bevy::render::render_resource::Extent3d {
                        width: rgba.width,
                        height: rgba.height,
                        depth_or_array_layers: 1,
                    },
                    bevy::render::render_resource::TextureDimension::D2,
                    rgba.pixels.into_iter().flatten().collect(),
                    bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                    default(),
                );
                let h = img_assets.add(bevy_img);
                ghost.emblem_handles.push((boss.emblem, h.clone()));
                h
            };
        if img_node.image != handle {
            img_node.image = handle;
        }
    }

    if let Ok((mut node, mut bg)) = fill.single_mut() {
        node.width = Val::Percent(100.0 * frac);
        bg.0 = fill_color(boss.phase);
    }
    if let Ok(mut ghost_node) = ghost_fill.single_mut() {
        ghost_node.width = Val::Percent(100.0 * ghost.frac);
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

    #[test]
    fn boss_bar_detects_king_slime_and_queen_bee_and_zero_max() {
        use crate::art::ui_icons::BossEmblem;
        use crate::view::enemies::ClientEnemy;
        use terravibe_sim::enemy::{EnemyId, EnemyKind};

        assert_eq!(bar_fill(0, 0), 0.0);

        let king_slime = ClientEnemy {
            id: EnemyId(30),
            kind: EnemyKind::KingSlime,
            hp: 1500,
            moving: false,
            vel: Vec3::ZERO,
            hurt: 0,
            telegraph_t: 0.0,
        };
        let queen_bee = ClientEnemy {
            id: EnemyId(31),
            kind: EnemyKind::QueenBee,
            hp: 1700,
            moving: false,
            vel: Vec3::ZERO,
            hurt: 0,
            telegraph_t: 0.0,
        };

        let ks_info = active_boss_info(None, None, std::iter::once(&king_slime))
            .expect("King Slime should be detected");
        assert_eq!(ks_info.name, "King Slime");
        assert_eq!(ks_info.emblem, BossEmblem::KingSlime);
        assert_eq!(ks_info.life, 1500);

        let qb_info = active_boss_info(None, None, std::iter::once(&queen_bee))
            .expect("Queen Bee should be detected");
        assert_eq!(qb_info.name, "Queen Bee");
        assert_eq!(qb_info.emblem, BossEmblem::QueenBee);
        assert_eq!(qb_info.life, 1700);
    }
}
