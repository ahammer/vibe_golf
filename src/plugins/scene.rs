use bevy::prelude::*;
use bevy::math::primitives::{Cuboid, Sphere};
use bevy_rapier3d::prelude::*;
use crate::plugins::terrain::TerrainSampler;
use crate::plugins::camera::OrbitCamera;
use crate::plugins::core_sim::SimState;
use bevy::input::mouse::MouseButton;
use bevy::render::camera::ClearColorConfig;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use std::fs;
use crate::plugins::particles::{BallGroundImpactEvent, TargetHitEvent, GameOverEvent, ShotFiredEvent};
use std::io::Write;
use std::path::Path;

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
pub struct Target;

#[derive(Component)]
pub struct PowerGauge;

#[derive(Component)]
pub struct ShotIndicator;

#[derive(Component)]
pub struct PowerBar;        // UI container for power bar
#[derive(Component)]
pub struct PowerBarFill;    // UI fill element whose width/color show power

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotMode {
    Idle,
    Charging,
}

#[derive(Resource, Debug)]
pub struct ShotState {
    pub mode: ShotMode,
    pub power: f32,   // 0..1
    pub rising: bool, // triangle wave direction
}

impl Default for ShotState {
    fn default() -> Self {
        Self { mode: ShotMode::Idle, power: 0.0, rising: true }
    }
}

#[derive(Resource, Debug)]
pub struct ShotConfig {
    pub osc_speed: f32,    // units per second (triangle wave edge speed)
    pub base_impulse: f32, // velocity applied at power=1
    pub up_angle_deg: f32, // fixed elevation angle
    pub indicator_base_len: f32,
    pub indicator_var_len: f32,
}

impl Default for ShotConfig {
    fn default() -> Self {
        Self {
            osc_speed: 1.6,
            base_impulse: 18.0,
            up_angle_deg: 45.0,
            indicator_base_len: 0.7,
            indicator_var_len: 1.6,
        }
    }
}

#[derive(Resource, Debug)]
pub struct Score {
    pub hits: u32,
    pub shots: u32,
    pub max_holes: u32,
    pub game_over: bool,
    pub final_time: f32,
    pub high_score_time: Option<f32>, // lowest completion time
}
impl Default for Score {
    fn default() -> Self {
        Self {
            hits: 0,
            shots: 0,
            max_holes: 5,
            game_over: false,
            final_time: 0.0,
            high_score_time: load_high_score_time(),
        }
    }
}

fn high_score_file_path() -> &'static str { "high_score_time.txt" }

fn load_high_score_time() -> Option<f32> {
    let path = Path::new(high_score_file_path());
    if let Ok(data) = fs::read_to_string(path) {
        if let Ok(v) = data.trim().parse::<f32>() {
            return Some(v);
        }
    }
    None
}

fn save_high_score_time(t: f32) {
    if let Ok(mut f) = fs::File::create(high_score_file_path()) {
        let _ = writeln!(f, "{t}");
    }
}

pub struct ScenePlugin;

// Generate an inside-facing (inverted) UV sphere suitable for equirectangular sky textures.
fn generate_inverted_sphere(longitudes: u32, latitudes: u32, radius: f32) -> Mesh {
    let longs = longitudes.max(3);
    let lats = latitudes.max(2);
    let mut positions = Vec::with_capacity(((longs + 1) * (lats + 1)) as usize);
    let mut uvs = Vec::with_capacity(positions.capacity());
    let mut normals = Vec::with_capacity(positions.capacity());
    for y in 0..=lats {
        let v = y as f32 / lats as f32;
        let theta = (v - 0.5) * std::f32::consts::PI;
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();
        for x in 0..=longs {
            let u = x as f32 / longs as f32;
            let phi = (u - 0.5) * std::f32::consts::TAU;
            let cos_phi = phi.cos();
            let sin_phi = phi.sin();
            let px = cos_theta * cos_phi;
            let py = sin_theta;
            let pz = cos_theta * sin_phi;
            positions.push([radius * px, radius * py, radius * pz]);
            normals.push([-px, -py, -pz]);
            uvs.push([u, 1.0 - v]);
        }
    }
    let mut indices: Vec<u32> = Vec::with_capacity((longs * lats * 6) as usize);
    let row_stride = longs + 1;
    for y in 0..lats {
        for x in 0..longs {
            let i0 = y * row_stride + x;
            let i1 = i0 + 1;
            let i2 = i0 + row_stride;
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(ShotState::default())
            .insert_resource(ShotConfig::default())
            .insert_resource(Score::default())
            .add_systems(Startup, (setup_scene, setup_ui))
            .add_systems(FixedUpdate, (simple_ball_physics, update_shot_charge, detect_target_hits))
            .add_systems(Update, (
                handle_shot_input,
                update_shot_indicator,
                update_power_gauge,
                update_power_bar,
                reset_game
            ));
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
    sampler: Res<TerrainSampler>,
) {
    let cam_start = Transform::from_xyz(-12.0, 10.0, 18.0)
        .looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y);
    commands.spawn((
        Camera3dBundle {
            transform: cam_start,
            camera: Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
            projection: PerspectiveProjection {
                fov: 80f32.to_radians(),
                ..default()
            }.into(),
            ..default()
        },
        OrbitCamera,
    ));

    let sky_tex = assets.load("skymap/kloppenheim_06_puresky_1k.hdr");
    let sky_mesh = generate_inverted_sphere(64, 32, 500.0);
    commands.spawn(PbrBundle {
        mesh: meshes.add(sky_mesh),
        material: mats.add(StandardMaterial {
            base_color_texture: Some(sky_tex),
            unlit: true,
            ..default()
        }),
        transform: Transform::IDENTITY,
        ..default()
    });

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 40_000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(30.0, 60.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    let ball_radius = 0.25;
    let x = 0.0;
    let z = 0.0;
    let ground_h = sampler.height(x, z);
    let spawn_y = ground_h + ball_radius + 10.0;
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(Sphere { radius: ball_radius })),
            material: mats.add(StandardMaterial {
                base_color: Color::srgb(0.92, 0.93, 0.95),
                emissive: LinearRgba::new(0.25, 0.35, 0.60, 1.0) * 0.4,
                perceptual_roughness: 0.35,
                metallic: 0.0,
                ..default()
            }),
            transform: Transform::from_xyz(x, spawn_y, z),
            ..default()
        })
        .insert(Ball)
        .insert(BallKinematic { radius: ball_radius, vel: Vec3::ZERO });

    // Shot indicator (emissive beam)
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::from_size(Vec3::new(0.12, 0.12, 1.0)))),
            material: mats.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.85, 0.10),
                emissive: LinearRgba::new(3.0, 2.0, 0.3, 1.0) * 0.5,
                perceptual_roughness: 0.5,
                metallic: 0.0,
                ..default()
            }),
            transform: Transform::from_xyz(x, ground_h + 0.25, z),
            visibility: Visibility::Hidden,
            ..default()
        })
        .insert(ShotIndicator);

    // Target pillar
    let target_x = 0.0;
    let target_z = 80.0;
    let pillar_height = 16.0;
    let target_ground = sampler.height(target_x, target_z);
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::from_size(Vec3::new(1.0, pillar_height, 1.0)))),
            material: mats.add(Color::srgb(0.9, 0.2, 0.2)),
            transform: Transform::from_xyz(
                target_x,
                target_ground + pillar_height * 0.5,
                target_z,
            ),
            ..default()
        })
        .insert(Target)
        .insert(RigidBody::Fixed)
        .insert(Collider::cuboid(0.5, pillar_height * 0.5, 0.5));
}

fn simple_ball_physics(
    mut q: Query<(&mut Transform, &mut BallKinematic), With<Ball>>,
    sampler: Res<TerrainSampler>,
    mut ev_impact: EventWriter<BallGroundImpactEvent>,
) {
    let Ok((mut t, mut kin)) = q.get_single_mut() else { return; };
    let dt = 1.0 / 60.0;
    let g = -9.81;

    kin.vel.y += g * dt;
    t.translation += kin.vel * dt;

    let h = sampler.height(t.translation.x, t.translation.z);
    let surface_y = h + kin.radius;

    if t.translation.y <= surface_y {
        t.translation.y = surface_y;

        let n = sampler.normal(t.translation.x, t.translation.z);

        let vn = kin.vel.dot(n);
        if vn < 0.0 {
            let impact_intensity = (-vn).max(0.0);
            if impact_intensity > 0.1 {
                ev_impact.send(BallGroundImpactEvent {
                    pos: t.translation,
                    intensity: impact_intensity,
                });
            }
            kin.vel -= vn * n;
        }

        let g_vec = Vec3::Y * g;
        let g_parallel = g_vec - n * g_vec.dot(n);
        kin.vel += g_parallel * dt;

        let mut tangential = kin.vel - n * kin.vel.dot(n);
        let speed = tangential.length();
        if speed > 1e-5 {
            let friction_coeff = 0.25;
            let decel = friction_coeff * -g;
            let drop = decel * dt;
            if drop >= speed {
                kin.vel -= tangential;
                tangential = Vec3::ZERO;
            } else {
                let new_speed = speed - drop;
                kin.vel += tangential.normalize() * (new_speed - speed);
                tangential = kin.vel - n * kin.vel.dot(n);
            }
        }

        let disp = tangential * dt;
        let disp_len = disp.length();
        if disp_len > 1e-6 {
            let axis = disp.cross(n).normalize_or_zero();
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
                TextStyle { font: font.clone(), font_size: 22.0, color: Color::WHITE },
            )
            .with_style(Style { position_type: PositionType::Absolute, left: Val::Px(12.0), top: Val::Px(8.0), ..default() }),
        )
        .insert(Hud);

    // Power gauge text
    commands
        .spawn(
            TextBundle::from_section(
                "Power: --",
                TextStyle { font: font.clone(), font_size: 22.0, color: Color::WHITE },
            )
            .with_style(Style {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Px(8.0),
                ..default()
            }),
        )
        .insert(PowerGauge);

    // Power bar container + fill
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    right: Val::Px(12.0),
                    top: Val::Px(36.0),
                    width: Val::Px(180.0),
                    height: Val::Px(18.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::FlexStart,
                    padding: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                background_color: Color::srgb(0.08, 0.08, 0.10).into(),
                ..default()
            },
            PowerBar,
        ))
        .with_children(|parent| {
            parent.spawn((
                NodeBundle {
                    style: Style {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    background_color: Color::srgb(0.15, 0.60, 0.25).into(),
                    ..default()
                },
                PowerBarFill,
            ));
        });
}

// -------- Shooting Systems --------

fn handle_shot_input(
    buttons: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<ShotState>,
    cfg: Res<ShotConfig>,
    mut score: ResMut<Score>,
    mut q_ball: Query<(&Transform, &mut BallKinematic), (With<Ball>, Without<ShotIndicator>)>,
    q_cam: Query<&Transform, (With<OrbitCamera>, Without<Ball>, Without<ShotIndicator>)>,
    mut q_indicator: Query<(&mut Transform, &mut Visibility), (With<ShotIndicator>, Without<Ball>, Without<OrbitCamera>)>,
    mut ev_shot: EventWriter<ShotFiredEvent>,
) {
    if score.game_over {
        return;
    }
    let Ok((ball_t, mut kin)) = q_ball.get_single_mut() else { return; };
    let Ok(cam_t) = q_cam.get_single() else { return; };
    let Ok((mut ind_t, mut vis)) = q_indicator.get_single_mut() else { return; };

    if buttons.just_pressed(MouseButton::Left) && state.mode == ShotMode::Idle {
        state.mode = ShotMode::Charging;
        state.power = 0.0;
        state.rising = true;
        ind_t.translation = ball_t.translation + Vec3::Y * (kin.radius * 0.5);
        *vis = Visibility::Visible;
    }

    if buttons.just_released(MouseButton::Left) && state.mode == ShotMode::Charging {
        let cam_to_ball = (ball_t.translation - cam_t.translation).normalize_or_zero();
        let horiz = Vec3::new(cam_to_ball.x, 0.0, cam_to_ball.z).normalize_or_zero();
        let angle = cfg.up_angle_deg.to_radians();
        let dir = (horiz * angle.cos() + Vec3::Y * angle.sin()).normalize_or_zero();

        let impulse = cfg.base_impulse * state.power.max(0.05);
        let shot_power = state.power;
        kin.vel += dir * impulse;
        score.shots += 1;
        ev_shot.send(ShotFiredEvent { pos: ball_t.translation, power: shot_power });

        state.mode = ShotMode::Idle;
        state.power = 0.0;
        *vis = Visibility::Hidden;
    }
}

fn update_shot_charge(
    time: Res<Time>,
    mut state: ResMut<ShotState>,
    cfg: Res<ShotConfig>,
) {
    if state.mode != ShotMode::Charging {
        return;
    }
    let dt = time.delta_seconds();
    let delta = cfg.osc_speed * dt;

    if state.rising {
        state.power += delta;
        if state.power >= 1.0 {
            state.power = 1.0;
            state.rising = false;
        }
    } else {
        state.power -= delta;
        if state.power <= 0.0 {
            state.power = 0.0;
            state.rising = true;
        }
    }
}

fn update_power_gauge(
    state: Res<ShotState>,
    mut q: Query<&mut Text, With<PowerGauge>>,
) {
    if !state.is_changed() {
        return;
    }
    if let Ok(mut text) = q.get_single_mut() {
        match state.mode {
            ShotMode::Idle => {
                text.sections[0].value = "Power: --".to_string();
            }
            ShotMode::Charging => {
                text.sections[0].value = format!("Power: {:>3}%", (state.power * 100.0) as u32);
            }
        }
    }
}

fn update_power_bar(
    state: Res<ShotState>,
    mut q_fill: Query<(&mut Style, &mut BackgroundColor), With<PowerBarFill>>,
) {
    if !state.is_changed() { return; }
    let power = match state.mode {
        ShotMode::Idle => 0.0,
        ShotMode::Charging => state.power,
    };
    if let Ok((mut style, mut color)) = q_fill.get_single_mut() {
        style.width = Val::Percent(power * 100.0);
        // Gradient green -> yellow -> red
        let col = if power < 0.5 {
            let t = power / 0.5;
            Color::srgb(
                0.15 + (0.70 - 0.15) * t,
                0.60 + (0.85 - 0.60) * t,
                0.25 + (0.10 - 0.25) * t,
            )
        } else {
            let t = (power - 0.5) / 0.5;
            Color::srgb(
                0.70 + (0.90 - 0.70) * t,
                0.85 + (0.20 - 0.85) * t,
                0.10 + (0.15 - 0.10) * t,
            )
        };
        *color = col.into();
    }
}

fn update_shot_indicator(
    state: Res<ShotState>,
    cfg: Res<ShotConfig>,
    q_ball: Query<&Transform, (With<Ball>, Without<ShotIndicator>)>,
    q_cam: Query<&Transform, (With<OrbitCamera>, Without<Ball>, Without<ShotIndicator>)>,
    mut q_ind: Query<(&mut Transform, &Handle<StandardMaterial>, &mut Visibility), With<ShotIndicator>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if state.mode != ShotMode::Charging {
        return;
    }
    let Ok(ball_t) = q_ball.get_single() else { return; };
    let Ok(cam_t) = q_cam.get_single() else { return; };
    let Ok((mut ind_t, mat_handle, mut vis)) = q_ind.get_single_mut() else { return; };
    *vis = Visibility::Visible;

    let cam_to_ball = (ball_t.translation - cam_t.translation).normalize_or_zero();
    let horiz = Vec3::new(cam_to_ball.x, 0.0, cam_to_ball.z).normalize_or_zero();
    let angle = cfg.up_angle_deg.to_radians();
    let dir = (horiz * angle.cos() + Vec3::Y * angle.sin()).normalize_or_zero();

    let len = cfg.indicator_base_len + cfg.indicator_var_len * state.power;
    let base_pos = ball_t.translation + Vec3::Y * 0.1;
    ind_t.translation = base_pos + dir * (len * 0.5);
    ind_t.scale = Vec3::new(0.12, 0.12, len);

    let from = Vec3::Z;
    if dir.length_squared() > 0.0 {
        ind_t.rotation = Quat::from_rotation_arc(from, dir);
    }

    if let Some(mat) = materials.get_mut(mat_handle) {
        let intensity = 0.5 + state.power * 2.5;
        mat.emissive = LinearRgba::new(3.0, 2.0, 0.3, 1.0) * intensity;
    }
}

fn detect_target_hits(
    mut score: ResMut<Score>,
    sim: Res<SimState>,
    sampler: Res<TerrainSampler>,
    mut q_target: Query<&mut Transform, (With<Target>, Without<Ball>)>,
    q_ball: Query<(&Transform, &BallKinematic), With<Ball>>,
    mut ev_hit: EventWriter<TargetHitEvent>,
    mut ev_game_over: EventWriter<GameOverEvent>,
) {
    let Ok((ball_t, kin)) = q_ball.get_single() else { return; };
    let Ok(mut target_t) = q_target.get_single_mut() else { return; };

    let half = 0.5 + kin.radius;
    let dx = (ball_t.translation.x - target_t.translation.x).abs();
    let dz = (ball_t.translation.z - target_t.translation.z).abs();
    if dx <= half && dz <= half {
        score.hits += 1;
        ev_hit.send(TargetHitEvent { pos: target_t.translation });
        if score.hits >= score.max_holes {
            score.game_over = true;
            score.final_time = sim.elapsed_seconds;
            ev_game_over.send(GameOverEvent { pos: ball_t.translation });
            let better = match score.high_score_time {
                Some(best) => score.final_time < best,
                None => true,
            };
            if better {
                score.high_score_time = Some(score.final_time);
                save_high_score_time(score.final_time);
            }
            return;
        }

        let chunk_half = 384.0 * 0.5 - 5.0;
        let angle_deg = (score.hits as f32 * 137.0) % 360.0;
        let angle = angle_deg.to_radians();
        let ring = 60.0 + (score.hits % 5) as f32 * 15.0;
        let mut new_x = ring * angle.cos();
        let mut new_z = ring * angle.sin();
        new_x = new_x.clamp(-chunk_half, chunk_half);
        new_z = new_z.clamp(-chunk_half, chunk_half);
        let pillar_half_height = target_t.scale.y * 0.5;
        let ground = sampler.height(new_x, new_z);
        target_t.translation = Vec3::new(new_x, ground + pillar_half_height, new_z);
    }
}

fn reset_game(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimState>,
    mut score: ResMut<Score>,
    mut q_ball: Query<(&mut Transform, &mut BallKinematic), With<Ball>>,
    mut q_target: Query<&mut Transform, (With<Target>, Without<Ball>)>,
    sampler: Res<TerrainSampler>,
) {
    if !(score.game_over && keys.just_pressed(KeyCode::KeyR)) {
        return;
    }
    sim.tick = 0;
    sim.elapsed_seconds = 0.0;

    score.hits = 0;
    score.shots = 0;
    score.game_over = false;
    score.final_time = 0.0;

    if let Ok((mut t, mut kin)) = q_ball.get_single_mut() {
        let x = 0.0;
        let z = 0.0;
        let ground_h = sampler.height(x, z);
        let spawn_y = ground_h + kin.radius + 10.0;
        t.translation = Vec3::new(x, spawn_y, z);
        t.rotation = Quat::IDENTITY;
        kin.vel = Vec3::ZERO;
    }

    if let Ok(mut tt) = q_target.get_single_mut() {
        let target_x = 0.0;
        let target_z = 80.0;
        let pillar_height = 16.0;
        let ground = sampler.height(target_x, target_z);
        tt.translation = Vec3::new(target_x, ground + pillar_height * 0.5, target_z);
    }
}
