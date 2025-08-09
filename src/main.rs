use bevy::prelude::*;
use bevy::math::primitives::{Cuboid, Sphere};
use bevy_rapier3d::prelude::*;

#[derive(Resource, Default)]
struct SimState {
    tick: u64,
}

impl SimState {
    fn step(&mut self) { self.tick += 1; }
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
        .add_systems(FixedUpdate, (tick_state, maybe_nudge_ball))
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

fn maybe_nudge_ball(
    sim: Res<SimState>,
    mut commands: Commands,
    q_ball: Query<Entity, With<Ball>>,
) {
    if sim.tick == 1 {
        if let Ok(entity) = q_ball.get_single() {
            commands.entity(entity).insert(ExternalImpulse {
                impulse: Vec3::new(2.0, 0.0, 3.0),
                torque_impulse: Vec3::ZERO,
            });
        }
    }
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
