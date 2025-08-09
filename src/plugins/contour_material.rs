use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};
use bevy::pbr::Material;

/// Animated contour-line unlit material (topographic map style).
/// Fragment shader computes:
/// - Elevation bands blended from a palette
/// - Moving contour lines (height offset scroll with time)
/// - Dark line ink mix
#[derive(AsBindGroup, Debug, Clone, Asset, TypePath)]
pub struct ContourMaterial {
    #[uniform(0)]
    pub params: ContourParams,
    #[uniform(1)]
    pub palette: ContourPalette,
}

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct ContourParams {
    pub min_height: f32,
    pub max_height: f32,
    pub interval: f32,
    pub thickness: f32,
    pub time: f32,
    pub scroll_speed: f32,
    pub darken: f32,
    pub palette_len: u32,
}

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct ContourPalette {
    // Up to 8 RGBA entries (alpha currently unused, kept for alignment/extension)
    pub colors: [Vec4; 8],
}

impl Default for ContourMaterial {
    fn default() -> Self {
        Self {
            params: ContourParams {
                min_height: 0.0,
                max_height: 10.0,
                interval: 0.5,
                thickness: 0.06,
                time: 0.0,
                scroll_speed: 0.15,
                darken: 0.9,
                palette_len: 0,
            },
            palette: ContourPalette {
                colors: [Vec4::ZERO; 8],
            },
        }
    }
}

impl Material for ContourMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/contour.wgsl".into())
    }
    fn vertex_shader() -> ShaderRef {
        // Use built-in mesh vertex shader (provides position/normal, etc.)
        ShaderRef::Default
    }
    fn alpha_mode(&self) -> bevy::prelude::AlphaMode {
        AlphaMode::Opaque
    }
}

/// Plugin registering the contour material and updating its animated time.
pub struct ContourMaterialPlugin;

impl Plugin for ContourMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ContourMaterial>::default())
            .add_systems(Update, advance_contour_time);
    }
}

fn advance_contour_time(
    time: Res<Time>,
    mut materials: ResMut<Assets<ContourMaterial>>,
) {
    let t = time.elapsed_seconds();
    for mat in materials.iter_mut() {
        mat.1.params.time = t;
    }
}

/// Helper to build a default topographic palette (returns (palette, palette_len))
pub fn topo_palette() -> ([Vec4; 8], u32) {
    // Colors echo common map elevation tones (low->high)
    let cols = [
        Vec4::new(0.05, 0.18, 0.16, 1.0), // dark low forest
        Vec4::new(0.12, 0.32, 0.22, 1.0), // forest green
        Vec4::new(0.30, 0.46, 0.24, 1.0), // grass
        Vec4::new(0.55, 0.58, 0.34, 1.0), // light grass / scrub
        Vec4::new(0.63, 0.55, 0.38, 1.0), // tan
        Vec4::new(0.52, 0.42, 0.34, 1.0), // brown
        Vec4::new(0.55, 0.55, 0.55, 1.0), // grey rock
        Vec4::new(0.85, 0.85, 0.85, 1.0), // light high / snow
    ];
    (cols, cols.len() as u32)
}
