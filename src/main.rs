use bevy::prelude::*;
use bevy::math::primitives::{Cuboid, Sphere};
use bevy_rapier3d::prelude::*;

// Simulation timing state now tracks real elapsed seconds & frame count (no fixed 60 Hz tick).
#[derive(Resource, Default)]
struct SimState {
    frames: u64,
    elapsed_seconds: f32,
}
impl SimState {
    fn advance(&mut self, delta: f32) {
        self.frames += 1;
        self.elapsed_seconds += delta;
    }
}

// Auto-play / instrumentation configuration (now time-based rather than tick-based)
#[derive(Resource)]
struct AutoConfig {
    run_duration_seconds: f32,   // total seconds before exit
    swing_interval_seconds: f32, // interval between scripted swings
    base_impulse: f32,         // magnitude of impulse per swing
    upward_factor: f32,        // Y component factor
}
impl Default for AutoConfig {
    fn default() -> Self { Self { run_duration_seconds: 20.0, swing_interval_seconds: 3.0, base_impulse: 6.0, upward_factor: 0.15 } }
}

// Runtime auto-play state (tracks the next scheduled swing time in seconds)
#[derive(Resource, Default)]
struct AutoRuntime { next_swing_time: f32 }

// Logging helper to ensure we only print once per integer second.
#[derive(Resource, Default)]
struct LogState { last_logged_second: u64 }

#[derive(Component)]
struct Ball;

#[derive(Component)]
struct Hud;

// Tag the active gameplay camera with follow params (kept minimal & deterministic).
#[derive(Component)]
struct CameraFollow {
    distance: f32,     // horizontal distance behind ball
    height: f32,       // vertical offset above ball
    lerp_factor: f32,  // interpolation fraction each frame (render schedule)
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.52, 0.80, 0.92)))
        .insert_resource(SimState::default())
        .insert_resource(AutoConfig::default())
        .insert_resource(AutoRuntime::default())
        .insert_resource(LogState::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Vibe Golf".into(), ..default() }),
            ..default()
        }))
        // physics
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        // .add_plugins(RapierDebugRenderPlugin::default())
        // scene setup
        .add_systems(Startup, (setup_scene, setup_ui))
        // per-frame systems (natural frame rate)
        .add_systems(Update, (
            advance_sim_time,
            scripted_autoplay,
            debug_log_each_second,
            exit_on_duration,
            update_hud,
            camera_follow,
        ))
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // camera
    commands.spawn((Camera3dBundle {
        transform: Transform::from_xyz(-12.0, 10.0, 18.0)
            .looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
        ..default()
    }, CameraFollow { distance: 12.5, height: 6.0, lerp_factor: 0.10 }));

    // light
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight { illuminance: 25_000.0, shadows_enabled: true, ..default() },
        transform: Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // ground (rendered as a very flat cuboid)
    let ground_size = Vec3::new(200.0, 0.2, 200.0);
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::from_size(ground_size))),
            material: mats.add(Color::srgb(0.25, 0.55, 0.25)),
            transform: Transform::from_xyz(0.0, -0.1, 0.0),
            ..default()
        })
        .insert(RigidBody::Fixed)
        .insert(Collider::cuboid(
            ground_size.x * 0.5,
            ground_size.y * 0.5,
            ground_size.z * 0.5,
        ));

    // ball
    let ball_radius = 0.25;
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(Sphere { radius: ball_radius })),
            material: mats.add(Color::srgb(0.95, 0.95, 0.95)),
            transform: Transform::from_xyz(-5.0, 1.0, -5.0),
            ..default()
        })
        .insert(Ball)
        .insert(RigidBody::Dynamic)
        .insert(Collider::ball(ball_radius))
        .insert(Restitution::coefficient(0.6))
        .insert(Damping { linear_damping: 0.05, angular_damping: 0.05 });

    // target cube
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::from_size(Vec3::splat(1.0)))),
            material: mats.add(Color::srgb(0.9, 0.2, 0.2)),
            transform: Transform::from_xyz(6.0, 0.5, 7.0),
            ..default()
        })
        .insert(RigidBody::Fixed)
        .insert(Collider::cuboid(0.5, 0.5, 0.5));
}

fn setup_ui(mut commands: Commands, assets: Res<AssetServer>) {
    // Put a TTF at assets/fonts/FiraSans-Bold.ttf
    let font = assets.load("fonts/FiraSans-Bold.ttf");
    commands
        .spawn(
            TextBundle::from_section(
                "Tick: 0 | Speed: 0.00 m/s",
                TextStyle { font, font_size: 22.0, color: Color::WHITE },
            )
            .with_style(Style { position_type: PositionType::Absolute, left: Val::Px(12.0), top: Val::Px(8.0), ..default() }),
        )
        .insert(Hud);
}

fn advance_sim_time(mut sim: ResMut<SimState>, time: Res<Time>) { sim.advance(time.delta_seconds()); }

// Periodically fire an impulse to move the ball, simulating a scripted auto-play.
fn scripted_autoplay(
    sim: Res<SimState>,
    mut runtime: ResMut<AutoRuntime>,
    cfg: Res<AutoConfig>,
    mut commands: Commands,
    q_ball: Query<(Entity, &Transform), With<Ball>>,
) {
    if sim.elapsed_seconds < runtime.next_swing_time { return; }
    if let Ok((entity, transform)) = q_ball.get_single() {
        // Derive a deterministic angle from the number of swings so far
        let swings_done = (runtime.next_swing_time / cfg.swing_interval_seconds).round() as u64;
        let angle = (swings_done as f32 * 13.0).to_radians();
        let dir_flat = Vec3::new(angle.cos(), 0.0, angle.sin()).normalize();
        let impulse = dir_flat * cfg.base_impulse + Vec3::Y * (cfg.base_impulse * cfg.upward_factor);
        commands.entity(entity).insert(ExternalImpulse { impulse, torque_impulse: Vec3::ZERO });
        info!(
            "AUTOPLAY swing t={:.2}s pos=({:.2},{:.2},{:.2}) impulse=({:.2},{:.2},{:.2})",
            sim.elapsed_seconds,
            transform.translation.x,
            transform.translation.y,
            transform.translation.z,
            impulse.x,
            impulse.y,
            impulse.z
        );
    }
    // schedule next swing
    runtime.next_swing_time += cfg.swing_interval_seconds;
}

// Log basic telemetry each in-game second.
fn debug_log_each_second(
    sim: Res<SimState>,
    mut log_state: ResMut<LogState>,
    q_ball: Query<(&Transform, &Velocity), With<Ball>>,
) {
    let current_second = sim.elapsed_seconds as u64;
    if current_second == 0 || current_second == log_state.last_logged_second { return; }
    log_state.last_logged_second = current_second;
    if let Ok((t, vel)) = q_ball.get_single() {
        info!(
            "T+{}s frames={} ball=({:.2},{:.2},{:.2}) speed={:.2}",
            current_second,
            sim.frames,
            t.translation.x,
            t.translation.y,
            t.translation.z,
            vel.linvel.length()
        );
    }
}

// Exit automatically after configured duration for automation / CI.
fn exit_on_duration(sim: Res<SimState>, cfg: Res<AutoConfig>, mut exit: EventWriter<AppExit>) {
    if sim.elapsed_seconds >= cfg.run_duration_seconds { exit.send(AppExit::Success); }
}

fn update_hud(
    sim: Res<SimState>,
    q_vel: Query<&Velocity, With<Ball>>,
    mut q_text: Query<&mut Text, With<Hud>>,
) {
    if let (Ok(vel), Ok(mut text)) = (q_vel.get_single(), q_text.get_single_mut()) {
        let speed = vel.linvel.length();
        text.sections[0].value = format!("Time: {:.2}s | Speed: {:.2} m/s", sim.elapsed_seconds, speed);
    }
}

// Smoothly move & orient the camera toward the ball each frame (render schedule only).
fn camera_follow(
    q_ball: Query<(&Transform, Option<&Velocity>), (With<Ball>, Without<CameraFollow>)>,
    mut q_cam: Query<(&mut Transform, &CameraFollow), Without<Ball>>,
) {
    let Ok((ball_t, vel_opt)) = q_ball.get_single() else { return; };
    let Ok((mut cam_t, follow)) = q_cam.get_single_mut() else { return; };

    // Determine horizontal forward direction based on velocity; fall back to current relative vector.
    let mut forward = vel_opt
        .and_then(|v| {
            let horiz = Vec3::new(v.linvel.x, 0.0, v.linvel.z);
            if horiz.length_squared() > 0.05 { Some(horiz.normalize()) } else { None }
        })
        .unwrap_or_else(|| {
            let rel = (ball_t.translation - cam_t.translation) * Vec3::new(1.0, 0.0, 1.0);
            if rel.length_squared() > 0.01 { rel.normalize() } else { Vec3::Z } // default
        });
    // Target camera position: behind the ball opposite forward.
    let desired = ball_t.translation - forward * follow.distance + Vec3::Y * follow.height;
    cam_t.translation = cam_t.translation.lerp(desired, follow.lerp_factor);
    // Always look slightly above the ball center.
    cam_t.look_at(ball_t.translation + Vec3::Y * 0.3, Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sim_state_advances() {
        let mut s = SimState::default();
        for _ in 0..5 { s.advance(0.016); }
        assert_eq!(s.frames, 5);
        assert!((s.elapsed_seconds - 0.08).abs() < 1e-6);
    }
}
