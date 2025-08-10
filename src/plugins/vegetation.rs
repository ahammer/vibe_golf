// Procedural vegetation spawning & runtime management.
// Aggressive optimizations applied:
//  - Early rejection before expensive surface sampling
//  - Batched progressive spawning over multiple frames (avoids startup hitch)
//  - Config as resource (tunable at runtime / dev console friendly)
//  - Preloaded scene handles (no per-entity string formatting)
//  - Spawn batching using spawn_batch
//  - Distance culling with hysteresis + timed passes
//  - Optional hard cap enforcement + overshoot trimming
//
// Further future work (not yet implemented):
//  - Real mesh/instance batching (extract meshes, use Single mesh + GPU instancing)
//  - Per-distance LOD (swap to simplified model or billboard)
//  - Streaming / unloading outside terrain chunk
//  - Editor tooling to visualize spawn masks
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
        app.insert_resource(VegetationConfig::default())
            .add_systems(Startup, prepare_vegetation)
            .insert_resource(VegetationCullingConfig::default())
            .insert_resource(VegetationCullingState {
                timer: Timer::from_seconds(VegetationCullingConfig::default().update_interval, TimerMode::Repeating),
            })
            .add_systems(Update, (
                progressive_spawn_trees,
                cull_trees.after(progressive_spawn_trees),
            ));
    }
}

#[derive(Component)]
pub struct Tree;
#[derive(Component)]
struct TreeCulled(bool); // true if currently hidden

// ---------------- Configuration Resources ----------------

#[derive(Resource, Clone)]
pub struct VegetationConfig {
    pub cell_size: f32,
    pub noise_freq: f64,
    pub base_density: f32,
    pub threshold: f32,
    pub max_instances: usize,
    pub min_slope_normal_y: f32,
    pub scale_min: f32,
    pub scale_max: f32,
    pub samples_per_frame: usize, // how many grid cells evaluated per frame during spawn
    pub batch_spawn_flush: usize, // flush batch when this many queued
}
impl Default for VegetationConfig {
    fn default() -> Self {
        Self {
            cell_size: 4.0,
            noise_freq: 0.035,
            base_density: 1.0,
            threshold: 0.50,
            max_instances: 3000,
            min_slope_normal_y: 0.70,
            scale_min: 5.0,
            scale_max: 10.0,
            samples_per_frame: 650,     // tuned: ~1–2ms typical (adjust as needed)
            batch_spawn_flush: 256,
        }
    }
}

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
            max_distance: 160.0, // slightly tighter for more aggressive perf
            hysteresis: 14.0,
            update_interval: 0.33,
        }
    }
}

#[derive(Resource)]
struct VegetationCullingState {
    timer: Timer,
}

// Preloaded assets & shared noise
#[derive(Resource)]
struct VegetationAssets {
    tree1: Handle<Scene>,
    tree2: Handle<Scene>,
    perlin: Perlin,
}

// Progressive spawn state
#[derive(Resource)]
struct VegetationSpawnState {
    points: Vec<Vec2>,
    cursor: usize,
    spawned: usize,
    attempts: usize,
    early_noise_rejects: usize,
    slope_rejects: usize,
    finished: bool,
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

// ---------------- Utility / Functional Stages ----------------

fn generate_grid_points(half: f32, cell: f32) -> Vec<Vec2> {
    let steps = ((half * 2.0) / cell).ceil() as i32;
    let mut pts = Vec::with_capacity((steps * steps) as usize);
    for j in -steps/2 ..= steps/2 {
        for i in -steps/2 ..= steps/2 {
            pts.push(Vec2::new(i as f32 * cell, j as f32 * cell));
        }
    }
    pts
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

fn slope_mask(normal: Vec3, min_slope_normal_y: f32) -> f32 {
    if normal.y < min_slope_normal_y { 0.0 } else { normal.y.clamp(0.6, 1.0) }
}

fn radial_mask(p: Vec2, play_radius: f32) -> f32 {
    let clear_r = play_radius * 0.20;
    let fade_r = play_radius * 0.65;
    let r = p.length();
    if r <= clear_r { 0.0 }
    else if r >= fade_r { 1.0 }
    else { ((r - clear_r) / (fade_r - clear_r)).clamp(0.0, 1.0) }
}

fn noise_density(perlin: &Perlin, p: Vec2, noise_freq: f64) -> f32 {
    let v = perlin.get([p.x as f64 * noise_freq, p.y as f64 * noise_freq]);
    ((v as f32) * 0.5 + 0.5).clamp(0.0, 1.0)
}

fn combine_density(base: f32, noise_norm: f32, radial: f32, slope: f32) -> f32 {
    base * noise_norm * radial * slope
}

fn decide_spawn(density: f32, threshold: f32) -> bool {
    density > threshold
}

fn build_transform(p: &Candidate, rng: &mut impl Rng, cfg: &VegetationConfig) -> Transform {
    let rot = Quat::from_rotation_y(rng.gen_range(0.0..std::f32::consts::TAU));
    let scale_base = rng.gen_range(cfg.scale_min..cfg.scale_max);
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

fn random_tree_handle(rng: &mut impl Rng, a: &Handle<Scene>, b: &Handle<Scene>) -> Handle<Scene> {
    if rng.gen_bool(0.5) { a.clone() } else { b.clone() }
}

// ---------------- Systems ----------------

fn prepare_vegetation(
    mut commands: Commands,
    assets: Res<AssetServer>,
    sampler: Res<TerrainSampler>,
    cfg: Res<VegetationConfig>,
) {
    let half = sampler.cfg.chunk_size * 0.5;
    let points = generate_grid_points(half, cfg.cell_size);
    let perlin = Perlin::new(sampler.cfg.seed.wrapping_add(917_331));
    let tree1 = assets.load("models/tree_1.glb#Scene0");
    let tree2 = assets.load("models/tree_2.glb#Scene0");
    commands.insert_resource(VegetationAssets { tree1, tree2, perlin });
    commands.insert_resource(VegetationSpawnState {
        points,
        cursor: 0,
        spawned: 0,
        attempts: 0,
        early_noise_rejects: 0,
        slope_rejects: 0,
        finished: false,
    });
}

fn progressive_spawn_trees(
    mut commands: Commands,
    sampler: Res<TerrainSampler>,
    mut state: ResMut<VegetationSpawnState>,
    assets: Res<VegetationAssets>,
    cfg: Res<VegetationConfig>,
) {
    if state.finished { return; }

    let mut rng = thread_rng();
    let mut batch: Vec<(SceneBundle, Tree, TreeCulled)> = Vec::with_capacity(cfg.batch_spawn_flush);

    let total_points = state.points.len();
    let end = (state.cursor + cfg.samples_per_frame).min(total_points);

    while state.cursor < end && state.spawned < cfg.max_instances {
        let base = state.points[state.cursor];
        state.cursor += 1;
        state.attempts += 1;

        // Jitter
        let p = jitter_point(base, cfg.cell_size, &mut rng);

        // Cheap masks first
        let r_mask = radial_mask(p, sampler.cfg.play_radius);
        if r_mask <= 0.0 {
            continue;
        }
        let n_val = noise_density(&assets.perlin, p, cfg.noise_freq);

        let prelim = cfg.base_density * n_val * r_mask;
        if prelim <= cfg.threshold {
            state.early_noise_rejects += 1;
            continue;
        }

        // Surface
        let (h, n) = sample_surface(&sampler, p);
        let s_mask = slope_mask(n, cfg.min_slope_normal_y);
        if s_mask <= 0.0 {
            state.slope_rejects += 1;
            continue;
        }

        let density = combine_density(cfg.base_density, n_val, r_mask, s_mask);
        let candidate = Candidate {
            pos: p,
            height: h,
            normal: n,
            noise_norm: n_val,
            radial_mask: r_mask,
            slope_mask: s_mask,
            density,
        };

        if decide_spawn(candidate.density, cfg.threshold) {
            let handle = random_tree_handle(&mut rng, &assets.tree1, &assets.tree2);
            let transform = build_transform(&candidate, &mut rng, &cfg);
            batch.push((
                SceneBundle {
                    scene: handle,
                    transform,
                    ..default()
                },
                Tree,
                TreeCulled(false),
            ));
            state.spawned += 1;
        }

        if batch.len() >= cfg.batch_spawn_flush {
            let drained = std::mem::take(&mut batch);
            commands.spawn_batch(drained);
        }
    }

    // Flush remainder
    if !batch.is_empty() {
        let drained = std::mem::take(&mut batch);
        commands.spawn_batch(drained);
    }

    // Finished condition
    if state.cursor >= total_points || state.spawned >= cfg.max_instances {
        state.finished = true;
        info!(
            "Vegetation build complete: spawned {} / attempts {} (early_noise_rejects={}, slope_rejects={}, points={})",
            state.spawned, state.attempts, state.early_noise_rejects, state.slope_rejects, total_points
        );
    }
}

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
