//! Server-streamed arrows: the arrow item model per projectile, oriented along
//! its velocity. The server sends the full live list every tick (and one empty list
//! when the last arrow is gone), so the view mirrors it by id.

use std::collections::HashMap;

use bevy::prelude::*;
use terravibe_sim::item::{
    BALL_O_HURT, BOMB, DYNAMITE, ENCHANTED_BOOMERANG, FLAMARANG, GRAPPLING_HOOK, GRENADE,
    MAGIC_MISSILE, MUSKET_BALL, SPEAR, VALOR, VILETHORN, WOODEN_ARROW,
};
use terravibe_sim::projectile::{ProjKind, Projectile};

use crate::art::rig_view::load_mesh;
use crate::session::AppState;
use crate::settings::Settings;
use crate::view::particles::{HitMaterial, ParticlePool, particle_color};

/// The arrow model's shaft runs from its tail (0, 0) to the tip (10, 10) in the
/// sprite plane.
const MODEL_AXIS: Vec3 = Vec3::new(1.0, 1.0, 0.0);
/// Arrow model size on screen: a quarter of its voxel size.
const ARROW_SCALE: f32 = 0.3;

/// Server-to-view projectile list, written by `net::pump`.
#[derive(Message, Clone, Debug)]
pub struct ProjectilesMsg(pub Vec<Projectile>);

#[derive(Resource, Default)]
struct ProjectileEntities(HashMap<u32, Entity>);

#[derive(Resource)]
struct ArrowVisuals {
    mesh: Handle<Mesh>,
    wood: Handle<StandardMaterial>,
    fire: Handle<StandardMaterial>,
    spark_mesh: Handle<Mesh>,
    spark: Handle<StandardMaterial>,
    laser_mesh: Handle<Mesh>,
    laser: Handle<StandardMaterial>,
    eye_laser: Handle<StandardMaterial>,
    boomerang_mesh: Handle<Mesh>,
    flamarang_mesh: Handle<Mesh>,
    spear_mesh: Handle<Mesh>,
    bullet_mesh: Handle<Mesh>,
    bullet: Handle<StandardMaterial>,
    flail_mesh: Handle<Mesh>,
    flail: Handle<StandardMaterial>,
    yoyo_mesh: Handle<Mesh>,
    yoyo: Handle<StandardMaterial>,
    vilethorn_mesh: Handle<Mesh>,
    vilethorn: Handle<StandardMaterial>,
    magic_missile_mesh: Handle<Mesh>,
    magic_missile: Handle<StandardMaterial>,
    water_stream_mesh: Handle<Mesh>,
    water_stream: Handle<StandardMaterial>,
    bomb_mesh: Handle<Mesh>,
    dynamite_mesh: Handle<Mesh>,
    grenade_mesh: Handle<Mesh>,
    grapple_mesh: Handle<Mesh>,
    star_mesh: Handle<Mesh>,
    star_mat: Handle<StandardMaterial>,
    explosive_mat: Handle<StandardMaterial>,
}

pub struct ProjectilesPlugin;

impl Plugin for ProjectilesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProjectileEntities>()
            .add_message::<ProjectilesMsg>()
            .add_systems(Startup, setup)
            .add_systems(
                OnExit(AppState::InGame),
                |mut ents: ResMut<ProjectileEntities>| {
                    ents.0.clear();
                },
            )
            .add_systems(Update, apply.run_if(in_state(AppState::InGame)));
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = crate::art::item_model(WOODEN_ARROW)
        .and_then(|p| load_mesh(&mut meshes, &p, [5.5, 5.5, 1.0]))
        .unwrap_or_else(|| meshes.add(Cuboid::new(0.05, 0.05, 0.45)));
    let mut mat = |c: Color, emissive: LinearRgba| {
        materials.add(StandardMaterial {
            base_color: c,
            emissive: emissive.into(),
            perceptual_roughness: 1.0,
            ..default()
        })
    };
    let wood = mat(Color::WHITE, LinearRgba::NONE);
    let fire = mat(Color::srgb(1.0, 0.65, 0.3), LinearRgba::rgb(1.2, 0.35, 0.0));
    let spark = mat(Color::srgb(1.0, 0.7, 0.2), LinearRgba::rgb(3.0, 1.2, 0.3));
    let spark_mesh = meshes.add(Cuboid::new(0.08, 0.08, 0.08));
    let laser_mesh = meshes.add(Cuboid::new(0.14, 0.14, 0.9));
    let laser = mat(Color::srgb(0.85, 0.25, 1.0), LinearRgba::rgb(2.6, 0.4, 3.2));
    let eye_laser = mat(Color::srgb(1.0, 0.18, 0.22), LinearRgba::rgb(3.4, 0.3, 0.3));
    let boomerang_mesh = crate::art::item_model(ENCHANTED_BOOMERANG)
        .and_then(|p| load_mesh(&mut meshes, &p, [8.0, 8.0, 1.0]))
        .unwrap_or_else(|| mesh.clone());
    let flamarang_mesh = crate::art::item_model(FLAMARANG)
        .and_then(|p| load_mesh(&mut meshes, &p, [8.0, 8.0, 1.0]))
        .unwrap_or_else(|| boomerang_mesh.clone());
    let spear_mesh = crate::art::item_model(SPEAR)
        .and_then(|p| load_mesh(&mut meshes, &p, [11.0, 11.0, 1.0]))
        .unwrap_or_else(|| mesh.clone());
    let bullet_mesh = crate::art::item_model(MUSKET_BALL)
        .and_then(|p| load_mesh(&mut meshes, &p, [3.0, 3.0, 1.0]))
        .unwrap_or_else(|| meshes.add(Cuboid::new(0.06, 0.06, 0.18)));
    let bullet = mat(
        Color::srgb(0.88, 0.82, 0.45),
        LinearRgba::rgb(0.8, 0.6, 0.2),
    );
    let flail_mesh = crate::art::item_model(BALL_O_HURT)
        .and_then(|p| load_mesh(&mut meshes, &p, [8.0, 8.0, 1.0]))
        .unwrap_or_else(|| meshes.add(Cuboid::new(0.28, 0.28, 0.28)));
    let flail = mat(
        Color::srgb(0.45, 0.42, 0.58),
        LinearRgba::rgb(0.15, 0.12, 0.28),
    );
    let yoyo_mesh = crate::art::item_model(VALOR)
        .and_then(|p| load_mesh(&mut meshes, &p, [5.5, 5.5, 3.5]))
        .unwrap_or_else(|| meshes.add(Cuboid::new(0.24, 0.24, 0.18)));
    let yoyo = mat(
        Color::srgb(0.25, 0.55, 0.95),
        LinearRgba::rgb(0.15, 0.35, 0.85),
    );
    let vilethorn_mesh = crate::art::item_model(VILETHORN)
        .and_then(|p| load_mesh(&mut meshes, &p, [8.0, 8.0, 1.0]))
        .unwrap_or_else(|| meshes.add(Cuboid::new(0.16, 0.16, 0.55)));
    let vilethorn = mat(
        Color::srgb(0.55, 0.28, 0.78),
        LinearRgba::rgb(0.45, 0.15, 0.85),
    );
    let magic_missile_mesh = crate::art::item_model(MAGIC_MISSILE)
        .and_then(|p| load_mesh(&mut meshes, &p, [8.0, 8.0, 1.0]))
        .unwrap_or_else(|| meshes.add(Cuboid::new(0.16, 0.16, 0.28)));
    let magic_missile = mat(Color::srgb(0.35, 0.82, 1.0), LinearRgba::rgb(0.6, 1.8, 3.0));
    let water_stream_mesh = meshes.add(Cuboid::new(0.11, 0.11, 0.32));
    let water_stream = mat(Color::srgb(0.20, 0.78, 1.0), LinearRgba::rgb(0.2, 0.9, 1.6));
    let bomb_mesh = crate::art::item_model(BOMB)
        .and_then(|p| load_mesh(&mut meshes, &p, [5.0, 6.0, 5.0]))
        .unwrap_or_else(|| meshes.add(Cuboid::new(0.25, 0.25, 0.25)));
    let dynamite_mesh = crate::art::item_model(DYNAMITE)
        .and_then(|p| load_mesh(&mut meshes, &p, [4.0, 7.0, 4.0]))
        .unwrap_or_else(|| meshes.add(Cuboid::new(0.20, 0.32, 0.20)));
    let grenade_mesh = crate::art::item_model(GRENADE)
        .and_then(|p| load_mesh(&mut meshes, &p, [4.0, 5.0, 4.0]))
        .unwrap_or_else(|| meshes.add(Cuboid::new(0.20, 0.22, 0.20)));
    let grapple_mesh = crate::art::item_model(GRAPPLING_HOOK)
        .and_then(|p| load_mesh(&mut meshes, &p, [7.0, 7.0, 1.0]))
        .unwrap_or_else(|| meshes.add(Cuboid::new(0.25, 0.25, 0.25)));
    let star_mesh = meshes.add(Cuboid::new(0.32, 0.32, 0.32));
    let star_mat = mat(Color::srgb(1.0, 0.65, 0.92), LinearRgba::rgb(3.2, 1.4, 2.8));
    let explosive_mat = mat(Color::WHITE, LinearRgba::NONE);
    commands.insert_resource(ArrowVisuals {
        mesh,
        wood,
        fire,
        spark_mesh,
        spark,
        laser_mesh,
        laser,
        eye_laser,
        boomerang_mesh,
        flamarang_mesh,
        spear_mesh,
        bullet_mesh,
        bullet,
        flail_mesh,
        flail,
        yoyo_mesh,
        yoyo,
        vilethorn_mesh,
        vilethorn,
        magic_missile_mesh,
        magic_missile,
        water_stream_mesh,
        water_stream,
        bomb_mesh,
        dynamite_mesh,
        grenade_mesh,
        grapple_mesh,
        star_mesh,
        star_mat,
        explosive_mat,
    });
}

fn projectile_scale_and_axis(kind: ProjKind) -> (f32, Vec3) {
    match kind {
        ProjKind::Fireball | ProjKind::FlowerOfFire => (1.5, MODEL_AXIS.normalize()),
        ProjKind::DemonScythe => (1.6, Vec3::Y),
        ProjKind::Laser | ProjKind::EyeLaser | ProjKind::SpaceLaser => (1.0, Vec3::Z),
        ProjKind::WaterStream | ProjKind::WaterBolt => (1.0, Vec3::Z),
        ProjKind::Bullet => (0.24, MODEL_AXIS.normalize()),
        ProjKind::Flail => (0.38, MODEL_AXIS.normalize()),
        ProjKind::Yoyo => (0.34, Vec3::Z),
        ProjKind::Vilethorn => (0.34, MODEL_AXIS.normalize()),
        ProjKind::MagicMissile | ProjKind::Flamelash => (0.32, MODEL_AXIS.normalize()),
        ProjKind::Bomb | ProjKind::Dynamite | ProjKind::Grenade => (0.45, Vec3::Y),
        ProjKind::GrappleHook => (0.42, MODEL_AXIS.normalize()),
        ProjKind::StarfuryStar | ProjKind::FallenStarShot => (1.0, Vec3::Y),
        ProjKind::BeeShot => (0.28, MODEL_AXIS.normalize()),
        ProjKind::Arrow
        | ProjKind::FlamingArrow
        | ProjKind::Spark
        | ProjKind::Boomerang
        | ProjKind::Flamarang
        | ProjKind::ThornChakram
        | ProjKind::Spear => (ARROW_SCALE, MODEL_AXIS.normalize()),
    }
}

fn projectile_trail_color(kind: ProjKind) -> Color {
    match kind {
        ProjKind::FlamingArrow
        | ProjKind::Flamarang
        | ProjKind::Spark
        | ProjKind::Fireball
        | ProjKind::FlowerOfFire
        | ProjKind::Flamelash => Color::srgb(1.0, 0.55, 0.15),
        ProjKind::DemonScythe => Color::srgb(0.68, 0.22, 0.95),
        ProjKind::Laser => Color::srgb(0.85, 0.30, 1.0),
        ProjKind::SpaceLaser => Color::srgb(0.25, 0.98, 0.35),
        ProjKind::EyeLaser => Color::srgb(1.0, 0.20, 0.22),
        ProjKind::MagicMissile => Color::srgb(0.35, 0.85, 1.0),
        ProjKind::WaterStream | ProjKind::WaterBolt => Color::srgb(0.18, 0.72, 1.0),
        ProjKind::Vilethorn => Color::srgb(0.58, 0.28, 0.82),
        ProjKind::ThornChakram => Color::srgb(0.32, 0.85, 0.32),
        ProjKind::Bullet => Color::srgb(0.95, 0.86, 0.45),
        ProjKind::Flail => Color::srgb(0.55, 0.52, 0.68),
        ProjKind::Yoyo => Color::srgb(0.30, 0.62, 0.98),
        ProjKind::Bomb | ProjKind::Dynamite | ProjKind::Grenade => Color::srgb(1.0, 0.55, 0.15),
        ProjKind::GrappleHook => Color::srgb(0.70, 0.74, 0.80),
        ProjKind::StarfuryStar | ProjKind::FallenStarShot => Color::srgb(1.0, 0.65, 0.92),
        ProjKind::BeeShot => Color::srgb(0.98, 0.80, 0.15),
        ProjKind::Arrow | ProjKind::Boomerang | ProjKind::Spear => Color::srgb(0.85, 0.85, 0.88),
    }
}

fn projectile_visual_pair(
    vis: &ArrowVisuals,
    kind: ProjKind,
) -> (Handle<StandardMaterial>, Handle<Mesh>) {
    match kind {
        ProjKind::Arrow => (vis.wood.clone(), vis.mesh.clone()),
        ProjKind::FlamingArrow => (vis.fire.clone(), vis.mesh.clone()),
        ProjKind::Spark => (vis.spark.clone(), vis.spark_mesh.clone()),
        ProjKind::Fireball | ProjKind::FlowerOfFire => (vis.fire.clone(), vis.spark_mesh.clone()),
        ProjKind::DemonScythe => (vis.magic_missile.clone(), vis.flamarang_mesh.clone()),
        ProjKind::Laser | ProjKind::SpaceLaser => (vis.laser.clone(), vis.laser_mesh.clone()),
        ProjKind::EyeLaser => (vis.eye_laser.clone(), vis.laser_mesh.clone()),
        ProjKind::Boomerang | ProjKind::ThornChakram => {
            (vis.wood.clone(), vis.boomerang_mesh.clone())
        }
        ProjKind::Flamarang => (vis.fire.clone(), vis.flamarang_mesh.clone()),
        ProjKind::Spear => (vis.wood.clone(), vis.spear_mesh.clone()),
        ProjKind::Bullet => (vis.bullet.clone(), vis.bullet_mesh.clone()),
        ProjKind::Flail => (vis.flail.clone(), vis.flail_mesh.clone()),
        ProjKind::Yoyo => (vis.yoyo.clone(), vis.yoyo_mesh.clone()),
        ProjKind::Vilethorn => (vis.vilethorn.clone(), vis.vilethorn_mesh.clone()),
        ProjKind::MagicMissile => (vis.magic_missile.clone(), vis.magic_missile_mesh.clone()),
        ProjKind::Flamelash => (vis.fire.clone(), vis.magic_missile_mesh.clone()),
        ProjKind::WaterStream | ProjKind::WaterBolt => {
            (vis.water_stream.clone(), vis.water_stream_mesh.clone())
        }
        ProjKind::Bomb => (vis.explosive_mat.clone(), vis.bomb_mesh.clone()),
        ProjKind::Dynamite => (vis.explosive_mat.clone(), vis.dynamite_mesh.clone()),
        ProjKind::Grenade => (vis.explosive_mat.clone(), vis.grenade_mesh.clone()),
        ProjKind::GrappleHook => (vis.explosive_mat.clone(), vis.grapple_mesh.clone()),
        ProjKind::StarfuryStar | ProjKind::FallenStarShot => {
            (vis.star_mat.clone(), vis.star_mesh.clone())
        }
        ProjKind::BeeShot => (vis.bullet.clone(), vis.bullet_mesh.clone()),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply(
    mut commands: Commands,
    mut msgs: MessageReader<ProjectilesMsg>,
    mut ents: ResMut<ProjectileEntities>,
    vis: Option<Res<ArrowVisuals>>,
    mut tf: Query<&mut Transform>,
    mut particles: Option<ResMut<ParticlePool>>,
    settings: Option<Res<Settings>>,
    local_player: Option<Res<crate::view::player::LocalPlayer>>,
    mut gizmos: Option<Gizmos>,
) {
    let (Some(vis), Some(ProjectilesMsg(list))) = (vis, msgs.read().last()) else {
        return;
    };
    let settings_val = settings.as_deref().copied().unwrap_or_default();
    let live: std::collections::HashSet<u32> = list.iter().map(|p| p.id).collect();
    ents.0.retain(|id, e| {
        let keep = live.contains(id);
        if !keep {
            if let (Some(pool), Ok(t)) = (particles.as_deref_mut(), tf.get(*e)) {
                pool.spawn_burst(
                    t.translation,
                    8,
                    particle_color(HitMaterial::Stone),
                    2.8,
                    0.07,
                    0.32,
                    10.0,
                    &settings_val,
                );
            }
            commands.entity(*e).despawn();
        }
        keep
    });
    for p in list {
        let pos = Vec3::from(p.pos);
        if p.kind == ProjKind::GrappleHook
            && let (Some(lp), Some(g)) = (local_player.as_ref(), gizmos.as_mut())
        {
            let hand = Vec3::new(lp.0.body.pos[0], lp.0.body.pos[1] + 1.0, lp.0.body.pos[2]);
            g.line(hand, pos, Color::srgb(0.65, 0.70, 0.78));
        }
        let dir = Vec3::from(p.vel);
        if let Some(pool) = particles.as_deref_mut()
            && dir.length_squared() > 1e-10
        {
            let trail_color = projectile_trail_color(p.kind);
            pool.spawn(
                pos,
                -dir.normalize() * 0.4,
                trail_color,
                0.05,
                0.20,
                0.5,
                &settings_val,
            );
        }
        let (scale, axis) = projectile_scale_and_axis(p.kind);
        let at = |t: &mut Transform| {
            t.translation = pos;
            t.scale = Vec3::splat(scale);
            if dir.length_squared() > 1e-10 {
                t.rotation = Quat::from_rotation_arc(axis, dir.normalize());
            }
        };
        if let Some(mut t) = ents.0.get(&p.id).and_then(|e| tf.get_mut(*e).ok()) {
            at(&mut t);
            continue;
        }
        let mut t = Transform::from_translation(pos);
        at(&mut t);
        let (m, mesh) = projectile_visual_pair(&vis, p.kind);
        let e = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(m),
                t,
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        ents.0.insert(p.id, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_builds_and_mirrors_list() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_state::<AppState>()
            .add_plugins(ProjectilesPlugin);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        let arrow = |id| {
            Projectile::fire_from_bow(
                id,
                0,
                terravibe_sim::item::WOODEN_ARROW,
                [0.0; 3],
                [1.0, 0.0, 0.0],
            )
            .unwrap()
        };
        app.update(); // Startup
        app.world_mut()
            .write_message(ProjectilesMsg(vec![arrow(1), arrow(2)]));
        app.update();
        assert_eq!(app.world().resource::<ProjectileEntities>().0.len(), 2);
        app.world_mut()
            .write_message(ProjectilesMsg(vec![arrow(2)]));
        app.update();
        assert_eq!(app.world().resource::<ProjectileEntities>().0.len(), 1);
        app.world_mut().write_message(ProjectilesMsg(vec![]));
        app.update();
        assert!(app.world().resource::<ProjectileEntities>().0.is_empty());
    }

    #[test]
    fn fireball_spawns_with_1_5_scale() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_state::<AppState>()
            .add_plugins(ProjectilesPlugin);
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::InGame);
        app.update(); // Startup
        let fireball = Projectile {
            id: 99,
            kind: ProjKind::Fireball,
            pos: [1.0, 2.0, 3.0],
            vel: [0.1, 0.0, 0.0],
            owner: 0,
            ttl: 100,
            damage: 20,
            knock_back: 1.0,
            crit: 4,
            ai0: 0,
            pierced: 0,
            origin: [1.0, 2.0, 3.0],
            armor_penetration: 0,
        };
        app.world_mut()
            .write_message(ProjectilesMsg(vec![fireball]));
        app.update();
        let ents = app.world().resource::<ProjectileEntities>();
        assert_eq!(ents.0.len(), 1);
        let e = ents.0[&99];
        let tf = app.world().entity(e).get::<Transform>().unwrap();
        assert_eq!(tf.scale, Vec3::splat(1.5));
    }

    #[test]
    fn laser_projectiles_orient_from_z_with_unit_scale() {
        assert_eq!(projectile_scale_and_axis(ProjKind::Laser), (1.0, Vec3::Z));
        assert_eq!(
            projectile_scale_and_axis(ProjKind::EyeLaser),
            (1.0, Vec3::Z)
        );
        assert_eq!(
            projectile_scale_and_axis(ProjKind::Arrow),
            (ARROW_SCALE, MODEL_AXIS.normalize())
        );
    }

    #[test]
    fn non_arrow_projectiles_use_distinct_visuals_and_trails() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_state::<AppState>()
            .add_plugins(ProjectilesPlugin);
        app.update();
        let vis = app.world().resource::<ArrowVisuals>();
        let (arrow_mat, arrow_mesh) = projectile_visual_pair(vis, ProjKind::Arrow);
        for kind in [
            ProjKind::Bullet,
            ProjKind::Flail,
            ProjKind::Yoyo,
            ProjKind::Vilethorn,
            ProjKind::MagicMissile,
            ProjKind::WaterStream,
            ProjKind::Flamarang,
        ] {
            let (mat, mesh) = projectile_visual_pair(vis, kind);
            assert!(
                mesh != arrow_mesh || mat != arrow_mat,
                "{kind:?} should not share Wooden Arrow mesh and material"
            );
            assert_ne!(
                projectile_trail_color(kind),
                projectile_trail_color(ProjKind::Arrow),
                "{kind:?} should have a distinct particle trail color"
            );
        }
    }
}
