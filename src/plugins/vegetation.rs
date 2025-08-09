// Procedural vegetation spawning (trees) using functional compositional pipeline.
// Updated: higher density + large scale (500–1000%) with functional decomposition.
//
// Pipeline Steps (functional style):
//  - generate_grid_points -> iterator of base positions
//  - jitter_point -> add randomness within a cell
//  - sample_surface -> query height & normal (expensive, deferred until after cheap tests)
//  - masks (slope_mask, radial_mask)
//  - noise_density -> Perlin noise modulation
//  - combine_density -> aggregate into final density
//  - decide_spawn -> thresholding
//  - build_transform -> rotation + big scale 5x–10x (500–1000%)
//  - spawn_tree
//
// Added (optimization stage):
//  - Early rejection before height/normal sampling
//  - Distance based visibility culling (with hysteresis + timed updates)
//
// NOTE: For determinism you could replace thread_rng with a seeded RNG from cfg.seed.

use bevy::prelude::*;
use noise::{NoiseFn, Perlin};
use rand::{thread_rng, Rng};
use crate::plugins::terrain::TerrainSampler;
use crate::plugins::scene::Ball;

pub struct VegetationPlugin;
impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        // Insert culling config/state + spawning + runtime culling system
        let cull_cfg = VegetationCullingConfig::default();
        let interval = cull_cfg.update_interval;
        app.insert_resource(cull_cfg)
            .insert_resource(VegetationCullingState {
                timer: Timer::from_seconds(interval, TimerMode::Repeating),
            })
            .add_systems(Startup, spawn_trees)
            .add_systems(Update, cull_trees);
    }
}

#[derive(Component)]
pub struct Tree;

#[derive(Component)]
struct TreeCulled(bool); // true if currently hidden

// Runtime-configurable (Resource later if needed)
const CELL_SIZE: f32 = 4.0;          // smaller cell => higher sampling density (was 10)
const NOISE_FREQ: f64 = 0.035;
const BASE_DENSITY: f32 = 1.0;
const THRESHOLD: f32 = 0.50;         // lower threshold => more spawns (was 0.58)
const MAX_INSTANCES: usize = 3000;   // safety cap (raise from 1200)
const MIN_SLOPE_NORMAL_Y: f32 = 0.70; // allow a bit steeper (was 0.72)
const SCALE_MIN: f32 = 5.0;          // 500%
const SCALE_MAX: f32 = 10.0;         // 1000%

// Distance culling configuration
#[derive(Resource)]
pub struct VegetationCullingConfig {
    pub max_distance: f32,    // soft visibility radius for trees
    pub hysteresis: f32,      // +/- band to avoid popping
    pub update_interval: f32, // seconds between culling passes
}
impl Default for VegetationCullingConfig {
    fn default() -> Self {
        Self {
            max_distance: 180.0,
            hysteresis: 12.0,
            update_interval: 0.4,
        }
    }
}

#[derive(Resource)]
struct VegetationCullingState {
    timer: Timer,
}

// Data structure passed through functional stages
#[derive(Clone, Debug)]
struct Candidate {
    pos: Vec2,
    height: f32,
    normal: Vec3,
    noise_norm: f32,
    radial_mask: f32,
    slope_mask: f32,
    density: f32,
}

// Functional stages

fn generate_grid_points(half: f32, cell: f32) -> impl Iterator<Item = Vec2> {
    // Inclusive coverage
    let steps = ((half * 2.0) / cell).ceil() as i32;
    (-steps / 2..=steps / 2).flat_map(move |j| {
        (-steps / 2..=steps / 2).map(move |i| Vec2::new(i as f32 * cell, j as f32 * cell))
    })
}

fn jitter_point(mut base: Vec2, cell: f32, rng: &mut impl Rng) -> Vec2 {
    base.x += rng.gen_range(-0.45..0.45) * cell;
    base.y += rng.gen_range(-0.45..0.45) * cell;
    base
}

fn sample_surface(sampler: &TerrainSampler, p: Vec2) -> (f32, Vec3) {
    let h = sampler.height(p.x, p.y);
    let n = sampler.normal(p.x, p.y);
    (h, n)
}

fn slope_mask(normal: Vec3) -> f32 {
    if normal.y < MIN_SLOPE_NORMAL_Y {
        0.0
    } else {
        normal.y.clamp(0.6, 1.0)
    }
}

fn radial_mask(p: Vec2, play_radius: f32) -> f32 {
    // Keep very center clearer; fade back in toward full density
    let clear_r = play_radius * 0.20;
    let fade_r = play_radius * 0.65;
    let r = p.length();
    if r <= clear_r {
        0.0
    } else if r >= fade_r {
        1.0
    } else {
        ((r - clear_r) / (fade_r - clear_r)).clamp(0.0, 1.0)
    }
}

fn noise_density(perlin: &Perlin, p: Vec2) -> f32 {
    let v = perlin.get([p.x as f64 * NOISE_FREQ, p.y as f64 * NOISE_FREQ]);
    ((v as f32) * 0.5 + 0.5).clamp(0.0, 1.0)
}

fn combine_density(base: f32, noise_norm: f32, radial: f32, slope: f32) -> f32 {
    base * noise_norm * radial * slope
}

fn decide_spawn(density: f32) -> bool {
    density > THRESHOLD
}

fn random_variant(rng: &mut impl Rng) -> u8 {
    if rng.gen_bool(0.5) {
        1
    } else {
        2
    }
}

fn build_transform(p: &Candidate, rng: &mut impl Rng) -> Transform {
    let rot = Quat::from_rotation_y(rng.gen_range(0.0..std::f32::consts::TAU));
    let scale_base = rng.gen_range(SCALE_MIN..SCALE_MAX);
    let scale_variation = Vec3::new(
        scale_base * rng.gen_range(0.95..1.05),
        scale_base * rng.gen_range(0.95..1.10),
        scale_base * rng.gen_range(0.95..1.05),
    );
    Transform {
        translation: Vec3::new(p.pos.x, p.height, p.pos.y),
        rotation: rot,
        scale: scale_variation,
    }
}

// Distance-based culling pass (runs at coarse interval to amortize cost)
fn cull_trees(
    time: Res<Time>,
    cfg: Res<VegetationCullingConfig>,
    mut state: ResMut<VegetationCullingState>,
    q_ball: Query<&Transform, With<Ball>>,
    mut q_trees: Query<(&mut Visibility, &Transform, &mut TreeCulled), With<Tree>>,
) {
    if !state.timer.tick(time.delta()).just_finished() {
        return;
    }
    let Ok(ball_t) = q_ball.get_single() else { return; };

    let origin = ball_t.translation;
    let max_d = cfg.max_distance;
    let h = cfg.hysteresis;
    let hide_r = max_d + h;
    let show_r = (max_d - h).max(0.0);
    let hide_r2 = hide_r * hide_r;
    let show_r2 = show_r * show_r;

    for (mut vis, t, mut culled) in &mut q_trees {
        let d2 = (t.translation - origin).length_squared();
        if !culled.0 && d2 > hide_r2 {
            *vis = Visibility::Hidden;
            culled.0 = true;
        } else if culled.0 && d2 < show_r2 {
            *vis = Visibility::Inherited;
            culled.0 = false;
        }
    }
}

fn spawn_trees(
    mut commands: Commands,
    assets: Res<AssetServer>,
    sampler: Res<TerrainSampler>,
) {
    let cfg = &sampler.cfg;
    let half = cfg.chunk_size * 0.5;
    let perlin = Perlin::new(cfg.seed.wrapping_add(917_331));
    let mut rng = thread_rng();

    let mut spawned = 0usize;
    let mut attempts = 0usize;
    let mut early_noise_rejects = 0usize;
    let mut slope_rejects = 0usize;

    for base in generate_grid_points(half, CELL_SIZE) {
        attempts += 1;
        if spawned >= MAX_INSTANCES {
            break;
        }

        // Jitter point within cell
        let p = jitter_point(base, CELL_SIZE, &mut rng);

        // Cheap radial + noise evaluation FIRST for early rejection
        // (height + normal sampling is comparatively expensive: up to 5 height queries)
        let r_mask = radial_mask(p, cfg.play_radius);
        if r_mask <= 0.0 {
            continue;
        }
        let n_val = noise_density(&perlin, p);

        // Preliminary density WITHOUT slope (slope_mask <= 1.0 cannot increase density)
        let prelim = BASE_DENSITY * n_val * r_mask;
        if prelim <= THRESHOLD {
            early_noise_rejects += 1;
            continue;
        }

        // Now sample surface (height + normal) only for survivors
        let (h, n) = sample_surface(&sampler, p);
        let s_mask = slope_mask(n);
        if s_mask <= 0.0 {
            slope_rejects += 1;
            continue;
        }

        let density = combine_density(BASE_DENSITY, n_val, r_mask, s_mask);

        let candidate = Candidate {
            pos: p,
            height: h,
            normal: n,
            noise_norm: n_val,
            radial_mask: r_mask,
            slope_mask: s_mask,
            density,
        };

        if decide_spawn(candidate.density) {
            let variant = random_variant(&mut rng);
            let scene_path = format!("models/tree_{}.glb#Scene0", variant);
            let transform = build_transform(&candidate, &mut rng);

            commands.spawn((
                SceneBundle {
                    scene: assets.load(scene_path),
                    transform,
                    ..default()
                },
                Tree,
                TreeCulled(false),
            ));
            spawned += 1;
        }
    }

    info!(
        "Vegetation: spawned {spawned} trees after {attempts} samples (cell={CELL_SIZE}, threshold={THRESHOLD}) | early_noise_rejects={early_noise_rejects} slope_rejects={slope_rejects}"
    );
}
