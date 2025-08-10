use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use noise::{Perlin, NoiseFn};
use bevy::render::mesh::PrimitiveTopology;
use bevy::pbr::{ExtendedMaterial, StandardMaterial};
use std::collections::HashMap;
use crate::plugins::contour_material::{ContourExtension, topo_palette};
use crate::plugins::terrain_graph::{build_terrain_graph, NodeRef, GraphContext};
use crate::plugins::ball::Ball;
use rand::prelude::*;

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
    pub mountain_start: f32,    // macro value where mountains begin
    pub mountain_end: f32,      // macro value where mountains are fully active
    pub valley_start: f32,      // macro value where valleys start fading out
    pub valley_end: f32,        // macro value where valleys fully active (lower than start)
    // Chunked terrain params
    pub chunk_size: f32,      // world size of a chunk (edge length)
    pub resolution: u32,      // quads per side (vertices = (res+1)^2)
    pub view_radius_chunks: i32, // Manhattan / square radius of chunks around center (load region is (2r+1)^2)
    pub max_spawn_per_frame: usize, // budget new chunk spawns per frame to avoid spikes
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
            amplitude: 4.0, // Reduced to soften overall vertical scale
            frequency: 0.08,
            octaves: 4,
            lacunarity: 2.0,
            gain: 0.5,
            base_frequency: 0.010,
            detail_frequency: 0.030, // Lower high‑freq detail to reduce sharp cliffs
            detail_octaves: 3,       // Fewer octaves = smoother
            warp_frequency: 0.020,
            warp_amplitude: 3.0,     // Less domain warp = gentler transitions
            // Reduced chunk size & resolution to allow many chunks with similar total vertex counts.
            chunk_size: 160.0,
            resolution: 96,
            view_radius_chunks: 4,        // (2*4+1)^2 = 81 chunks (wider horizon)
            max_spawn_per_frame: 8,
            macro_frequency: 0.0025,      // very low frequency (macro landforms)
            mountain_start: 0.62,
            mountain_end: 0.75,
            valley_start: 0.45,
            valley_end: 0.30,
            play_radius: 70.0,
            rim_start: 90.0,
            rim_peak: 150.0,
            rim_height: 10.0,
            vegetation_per_chunk: 40,
            mountain_height: 10.0,   // Lower mountain uplift
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
        // Derive offsets from seed.
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

        // Macro shaping (valleys, hills, mountains)
        let macro_v = self.macro_value(x, z);
        let cfg = &self.cfg;
        let smooth = |a: f32, b: f32, v: f32| {
            if (b - a).abs() < 1e-6 { return 0.0; }
            let mut t = ((v - a) / (b - a)).clamp(0.0, 1.0);
            t = t * t * (3.0 - 2.0 * t);
            t
        };
        let mountain_t = smooth(cfg.mountain_start, cfg.mountain_end, macro_v);
        let valley_t = smooth(cfg.valley_end, cfg.valley_start, macro_v); // reversed (higher when macro_v lower)

        // Scale base undulations: valleys slightly smoother, mountains amplify relief
        // Soften relief variation: higher base, lower amplification in mountains/valleys
        let relief_scale = 0.80 + 0.25 * mountain_t + 0.15 * valley_t;

        // Add large scale offsets: uplift mountains, depress valleys
        let uplift = mountain_t.powf(1.15) * cfg.mountain_height;
        let depress = valley_t.powf(1.05) * cfg.valley_depth;

        (base * relief_scale + uplift - depress) * cfg.amplitude
    }

    /// Central-difference normal.
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

    /// Macro biome value 0..1 (valley..mountain) used for vegetation & future biome logic.
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

/// Terrain chunk marker (with grid coord).
#[derive(Component)]
pub struct TerrainChunk {
    pub coord: IVec2,
}

/// Tracks loaded chunk entities.
#[derive(Resource, Default)]
pub struct LoadedChunks {
    pub map: HashMap<IVec2, Entity>,
}

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TerrainConfig::default())
            .add_systems(PreStartup, init_sampler)
            .insert_resource(LoadedChunks::default())
            .add_systems(Update, (update_terrain_chunks, populate_chunk_vegetation, align_vegetation));
    }
}

fn init_sampler(mut commands: Commands, cfg: Res<TerrainConfig>) {
    commands.insert_resource(TerrainSampler::new(cfg.clone()));
}

/// Update which chunks should be present around the player (Ball) or origin.
fn update_terrain_chunks(
    mut commands: Commands,
    mut loaded: ResMut<LoadedChunks>,
    sampler: Res<TerrainSampler>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut contour_mats: ResMut<Assets<ExtendedMaterial<StandardMaterial, ContourExtension>>>,
    q_ball: Query<&Transform, With<Ball>>,
    time: Res<Time>,
) {
    let cfg = &sampler.cfg;
    let center_pos = q_ball.get_single().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    let center_chunk = IVec2::new(
        (center_pos.x / cfg.chunk_size).floor() as i32,
        (center_pos.z / cfg.chunk_size).floor() as i32,
    );

    let radius = cfg.view_radius_chunks;
    // Desired set of coords
    let mut desired: Vec<IVec2> = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            desired.push(IVec2::new(center_chunk.x + dx, center_chunk.y + dz));
        }
    }

    // Sort desired coords so nearer chunks spawn first (improves perceived pop-in)
    desired.sort_by_key(|c| {
        let dx = c.x - center_chunk.x;
        let dz = c.y - center_chunk.y;
        dx * dx + dz * dz
    });

    // Spawn budget
    let mut spawned_this_frame = 0usize;

    // Add missing
    for coord in desired.iter() {
        if !loaded.map.contains_key(coord) && spawned_this_frame < cfg.max_spawn_per_frame {
            let e = spawn_chunk(*coord, &mut commands, &sampler, &mut meshes, &mut contour_mats);
            loaded.map.insert(*coord, e);
            spawned_this_frame += 1;
        }
        if spawned_this_frame >= cfg.max_spawn_per_frame {
            break;
        }
    }

    // Despawn out-of-range
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

    // Optional: future LOD / streaming pacing can use time.delta_seconds() etc.
    let _dt = time.delta_seconds();
}

#[derive(Component)]
pub struct VegetationInstance;

fn populate_chunk_vegetation(
    mut commands: Commands,
    sampler: Res<TerrainSampler>,
    cfg: Res<TerrainConfig>,
    assets: Res<AssetServer>,
    q_new: Query<(Entity, &TerrainChunk, &Transform), Added<TerrainChunk>>,
) {
    let models = [
        "models/tree_1.glb#Scene0",
        "models/tree_2.glb#Scene0",
        "models/candy_1.glb#Scene0",
        "models/candy_2.glb#Scene0",
        "models/meatball.glb#Scene0",
        "models/snowflake.glb#Scene0",
    ];
    for (chunk_entity, _chunk, t_chunk) in &q_new {
        let base = t_chunk.translation;
        // Parent vegetation to chunk so it despawns automatically.
        commands.entity(chunk_entity).with_children(|c| {
            // Rejection sampling: attempt more candidates to achieve target count biased to valleys & gentle slopes.
            let mut placed = 0u32;
            let mut attempts = 0u32;
            let max_attempts = cfg.vegetation_per_chunk * 6;
            while placed < cfg.vegetation_per_chunk && attempts < max_attempts {
                attempts += 1;
                let rx: f32 = random();
                let rz: f32 = random();
                let x = base.x + rx * cfg.chunk_size;
                let z = base.z + rz * cfg.chunk_size;
                let h = sampler.height(x, z);
                let n = sampler.normal(x, z);
                if n.y < 0.60 { // avoid steeper slopes
                    continue;
                }
                let macro_v = sampler.macro_value(x, z);
                // Compute valley & mountain factors
                let m_start = cfg.mountain_start;
                let m_end = cfg.mountain_end;
                let v_start = cfg.valley_start;
                let v_end = cfg.valley_end;
                let smooth = |a: f32, b: f32, v: f32| {
                    if (b - a).abs() < 1e-6 { return 0.0; }
                    let mut t = ((v - a) / (b - a)).clamp(0.0, 1.0);
                    t = t * t * (3.0 - 2.0 * t);
                    t
                };
                let mountain_t = smooth(m_start, m_end, macro_v);
                let valley_t = smooth(v_end, v_start, macro_v); // reversed

                // Probability weight: prefer valleys & mid hills, sparse mountains
                let weight = 0.15 + valley_t * 0.55 + (1.0 - mountain_t) * 0.25;
                if random::<f32>() > weight {
                    continue;
                }

                let model = models[random::<usize>() % models.len()];
                let r = random::<f32>();
                // Scale influenced by valley/mountain (larger in valleys)
                let scale_bias = 0.4 + valley_t * 0.8;
                let scale = (0.4 + (r * r) * 1.8) * scale_bias.clamp(0.5, 1.4);
                c.spawn((
                    SceneBundle {
                        scene: assets.load(model),
                        transform: Transform::from_xyz(x, h, z)
                            .with_scale(Vec3::splat(scale))
                            .with_rotation(Quat::from_rotation_y(random::<f32>() * std::f32::consts::TAU)),
                        ..default()
                    },
                    VegetationInstance,
                ));
                placed += 1;
            }
        });
    }
}

fn align_vegetation(
    sampler: Res<TerrainSampler>,
    mut q_added: Query<&mut Transform, (With<VegetationInstance>, Added<VegetationInstance>)>,
) {
    for mut t in &mut q_added {
        let x = t.translation.x;
        let z = t.translation.z;
        let h = sampler.height(x, z);
        t.translation.y = h;
        let n = sampler.normal(x, z);

        // Limit tilt for stylization (max ~17 degrees)
        let max_tilt = 0.30_f32;
        let up = Vec3::Y;
        let angle = up.angle_between(n);
        if angle > 1e-3 {
            let clamped = angle.min(max_tilt);
            let axis = up.cross(n).normalize_or_zero();
            if axis.length_squared() > 0.0 {
                let q = Quat::from_axis_angle(axis, clamped);
                // Preserve existing yaw (world up rotation)
                let yaw = t.rotation.to_euler(EulerRot::YXZ).0;
                t.rotation = Quat::from_rotation_y(yaw) * q;
            }
        }
    }
}

/// Spawn a single chunk at grid coordinate.
fn spawn_chunk(
    coord: IVec2,
    commands: &mut Commands,
    sampler: &TerrainSampler,
    meshes: &mut ResMut<Assets<Mesh>>,
    contour_mats: &mut ResMut<Assets<ExtendedMaterial<StandardMaterial, ContourExtension>>>,
) -> Entity {
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

    // World origin of chunk (lower-left corner)
    let origin_x = coord.x as f32 * size;
    let origin_z = coord.y as f32 * size;

    // Pre-sample heights
    for j in 0..=res {
        for i in 0..=res {
            let world_x = origin_x + i as f32 * step;
            let world_z = origin_z + j as f32 * step;
            heights.push(sampler.height(world_x, world_z));
        }
    }

    // Min/max for coloring
    let (min_h, max_h) = heights.iter().fold((f32::MAX, f32::MIN), |(mn, mx), &h| (mn.min(h), mx.max(h)));

    // Build vertices
    for j in 0..=res {
        for i in 0..=res {
            let idx = (j * (res + 1) + i) as usize;
            let h = heights[idx];

            // Neighbors for normal
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

            // Local (chunk space) positions
            let local_x = i as f32 * step;
            let local_z = j as f32 * step;
            positions.push([local_x, h, local_z]);
            normals.push([n.x, n.y, n.z]);
            uvs.push([i as f32 / res as f32, j as f32 / res as f32]);

            // Color bands (same as previous implementation)
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
            let line_strength = (1.0 - (d_line / contour_thickness)).clamp(0.0, 1.0).powf(1.5);

            let slope_factor = n.y.clamp(0.0, 1.0).powf(0.8);
            let slope_dark = 0.85 + 0.15 * slope_factor;

            let ink = Vec3::new(0.15, 0.12, 0.10);
            let contour_col = base_col.lerp(ink, line_strength * 0.85);
            let final_col = contour_col * slope_dark * 0.92;

            colors.push([final_col.x, final_col.y, final_col.z, 1.0]);
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    let mesh_handle = meshes.add(mesh);

    // Extended material (per-chunk min/max for shader)
    let (palette_arr, palette_len) = topo_palette();
    let mut ext = ContourExtension::default();
    ext.data.min_height = min_h;
    ext.data.max_height = max_h;
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

    // Heightfield collider: use local heights with scale step
    let nrows = (res + 1) as usize;
    let ncols = (res + 1) as usize;
    let collider = Collider::heightfield(heights, nrows, ncols, Vec3::new(step, 1.0, step));

    // Spawn: transform places chunk at its world origin
    commands
        .spawn((
            MaterialMeshBundle {
                mesh: mesh_handle,
                material,
                transform: Transform::from_translation(Vec3::new(origin_x, 0.0, origin_z)),
                ..default()
            },
            RigidBody::Fixed,
            collider,
            Friction { coefficient: 1.0, combine_rule: CoefficientCombineRule::Average },
            TerrainChunk { coord },
        ))
        .id()
}
