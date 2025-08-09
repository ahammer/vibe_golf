use bevy::prelude::*;
use bevy::math::primitives::{Cuboid, Sphere};
use bevy_rapier3d::prelude::*;
use crate::plugins::terrain::TerrainSampler;

#[derive(Component)]
pub struct Ball;
#[derive(Component)]
pub struct Hud;
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
        directional_light: DirectionalLight { illuminance: 25_000.0, shadows_enabled: true, ..default() },
        transform: Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // ball
    let ball_radius = 0.25;
    let base_h = sampler.height(-5.0, -5.0);
    let spawn_y = base_h + ball_radius + 0.25;
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(Sphere { radius: ball_radius })),
            material: mats.add(Color::srgb(0.95, 0.95, 0.95)),
            transform: Transform::from_xyz(-5.0, spawn_y, -5.0),
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
