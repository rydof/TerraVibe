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
@group(3) @binding(105) var<uniform> liquid_params: vec4<f32>;

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn vnoise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm2(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 3; i = i + 1) {
        v += amp * vnoise2(q);
        q = q * 2.07 + vec2<f32>(17.3, 9.2);
        amp *= 0.5;
    }
    return v;
}

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

    if (liquid_params.x > 0.5) {
        let t = liquid_params.y;
        let wp = in.world_position.xyz;
        let is_top_or_bottom = abs(in.world_normal.y) > 0.5;
        let uv_l = select(
            vec2<f32>(wp.x + wp.z, wp.y + t * 0.75),
            wp.xz,
            is_top_or_bottom,
        );

        // Lava uses opaque vertex alpha (1.0), water uses translucent vertex alpha (~0.65).
        if (pbr_input.material.base_color.a > 0.9) {
            let p1 = uv_l * 0.85 + vec2<f32>(t * 0.16, -t * 0.11);
            let warp = vec2<f32>(fbm2(p1), fbm2(p1 + vec2<f32>(5.2, 1.3)));
            let n = fbm2(uv_l * 0.95 + warp * 1.45 + vec2<f32>(-t * 0.10, t * 0.14));
            let pulse = 0.06 * sin(t * 2.2 + uv_l.x * 1.3 + uv_l.y * 1.1);
            let heat = clamp(n + pulse, 0.0, 1.0);

            // Multi-stop molten gradient: cooling basalt crust -> crimson -> orange -> white-hot core.
            let c_crust = vec3<f32>(0.22, 0.03, 0.01);
            let c_deep = vec3<f32>(0.92, 0.18, 0.015);
            let c_hot = vec3<f32>(1.55, 0.58, 0.05);
            let c_core = vec3<f32>(2.20, 1.35, 0.35);

            var magma = mix(c_crust, c_deep, smoothstep(0.24, 0.45, heat));
            magma = mix(magma, c_hot, smoothstep(0.45, 0.70, heat));
            magma = mix(magma, c_core, smoothstep(0.70, 0.88, heat));

            var out: FragmentOutput;
            out.color = main_pass_post_lighting_processing(pbr_input, vec4<f32>(magma, 1.0));
            return out;
        } else {
            let ripple_a = vnoise2(uv_l * 1.6 + vec2<f32>(t * 0.55, t * 0.35));
            let ripple_b = vnoise2(uv_l * 2.3 - vec2<f32>(t * 0.42, -t * 0.48));
            let wave = (ripple_a + ripple_b) * 0.5;
            let foam = smoothstep(0.74, 0.92, wave) * 0.18;
            let tint = 0.88 + wave * 0.28;
            pbr_input.material.base_color = vec4<f32>(
                (pbr_input.material.base_color.rgb * tint + vec3<f32>(foam * 0.5, foam * 0.8, foam)) * block_light,
                clamp(pbr_input.material.base_color.a + foam * 0.25, 0.55, 0.85),
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
    }

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
