// Chunk fragment shader: standard PBR, with the sampled block light multiplied
// into the base colour. The light texture is 34³: the 32³ chunk volume with one
// texel of border on every side, so a face on a chunk boundary samples the air
// cell in the neighbouring chunk instead of clamping to the edge block. The
// sampled cell is half a block along the face normal, so a wall is lit like the
// air it faces, as Terraria's tiles are.
//
// `origin` is the padded minimum corner (chunk minimum - 1 block), which puts
// texel centres at world `origin + (i + 0.5)`. 34.0 must match
// LIGHT_IMAGE_SIZE in light/mod.rs.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
};

// Group 3 is Bevy 0.20's material bind group (`MATERIAL_BIND_GROUP_INDEX`).
@group(3) @binding(100) var light_texture: texture_3d<f32>;
@group(3) @binding(101) var light_sampler: sampler;
@group(3) @binding(102) var<uniform> light_origin: vec4<f32>;
@group(3) @binding(103) var tiles_texture: texture_2d_array<f32>;
@group(3) @binding(104) var tiles_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    @if(VERTEX_UVS_B) {
        let tile_layer = i32(round(in.uv_b.x));
        let tile_color = textureSampleLevel(tiles_texture, tiles_sampler, in.uv, tile_layer, 0.0);
        @if(MAY_DISCARD) {
            if (tile_color.a < pbr_input.material.alpha_cutoff) {
                discard;
            }
        }
        pbr_input.material.base_color = vec4(
            tile_color.rgb * pbr_input.material.base_color.rgb,
            tile_color.a * pbr_input.material.base_color.a,
        );
    }

    let light_uvw =
        (in.world_position.xyz - light_origin.xyz + in.world_normal.xyz * 0.5) / 34.0;
    // rgb = max(sky, block) per channel; a = sky-only. `light_origin.w` is the
    // global sky brightness (day/night). Where block light exceeds the sky it
    // is kept as is; otherwise the sky share is scaled, so a time change is a
    // single uniform write and never a relight.
    let texel = textureSampleLevel(light_texture, light_sampler, light_uvw, 0.0);
    let sky = texel.a;
    let block_light = select(
        vec3(sky * light_origin.w),
        texel.rgb,
        texel.rgb > vec3(sky + 0.004),
    );
    pbr_input.material.base_color = vec4(
        pbr_input.material.base_color.rgb * block_light,
        pbr_input.material.base_color.a,
    );

    var out: FragmentOutput;
    if ((pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u) {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
