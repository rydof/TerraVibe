//! Classic Terraria death modal overlay.

use bevy::prelude::*;
use terravibe_sim::enemy::EnemyKind;
use terravibe_sim::net::ClientMsg;
use terravibe_sim::player::DeathCause;
use terravibe_sim::projectile::ProjKind;

use super::text::HudText;
use crate::net::Net;

#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct DeathModalState {
    pub active: bool,
    pub cause: DeathCause,
    pub coins_lost: u32,
    pub survived_ticks: u32,
    pub respawn_ticks: i32,
    pub fade_alpha: f32,
}

#[derive(Component)]
pub struct DeathModalRoot;

#[derive(Component)]
pub struct DeathCauseLabel;

#[derive(Component)]
pub struct DeathSurvivedLabel;

#[derive(Component)]
pub struct DeathCoinsLabel;

#[derive(Component)]
pub struct DeathCountdownLabel;

#[derive(Component)]
pub struct RespawnButton;

pub fn cause_text(cause: DeathCause) -> String {
    match cause {
        DeathCause::Contact(enemy) => match enemy {
            EnemyKind::Slime => "Slain by Slime.".into(),
            EnemyKind::Zombie => "Slain by Zombie.".into(),
            EnemyKind::DemonEye => "Slain by Demon Eye.".into(),
            EnemyKind::EyeOfCthulhu => "Slain by Eye of Cthulhu.".into(),
            EnemyKind::ServantOfCthulhu => "Slain by Servant of Cthulhu.".into(),
            EnemyKind::EaterOfSouls => "Slain by Eater of Souls.".into(),
            EnemyKind::EaterOfWorldsHead
            | EnemyKind::EaterOfWorldsBody
            | EnemyKind::EaterOfWorldsTail => "Slain by Eater of Worlds.".into(),
            EnemyKind::Skeleton => "Slain by Skeleton.".into(),
            EnemyKind::CaveBat => "Slain by Cave Bat.".into(),
            EnemyKind::GiantWorm => "Slain by Giant Worm.".into(),
            EnemyKind::AngryBones => "Slain by Angry Bones.".into(),
            EnemyKind::DarkCaster => "Slain by Dark Caster.".into(),
            EnemyKind::CursedSkull => "Slain by Cursed Skull.".into(),
            EnemyKind::SkeletronHead | EnemyKind::SkeletronHand => "Slain by Skeletron.".into(),
            EnemyKind::DungeonGuardian => "Slain by Dungeon Guardian.".into(),
            EnemyKind::Hellbat => "Slain by Hellbat.".into(),
            EnemyKind::LavaSlime => "Slain by Lava Slime.".into(),
            EnemyKind::FireImp => "Slain by Fire Imp.".into(),
            EnemyKind::Demon => "Slain by Demon.".into(),
            EnemyKind::VoodooDemon => "Slain by Voodoo Demon.".into(),
            EnemyKind::BoneSerpentHead
            | EnemyKind::BoneSerpentBody
            | EnemyKind::BoneSerpentTail => "Slain by Bone Serpent.".into(),
            EnemyKind::WallOfFleshMouth | EnemyKind::WallOfFleshEye => {
                "Slain by Wall of Flesh.".into()
            }
            EnemyKind::TheHungry => "Slain by The Hungry.".into(),
            EnemyKind::Leech => "Slain by Leech.".into(),
            EnemyKind::Guide => "Slain by Guide.".into(),
            EnemyKind::Merchant => "Slain by Merchant.".into(),
            EnemyKind::ArmsDealer => "Slain by Arms Dealer.".into(),
            EnemyKind::Nurse => "Slain by Nurse.".into(),
            EnemyKind::Demolitionist => "Slain by Demolitionist.".into(),
            EnemyKind::Dryad => "Slain by Dryad.".into(),
            EnemyKind::GoblinTinkerer => "Slain by Goblin Tinkerer.".into(),
            EnemyKind::KingSlime => "Squashed by King Slime.".into(),
            EnemyKind::QueenBee => "Stung to death by Queen Bee.".into(),
            EnemyKind::Bee => "Stung by a Bee.".into(),
            EnemyKind::BloodZombie => "Torn apart by a Blood Zombie.".into(),
            EnemyKind::Drippler => "Drained by a Drippler.".into(),
            EnemyKind::GoblinPeon => "Slain by a Goblin Peon.".into(),
            EnemyKind::GoblinThief => "Robbed by a Goblin Thief.".into(),
            EnemyKind::GoblinWarrior => "Crushed by a Goblin Warrior.".into(),
            EnemyKind::GoblinSorcerer => "Blasted by a Goblin Sorcerer.".into(),
            EnemyKind::GoblinArcher => "Shot by a Goblin Archer.".into(),
            EnemyKind::Antlion => "Chomped by an Antlion.".into(),
            EnemyKind::Vulture => "Pecked by a Vulture.".into(),
            EnemyKind::IceSlime => "Frozen by an Ice Slime.".into(),
            EnemyKind::SnowFlinx => "Mauled by a Snow Flinx.".into(),
            EnemyKind::JungleSlime => "Dissolved by a Jungle Slime.".into(),
            EnemyKind::Hornet => "Stung by a Hornet.".into(),
            EnemyKind::ManEater => "Devoured by a Man Eater.".into(),
            EnemyKind::Harpy => "Shredded by a Harpy.".into(),
            EnemyKind::Shark => "Bitten in half by a Shark.".into(),
            EnemyKind::Crab => "Pinched by a Crab.".into(),
            EnemyKind::Piranha => "Swarmed by a Piranha.".into(),
            EnemyKind::OldMan => "Slain by Old Man.".into(),
            EnemyKind::Bunny | EnemyKind::Squirrel | EnemyKind::Bird | EnemyKind::CaveCritter => {
                "Slain by critter.".into()
            }
        },
        DeathCause::Projectile(proj) => match proj {
            ProjKind::Arrow => "Shot by arrow.".into(),
            ProjKind::FlamingArrow => "Shot by flaming arrow.".into(),
            ProjKind::Bullet => "Shot by bullet.".into(),
            ProjKind::Spark => "Struck by magic spark.".into(),
            ProjKind::MagicMissile => "Struck by magic bolt.".into(),
            ProjKind::WaterStream => "Washed away by Aqua Scepter.".into(),
            ProjKind::Vilethorn => "Impaled by Vilethorn.".into(),
            ProjKind::Boomerang => "Struck by boomerang.".into(),
            ProjKind::Flamarang => "Struck by Flamarang.".into(),
            ProjKind::Spear => "Impaled by spear.".into(),
            ProjKind::Flail => "Crushed by flail.".into(),
            ProjKind::Yoyo => "Struck by Valor.".into(),
            ProjKind::Fireball => "Incinerated by fireball.".into(),
            ProjKind::DemonScythe => "Sliced by a Demon Scythe.".into(),
            ProjKind::Laser => "Vaporized by laser.".into(),
            ProjKind::EyeLaser => "Vaporized by Eye Laser.".into(),
            ProjKind::Grenade => "Blown up by a Grenade.".into(),
            ProjKind::Bomb => "Blown up by a Bomb.".into(),
            ProjKind::Dynamite => "Blown to pieces by Dynamite.".into(),
            ProjKind::GrappleHook => "Struck by Grappling Hook.".into(),
            ProjKind::StarfuryStar => "Struck by a falling star.".into(),
            ProjKind::ThornChakram => "Sliced by a Thorn Chakram.".into(),
            ProjKind::WaterBolt => "Drowned by a Water Bolt.".into(),
            ProjKind::SpaceLaser => "Zapped by a Space Gun.".into(),
            ProjKind::FallenStarShot => "Obliterated by a Star Cannon.".into(),
            ProjKind::FlowerOfFire => "Incinerated by a Flower of Fire.".into(),
            ProjKind::Flamelash => "Scorched by a Flamelash.".into(),
            ProjKind::BeeShot => "Stung by a homing Bee.".into(),
        },
        DeathCause::Fall => "Fell to their death.".into(),
        DeathCause::Lava => "Burned in lava.".into(),
        DeathCause::Drown => "Drowned.".into(),
        DeathCause::Suffocation => "Impaled on spikes.".into(),
        DeathCause::Fire => "Burned to ashes.".into(),
        DeathCause::Unknown => "You were slain...".into(),
    }
}

pub fn format_survived_time(ticks: u32) -> String {
    let total_secs = ticks / 60;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins:02}:{secs:02}")
}

pub fn format_coins_lost(copper: u32) -> String {
    if copper == 0 {
        return "0c".into();
    }
    let p = copper / 1_000_000;
    let g = (copper % 1_000_000) / 10_000;
    let s = (copper % 10_000) / 100;
    let c = copper % 100;
    let mut parts = Vec::new();
    if p > 0 {
        parts.push(format!("{p}p"));
    }
    if g > 0 {
        parts.push(format!("{g}g"));
    }
    if s > 0 {
        parts.push(format!("{s}s"));
    }
    if c > 0 {
        parts.push(format!("{c}c"));
    }
    parts.join(" ")
}

pub fn setup(mut commands: Commands) {
    commands
        .spawn((
            DeathModalRoot,
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            Visibility::Hidden,
            GlobalZIndex(1200),
            DespawnOnExit(crate::session::AppState::InGame),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(36.0), Val::Px(28.0)),
                        border: UiRect::all(Val::Px(3.0)),
                        border_radius: BorderRadius::all(Val::Px(super::style::RADIUS_PANEL)),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(14.0),
                        min_width: Val::Px(360.0),
                        ..default()
                    },
                    BackgroundColor(super::style::PANEL_BG_SOLID),
                    BorderColor::all(super::style::GOLD_BORDER),
                ))
                .with_children(|card| {
                    // Title / cause message
                    card.spawn((
                        DeathCauseLabel,
                        HudText {
                            text: "You were slain...".into(),
                            death: true,
                        },
                        Node::default(),
                    ));

                    // Survived time
                    card.spawn((
                        DeathSurvivedLabel,
                        HudText {
                            text: "".into(),
                            death: false,
                        },
                        Node::default(),
                    ));

                    // Coins lost
                    card.spawn((
                        DeathCoinsLabel,
                        HudText {
                            text: "".into(),
                            death: false,
                        },
                        Node::default(),
                    ));

                    // Respawn button / countdown
                    card.spawn((
                        RespawnButton,
                        Node {
                            min_width: Val::Px(200.0),
                            margin: UiRect::top(Val::Px(6.0)),
                            padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
                            border: UiRect::all(Val::Px(2.0)),
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
                            DeathCountdownLabel,
                            HudText {
                                text: "Respawn Now".into(),
                                death: false,
                            },
                            Node::default(),
                        ));
                    });
                });
        });
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    time: Res<Time>,
    mut state: ResMut<DeathModalState>,
    mut root_q: Query<(&mut Visibility, &mut BackgroundColor), With<DeathModalRoot>>,
    mut cursor_q: Query<&mut bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
    mut cause_q: Query<
        &mut HudText,
        (
            With<DeathCauseLabel>,
            Without<DeathSurvivedLabel>,
            Without<DeathCoinsLabel>,
            Without<DeathCountdownLabel>,
        ),
    >,
    mut surv_q: Query<
        &mut HudText,
        (
            With<DeathSurvivedLabel>,
            Without<DeathCauseLabel>,
            Without<DeathCoinsLabel>,
            Without<DeathCountdownLabel>,
        ),
    >,
    mut coins_q: Query<
        &mut HudText,
        (
            With<DeathCoinsLabel>,
            Without<DeathCauseLabel>,
            Without<DeathSurvivedLabel>,
            Without<DeathCountdownLabel>,
        ),
    >,
    mut count_q: Query<
        &mut HudText,
        (
            With<DeathCountdownLabel>,
            Without<DeathCauseLabel>,
            Without<DeathSurvivedLabel>,
            Without<DeathCoinsLabel>,
        ),
    >,
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
    if let Ok(mut opts) = cursor_q.single_mut() {
        opts.grab_mode = bevy::window::CursorGrabMode::None;
        opts.visible = true;
    }
    state.fade_alpha = (state.fade_alpha + time.delta_secs() * 2.0).min(1.0);
    bg.0 = Color::srgba(0.0, 0.0, 0.0, 0.75 * state.fade_alpha);

    let cause_str = cause_text(state.cause);
    if let Ok(mut t) = cause_q.single_mut() {
        if t.text != cause_str {
            t.text = cause_str;
        }
    }

    let surv_str = format!("Survived: {}", format_survived_time(state.survived_ticks));
    if let Ok(mut t) = surv_q.single_mut() {
        if t.text != surv_str {
            t.text = surv_str;
        }
    }

    let coins_str = if state.coins_lost > 0 {
        format!("Dropped: {}", format_coins_lost(state.coins_lost))
    } else {
        "".into()
    };
    if let Ok(mut t) = coins_q.single_mut() {
        if t.text != coins_str {
            t.text = coins_str;
        }
    }

    let secs = (state.respawn_ticks / 60).max(0);
    let count_str = if secs > 0 {
        format!("Respawn in {secs}s (or Click / Space)")
    } else {
        "Respawn Now (Click / Space)".into()
    };
    if let Ok(mut t) = count_q.single_mut() {
        if t.text != count_str {
            t.text = count_str;
        }
    }
}

pub fn handle_input(
    state: Res<DeathModalState>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    gamepads: Query<&Gamepad>,
    settings: Option<Res<crate::settings::Settings>>,
    net: Option<Res<Net>>,
) {
    if !state.active {
        return;
    }
    let jump_key = settings.as_deref().map_or(KeyCode::Space, |s| {
        s.bindings.key(crate::input_map::GameAction::Jump)
    });
    let key_pressed = keys.is_some_and(|k| {
        k.just_pressed(jump_key)
            || k.just_pressed(KeyCode::Space)
            || k.just_pressed(KeyCode::Enter)
            || k.just_pressed(KeyCode::NumpadEnter)
    });
    let mouse_pressed = mouse.is_some_and(|m| m.just_pressed(MouseButton::Left));
    let gamepad_pressed = gamepads
        .iter()
        .any(|g| g.just_pressed(GamepadButton::South));

    if key_pressed || mouse_pressed || gamepad_pressed {
        if let Some(net) = net.as_ref() {
            let _ = net.send(ClientMsg::Respawn);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cause_text_formatting() {
        assert_eq!(cause_text(DeathCause::Fall), "Fell to their death.");
        assert_eq!(cause_text(DeathCause::Lava), "Burned in lava.");
        assert_eq!(cause_text(DeathCause::Drown), "Drowned.");
        assert_eq!(cause_text(DeathCause::Suffocation), "Impaled on spikes.");
        assert_eq!(cause_text(DeathCause::Fire), "Burned to ashes.");
        assert_eq!(cause_text(DeathCause::Unknown), "You were slain...");
        assert_eq!(
            cause_text(DeathCause::Contact(EnemyKind::Zombie)),
            "Slain by Zombie."
        );
        assert_eq!(
            cause_text(DeathCause::Projectile(ProjKind::Arrow)),
            "Shot by arrow."
        );

        // Verify all EnemyKind and ProjKind variants format with clean display text (no raw Debug enum identifiers)
        for kind in [
            EnemyKind::Slime,
            EnemyKind::Zombie,
            EnemyKind::DemonEye,
            EnemyKind::EyeOfCthulhu,
            EnemyKind::ServantOfCthulhu,
            EnemyKind::EaterOfSouls,
            EnemyKind::EaterOfWorldsHead,
            EnemyKind::EaterOfWorldsBody,
            EnemyKind::EaterOfWorldsTail,
            EnemyKind::Skeleton,
            EnemyKind::CaveBat,
            EnemyKind::GiantWorm,
            EnemyKind::AngryBones,
            EnemyKind::DarkCaster,
            EnemyKind::CursedSkull,
            EnemyKind::SkeletronHead,
            EnemyKind::SkeletronHand,
            EnemyKind::DungeonGuardian,
            EnemyKind::Hellbat,
            EnemyKind::LavaSlime,
            EnemyKind::FireImp,
            EnemyKind::Demon,
            EnemyKind::VoodooDemon,
            EnemyKind::BoneSerpentHead,
            EnemyKind::BoneSerpentBody,
            EnemyKind::BoneSerpentTail,
            EnemyKind::WallOfFleshMouth,
            EnemyKind::WallOfFleshEye,
            EnemyKind::TheHungry,
            EnemyKind::Leech,
            EnemyKind::Guide,
            EnemyKind::Merchant,
            EnemyKind::ArmsDealer,
            EnemyKind::OldMan,
            EnemyKind::Bunny,
            EnemyKind::Squirrel,
            EnemyKind::Bird,
            EnemyKind::CaveCritter,
        ] {
            let txt = cause_text(DeathCause::Contact(kind));
            assert!(
                !txt.contains("WallOfFlesh")
                    && !txt.contains("SkeletronHead")
                    && !txt.contains("BoneSerpentHead"),
                "cause_text for {kind:?} must not use raw enum Debug name: {txt}"
            );
        }

        for pk in [
            ProjKind::Arrow,
            ProjKind::FlamingArrow,
            ProjKind::Spark,
            ProjKind::Boomerang,
            ProjKind::Flamarang,
            ProjKind::Spear,
            ProjKind::Bullet,
            ProjKind::Flail,
            ProjKind::Yoyo,
            ProjKind::Vilethorn,
            ProjKind::MagicMissile,
            ProjKind::WaterStream,
            ProjKind::Fireball,
            ProjKind::Laser,
            ProjKind::EyeLaser,
        ] {
            let txt = cause_text(DeathCause::Projectile(pk));
            assert_ne!(
                txt, "Killed by projectile.",
                "cause_text for {pk:?} must have a specific description"
            );
        }
    }

    #[test]
    fn survived_time_formatting() {
        assert_eq!(format_survived_time(0), "00:00");
        assert_eq!(format_survived_time(60), "00:01");
        assert_eq!(format_survived_time(3600), "01:00");
        assert_eq!(format_survived_time(3660), "01:01");
        assert_eq!(format_survived_time(7500), "02:05");
    }

    #[test]
    fn coins_lost_formatting() {
        assert_eq!(format_coins_lost(0), "0c");
        assert_eq!(format_coins_lost(5), "5c");
        assert_eq!(format_coins_lost(105), "1s 5c");
        assert_eq!(format_coins_lost(10_000), "1g");
        assert_eq!(format_coins_lost(10_203), "1g 2s 3c");
        assert_eq!(format_coins_lost(1_000_000), "1p");
        assert_eq!(format_coins_lost(2_040_506), "2p 4g 5s 6c");
    }

    #[test]
    fn death_modal_setup_and_update() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<DeathModalState>()
            .add_systems(Startup, setup)
            .add_systems(Update, update);

        app.update();

        // Initially inactive and hidden
        let vis = *app
            .world_mut()
            .query_filtered::<&Visibility, With<DeathModalRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(vis, Visibility::Hidden);

        // Activate death modal
        {
            let mut state = app.world_mut().resource_mut::<DeathModalState>();
            state.active = true;
            state.cause = DeathCause::Lava;
            state.survived_ticks = 3600;
            state.coins_lost = 10_000;
            state.respawn_ticks = 300;
        }

        app.update();

        // Now visible
        let vis = *app
            .world_mut()
            .query_filtered::<&Visibility, With<DeathModalRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(vis, Visibility::Inherited);

        // Cause text updated
        let cause_text = app
            .world_mut()
            .query_filtered::<&HudText, With<DeathCauseLabel>>()
            .single(app.world())
            .unwrap()
            .text
            .clone();
        assert_eq!(cause_text, "Burned in lava.");

        // Survived text updated
        let surv_text = app
            .world_mut()
            .query_filtered::<&HudText, With<DeathSurvivedLabel>>()
            .single(app.world())
            .unwrap()
            .text
            .clone();
        assert_eq!(surv_text, "Survived: 01:00");

        // Coins dropped updated
        let coins_text = app
            .world_mut()
            .query_filtered::<&HudText, With<DeathCoinsLabel>>()
            .single(app.world())
            .unwrap()
            .text
            .clone();
        assert_eq!(coins_text, "Dropped: 1g");
    }

    #[test]
    fn death_modal_input_sends_respawn() {
        use std::sync::{Arc, Mutex};
        use terravibe_sim::net::{ClientTransport, Disconnected, ServerMsg};

        struct Recorder(Arc<Mutex<Vec<ClientMsg>>>);
        impl ClientTransport for Recorder {
            fn send(&self, m: ClientMsg) -> Result<(), Disconnected> {
                self.0.lock().unwrap().push(m);
                Ok(())
            }
            fn try_recv(&self) -> Result<Option<ServerMsg>, Disconnected> {
                Ok(None)
            }
        }

        let sent = Arc::new(Mutex::new(Vec::new()));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<DeathModalState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .insert_resource(Net::new(Recorder(sent.clone())))
            .add_systems(Update, handle_input);

        // When inactive, pressing space sends nothing
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();
        assert!(sent.lock().unwrap().is_empty());

        // When active, space sends Respawn
        app.world_mut().resource_mut::<DeathModalState>().active = true;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::Space);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear_just_pressed(KeyCode::Space);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();
        let msgs = sent.lock().unwrap().clone();
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], ClientMsg::Respawn));
    }
}
