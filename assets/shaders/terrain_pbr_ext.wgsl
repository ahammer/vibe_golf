// Realistic terrain PBR extension fragment shader.
//
// Strategy:
//  - Keep full PBR lighting & shadows from StandardMaterial
//  - Derive biome weights from normalized elevation + slope
//  - Four layers: lowland, grass, rock, snow
//  - Smooth transitions using smoothstep & renormalization
//  - Cheap procedural noise to break up banding (& subtle wind tint time variation)
//  - Blend roughness per-layer
//
// Expected Rust uniform: RealTerrainUniform (mirrors this struct).
//
// Vertex data needed: position, normal, uv (optional color ignored here for portability).
// If a COLOR vertex attribute is present, Bevy will pass it (we attempt to read, but fall back if absent).
//
// NOTE: This shader only adjusts base_color and perceptual_roughness before lighting.
// Metallic remains from base StandardMaterial (defaults to 0).
//
// If you later add splat textures / normal maps, they can be integrated here by sampling
// from an atlas using biome weights.

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

// Must match RealTerrainUniform layout.
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
    _pad2: vec2<f32>,
};

@group(2) @binding(100)
var<uniform> realterrain_extended_material: RealTerrainExtendedMaterial;

// Hash / noise helpers (cheap, non-periodic).
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

// Smooth weight normalizer
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

    // Height normalization
    let h_denom = max(0.0001, realterrain_extended_material.max_height - realterrain_extended_material.min_height);
    let h_norm = clamp((in.world_position.y - realterrain_extended_material.min_height) / h_denom, 0.0, 1.0);

    // Slope (0 flat -> 1 steep)
    let slope = clamp(1.0 - clamp(in.world_normal.y, 0.0, 1.0), 0.0, 1.0);

    // Snow weight (height-based, smooth over configured band)
    let snow_w = smoothstep(realterrain_extended_material.snow_height_start,
                            realterrain_extended_material.snow_height_end,
                            h_norm);

    // Rock weight (slope-based, suppressed by snow)
    let rock_edge = realterrain_extended_material.rock_slope_start;
    let rock_w_raw = smoothstep(rock_edge, rock_edge + 0.15, slope);
    let rock_w = rock_w_raw * (1.0 - snow_w);

    // Lowland emphasis near bottom 0..~0.25
    let lowland_raw = 1.0 - smoothstep(0.15, 0.35, h_norm);
    let lowland_w = lowland_raw * (1.0 - snow_w);

    // Grass provisional (will renormalize)
    var grass_w = 1.0 - (lowland_w + rock_w + snow_w);
    grass_w = max(0.0, grass_w);

    // Procedural noise to break up transitions
    let npos = in.world_position.xz * realterrain_extended_material.noise_scale;
    let t = realterrain_extended_material.time;
    let wind_shift = vec2<f32>(0.07 * t, 0.05 * t);
    let n = noise(npos + wind_shift);
    let n2 = noise((npos + wind_shift * 1.7) * 1.9);

    let variation = (n * 0.6 + n2 * 0.4) * 2.0 - 1.0; // -1..1
    let v_amp = realterrain_extended_material.color_variation;

    // Apply small perturbations to weights pre-renormalization (only grass/lowland for "patchiness")
    let lowland_w_p = lowland_w * (1.0 + variation * v_amp * 0.5);
    let grass_w_p   = grass_w   * (1.0 - variation * v_amp * 0.5);

    var weights = vec4<f32>(lowland_w_p, grass_w_p, rock_w, snow_w);
    weights = max(weights, vec4<f32>(0.0));
    weights = renorm4(weights);

    // Fetch palette colors
    let c_low  = realterrain_extended_material.colors[0u].rgb;
    let c_grass= realterrain_extended_material.colors[1u].rgb;
    let c_rock = realterrain_extended_material.colors[2u].rgb;
    let c_snow = realterrain_extended_material.colors[3u].rgb;

    var base_col = c_low * weights.x +
                   c_grass * weights.y +
                   c_rock * weights.z +
                   c_snow * weights.w;

    // Subtle ambient occlusion from slope + inverse height (heuristic)
    let cavity = clamp((slope * 0.6) + (1.0 - h_norm) * 0.25, 0.0, 1.0);
    let ao = mix(1.0, cavity, realterrain_extended_material.ao_strength);

    // Apply AO pre-lighting
    base_col *= ao;

    // Slight tone mapping / soft contrast
    let avg = (base_col.r + base_col.g + base_col.b) / 3.0;
    base_col = mix(vec3<f32>(avg), base_col, 1.10); // saturation boost

    // Blend roughness
    let r_low  = realterrain_extended_material.roughness_lowland;
    let r_grass= realterrain_extended_material.roughness_grass;
    let r_rock = realterrain_extended_material.roughness_rock;
    let r_snow = realterrain_extended_material.roughness_snow;
    let rough = r_low * weights.x + r_grass * weights.y + r_rock * weights.z + r_snow * weights.w;

    // Set material overrides
    pbr_input.material.base_color = vec4<f32>(base_col, pbr_input.material.base_color.a);
    pbr_input.material.perceptual_roughness = clamp(rough, 0.04, 1.0);

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
