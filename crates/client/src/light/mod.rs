//! Terraria-style block lighting: a pure per-chunk light volume computed on a
//! worker thread and sampled by the chunk shader.
//!
//! Light is view state: the server never sends it. Each chunk's volume is
//! computed from its own blocks plus a padded box of neighbours, so a torch
//! near a border lights the chunk on the other side of it.

pub mod engine;
pub mod material;

pub use engine::{LightVolume, light_chunk, relight_below, relight_below_edit, relight_targets};
pub use material::{ChunkMaterial, LightExt};

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Chunk block side length. `LightVolume::get` and the visible interior keep
/// these 32³ semantics.
pub const LIGHT_SIZE: u32 = 32;
/// One texel of border around the chunk volume. A face on a chunk boundary
/// samples the air cell in the neighbouring chunk; with a bare 32³,
/// clamp-to-edge would sample the edge block itself and draw a dark seam.
pub const LIGHT_BORDER: u32 = 1;
/// Side of the uploaded 3D texture: the 32³ chunk plus one texel of border on
/// every side. Must match the `/ 34.0` in `shaders/chunk_light.wesl`.
pub const LIGHT_IMAGE_SIZE: u32 = LIGHT_SIZE + 2 * LIGHT_BORDER;

/// Packs the padded light volume into the `Rgba8Unorm` byte buffer uploaded to
/// the GPU. Kept separate from [`light_image`] so tests can inspect the bytes
/// without reading back render-world-only image data.
pub fn light_image_bytes(v: &LightVolume) -> Vec<u8> {
    let texels = (LIGHT_IMAGE_SIZE * LIGHT_IMAGE_SIZE * LIGHT_IMAGE_SIZE) as usize;
    assert_eq!(
        v.data.len(),
        texels,
        "light volume must be {LIGHT_IMAGE_SIZE}³"
    );
    let mut data = Vec::with_capacity(texels * 4);
    for rgba in &v.data {
        data.extend_from_slice(rgba);
    }
    data
}

/// Converts a light volume to the uploaded 34³ `Rgba8Unorm` image with linear
/// filtering and clamp-to-edge, ready for the chunk material's 3D texture.
/// The CPU copy is discarded once the render world has the texture.
pub fn light_image(v: &LightVolume) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: LIGHT_IMAGE_SIZE,
            height: LIGHT_IMAGE_SIZE,
            depth_or_array_layers: LIGHT_IMAGE_SIZE,
        },
        TextureDimension::D3,
        light_image_bytes(v),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = bevy::image::ImageSampler::linear();
    image
}

/// Full-white 1³ texture used until a chunk's light volume is ready, so a
/// newly spawned chunk is never black.
pub fn default_light_image() -> Image {
    let mut image = Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D3,
        vec![255, 255, 255, 255],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = bevy::image::ImageSampler::linear();
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texel(x: i32, y: i32, z: i32) -> usize {
        let side = LIGHT_IMAGE_SIZE as i32;
        ((x + LIGHT_BORDER as i32)
            + (y + LIGHT_BORDER as i32) * side
            + (z + LIGHT_BORDER as i32) * side * side) as usize
    }

    fn padded_volume() -> LightVolume {
        let texels = (LIGHT_IMAGE_SIZE * LIGHT_IMAGE_SIZE * LIGHT_IMAGE_SIZE) as usize;
        LightVolume {
            data: vec![[0, 0, 0, 0]; texels],
        }
    }

    #[test]
    fn light_image_layout() {
        let mut v = padded_volume();
        let (x, y, z) = (3i32, 5i32, 7i32);
        v.data[texel(x, y, z)] = [10, 20, 30, 40];
        // The border texels are the padded volume's local -1 plane.
        v.data[texel(-1, 0, 0)] = [1, 2, 3, 4];
        v.data[texel(0, -1, 0)] = [5, 6, 7, 8];
        v.data[texel(0, 0, -1)] = [9, 10, 11, 12];

        let data = light_image_bytes(&v);
        assert_eq!(
            data.len(),
            (LIGHT_IMAGE_SIZE * LIGHT_IMAGE_SIZE * LIGHT_IMAGE_SIZE * 4) as usize
        );
        let i = texel(x, y, z) * 4;
        assert_eq!(&data[i..i + 4], &[10, 20, 30, 40]);
        let b = texel(-1, 0, 0) * 4;
        assert_eq!(&data[b..b + 4], &[1, 2, 3, 4]);

        let image = light_image(&v);
        assert_eq!(image.texture_descriptor.size.width, 34);
        assert_eq!(image.texture_descriptor.size.height, 34);
        assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 34);
        assert_eq!(image.asset_usage, RenderAssetUsages::RENDER_WORLD);
    }

    #[test]
    fn light_texture_side_matches_shader() {
        assert_eq!(LIGHT_IMAGE_SIZE, 34);
        assert_eq!(LIGHT_SIZE + 2 * LIGHT_BORDER, LIGHT_IMAGE_SIZE);
        // Shader WESL cannot see Rust consts; the literal must stay in sync.
        assert!(
            include_str!("shaders/chunk_light.wesl").contains("/ 34.0"),
            "chunk_light.wesl must divide by the padded texture side"
        );
    }

    #[test]
    fn default_light_is_bright() {
        let image = default_light_image();
        assert_eq!(image.texture_descriptor.size.width, 1);
        assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 1);
        assert_eq!(image.asset_usage, RenderAssetUsages::RENDER_WORLD);
    }
}
