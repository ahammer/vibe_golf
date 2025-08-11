/* Simplified terrain PBR extension fragment shader.
   Goal: reduce complexity & avoid NaN / unexpected color artifacts (purple).
   Removes macro/micro animated noise & gamma shaping; reintroduces simple saturation & contrast.
   Biomes repurposed: snow layer now high rocky/sandy tone (no white peaks).
   Keeps basic biome blending (lowland / grass / rock / highrock) + roughness blend. */
//
// NOTE: Uniform struct kept identical for compatibility, many params now unused safely.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
}
#endif

struct RealTerrainExtendedMaterial {
    min_height: f32,
    max_height: f32,
    rock_slope_start: f32,
    snow_height_start: f32,
    snow_height_end: f32,
    time: f32,
    noise_scale: f32,
    _pad1: f32,
    colors: array<vec4<f32>, 4u>,  // lowland, grass, rock, snow (rgba)
    roughness_lowland: f32,
    roughness_grass: f32,
    roughness_rock: f32,
    roughness_snow: f32,
    color_variation: f32,
    ao_strength: f32,
    brightness: f32,
    contrast: f32,
    saturation: f32,
    macro_amp: f32,
    micro_amp: f32,
    edge_accent: f32,
    gamma: f32,
    macro_scale: f32,
    micro_scale: f32,
    animation_speed: f32,
};

@group(2) @binding(100)
var<uniform> realterrain_extended_material: RealTerrainExtendedMaterial;

// Tiny hash / noise retained only for subtle grass/lowland breakup (very mild).
fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn renorm4(w: vec4<f32>) -> vec4<f32> {
    let s = max(1e-4, w.x + w.y + w.z + w.w);
    return w / s;
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Normalize height 0..1
    let h_denom = max(0.0001, realterrain_extended_material.max_height - realterrain_extended_material.min_height);
    let h_norm = clamp((in.world_position.y - realterrain_extended_material.min_height) / h_denom, 0.0, 1.0);

    // Slope (0 flat, 1 steep)
    let slope = clamp(1.0 - clamp(in.world_normal.y, 0.0, 1.0), 0.0, 1.0);

    // Snow (height band)
    let snow_w = smoothstep(realterrain_extended_material.snow_height_start,
                            realterrain_extended_material.snow_height_end,
                            h_norm);

    // Rock (slope, suppressed by snow)
    let rock_edge = realterrain_extended_material.rock_slope_start;
    let rock_w_raw = smoothstep(rock_edge, rock_edge + 0.15, slope);
    let rock_w = rock_w_raw * (1.0 - snow_w);

    // Lowland near base heights
    let lowland_raw = 1.0 - smoothstep(0.15, 0.35, h_norm);
    let lowland_w = lowland_raw * (1.0 - snow_w);

    // Remaining => grass
    var grass_w = 1.0 - (lowland_w + rock_w + snow_w);
    grass_w = max(0.0, grass_w);

    // Mild low-frequency noise to break perfect contours (very small amplitude)
    let npos = in.world_position.xz * (realterrain_extended_material.noise_scale * 0.5 + 0.0005);
    let n = noise(npos);
    let v = (n - 0.5) * realterrain_extended_material.color_variation * 0.3;

    // Perturb lowland / grass slightly then renormalize
    let lowland_p = max(0.0, lowland_w * (1.0 + v));
    let grass_p   = max(0.0, grass_w   * (1.0 - v));
    var weights = vec4<f32>(lowland_p, grass_p, rock_w, snow_w);
    weights = renorm4(weights);

    // Palette
    let c_low  = realterrain_extended_material.colors[0u].rgb;
    let c_grass= realterrain_extended_material.colors[1u].rgb;
    let c_rock = realterrain_extended_material.colors[2u].rgb;
    let c_snow = realterrain_extended_material.colors[3u].rgb;

    var base_col = c_low * weights.x +
                   c_grass * weights.y +
                   c_rock * weights.z +
                   c_snow * weights.w;

    // Simple ambient occlusion heuristic: more shadow in steeper + lower areas
    let cavity = clamp((slope * 0.6) + (1.0 - h_norm) * 0.25, 0.0, 1.0);
    let ao = mix(1.0, cavity, realterrain_extended_material.ao_strength);
    base_col *= ao;

    // Brightness then apply saturation & contrast for deeper, bolder hues
    base_col *= realterrain_extended_material.brightness;
    let avg = (base_col.r + base_col.g + base_col.b) / 3.0;
    base_col = mix(vec3<f32>(avg), base_col, realterrain_extended_material.saturation);
    base_col = (base_col - vec3<f32>(0.5)) * realterrain_extended_material.contrast + vec3<f32>(0.5);

    // Edge accent (darken steep/high transitional surfaces slightly)
    let edge_factor = clamp(slope * 0.5 + smoothstep(0.55, 0.75, h_norm) * 0.3, 0.0, 1.0);
    base_col *= (1.0 - edge_factor * realterrain_extended_material.edge_accent);

    // Final clamp to guarantee finite color (avoid NaNs / purples).
    base_col = clamp(base_col, vec3<f32>(0.0), vec3<f32>(1.0));

    // Blend roughness
    let r_low  = realterrain_extended_material.roughness_lowland;
    let r_grass= realterrain_extended_material.roughness_grass;
    let r_rock = realterrain_extended_material.roughness_rock;
    let r_snow = realterrain_extended_material.roughness_snow;
    let rough = r_low * weights.x + r_grass * weights.y + r_rock * weights.z + r_snow * weights.w;
    pbr_input.material.perceptual_roughness = clamp(rough, 0.04, 1.0);

    // Assign
    pbr_input.material.base_color = vec4<f32>(base_col, pbr_input.material.base_color.a);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
    return out;
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
#endif
}
