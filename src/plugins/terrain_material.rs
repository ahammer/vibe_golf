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
    // New tunable visual parameters
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub macro_amp: f32,
    pub micro_amp: f32,
    pub edge_accent: f32,
    pub gamma: f32,
    pub macro_scale: f32,
    pub micro_scale: f32,
    pub animation_speed: f32,
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
                Vec4::new(0.11, 0.19, 0.09, 1.0), // lowland muddy moss (deep green)
                Vec4::new(0.24, 0.37, 0.15, 1.0), // richer moss / grassy
                Vec4::new(0.35, 0.34, 0.32, 1.0), // mid warm grey rock
                Vec4::new(0.50, 0.47, 0.41, 1.0), // high rocky / sandy grey (replaces snow)
            ],
            roughness_lowland: 0.88,
            roughness_grass: 0.75,
            roughness_rock: 0.55,
            roughness_snow: 0.60, // high rocky/sandy
            color_variation: 0.06,
            ao_strength: 0.65,
            brightness: 0.85,
            contrast: 1.45,
            saturation: 1.40,
            macro_amp: 0.30,
            micro_amp: 0.08,
            edge_accent: 0.12,
            gamma: 1.05,
            macro_scale: 0.18,
            micro_scale: 3.5,
            animation_speed: 0.0, // 0 = static (prevents temporal aliasing)
        }
    }
}

/// Extension type.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct RealTerrainExtension {
    #[uniform(100)]
    pub data: RealTerrainUniform,
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
