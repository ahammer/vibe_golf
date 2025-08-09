// Extended contour material fragment shader (works with ExtendedMaterial<StandardMaterial, ContourExtension>)
// Preserves PBR lighting & shadows; we only modify the StandardMaterial base_color before lighting,
// using elevation bands + animated contour lines.

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

// Auto-generated naming convention (see extended_material example):
// For Rust extension struct `ContourExtension`, Bevy exposes a uniform struct named `ContourExtendedMaterial`
// and a uniform binding variable named `contour_extended_material` at the specified group/binding.
//
// Must match layout of ContourUniform in Rust.
struct ContourExtendedMaterial {
    min_height: f32,
    max_height: f32,
    interval: f32,
    thickness: f32,
    time: f32,
    scroll_speed: f32,
    darken: f32,
    palette_len: u32,
    colors: array<vec4<f32>, 8u>,
}

@group(2) @binding(100)
var<uniform> contour_extended_material: ContourExtendedMaterial;

// Helpers
fn palette_color(idx: u32) -> vec3<f32> {
    return contour_extended_material.colors[min(idx, 7u)].rgb;
}

fn band_color(norm_h: f32) -> vec3<f32> {
    let len = max(1u, contour_extended_material.palette_len - 1u);
    let fidx = norm_h * f32(len);
    let i0 = u32(clamp(floor(fidx), 0.0, f32(len - 1u)));
    let t  = clamp(fidx - f32(i0), 0.0, 1.0);
    let c0 = palette_color(i0);
    let c1 = palette_color(i0 + 1u);
    return mix(c0, c1, t);
}

fn contour_mask(world_h: f32) -> f32 {
    let interval = max(0.0001, contour_extended_material.interval);
    let scroll = contour_extended_material.time * contour_extended_material.scroll_speed;
    let frac = fract((world_h + scroll) / interval);
    let d = min(frac, 1.0 - frac);
    let thickness = contour_extended_material.thickness;
    let m = clamp(1.0 - d / thickness, 0.0, 1.0);
    return pow(m, 1.5);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Build standard PBR input
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Height normalization
    let denom = max(0.0001, contour_extended_material.max_height - contour_extended_material.min_height);
    let norm_h = clamp((in.world_position.y - contour_extended_material.min_height) / denom, 0.0, 1.0);

    // Base elevation gradient
    var base_col = band_color(norm_h);

    // Line overlay
    let line_m = contour_mask(in.world_position.y);
    let ink = vec3<f32>(0.15, 0.13, 0.11);
    base_col = mix(base_col, ink, line_m * 0.85);

    // Saturation & contrast adjustments to avoid pastel look (done pre-darken so darken scales final result)
    // Boost saturation
    let luma = (base_col.r + base_col.g + base_col.b) / 3.0;
    let saturation = 1.25;
    base_col = clamp(mix(vec3<f32>(luma), base_col, saturation), vec3<f32>(0.0), vec3<f32>(1.0));
    // Slight contrast boost
    let contrast = 1.08;
    base_col = clamp((base_col - vec3<f32>(0.5)) * contrast + vec3<f32>(0.5), vec3<f32>(0.0), vec3<f32>(1.0));

    // Apply global darken prior to lighting so shadows still modulate correctly
    base_col *= contour_extended_material.darken;

    // Set as base color (retain alpha)
    pbr_input.material.base_color = vec4<f32>(base_col, pbr_input.material.base_color.a);

    // Alpha discard (not really used here, but keeps pipeline consistent)
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    // Deferred path: lighting done later; we cannot touch lit color here.
    let out = deferred_output(in, pbr_input);
    return out;
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    // Post-lighting processing (fog, tonemap etc.)
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
#endif
}
