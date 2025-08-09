use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use noise::{NoiseFn, Perlin};
use bevy::render::mesh::PrimitiveTopology;

/// Configuration for procedural terrain height sampling & mesh generation.
#[derive(Resource, Clone)]
pub struct TerrainConfig {
    pub seed: u32,
    pub amplitude: f32,
    pub frequency: f64,
    pub octaves: u8,
    pub lacunarity: f64,
    pub gain: f64,
    pub chunk_size: f32,
    pub resolution: u32, // number of quads per side (vertices = (res+1)^2)
}
impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            seed: 1337,
            amplitude: 3.0,
            frequency: 0.08,
            octaves: 4,
            lacunarity: 2.0,
            gain: 0.5,
            chunk_size: 128.0,
            resolution: 128,
        }
    }
}

/// Lightweight sampler that can evaluate heights deterministically.
#[derive(Resource, Clone)]
pub struct TerrainSampler {
    perlin: Perlin,
    cfg: TerrainConfig,
    seed_offset: Vec2,
}

impl TerrainSampler {
    pub fn new(cfg: TerrainConfig) -> Self {
        // Derive offsets from seed to avoid symmetry.
        let sx = (cfg.seed.wrapping_mul(747796405) ^ 0xA5A5A5A5) as f32 * 0.00123;
        let sz = (cfg.seed.wrapping_mul(2891336453) ^ 0x5A5A5A5A) as f32 * 0.00097;
        Self {
            perlin: Perlin::new(cfg.seed),
            cfg,
            seed_offset: Vec2::new(sx, sz),
        }
    }

    pub fn height(&self, x: f32, z: f32) -> f32 {
        let mut amp = 1.0;
        let mut freq = self.cfg.frequency;
        let mut sum = 0.0;
        for _ in 0..self.cfg.octaves {
            let nx = (x + self.seed_offset.x) as f64 * freq;
            let nz = (z + self.seed_offset.y) as f64 * freq;
            let n = self.perlin.get([nx, nz]) as f32; // in [-1,1]
            sum += n * amp;
            freq *= self.cfg.lacunarity;
            amp *= self.cfg.gain as f32;
        }
        sum * self.cfg.amplitude
    }

    /// Central-difference normal.
    pub fn normal(&self, x: f32, z: f32) -> Vec3 {
        // Sample spacing proportional to underlying grid cell size.
        let mut d = self.cfg.chunk_size / self.cfg.resolution as f32;
        // Clamp to avoid too small (noise precision / fp noise) or too large (loss of detail).
        d = d.clamp(0.05, 0.5);
        let h_l = self.height(x - d, z);
        let h_r = self.height(x + d, z);
        let h_d = self.height(x, z - d);
        let h_u = self.height(x, z + d);
        let dx = h_l - h_r;
        let dz = h_d - h_u;
        Vec3::new(dx, 2.0 * d, dz).normalize_or_zero()
    }
}

pub fn sample_height(x: f32, z: f32, sampler: &TerrainSampler) -> f32 {
    sampler.height(x, z)
}

pub fn sample_height_normal(x: f32, z: f32, sampler: &TerrainSampler) -> (f32, Vec3) {
    (sampler.height(x, z), sampler.normal(x, z))
}

#[derive(Component)]
pub struct TerrainChunk; // Single chunk (M0)

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TerrainConfig::default())
            // Ensure sampler exists before any Startup systems needing it.
            .add_systems(PreStartup, init_sampler)
            .add_systems(Startup, generate_single_chunk);
    }
}

fn init_sampler(mut commands: Commands, cfg: Res<TerrainConfig>) {
    commands.insert_resource(TerrainSampler::new(cfg.clone()));
}

fn generate_single_chunk(
    mut commands: Commands,
    sampler: Res<TerrainSampler>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let cfg = &sampler.cfg;
    let res = cfg.resolution;
    let size = cfg.chunk_size;
    let step = size / res as f32;
    let half = size * 0.5;

    let verts_count = ((res + 1) * (res + 1)) as usize;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(verts_count);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(verts_count);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(verts_count);

    // Heights grid (row-major z=j, x=i) and also store for heightfield.
    let mut heights: Vec<f32> = Vec::with_capacity(verts_count);
    for j in 0..=res {
        for i in 0..=res {
            let world_x = -half + i as f32 * step;
            let world_z = -half + j as f32 * step;
            heights.push(sampler.height(world_x, world_z));
        }
    }

    // Visual mesh centered at origin; we use local grid coordinates then translate entity by (-half,0,-half).
    for j in 0..=res {
        for i in 0..=res {
            let idx = (j * (res + 1) + i) as usize;
            let h = heights[idx];

            let i_l = if i == 0 { i } else { i - 1 };
            let i_r = if i == res { i } else { i + 1 };
            let j_d = if j == 0 { j } else { j - 1 };
            let j_u = if j == res { j } else { j + 1 };
            let h_l = heights[(j * (res + 1) + i_l) as usize];
            let h_r = heights[(j * (res + 1) + i_r) as usize];
            let h_d = heights[(j_d * (res + 1) + i) as usize];
            let h_u = heights[(j_u * (res + 1) + i) as usize];
            let dx = h_l - h_r;
            let dz = h_d - h_u;
            let n = Vec3::new(dx, 2.0 * step, dz).normalize_or_zero();

            let local_x = i as f32 * step;
            let local_z = j as f32 * step;
            positions.push([local_x, h, local_z]);
            normals.push([n.x, n.y, n.z]);
            uvs.push([i as f32 / res as f32, j as f32 / res as f32]);
        }
    }

    // Indices
    let mut indices: Vec<u32> = Vec::with_capacity((res * res * 6) as usize);
    for j in 0..res {
        for i in 0..res {
            let row = res + 1;
            let i0 = j * row + i;
            let i1 = i0 + 1;
            let i2 = i0 + row;
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3].map(|v| v as u32));
        }
    }

    // Mesh
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    let mesh_handle = meshes.add(mesh);
    let material = mats.add(Color::srgb(0.22, 0.50, 0.22));

    // Heightfield collider (Rapier expects rows * cols)
    let nrows = (res + 1) as usize;
    let ncols = (res + 1) as usize;
    let collider = Collider::heightfield(heights, nrows, ncols, Vec3::new(step, 1.0, step));

    commands
        .spawn(PbrBundle {
            mesh: mesh_handle,
            material,
            transform: Transform::from_xyz(-half, 0.0, -half),
            ..default()
        })
        .insert(RigidBody::Fixed)
        .insert(collider)
        .insert(Friction { coefficient: 1.0, combine_rule: CoefficientCombineRule::Average })
        .insert(TerrainChunk);
}
