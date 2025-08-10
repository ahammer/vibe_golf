use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::window::{PrimaryWindow, CursorGrabMode};
use crate::plugins::ball::Ball;
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
    pub follow_lag_speed: f32, // exponential smoothing speed
}

impl Default for OrbitCameraConfig {
    fn default() -> Self {
        Self {
            pitch_min: (-10f32).to_radians(),
            pitch_max: 65f32.to_radians(),
            radius_min: 4.0,
            radius_max: 32.0,
            zoom_speed: 1.0,
            sens_yaw: 0.005,
            sens_pitch: 0.005,
            target_height_offset: 0.3,
            min_clearance: 1.0,
            follow_lag_speed: 6.0,
        }
    }
}

impl Default for OrbitCameraState {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 20f32.to_radians(),
            radius: 22.0,
        }
    }
}

pub struct CameraPlugin;

/// Tracks whether the cursor is currently locked for orbit control.
#[derive(Resource, Default)]
pub struct OrbitCaptureState {
    pub captured: bool,
}
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OrbitCameraConfig::default())
            .insert_resource(OrbitCameraState::default())
            .insert_resource(OrbitCaptureState::default())
            .add_systems(Update, (orbit_camera_capture, orbit_camera_input, orbit_camera_apply));
    }
}

fn orbit_camera_capture(
    buttons: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut cap: ResMut<OrbitCaptureState>,
) {
    if let Ok(mut win) = windows.get_single_mut() {
        let want = buttons.pressed(MouseButton::Right);
        if want && !cap.captured {
            win.cursor.visible = false;
            win.cursor.grab_mode = CursorGrabMode::Locked;
            cap.captured = true;
        } else if !want && cap.captured {
            win.cursor.visible = true;
            win.cursor.grab_mode = CursorGrabMode::None;
            cap.captured = false;
        }
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
    time: Res<Time>,
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

    // Exponential smoothing toward desired position (spring-like lag)
    let alpha = 1.0 - (-cfg.follow_lag_speed * time.delta_seconds()).exp();
    cam_t.translation = cam_t.translation.lerp(desired_pos, alpha);
    cam_t.look_at(target, Vec3::Y);
}
