use bevy::prelude::*;

/// Simple debug scene to verify 3D rendering pipeline still active.
/// Spawns its own camera/light + a test mesh and periodically logs entity counts.
pub struct DebugScenePlugin;

#[derive(Resource)]
struct DebugSceneTimer(Timer);

impl Plugin for DebugScenePlugin {
    fn build(&self, app: &mut App) {
        // Only insert if RENDER_DEBUG env var is set (opt‑in) so normal runs unaffected.
        if std::env::var("RENDER_DEBUG").is_ok() {
            app.insert_resource(DebugSceneTimer(Timer::from_seconds(2.0, TimerMode::Repeating)))
                .add_systems(Startup, spawn_debug_scene)
                .add_systems(Update, log_debug_scene_counts);
        }
    }
}

fn spawn_debug_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    info!("DebugScene: spawning test camera/light/mesh (set RENDER_DEBUG unset to disable).");

    // Independent camera so we can see something even if level camera failed.
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 8.0, 18.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // Light
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 30_000.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(20.0, 30.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // Test ground quad
    let mut ground = Mesh::from(shape::Plane { size: 40.0 });
    // Slightly raise to avoid z-fighting if terrain appears later
    commands.spawn(PbrBundle {
        mesh: meshes.add(ground),
        material: materials.add(Color::srgb(0.2, 0.7, 0.4)),
        transform: Transform::from_xyz(0.0, 0.01, 0.0),
        ..default()
    });

    // Test sphere
    let sphere_mesh = Mesh::from(shape::Icosphere {
        radius: 2.0,
        subdivisions: 4,
    });
    commands.spawn(PbrBundle {
        mesh: meshes.add(sphere_mesh),
        material: materials.add(Color::srgb(0.9, 0.2, 0.2)),
        transform: Transform::from_xyz(0.0, 2.0, 0.0),
        ..default()
    });
}

fn log_debug_scene_counts(
    time: Res<Time>,
    mut timer: ResMut<DebugSceneTimer>,
    q_pbr: Query<Entity, (With<Handle<Mesh>>, With<Handle<StandardMaterial>>)>,
    q_cams: Query<Entity, With<Camera3d>>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        info!(
            "DebugScene: pbr_entities={} cameras={}",
            q_pbr.iter().count(),
            q_cams.iter().count()
        );
    }
}
