use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::window::{PrimaryWindow, CursorGrabMode};
use crate::plugins::ball::Ball;
use crate::plugins::terrain::TerrainSampler;
use crate::plugins::main_menu::GamePhase;

/// Marker component for the single orbit camera.
#[derive(Component)]
pub struct OrbitCamera;

/// Runtime mutable orbit state (user-controlled angles & zoom during gameplay).
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
    // Maximum linear speeds (units / second) for speed‑limited lerp
    pub cam_max_speed: f32,
    pub target_max_speed: f32, // should be >= cam_max_speed (spec: 2x)
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
            cam_max_speed: 20.0,
            target_max_speed: 40.0,
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

/// Tracks smoothed follow target for camera (speed limited).
#[derive(Resource)]
pub struct CameraFollow {
    pub smoothed_target: Vec3,
}
impl Default for CameraFollow {
    fn default() -> Self {
        Self { smoothed_target: Vec3::ZERO }
    }
}

/// Tracks whether the cursor is currently locked for orbit control.
#[derive(Resource, Default)]
pub struct OrbitCaptureState {
    pub captured: bool,
}

/// Menu orbit animation state.
#[derive(Resource, Default)]
pub struct MenuCameraOrbit {
    pub angle: f32,
}

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OrbitCameraConfig::default())
            .insert_resource(OrbitCameraState::default())
            .insert_resource(CameraFollow::default())
            .insert_resource(OrbitCaptureState::default())
            .insert_resource(MenuCameraOrbit::default())
            .add_systems(Update, (
                orbit_camera_capture,
                orbit_camera_input,
                menu_camera_orbit,
                orbit_camera_apply,
            ));
    }
}

fn orbit_camera_capture(
    buttons: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut cap: ResMut<OrbitCaptureState>,
    phase: Option<Res<GamePhase>>,
) {
    // Disable capture in menu.
    if matches!(phase.map(|p| *p), Some(GamePhase::Menu)) {
        if cap.captured {
            if let Ok(mut win) = windows.get_single_mut() {
                win.cursor.visible = true;
                win.cursor.grab_mode = CursorGrabMode::None;
            }
            cap.captured = false;
        }
        return;
    }

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

/// Process mouse input to update orbit state (yaw, pitch, radius) only in gameplay.
fn orbit_camera_input(
    mut state: ResMut<OrbitCameraState>,
    cfg: Res<OrbitCameraConfig>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut ev_motion: EventReader<MouseMotion>,
    mut ev_wheel: EventReader<MouseWheel>,
    phase: Option<Res<GamePhase>>,
) {
    if matches!(phase.map(|p| *p), Some(GamePhase::Menu)) {
        return;
    }

    // Scroll wheel zoom
    for w in ev_wheel.read() {
        let delta = w.y * cfg.zoom_speed;
        state.radius = (state.radius - delta).clamp(cfg.radius_min, cfg.radius_max);
    }

    // Right mouse drag to adjust yaw/pitch
    if buttons.pressed(MouseButton::Right) {
        for m in ev_motion.read() {
            state.yaw -= m.delta.x * cfg.sens_yaw;
            state.pitch -= m.delta.y * cfg.sens_pitch;
        }
        // Clamp pitch
        state.pitch = state.pitch.clamp(cfg.pitch_min, cfg.pitch_max);
    }
}

/// Automatic orbit while in main menu (ball absent).
fn menu_camera_orbit(
    time: Res<Time>,
    mut orbit: ResMut<MenuCameraOrbit>,
    phase: Option<Res<GamePhase>>,
    mut q_cam: Query<&mut Transform, With<OrbitCamera>>,
) {
    if !matches!(phase.map(|p| *p), Some(GamePhase::Menu)) {
        return;
    }
    let Ok(mut cam_t) = q_cam.get_single_mut() else { return; };
    let radius = 22.0;
    let speed = 0.25; // rad/s
    orbit.angle = (orbit.angle + speed * time.delta_seconds()) % (std::f32::consts::TAU);
    let x = orbit.angle.cos() * radius;
    let z = orbit.angle.sin() * radius;
    let y = 10.0;
    cam_t.translation = Vec3::new(x, y, z);
    cam_t.look_at(Vec3::ZERO, Vec3::Y);
}

/// Apply gameplay camera follow with speed limits (position & target smoothing).
fn orbit_camera_apply(
    time: Res<Time>,
    state: Res<OrbitCameraState>,
    cfg: Res<OrbitCameraConfig>,
    sampler: Option<Res<TerrainSampler>>,
    phase: Option<Res<GamePhase>>,
    mut follow: ResMut<CameraFollow>,
    q_ball: Query<&Transform, With<Ball>>,
    mut q_cam: Query<&mut Transform, (With<OrbitCamera>, Without<Ball>)>,
) {
    // Skip if not in gameplay phase.
    if !matches!(phase.map(|p| *p), Some(GamePhase::Playing)) {
        return;
    }

    let Ok(ball_t) = q_ball.get_single() else { return; };
    let Ok(mut cam_t) = q_cam.get_single_mut() else { return; };

    let raw_target = ball_t.translation + Vec3::Y * cfg.target_height_offset;

    // Speed-limited target smoothing
    let dt = time.delta_seconds();
    let max_target_step = cfg.target_max_speed * dt;
    let to_target = raw_target - follow.smoothed_target;
    let dist_t = to_target.length();
    if dist_t <= max_target_step || dist_t == 0.0 {
        follow.smoothed_target = raw_target;
    } else {
        follow.smoothed_target += to_target / dist_t * max_target_step;
    }

    // Desired camera position (based on smoothed target)
    let rot = Quat::from_rotation_y(state.yaw) * Quat::from_rotation_x(state.pitch);
    let offset = rot * (Vec3::Z * state.radius);
    let mut desired_pos = follow.smoothed_target + offset;

    // Terrain clearance (optional)
    if let Some(s) = &sampler {
        let ground_y = s.height(desired_pos.x, desired_pos.z);
        if desired_pos.y < ground_y + cfg.min_clearance {
            desired_pos.y = ground_y + cfg.min_clearance;
        }
    }

    // Immediate camera position (no speed limit) so orientation & controls remain responsive.
    cam_t.translation = desired_pos;
    cam_t.look_at(follow.smoothed_target, Vec3::Y);
}
