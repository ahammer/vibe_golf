use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use noise::{Perlin, NoiseFn};
use bevy::render::mesh::PrimitiveTopology;
use bevy::pbr::{ExtendedMaterial, StandardMaterial};
use std::collections::{HashMap, HashSet};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future::{block_on, poll_once};
use crate::plugins::contour_material::{ContourExtension, topo_palette};
use crate::plugins::terrain_graph::{build_terrain_graph, NodeRef, GraphContext};
use crate::plugins::ball::Ball;

/// Configuration for procedural terrain height sampling & mesh generation.
#[derive(Resource, Clone)]
pub struct TerrainConfig {
    pub seed: u32,
    pub amplitude: f32,
    // Legacy fields (kept for potential reuse)
    pub frequency: f64,
    pub octaves: u8,
    pub lacunarity: f64,
    pub gain: f64,
    // Multi-scale parameters
    pub base_frequency: f64,
    pub detail_frequency: f64,
    pub detail_octaves: u8,
    pub warp_frequency: f64,
    pub warp_amplitude: f32,
    // Macro terrain / biome parameters
    pub macro_frequency: f64,   // very low frequency controlling valleys / mountains
    pub mountain_start: f32,
    pub mountain_end: f32,
    pub valley_start: f32,
    pub valley_end: f32,
    // Chunked terrain params
    pub chunk_size: f32,
    pub resolution: u32,
    pub view_radius_chunks: i32,
    pub max_spawn_per_frame: usize,
    // Radial shaping (local crater) still applied (can later gate by distance)
    pub play_radius: f32,
    pub rim_start: f32,
    pub rim_peak: f32,
    pub rim_height: f32,
    pub vegetation_per_chunk: u32,
    pub mountain_height: f32,
    pub valley_depth: f32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            seed: 1337,
            amplitude: 4.0,
            frequency: 0.08,
            octaves: 4,
            lacunarity: 2.0,
            gain: 0.5,
            base_frequency: 0.010,
            detail_frequency: 0.030,
            detail_octaves: 3,
            warp_frequency: 0.020,
            warp_amplitude: 3.0,
            chunk_size: 160.0,
            resolution: 96,
            view_radius_chunks: 8, // increased for farther terrain retention (was 4)
            max_spawn_per_frame: 16, // spawn more chunks per frame to fill extended radius faster (was 8)
            macro_frequency: 0.0025,
            mountain_start: 0.62,
            mountain_end: 0.75,
            valley_start: 0.45,
            valley_end: 0.30,
            play_radius: 70.0,
            rim_start: 90.0,
            rim_peak: 150.0,
            rim_height: 10.0,
            vegetation_per_chunk: 40,
            mountain_height: 10.0,
            valley_depth: 8.0,
        }
    }
}

/// Lightweight sampler that can evaluate heights deterministically.
#[derive(Resource, Clone)]
pub struct TerrainSampler {
    perlin: Perlin,
    pub cfg: TerrainConfig,
    seed_offset: Vec2,
    graph: NodeRef,
}

impl TerrainSampler {
    pub fn new(cfg: TerrainConfig) -> Self {
        let sx = (cfg.seed.wrapping_mul(747796405) ^ 0xA5A5A5A5) as f32 * 0.00123;
        let sz = (cfg.seed.wrapping_mul(2891336453) ^ 0x5A5A5A5A) as f32 * 0.00097;
        let graph = build_terrain_graph(&cfg);
        Self {
            perlin: Perlin::new(cfg.seed),
            cfg,
            seed_offset: Vec2::new(sx, sz),
            graph,
        }
    }

    pub fn height(&self, x: f32, z: f32) -> f32 {
        let ctx = GraphContext {
            perlin: &self.perlin,
            cfg: &self.cfg,
            seed_offset: self.seed_offset,
        };
        let base = self.graph.sample(x, z, &ctx);
        let macro_v = self.macro_value(x, z);
        let cfg = &self.cfg;
        let smooth = |a: f32, b: f32, v: f32| {
            if (b - a).abs() < 1e-6 {
                return 0.0;
            }
            let mut t = ((v - a) / (b - a)).clamp(0.0, 1.0);
            t = t * t * (3.0 - 2.0 * t);
            t
        };
        let mountain_t = smooth(cfg.mountain_start, cfg.mountain_end, macro_v);
        let valley_t = smooth(cfg.valley_end, cfg.valley_start, macro_v);
        let relief_scale = 0.80 + 0.25 * mountain_t + 0.15 * valley_t;
        let uplift = mountain_t.powf(1.15) * cfg.mountain_height;
        let depress = valley_t.powf(1.05) * cfg.valley_depth;
        (base * relief_scale + uplift - depress) * cfg.amplitude
    }

    pub fn normal(&self, x: f32, z: f32) -> Vec3 {
        let mut d = self.cfg.chunk_size / self.cfg.resolution as f32;
        d = d.clamp(0.05, 0.5);
        let h_l = self.height(x - d, z);
        let h_r = self.height(x + d, z);
        let h_d = self.height(x, z - d);
        let h_u = self.height(x, z + d);
        let dx = h_l - h_r;
        let dz = h_d - h_u;
        Vec3::new(dx, 2.0 * d, dz).normalize_or_zero()
    }

    pub fn macro_value(&self, x: f32, z: f32) -> f32 {
        let nx = (x + self.seed_offset.x) as f64 * self.cfg.macro_frequency;
        let nz = (z + self.seed_offset.y) as f64 * self.cfg.macro_frequency;
        (self.perlin.get([nx, nz]) as f32) * 0.5 + 0.5
    }
}

pub fn sample_height(x: f32, z: f32, sampler: &TerrainSampler) -> f32 {
    sampler.height(x, z)
}

pub fn sample_height_normal(x: f32, z: f32, sampler: &TerrainSampler) -> (f32, Vec3) {
    (sampler.height(x, z), sampler.normal(x, z))
}

#[derive(Component)]
pub struct TerrainChunk {
    pub coord: IVec2,
}

#[derive(Resource, Default)]
pub struct LoadedChunks {
    pub map: HashMap<IVec2, Entity>,
}

#[derive(Resource, Default)]
pub struct InProgressChunks {
    pub set: HashSet<IVec2>,
}

struct ChunkBuildResult {
    coord: IVec2,
    mesh: Mesh,
    heights: Vec<f32>,
    min_h: f32,
    max_h: f32,
    res: u32,
    step: f32,
}

#[derive(Component)]
struct ChunkBuildTask {
    coord: IVec2,
    task: Task<ChunkBuildResult>,
}

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TerrainConfig::default())
            .add_systems(PreStartup, init_sampler)
            .insert_resource(LoadedChunks::default())
            .insert_resource(InProgressChunks::default())
            .add_systems(
                Update,
                (update_terrain_chunks, finalize_chunk_tasks.after(update_terrain_chunks)),
            );
    }
}

fn init_sampler(mut commands: Commands, cfg: Res<TerrainConfig>) {
    commands.insert_resource(TerrainSampler::new(cfg.clone()));
}

fn update_terrain_chunks(
    mut commands: Commands,
    mut loaded: ResMut<LoadedChunks>,
    mut in_progress: ResMut<InProgressChunks>,
    sampler: Res<TerrainSampler>,
    q_ball: Query<&Transform, With<Ball>>,
) {
    let cfg = &sampler.cfg;
    let center_pos = q_ball.get_single().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    let center_chunk = IVec2::new(
        (center_pos.x / cfg.chunk_size).floor() as i32,
        (center_pos.z / cfg.chunk_size).floor() as i32,
    );

    let radius = cfg.view_radius_chunks;
    let mut desired: Vec<IVec2> = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            desired.push(IVec2::new(center_chunk.x + dx, center_chunk.y + dz));
        }
    }
    desired.sort_by_key(|c| {
        let dx = c.x - center_chunk.x;
        let dz = c.y - center_chunk.y;
        dx * dx + dz * dz
    });

    let mut spawned_this_frame = 0usize;
    for coord in desired.iter() {
        if loaded.map.contains_key(coord) || in_progress.set.contains(coord) {
            continue;
        }
        if spawned_this_frame >= cfg.max_spawn_per_frame {
            break;
        }
        spawn_chunk_task(&mut commands, *coord, sampler.as_ref().clone());
        in_progress.set.insert(*coord);
        spawned_this_frame += 1;
    }

    let mut to_remove: Vec<IVec2> = Vec::new();
    for (coord, ent) in loaded.map.iter() {
        if (coord.x - center_chunk.x).abs() > radius || (coord.y - center_chunk.y).abs() > radius {
            commands.entity(*ent).despawn_recursive();
            to_remove.push(*coord);
        }
    }
    for c in to_remove {
        loaded.map.remove(&c);
    }
}

fn spawn_chunk_task(commands: &mut Commands, coord: IVec2, sampler: TerrainSampler) {
    let task_pool = AsyncComputeTaskPool::get();
    let task = task_pool.spawn(async move {
        let cfg = &sampler.cfg;
        let res = cfg.resolution;
        let size = cfg.chunk_size;
        let step = size / res as f32;

        let verts_count = ((res + 1) * (res + 1)) as usize;
        let mut positions: Vec<[f32; 3]> = Vec::with_capacity(verts_count);
        let mut normals: Vec<[f32; 3]> = Vec::with_capacity(verts_count);
        let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(verts_count);
        let mut colors: Vec<[f32; 4]> = Vec::with_capacity(verts_count);
        let mut heights: Vec<f32> = Vec::with_capacity(verts_count);

        let origin_x = coord.x as f32 * size;
        let origin_z = coord.y as f32 * size;

        for j in 0..=res {
            for i in 0..=res {
                let world_x = origin_x + i as f32 * step;
                let world_z = origin_z + j as f32 * step;
                heights.push(sampler.height(world_x, world_z));
            }
        }
        let (min_h, max_h) =
            heights.iter().fold((f32::MAX, f32::MIN), |(mn, mx), &h| (mn.min(h), mx.max(h)));

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

                let h_norm = if max_h > min_h { (h - min_h) / (max_h - min_h) } else { 0.0 };
                let palette: [Vec3; 7] = [
                    Vec3::new(0.06, 0.20, 0.18),
                    Vec3::new(0.12, 0.32, 0.22),
                    Vec3::new(0.32, 0.46, 0.24),
                    Vec3::new(0.55, 0.58, 0.34),
                    Vec3::new(0.63, 0.55, 0.38),
                    Vec3::new(0.52, 0.42, 0.34),
                    Vec3::new(0.55, 0.55, 0.55),
                ];
                let bands = (palette.len() - 1) as f32;
                let scaled = h_norm * bands;
                let band_idx = scaled.floor().clamp(0.0, bands - 1.0) as usize;
                let t_band = (scaled - band_idx as f32).clamp(0.0, 1.0);
                let base_col = palette[band_idx].lerp(palette[band_idx + 1], t_band);

                let contour_interval = 1.0_f32;
                let contour_thickness = 0.12_f32;
                let h_mod = (h / contour_interval).fract();
                let d_line = h_mod.min(1.0 - h_mod);
                let line_strength =
                    (1.0 - (d_line / contour_thickness)).clamp(0.0, 1.0).powf(1.5);

                let slope_factor = n.y.clamp(0.0, 1.0).powf(0.8);
                let slope_dark = 0.85 + 0.15 * slope_factor;

                let ink = Vec3::new(0.15, 0.12, 0.10);
                let contour_col = base_col.lerp(ink, line_strength * 0.85);
                let final_col = contour_col * slope_dark * 0.92;

                colors.push([final_col.x, final_col.y, final_col.z, 1.0]);
            }
        }

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

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

        ChunkBuildResult {
            coord,
            mesh,
            heights,
            min_h,
            max_h,
            res,
            step,
        }
    });
    commands.spawn(ChunkBuildTask { coord, task });
}

fn finalize_chunk_tasks(
    mut commands: Commands,
    mut loaded: ResMut<LoadedChunks>,
    mut in_progress: ResMut<InProgressChunks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut contour_mats: ResMut<Assets<ExtendedMaterial<StandardMaterial, ContourExtension>>>,
    mut q_tasks: Query<(Entity, &mut ChunkBuildTask)>,
) {
    for (e, mut build) in q_tasks.iter_mut() {
        if let Some(result) = block_on(poll_once(&mut build.task)) {
            let coord = result.coord;
            let (palette_arr, palette_len) = topo_palette();
            let mut ext = ContourExtension::default();
            ext.data.min_height = result.min_h;
            ext.data.max_height = result.max_h;
            ext.data.interval = 1.6;
            ext.data.thickness = 0.70;
            ext.data.scroll_speed = 0.40;
            ext.data.darken = 0.88;
            ext.data.palette_len = palette_len;
            for i in 0..palette_len as usize {
                ext.data.colors[i] = palette_arr[i];
            }
            let base = StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.9,
                metallic: 0.0,
                ..default()
            };
            let material = contour_mats.add(ExtendedMaterial { base, extension: ext });
            let mesh_handle = meshes.add(result.mesh);

            let nrows = (result.res + 1) as usize;
            let ncols = (result.res + 1) as usize;
            let collider = Collider::heightfield(
                result.heights,
                nrows,
                ncols,
                Vec3::new(result.step, 1.0, result.step),
            );

            let origin_x = coord.x as f32 * result.res as f32 * result.step;
            let origin_z = coord.y as f32 * result.res as f32 * result.step;

            commands
                .entity(e)
                .remove::<ChunkBuildTask>()
                .insert((
                    MaterialMeshBundle {
                        mesh: mesh_handle,
                        material,
                        transform: Transform::from_translation(Vec3::new(origin_x, 0.0, origin_z)),
                        ..default()
                    },
                    RigidBody::Fixed,
                    collider,
                    Friction {
                        coefficient: 1.0,
                        combine_rule: CoefficientCombineRule::Average,
                    },
                    TerrainChunk { coord },
                ));

            loaded.map.insert(coord, e);
            in_progress.set.remove(&coord);
        }
    }
}
