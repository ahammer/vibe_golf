use bevy::prelude::*;
use bevy::sprite::{ColorMaterial, MaterialMesh2dBundle};
use bevy::render::mesh::Mesh;
use bevy::render::render_asset::RenderAssetUsages;

use crate::plugins::core_sim::SimState;
use crate::plugins::ball::{BallKinematic, Ball};
use crate::plugins::game_state::Score;
use crate::plugins::target::Target;
use crate::plugins::camera::OrbitCameraState;
use bevy::window::PrimaryWindow;

#[derive(Component)]
pub struct Hud;

// ---------------- Compass (graphics) ----------------
#[derive(Component)]
pub struct CompassRoot;
#[derive(Component)]
pub struct CompassTargetMarker;
#[derive(Component)]
pub struct CompassDistanceText;

pub struct HudPlugin;
impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_hud_text, spawn_compass_graphics))
            .add_systems(Update, (update_hud, update_compass_graphics));
    }
}

fn spawn_hud_text(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("fonts/FiraSans-Bold.ttf");
    commands.spawn((
        TextBundle::from_section(
            "Initializing...",
            TextStyle { font: font.clone(), font_size: 22.0, color: Color::WHITE },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(8.0),
            ..default()
        }),
        Hud,
    ));
}

// Build a simple filled circle (triangle fan)
fn build_circle_mesh(radius: f32, segments: usize) -> Mesh {
    use bevy::render::mesh::{Indices, PrimitiveTopology};
    let segs = segments.max(3);
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(segs + 2);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(segs + 2);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(segs + 2);
    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, 0.0, 1.0]);
    uvs.push([0.5, 0.5]);
    for i in 0..=segs {
        let a = (i as f32 / segs as f32) * std::f32::consts::TAU;
        let x = radius * a.cos();
        let y = radius * a.sin();
        positions.push([x, y, 0.0]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([(x / radius + 1.0) * 0.5, (y / radius + 1.0) * 0.5]);
    }
    let mut indices: Vec<u32> = Vec::with_capacity(segs * 3);
    for i in 1..=segs {
        indices.push(0);
        indices.push(i as u32);
        indices.push((i + 1) as u32);
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}


// Spawn compass graphics (2D overlay camera + circle & markers)
fn spawn_compass_graphics(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    q_cam2d: Query<Entity, With<Camera2d>>,
    q_win: Query<&Window, With<PrimaryWindow>>,
    assets: Res<AssetServer>,
) {
    // 2D camera overlay (only if none)
    if q_cam2d.iter().next().is_none() {
        commands.spawn((
            Camera2dBundle {
                camera: Camera {
                    order: 10, // render on top of 3D
                    ..default()
                },
                ..default()
            },
        ));
    }

    let Ok(win) = q_win.get_single() else { return; };

    let radius = 70.0;
    let margin = 90.0;
    // Screen space (0,0) at center for 2D camera; place compass top-left
    let screen_x = -win.width() * 0.5 + margin;
    let screen_y = win.height() * 0.5 - margin;

    let circle_mesh = meshes.add(build_circle_mesh(radius, 64));
    let circle_mat = materials.add(Color::srgba(1.0, 1.0, 1.0, 0.07));
    // removed forward line (not needed)
    let target_mesh = meshes.add(build_circle_mesh(6.0, 24));
    let target_mat = materials.add(Color::srgb(0.95, 0.2, 0.2));

    let root = commands
        .spawn((
            SpatialBundle {
                transform: Transform::from_translation(Vec3::new(screen_x, screen_y, 0.0)),
                ..default()
            },
            CompassRoot,
        ))
        .id();

    // Circle
    commands.entity(root).with_children(|p| {
        p.spawn((
            MaterialMesh2dBundle {
                mesh: circle_mesh.clone().into(),
                material: circle_mat.clone(),
                transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
                ..default()
            },
        ));
        // Target marker (will be positioned each frame)
        p.spawn((
            MaterialMesh2dBundle {
                mesh: target_mesh.clone().into(),
                material: target_mat.clone(),
                transform: Transform::from_translation(Vec3::new(0.0, 0.0, 1.0)),
                ..default()
            },
            CompassTargetMarker,
        ));
        // Distance text (2D)
        p.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Dist: --.-m",
                    TextStyle {
                        font: assets.load("fonts/FiraSans-Bold.ttf"),
                        font_size: 16.0,
                        color: Color::WHITE,
                    },
                ),
                transform: Transform::from_translation(Vec3::new(0.0, -radius - 18.0, 1.5)),
                ..default()
            },
            CompassDistanceText,
        ));
    });
}

fn update_hud(
    sim: Res<SimState>,
    score: Res<Score>,
    q_ball: Query<&BallKinematic>,
    mut q_text: Query<&mut Text, With<Hud>>,
) {
    if let (Ok(kin), Ok(mut text)) = (q_ball.get_single(), q_text.get_single_mut()) {
        let speed = kin.vel.length();
        if score.game_over {
            let avg_time = score.final_time / score.hits.max(1) as f32;
            let avg_shots = score.shots as f32 / score.hits.max(1) as f32;
            let best = score.high_score_time.map(|v| format!("{:.2}s", v)).unwrap_or_else(|| "--".to_string());
            text.sections[0].value = format!(
                "GAME OVER | Time: {:.2}s | Best: {best} | Holes: {} | Shots: {} | Avg T/H: {:.2}s | Avg S/H: {:.2} | Press R",
                score.final_time,
                score.hits,
                score.shots,
                avg_time,
                avg_shots,
            );
        } else {
            let current_hole = score.hits + 1;
            let avg_time = if score.hits > 0 { sim.elapsed_seconds / score.hits as f32 } else { 0.0 };
            let avg_shots = if score.hits > 0 { score.shots as f32 / score.hits as f32 } else { 0.0 };
            text.sections[0].value = format!(
                "Time: {:.2}s | Speed: {:.2} m/s | Hole: {}/{} | Shots: {} | Avg T/H: {:.2}s | Avg S/H: {:.2}",
                sim.elapsed_seconds,
                speed,
                current_hole,
                score.max_holes,
                score.shots,
                avg_time,
                avg_shots,
            );
        }
    }
}

fn update_compass_graphics(
    score: Res<Score>,
    state: Option<Res<OrbitCameraState>>,
    q_ball_t: Query<&Transform, With<Ball>>,
    q_target_t: Query<&Transform, (With<Target>, Without<Ball>, Without<CompassTargetMarker>)>,
    mut q_marker: Query<&mut Transform, (With<CompassTargetMarker>, Without<Target>, Without<Ball>)>,
    mut q_dist_text: Query<&mut Text, With<CompassDistanceText>>,
) {
    if score.game_over {
        return;
    }
    let (Some(state), Ok(ball_t), Ok(target_t)) =
        (state, q_ball_t.get_single(), q_target_t.get_single()) else { return; };

    let Ok(mut marker_t) = q_marker.get_single_mut() else { return; };
    let Ok(mut dist_text) = q_dist_text.get_single_mut() else { return; };

    let to_target = target_t.translation - ball_t.translation;
    let horiz = Vec3::new(to_target.x, 0.0, to_target.z);
    let dist = horiz.length();
    if dist < 0.001 {
        dist_text.sections[0].value = "Dist: 0.0m".to_string();
        marker_t.translation = Vec3::new(0.0, 0.0, marker_t.translation.z);
        return;
    }
    let dir = horiz / dist;

    // Camera forward vector (see orbit implementation: position offset uses yaw around Y then pitch; yaw rotation means offset Z component is cos(yaw) along +Z, but forward is -sin(yaw), -cos(yaw) in (x,z))
    let forward = Vec3::new(-state.yaw.sin(), 0.0, -state.yaw.cos()).normalize();

    // Signed relative angle (forward = 0, right = +PI/2)
    let dot = forward.dot(dir).clamp(-1.0, 1.0);
    let cross_y = forward.x * dir.z - forward.z * dir.x;
    let rel_angle = cross_y.atan2(dot); // (-PI, PI]

    // Radius derived from circle child (root has children at local (0,0))
    // We stored forward marker line length ~0.9R; just reuse distance between root and forward marker tip:
    let radius = 70.0;

    // Map angle to circle coordinates: 0 at top, positive clockwise
    // Using x = sin(angle), y = cos(angle) as earlier reasoning
    let x = rel_angle.sin() * radius;
    let y = rel_angle.cos() * radius;
    marker_t.translation = Vec3::new(x, y, marker_t.translation.z);

    dist_text.sections[0].value = format!("Dist: {:.1}m", dist);

}
