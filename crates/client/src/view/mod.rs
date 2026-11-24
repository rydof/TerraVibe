//! The playable spike: streamed relief chunks, a fly camera, enemies and a click sound.

pub(crate) mod avatar;
pub(crate) mod block_models;
pub(crate) mod boss;
pub(crate) mod camera;
pub(crate) mod chunks;
pub(crate) mod drops;
pub(crate) mod enemies;
pub(crate) mod held_anim;
pub(crate) mod interact;
pub(crate) mod liquid_mesh;
pub(crate) mod particles;
pub(crate) mod player;
pub(crate) mod player_art;
pub(crate) mod projectiles;
pub(crate) mod sky;
pub(crate) mod stars;
pub(crate) mod weather;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use terravibe_assets::{RgbaImage, TerrariaContent};

use crate::art::mesh_data::MeshData;
use crate::session::AppState;

/// World seed for the spike terrain.
pub const SEED: u32 = 1;

/// Env var: save a screenshot to this path once chunk streaming settles, then exit.
pub const ENV_SCREENSHOT: &str = "TERRAVIBE_SCREENSHOT";
const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(20);
/// Frames to wait after streaming settles so the last meshes are uploaded and drawn.
const SETTLE_FRAMES: u32 = 10;

pub struct ViewPlugin {
    pub content: TerrariaContent,
    /// `--bench`: run the scripted benchmark (see `crate::bench`) instead of play.
    pub bench: bool,
}

/// Whether `--bench` was given.
#[derive(Resource)]
pub struct BenchMode(pub bool);

/// Prints `msg` and exits with code 1 (asset failures are not panics).
pub(crate) fn fatal(msg: impl std::fmt::Display) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(1)
}

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        let c = &self.content;
        let art = Arc::new(crate::art::blocks::BlockArt::load(&crate::art::art_dir()));
        let cracks = c
            .texture("Images/TileCracks")
            .unwrap_or_else(|e| fatal(format!("Images/TileCracks: {e}")));
        // Pre-render crisp 32x32 icons for all 225 items from BlockArt + .vox models.
        let icons = Arc::new(crate::art::item_icons::render_all_item_icons(
            &art,
            &crate::art::art_dir().join("models"),
        ));
        info!(
            "assets from {} ({})",
            c.root().display(),
            if c.is_pack() {
                "asset pack"
            } else {
                "Terraria install"
            }
        );

        let font = |n: &str| Arc::new(c.font(n).unwrap_or_else(|e| fatal(format!("{n}: {e}"))));
        let hud = crate::hud::HudPlugin {
            mouse: font("Fonts/Mouse_Text"),
            death: font("Fonts/Death_Text"),
            stack: font("Fonts/Item_Stack"),
            hotbar: crate::hud::hotbar::HotbarSource {
                back: c
                    .texture("Images/Inventory_Back")
                    .unwrap_or_else(|e| fatal(format!("Images/Inventory_Back: {e}"))),
                back_selected: c
                    .texture("Images/Inventory_Back14")
                    .unwrap_or_else(|e| fatal(format!("Images/Inventory_Back14: {e}"))),
                icons: icons.iter().map(|(k, v)| (*k, v.clone())).collect(),
            },
            heart: c
                .texture("Images/Heart")
                .unwrap_or_else(|e| fatal(format!("Images/Heart: {e}"))),
            mana: c
                .texture("Images/Mana")
                .unwrap_or_else(|e| fatal(format!("Images/Mana: {e}"))),
            extra_ui: crate::hud::ExtraUiSource {
                coin_ghosts: [
                    c.texture("Images/Coin_0").ok(),
                    c.texture("Images/Coin_1").ok(),
                    c.texture("Images/Coin_2").ok(),
                    c.texture("Images/Coin_3").ok(),
                ],
                ammo_ghost: c.texture("Images/Item_40").ok(),
                trash_ghost: c.texture("Images/Trash").ok(),
                armor_head_ghost: c.texture("Images/UI/DisplaySlots_0").ok(),
                armor_body_ghost: c.texture("Images/UI/DisplaySlots_1").ok(),
                armor_legs_ghost: c.texture("Images/UI/DisplaySlots_2").ok(),
                accessory_ghost: c.texture("Images/UI/DisplaySlots_3").ok(),
                hammer_btn: c.texture("Images/UI/Craft_Toggle_0").ok(),
                quick_stack_btn: c.texture("Images/UI/ChestStack_0").ok(),
                sort_btn: c.texture("Images/UI/Sort_0").ok(),
                shield_icon: c.texture("Images/UI/Bestiary/Stat_Defense").ok(),
                chat_back: c.texture("Images/Chat_Back").ok(),
                heart_fancy: c
                    .texture("Images/UI/PlayerResourceSets/FancyClassic/Heart_Single_Fancy")
                    .ok(),
                star_fancy: c
                    .texture("Images/UI/PlayerResourceSets/FancyClassic/Star_Single")
                    .ok(),
                buff_potion_sickness: c.texture("Images/Buff_21").ok(),
            },
        };

        let mode = if self.bench
            || std::env::var_os(camera::ENV_CAMERA).is_some()
            || interact::demo_is_tunnel()
        {
            camera::ControlMode::Fly
        } else {
            camera::ControlMode::Player
        };
        let sounds = player::PlayerSounds::load(c);

        app.insert_resource(BenchMode(self.bench))
            .insert_resource(mode)
            .insert_resource(GlobalAmbientLight {
                color: Color::WHITE,
                brightness: 400.0,
                ..default()
            })
            .init_resource::<sky::SkyClock>()
            .init_resource::<sky::SkyAtmosphere>()
            .add_systems(OnEnter(AppState::InGame), sky::setup_skybox)
            .add_systems(
                Update,
                (sky::update_sky, sky::update_celestial_skybox).run_if(in_state(AppState::InGame)),
            )
            .add_plugins((
                chunks::ChunksPlugin { art },
                interact::InteractPlugin {
                    cracks: Arc::new(cracks),
                    icons: icons.clone(),
                    sounds: interact::UseSounds::load(c),
                },
                camera::FlyCameraPlugin,
                crate::net::NetPlugin,
                player::PlayerPlugin { sounds },
                avatar::AvatarPlugin,
                hud,
                enemies::EnemiesPlugin {
                    sounds: enemies::EnemySounds::load(c),
                },
                projectiles::ProjectilesPlugin,
                particles::ParticlesPlugin,
                weather::WeatherPlugin,
                stars::StarsPlugin,
                boss::BossPlugin,
                drops::DropsPlugin {
                    icons: icons.clone(),
                },
            ))
            .add_systems(OnEnter(AppState::InGame), setup_light);
        if self.bench {
            app.add_plugins(crate::bench::BenchPlugin);
        }

        if let Some(path) = std::env::var_os(ENV_SCREENSHOT).filter(|v| !v.is_empty()) {
            app.insert_resource(ScreenshotJob {
                path: PathBuf::from(path),
                settled_frames: 0,
                requested: false,
                saved: false,
            })
            .add_systems(
                Update,
                screenshot_when_settled
                    .after(chunks::stream_chunks)
                    .run_if(in_state(AppState::InGame)),
            );
        }
    }
}

fn setup_light(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0).looking_to(Vec3::new(-0.4, -1.0, -0.6), Vec3::Y),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Top solid block's y in column (x, z), scanning down from the top of the block layers.
pub(crate) fn surface_y(x: i32, z: i32) -> Option<i32> {
    terravibe_server::surface_y(SEED, x, z)
}

/// Converts straight-alpha RGBA8 pixels into an sRGB Bevy image with a nearest sampler.
pub(crate) fn to_bevy_image(img: &RgbaImage) -> Image {
    let data = img.pixels.iter().flatten().copied().collect();
    let mut out = Image::new(
        Extent3d {
            width: img.width,
            height: img.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    out.sampler = ImageSampler::nearest();
    out
}

/// Converts CPU mesh data into a Bevy triangle-list mesh; `None` if empty.
pub(crate) fn to_bevy_mesh(m: MeshData) -> Option<Mesh> {
    if m.indices.is_empty() {
        return None;
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, m.uvs)
    .with_inserted_indices(Indices::U32(m.indices));
    if !m.colors.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, m.colors);
    }
    Some(mesh)
}

/// Converts CPU mesh data into Bevy meshes of at most `max_quads` quads each
/// (u16 indices), so a big chunk can be uploaded across several frames.
/// Relies on `MeshData` holding whole quads: 4 vertices and 6 indices each.
pub(crate) fn to_bevy_mesh_parts(m: MeshData, max_quads: usize) -> Vec<Mesh> {
    assert!(max_quads * 4 <= usize::from(u16::MAX) + 1);
    let quads = m.quad_count();
    (0..quads)
        .step_by(max_quads)
        .map(|q0| {
            let q1 = (q0 + max_quads).min(quads);
            let (v0, v1) = (q0 * 4, q1 * 4);
            let base = v0 as u32;
            let indices = m.indices[q0 * 6..q1 * 6]
                .iter()
                .map(|&i| (i - base) as u16)
                .collect();
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions[v0..v1].to_vec())
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals[v0..v1].to_vec())
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, m.uvs[v0..v1].to_vec())
            .with_inserted_indices(Indices::U16(indices));
            if !m.colors.is_empty() {
                mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, m.colors[v0..v1].to_vec());
            }
            mesh
        })
        .collect()
}

#[derive(Resource)]
struct ScreenshotJob {
    path: PathBuf,
    settled_frames: u32,
    requested: bool,
    saved: bool,
}

/// Debug aid: waits for chunk streaming to go idle (or the timeout), then
/// saves one screenshot and exits.
fn screenshot_when_settled(
    mut commands: Commands,
    mut job: ResMut<ScreenshotJob>,
    stream: Res<chunks::ChunkStream>,
    time: Res<Time<Real>>,
    mut exit: MessageWriter<AppExit>,
    demo: Option<Res<interact::DemoPending>>,
    welcomed: Option<Res<player::LocalPlayer>>,
    mode: Res<camera::ControlMode>,
) {
    if job.saved {
        info!("screenshot saved to {}", job.path.display());
        exit.write(AppExit::Success);
        return;
    }
    if job.requested {
        return;
    }
    // Wait for the server Welcome (spawn + demo setup), the demo and streaming.
    let joins = *mode == camera::ControlMode::Player || interact::demo_is_tunnel();
    if (joins && welcomed.is_none()) || stream.is_busy() || demo.is_some() {
        job.settled_frames = 0;
    } else {
        job.settled_frames += 1;
    }
    let timed_out = time.elapsed() >= SCREENSHOT_TIMEOUT;
    if job.settled_frames >= SETTLE_FRAMES || timed_out {
        if timed_out {
            warn!("screenshot: chunk streaming still busy after {SCREENSHOT_TIMEOUT:?}");
        }
        info!("screenshot: {} chunk meshes", stream.mesh_count());
        job.requested = true;
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(job.path.clone()))
            .observe(|_: On<ScreenshotCaptured>, mut job: ResMut<ScreenshotJob>| job.saved = true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_parts_split_whole_quads_with_local_indices() {
        let mut m = MeshData::default();
        for i in 0..5 {
            let x = i as f32;
            m.push_quad([[x, 0.0, 0.0]; 4], [0.0, 1.0, 0.0], [[0.0, 0.0]; 4]);
        }
        let parts = to_bevy_mesh_parts(m, 2);
        let verts: Vec<usize> = parts.iter().map(Mesh::count_vertices).collect();
        assert_eq!(verts, [8, 8, 4]);
        let Some(Indices::U16(idx)) = parts[2].indices() else {
            panic!("expected u16 indices")
        };
        assert_eq!(idx, &[0, 1, 2, 0, 2, 3]);
        assert!(to_bevy_mesh_parts(MeshData::default(), 2).is_empty());
    }
}
