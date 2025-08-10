use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};

use crate::plugins::ball::Ball;
use crate::plugins::main_menu::GamePhase;
use crate::plugins::terrain::TerrainSampler;

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

impl Default for OrbitCameraState {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 20f32.to_radians(),
            radius: 22.0,
        }
    }
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
    // Maximum linear speeds (units / second) for speed‑limited lerp (currently unused; immediate follow).
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

/// Tracks smoothed follow target for camera (speed limited).
#[derive(Resource)]
pub struct CameraFollow {
    pub smoothed_target: Vec3,
}
impl Default for CameraFollow {
    fn default() -> Self {
        Self {
            smoothed_target: Vec3::ZERO,
        }
    }
}

/// Tracks whether the cursor is currently locked for orbit control.
#[derive(Resource, Default)]
pub struct OrbitCaptureState {
    pub captured: bool,
}

/// Endless menu flight animation state.
/// The camera gently wanders around the origin, changing heading slowly
/// and keeping within a configurable radius. Creates a feeling of flying
/// over the world instead of spinning in place.
#[derive(Resource)]
pub struct MenuCameraFlight {
    pub heading: f32,
    pub pos: Vec3,
    pub t: f32,
}

impl Default for MenuCameraFlight {
    fn default() -> Self {
        Self {
            heading: 0.0,
            pos: Vec3::new(0.0, 14.0, -35.0),
            t: 0.0,
        }
    }
}

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OrbitCameraConfig::default())
            .insert_resource(OrbitCameraState::default())
            .insert_resource(CameraFollow::default())
            .insert_resource(OrbitCaptureState::default())
            .insert_resource(MenuCameraFlight::default())
            .add_systems(
                Update,
                (
                    orbit_camera_capture,
                    orbit_camera_input,
                    menu_camera_flight,
                    orbit_camera_apply,
                ),
            );
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

/// Endless flight while in main menu.
/// The camera:
/// - Moves forward with gentle speed variation
/// - Slowly drifts (heading changes) using layered sine noise
/// - Bobs up/down
/// - Stays within a soft bounding radius, turning back toward the center
/// - Looks toward the world center for a consistent focal point
fn menu_camera_flight(
    time: Res<Time>,
    mut flight: ResMut<MenuCameraFlight>,
    phase: Option<Res<GamePhase>>,
    sampler: Option<Res<TerrainSampler>>,
    mut q_cam: Query<&mut Transform, With<OrbitCamera>>,
) {
    if !matches!(phase.map(|p| *p), Some(GamePhase::Menu)) {
        return;
    }
    let Ok(mut cam_t) = q_cam.get_single_mut() else {
        return;
    };

    let dt = time.delta_seconds();
    let t = {
        flight.t += dt;
        flight.t
    };

    // Heading wander (layered low‑freq sines)
    let wander = (t * 0.05).sin() * 0.35
        + (t * 0.083).cos() * 0.18
        + (t * 0.021).sin() * 0.12;
    flight.heading += wander * dt;

    // Forward velocity (gently varying)
    let base_speed = 26.0;
    let speed_variation = 6.0 * (t * 0.23).sin() + 3.0 * (t * 0.11).cos();
    let speed = (base_speed + speed_variation).max(2.0);

    // Advance position
    let forward_flat = Vec3::new(flight.heading.cos(), 0.0, flight.heading.sin());
    flight.pos += forward_flat * speed * dt;

    // Vertical profile: base + bob + terrain follow
    let bob = (t * 0.37).sin() * 3.0 + (t * 0.19).cos() * 1.2;
    let mut desired_y = 18.0 + bob;
    if let Some(s) = &sampler {
        let ground = s.height(flight.pos.x, flight.pos.z);
        let terrain_clear = ground + 12.0; // keep ~12 units above terrain + bob
        if desired_y < terrain_clear {
            desired_y = terrain_clear;
        }
    }
    // Smooth Y toward desired to avoid abrupt jumps.
    flight.pos.y = flight.pos.y.lerp(desired_y, (dt * 1.5).clamp(0.0, 1.0));

    cam_t.translation = flight.pos;

    // Look ahead along path with slight downward tilt.
    let look_target = flight.pos + forward_flat * 60.0 + Vec3::new(0.0, -8.0, 0.0);
    cam_t.look_at(look_target, Vec3::Y);
}

/// Apply gameplay camera follow with speed limits (position & target smoothing).
fn orbit_camera_apply(
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

    let Ok(ball_t) = q_ball.get_single() else {
        return;
    };
    let Ok(mut cam_t) = q_cam.get_single_mut() else {
        return;
    };

    let raw_target = ball_t.translation + Vec3::Y * cfg.target_height_offset;

    // Immediate target (no smoothing / perfect follow)
    follow.smoothed_target = raw_target;

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
