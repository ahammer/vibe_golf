use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy::pbr::{ExtendedMaterial, StandardMaterial};
use bevy::render::alpha::AlphaMode;
use std::collections::{HashMap, HashSet};
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future::{block_on, poll_once};
use crate::plugins::terrain_material::RealTerrainExtension;
use crate::plugins::ball::Ball;
use std::sync::Arc;

/// Configuration for terrain. Retains legacy procedural fields for now (unused in heightmap mode).
#[derive(Resource, Clone)]
pub struct TerrainConfig {
    pub seed: u32,
    pub amplitude: f32,
    // Legacy fields (unused now)
    pub frequency: f64,
    pub octaves: u8,
    pub lacunarity: f64,
    pub gain: f64,
    // Multi-scale parameters (unused)
    pub base_frequency: f64,
    pub detail_frequency: f64,
    pub detail_octaves: u8,
    pub warp_frequency: f64,
    pub warp_amplitude: f32,
    // Macro terrain / biome parameters (unused)
    pub macro_frequency: f64,
    pub mountain_start: f32,
    pub mountain_end: f32,
    pub valley_start: f32,
    pub valley_end: f32,
    // Chunked terrain params
    pub chunk_size: f32,
    pub resolution: u32,
    pub view_radius_chunks: i32,
    pub max_spawn_per_frame: usize,
    // Radial shaping (unused now)
    pub play_radius: f32,
    pub rim_start: f32,
    pub rim_peak: f32,
    pub rim_height: f32,
    pub vegetation_per_chunk: u32,
    pub mountain_height: f32,
    pub valley_depth: f32,
    // LOD
    pub lod_mid_distance: f32,
    pub lod_far_distance: f32,
    pub lod_mid_resolution: u32,
    pub lod_far_resolution: u32,
    // Heightmap specific
    // World size of the heightmap square in meters (2 km x 2 km).
    pub heightmap_world_size: f32,
    // Maximum elevation represented by a full-intensity (255) red channel (700 m).
    pub heightmap_max_height: f32,
    // Path to heightmap (red channel = height).
    pub heightmap_path: String,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            seed: 1337,
            amplitude: 1.0, // no longer used as main vertical scale; kept for optional post-scale
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
            view_radius_chunks: 6,
            max_spawn_per_frame: 16,
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
            lod_mid_distance: 160.0 * 3.2,
            lod_far_distance: 160.0 * 5.0,
            lod_mid_resolution: 48,
            lod_far_resolution: 24,
            heightmap_world_size: 2000.0, // 2 km
            heightmap_max_height: 200.0,  // meters
            heightmap_path: "assets/heightmaps/level1.png".to_string(),
        }
    }
}

#[derive(Clone)]
struct Heightmap {
    width: u32,
    height: u32,
    // Red channel bytes only (row-major).
    data_r: Arc<Vec<u8>>,
}

impl Heightmap {
    fn load(path: &str) -> Self {
        let img = image::open(path).expect(&format!("Failed to open heightmap {}", path)).to_rgb8();
        let (w, h) = img.dimensions();
        let raw = img.into_raw();
        let mut red = Vec::with_capacity((w * h) as usize);
        for i in (0..raw.len()).step_by(3) {
            red.push(raw[i]); // red channel
        }
        info!("Heightmap loaded: {} ({} x {})", path, w, h);
        Self {
            width: w,
            height: h,
            data_r: Arc::new(red),
        }
    }

    #[inline]
    fn sample_red_linear(&self, u: f32, v: f32) -> f32 {
        // u,v in pixel space (0..width-1, 0..height-1)
        if u < 0.0 || v < 0.0 || u > (self.width - 1) as f32 || v > (self.height - 1) as f32 {
            return 0.0;
        }
        let x0 = u.floor() as i32;
        let z0 = v.floor() as i32;
        let x1 = (x0 + 1).clamp(0, self.width as i32 - 1);
        let z1 = (z0 + 1).clamp(0, self.height as i32 - 1);
        let tx = u - x0 as f32;
        let tz = v - z0 as f32;

        let idx = |x: i32, z: i32| -> usize {
            (z as u32 * self.width + x as u32) as usize
        };

        let r00 = self.data_r[idx(x0, z0)] as f32;
        let r10 = self.data_r[idx(x1, z0)] as f32;
        let r01 = self.data_r[idx(x0, z1)] as f32;
        let r11 = self.data_r[idx(x1, z1)] as f32;

        let a = r00 + (r10 - r00) * tx;
        let b = r01 + (r11 - r01) * tx;
        (a + (b - a) * tz) / 255.0
    }
}

/// Heightmap-based sampler.
#[derive(Resource, Clone)]
pub struct TerrainSampler {
    pub cfg: TerrainConfig,
    heightmap: Heightmap,
}

impl TerrainSampler {
    pub fn new(cfg: TerrainConfig) -> Self {
        let hm = Heightmap::load(&cfg.heightmap_path);
        Self { cfg, heightmap: hm }
    }

    fn sample_heightmap(&self, x: f32, z: f32) -> f32 {
        // Interpret world (x,z) centered at (0,0). Range [-world_size/2, +world_size/2] maps to [0,1] across the heightmap.
        let world_size = self.cfg.heightmap_world_size;
        let nx = (x / world_size) + 0.5;
        let nz = (z / world_size) + 0.5;
        if nx < 0.0 || nx > 1.0 || nz < 0.0 || nz > 1.0 {
            return 0.0;
        }
        let u = nx * (self.heightmap.width - 1) as f32;
        let v = nz * (self.heightmap.height - 1) as f32;
        let h_norm = self.heightmap.sample_red_linear(u, v);
        h_norm * self.cfg.heightmap_max_height * self.cfg.amplitude
    }

    pub fn height(&self, x: f32, z: f32) -> f32 {
        self.sample_heightmap(x, z)
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
    pub res: u32,
}

#[derive(Resource, Default)]
pub struct LoadedChunks {
    pub map: HashMap<IVec2, Entity>,
}

#[derive(Resource, Default)]
pub struct InProgressChunks {
    pub set: HashSet<IVec2>,
}

#[derive(Resource, Default)]
struct TerrainGlobalMaterial {
    handle: Option<Handle<ExtendedMaterial<StandardMaterial, RealTerrainExtension>>>,
    min_h: f32,
    max_h: f32,
    created_logged: bool,
}

struct ChunkBuildResult {
    coord: IVec2,
    mesh: Mesh,
    heights: Vec<f32>,
    min_h: f32,
    max_h: f32,
    res: u32,
    step: f32,
    create_collider: bool,
}

#[derive(Component)]
#[cfg(not(target_arch = "wasm32"))]
struct ChunkBuildTask {
    coord: IVec2,
    task: Task<ChunkBuildResult>,
}

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        let app = app
            .insert_resource(TerrainConfig::default())
            .add_systems(PreStartup, init_sampler)
            .insert_resource(LoadedChunks::default())
            .insert_resource(InProgressChunks::default())
            .insert_resource(TerrainGlobalMaterial::default())
            .add_systems(Startup, spawn_water);

        #[cfg(not(target_arch = "wasm32"))]
        {
            app.add_systems(
                Update,
                (
                    update_terrain_chunks,
                    finalize_chunk_tasks.after(update_terrain_chunks),
                    apply_terrain_config_changes.after(finalize_chunk_tasks),
                ),
            );
        }

        #[cfg(target_arch = "wasm32")]
        {
            app.add_systems(
                Update,
                (
                    update_terrain_chunks,
                    apply_terrain_config_changes.after(update_terrain_chunks),
                ),
            );
        }
    }
}

fn apply_terrain_config_changes(
    mut commands: Commands,
    cfg: Res<TerrainConfig>,
    sampler: Res<TerrainSampler>,
    mut loaded: ResMut<LoadedChunks>,
    q_chunks: Query<Entity, With<TerrainChunk>>,
) {
    if !cfg.is_changed() {
        return;
    }
    // Rebuild sampler if fundamental params changed (world size, heightmap path, amplitude, view radius).
    if cfg.amplitude != sampler.cfg.amplitude
        || cfg.view_radius_chunks != sampler.cfg.view_radius_chunks
        || cfg.heightmap_world_size != sampler.cfg.heightmap_world_size
        || cfg.heightmap_path != sampler.cfg.heightmap_path
        || cfg.heightmap_max_height != sampler.cfg.heightmap_max_height
    {
        for e in q_chunks.iter() {
            commands.entity(e).despawn_recursive();
        }
        loaded.map.clear();
        commands.insert_resource(TerrainSampler::new(cfg.as_ref().clone()));
        info!("Terrain config changed (heightmap related) -> clearing & regenerating terrain");
    }
}

fn init_sampler(mut commands: Commands, cfg: Res<TerrainConfig>) {
    commands.insert_resource(TerrainSampler::new(cfg.clone()));
}

// Spawn a very large water plane at a fixed elevation (y = 25).
fn spawn_water(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let size = 5000.0; // Large enough to appear "infinite" within gameplay area
    // Manually build a large quad for water (since shape::Plane not available).
    let half = size * 0.5;
    let positions: Vec<[f32; 3]> = vec![
        [-half, 0.0, -half],
        [ half, 0.0, -half],
        [-half, 0.0,  half],
        [ half, 0.0,  half],
    ];
    let normals: Vec<[f32; 3]> = vec![[0.0, 1.0, 0.0]; 4];
    let uvs: Vec<[f32; 2]> = vec![[0.0, 0.0],[1.0,0.0],[0.0,1.0],[1.0,1.0]];
    let indices: Vec<u32> = vec![0,2,1, 1,2,3];

    let mut water_mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
    water_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    water_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    water_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    water_mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

    let mesh_handle = meshes.add(water_mesh);
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.05, 0.25, 0.6, 0.7),
        perceptual_roughness: 0.05,
        metallic: 0.0,
        reflectance: 0.8,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    commands.spawn(PbrBundle {
        mesh: mesh_handle,
        material,
        transform: Transform::from_translation(Vec3::new(0.0, 25.0, 0.0)),
        ..default()
    });
}

fn update_terrain_chunks(
    mut commands: Commands,
    mut loaded: ResMut<LoadedChunks>,
    mut in_progress: ResMut<InProgressChunks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain_mats: ResMut<Assets<ExtendedMaterial<StandardMaterial, RealTerrainExtension>>>,
    mut global_mat: ResMut<TerrainGlobalMaterial>,
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
        let chunk_world_center = Vec3::new(
            coord.x as f32 * cfg.chunk_size + cfg.chunk_size * 0.5,
            0.0,
            coord.y as f32 * cfg.chunk_size + cfg.chunk_size * 0.5,
        );
        let dist = chunk_world_center.xy().distance(center_pos.xy());
        let chosen_res = if dist > cfg.lod_far_distance {
            cfg.lod_far_resolution
        } else if dist > cfg.lod_mid_distance {
            cfg.lod_mid_resolution
        } else {
            cfg.resolution
        };
        let create_collider = chosen_res != cfg.lod_far_resolution;

        #[cfg(not(target_arch = "wasm32"))]
        {
            spawn_chunk_task(&mut commands, *coord, sampler.as_ref().clone(), chosen_res, create_collider);
            in_progress.set.insert(*coord);
        }

        // On wasm build chunks synchronously (no AsyncComputeTaskPool multithreading)
        #[cfg(target_arch = "wasm32")]
        {
            let res = chosen_res;
            let size = cfg.chunk_size;
            let step = size / res as f32;

            let verts_count = ((res + 1) * (res + 1)) as usize;
            let mut positions: Vec<[f32; 3]> = Vec::with_capacity(verts_count);
            let mut normals: Vec<[f32; 3]> = Vec::with_capacity(verts_count);
            let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(verts_count);
            let mut heights: Vec<f32> = Vec::with_capacity(verts_count);

            let origin_x_chunk = coord.x as f32 * size;
            let origin_z_chunk = coord.y as f32 * size;

            for j in 0..=res {
                for i in 0..=res {
                    let world_x = origin_x_chunk + i as f32 * step;
                    let world_z = origin_z_chunk + j as f32 * step;
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
                    let dxn = h_l - h_r;
                    let dzn = h_d - h_u;
                    let n = Vec3::new(dxn, 2.0 * step, dzn).normalize_or_zero();

                    let local_x = i as f32 * step;
                    let local_z = j as f32 * step;
                    positions.push([local_x, h, local_z]);
                    normals.push([n.x, n.y, n.z]);
                    uvs.push([i as f32 / res as f32, j as f32 / res as f32]);
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

            // Global material min/max update
            if global_mat.min_h == 0.0 && global_mat.max_h == 0.0 && global_mat.handle.is_none() {
                // sentinel
            }
            if global_mat.min_h == 0.0 && global_mat.handle.is_none() {
                global_mat.min_h = f32::MAX;
                global_mat.max_h = f32::MIN;
            }
            global_mat.min_h = global_mat.min_h.min(min_h);
            global_mat.max_h = global_mat.max_h.max(max_h);

            if global_mat.handle.is_none() {
                let mut ext = RealTerrainExtension::default();
                ext.data.min_height = min_h;
                ext.data.max_height = max_h;
                let base = StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.85,
                    metallic: 0.0,
                    ..default()
                };
                let handle = terrain_mats.add(ExtendedMaterial { base, extension: ext });
                global_mat.handle = Some(handle.clone());
                if !global_mat.created_logged {
                    info!("Terrain realistic material created (heightmap mode, wasm immediate)");
                    global_mat.created_logged = true;
                }
            }
            if let Some(handle) = &global_mat.handle {
                if let Some(mat) = terrain_mats.get_mut(handle) {
                    mat.extension.data.min_height = global_mat.min_h;
                    mat.extension.data.max_height = global_mat.max_h;
                }
            }
            let material = global_mat.handle.as_ref().unwrap().clone();

            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, Default::default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
            mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

            let mesh_handle = meshes.add(mesh);

            let origin_x = coord.x as f32 * res as f32 * step;
            let origin_z = coord.y as f32 * res as f32 * step;

            let mut ec = commands.spawn((
                MaterialMeshBundle {
                    mesh: mesh_handle,
                    material,
                    transform: Transform::from_translation(Vec3::new(origin_x, 0.0, origin_z)),
                    ..default()
                },
                TerrainChunk { coord: *coord, res },
            ));

            if create_collider {
                let nrows = (res + 1) as usize;
                let ncols = (res + 1) as usize;
                let collider = Collider::heightfield(
                    heights,
                    nrows,
                    ncols,
                    Vec3::new(step, 1.0, step),
                );
                ec.insert((
                    RigidBody::Fixed,
                    collider,
                    Friction {
                        coefficient: 1.0,
                        combine_rule: CoefficientCombineRule::Average,
                    },
                ));
            }

            loaded.map.insert(*coord, ec.id());
        }

        spawned_this_frame += 1;
    }

    // Despawn out-of-range chunks
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

#[cfg(not(target_arch = "wasm32"))]
fn spawn_chunk_task(commands: &mut Commands, coord: IVec2, sampler: TerrainSampler, override_res: u32, create_collider: bool) {
    let task_pool = AsyncComputeTaskPool::get();
    let task = task_pool.spawn(async move {
        let cfg = &sampler.cfg;
        let res = override_res;
        let size = cfg.chunk_size;
        let step = size / res as f32;

        let verts_count = ((res + 1) * (res + 1)) as usize;
        let mut positions: Vec<[f32; 3]> = Vec::with_capacity(verts_count);
        let mut normals: Vec<[f32; 3]> = Vec::with_capacity(verts_count);
        let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(verts_count);
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
        mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));

        ChunkBuildResult {
            coord,
            mesh,
            heights,
            min_h,
            max_h,
            res,
            step,
            create_collider,
        }
    });
    commands.spawn(ChunkBuildTask { coord, task });
}

#[cfg(not(target_arch = "wasm32"))]
fn finalize_chunk_tasks(
    mut commands: Commands,
    mut loaded: ResMut<LoadedChunks>,
    mut in_progress: ResMut<InProgressChunks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain_mats: ResMut<Assets<ExtendedMaterial<StandardMaterial, RealTerrainExtension>>>,
    mut global_mat: ResMut<TerrainGlobalMaterial>,
    mut q_tasks: Query<(Entity, &mut ChunkBuildTask)>,
) {
    for (e, mut build) in q_tasks.iter_mut() {
        if let Some(result) = block_on(poll_once(&mut build.task)) {
            let coord = result.coord;

            if global_mat.min_h == 0.0 && global_mat.max_h == 0.0 && global_mat.handle.is_none() {
                // sentinel - no action
            }
            if global_mat.min_h == 0.0 && global_mat.handle.is_none() {
                global_mat.min_h = f32::MAX;
                global_mat.max_h = f32::MIN;
            }
            global_mat.min_h = global_mat.min_h.min(result.min_h);
            global_mat.max_h = global_mat.max_h.max(result.max_h);

            if global_mat.handle.is_none() {
                let mut ext = RealTerrainExtension::default();
                ext.data.min_height = result.min_h;
                ext.data.max_height = result.max_h;
                let base = StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.85,
                    metallic: 0.0,
                    ..default()
                };
                let handle = terrain_mats.add(ExtendedMaterial { base, extension: ext });
                global_mat.handle = Some(handle.clone());
                if !global_mat.created_logged {
                    info!("Terrain realistic material created (heightmap mode)");
                    global_mat.created_logged = true;
                }
            }
            if let Some(handle) = &global_mat.handle {
                if let Some(mat) = terrain_mats.get_mut(handle) {
                    mat.extension.data.min_height = global_mat.min_h;
                    mat.extension.data.max_height = global_mat.max_h;
                }
            }

            let material = global_mat.handle.as_ref().unwrap().clone();
            let mesh_handle = meshes.add(result.mesh);

            let nrows = (result.res + 1) as usize;
            let ncols = (result.res + 1) as usize;

            let origin_x = coord.x as f32 * result.res as f32 * result.step;
            let origin_z = coord.y as f32 * result.res as f32 * result.step;

            let mut ec = commands.entity(e);
            ec.remove::<ChunkBuildTask>();
            ec.insert((
                MaterialMeshBundle {
                    mesh: mesh_handle,
                    material,
                    transform: Transform::from_translation(Vec3::new(origin_x, 0.0, origin_z)),
                    ..default()
                },
                TerrainChunk { coord, res: result.res },
            ));

            if result.create_collider {
                let collider = Collider::heightfield(
                    result.heights,
                    nrows,
                    ncols,
                    Vec3::new(result.step, 1.0, result.step),
                );
                ec.insert((
                    RigidBody::Fixed,
                    collider,
                    Friction {
                        coefficient: 1.0,
                        combine_rule: CoefficientCombineRule::Average,
                    },
                ));
            }

            loaded.map.insert(coord, e);
            in_progress.set.remove(&coord);
        }
    }
}
