use bevy::prelude::*;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial};
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};

/// Uniform buffer for the realistic terrain extension.
/// Matches WGSL struct RealTerrainExtendedMaterial.
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct RealTerrainUniform {
    pub min_height: f32,
    pub max_height: f32,
    pub rock_slope_start: f32,
    pub snow_height_start: f32,
    pub snow_height_end: f32,
    pub time: f32,
    pub noise_scale: f32,
    pub _pad1: f32,
    pub colors: [Vec4; 4], // lowland, grass, rock, snow
    pub roughness_lowland: f32,
    pub roughness_grass: f32,
    pub roughness_rock: f32,
    pub roughness_snow: f32,
    pub color_variation: f32,
    pub ao_strength: f32,
    pub _pad2: Vec2,
}

impl Default for RealTerrainUniform {
    fn default() -> Self {
        Self {
            min_height: 0.0,
            max_height: 10.0,
            rock_slope_start: 0.35,
            snow_height_start: 0.65,
            snow_height_end: 0.85,
            time: 0.0,
            noise_scale: 0.0015,
            _pad1: 0.0,
            colors: [
                Vec4::new(0.10, 0.25, 0.06, 1.0), // lowland deep grass
                Vec4::new(0.20, 0.50, 0.18, 1.0), // lush grass
                Vec4::new(0.38, 0.36, 0.34, 1.0), // rock
                Vec4::new(0.90, 0.93, 0.95, 1.0), // snow
            ],
            roughness_lowland: 0.85,
            roughness_grass: 0.70,
            roughness_rock: 0.55,
            roughness_snow: 0.40,
            color_variation: 0.08,
            ao_strength: 0.6,
            _pad2: Vec2::ZERO,
        }
    }
}

/// Extension type.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct RealTerrainExtension {
    #[uniform(100)]
    pub data: RealTerrainUniform,
}

impl Default for RealTerrainExtension {
    fn default() -> Self {
        Self {
            data: RealTerrainUniform::default(),
        }
    }
}

impl MaterialExtension for RealTerrainExtension {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/terrain_pbr_ext.wgsl".into())
    }

    fn deferred_fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/terrain_pbr_ext.wgsl".into())
    }
}

/// Plugin registering the realistic terrain material.
pub struct TerrainMaterialPlugin;

impl Plugin for TerrainMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ExtendedMaterial<StandardMaterial, RealTerrainExtension>>::default())
            .add_systems(Update, advance_time);
    }
}

fn advance_time(
    time: Res<Time>,
    mut materials: ResMut<Assets<ExtendedMaterial<StandardMaterial, RealTerrainExtension>>>,
) {
    let t = time.elapsed_seconds();
    for (_, mat) in materials.iter_mut() {
        mat.extension.data.time = t;
    }
}
