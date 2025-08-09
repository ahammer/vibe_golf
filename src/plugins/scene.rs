use bevy::prelude::*;
use bevy::math::primitives::{Cuboid, Sphere};
use bevy_rapier3d::prelude::*;
use crate::plugins::terrain::TerrainSampler;

#[derive(Component)]
pub struct Ball;
#[derive(Component)]
pub struct Hud;
#[derive(Component)]
pub struct BallKinematic {
    pub radius: f32,
    pub vel: Vec3,
}
#[derive(Component)]
pub struct CameraFollow {
    pub distance: f32,
    pub height: f32,
    pub lerp_factor: f32,
}

pub struct ScenePlugin;
impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_scene, setup_ui));
        app.add_systems(FixedUpdate, simple_ball_physics);
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    sampler: Res<TerrainSampler>,
) {
    // camera
    commands.spawn((Camera3dBundle {
        transform: Transform::from_xyz(-12.0, 10.0, 18.0)
            .looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
        ..default()
    }, CameraFollow { distance: 12.5, height: 6.0, lerp_factor: 0.10 }));

    // light
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight { illuminance: 25_000.0, shadows_enabled: false, ..default() },
        transform: Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // ball (manual kinematic vertical drop with sampler collision)
    let ball_radius = 0.25;
    let x = 0.0;
    let z = 0.0;
    let ground_h = sampler.height(x, z);
    let spawn_y = ground_h + ball_radius + 10.0;
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(Sphere { radius: ball_radius })),
            material: mats.add(Color::srgb(0.95, 0.95, 0.95)),
            transform: Transform::from_xyz(x, spawn_y, z),
            ..default()
        })
        .insert(Ball)
        .insert(BallKinematic { radius: ball_radius, vel: Vec3::ZERO });

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

fn simple_ball_physics(
    mut q: Query<(&mut Transform, &mut BallKinematic), With<Ball>>,
    sampler: Res<TerrainSampler>,
) {
    let Ok((mut t, mut kin)) = q.get_single_mut() else { return; };
    let dt = 1.0 / 60.0;
    let g = -9.81;

    // Apply gravity
    kin.vel.y += g * dt;

    // Predict position
    t.translation += kin.vel * dt;

    // Sample terrain height & normal under new position
    let h = sampler.height(t.translation.x, t.translation.z);
    let surface_y = h + kin.radius;

    if t.translation.y <= surface_y {
        // We are contacting / below surface: project onto surface
        t.translation.y = surface_y;

        // Terrain normal (for slope)
        let n = sampler.normal(t.translation.x, t.translation.z);

        // Remove any inward (into ground) velocity component
        let vn = kin.vel.dot(n);
        if vn < 0.0 {
            kin.vel -= vn * n;
        }

        // Compute tangential component for sliding (vel already had normal removed if downward)
        let tangent_vel = kin.vel - n * kin.vel.dot(n);

        // Add gravity component along the plane (simulate rolling/sliding). Gravity vector is (0,g,0).
        // Component of gravity along plane: g_parallel = g_vec - n*(g_vec·n)
        let g_vec = Vec3::Y * g;
        let g_parallel = g_vec - n * g_vec.dot(n);
        kin.vel += g_parallel * dt;

        // Simple rolling friction proportional to normal force (|g|) and opposite tangential direction.
        let mut tangential = kin.vel - n * kin.vel.dot(n);
        let speed = tangential.length();
        if speed > 1e-5 {
            let friction_coeff = 0.25; // tweak
            let decel = friction_coeff * -g; // positive value
            let drop = decel * dt;
            if drop >= speed {
                // Stop
                kin.vel -= tangential;
                tangential = Vec3::ZERO;
            } else {
                let new_speed = speed - drop;
                kin.vel += (tangential.normalize() * (new_speed - speed));
                tangential = kin.vel - n * kin.vel.dot(n);
            }
        }

        // Visual rolling: rotate sphere based on tangential displacement
        let disp = tangential * dt;
        let disp_len = disp.length();
        if disp_len > 1e-6 {
            let axis = disp.cross(n).normalize_or_zero(); // rotation axis perpendicular to motion & normal
            if axis.length_squared() > 0.0 {
                let angle = disp_len / kin.radius;
                t.rotate_local(Quat::from_axis_angle(axis, angle));
            }
        }
    }
}

fn setup_ui(mut commands: Commands, assets: Res<AssetServer>) {
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
