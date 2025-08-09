use bevy::prelude::*;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial};
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};

/// Uniform data for contour extension (single buffer).
#[derive(Clone, Copy, Debug, ShaderType, Default)]
pub struct ContourUniform {
    pub min_height: f32,
    pub max_height: f32,
    pub interval: f32,
    pub thickness: f32,
    pub time: f32,
    pub scroll_speed: f32,
    pub darken: f32,
    pub palette_len: u32,
    pub colors: [Vec4; 8],
}

/// Extension part of the extended material.
/// Uses binding slots starting at 100 to avoid conflicts with base StandardMaterial bindings.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct ContourExtension {
    #[uniform(100)]
    pub data: ContourUniform,
}

impl Default for ContourExtension {
    fn default() -> Self {
        Self {
            data: ContourUniform {
                min_height: 0.0,
                max_height: 10.0,
                interval: 0.5,
                thickness: 0.06,
                time: 0.0,
                scroll_speed: 0.10,
                darken: 0.9,
                palette_len: 0,
                colors: [Vec4::ZERO; 8],
            },
        }
    }
}

impl MaterialExtension for ContourExtension {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/contour_ext.wgsl".into())
    }

    fn deferred_fragment_shader() -> ShaderRef {
        // same shader works in deferred: it only alters the lit color multiplicatively
        ShaderRef::Path("shaders/contour_ext.wgsl".into())
    }
}

/// Plugin registering ExtendedMaterial<StandardMaterial, ContourExtension> and time animation.
pub struct ContourMaterialPlugin;

impl Plugin for ContourMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ExtendedMaterial<StandardMaterial, ContourExtension>>::default())
            .add_systems(Update, advance_contour_time);
    }
}

fn advance_contour_time(
    time: Res<Time>,
    mut materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, ContourExtension>>>,
) {
    let t = time.elapsed_seconds();
    for (_, mat) in materials.iter_mut() {
        mat.extension.data.time = t;
    }
}

/// Helper to build a default topographic palette (returns (colors, len))
pub fn topo_palette() -> ([Vec4; 8], u32) {
    let cols = [
        Vec4::new(0.05, 0.18, 0.16, 1.0), // dark low forest
        Vec4::new(0.12, 0.32, 0.22, 1.0), // forest green
        Vec4::new(0.30, 0.46, 0.24, 1.0), // grass
        Vec4::new(0.55, 0.58, 0.34, 1.0), // light grass / scrub
        Vec4::new(0.63, 0.55, 0.38, 1.0), // tan
        Vec4::new(0.52, 0.42, 0.34, 1.0), // brown
        Vec4::new(0.55, 0.55, 0.55, 1.0), // grey rock
        Vec4::new(0.85, 0.85, 0.85, 1.0), // high / snow
    ];
    (cols, cols.len() as u32)
}
