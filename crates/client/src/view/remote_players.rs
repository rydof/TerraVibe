//! Remote multiplayer player 3D avatars, animations, gear, and overhead nametags.

use std::collections::HashMap;

use bevy::prelude::*;
use terravibe_sim::character::Appearance;
use terravibe_sim::item::ItemId;
use terravibe_sim::net::PlayerId;

use super::avatar::{
    AVATAR_SCALE, ItemModels, TURN_RATE, WALK_SPEED, apply_armor_to_rig, resolve_effective_armor,
    spawn_player_rig_parts, update_held_item_on_rig, wrap_pi,
};
use super::held_anim::{HeldAnimKind, HeldAnimState, compute_tp_arm_rot, held_anim_kind};
use crate::art::rig::Rig;
use crate::art::rig_view::{RigAnim, RigPart, animate_rigs};
use crate::net::RemotePlayerMsg;
use crate::session::AppState;

/// Exponential smoothing rate (1/s) for 20 Hz remote player position updates.
const INTERP_RATE: f32 = 15.0;
/// Distance (blocks) above which position snaps instead of interpolating.
const SNAP_DISTANCE: f32 = 8.0;
/// Height (blocks) above feet where the overhead nametag anchors.
const NAMETAG_HEIGHT: f32 = 2.15;
/// Period (seconds) of a full right-arm swing cycle on remote players.
const SWING_PERIOD_SECS: f32 = 0.35;

/// Marker component on each remote player's root 3D avatar entity.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemotePlayerEntity {
    pub id: PlayerId,
}

/// Marker component on the screen-space overhead nametag UI node for a remote player.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemotePlayerNametag {
    pub id: PlayerId,
}

/// Maps connected remote [`PlayerId`]s to their spawned root avatar entities.
#[derive(Resource, Default)]
pub struct RemotePlayerEntities(pub HashMap<PlayerId, Entity>);

/// Per-remote-player replication, interpolation, gear, and rig state.
#[derive(Component)]
pub(crate) struct RemotePlayerState {
    pub(crate) name: String,
    pub(crate) appearance: Appearance,
    pub(crate) rig: Rig,
    #[allow(dead_code)]
    pub(crate) material: Handle<StandardMaterial>,
    pub(crate) cache: HashMap<String, Handle<Mesh>>,
    pub(crate) target_pos: Vec3,
    pub(crate) current_pos: Vec3,
    pub(crate) vel: Vec3,
    pub(crate) target_yaw: f32,
    pub(crate) current_yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) swinging: bool,
    pub(crate) swing_t: f32,
    pub(crate) dead: bool,
    pub(crate) armor: [Option<ItemId>; 3],
    pub(crate) vanity_armor: [Option<ItemId>; 3],
    pub(crate) held_item: Option<ItemId>,
    pub(crate) held_entity: Option<(ItemId, Entity)>,
    pub(crate) armor_dirty: bool,
    pub(crate) held_dirty: bool,
    pub(crate) nametag: Option<Entity>,
}

impl RemotePlayerState {
    /// Effective displayed armor slots (vanity overrides worn armor).
    pub(crate) fn effective_armor(&self) -> [Option<ItemId>; 3] {
        resolve_effective_armor(self.armor, self.vanity_armor)
    }
}

fn despawn_remote_player(commands: &mut Commands, root: Entity, nametag: Option<Entity>) {
    commands.entity(root).despawn();
    if let Some(tag) = nametag {
        commands.entity(tag).despawn();
    }
}

fn handle_remote_player_msgs(
    mut commands: Commands,
    mut msgs: MessageReader<RemotePlayerMsg>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut entities: ResMut<RemotePlayerEntities>,
    mut states: Query<&mut RemotePlayerState>,
) {
    for msg in msgs.read() {
        match msg {
            RemotePlayerMsg::Spawn {
                id,
                name,
                appearance,
                pos,
                yaw,
            } => {
                if let Some(old_root) = entities.0.remove(id) {
                    let old_tag = states.get(old_root).ok().and_then(|s| s.nametag);
                    despawn_remote_player(&mut commands, old_root, old_tag);
                }

                let spawned = spawn_player_rig_parts(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    Some(appearance),
                );
                let start_pos = Vec3::from(*pos);
                let nametag = commands
                    .spawn((
                        Name::new(format!("nametag_{id}")),
                        RemotePlayerNametag { id: *id },
                        Text::new(name.clone()),
                        TextFont {
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
                        Node {
                            position_type: PositionType::Absolute,
                            padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                            ..default()
                        },
                        Visibility::Hidden,
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id();

                commands.entity(spawned.root).insert((
                    Name::new(format!("remote_player_{id}")),
                    RemotePlayerEntity { id: *id },
                    Transform {
                        translation: start_pos,
                        rotation: Quat::from_rotation_y(*yaw),
                        scale: Vec3::splat(AVATAR_SCALE),
                    },
                    RemotePlayerState {
                        name: name.clone(),
                        appearance: appearance.clone(),
                        rig: spawned.rig,
                        material: spawned.material,
                        cache: spawned.cache,
                        target_pos: start_pos,
                        current_pos: start_pos,
                        vel: Vec3::ZERO,
                        target_yaw: *yaw,
                        current_yaw: *yaw,
                        pitch: 0.0,
                        swinging: false,
                        swing_t: 0.0,
                        dead: false,
                        armor: [None; 3],
                        vanity_armor: [None; 3],
                        held_item: None,
                        held_entity: None,
                        armor_dirty: true,
                        held_dirty: false,
                        nametag: Some(nametag),
                    },
                ));
                entities.0.insert(*id, spawned.root);
            }
            RemotePlayerMsg::State {
                id,
                pos,
                vel,
                yaw,
                pitch,
                swinging,
                dead,
            } => {
                let Some(&root) = entities.0.get(id) else {
                    continue;
                };
                let Ok(mut state) = states.get_mut(root) else {
                    continue;
                };
                let next_pos = Vec3::from(*pos);
                if state.current_pos.distance(next_pos) > SNAP_DISTANCE {
                    state.current_pos = next_pos;
                }
                state.target_pos = next_pos;
                state.vel = Vec3::from(*vel);
                state.target_yaw = *yaw;
                state.pitch = *pitch;
                state.swinging = *swinging;
                state.dead = *dead;
            }
            RemotePlayerMsg::Equipment {
                id,
                armor,
                vanity_armor,
                held,
            } => {
                let Some(&root) = entities.0.get(id) else {
                    continue;
                };
                let Ok(mut state) = states.get_mut(root) else {
                    continue;
                };
                state.armor = *armor;
                state.vanity_armor = *vanity_armor;
                state.held_item = *held;
                state.armor_dirty = true;
                state.held_dirty = true;
            }
            RemotePlayerMsg::Despawn { id } => {
                if let Some(root) = entities.0.remove(id) {
                    let tag = states.get(root).ok().and_then(|s| s.nametag);
                    despawn_remote_player(&mut commands, root, tag);
                }
            }
        }
    }
}

fn sync_remote_equipment(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    models: Option<Res<ItemModels>>,
    parts: Query<(Entity, &RigPart)>,
    mut remotes: Query<(Entity, &mut RemotePlayerState)>,
) {
    for (root, mut state) in &mut remotes {
        if state.armor_dirty {
            let effective = state.effective_armor();
            let app = state.appearance.clone();
            let s = &mut *state;
            if apply_armor_to_rig(
                &mut commands,
                root,
                &s.rig,
                &mut s.cache,
                &mut meshes,
                effective,
                Some(&app),
                &parts,
            ) {
                s.armor_dirty = false;
            }
        }
        if state.held_dirty {
            let want = state.held_item;
            if update_held_item_on_rig(
                &mut commands,
                root,
                want,
                &mut state.held_entity,
                models.as_deref(),
                &parts,
            ) {
                state.held_dirty = false;
            }
        }
    }
}

fn drive_remote_players(
    time: Res<Time<Real>>,
    mut pool: Option<ResMut<super::particles::ParticlePool>>,
    settings: Option<Res<crate::settings::Settings>>,
    mut remotes: Query<(
        &mut RemotePlayerState,
        &mut Transform,
        &mut RigAnim,
        &mut Visibility,
    )>,
) {
    let dt = time.delta_secs();
    let pos_k = 1.0 - (-INTERP_RATE * dt).exp();
    let yaw_k = 1.0 - (-TURN_RATE * dt).exp();

    for (mut state, mut tf, mut anim, mut vis) in &mut remotes {
        state.current_pos = state.current_pos.lerp(state.target_pos, pos_k);
        let dyaw = wrap_pi(state.target_yaw - state.current_yaw);
        state.current_yaw = wrap_pi(state.current_yaw + dyaw * yaw_k);

        tf.translation = state.current_pos;
        tf.rotation = Quat::from_rotation_y(state.current_yaw);

        let horiz_speed = Vec2::new(state.vel.x, state.vel.z).length();
        let clip = if state.swinging {
            "swing"
        } else if horiz_speed > WALK_SPEED {
            "walk"
        } else {
            "idle"
        };
        anim.play(clip);

        if state.swinging {
            state.swing_t = (state.swing_t + dt) % SWING_PERIOD_SECS;
        } else {
            state.swing_t = 0.0;
        }

        *vis = if state.dead {
            Visibility::Hidden
        } else {
            if let (Some(ref mut p), Some(fx)) = (
                pool.as_deref_mut(),
                super::avatar::active_armor_set_fx(state.effective_armor()),
            ) {
                let s = settings.as_deref().copied().unwrap_or_default();
                if !s.reduced_motion && ((time.elapsed_secs() * 10.0) as u32).is_multiple_of(3) {
                    super::avatar::spawn_armor_set_particles(
                        p,
                        fx,
                        state.current_pos,
                        state.vel,
                        time.elapsed_secs(),
                        &s,
                    );
                }
            }
            Visibility::Inherited
        };
    }
}

/// Overrides `arm_r` rotation for swinging remote players so swings are clearly
/// visible with any held tool/weapon.
fn animate_remote_swing_arm(
    remotes: Query<(Entity, &RemotePlayerState)>,
    mut parts: Query<(&RigPart, &mut Transform)>,
) {
    for (root, state) in &remotes {
        if !state.swinging || state.dead {
            continue;
        }
        let kind = state
            .held_item
            .map(held_anim_kind)
            .unwrap_or(HeldAnimKind::SwordSlash);
        let phase = (state.swing_t / SWING_PERIOD_SECS).clamp(0.0, 0.999);
        let anim_state = HeldAnimState {
            use_t: Some(phase),
            ..default()
        };
        let rot = compute_tp_arm_rot(kind, &anim_state);
        for (part, mut tf) in &mut parts {
            if part.root == root && part.name == "arm_r" {
                tf.rotation = rot;
                break;
            }
        }
    }
}

fn update_remote_nametags(
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    remotes: Query<&RemotePlayerState>,
    mut tags: Query<(&mut Node, &mut Visibility), With<RemotePlayerNametag>>,
) {
    let cam = camera_q.iter().next();
    for state in &remotes {
        let Some(tag_entity) = state.nametag else {
            continue;
        };
        let Ok((mut node, mut vis)) = tags.get_mut(tag_entity) else {
            continue;
        };
        if state.dead {
            *vis = Visibility::Hidden;
            continue;
        }
        let Some((camera, cam_xf)) = cam else {
            *vis = Visibility::Hidden;
            continue;
        };
        let anchor = state.current_pos + Vec3::Y * NAMETAG_HEIGHT;
        let Ok(screen) = camera.world_to_viewport(cam_xf, anchor) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let half_w = (state.name.len() as f32 * 4.0).max(16.0);
        node.left = Val::Px(screen.x - half_w);
        node.top = Val::Px(screen.y - 12.0);
        *vis = Visibility::Inherited;
    }
}

fn cleanup_remote_players_on_exit(
    mut commands: Commands,
    mut entities: ResMut<RemotePlayerEntities>,
    states: Query<&RemotePlayerState>,
) {
    for (_, root) in entities.0.drain() {
        let tag = states.get(root).ok().and_then(|s| s.nametag);
        despawn_remote_player(&mut commands, root, tag);
    }
}

/// Plugin rendering and animating remote players from [`crate::net::RemotePlayerMsg`].
pub struct RemotePlayersPlugin;

impl Plugin for RemotePlayersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RemotePlayerEntities>()
            .add_systems(OnExit(AppState::InGame), cleanup_remote_players_on_exit)
            .add_systems(
                Update,
                (
                    handle_remote_player_msgs,
                    sync_remote_equipment,
                    drive_remote_players,
                    update_remote_nametags,
                )
                    .chain()
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                animate_remote_swing_arm
                    .after(animate_rigs)
                    .after(drive_remote_players)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use terravibe_sim::item::{GOLD_BROADSWORD, GOLD_HELMET, IRON_HELMET};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::time::TimePlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_state::<AppState>()
            .add_message::<RemotePlayerMsg>()
            .add_plugins(RemotePlayersPlugin);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update();
        app
    }

    #[test]
    fn remote_player_spawn_state_and_despawn() {
        let mut app = test_app();
        let pid: PlayerId = 7;

        // 1. Spawn
        app.world_mut().write_message(RemotePlayerMsg::Spawn {
            id: pid,
            name: "Alice".into(),
            appearance: Appearance::default(),
            pos: [10.0, 20.0, 30.0],
            yaw: 0.5,
        });
        app.update();

        let spawned: Vec<(Entity, RemotePlayerEntity)> = {
            let world = app.world_mut();
            let mut q = world.query::<(Entity, &RemotePlayerEntity)>();
            q.iter(world).map(|(e, r)| (e, *r)).collect()
        };
        assert_eq!(spawned.len(), 1, "expected 1 spawned remote player");
        assert_eq!(spawned[0].1.id, pid);
        let root = spawned[0].0;

        // Verify rig parts and nametag were spawned
        let part_count = {
            let world = app.world_mut();
            let mut q = world.query::<&RigPart>();
            q.iter(world).filter(|p| p.root == root).count()
        };
        assert!(
            part_count >= 6,
            "expected player rig parts, got {part_count}"
        );
        let tag_count = {
            let world = app.world_mut();
            let mut q = world.query::<&RemotePlayerNametag>();
            q.iter(world).filter(|t| t.id == pid).count()
        };
        assert_eq!(tag_count, 1, "expected 1 overhead nametag");

        // 2. State update: moving & swinging -> interpolated transform, swing anim, visible
        app.world_mut().write_message(RemotePlayerMsg::State {
            id: pid,
            pos: [12.0, 20.0, 30.0],
            vel: [0.2, 0.0, 0.0],
            yaw: 1.0,
            pitch: 0.1,
            swinging: true,
            dead: false,
        });
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_millis(50));
        app.update();

        let tf = app.world().get::<Transform>(root).unwrap();
        assert!(
            tf.translation.x > 10.0 && tf.translation.x <= 12.0,
            "expected interpolated x in (10, 12], got {}",
            tf.translation.x
        );
        let anim = app.world().get::<RigAnim>(root).unwrap();
        assert_eq!(anim.name, "swing");
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            Visibility::Inherited
        );

        // State update: walking (not swinging) -> walk anim
        app.world_mut().write_message(RemotePlayerMsg::State {
            id: pid,
            pos: [12.5, 20.0, 30.0],
            vel: [0.15, 0.0, 0.0],
            yaw: 1.0,
            pitch: 0.0,
            swinging: false,
            dead: false,
        });
        app.update();
        assert_eq!(app.world().get::<RigAnim>(root).unwrap().name, "walk");

        // State update: idle
        app.world_mut().write_message(RemotePlayerMsg::State {
            id: pid,
            pos: [12.5, 20.0, 30.0],
            vel: [0.0, 0.0, 0.0],
            yaw: 1.0,
            pitch: 0.0,
            swinging: false,
            dead: false,
        });
        app.update();
        assert_eq!(app.world().get::<RigAnim>(root).unwrap().name, "idle");

        // State update: dead -> hidden
        app.world_mut().write_message(RemotePlayerMsg::State {
            id: pid,
            pos: [12.5, 20.0, 30.0],
            vel: [0.0, 0.0, 0.0],
            yaw: 1.0,
            pitch: 0.0,
            swinging: false,
            dead: true,
        });
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(root).unwrap(),
            Visibility::Hidden
        );

        // 3. Equipment update (vanity preferred over armor, held item set)
        app.world_mut().write_message(RemotePlayerMsg::Equipment {
            id: pid,
            armor: [Some(IRON_HELMET), None, None],
            vanity_armor: [Some(GOLD_HELMET), None, None],
            held: Some(GOLD_BROADSWORD),
        });
        app.update();
        {
            let state = app.world().get::<RemotePlayerState>(root).unwrap();
            assert_eq!(
                state.effective_armor(),
                [Some(GOLD_HELMET), None, None],
                "vanity armor must override worn armor"
            );
            assert_eq!(state.held_item, Some(GOLD_BROADSWORD));
            assert!(
                !state.armor_dirty,
                "armor should have been applied to rig parts"
            );
        }

        // 4. Despawn
        app.world_mut()
            .write_message(RemotePlayerMsg::Despawn { id: pid });
        app.update();

        let remaining = {
            let world = app.world_mut();
            let mut q = world.query::<&RemotePlayerEntity>();
            q.iter(world).count()
        };
        assert_eq!(remaining, 0, "expected remote player despawned");
        let remaining_tags = {
            let world = app.world_mut();
            let mut q = world.query::<&RemotePlayerNametag>();
            q.iter(world).count()
        };
        assert_eq!(remaining_tags, 0, "expected remote nametag despawned");

        // 5. OnExit(AppState::InGame) cleanup
        let pid2: PlayerId = 8;
        app.world_mut().write_message(RemotePlayerMsg::Spawn {
            id: pid2,
            name: "Bob".into(),
            appearance: Appearance::default(),
            pos: [0.0, 10.0, 0.0],
            yaw: 0.0,
        });
        app.update();
        {
            let world = app.world_mut();
            let mut q = world.query::<&RemotePlayerEntity>();
            assert_eq!(q.iter(world).count(), 1);
        }

        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Menu);
        app.update();
        {
            let world = app.world_mut();
            let mut q = world.query::<&RemotePlayerEntity>();
            assert_eq!(
                q.iter(world).count(),
                0,
                "expected remote players cleaned up on OnExit(AppState::InGame)"
            );
            assert!(
                world.resource::<RemotePlayerEntities>().0.is_empty(),
                "expected RemotePlayerEntities map cleared on exit"
            );
        }
    }
}
