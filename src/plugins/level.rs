// Level loading & world setup (camera, sky, walls, ball, target).
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use serde::Deserialize;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use rand::Rng;

use crate::plugins::camera::OrbitCamera;
use crate::plugins::ball::{Ball, BallKinematic};
use crate::plugins::main_menu::GamePhase;
use crate::plugins::target::{Target, TargetFloat, TargetParams};
use crate::plugins::game_state::{ShotConfig, Score};
use crate::plugins::terrain::TerrainSampler;

// ----------------------- Level Definition (RON) -----------------------

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct Vec3Def { pub x: f32, pub y: f32, pub z: f32 }
impl Vec3Def { pub fn to_vec3(self) -> Vec3 { Vec3::new(self.x, self.y, self.z) } }

#[derive(Debug, Deserialize, Clone)]
pub struct SkyDef {
    pub texture: String,
    pub radius: f32,
    pub longitudes: u32,
    pub latitudes: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BallSpawn {
    pub model: String,
    pub pos: BallPos,
    pub spawn_height_offset: f32,
    pub collider_radius: f32,
    pub visual_scale: f32,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct BallPos { pub x: f32, pub z: f32 }

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct FloatParams {
    pub base_height: f32,
    pub amplitude: f32,
    pub bob_freq: f32,
    pub rot_speed: f32,
    pub collider_radius: f32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TargetDef {
    pub model: String,
    pub initial: TargetInitial,
    pub float: FloatParams,
}
#[derive(Debug, Deserialize, Clone, Copy)]
pub struct TargetInitial { pub x: f32, pub z: f32 }

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct WorldBounds {
    pub half_extent: f32,
    pub wall_height: f32,
    pub wall_fade_distance: f32,
    pub wall_restitution: f32,
    pub wall_color: (f32, f32, f32, f32),
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct ShotConfigDef {
    pub osc_speed: f32,
    pub base_impulse: f32,
    pub up_angle_deg: f32,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct Scoring {
    pub max_holes: u32,
}

#[derive(Debug, Deserialize, Resource)]
pub struct LevelDef {
    pub camera_start: Vec3Def,
    pub camera_look_at: Vec3Def,
    pub sky: SkyDef,
    pub ball: BallSpawn,
    pub target: TargetDef,
    pub world: WorldBounds,
    pub shot: ShotConfigDef,
    pub scoring: Scoring,
}

// ----------------------- Components / Resources -----------------------


// ----------------------- Plugin -----------------------

pub struct LevelPlugin;

#[derive(Component)]
struct SkyDome;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_level)
            .add_systems(Startup, spawn_level.after(load_level))
            .add_systems(Update, (spawn_runtime_ball, track_sky_dome));
    }
}

// ----------------------- Systems -----------------------

fn load_level(mut commands: Commands) {
    // Hard-coded single level for now.
    #[cfg(target_arch = "wasm32")]
    {
        // Embed the level definition at compile time for web (no filesystem access in browser).
        let data = include_str!("../../assets/levels/level1.ron");
        match ron::from_str::<LevelDef>(data) {
            Ok(def) => commands.insert_resource(def),
            Err(e) => error!("Failed to parse embedded level: {e}"),
        }
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = "assets/levels/level1.ron";
        if let Ok(data) = fs::read_to_string(path) {
            match ron::from_str::<LevelDef>(&data) {
                Ok(def) => {
                    commands.insert_resource(def);
                }
                Err(e) => {
                    error!("Failed to parse {path}: {e}");
                }
            }
        } else {
            error!("Failed to read level file {path}");
        }
    }
}

fn spawn_level(
    mut commands: Commands,
    level: Option<Res<LevelDef>>,
    sampler: Res<TerrainSampler>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
    mut score: Option<ResMut<Score>>,
) {
    let Some(level) = level else { return; };

    // Camera
    let cam_start = Transform::from_translation(level.camera_start.to_vec3())
        .looking_at(level.camera_look_at.to_vec3(), Vec3::Y);
    commands.spawn((
        Camera3dBundle {
            transform: cam_start,
            projection: PerspectiveProjection {
                near: 0.05,
                far: 25000.0,
                ..Default::default()
            }.into(),
            ..default()
        },
        OrbitCamera,
    ));

    // Sky
    let sky_tex = assets.load(level.sky.texture.clone());
    let sky_mesh = generate_inverted_sphere(level.sky.longitudes, level.sky.latitudes, level.sky.radius);
    commands.spawn((
        PbrBundle {
            mesh: meshes.add(sky_mesh),
            material: mats.add(StandardMaterial {
                base_color_texture: Some(sky_tex),
                unlit: true,
                ..default()
            }),
            transform: Transform::IDENTITY,
            ..default()
        },
        SkyDome,
    ));

    // Directional light (simple fixed)
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 40_000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(30.0, 60.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // Ball is spawned lazily when entering gameplay phase (see spawn_runtime_ball).

    // Target spawn + params resource
    const MIN_TARGET_GROUND: f32 = 50.0;
    let mut t_x = level.target.initial.x;
    let mut t_z = level.target.initial.z;
    let mut t_ground = sampler.height(t_x, t_z);
    if t_ground < MIN_TARGET_GROUND {
        let mut rng = rand::thread_rng();
        for _ in 0..80 {
            let dist = rng.gen_range(500.0..800.0);
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let cand_x = t_x + dist * angle.cos();
            let cand_z = t_z + dist * angle.sin();
            let g = sampler.height(cand_x, cand_z);
            if g >= MIN_TARGET_GROUND {
                t_x = cand_x;
                t_z = cand_z;
                t_ground = g;
                break;
            }
        }
        // If still below, leave position (will be below threshold but unavoidable); do not force floating
    }
    let phase = rand::random::<f32>() * std::f32::consts::TAU;
    let initial_y = t_ground + level.target.float.base_height + level.target.float.amplitude * phase.sin();
    commands.insert_resource(TargetParams {
        base_height: level.target.float.base_height,
        amplitude: level.target.float.amplitude,
        bob_freq: level.target.float.bob_freq,
        rot_speed: level.target.float.rot_speed,
        collider_radius: level.target.float.collider_radius,
        visual_offset: 3.6, // increased (200% more) lift to keep model clearly above ground
    });
    commands.spawn((
        SceneBundle {
            scene: assets.load(level.target.model.clone()),
            transform: Transform::from_xyz(t_x, initial_y, t_z),
            ..default()
        },
        Target,
        TargetFloat {
            ground: t_ground,
            base_height: level.target.float.base_height,
            amplitude: level.target.float.amplitude,
            phase,
            rot_speed: level.target.float.rot_speed,
            bounce_freq: level.target.float.bob_freq,
        },
    ));

    // Open world: removed enclosing walls

    // Inject ShotConfig override from level
    commands.insert_resource(ShotConfig {
        osc_speed: level.shot.osc_speed,
        base_impulse: level.shot.base_impulse,
        up_angle_deg: level.shot.up_angle_deg,
    });
    if let Some(ref mut s) = score {
        s.max_holes = level.scoring.max_holes;
    }
}

fn track_sky_dome(
    q_cam: Query<&Transform, (With<OrbitCamera>, Without<SkyDome>)>,
    mut q_sky: Query<&mut Transform, (With<SkyDome>, Without<OrbitCamera>)>,
) {
    if let (Ok(cam), Ok(mut sky)) = (q_cam.get_single(), q_sky.get_single_mut()) {
        // Keep sky centered on camera so it appears infinite.
        sky.translation = cam.translation;
    }
}

fn spawn_runtime_ball(
    mut commands: Commands,
    phase: Option<Res<GamePhase>>,
    level: Option<Res<LevelDef>>,
    sampler: Option<Res<TerrainSampler>>,
    assets: Res<AssetServer>,
    q_ball: Query<Entity, With<Ball>>,
) {
    if !matches!(phase.map(|p| *p), Some(GamePhase::Playing)) { return; }
    if q_ball.get_single().is_ok() { return; }
    let (Some(level), Some(sampler)) = (level, sampler) else { return; };

    let ball_pos = Vec3::new(level.ball.pos.x, 0.0, level.ball.pos.z);
    let ground_h = sampler.height(ball_pos.x, ball_pos.z);
    let spawn_y = ground_h + level.ball.collider_radius + level.ball.spawn_height_offset;

    commands.spawn((
        SceneBundle {
            scene: assets.load(level.ball.model.clone()),
            transform: Transform::from_translation(Vec3::new(ball_pos.x, spawn_y, ball_pos.z))
                .with_scale(Vec3::splat(level.ball.visual_scale)),
            ..default()
        },
        Ball,
        BallKinematic {
            collider_radius: level.ball.collider_radius,
            visual_radius: 0.5 * level.ball.visual_scale,
            vel: Vec3::ZERO,
            angular_vel: Vec3::ZERO,
        },
    ));
}


// ----------------------- Utilities -----------------------

fn generate_inverted_sphere(longitudes: u32, latitudes: u32, radius: f32) -> Mesh {
    let longs = longitudes.max(3);
    let lats = latitudes.max(2);
    let mut positions = Vec::with_capacity(((longs + 1) * (lats + 1)) as usize);
    let mut uvs = Vec::with_capacity(positions.capacity());
    let mut normals = Vec::with_capacity(positions.capacity());
    for y in 0..=lats {
        let v = y as f32 / lats as f32;
        let theta = (v - 0.5) * std::f32::consts::PI;
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();
        for x in 0..=longs {
            let u = x as f32 / longs as f32;
            let phi = (u - 0.5) * std::f32::consts::TAU;
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();
            let px = cos_theta * cos_phi;
            let py = sin_theta;
            let pz = cos_theta * sin_phi;
            positions.push([radius * px, radius * py, radius * pz]);
            normals.push([-px, -py, -pz]);
            uvs.push([u, 1.0 - v]);
        }
    }
    let mut indices: Vec<u32> = Vec::with_capacity((longs * lats * 6) as usize);
    let row_stride = longs + 1;
    for y in 0..lats {
        for x in 0..longs {
            let i0 = y * row_stride + x;
            let i1 = i0 + 1;
            let i2 = i0 + row_stride;
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

// Re-exports
pub use FloatParams as FloatParamsExport;
pub use WorldBounds as WorldBoundsExport;
