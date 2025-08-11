use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::input::touch::TouchInput;
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
            // Higher initial pitch for elevated vantage
            pitch: 50f32.to_radians(),
            // Larger default radius so camera starts farther and higher
            radius: 55.0,
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
    // Spring constants (higher = snappier)
    pub follow_spring: f32,
    pub camera_spring: f32,
    // Legacy speed limits (still available, unused in spring mode)
    pub cam_max_speed: f32,
    pub target_max_speed: f32, // should be >= cam_max_speed (spec: 2x)
}

impl Default for OrbitCameraConfig {
    fn default() -> Self {
        Self {
            pitch_min: 5f32.to_radians(),
            pitch_max: 85f32.to_radians(),
            radius_min: 4.0,
            radius_max: 100.0,
            zoom_speed: 1.0,
            sens_yaw: 0.005,
            sens_pitch: 0.005,
            // Raise follow point a bit so even low pitches keep camera higher
            target_height_offset: 1.0,
            min_clearance: 1.0,
            // Increased for tighter, faster convergence
            follow_spring: 60.0,
            camera_spring: 6.0,
            cam_max_speed: 20.0,
            target_max_speed: 40.0,
        }
    }
}

/// Tracks smoothed follow target for camera (speed limited).
#[derive(Resource)]
pub struct CameraFollow {
    pub target: Vec3, // desired (raw) follow target (ball)
    pub actual: Vec3, // smoothed (tweened) follow target
    pub initialized: bool,
}
impl Default for CameraFollow {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            actual: Vec3::ZERO,
            initialized: false,
        }
    }
}

#[derive(Resource, Default)]
pub struct CameraActual {
    pub target: Vec3, // desired camera position
    pub actual: Vec3, // smoothed camera position
    pub initialized: bool,
}

/// Tracks whether the cursor is currently locked for orbit control.
#[derive(Resource, Default)]
pub struct OrbitCaptureState {
    pub captured: bool,
}

/// Single-finger orbit swipe tracking.
#[derive(Resource, Default)]
pub struct TouchOrbit {
    pub active_id: Option<u64>,
    pub last_pos: Vec2,
    pub look_active: bool,
}

/// Pinch zoom tracking (two-finger).
#[derive(Resource, Default)]
pub struct PinchZoom {
    pub id1: Option<u64>,
    pub id2: Option<u64>,
    pub pos1: Vec2,
    pub pos2: Vec2,
    pub initial_distance: f32,
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
            .insert_resource(CameraActual::default())
            .insert_resource(OrbitCaptureState::default())
            .insert_resource(MenuCameraFlight::default())
            .insert_resource(TouchOrbit::default())
            .insert_resource(PinchZoom::default())
            .add_systems(
                Update,
                (
                    orbit_camera_capture,
                    orbit_camera_input,
                    menu_camera_flight,
                    camera_phase_transition,
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
    mut ev_touch: EventReader<TouchInput>,
    mut touch_orbit: ResMut<TouchOrbit>,
    mut pinch: ResMut<PinchZoom>,
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

    // Touch processing (swipe to look, pinch to zoom)
    for ev in ev_touch.read() {
        match ev.phase {
            bevy::input::touch::TouchPhase::Started => {
                // Pinch setup
                if pinch.id1.is_none() {
                    pinch.id1 = Some(ev.id);
                    pinch.pos1 = ev.position;
                } else if pinch.id2.is_none() && pinch.id1 != Some(ev.id) {
                    pinch.id2 = Some(ev.id);
                    pinch.pos2 = ev.position;
                    pinch.initial_distance = (pinch.pos2 - pinch.pos1).length().max(1.0);
                }
                // Orbit swipe setup (only if no active orbit finger and not part of pinch yet)
                if touch_orbit.active_id.is_none() && (pinch.id1 == Some(ev.id) && pinch.id2.is_none()) {
                    touch_orbit.active_id = Some(ev.id);
                    touch_orbit.last_pos = ev.position;
                    touch_orbit.look_active = false;
                }
            }
            bevy::input::touch::TouchPhase::Moved => {
                // Update pinch positions
                if pinch.id1 == Some(ev.id) {
                    pinch.pos1 = ev.position;
                } else if pinch.id2 == Some(ev.id) {
                    pinch.pos2 = ev.position;
                }
                // Pinch zoom
                if pinch.id1.is_some() && pinch.id2.is_some() {
                    let current = (pinch.pos2 - pinch.pos1).length().max(1.0);
                    let diff = current - pinch.initial_distance;
                    // Scale radius inversely to pinch distance change
                    state.radius = (state.radius - diff * 0.05 * cfg.zoom_speed)
                        .clamp(cfg.radius_min, cfg.radius_max);
                    pinch.initial_distance = current;
                } else if touch_orbit.active_id == Some(ev.id) {
                    // Single finger orbit
                    let delta = ev.position - touch_orbit.last_pos;
                    // Activate look after small threshold to avoid accidental shot cancel
                    if delta.length() > 2.0 {
                        touch_orbit.look_active = true;
                    }
                    if touch_orbit.look_active {
                        state.yaw -= delta.x * cfg.sens_yaw * 0.6;
                        state.pitch -= delta.y * cfg.sens_pitch * 0.6;
                        state.pitch = state.pitch.clamp(cfg.pitch_min, cfg.pitch_max);
                    }
                    touch_orbit.last_pos = ev.position;
                }
            }
            bevy::input::touch::TouchPhase::Ended | bevy::input::touch::TouchPhase::Canceled => {
                if pinch.id1 == Some(ev.id) || pinch.id2 == Some(ev.id) {
                    // Reset pinch state completely
                    pinch.id1 = None;
                    pinch.id2 = None;
                }
                if touch_orbit.active_id == Some(ev.id) {
                    touch_orbit.active_id = None;
                    touch_orbit.look_active = false;
                }
            }
        }
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
    // Only active in menu.
    if !matches!(phase.map(|p| *p), Some(GamePhase::Menu)) {
        return;
    }
    let Ok(mut cam_t) = q_cam.get_single_mut() else {
        return;
    };

    let dt = time.delta_seconds();
    flight.t += dt;

    if let Some(s) = &sampler {
        // Heightmap spans [-world_size/2, +world_size/2] in both X & Z.
        let world_size = s.cfg.heightmap_world_size;
        let half = world_size * 0.5;

        // Radius chosen to comfortably fit full island (diagonal ~ world_size * sqrt(2)).
        // Extra padding factor so edges remain in frame.
        let radius = half * 1.3; // e.g. 1000 * 1.3 = 1300

        // Base altitude and gentle vertical bob.
        let base_height = half * 0.8; // e.g. 800 for 2 km world
        let bob = (flight.t * 0.15).sin() * 40.0 + (flight.t * 0.07).cos() * 25.0;
        let y = base_height + bob;

        // Constant angular velocity for smooth orbit (~60s per revolution).
        let ang_speed = std::f32::consts::TAU / 60.0; // rad/sec
        flight.heading = (flight.heading + ang_speed * dt) % std::f32::consts::TAU;

        // Position on horizontal circle.
        let x = radius * flight.heading.cos();
        let z = radius * flight.heading.sin();
        flight.pos = Vec3::new(x, y, z);

        // Focus point: center of island slightly above ground for nicer composition.
        let center_height = s.height(0.0, 0.0);
        let focus = Vec3::new(0.0, center_height - 150.0, 0.0);

        cam_t.translation = flight.pos;
        cam_t.look_at(focus, Vec3::Y);
    } else {
        // Fallback (no sampler yet): simple slow spin at fixed params.
        let ang_speed = std::f32::consts::TAU / 80.0;
        flight.heading = (flight.heading + ang_speed * dt) % std::f32::consts::TAU;
        let radius = 800.0;
        let y = 600.0 + (flight.t * 0.2).sin() * 50.0;
        let x = radius * flight.heading.cos();
        let z = radius * flight.heading.sin();
        flight.pos = Vec3::new(x, y, z);
        cam_t.translation = flight.pos;
        cam_t.look_at(Vec3::new(0.0, -150.0, 0.0), Vec3::Y);
    }
}

fn camera_phase_transition(
    phase: Option<Res<GamePhase>>,
    mut last: Local<Option<GamePhase>>,
    mut q_cam: Query<&mut Transform, With<OrbitCamera>>,
    mut follow: ResMut<CameraFollow>,
    mut actual: ResMut<CameraActual>,
) {
    let current = phase.map(|p| *p);
    if current != *last {
        if matches!(current, Some(GamePhase::Playing)) {
            if let Ok(mut t) = q_cam.get_single_mut() {
                // High-altitude initial spawn to show whole landscape
                t.translation = Vec3::new(0.0, 1000.0, 0.0);
            }
            follow.initialized = false;
            actual.initialized = false;
        }
        *last = current;
    }
}

/// Apply gameplay camera follow with speed limits (position & target smoothing).
fn orbit_camera_apply(
    time: Res<Time>,
    state: Res<OrbitCameraState>,
    cfg: Res<OrbitCameraConfig>,
    sampler: Option<Res<TerrainSampler>>,
    phase: Option<Res<GamePhase>>,
    mut follow: ResMut<CameraFollow>,
    mut actual: ResMut<CameraActual>,
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
    follow.target = raw_target;

    // Spring smoothing for follow target (magnetically attracted)
    if !follow.initialized {
        follow.actual = follow.target;
        follow.initialized = true;
    } else {
        let dt = time.delta_seconds();
        let k = cfg.follow_spring;
        let alpha = 1.0 - (-k * dt).exp();
        let target = follow.target;
        let current = follow.actual;
        follow.actual = current + (target - current) * alpha;
    }

    // Desired camera position (spherical from yaw/pitch so positive pitch raises camera)
    // pitch in [0, ~pi/2]: 0 = horizontal, increasing -> higher
    let yaw = state.yaw;
    let pitch = state.pitch;
    let dir = Vec3::new(
        pitch.cos() * yaw.sin(),
        pitch.sin(),
        pitch.cos() * yaw.cos(),
    );
    let mut desired_pos = follow.actual + dir * state.radius;

    // Terrain clearance (optional)
    if let Some(s) = &sampler {
        let ground_y = s.height(desired_pos.x, desired_pos.z);
        if desired_pos.y < ground_y + cfg.min_clearance {
            desired_pos.y = ground_y + cfg.min_clearance;
        }
    }
    actual.target = desired_pos;

    // Spring camera position toward desired target; keep initial high spawn for descent animation
    if !actual.initialized {
        actual.actual = cam_t.translation;
        actual.initialized = true;
    } else {
        let dt = time.delta_seconds();
        let k = cfg.camera_spring;
        let alpha = 1.0 - (-k * dt).exp();
        let target = actual.target;
        let current = actual.actual;
        actual.actual = current + (target - current) * alpha;
    }
    cam_t.translation = actual.actual;
    cam_t.look_at(follow.actual, Vec3::Y);
}
