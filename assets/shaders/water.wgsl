// Ocean-style water fragment shader using Bevy PBR pipeline (no custom vertex stage).
// We only adjust the StandardMaterial base_color in the fragment based on
// procedural wave functions + fresnel. Geometry stays flat (large quad).
//
// NOTE: This replaces the previous custom vertex/fragment pair that caused a
// pipeline mismatch. By using the standard PBR vertex stage we avoid IO mismatches.
//
// Rust side: WaterMaterial uniform at @group(2) @binding(0)
// (auto layout produced by AsBindGroup with #[uniform(0)] on params).

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
}

// Must match WaterParams / WaterMaterial uniform layout in Rust (see terrain.rs).
struct WaterMaterial {
    time: f32,
    wave_amp: f32,
    wave_len: f32,
    wave_speed: f32,
    fresnel_power: f32,
    color_deep: vec4<f32>,
    color_shallow: vec4<f32>,
};

@group(2) @binding(0)
var<uniform> water_material: WaterMaterial;

// Simple 2-direction blended sine waves (height only used for color modulation here).
fn wave_field(p: vec2<f32>, t: f32) -> f32 {
    let k1 = 6.28318 / max(0.0001, water_material.wave_len);
    let k2 = 6.28318 / max(0.0001, water_material.wave_len * 0.47);
    let d1 = normalize(vec2<f32>(0.82, 0.54));
    let d2 = normalize(vec2<f32>(-0.35, 0.93));
    let ph1 = dot(p, d1) * k1 + t * water_material.wave_speed * 0.85;
    let ph2 = dot(p, d2) * k2 - t * water_material.wave_speed * 1.35;
    let h = sin(ph1) + 0.55 * sin(ph2 + 0.75);
    return h * water_material.wave_amp;
}

// Cheap normal approximation from derivative of the wave field (for fresnel & lighting tint only).
fn approximate_normal(p: vec2<f32>, t: f32) -> vec3<f32> {
    let eps = 0.9 * water_material.wave_len * 0.02;
    let h  = wave_field(p, t);
    let hx = wave_field(p + vec2<f32>(eps, 0.0), t);
    let hz = wave_field(p + vec2<f32>(0.0, eps), t);
    let dx = (hx - h) / eps;
    let dz = (hz - h) / eps;
    return normalize(vec3<f32>(-dx, 1.0, -dz));
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) _is_front: bool,
) -> FragmentOutput {
    // Standard PBR input (we override base_color only; metallic/roughness etc. come from StandardMaterial defaults).
    var pbr_input = pbr_input_from_standard_material(in, true);

    let t = water_material.time;
    let wp = in.world_position.xyz;
    let waves = wave_field(wp.xz, t);

    // Height coloration (simulate shallow crest vs deeper trough).
    let baseline = 25.0; // matches spawn height of plane
    let rel = clamp((baseline - (baseline + waves)) / (2.0 * water_material.wave_amp + 0.0001) + 0.5, 0.0, 1.0);

    let deep_col = water_material.color_deep.rgb;
    let shallow_col = water_material.color_shallow.rgb;
    var base_col = mix(deep_col, shallow_col, rel);

    // Subtle animated dark ripples (screen-independent)
    let ripple = sin((wp.x + wp.z) * 0.08 + t * 0.6) * 0.5 + 0.5;
    base_col *= mix(0.92, 1.04, ripple);

    // Approximate normal for fresnel effect (not altering actual lighting normal).
    let n = approximate_normal(wp.xz, t);
    let view_dir = normalize(pbr_input.frag_view_dir);
    let fres = pow(1.0 - max(dot(n, view_dir), 0.0), water_material.fresnel_power);
    base_col += fres * vec3<f32>(0.15, 0.22, 0.28);

    // Clamp & apply a mild tone compression
    base_col = clamp(base_col, vec3<f32>(0.0), vec3<f32>(1.0));
    base_col = pow(base_col, vec3<f32>(0.95)); // slight contrast

    pbr_input.material.base_color = vec4<f32>(base_col, 1.0);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
