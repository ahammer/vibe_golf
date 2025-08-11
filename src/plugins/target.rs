// Target components, motion update, and hit detection / progression logic.
use bevy::prelude::*;
use rand::Rng;

use crate::plugins::ball::{Ball, BallKinematic};
use crate::plugins::game_state::{Score, update_high_score};
use crate::plugins::core_sim::SimState;
use crate::plugins::terrain::TerrainSampler;
use crate::plugins::particles::{TargetHitEvent, GameOverEvent};

#[derive(Component)]
pub struct Target;

#[derive(Component)]
pub struct TargetFloat {
    pub ground: f32,
    pub base_height: f32,
    pub amplitude: f32,
    pub phase: f32,
    pub rot_speed: f32,
    pub bounce_freq: f32,
}

// Runtime tunable target parameters (collider + animation config)
#[derive(Resource, Clone, Copy)]
pub struct TargetParams {
    pub base_height: f32,
    pub amplitude: f32,
    pub bob_freq: f32,
    pub rot_speed: f32,
    pub collider_radius: f32,
    pub visual_offset: f32, // constant vertical lift to account for model pivot (added)
}

pub struct TargetPlugin;
impl Plugin for TargetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, detect_target_hits)
            .add_systems(Update, update_target_motion);
    }
}

fn update_target_motion(
    time: Res<Time>,
    mut q: Query<(&mut Transform, &mut TargetFloat), With<Target>>,
) {
    let dt = time.delta_seconds();
    for (mut t, mut f) in &mut q {
        f.phase += dt * f.bounce_freq * std::f32::consts::TAU;
        let y = f.ground + f.base_height + f.amplitude * f.phase.sin();
        t.translation.y = y;
        t.rotate_local(Quat::from_rotation_y(f.rot_speed * dt));
    }
}

pub fn detect_target_hits(
    mut score: ResMut<Score>,
    sim: Res<SimState>,
    sampler: Res<TerrainSampler>,
    params: Option<Res<TargetParams>>,
    mut q_target: Query<(&mut Transform, &mut TargetFloat), (With<Target>, Without<Ball>)>,
    q_ball: Query<(&Transform, &BallKinematic), With<Ball>>,
    mut ev_hit: EventWriter<TargetHitEvent>,
    mut ev_game_over: EventWriter<GameOverEvent>,
) {
    let Ok((ball_t, kin)) = q_ball.get_single() else { return; };
    let Ok((mut target_t, mut float)) = q_target.get_single_mut() else { return; };
    let params = match params {
        Some(p) => *p,
        None => return,
    };

    // Collision test
    let center_dist = (ball_t.translation - target_t.translation).length();
    if center_dist > params.collider_radius + kin.collider_radius {
        return;
    }

    // Register hit
    score.hits += 1;
    ev_hit.send(TargetHitEvent { pos: target_t.translation });

    // Completion check
    if score.hits >= score.max_holes {
        score.game_over = true;
        score.final_time = sim.elapsed_seconds;
        ev_game_over.send(GameOverEvent { pos: ball_t.translation });
        update_high_score(&mut score);
        return;
    }

    // Reposition target:
    // Choose a random direction and distance (500..800) from the LAST target position.
    let mut rng = rand::thread_rng();
    float.phase = rng.gen_range(0.0..std::f32::consts::TAU);

    // Reposition target ensuring it does not spawn below minimum ground elevation.
    const MIN_TARGET_GROUND: f32 = 50.0;
    let base_x = target_t.translation.x;
    let base_z = target_t.translation.z;
    let mut chosen: Option<(f32, f32, f32)> = None;
    for _ in 0..40 {
        let dist = rng.gen_range(500.0..800.0);
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let cand_x = base_x + dist * angle.cos();
        let cand_z = base_z + dist * angle.sin();
        let g = sampler.height(cand_x, cand_z);
        if g >= MIN_TARGET_GROUND {
            chosen = Some((cand_x, cand_z, g));
            break;
        }
    }
    let (new_x, new_z, ground) = chosen.unwrap_or_else(|| {
        let g = sampler.height(base_x, base_z);
        (base_x, base_z, g)
    });
    float.ground = ground;
    float.base_height = params.base_height + params.visual_offset;
    float.amplitude = params.amplitude;
    float.bounce_freq = params.bob_freq;
    float.rot_speed = params.rot_speed;

    target_t.translation = Vec3::new(new_x, ground + params.base_height + params.visual_offset, new_z);
}
