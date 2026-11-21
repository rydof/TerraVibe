//! Spawns a [`Rig`] as an entity tree and animates it.
//!
//! Positions are in blocks: a rig's voxel offsets and pivots are divided by
//! [`VOXELS_PER_UNIT`]. Parts are named children of the root.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::rig::{self, Rig};
use super::vox;
use super::voxmesh::{VOXELS_PER_UNIT, mesh};

/// Plays `name` on a rig root; `t` is seconds into the animation.
#[derive(Component, Clone, Debug)]
pub struct RigAnim {
    pub name: String,
    pub t: f32,
}

impl RigAnim {
    /// Switches animation, restarting it; the same name keeps its progress.
    pub fn play(&mut self, name: &str) {
        if self.name != name {
            self.name = name.into();
            self.t = 0.0;
        }
    }
}

/// The rig definition on its root.
#[derive(Component)]
pub struct RigRoot(pub Rig);

/// A part entity: its rig root and rest position (blocks) relative to its parent.
#[derive(Component)]
pub struct RigPart {
    pub root: Entity,
    pub name: String,
    pub rest: Vec3,
}

/// Meshes the `.vox` at `path` about `pivot` (voxels, game axes) into `meshes`.
/// `None` (with a warning) when the file is missing or empty.
pub fn load_mesh(meshes: &mut Assets<Mesh>, path: &Path, pivot: [f32; 3]) -> Option<Handle<Mesh>> {
    let m = match vox::try_load(path) {
        Ok(m) => m,
        Err(e) => {
            warn!("{e}");
            return None;
        }
    };
    crate::view::to_bevy_mesh(mesh(&m, pivot)).map(|m| meshes.add(m))
}

/// Part meshes already built, by `.vox` path and pivot, shared by rigs that
/// are spawned repeatedly (enemies).
#[derive(Default)]
pub struct MeshCache(HashMap<(PathBuf, [i32; 3]), Option<Handle<Mesh>>>);

impl MeshCache {
    /// The mesh for `path` about `pivot`, built on first use only.
    fn get_or_load(
        &mut self,
        meshes: &mut Assets<Mesh>,
        path: PathBuf,
        pivot: [f32; 3],
    ) -> Option<Handle<Mesh>> {
        let key = (path, pivot.map(|v| (v * 16.0) as i32));
        self.0
            .entry(key)
            .or_insert_with_key(|k| load_mesh(meshes, &k.0, pivot))
            .clone()
    }
}

/// Spawns the rig whose parts and `.vox` files live in `dir`. Returns the root
/// (at the origin, rig units of 1 block); the parts are its named children.
/// Parts must list their parent before them.
pub fn spawn_rig(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rig: &Rig,
    dir: &Path,
) -> Entity {
    spawn_rig_with(
        commands,
        meshes,
        materials,
        rig,
        dir,
        &mut MeshCache::default(),
    )
    .0
}

/// [`spawn_rig`] reusing `cache` for the part meshes; also returns the rig's
/// own material, so callers can tint or blend it.
pub fn spawn_rig_with(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    rig: &Rig,
    dir: &Path,
    cache: &mut MeshCache,
) -> (Entity, Handle<StandardMaterial>) {
    let material = materials.add(StandardMaterial {
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });
    let root = commands
        .spawn((
            Name::new("rig"),
            Transform::default(),
            Visibility::default(),
            RigAnim {
                name: "idle".into(),
                t: 0.0,
            },
            RigRoot(rig.clone()),
        ))
        .id();
    let mut spawned: Vec<(String, Entity)> = Vec::new();
    for p in &rig.parts {
        let rest = Vec3::from(p.offset) / VOXELS_PER_UNIT;
        let mut e = commands.spawn((
            Name::new(p.name.clone()),
            Transform::from_translation(rest),
            Visibility::default(),
            RigPart {
                root,
                name: p.name.clone(),
                rest,
            },
        ));
        if let Some(h) = cache.get_or_load(meshes, dir.join(&p.file), p.pivot) {
            e.insert((Mesh3d(h), MeshMaterial3d(material.clone())));
        }
        let id = e.id();
        let parent = p
            .parent
            .as_ref()
            .and_then(|n| spawned.iter().find(|(m, _)| m == n))
            .map_or(root, |(_, e)| *e);
        commands.entity(parent).add_child(id);
        spawned.push((p.name.clone(), id));
    }
    (root, material)
}

/// Swaps the mesh of a part (armor).
pub fn set_part_model(commands: &mut Commands, part: Entity, mesh: Handle<Mesh>) {
    commands.entity(part).insert(Mesh3d(mesh));
}

/// Advances every [`RigAnim`] and poses the parts from [`rig::sample`].
pub fn animate_rigs(
    time: Res<Time>,
    mut roots: Query<(&mut RigAnim, &RigRoot)>,
    mut parts: Query<(&RigPart, &mut Transform)>,
) {
    for (mut a, r) in &mut roots {
        let Some(anim) = r.0.anims.get(&a.name) else {
            continue;
        };
        a.t += time.delta_secs();
        if anim.looping {
            if anim.len > 0.0 {
                a.t %= anim.len;
            }
        } else {
            a.t = a.t.min(anim.len);
        }
    }
    for (part, mut tf) in &mut parts {
        let Ok((a, r)) = roots.get(part.root) else {
            continue;
        };
        let Some(anim) = r.0.anims.get(&a.name) else {
            continue;
        };
        let (q, off) = rig::sample(anim, &part.name, a.t);
        tf.rotation = q;
        tf.translation = part.rest + off / VOXELS_PER_UNIT;
        tf.scale = rig::sample_scale(anim, &part.name, a.t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hit_for_same_path_and_pivot() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/models/slime/body.vox");
        assert!(path.exists(), "{path:?}");
        let mut meshes = Assets::<Mesh>::default();
        let mut cache = MeshCache::default();
        let a = cache
            .get_or_load(&mut meshes, path.clone(), [1.0, 2.0, 3.0])
            .unwrap();
        let b = cache
            .get_or_load(&mut meshes, path.clone(), [1.0, 2.0, 3.0])
            .unwrap();
        assert_eq!(a, b);
        assert_eq!((meshes.len(), cache.0.len()), (1, 1));
        let c = cache
            .get_or_load(&mut meshes, path, [0.0, 0.0, 0.0])
            .unwrap();
        assert_ne!(a, c);
        assert_eq!((meshes.len(), cache.0.len()), (2, 2));
    }
}
