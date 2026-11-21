//! The chunk material: `StandardMaterial` plus a per-chunk 3D light texture.
//!
//! Each chunk gets its own material instance because each has its own 32³
//! light volume. `origin` is the chunk's minimum world corner, so the shader
//! can turn a world position into texture space without per-vertex attributes.

use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

pub type ChunkMaterial = ExtendedMaterial<StandardMaterial, LightExt>;

/// The fragment extension's bind group data. Bindings start at 100; the base
/// material owns 0..100, and the material bind group is group 3 in Bevy 0.20.
#[derive(Asset, AsBindGroup, TypePath, Clone, Debug)]
pub struct LightExt {
    /// 34³ `Rgba8Unorm` light volume: the chunk plus one border texel on every
    /// side (`x + y*34 + z*34*34`, alpha unused).
    #[texture(100, dimension = "3d")]
    #[sampler(101)]
    pub light: Handle<Image>,
    /// Minimum *padded* corner of the light texture in world space, i.e. the
    /// chunk minimum minus one block; `w` unused.
    #[uniform(102)]
    pub origin: Vec4,
    /// `Texture2DArray` of 16x16 block tiles (sampled when `VERTEX_UVS_B` is present).
    #[texture(103, dimension = "2d_array")]
    #[sampler(104)]
    pub tiles: Handle<Image>,
    /// Liquid rendering parameters: `x > 0.5` marks liquid chunks, `y` = elapsed time (seconds).
    #[uniform(105)]
    pub liquid_params: Vec4,
}

impl MaterialExtension for LightExt {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(
            bevy::asset::AssetPath::from_path_buf(bevy::asset::embedded_path!(
                "shaders/chunk_light.wesl"
            ))
            .with_source("embedded"),
        )
    }

    fn deferred_fragment_shader() -> ShaderRef {
        self::LightExt::fragment_shader()
    }
}

/// Embeds the shader and registers the material type. Called from the chunks
/// plugin so the shader is available before the first chunk is drawn.
pub fn register(app: &mut App) {
    bevy::asset::embedded_asset!(app, "shaders/chunk_light.wesl");
    app.add_plugins(bevy::pbr::MaterialPlugin::<ChunkMaterial>::default());
}
