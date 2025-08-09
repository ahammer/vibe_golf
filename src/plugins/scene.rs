use bevy::prelude::*;
use bevy::math::primitives::{Cuboid, Sphere};
use bevy_rapier3d::prelude::*;
use crate::plugins::terrain::TerrainSampler;
use crate::plugins::camera::OrbitCamera;
use bevy::input::mouse::MouseButton;

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

#[derive(Resource, Default, Debug)]
pub struct Score {
    pub hits: u32,
}

pub struct ScenePlugin;
impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(ShotState::default())
            .insert_resource(ShotConfig::default())
            .insert_resource(Score::default())
            .add_systems(Startup, (setup_scene, setup_ui))
            .add_systems(FixedUpdate, (simple_ball_physics, update_shot_charge, detect_target_hits))
            .add_systems(Update, (handle_shot_input, update_shot_indicator, update_power_gauge));
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    sampler: Res<TerrainSampler>,
) {
    // camera (orbit)
    let cam_start = Transform::from_xyz(-12.0, 10.0, 18.0)
        .looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y);
    commands.spawn((
        Camera3dBundle {
            transform: cam_start,
            ..default()
        },
        OrbitCamera,
    ));

    // light with shadows (using default cascades)
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 40_000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(30.0, 60.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
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

    // shot indicator (hidden until charging); local +Z points along direction
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(Cuboid::from_size(Vec3::new(0.12, 0.12, 1.0)))),
            material: mats.add(Color::srgb(1.0, 0.85, 0.1)),
            transform: Transform::from_xyz(x, ground_h + 0.25, z),
            visibility: Visibility::Hidden,
            ..default()
        })
        .insert(ShotIndicator);

    // distant tall target pillar (easier to see from spawn)
    let target_x = 0.0;
    let target_z = 80.0;
    let pillar_height = 16.0; // doubled height for higher visibility
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

        // Compute tangential component for sliding
        let g_vec = Vec3::Y * g;
        let g_parallel = g_vec - n * g_vec.dot(n);
        kin.vel += g_parallel * dt;

        // Friction
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

        // Visual rolling
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
    // Left HUD
    commands
        .spawn(
            TextBundle::from_section(
                "Tick: 0 | Speed: 0.00 m/s",
                TextStyle { font: font.clone(), font_size: 22.0, color: Color::WHITE },
            )
            .with_style(Style { position_type: PositionType::Absolute, left: Val::Px(12.0), top: Val::Px(8.0), ..default() }),
        )
        .insert(Hud);

    // Power gauge (top-right)
    commands
        .spawn(
            TextBundle::from_section(
                "Power: --",
                TextStyle { font, font_size: 22.0, color: Color::WHITE },
            )
            .with_style(Style {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Px(8.0),
                ..default()
            }),
        )
        .insert(PowerGauge);
}

// -------- Shooting Systems --------

fn handle_shot_input(
    buttons: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<ShotState>,
    cfg: Res<ShotConfig>,
    mut q_ball: Query<(&Transform, &mut BallKinematic), (With<Ball>, Without<ShotIndicator>)>,
    q_cam: Query<&Transform, (With<OrbitCamera>, Without<Ball>, Without<ShotIndicator>)>,
    mut q_indicator: Query<(&mut Transform, &mut Visibility), (With<ShotIndicator>, Without<Ball>, Without<OrbitCamera>)>,
) {
    let Ok((ball_t, mut kin)) = q_ball.get_single_mut() else { return; };
    let Ok(cam_t) = q_cam.get_single() else { return; };
    let Ok((mut ind_t, mut vis)) = q_indicator.get_single_mut() else { return; };

    // Start charging
    if buttons.just_pressed(MouseButton::Left) && state.mode == ShotMode::Idle {
        state.mode = ShotMode::Charging;
        state.power = 0.0;
        state.rising = true;
        // Position indicator at ball
        ind_t.translation = ball_t.translation + Vec3::Y * (kin.radius * 0.5);
        *vis = Visibility::Visible;
    }

    // Release => fire
    if buttons.just_released(MouseButton::Left) && state.mode == ShotMode::Charging {
        // Direction: camera->ball vector elevated by fixed angle
        let cam_to_ball = (ball_t.translation - cam_t.translation).normalize_or_zero();
        let horiz = Vec3::new(cam_to_ball.x, 0.0, cam_to_ball.z).normalize_or_zero();
        let angle = cfg.up_angle_deg.to_radians();
        let dir = (horiz * angle.cos() + Vec3::Y * angle.sin()).normalize_or_zero();

        let impulse = cfg.base_impulse * state.power.max(0.05);
        kin.vel += dir * impulse;

        // Reset
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

fn update_shot_indicator(
    state: Res<ShotState>,
    cfg: Res<ShotConfig>,
    q_ball: Query<&Transform, (With<Ball>, Without<ShotIndicator>)>,
    q_cam: Query<&Transform, (With<OrbitCamera>, Without<Ball>, Without<ShotIndicator>)>,
    mut q_ind: Query<&mut Transform, (With<ShotIndicator>, Without<Ball>, Without<OrbitCamera>)>,
    mut q_ind_vis: Query<&mut Visibility, (With<ShotIndicator>, Without<Ball>, Without<OrbitCamera>)>,
) {
    if state.mode != ShotMode::Charging {
        return;
    }
    let Ok(ball_t) = q_ball.get_single() else { return; };
    let Ok(cam_t) = q_cam.get_single() else { return; };
    let Ok(mut ind_t) = q_ind.get_single_mut() else { return; };
    let Ok(mut vis) = q_ind_vis.get_single_mut() else { return; };
    *vis = Visibility::Visible;

    let cam_to_ball = (ball_t.translation - cam_t.translation).normalize_or_zero();
    let horiz = Vec3::new(cam_to_ball.x, 0.0, cam_to_ball.z).normalize_or_zero();
    let angle = cfg.up_angle_deg.to_radians();
    let dir = (horiz * angle.cos() + Vec3::Y * angle.sin()).normalize_or_zero();

    // Length scale
    let len = cfg.indicator_base_len + cfg.indicator_var_len * state.power;
    // Position near ball surface
    let base_pos = ball_t.translation + Vec3::Y * 0.1;
    ind_t.translation = base_pos + dir * (len * 0.5);
    // Scale: keep x,y thin, z set to len
    ind_t.scale = Vec3::new(0.12, 0.12, len);

    // Orient: from +Z to dir
    let from = Vec3::Z;
    if dir.length_squared() > 0.0 {
        ind_t.rotation = Quat::from_rotation_arc(from, dir);
    }
}

fn detect_target_hits(
    mut score: ResMut<Score>,
    sampler: Res<TerrainSampler>,
    mut q_target: Query<&mut Transform, (With<Target>, Without<Ball>)>,
    q_ball: Query<(&Transform, &BallKinematic), With<Ball>>,
) {
    let Ok((ball_t, kin)) = q_ball.get_single() else { return; };
    let Ok(mut target_t) = q_target.get_single_mut() else { return; };

    // Pillar dimensions: 1.0 x pillar_height x 1.0; half extents 0.5,0.5 horizontally
    let half = 0.5 + kin.radius;
    let dx = (ball_t.translation.x - target_t.translation.x).abs();
    let dz = (ball_t.translation.z - target_t.translation.z).abs();
    if dx <= half && dz <= half {
        // Hit
        score.hits += 1;

        // Reposition pillar pseudo-randomly within chunk using angular increment
        // Approx chunk half-size (matches TerrainConfig::default chunk_size 384.0)
        let chunk_half = 384.0 * 0.5 - 5.0;
        let angle_deg = (score.hits as f32 * 137.0) % 360.0;
        let angle = angle_deg.to_radians();
        let ring = 60.0 + (score.hits % 5) as f32 * 15.0;
        let mut new_x = ring * angle.cos();
        let mut new_z = ring * angle.sin();
        new_x = new_x.clamp(-chunk_half, chunk_half);
        new_z = new_z.clamp(-chunk_half, chunk_half);
        let pillar_half_height = target_t.scale.y * 0.5; // original scaling 1.0 in Y (no scale change), fallback
        let ground = sampler.height(new_x, new_z);
        target_t.translation = Vec3::new(new_x, ground + pillar_half_height, new_z);
    }
}
