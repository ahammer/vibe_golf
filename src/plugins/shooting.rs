// Shooting & trajectory visualization plugin.
// Responsible for: input handling, spawning & updating trajectory dots,
// power gauge + bar UI, and applying launch impulse to the ball.
//
// Depends on:
//  - ShotState + ShotConfig (game_state)
//  - Ball + BallKinematic (ball)
//  - OrbitCamera (camera)
//  - Events (ShotFiredEvent) from particles
//
// UI components here are limited to shooting-specific elements (power gauge & bar).
// The main HUD text (score/time) lives in hud.rs.

use bevy::prelude::*;
use bevy::input::touch::TouchInput;
use crate::plugins::ball::{Ball, BallKinematic};
use crate::plugins::camera::OrbitCamera;
use crate::plugins::game_state::{ShotState, ShotConfig, ShotMode};
use crate::plugins::game_state::ShotMode::*;
use crate::plugins::particles::ShotFiredEvent;

/// Trajectory visualization parameters
const TRAJ_DOT_COUNT: usize = 20;
const TRAJ_DOT_DT: f32 = 0.2;

#[derive(Component)]
pub struct ShotIndicator;
#[derive(Component)]
pub struct ShotIndicatorDot {
    pub index: usize,
}

#[derive(Component)]
pub struct PowerGauge;

#[derive(Component)]
pub struct PowerBar;
#[derive(Component)]
pub struct PowerBarFill;

pub struct ShootingPlugin;
impl Plugin for ShootingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_shot_indicators, spawn_power_ui))
            .add_systems(Update, (
                handle_shot_input,
                update_shot_indicator,
                update_power_gauge,
                update_power_bar,
            ));
    }
}

// ---------------- Spawning ----------------

fn spawn_shot_indicators(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // Spawn hidden dots at origin (they relocate when charging).
    for i in 0..TRAJ_DOT_COUNT {
        let tint = 0.3 + (i as f32 / TRAJ_DOT_COUNT as f32) * 0.7;
        commands
            .spawn(PbrBundle {
                mesh: meshes.add(Mesh::from(bevy::math::primitives::Sphere { radius: 0.18 })),
                material: mats.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.85 * tint, 0.10 * tint),
                    emissive: LinearRgba::new(3.0, 2.0, 0.3, 1.0) * 0.2,
                    unlit: false,
                    ..default()
                }),
                transform: Transform::from_xyz(0.0, 0.0, 0.0),
                visibility: Visibility::Hidden,
                ..default()
            })
            .insert(ShotIndicator)
            .insert(ShotIndicatorDot { index: i });
    }
}

fn spawn_power_ui(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("fonts/FiraSans-Bold.ttf");

    // Power gauge text
    commands
        .spawn((
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
            PowerGauge,
        ));

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

// ---------------- Systems ----------------

fn handle_shot_input(
    buttons: Res<ButtonInput<MouseButton>>,
    mut state: ResMut<ShotState>,
    cfg: Res<ShotConfig>,
    mut q_ball: Query<(&mut Transform, &mut BallKinematic), (With<Ball>, Without<ShotIndicator>)>,
    q_cam: Query<&Transform, (With<OrbitCamera>, Without<Ball>, Without<ShotIndicator>)>,
    mut q_indicators: Query<(&mut Transform, &mut Visibility, &ShotIndicatorDot), (With<ShotIndicator>, Without<Ball>, Without<OrbitCamera>)>,
    mut ev_shot: EventWriter<ShotFiredEvent>,
    mut ev_touch: EventReader<TouchInput>,
    touch_orbit: Option<Res<crate::plugins::camera::TouchOrbit>>,
) {
    let Ok((ball_t, mut kin)) = q_ball.get_single_mut() else { return; };
    let Ok(cam_t) = q_cam.get_single() else { return; };

    // Touch handling (mobile)
    for ev in ev_touch.read() {
        match ev.phase {
            bevy::input::touch::TouchPhase::Started => {
                if state.mode == Idle && state.touch_id.is_none() {
                    state.touch_id = Some(ev.id);
                    state.mode = Charging;
                    state.power = 0.0;
                    state.rising = true;
                    let indicator_origin = ball_t.translation + Vec3::Y * (kin.collider_radius * 0.5);
                    for (mut t, mut vis, _) in &mut q_indicators {
                        t.translation = indicator_origin;
                        *vis = Visibility::Visible;
                    }
                }
            }
            bevy::input::touch::TouchPhase::Moved => {
                // If this touch became a look (orbit) gesture, cancel charging.
                if state.touch_id == Some(ev.id) {
                    if let Some(to) = touch_orbit.as_ref() {
                        if to.look_active {
                            // Cancel shot charge
                            state.mode = ShotMode::Idle;
                            state.power = 0.0;
                            state.touch_id = None;
                            for (_, mut vis, _) in &mut q_indicators {
                                *vis = Visibility::Hidden;
                            }
                        }
                    }
                }
            }
            bevy::input::touch::TouchPhase::Ended | bevy::input::touch::TouchPhase::Canceled => {
                if state.touch_id == Some(ev.id) && state.mode == Charging {
                    // Fire shot (same logic as mouse release)
                    let cam_to_ball = (ball_t.translation - cam_t.translation).normalize_or_zero();
                    let horiz = Vec3::new(cam_to_ball.x, 0.0, cam_to_ball.z).normalize_or_zero();
                    let angle = cfg.up_angle_deg.to_radians();
                    let dir = (horiz * angle.cos() + Vec3::Y * angle.sin()).normalize_or_zero();
                    let power_scale = 0.25 + state.power * (2.0 - 0.25);
                    let impulse = cfg.base_impulse * power_scale;
                    kin.vel += dir * impulse;
                    ev_shot.send(ShotFiredEvent { pos: ball_t.translation, power: power_scale });
                    state.mode = ShotMode::Idle;
                    state.power = 0.0;
                    state.touch_id = None;
                    for (_, mut vis, _) in &mut q_indicators {
                        *vis = Visibility::Hidden;
                    }
                } else if state.touch_id == Some(ev.id) {
                    // Just clear the touch id if not charging
                    state.touch_id = None;
                }
            }
        }
    }

    // Mouse input (desktop / browser with mouse)
    if buttons.just_pressed(MouseButton::Left) && state.mode == Idle {
        state.mode = Charging;
        state.power = 0.0;
        state.rising = true;
        let indicator_origin = ball_t.translation + Vec3::Y * (kin.collider_radius * 0.5);
        for (mut t, mut vis, _) in &mut q_indicators {
            t.translation = indicator_origin;
            *vis = Visibility::Visible;
        }
    }

    if buttons.just_released(MouseButton::Left) && state.mode == Charging {
        let cam_to_ball = (ball_t.translation - cam_t.translation).normalize_or_zero();
        let horiz = Vec3::new(cam_to_ball.x, 0.0, cam_to_ball.z).normalize_or_zero();
        let angle = cfg.up_angle_deg.to_radians();
        let dir = (horiz * angle.cos() + Vec3::Y * angle.sin()).normalize_or_zero();

        let power_scale = 0.25 + state.power * (2.0 - 0.25);
        let impulse = cfg.base_impulse * power_scale;
        kin.vel += dir * impulse;
        ev_shot.send(ShotFiredEvent { pos: ball_t.translation, power: power_scale });

        state.mode = Idle;
        state.power = 0.0;
        for (_, mut vis, _) in &mut q_indicators {
            *vis = Visibility::Hidden;
        }
    }
}

fn update_shot_indicator(
    state: Res<ShotState>,
    cfg: Res<ShotConfig>,
    q_ball: Query<&Transform, (With<Ball>, Without<ShotIndicator>)>,
    q_cam: Query<&Transform, (With<OrbitCamera>, Without<Ball>, Without<ShotIndicator>)>,
    mut q_ind: Query<(&mut Transform, &Handle<StandardMaterial>, &mut Visibility, &ShotIndicatorDot), (With<ShotIndicator>, Without<Ball>, Without<OrbitCamera>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if state.mode != ShotMode::Charging {
        return;
    }
    let Ok(ball_t) = q_ball.get_single() else { return; };
    let Ok(cam_t) = q_cam.get_single() else { return; };
    let ball_pos = ball_t.translation;

    let cam_to_ball = (ball_pos - cam_t.translation).normalize_or_zero();
    let horiz = Vec3::new(cam_to_ball.x, 0.0, cam_to_ball.z).normalize_or_zero();
    let angle = cfg.up_angle_deg.to_radians();
    let dir = (horiz * angle.cos() + Vec3::Y * angle.sin()).normalize_or_zero();

    let power_scale = 0.25 + state.power * (2.0 - 0.25);
    let v0 = dir * (cfg.base_impulse * power_scale);
    let g = -9.81;
    let origin = ball_pos + Vec3::Y * 0.1;

    for (mut t, mat_handle, mut vis, dot) in &mut q_ind {
        *vis = Visibility::Visible;
        let time = (dot.index as f32 + 1.0) * TRAJ_DOT_DT;
        let displacement = v0 * time + 0.5 * Vec3::Y * g * time * time;
        t.translation = origin + displacement;

        if let Some(mat) = materials.get_mut(mat_handle) {
            let fade = 1.0 - (dot.index as f32 / TRAJ_DOT_COUNT as f32);
            let intensity = 0.3 + power_scale * 0.4 * fade;
            mat.emissive = LinearRgba::new(3.0, 2.0, 0.3, 1.0) * intensity;
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
            Idle => {
                text.sections[0].value = "Power: --".to_string();
            }
            Charging => {
                let power_scale = 0.25 + state.power * (2.0 - 0.25);
                text.sections[0].value = format!("Power: {:>3}%", (power_scale * 100.0) as u32);
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
        Idle => 0.0,
        Charging => state.power,
    };
    if let Ok((mut style, mut color)) = q_fill.get_single_mut() {
        style.width = Val::Percent(power * 100.0);
        // Gradient green -> yellow -> red (same logic as original)
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
