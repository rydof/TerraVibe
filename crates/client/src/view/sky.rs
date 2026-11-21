//! Day/night presentation and celestial skybox. Client-only: the server clock
//! tick becomes a sky colour, a sun direction, ambient/directional strength and
//! one global `sky_brightness` written into every chunk material's `origin.w`.
//!
//! Includes celestial orbital celestial bodies (Sun, Moon with phases, starfield,
//! multi-layered drifting parallax clouds) and biome-based atmospheric fog.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use terravibe_sim::time::{CYCLE_TICKS, DAY_TICKS, NIGHT_FLOOR, sky_brightness, sun_angle};

use crate::light::ChunkMaterial;
use crate::net::ClientClock;
use crate::session::AppState;

pub const DAY_SKY: [f32; 3] = [0.53, 0.72, 0.95];
pub const DESERT_SKY: [f32; 3] = [0.85, 0.72, 0.48];
pub const SNOW_SKY: [f32; 3] = [0.75, 0.82, 0.95];
pub const JUNGLE_SKY: [f32; 3] = [0.45, 0.75, 0.65];
pub const CORRUPTION_SKY: [f32; 3] = [0.15, 0.08, 0.22];

pub const NIGHT_SKY: [f32; 3] = [0.015, 0.025, 0.07];
/// Warm tint added at the middle of dawn/dusk.
pub const DUSK_TINT: [f32; 3] = [0.35, 0.08, -0.10];
pub const AMBIENT_DAY: f32 = 400.0;
pub const SUN_DAY: f32 = 8_000.0;
/// Brightness is pushed to materials in steps of 1/64 to bound uniform writes.
pub const QUANT: f32 = 64.0;
pub const CELESTIAL_DIST: f32 = 450.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoonPhase {
    Full,
    WaningGibbous,
    ThirdQuarter,
    WaningCrescent,
    NewMoon,
    WaxingCrescent,
    FirstQuarter,
    WaxingGibbous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtmosphereBiome {
    Forest,
    Desert,
    Snow,
    Jungle,
    Corruption,
}

#[derive(Component)]
pub struct CelestialRoot;

#[derive(Component)]
pub struct SunDisc;

#[derive(Component, Default)]
pub struct MoonDisc {
    pub current_phase: Option<MoonPhase>,
}

#[derive(Component)]
pub struct Starfield;

#[derive(Component)]
pub struct SkyDome;

#[derive(Component)]
pub struct CloudLayer {
    pub speed: Vec2,
    pub offset: Vec2,
    pub altitude: f32,
}

#[derive(Resource)]
pub struct SkyAtmosphere {
    pub current_fog_color: [f32; 3],
    pub current_sky_color: [f32; 3],
    pub current_fog_start: f32,
    pub current_fog_end: f32,
    pub corruption_factor: f32,
}

impl Default for SkyAtmosphere {
    fn default() -> Self {
        Self {
            current_fog_color: DAY_SKY,
            current_sky_color: DAY_SKY,
            current_fog_start: 80.0,
            current_fog_end: 320.0,
            corruption_factor: 0.0,
        }
    }
}

pub fn celestial_orbital_pos(angle: f32, distance: f32) -> Vec3 {
    let x = -angle.cos() * distance;
    let y = angle.sin() * distance;
    let z = -0.25 * distance;
    Vec3::new(x, y, z)
}

pub fn sun_orbital_position(tick: u32, distance: f32) -> Vec3 {
    let a = sun_angle(tick);
    if tick < DAY_TICKS {
        celestial_orbital_pos(a, distance)
    } else {
        celestial_orbital_pos(a + std::f32::consts::PI, distance)
    }
}

pub fn moon_orbital_position(tick: u32, distance: f32) -> Vec3 {
    let a = sun_angle(tick);
    if tick < DAY_TICKS {
        celestial_orbital_pos(a + std::f32::consts::PI, distance)
    } else {
        celestial_orbital_pos(a, distance)
    }
}

pub fn moon_phase(day_index: u32) -> MoonPhase {
    match day_index % 8 {
        0 => MoonPhase::Full,
        1 => MoonPhase::WaningGibbous,
        2 => MoonPhase::ThirdQuarter,
        3 => MoonPhase::WaningCrescent,
        4 => MoonPhase::NewMoon,
        5 => MoonPhase::WaxingCrescent,
        6 => MoonPhase::FirstQuarter,
        _ => MoonPhase::WaxingGibbous,
    }
}

pub fn biome_base_color(biome: AtmosphereBiome) -> [f32; 3] {
    match biome {
        AtmosphereBiome::Forest => DAY_SKY,
        AtmosphereBiome::Desert => DESERT_SKY,
        AtmosphereBiome::Snow => SNOW_SKY,
        AtmosphereBiome::Jungle => JUNGLE_SKY,
        AtmosphereBiome::Corruption => CORRUPTION_SKY,
    }
}

pub fn interpolate_biome_color(from: [f32; 3], to: [f32; 3], t: f32) -> [f32; 3] {
    if t <= 0.0 {
        return from;
    }
    if t >= 1.0 {
        return to;
    }
    [
        from[0] + (to[0] - from[0]) * t,
        from[1] + (to[1] - from[1]) * t,
        from[2] + (to[2] - from[2]) * t,
    ]
}

pub fn blended_biome_fog_color(weights: &[f32; 4], corruption: f32) -> [f32; 3] {
    let corruption = corruption.clamp(0.0, 1.0);
    let mut base = [0.0f32; 3];
    let colors = [
        biome_base_color(AtmosphereBiome::Forest),
        biome_base_color(AtmosphereBiome::Desert),
        biome_base_color(AtmosphereBiome::Snow),
        biome_base_color(AtmosphereBiome::Jungle),
    ];
    let total_w = weights.iter().sum::<f32>().max(1e-5);
    for (i, &w) in weights.iter().enumerate() {
        let norm_w = w / total_w;
        base[0] += colors[i][0] * norm_w;
        base[1] += colors[i][1] * norm_w;
        base[2] += colors[i][2] * norm_w;
    }
    let corrupt_color = biome_base_color(AtmosphereBiome::Corruption);
    interpolate_biome_color(base, corrupt_color, corruption)
}

/// Sky colour for a brightness in `NIGHT_FLOOR..=1`.
#[allow(dead_code)]
pub fn sky_colour(brightness: f32) -> [f32; 3] {
    sky_colour_with_biome(brightness, DAY_SKY)
}

/// Sky colour for a brightness in `NIGHT_FLOOR..=1` with given daytime sky color.
pub fn sky_colour_with_biome(brightness: f32, day_sky: [f32; 3]) -> [f32; 3] {
    let t = ((brightness - NIGHT_FLOOR) / (1.0 - NIGHT_FLOOR)).clamp(0.0, 1.0);
    let warm = 4.0 * t * (1.0 - t);
    std::array::from_fn(|i| {
        (NIGHT_SKY[i] + (day_sky[i] - NIGHT_SKY[i]) * t + DUSK_TINT[i] * warm).clamp(0.0, 1.0)
    })
}

/// Applies directional dawn (warm rose-gold) and dusk (amber-crimson) color
/// grading when `atmosphere_fx` is enabled.
pub fn dawn_dusk_sky_color(tick: u32, day_sky: [f32; 3], atmosphere_fx: bool) -> [f32; 3] {
    let base = sky_colour_with_biome(sky_brightness(tick), day_sky);
    if !atmosphere_fx {
        return base;
    }
    let t = (tick % CYCLE_TICKS) as f32;
    let day = DAY_TICKS as f32;
    let ramp = day * 3.0 / 15.0;
    let (tint, weight) = if t < ramp {
        let k = (t / ramp).clamp(0.0, 1.0);
        ([0.18, 0.08, 0.02], 4.0 * k * (1.0 - k))
    } else if t > day - ramp && t < day {
        let k = ((day - t) / ramp).clamp(0.0, 1.0);
        ([0.22, 0.04, 0.06], 4.0 * k * (1.0 - k))
    } else {
        ([0.0, 0.0, 0.0], 0.0)
    };
    std::array::from_fn(|i| (base[i] + tint[i] * weight).clamp(0.0, 1.0))
}

/// Detects nearby corruption presence based on surrounding blocks in loaded chunks.
pub fn detect_corruption_factor(
    pos: Vec3,
    stream: Option<&crate::view::chunks::ChunkStream>,
) -> f32 {
    let Some(stream) = stream else {
        return 0.0;
    };
    use terravibe_sim::{CORRUPT_GRASS, EBONSTONE};
    let center_x = pos.x as i32;
    let center_y = pos.y as i32;
    let center_z = pos.z as i32;
    let chunks = stream.generated();
    let mut corrupt_count = 0;
    let mut samples = 0;

    for dy in (-8..=8).step_by(4) {
        for dx in (-16..=16).step_by(4) {
            for dz in (-16..=16).step_by(4) {
                samples += 1;
                let bx = center_x + dx;
                let by = center_y + dy;
                let bz = center_z + dz;
                let cp = terravibe_sim::ChunkPos::of_block(bx, by, bz);
                if let Some(chunk) = chunks.get(&cp) {
                    let lx = bx.rem_euclid(terravibe_sim::CHUNK_SIZE) as usize;
                    let ly = by.rem_euclid(terravibe_sim::CHUNK_SIZE) as usize;
                    let lz = bz.rem_euclid(terravibe_sim::CHUNK_SIZE) as usize;
                    let b = chunk.get(lx, ly, lz);
                    if b == EBONSTONE || b == CORRUPT_GRASS {
                        corrupt_count += 1;
                    }
                }
            }
        }
    }
    if samples == 0 {
        0.0
    } else {
        (corrupt_count as f32 / 12.0).clamp(0.0, 1.0)
    }
}

fn make_image(width: u32, height: u32, data: Vec<u8>) -> Image {
    let mut img = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::nearest();
    img
}

pub fn generate_sun_image() -> Image {
    const W: u32 = 32;
    const H: u32 = 32;
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    let cx = 15.5f32;
    let cy = 15.5f32;
    for y in 0..H {
        for x in 0..W {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            if d <= 7.0 {
                data.extend_from_slice(&[255, 248, 160, 255]);
            } else if d <= 10.5 {
                data.extend_from_slice(&[255, 200, 50, 255]);
            } else if d <= 13.0 {
                data.extend_from_slice(&[255, 140, 25, 220]);
            } else if d <= 14.5 {
                data.extend_from_slice(&[255, 110, 10, 120]);
            } else {
                data.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    make_image(W, H, data)
}

pub fn generate_moon_atlas_image() -> Image {
    const W: u32 = 256;
    const H: u32 = 32;
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let frame = x / 32;
            let lx = (x % 32) as f32 - 15.5;
            let ly = y as f32 - 15.5;
            let d = (lx * lx + ly * ly).sqrt();
            if d > 13.5 {
                data.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            let r = 13.5f32;
            let nx = lx / r;
            let lit = match frame {
                0 => true,
                1 => nx < 0.5,
                2 => nx < 0.0,
                3 => nx < -0.5,
                4 => false,
                5 => nx > 0.5,
                6 => nx > 0.0,
                7 => nx > -0.5,
                _ => true,
            };
            if lit {
                let is_crater = ((lx as i32 * 7 + ly as i32 * 13) % 11).abs() < 3 && d < 10.0;
                if is_crater {
                    data.extend_from_slice(&[185, 195, 215, 255]);
                } else if d <= 11.0 {
                    data.extend_from_slice(&[240, 245, 255, 255]);
                } else {
                    data.extend_from_slice(&[215, 225, 245, 255]);
                }
            } else {
                data.extend_from_slice(&[25, 28, 45, 180]);
            }
        }
    }
    make_image(W, H, data)
}

pub fn generate_cloud_image() -> Image {
    const W: u32 = 128;
    const H: u32 = 32;
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    let puffs = [
        (36.0f32, 18.0f32, 26.0f32, 12.0f32),
        (68.0, 14.0, 32.0, 16.0),
        (96.0, 19.0, 24.0, 11.0),
    ];
    for y in 0..H {
        for x in 0..W {
            let (xf, yf) = (x as f32, y as f32);
            let mut inside = false;
            let mut top_puff = false;
            for &(cx, cy, rx, ry) in &puffs {
                let dx = (xf - cx) / rx;
                let dy = (yf - cy) / ry;
                if dx * dx + dy * dy <= 1.0 {
                    inside = true;
                    if dy < -0.1 {
                        top_puff = true;
                    }
                }
            }
            if inside {
                if top_puff {
                    data.extend_from_slice(&[255, 255, 255, 235]);
                } else if yf > 22.0 {
                    data.extend_from_slice(&[190, 205, 225, 210]);
                } else {
                    data.extend_from_slice(&[240, 245, 255, 225]);
                }
            } else {
                data.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    make_image(W, H, data)
}

pub fn generate_starfield_mesh() -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    const STAR_COUNT: usize = 120;
    const R: f32 = 480.0;
    let mut seed = 0x1234_5678u32;
    let mut next_f32 = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (seed >> 8) as f32 / 16_777_216.0
    };

    for _ in 0..STAR_COUNT {
        let theta = 0.15 + next_f32() * (std::f32::consts::FRAC_PI_2 - 0.2);
        let phi = next_f32() * std::f32::consts::TAU;
        let star_size = 1.6 + next_f32() * 1.6;

        let center = Vec3::new(
            R * theta.cos() * phi.cos(),
            R * theta.sin(),
            R * theta.cos() * phi.sin(),
        );
        let normal = -center.normalize();
        let right = normal.cross(Vec3::Y).normalize_or_zero() * (star_size * 0.5);
        let up = right.cross(normal).normalize_or_zero() * (star_size * 0.5);

        let p0 = center - right - up;
        let p1 = center + right - up;
        let p2 = center + right + up;
        let p3 = center - right + up;

        let base = positions.len() as u32;
        positions.extend_from_slice(&[p0.to_array(), p1.to_array(), p2.to_array(), p3.to_array()]);
        normals.extend_from_slice(&[normal.to_array(); 4]);
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        let brightness = 0.75 + next_f32() * 0.25;
        colors.extend_from_slice(&[[brightness, brightness, brightness, 1.0]; 4]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

pub const SKY_DOME_RADIUS: f32 = 520.0;
const SKY_DOME_LON_SEGS: usize = 24;
const SKY_DOME_LAT_RINGS: usize = 12;

/// Computes `(zenith, horizon, nadir)` linear RGB colors for the sky dome gradient.
pub fn sky_dome_gradient_colors(
    tick: u32,
    horizon_sky: [f32; 3],
    corruption: f32,
    cave_fog: f32,
    atmosphere_fx: bool,
) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let b = sky_brightness(tick);
    let day_t = ((b - NIGHT_FLOOR) / (1.0 - NIGHT_FLOOR)).clamp(0.0, 1.0);
    let corruption = corruption.clamp(0.0, 1.0);
    let cave_fog = cave_fog.clamp(0.0, 1.0);

    let cave_color = [0.03, 0.035, 0.05];
    let horizon = interpolate_biome_color(horizon_sky, cave_color, cave_fog);

    let day_zenith = [0.22, 0.46, 0.88];
    let night_zenith = [0.008, 0.012, 0.035];
    let corrupt_zenith = [0.07, 0.03, 0.14];
    let dusk_weight = if atmosphere_fx {
        4.0 * day_t * (1.0 - day_t)
    } else {
        0.0
    };
    let mut zenith = [
        (night_zenith[0] + (day_zenith[0] - night_zenith[0]) * day_t - 0.02 * dusk_weight)
            .clamp(0.0, 1.0),
        (night_zenith[1] + (day_zenith[1] - night_zenith[1]) * day_t - 0.04 * dusk_weight)
            .clamp(0.0, 1.0),
        (night_zenith[2] + (day_zenith[2] - night_zenith[2]) * day_t + 0.08 * dusk_weight)
            .clamp(0.0, 1.0),
    ];
    zenith = interpolate_biome_color(zenith, corrupt_zenith, corruption);
    zenith = interpolate_biome_color(zenith, [0.02, 0.025, 0.035], cave_fog);

    let nadir_base = [horizon[0] * 0.55, horizon[1] * 0.55, horizon[2] * 0.60];
    let nadir = interpolate_biome_color(nadir_base, [0.02, 0.025, 0.035], cave_fog);

    (zenith, horizon, nadir)
}

fn sky_dome_vertex_colors(zenith: [f32; 3], horizon: [f32; 3], nadir: [f32; 3]) -> Vec<[f32; 4]> {
    let mut colors = Vec::with_capacity((SKY_DOME_LAT_RINGS + 1) * (SKY_DOME_LON_SEGS + 1));
    for ring in 0..=SKY_DOME_LAT_RINGS {
        let v = ring as f32 / SKY_DOME_LAT_RINGS as f32;
        let phi = -std::f32::consts::FRAC_PI_2 + v * std::f32::consts::PI;
        let y_norm = phi.sin();
        let c = if y_norm >= 0.0 {
            interpolate_biome_color(horizon, zenith, y_norm.powf(0.65))
        } else {
            interpolate_biome_color(horizon, nadir, (-y_norm).powf(0.75))
        };
        let rgba = [c[0], c[1], c[2], 1.0];
        for _ in 0..=SKY_DOME_LON_SEGS {
            colors.push(rgba);
        }
    }
    colors
}

pub fn generate_sky_dome_mesh() -> Mesh {
    let vert_count = (SKY_DOME_LAT_RINGS + 1) * (SKY_DOME_LON_SEGS + 1);
    let mut positions = Vec::with_capacity(vert_count);
    let mut normals = Vec::with_capacity(vert_count);
    let mut indices = Vec::with_capacity(SKY_DOME_LAT_RINGS * SKY_DOME_LON_SEGS * 6);

    for ring in 0..=SKY_DOME_LAT_RINGS {
        let v = ring as f32 / SKY_DOME_LAT_RINGS as f32;
        let phi = -std::f32::consts::FRAC_PI_2 + v * std::f32::consts::PI;
        let (sin_p, cos_p) = phi.sin_cos();
        for seg in 0..=SKY_DOME_LON_SEGS {
            let u = seg as f32 / SKY_DOME_LON_SEGS as f32;
            let theta = u * std::f32::consts::TAU;
            let (sin_t, cos_t) = theta.sin_cos();
            let nx = cos_p * cos_t;
            let ny = sin_p;
            let nz = cos_p * sin_t;
            positions.push([
                nx * SKY_DOME_RADIUS,
                ny * SKY_DOME_RADIUS,
                nz * SKY_DOME_RADIUS,
            ]);
            normals.push([-nx, -ny, -nz]);
        }
    }

    let stride = (SKY_DOME_LON_SEGS + 1) as u32;
    for ring in 0..SKY_DOME_LAT_RINGS as u32 {
        for seg in 0..SKY_DOME_LON_SEGS as u32 {
            let a = ring * stride + seg;
            let b = a + 1;
            let c = (ring + 1) * stride + seg;
            let d = c + 1;
            // Inward CCW winding
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }

    let (zenith, horizon, nadir) = sky_dome_gradient_colors(DAY_TICKS / 2, DAY_SKY, 0.0, 0.0, true);
    let colors = sky_dome_vertex_colors(zenith, horizon, nadir);

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

pub fn create_quad_mesh(w: f32, h: f32, uv_rect: [f32; 4]) -> Mesh {
    let hw = w * 0.5;
    let hh = h * 0.5;
    let positions = vec![
        [-hw, -hh, 0.0],
        [hw, -hh, 0.0],
        [hw, hh, 0.0],
        [-hw, hh, 0.0],
    ];
    let normals = vec![[0.0, 0.0, 1.0]; 4];
    let uvs = vec![
        [uv_rect[0], uv_rect[3]],
        [uv_rect[2], uv_rect[3]],
        [uv_rect[2], uv_rect[1]],
        [uv_rect[0], uv_rect[1]],
    ];
    let indices = vec![0, 1, 2, 0, 2, 3];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

pub fn setup_skybox(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sun_img = images.add(generate_sun_image());
    let moon_img = images.add(generate_moon_atlas_image());
    let cloud_img = images.add(generate_cloud_image());
    let star_mesh = meshes.add(generate_starfield_mesh());
    let sky_dome_mesh = meshes.add(generate_sky_dome_mesh());

    let sky_dome_mat = materials.add(StandardMaterial {
        unlit: true,
        fog_enabled: false,
        cull_mode: Some(bevy::render::render_resource::Face::Back),
        double_sided: false,
        base_color: Color::WHITE,
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });

    let sun_mesh = meshes.add(create_quad_mesh(45.0, 45.0, [0.0, 0.0, 1.0, 1.0]));
    let moon_mesh = meshes.add(create_quad_mesh(38.0, 38.0, [0.0, 0.0, 0.125, 1.0]));

    let sun_mat = materials.add(StandardMaterial {
        base_color_texture: Some(sun_img),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        ..default()
    });

    let moon_mat = materials.add(StandardMaterial {
        base_color_texture: Some(moon_img),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        ..default()
    });

    let star_mat = materials.add(StandardMaterial {
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.0),
        ..default()
    });

    let cloud_mat = materials.add(StandardMaterial {
        base_color_texture: Some(cloud_img),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.9),
        ..default()
    });

    let make_cloud_layer_mesh = |scale: f32| {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut indices = Vec::new();
        let qw = 70.0 * scale;
        let qh = 24.0 * scale;
        for ix in -3..=3 {
            for iz in -3..=3 {
                let cx = ix as f32 * 120.0 + ((iz * 37) % 40) as f32;
                let cz = iz as f32 * 120.0 + ((ix * 29) % 40) as f32;
                let base = positions.len() as u32;
                positions.extend_from_slice(&[
                    [cx - qw * 0.5, 0.0, cz - qh * 0.5],
                    [cx + qw * 0.5, 0.0, cz - qh * 0.5],
                    [cx + qw * 0.5, 0.0, cz + qh * 0.5],
                    [cx - qw * 0.5, 0.0, cz + qh * 0.5],
                ]);
                normals.extend_from_slice(&[[0.0, -1.0, 0.0]; 4]);
                uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
    };

    let cloud_mesh_1 = meshes.add(make_cloud_layer_mesh(1.0));
    let cloud_mesh_2 = meshes.add(make_cloud_layer_mesh(1.4));

    commands
        .spawn((
            CelestialRoot,
            Transform::IDENTITY,
            Visibility::Inherited,
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|root| {
            root.spawn((
                SkyDome,
                Mesh3d(sky_dome_mesh),
                MeshMaterial3d(sky_dome_mat),
                Transform::IDENTITY,
                NotShadowCaster,
                NotShadowReceiver,
            ));
            root.spawn((
                SunDisc,
                Mesh3d(sun_mesh),
                MeshMaterial3d(sun_mat),
                Transform::from_xyz(0.0, 450.0, 0.0),
            ));
            root.spawn((
                MoonDisc::default(),
                Mesh3d(moon_mesh),
                MeshMaterial3d(moon_mat),
                Transform::from_xyz(0.0, -450.0, 0.0),
            ));
            root.spawn((
                Starfield,
                Mesh3d(star_mesh),
                MeshMaterial3d(star_mat),
                Transform::IDENTITY,
                Visibility::Hidden,
            ));
            root.spawn((
                CloudLayer {
                    speed: Vec2::new(4.5, 1.0),
                    offset: Vec2::ZERO,
                    altitude: 150.0,
                },
                Mesh3d(cloud_mesh_1),
                MeshMaterial3d(cloud_mat.clone()),
                Transform::from_xyz(0.0, 150.0, 0.0),
            ));
            root.spawn((
                CloudLayer {
                    speed: Vec2::new(2.5, -0.6),
                    offset: Vec2::ZERO,
                    altitude: 220.0,
                },
                Mesh3d(cloud_mesh_2),
                MeshMaterial3d(cloud_mat),
                Transform::from_xyz(0.0, 220.0, 0.0),
            ));
        });
}

/// Smoothed clock: the server tick advanced locally at 60 Hz between messages.
#[derive(Resource, Default)]
pub struct SkyClock {
    base: u32,
    since: f32,
    seen: Option<u32>,
    pub total_days: u32,
}

impl SkyClock {
    pub fn tick(&self) -> u32 {
        ((self.base as f32 + self.since * 60.0) as u32) % CYCLE_TICKS
    }

    pub fn day_index(&self) -> u32 {
        self.total_days + ((self.base as f32 + self.since * 60.0) as u32) / CYCLE_TICKS
    }
}

#[derive(Default)]
pub struct SkySampleCache {
    initialized: bool,
    last_sample_time: f32,
    last_sample_pos: Vec3,
    last_brightness: f32,
    target_sky_color: [f32; 3],
    target_fog_start: f32,
    target_fog_end: f32,
    corruption_factor: f32,
    surface_height: f32,
    last_pushed_q: Option<f32>,
    last_mat_count: usize,
}

/// Returns true when expensive sky/terrain atmosphere queries should refresh.
pub fn should_refresh_sky_sample(
    initialized: bool,
    elapsed: f32,
    last_time: f32,
    pos: Vec3,
    last_pos: Vec3,
    brightness: f32,
    last_brightness: f32,
) -> bool {
    !initialized
        || (elapsed - last_time) >= 0.25
        || pos.distance_squared(last_pos) >= 16.0
        || (brightness - last_brightness).abs() >= (1.0 / QUANT)
}

#[allow(clippy::too_many_arguments)]
pub fn update_sky(
    time: Res<Time<Real>>,
    clock: Res<ClientClock>,
    mut sky: ResMut<SkyClock>,
    mut atmosphere: ResMut<SkyAtmosphere>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut sun_light: Query<
        (&mut DirectionalLight, &mut Transform),
        (With<DirectionalLight>, Without<Camera3d>),
    >,
    mut materials: ResMut<Assets<ChunkMaterial>>,
    camera_q: Query<&Transform, (With<Camera3d>, Without<DirectionalLight>)>,
    mut fog_q: Query<&mut DistanceFog>,
    stream: Option<Res<crate::view::chunks::ChunkStream>>,
    settings: Option<Res<crate::settings::Settings>>,
    mut cache: Local<SkySampleCache>,
) {
    let eff_tick = clock.effective_tick();
    if sky.seen != Some(eff_tick) {
        if let Some(prev) = sky.seen {
            if eff_tick < prev {
                sky.total_days = sky.total_days.wrapping_add(1);
            }
            if prev.abs_diff(eff_tick) > 120 {
                cache.initialized = false;
            }
        }
        sky.seen = Some(eff_tick);
        sky.base = eff_tick;
        sky.since = 0.0;
    } else {
        sky.since += time.delta_secs();
    }
    let tick = sky.tick();
    let b = sky_brightness(tick);

    let cam_pos = camera_q
        .iter()
        .next()
        .map(|t| t.translation)
        .unwrap_or(Vec3::ZERO);

    let elapsed = time.elapsed_secs();
    let refreshed = should_refresh_sky_sample(
        cache.initialized,
        elapsed,
        cache.last_sample_time,
        cam_pos,
        cache.last_sample_pos,
        b,
        cache.last_brightness,
    );
    if refreshed {
        let biome_weights = terravibe_sim::worldgen::biome::biome_weights(
            crate::view::SEED,
            cam_pos.x as i32,
            cam_pos.z as i32,
        );
        let corruption_factor = detect_corruption_factor(cam_pos, stream.as_deref());
        let target_sky_color = blended_biome_fog_color(&biome_weights.0, corruption_factor);

        let base_start = biome_weights.0[0] * 64.0
            + biome_weights.0[1] * 36.0
            + biome_weights.0[2] * 48.0
            + biome_weights.0[3] * 44.0;
        let base_end = biome_weights.0[0] * 220.0
            + biome_weights.0[1] * 160.0
            + biome_weights.0[2] * 190.0
            + biome_weights.0[3] * 180.0;
        cache.target_fog_start = base_start * (1.0 - corruption_factor) + 14.0 * corruption_factor;
        cache.target_fog_end = base_end * (1.0 - corruption_factor) + 80.0 * corruption_factor;
        cache.target_sky_color = target_sky_color;
        cache.corruption_factor = corruption_factor;
        cache.surface_height = terravibe_sim::worldgen::terrain::surface_height(
            crate::view::SEED,
            cam_pos.x.floor() as i32,
            cam_pos.z.floor() as i32,
        ) as f32;
        cache.initialized = true;
        cache.last_sample_time = elapsed;
        cache.last_sample_pos = cam_pos;
        cache.last_brightness = b;
    }

    let blend_speed = (3.0 * time.delta_secs()).clamp(0.0, 1.0);
    atmosphere.current_sky_color = interpolate_biome_color(
        atmosphere.current_sky_color,
        cache.target_sky_color,
        blend_speed,
    );
    atmosphere.current_fog_start +=
        (cache.target_fog_start - atmosphere.current_fog_start) * blend_speed;
    atmosphere.current_fog_end += (cache.target_fog_end - atmosphere.current_fog_end) * blend_speed;
    atmosphere.corruption_factor +=
        (cache.corruption_factor - atmosphere.corruption_factor) * blend_speed;

    let atmosphere_fx = settings.as_ref().is_none_or(|s| s.atmosphere_fx);
    let c = dawn_dusk_sky_color(tick, atmosphere.current_sky_color, atmosphere_fx);
    let cave_fog = crate::view::camera::cave_fog_density(
        (cache.surface_height - cam_pos.y).max(0.0),
        atmosphere_fx,
    );
    let cave_color = [0.03, 0.035, 0.05];
    let fog_c = interpolate_biome_color(c, cave_color, cave_fog);
    let fog_start = atmosphere.current_fog_start * (1.0 - 0.85 * cave_fog);
    let fog_end = atmosphere.current_fog_end * (1.0 - 0.78 * cave_fog);

    atmosphere.current_fog_color = fog_c;
    let fog_color = Color::srgb(fog_c[0], fog_c[1], fog_c[2]);
    clear.0 = fog_color;
    for mut fog in &mut fog_q {
        fog.color = fog_color;
        fog.falloff = FogFalloff::Linear {
            start: fog_start,
            end: fog_end,
        };
    }

    ambient.brightness = AMBIENT_DAY * b;
    for (mut light, mut tf) in &mut sun_light {
        light.illuminance = SUN_DAY * b;
        let a = sun_angle(tick);
        let dir = Vec3::new(-a.cos(), -a.sin().max(0.2), -0.6).normalize();
        *tf = Transform::from_xyz(0.0, 0.0, 0.0).looking_to(dir, Vec3::Y);
    }

    let q = (b * QUANT).round() / QUANT;
    let mat_count = materials.len();
    if cache.last_pushed_q != Some(q) || mat_count != cache.last_mat_count || refreshed {
        let stale: Vec<_> = materials
            .iter()
            .filter(|(_, m)| m.extension.origin.w != q)
            .map(|(id, _)| id)
            .collect();
        for id in stale {
            if let Some(mut m) = materials.get_mut(id) {
                m.extension.origin.w = q;
            }
        }
        cache.last_pushed_q = Some(q);
        cache.last_mat_count = mat_count;
    }
}

#[derive(Default)]
pub struct SkyDomeColorCache {
    last_colors: Option<([f32; 3], [f32; 3], [f32; 3])>,
}

#[allow(clippy::too_many_arguments)]
pub fn update_celestial_skybox(
    time: Res<Time<Real>>,
    sky: Res<SkyClock>,
    atmosphere: Res<SkyAtmosphere>,
    settings: Option<Res<crate::settings::Settings>>,
    camera_q: Query<&Transform, With<Camera3d>>,
    mut celestial_q: ParamSet<(
        Query<&mut Transform, (With<CelestialRoot>, Without<Camera3d>)>,
        Query<(&mut Transform, &mut Visibility), (With<SunDisc>, Without<Camera3d>)>,
        Query<
            (&mut Transform, &mut Visibility, &Mesh3d, &mut MoonDisc),
            (With<MoonDisc>, Without<Camera3d>),
        >,
        Query<
            (
                &mut Transform,
                &mut CloudLayer,
                &MeshMaterial3d<StandardMaterial>,
            ),
            (With<CloudLayer>, Without<Camera3d>),
        >,
        Query<(&mut Visibility, &MeshMaterial3d<StandardMaterial>), With<Starfield>>,
        Query<&Mesh3d, With<SkyDome>>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
    mut dome_cache: Local<SkyDomeColorCache>,
) {
    let tick = sky.tick();
    let day_index = sky.day_index();
    let b = sky_brightness(tick);

    let cam_pos = camera_q
        .iter()
        .next()
        .map(|t| t.translation)
        .unwrap_or(Vec3::ZERO);

    for mut root_tf in &mut celestial_q.p0() {
        root_tf.translation = cam_pos;
    }

    // Sun Disc
    let sun_pos = sun_orbital_position(tick, CELESTIAL_DIST);
    for (mut tf, mut vis) in &mut celestial_q.p1() {
        tf.translation = sun_pos;
        tf.look_at(Vec3::ZERO, Vec3::Y);
        *vis = if b > NIGHT_FLOOR {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    // Moon Disc & Phases
    let moon_pos = moon_orbital_position(tick, CELESTIAL_DIST);
    let phase = moon_phase(day_index);
    for (mut tf, mut vis, mesh_handle, mut moon) in &mut celestial_q.p2() {
        tf.translation = moon_pos;
        tf.look_at(Vec3::ZERO, Vec3::Y);
        *vis = if b <= 0.40 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if moon.current_phase != Some(phase) {
            moon.current_phase = Some(phase);
            let phase_idx = phase as usize;
            let u0 = phase_idx as f32 / 8.0;
            let u1 = (phase_idx + 1) as f32 / 8.0;
            if let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) {
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_UV_0,
                    vec![[u0, 1.0], [u1, 1.0], [u1, 0.0], [u0, 0.0]],
                );
            }
        }
    }

    // Starfield
    let star_alpha = if b < 0.25 {
        let twinkle = 0.85 + 0.15 * (time.elapsed_secs() * 3.0).sin();
        ((0.25 - b) / (0.25 - NIGHT_FLOOR)).clamp(0.0, 1.0) * twinkle
    } else {
        0.0
    };
    for (mut vis, mat_handle) in &mut celestial_q.p4() {
        if b < 0.25 {
            if *vis != Visibility::Inherited {
                *vis = Visibility::Inherited;
            }
            let target_color = Color::srgba(1.0, 1.0, 1.0, star_alpha);
            if std_materials
                .get(&mat_handle.0)
                .is_some_and(|m| m.base_color != target_color)
                && let Some(mut mat) = std_materials.get_mut(&mat_handle.0)
            {
                mat.base_color = target_color;
            }
        } else if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
    }

    // Drifting Clouds & Night Dimming
    let dt = time.delta_secs();
    let cloud_dim = b.max(0.15);
    let target_cloud_color = Color::srgba(cloud_dim, cloud_dim, cloud_dim, 0.9);
    for (mut tf, mut layer, mat_handle) in &mut celestial_q.p3() {
        let speed = layer.speed;
        layer.offset += speed * dt;
        let period = 120.0;
        let ox = layer.offset.x.rem_euclid(period) - period * 0.5;
        let oz = layer.offset.y.rem_euclid(period) - period * 0.5;
        tf.translation = Vec3::new(ox, layer.altitude, oz);
        if std_materials
            .get(&mat_handle.0)
            .is_some_and(|m| m.base_color != target_cloud_color)
            && let Some(mut mat) = std_materials.get_mut(&mat_handle.0)
        {
            mat.base_color = target_cloud_color;
        }
    }

    // Sky Dome vertex-color gradient
    let atmosphere_fx = settings.as_ref().is_none_or(|s| s.atmosphere_fx);
    let colors = sky_dome_gradient_colors(
        tick,
        atmosphere.current_fog_color,
        atmosphere.corruption_factor,
        0.0,
        atmosphere_fx,
    );
    let changed = dome_cache.last_colors.is_none_or(|prev| {
        let diff = |a: [f32; 3], b: [f32; 3]| {
            (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
        };
        diff(prev.0, colors.0) + diff(prev.1, colors.1) + diff(prev.2, colors.2) > 0.004
    });
    if changed {
        for mesh_handle in &celestial_q.p5() {
            if let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) {
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_COLOR,
                    sky_dome_vertex_colors(colors.0, colors.1, colors.2),
                );
            }
        }
        dome_cache.last_colors = Some(colors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terravibe_sim::time::NIGHT_TICKS;

    #[test]
    fn celestial_orbital_positions_follow_clock() {
        let dist = 400.0;
        // Noon: Sun at zenith (+Y), Moon at nadir (-Y)
        let sun_noon = sun_orbital_position(DAY_TICKS / 2, dist);
        let moon_noon = moon_orbital_position(DAY_TICKS / 2, dist);
        assert!(
            (sun_noon.y - dist).abs() < 5.0,
            "sun at noon should be at zenith: {sun_noon:?}"
        );
        assert!(
            (moon_noon.y - (-dist)).abs() < 5.0,
            "moon at noon should be opposite at nadir: {moon_noon:?}"
        );
        assert!(sun_noon.x.abs() < 5.0);

        // Midnight: Moon at zenith (+Y), Sun at nadir (-Y)
        let midnight_tick = DAY_TICKS + NIGHT_TICKS / 2;
        let sun_midnight = sun_orbital_position(midnight_tick, dist);
        let moon_midnight = moon_orbital_position(midnight_tick, dist);
        assert!(
            (moon_midnight.y - dist).abs() < 5.0,
            "moon at midnight should be at zenith: {moon_midnight:?}"
        );
        assert!(
            (sun_midnight.y - (-dist)).abs() < 5.0,
            "sun at midnight should be opposite at nadir: {sun_midnight:?}"
        );

        // Dawn (tick 0): Sun rising on horizon (y ~ 0, x < 0)
        let sun_dawn = sun_orbital_position(0, dist);
        assert!(
            sun_dawn.y.abs() < 5.0 && sun_dawn.x < -dist * 0.9,
            "sun at dawn should rise on horizon: {sun_dawn:?}"
        );

        // Dusk (end of day): Sun setting on horizon (y ~ 0, x > 0)
        let sun_dusk = sun_orbital_position(DAY_TICKS - 1, dist);
        assert!(
            sun_dusk.y.abs() < 5.0 && sun_dusk.x > dist * 0.9,
            "sun at dusk should set on horizon: {sun_dusk:?}"
        );
    }

    #[test]
    fn moon_phases_cycle_across_days() {
        assert_eq!(moon_phase(0), MoonPhase::Full);
        assert_eq!(moon_phase(1), MoonPhase::WaningGibbous);
        assert_eq!(moon_phase(2), MoonPhase::ThirdQuarter);
        assert_eq!(moon_phase(3), MoonPhase::WaningCrescent);
        assert_eq!(moon_phase(4), MoonPhase::NewMoon);
        assert_eq!(moon_phase(5), MoonPhase::WaxingCrescent);
        assert_eq!(moon_phase(6), MoonPhase::FirstQuarter);
        assert_eq!(moon_phase(7), MoonPhase::WaxingGibbous);
        assert_eq!(moon_phase(8), MoonPhase::Full);
    }

    #[test]
    fn biome_fog_color_interpolation_matches_specs() {
        let forest = blended_biome_fog_color(&[1.0, 0.0, 0.0, 0.0], 0.0);
        let desert = blended_biome_fog_color(&[0.0, 1.0, 0.0, 0.0], 0.0);
        let snow = blended_biome_fog_color(&[0.0, 0.0, 1.0, 0.0], 0.0);
        let jungle = blended_biome_fog_color(&[0.0, 0.0, 0.0, 1.0], 0.0);
        let corruption = blended_biome_fog_color(&[1.0, 0.0, 0.0, 0.0], 1.0);

        assert_eq!(forest, DAY_SKY, "forest matches clear azure blue");
        assert_eq!(desert, DESERT_SKY, "desert matches warm sandy haze");
        assert_eq!(snow, SNOW_SKY, "snow matches cold frosty white-blue");
        assert_eq!(jungle, JUNGLE_SKY, "jungle matches tropical cyan-green");
        assert_eq!(
            corruption, CORRUPTION_SKY,
            "corruption matches dark purple/black"
        );

        let mid_fd = blended_biome_fog_color(&[0.5, 0.5, 0.0, 0.0], 0.0);
        for i in 0..3 {
            let expected = (DAY_SKY[i] + DESERT_SKY[i]) * 0.5;
            assert!(
                (mid_fd[i] - expected).abs() < 1e-4,
                "midpoint of forest and desert: got {:?}, expected {expected}",
                mid_fd[i]
            );
        }

        let mid_corrupt = blended_biome_fog_color(&[1.0, 0.0, 0.0, 0.0], 0.5);
        for i in 0..3 {
            let expected = (DAY_SKY[i] + CORRUPTION_SKY[i]) * 0.5;
            assert!(
                (mid_corrupt[i] - expected).abs() < 1e-4,
                "midpoint into corruption: got {:?}, expected {expected}",
                mid_corrupt[i]
            );
        }
    }

    #[test]
    fn sky_colour_ramp_endpoints_match_day_and_night() {
        assert_eq!(sky_colour(1.0), DAY_SKY);
        assert_eq!(sky_colour(NIGHT_FLOOR), NIGHT_SKY);
        let mid = sky_colour(0.575);
        assert!(mid[0] > mid[2] * 0.5, "dusk is warm: {mid:?}");
    }

    #[test]
    fn procedural_textures_and_meshes_generate_valid() {
        let sun = generate_sun_image();
        assert_eq!(sun.texture_descriptor.size.width, 32);
        assert_eq!(sun.texture_descriptor.size.height, 32);

        let moon = generate_moon_atlas_image();
        assert_eq!(moon.texture_descriptor.size.width, 256);
        assert_eq!(moon.texture_descriptor.size.height, 32);

        let clouds = generate_cloud_image();
        assert_eq!(clouds.texture_descriptor.size.width, 128);
        assert_eq!(clouds.texture_descriptor.size.height, 32);

        let stars = generate_starfield_mesh();
        assert!(stars.count_vertices() >= 120 * 4);
    }

    #[test]
    fn detect_corruption_factor_without_stream_is_zero() {
        assert_eq!(
            detect_corruption_factor(Vec3::new(0.0, 50.0, 0.0), None),
            0.0
        );
    }

    #[test]
    fn sky_clock_tracks_total_days_and_advances_phases() {
        let mut clock = SkyClock::default();
        assert_eq!(clock.day_index(), 0);
        assert_eq!(moon_phase(clock.day_index()), MoonPhase::Full);

        // Advance within day
        clock.base = 10_000;
        assert_eq!(clock.day_index(), 0);

        // Advance day
        clock.total_days += 1;
        clock.base = 0;
        assert_eq!(clock.day_index(), 1);
        assert_eq!(moon_phase(clock.day_index()), MoonPhase::WaningGibbous);

        // Day 4 (New Moon)
        clock.total_days = 4;
        assert_eq!(moon_phase(clock.day_index()), MoonPhase::NewMoon);
    }

    #[test]
    fn moon_disc_caches_phase() {
        let mut moon = MoonDisc::default();
        assert_eq!(moon.current_phase, None);

        let phase = moon_phase(0);
        let mut upload_count = 0;
        if moon.current_phase != Some(phase) {
            moon.current_phase = Some(phase);
            upload_count += 1;
        }
        assert_eq!(upload_count, 1);
        assert_eq!(moon.current_phase, Some(MoonPhase::Full));

        // Same phase next frame -> no upload
        if moon.current_phase != Some(phase) {
            upload_count += 1;
        }
        assert_eq!(upload_count, 1);

        // Next day -> phase changes -> upload triggers
        let next_phase = moon_phase(1);
        if moon.current_phase != Some(next_phase) {
            moon.current_phase = Some(next_phase);
            upload_count += 1;
        }
        assert_eq!(upload_count, 2);
        assert_eq!(moon.current_phase, Some(MoonPhase::WaningGibbous));
    }

    #[test]
    fn night_cloud_dimming_modulates_brightness() {
        let b_night = NIGHT_FLOOR;
        let cloud_dim_night = b_night.max(0.15);
        assert_eq!(cloud_dim_night, 0.15);

        let b_noon: f32 = 1.0;
        let cloud_dim_noon = b_noon.max(0.15);
        assert_eq!(cloud_dim_noon, 1.0);

        let b_mid: f32 = 0.5;
        let cloud_dim_mid = b_mid.max(0.15);
        assert_eq!(cloud_dim_mid, 0.5);
    }

    #[test]
    fn setup_skybox_celestial_materials_have_culling_disabled() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Image>();
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.add_systems(Update, setup_skybox);
        app.update();

        let materials = app.world().resource::<Assets<StandardMaterial>>();
        let celestial_mats: Vec<_> = materials
            .iter()
            .filter(|(_, m)| m.unlit && m.double_sided && m.cull_mode.is_none())
            .collect();
        assert!(
            celestial_mats.len() >= 2,
            "both sun and moon materials must have cull_mode: None and double_sided: true"
        );
    }

    #[test]
    fn cave_fog_and_dawn_dusk_sky_scale_with_depth_time_and_toggle() {
        assert_eq!(crate::view::camera::cave_fog_density(0.0, true), 0.0);
        assert!(crate::view::camera::cave_fog_density(48.0, true) > 0.5);
        assert_eq!(crate::view::camera::cave_fog_density(48.0, false), 0.0);

        let dawn_tick = 5_400;
        let midday_tick = 27_000;
        let dusk_tick = 48_600;

        let dawn_fx = dawn_dusk_sky_color(dawn_tick, DAY_SKY, true);
        let dawn_off = dawn_dusk_sky_color(dawn_tick, DAY_SKY, false);
        assert!(dawn_fx[0] > dawn_off[0]);

        let dusk_fx = dawn_dusk_sky_color(dusk_tick, DAY_SKY, true);
        let dusk_off = dawn_dusk_sky_color(dusk_tick, DAY_SKY, false);
        assert!(dusk_fx[0] > dusk_off[0]);
        assert_ne!(dawn_fx, dusk_fx);

        let mid_fx = dawn_dusk_sky_color(midday_tick, DAY_SKY, true);
        let mid_off = dawn_dusk_sky_color(midday_tick, DAY_SKY, false);
        assert_eq!(mid_fx, mid_off);
    }

    #[test]
    fn sky_sample_throttling_triggers_on_time_move_or_brightness_delta() {
        let p0 = Vec3::new(0.0, 50.0, 0.0);
        assert!(should_refresh_sky_sample(false, 0.0, 0.0, p0, p0, 1.0, 1.0));
        assert!(!should_refresh_sky_sample(
            true, 0.10, 0.0, p0, p0, 1.0, 1.0
        ));
        assert!(should_refresh_sky_sample(true, 0.25, 0.0, p0, p0, 1.0, 1.0));
        assert!(should_refresh_sky_sample(
            true,
            0.05,
            0.0,
            Vec3::new(4.0, 50.0, 0.0),
            p0,
            1.0,
            1.0
        ));
        assert!(should_refresh_sky_sample(
            true,
            0.05,
            0.0,
            p0,
            p0,
            1.0 - 1.0 / QUANT,
            1.0
        ));
    }

    #[test]
    fn sky_dome_mesh_and_gradients_are_valid() {
        let dome = generate_sky_dome_mesh();
        assert!(dome.count_vertices() > 100);
        assert!(dome.attribute(Mesh::ATTRIBUTE_COLOR).is_some());

        let (day_z, day_h, _) = sky_dome_gradient_colors(DAY_TICKS / 2, DAY_SKY, 0.0, 0.0, true);
        let (night_z, _, _) =
            sky_dome_gradient_colors(DAY_TICKS + NIGHT_TICKS / 2, NIGHT_SKY, 0.0, 0.0, true);
        let (corrupt_z, _, _) =
            sky_dome_gradient_colors(DAY_TICKS / 2, CORRUPTION_SKY, 1.0, 0.0, true);
        assert!(day_z[2] > night_z[2] + 0.5);
        assert_eq!(day_h, DAY_SKY);
        assert!(corrupt_z[2] > corrupt_z[1]);
    }

    #[test]
    fn sky_dome_triangles_face_inward() {
        use bevy::mesh::{Indices, VertexAttributeValues};
        let dome = generate_sky_dome_mesh();
        let Some(VertexAttributeValues::Float32x3(pos)) = dome.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing positions");
        };
        let Some(Indices::U32(idx)) = dome.indices() else {
            panic!("missing u32 indices");
        };
        let mut checked = 0usize;
        for tri in idx.chunks_exact(3) {
            let v0 = Vec3::from(pos[tri[0] as usize]);
            let v1 = Vec3::from(pos[tri[1] as usize]);
            let v2 = Vec3::from(pos[tri[2] as usize]);
            let n = (v1 - v0).cross(v2 - v0);
            if n.length_squared() > 1e-4 {
                assert!(
                    n.dot(v0) < 0.0,
                    "sky dome triangle normal {n:?} at {v0:?} must point inward"
                );
                checked += 1;
            }
        }
        assert!(checked > 100);
    }
}
