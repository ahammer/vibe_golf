use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use crate::plugins::scene::Ball;
use crate::plugins::terrain::TerrainSampler;

/// Marker component for the single orbit camera.
#[derive(Component)]
pub struct OrbitCamera;

/// Runtime mutable orbit state.
#[derive(Resource)]
pub struct OrbitCameraState {
    pub yaw: f32,    // radians
    pub pitch: f32,  // radians
    pub radius: f32, // world units
}

/// Configuration constants for orbit behavior & constraints.
#[derive(Resource)]
pub struct OrbitCameraConfig {
    pub pitch_min: f32,
    pub pitch_max: f32,
    pub radius_min: f32,
    pub radius_max: f32,
    pub zoom_speed: f32,
    pub sens_yaw: f32,
    pub sens_pitch: f32,
    pub target_height_offset: f32,
    pub min_clearance: f32,
}

impl Default for OrbitCameraConfig {
    fn default() -> Self {
        Self {
            pitch_min: (-10f32).to_radians(),
            pitch_max: 65f32.to_radians(),
            radius_min: 4.0,
            radius_max: 18.0,
            zoom_speed: 1.0,
            sens_yaw: 0.005,
            sens_pitch: 0.005,
            target_height_offset: 0.3,
            min_clearance: 1.0,
        }
    }
}

impl Default for OrbitCameraState {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 20f32.to_radians(),
            radius: 12.0,
        }
    }
}

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OrbitCameraConfig::default())
            .insert_resource(OrbitCameraState::default())
            .add_systems(Update, (orbit_camera_input, orbit_camera_apply));
    }
}

/// Process mouse input to update orbit state (yaw, pitch, radius).
fn orbit_camera_input(
    mut state: ResMut<OrbitCameraState>,
    cfg: Res<OrbitCameraConfig>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut ev_motion: EventReader<MouseMotion>,
    mut ev_wheel: EventReader<MouseWheel>,
) {
    // Scroll wheel zoom
    for w in ev_wheel.read() {
        // Bevy's MouseWheel.y is vertical scroll (line or pixel units)
        let delta = w.y * cfg.zoom_speed;
        state.radius = (state.radius - delta).clamp(cfg.radius_min, cfg.radius_max);
    }

    // Right mouse drag to adjust yaw/pitch
    if buttons.pressed(MouseButton::Right) {
        for m in ev_motion.read() {
            state.yaw -= m.delta.x * cfg.sens_yaw; // invert horizontal if desired
            state.pitch -= m.delta.y * cfg.sens_pitch;
        }
        // Clamp pitch
        if state.pitch < cfg.pitch_min {
            state.pitch = cfg.pitch_min;
        } else if state.pitch > cfg.pitch_max {
            state.pitch = cfg.pitch_max;
        }
    }
}

/// Apply orbit transform each frame after input.
fn orbit_camera_apply(
    state: Res<OrbitCameraState>,
    cfg: Res<OrbitCameraConfig>,
    sampler: Option<Res<TerrainSampler>>,
    q_ball: Query<&Transform, With<Ball>>,
    mut q_cam: Query<&mut Transform, (With<OrbitCamera>, Without<Ball>)>,
) {
    let Ok(ball_t) = q_ball.get_single() else { return; };
    let Ok(mut cam_t) = q_cam.get_single_mut() else { return; };

    let target = ball_t.translation + Vec3::Y * cfg.target_height_offset;

    // Spherical offset: start pointing along -Z (camera looks toward origin),
    // rotate by yaw around Y, then pitch around X.
    let rot = Quat::from_rotation_y(state.yaw) * Quat::from_rotation_x(state.pitch);
    let offset = rot * (Vec3::Z * state.radius);
    let mut desired_pos = target + offset;

    // Terrain clearance (optional)
    if let Some(s) = &sampler {
        let ground_y = s.height(desired_pos.x, desired_pos.z);
        if desired_pos.y < ground_y + cfg.min_clearance {
            desired_pos.y = ground_y + cfg.min_clearance;
        }
    }

    cam_t.translation = desired_pos;
    cam_t.look_at(target, Vec3::Y);
}
