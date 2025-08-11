// Game state & scoring resources, shot charge logic, and reset handling.

use bevy::prelude::*;
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::plugins::core_sim::SimState;
use crate::plugins::level::LevelDef;
use crate::plugins::ball::{Ball, BallKinematic};
use crate::plugins::target::{Target, TargetFloat, TargetParams};
use crate::plugins::terrain::TerrainSampler;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotMode {
    Idle,
    Charging,
}

#[derive(Resource, Debug)]
pub struct ShotState {
    pub mode: ShotMode,
    pub power: f32,          // 0..1 (oscillating)
    pub rising: bool,        // triangle wave direction
    pub touch_id: Option<u64>, // active charging touch (mobile)
}
impl Default for ShotState {
    fn default() -> Self {
        Self { mode: ShotMode::Idle, power: 0.0, rising: true, touch_id: None }
    }
}

#[derive(Resource, Debug, Clone, Copy, Deserialize)]
pub struct ShotConfig {
    pub osc_speed: f32,    // units per second (triangle wave edge speed)
    pub base_impulse: f32, // base launch velocity scale (multiplied by power scale)
    pub up_angle_deg: f32, // launch elevation angle
}
impl Default for ShotConfig {
    fn default() -> Self {
        Self { osc_speed: 1.6, base_impulse: 18.0, up_angle_deg: 45.0 }
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
            max_holes: 1,
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

pub struct GameStatePlugin;
impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ShotState::default())
            .insert_resource(ShotConfig::default())
            .insert_resource(Score::default())
            .add_systems(Update, update_shot_charge)
            .add_systems(Update, reset_game.after(crate::plugins::target::detect_target_hits)); // run after hit detection
    }
}

// Shot charging (triangle wave)
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

// Reset game when finished
fn reset_game(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<SimState>,
    mut score: ResMut<Score>,
    mut q_ball: Query<(&mut Transform, &mut BallKinematic), With<Ball>>,
    mut q_target: Query<(&mut Transform, &mut TargetFloat), (With<Target>, Without<Ball>)>,
    sampler: Res<TerrainSampler>,
    level: Option<Res<LevelDef>>,
    target_params: Option<Res<TargetParams>>,
) {
    if !(score.game_over && keys.just_pressed(KeyCode::KeyR)) {
        return;
    }
    sim.tick = 0;
    sim.elapsed_seconds = 0.0;

    let max_holes = level.as_ref().map(|l| l.scoring.max_holes).unwrap_or(score.max_holes);
    score.hits = 0;
    score.shots = 0;
    score.max_holes = max_holes;
    score.game_over = false;
    score.final_time = 0.0;

    if let Ok((mut t, mut kin)) = q_ball.get_single_mut() {
        // Spawn position from level or defaults
        if let Some(level) = level.as_ref() {
            let x = level.ball.pos.x;
            let z = level.ball.pos.z;
            let ground_h = sampler.height(x, z);
            let spawn_y = ground_h + kin.collider_radius + level.ball.spawn_height_offset;
            t.translation = Vec3::new(x, spawn_y, z);
        } else {
            let ground_h = sampler.height(0.0, 0.0);
            t.translation = Vec3::new(0.0, ground_h + kin.collider_radius + 10.0, 0.0);
        }
        t.rotation = Quat::IDENTITY;
        kin.vel = Vec3::ZERO;
    }

    if let (Ok((mut tt, mut tf)), Some(level), Some(params)) = (q_target.get_single_mut(), level.as_ref(), target_params) {
        let target_x = level.target.initial.x;
        let target_z = level.target.initial.z;
        let ground = sampler.height(target_x, target_z);
        tf.ground = ground;
        tf.phase = rand::random::<f32>() * std::f32::consts::TAU;
        tf.base_height = params.base_height;
        tf.amplitude = params.amplitude;
        tf.bounce_freq = params.bob_freq;
        tf.rot_speed = params.rot_speed;
        tt.translation = Vec3::new(
            target_x,
            ground + params.base_height + params.amplitude * tf.phase.sin(),
            target_z,
        );
    }
}

// Public utility for updating high score when finishing game
pub fn update_high_score(score: &mut Score) {
    let better = match score.high_score_time {
        Some(best) => score.final_time < best,
        None => true,
    };
    if better {
        score.high_score_time = Some(score.final_time);
        save_high_score_time(score.final_time);
    }
}

// Re-export commonly used items
pub use ShotMode::*;
