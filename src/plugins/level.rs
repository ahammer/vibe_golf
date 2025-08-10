// Level loading & world setup (camera, sky, walls, ball, target).
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use serde::Deserialize;
use std::fs;

use crate::plugins::camera::OrbitCamera;
use crate::plugins::ball::{Ball, BallKinematic};
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

#[derive(Component)]
pub struct Wall {
    pub normal: Vec3,
    pub plane_d: f32,
}

// ----------------------- Plugin -----------------------

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_level)
            .add_systems(Startup, spawn_level.after(load_level))
            .add_systems(Update, update_wall_fade);
    }
}

// ----------------------- Systems -----------------------

fn load_level(mut commands: Commands) {
    // Hard-coded single level for now.
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
            ..default()
        },
        OrbitCamera,
    ));

    // Sky
    let sky_tex = assets.load(level.sky.texture.clone());
    let sky_mesh = generate_inverted_sphere(level.sky.longitudes, level.sky.latitudes, level.sky.radius);
    commands.spawn(PbrBundle {
        mesh: meshes.add(sky_mesh),
        material: mats.add(StandardMaterial {
            base_color_texture: Some(sky_tex),
            unlit: true,
            ..default()
        }),
        transform: Transform::IDENTITY,
        ..default()
    });

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

    // Ball spawn
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
            visual_radius: 0.5 * level.ball.visual_scale, // assumes original diameter ~1.0
            vel: Vec3::ZERO,
            angular_vel: Vec3::ZERO,
        },
    ));

    // Target spawn + params resource
    let t_x = level.target.initial.x;
    let t_z = level.target.initial.z;
    let t_ground = sampler.height(t_x, t_z);
    let phase = rand::random::<f32>() * std::f32::consts::TAU;
    let initial_y = t_ground + level.target.float.base_height + level.target.float.amplitude * phase.sin();
    commands.insert_resource(TargetParams {
        base_height: level.target.float.base_height,
        amplitude: level.target.float.amplitude,
        bob_freq: level.target.float.bob_freq,
        rot_speed: level.target.float.rot_speed,
        collider_radius: level.target.float.collider_radius,
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

    // Walls
    let half = level.world.half_extent;
    let height = level.world.wall_height;
    let thickness = 1.0;
    let base_col = level.world.wall_color;
    // X walls
    for &sign in &[-1.0f32, 1.0] {
        let material = mats.add(StandardMaterial {
            base_color: Color::srgba(base_col.0, base_col.1, base_col.2, base_col.3),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Mesh::from(bevy::math::primitives::Cuboid {
                    half_size: Vec3::new(thickness * 0.5, height * 0.5, half),
                })),
                material,
                transform: Transform::from_xyz(sign * half, height * 0.5, 0.0),
                ..default()
            },
            Wall { normal: Vec3::new(sign, 0.0, 0.0), plane_d: sign * half },
        ));
    }
    // Z walls
    for &sign in &[-1.0f32, 1.0] {
        let material = mats.add(StandardMaterial {
            base_color: Color::srgba(base_col.0, base_col.1, base_col.2, base_col.3),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        commands.spawn((
            PbrBundle {
                mesh: meshes.add(Mesh::from(bevy::math::primitives::Cuboid {
                    half_size: Vec3::new(half, height * 0.5, thickness * 0.5),
                })),
                material,
                transform: Transform::from_xyz(0.0, height * 0.5, sign * half),
                ..default()
            },
            Wall { normal: Vec3::new(0.0, 0.0, sign), plane_d: sign * half },
        ));
    }

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

// Fade walls based on proximity to ball
fn update_wall_fade(
    level: Option<Res<LevelDef>>,
    q_ball: Query<&Transform, With<Ball>>,
    mut q_walls: Query<(&Wall, &Handle<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(level), Ok(ball_t)) = (level, q_ball.get_single()) else { return; };
    let bx = ball_t.translation.x.abs();
    let bz = ball_t.translation.z.abs();
    for (wall, mat_handle) in &mut q_walls {
        if let Some(mat) = materials.get_mut(mat_handle) {
            let dist_to_wall = if wall.normal.x != 0.0 {
                (level.world.half_extent - bx).max(0.0)
            } else if wall.normal.z != 0.0 {
                (level.world.half_extent - bz).max(0.0)
            } else {
                level.world.wall_fade_distance
            };
            let alpha = (1.0 - (dist_to_wall / level.world.wall_fade_distance)).clamp(0.0, 1.0);
            let (r, g, b, _) = level.world.wall_color;
            mat.base_color = Color::srgba(r, g, b, alpha);
        }
    }
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
