use bevy::prelude::*;
use bevy::math::primitives::{Cuboid, Sphere};
use bevy_rapier3d::prelude::*;

#[derive(Resource, Default)]
struct SimState { tick: u64 }
impl SimState { fn step(&mut self) { self.tick += 1; } }

// Auto-play / instrumentation configuration
#[derive(Resource)]
struct AutoConfig {
    run_duration_ticks: u64,   // total fixed ticks before exit
    swing_interval_ticks: u64, // interval between scripted swings
    base_impulse: f32,         // magnitude of impulse per swing
    upward_factor: f32,        // Y component factor
}
impl Default for AutoConfig {
    fn default() -> Self { Self { run_duration_ticks: 60*20, swing_interval_ticks: 180, base_impulse: 6.0, upward_factor: 0.15 } }
}

#[derive(Component)]
struct Ball;

#[derive(Component)]
struct Hud;

fn main() {
    App::new()
        // fixed tick for game logic
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .insert_resource(ClearColor(Color::srgb(0.52, 0.80, 0.92)))
        .insert_resource(SimState::default())
    .insert_resource(AutoConfig::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Vibe Golf".into(), ..default() }),
            ..default()
        }))
        // physics
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        // .add_plugins(RapierDebugRenderPlugin::default())
        // scene setup
        .add_systems(Startup, (setup_scene, setup_ui))
        // fixed-tick simulation
    .add_systems(FixedUpdate, (tick_state, scripted_autoplay, debug_log_each_second, exit_on_duration))
        // per-frame render-side updates
        .add_systems(Update, update_hud)
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-12.0, 10.0, 18.0)
            .looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
        ..default()
    });

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

fn tick_state(mut sim: ResMut<SimState>) {
    sim.step();
}

// Periodically fire an impulse to move the ball, simulating a scripted auto-play.
fn scripted_autoplay(
    sim: Res<SimState>,
    cfg: Res<AutoConfig>,
    mut commands: Commands,
    q_ball: Query<(Entity, &Transform), With<Ball>>,
) {
    if sim.tick == 0 { return; }
    if sim.tick % cfg.swing_interval_ticks != 5 { return; }
    if let Ok((entity, transform)) = q_ball.get_single() {
        // Aim roughly toward +X+Z direction but add slight variation based on tick
        let angle = (sim.tick as f32 * 13.0).to_radians();
        let dir_flat = Vec3::new(angle.cos(), 0.0, angle.sin()).normalize();
        let impulse = dir_flat * cfg.base_impulse + Vec3::Y * (cfg.base_impulse * cfg.upward_factor);
        commands.entity(entity).insert(ExternalImpulse { impulse, torque_impulse: Vec3::ZERO });
        info!("AUTOPLAY swing at tick={} pos=({:.2},{:.2},{:.2}) impulse=({:.2},{:.2},{:.2})", sim.tick, transform.translation.x, transform.translation.y, transform.translation.z, impulse.x, impulse.y, impulse.z);
    }
}

// Log basic telemetry each in-game second.
fn debug_log_each_second(
    sim: Res<SimState>,
    q_ball: Query<(&Transform, &Velocity), With<Ball>>,
) {
    if sim.tick % 60 != 0 { return; }
    if let Ok((t, vel)) = q_ball.get_single() {
        info!("T+{:.1}s tick={} ball=({:.2},{:.2},{:.2}) speed={:.2}", sim.tick as f32 / 60.0, sim.tick, t.translation.x, t.translation.y, t.translation.z, vel.linvel.length());
    }
}

// Exit automatically after configured duration for automation / CI.
fn exit_on_duration(sim: Res<SimState>, cfg: Res<AutoConfig>, mut exit: EventWriter<AppExit>) {
    if sim.tick >= cfg.run_duration_ticks { exit.send(AppExit::Success); }
}

fn update_hud(
    sim: Res<SimState>,
    q_vel: Query<&Velocity, With<Ball>>,
    mut q_text: Query<&mut Text, With<Hud>>,
) {
    if let (Ok(vel), Ok(mut text)) = (q_vel.get_single(), q_text.get_single_mut()) {
        let speed = vel.linvel.length();
        text.sections[0].value = format!("Tick: {} | Speed: {:.2} m/s", sim.tick, speed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sim_state_steps() {
        let mut s = SimState::default();
        for _ in 0..5 { s.step(); }
        assert_eq!(s.tick, 5);
    }
}
