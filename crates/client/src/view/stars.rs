//! Falling star client visuals: animated streaks falling across the night sky.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use terravibe_sim::star::Star;

use crate::session::AppState;

/// Streamed falling star list from `ServerMsg::Stars`.
#[derive(Message, Clone, Debug)]
pub struct StarsMsg(pub Vec<Star>);

#[derive(Resource, Default)]
pub struct StarEntities(pub HashMap<u32, Entity>);

#[derive(Resource)]
struct StarVisuals {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

pub struct StarsPlugin;

impl Plugin for StarsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StarEntities>()
            .add_message::<StarsMsg>()
            .add_systems(Startup, setup)
            .add_systems(
                OnExit(AppState::InGame),
                |mut ents: ResMut<StarEntities>| {
                    ents.0.clear();
                },
            )
            .add_systems(Update, apply.run_if(in_state(AppState::InGame)));
    }
}

const MODEL_AXIS: Vec3 = Vec3::Y;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cuboid::new(0.15, 1.2, 0.15));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.6),
        emissive: LinearRgba::rgb(4.0, 4.0, 1.5).into(),
        perceptual_roughness: 0.2,
        ..default()
    });
    commands.insert_resource(StarVisuals { mesh, material });
}

fn apply(
    mut commands: Commands,
    mut msgs: MessageReader<StarsMsg>,
    mut ents: ResMut<StarEntities>,
    vis: Option<Res<StarVisuals>>,
    mut tf: Query<&mut Transform>,
) {
    let (Some(vis), Some(StarsMsg(list))) = (vis, msgs.read().last()) else {
        return;
    };
    let live: HashSet<u32> = list.iter().map(|s| s.id).collect();
    ents.0.retain(|id, e| {
        let keep = live.contains(id);
        if !keep {
            commands.entity(*e).despawn();
        }
        keep
    });
    for s in list {
        let pos = Vec3::from(s.pos);
        let vel = Vec3::from(s.vel);
        let at = |t: &mut Transform| {
            t.translation = pos;
            if vel.length_squared() > 1e-10 {
                t.rotation = Quat::from_rotation_arc(MODEL_AXIS.normalize(), vel.normalize());
            }
        };
        if let Some(mut t) = ents.0.get(&s.id).and_then(|e| tf.get_mut(*e).ok()) {
            at(&mut t);
            continue;
        }
        let mut t = Transform::from_translation(pos);
        at(&mut t);
        let e = commands
            .spawn((
                Mesh3d(vis.mesh.clone()),
                MeshMaterial3d(vis.material.clone()),
                t,
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        ents.0.insert(s.id, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stars_plugin_builds_and_mirrors_list() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_state::<AppState>()
            .add_plugins(StarsPlugin);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        let star = |id| Star {
            id,
            pos: [0.0, 50.0, 0.0],
            vel: [1.0, -8.0, 0.0],
        };
        app.update(); // Startup
        app.world_mut()
            .write_message(StarsMsg(vec![star(1), star(2)]));
        app.update();
        assert_eq!(app.world().resource::<StarEntities>().0.len(), 2);
        app.world_mut().write_message(StarsMsg(vec![star(2)]));
        app.update();
        assert_eq!(app.world().resource::<StarEntities>().0.len(), 1);
        app.world_mut().write_message(StarsMsg(vec![]));
        app.update();
        assert!(app.world().resource::<StarEntities>().0.is_empty());
    }
}
