// Procedural vegetation spawning & runtime management.
// Optimizations & cleanup applied:
//  - Region weight + spacing logic refactored into helpers
//  - Spatial hash (uniform grid) for blue‑noise spacing (replaces O(n) scan)
//  - Reduced repeated calculations & clarified thresholds
//  - Early returns consolidated
//  - Minor inlining hints for hot path helpers
//
// Existing optimizations retained:
//  - Early rejection before expensive surface sampling
//  - Progressive streaming spawn (frame‑budgeted)
//  - Config resources (runtime tunable)
//  - Preloaded scene handles
//  - Batched entity creation (spawn_batch)
//  - Distance culling with hysteresis + timed passes
//  - Shadow LOD with hysteresis
//  - Adaptive performance tuner
//
// Future potential (not yet):
//  - True GPU instancing capture original child local transforms
//  - Billboard / impostor far LOD
//  - Streaming unload + spatial partition for runtime memory reclaim
//  - Parallel sampling via task pool
//
// NOTE: For determinism you could replace thread_rng with a seeded RNG from cfg.seed.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use noise::{NoiseFn, Perlin};
use rand::{thread_rng, Rng};
use std::collections::HashMap;

use crate::plugins::ball::Ball;
use crate::plugins::terrain::TerrainSampler;

pub struct VegetationPlugin;
impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(VegetationConfig::default())
            .insert_resource(VegetationCullingConfig::default())
            .insert_resource(VegetationLodConfig::default())
            .insert_resource(VegetationPerfTuner::default())
            .insert_resource(VegetationMeshVariants::default())
            .add_systems(Startup, prepare_vegetation)
            .insert_resource(VegetationCullingState {
                timer: Timer::from_seconds(
                    VegetationCullingConfig::default().update_interval,
                    TimerMode::Repeating,
                ),
            })
            .insert_resource(VegetationLodState {
                timer: Timer::from_seconds(
                    VegetationLodConfig::default().update_interval,
                    TimerMode::Repeating,
                ),
            })
            .add_systems(
                Update,
                (
                    extract_tree_mesh_variants.before(progressive_spawn_trees),
                    progressive_spawn_trees,
                    cull_trees.after(progressive_spawn_trees),
                    tree_lod_update.after(cull_trees),
                    vegetation_perf_tuner.after(tree_lod_update),
                ),
            );
    }
}

#[derive(Component)]
pub struct Tree;
#[derive(Component)]
struct TreeCulled(bool); // true if currently hidden

#[derive(Component)]
struct TreeLod {
    shadows_on: bool,
}

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
    pub samples_per_frame: usize, // grid cells evaluated per frame
    pub batch_spawn_flush: usize, // flush batch when queued >= this
    // Minimum spacing (approx) between accepted trees per region (pseudo blue-noise)
    pub min_spacing_inner: f32,
    pub min_spacing_slope: f32,
    pub min_spacing_rim: f32,
}
impl Default for VegetationConfig {
    fn default() -> Self {
        Self {
            cell_size: 6.0, // slightly finer sampling
            noise_freq: 0.035,
            base_density: 1.0,
            threshold: 0.50,
            max_instances: 2600,
            min_slope_normal_y: 0.70,
            scale_min: 5.0,
            scale_max: 10.0,
            samples_per_frame: 700,
            batch_spawn_flush: 256,
            min_spacing_inner: 22.0,
            min_spacing_slope: 12.0,
            min_spacing_rim: 8.0,
        }
    }
}

// Distance culling configuration
#[derive(Resource)]
pub struct VegetationCullingConfig {
    pub max_distance: f32,     // soft visibility radius
    pub hysteresis: f32,       // +/- band to avoid popping
    pub update_interval: f32,  // seconds between passes
    pub enable_distance: bool, // if false, no distance-based hide
}
impl Default for VegetationCullingConfig {
    fn default() -> Self {
        Self {
            max_distance: 1200.0,
            hysteresis: 14.0,
            update_interval: 0.33,
            enable_distance: false,
        }
    }
}

#[derive(Resource)]
struct VegetationCullingState {
    timer: Timer,
}

// Shadow LOD config
#[derive(Resource)]
pub struct VegetationLodConfig {
    pub shadows_full_on: f32,
    pub shadows_full_off: f32,
    pub hysteresis: f32,
    pub update_interval: f32,
}
impl Default for VegetationLodConfig {
    fn default() -> Self {
        Self {
            shadows_full_on: 110.0,
            shadows_full_off: 135.0,
            hysteresis: 6.0,
            update_interval: 0.25,
        }
    }
}

#[derive(Resource)]
struct VegetationLodState {
    timer: Timer,
}

// Adaptive performance tuner
#[derive(Resource)]
struct VegetationPerfTuner {
    timer: Timer,
    target_fps: f32,
    low_band: f32,
    high_band: f32,
    default_cull: f32,
    default_shadow_on: f32,
    default_shadow_off: f32,
    min_cull: f32,
    max_cull: f32,
    min_shadow_on: f32,
    max_shadow_on: f32,
    min_shadow_off: f32,
    max_shadow_off: f32,
    adjust_step: f32,
}
impl Default for VegetationPerfTuner {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.6, TimerMode::Repeating),
            target_fps: 120.0,
            low_band: 0.92,
            high_band: 1.05,
            default_cull: 200.0,
            default_shadow_on: 110.0,
            default_shadow_off: 135.0,
            min_cull: 130.0,
            max_cull: 240.0,
            min_shadow_on: 60.0,
            max_shadow_on: 140.0,
            min_shadow_off: 80.0,
            max_shadow_off: 200.0,
            adjust_step: 6.0,
        }
    }
}

// Preloaded assets & shared noise
#[derive(Resource)]
struct VegetationAssets {
    tree1: Handle<Scene>,
    tree2: Handle<Scene>,
    perlin: Perlin,
}

// Instanced mesh/material variants extracted from the scene glbs.
#[derive(Resource, Default)]
struct VegetationMeshVariants {
    ready: bool,
    variants: Vec<(Handle<Mesh>, Handle<StandardMaterial>)>,
}

#[derive(Component)]
struct TreeTemplate;

// ---------------- Spatial Hash For Spacing Rejection ----------------

#[derive(Default)]
struct SpacingGrid {
    cell: f32,
    cells: HashMap<(i32, i32), Vec<Vec2>>,
}

impl SpacingGrid {
    fn new(cell: f32) -> Self {
        Self {
            cell,
            cells: HashMap::new(),
        }
    }

    #[inline(always)]
    fn key(&self, p: Vec2) -> (i32, i32) {
        let inv = 1.0 / self.cell;
        let x = (p.x * inv).floor() as i32;
        let y = (p.y * inv).floor() as i32;
        (x, y)
    }

    #[inline(always)]
    fn insert(&mut self, p: Vec2) {
        let k = self.key(p);
        self.cells.entry(k).or_default().push(p);
    }

    fn too_close(&self, p: Vec2, spacing: f32) -> bool {
        if spacing <= 0.0 {
            return false;
        }
        let cell_range = ((spacing / self.cell).ceil() as i32).max(1);
        let (kx, ky) = self.key(p);
        let spacing2 = spacing * spacing;
        for dy in -cell_range..=cell_range {
            for dx in -cell_range..=cell_range {
                if let Some(list) = self.cells.get(&(kx + dx, ky + dy)) {
                    for &q in list {
                        if q.distance_squared(p) < spacing2 {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
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
    inner_spawned: usize,
    finished: bool,
    batch: Vec<(SceneBundle, (Tree, TreeCulled, TreeLod))>,
    spacing_grid: SpacingGrid,
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
    for j in -steps / 2..=steps / 2 {
        for i in -steps / 2..=steps / 2 {
            pts.push(Vec2::new(i as f32 * cell, j as f32 * cell));
        }
    }
    pts
}

#[inline(always)]
fn jitter_point(mut base: Vec2, cell: f32, rng: &mut impl Rng) -> Vec2 {
    base.x += rng.gen_range(-0.45..0.45) * cell;
    base.y += rng.gen_range(-0.45..0.45) * cell;
    base
}

#[inline(always)]
fn sample_surface(sampler: &TerrainSampler, p: Vec2) -> (f32, Vec3) {
    let h = sampler.height(p.x, p.y);
    let n = sampler.normal(p.x, p.y);
    (h, n)
}

#[inline(always)]
fn slope_mask(normal: Vec3, min_slope_normal_y: f32) -> f32 {
    if normal.y < min_slope_normal_y {
        0.0
    } else {
        normal.y.clamp(0.6, 1.0)
    }
}

#[inline(always)]
fn radial_mask(p: Vec2, play_radius: f32) -> f32 {
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

#[inline(always)]
fn noise_density(perlin: &Perlin, p: Vec2, noise_freq: f64) -> f32 {
    let v = perlin.get([p.x as f64 * noise_freq, p.y as f64 * noise_freq]);
    ((v as f32) * 0.5 + 0.5).clamp(0.0, 1.0)
}

#[inline(always)]
fn combine_density(base: f32, noise_norm: f32, radial: f32, slope: f32) -> f32 {
    base * noise_norm * radial * slope
}

#[inline(always)]
fn decide_spawn(density: f32, threshold: f32) -> bool {
    density > threshold
}

#[inline(always)]
fn build_transform(c: &Candidate, rng: &mut impl Rng, cfg: &VegetationConfig) -> Transform {
    let rot = Quat::from_rotation_y(rng.gen_range(0.0..std::f32::consts::TAU));
    let scale_base = rng.gen_range(cfg.scale_min..cfg.scale_max);
    let scale = Vec3::new(
        scale_base * rng.gen_range(0.95..1.05),
        scale_base * rng.gen_range(0.95..1.10),
        scale_base * rng.gen_range(0.95..1.05),
    );
    Transform {
        translation: Vec3::new(c.pos.x, c.height, c.pos.y),
        rotation: rot,
        scale,
    }
}

#[inline(always)]
fn random_tree_handle(rng: &mut impl Rng, a: &Handle<Scene>, b: &Handle<Scene>) -> Handle<Scene> {
    if rng.gen_bool(0.5) {
        a.clone()
    } else {
        b.clone()
    }
}

// Region weighting strategy.
// Returns (weight, region_inner_flag).
fn region_weight(r_len: f32, play_r: f32, rim_start: f32, rim_peak: f32) -> (f32, bool) {
    if r_len < play_r * 0.5 {
        return (0.0, false);
    }
    if r_len < play_r {
        return (0.10, true); // sparse inner
    }
    if r_len < rim_start {
        let t = ((r_len - play_r) / (rim_start - play_r)).clamp(0.0, 1.0);
        let smooth = t * t * (3.0 - 2.0 * t);
        (0.35 + 0.35 * smooth, false)
    } else {
        let t = ((r_len - rim_start) / (rim_peak - rim_start)).clamp(0.0, 1.0);
        let smooth = t * t * (3.0 - 2.0 * t);
        (0.70 + 0.30 * smooth, false)
    }
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

    // Spacing grid cell: half of smallest spacing for fine granularity
    let spacing_cell = (cfg.min_spacing_rim.min(cfg.min_spacing_slope).min(cfg.min_spacing_inner) * 0.5).max(1.0);

    commands.insert_resource(VegetationAssets {
        tree1: tree1.clone(),
        tree2: tree2.clone(),
        perlin,
    });
    commands.insert_resource(VegetationSpawnState {
        points,
        cursor: 0,
        spawned: 0,
        attempts: 0,
        early_noise_rejects: 0,
        slope_rejects: 0,
        inner_spawned: 0,
        finished: false,
        batch: Vec::with_capacity(cfg.batch_spawn_flush),
        spacing_grid: SpacingGrid::new(spacing_cell),
    });

    // Hidden template scenes to extract mesh/material variants later.
    commands.spawn((
        SceneBundle {
            scene: tree1.clone(),
            visibility: Visibility::Hidden,
            ..default()
        },
        TreeTemplate,
        Name::new("TreeTemplate1"),
    ));
    commands.spawn((
        SceneBundle {
            scene: tree2.clone(),
            visibility: Visibility::Hidden,
            ..default()
        },
        TreeTemplate,
        Name::new("TreeTemplate2"),
    ));
}

fn extract_tree_mesh_variants(
    mut commands: Commands,
    mut variants: ResMut<VegetationMeshVariants>,
    q_templates: Query<Entity, With<TreeTemplate>>,
    q_children: Query<&Children>,
    q_mesh_mats: Query<(&Handle<Mesh>, &Handle<StandardMaterial>)>,
) {
    if variants.ready {
        return;
    }
    let mut collected: Vec<(Handle<Mesh>, Handle<StandardMaterial>)> = Vec::new();

    fn visit(
        e: Entity,
        q_children: &Query<&Children>,
        q_mesh_mats: &Query<(&Handle<Mesh>, &Handle<StandardMaterial>)>,
        out: &mut Vec<(Handle<Mesh>, Handle<StandardMaterial>)>,
    ) {
        if let Ok((mesh, mat)) = q_mesh_mats.get(e) {
            out.push((mesh.clone(), mat.clone()));
        }
        if let Ok(children) = q_children.get(e) {
            for &c in children.iter() {
                visit(c, q_children, q_mesh_mats, out);
            }
        }
    }

    for root in q_templates.iter() {
        visit(root, &q_children, &q_mesh_mats, &mut collected);
    }

    if !collected.is_empty() {
        collected.truncate(2);
        variants.variants = collected;
        variants.ready = true;
        for root in q_templates.iter() {
            commands.entity(root).despawn_recursive();
        }
        info!(
            "Vegetation instancing: extracted {} tree mesh variants",
            variants.variants.len()
        );
    }
}

fn progressive_spawn_trees(
    mut commands: Commands,
    sampler: Res<TerrainSampler>,
    mut state: ResMut<VegetationSpawnState>,
    assets: Res<VegetationAssets>,
    _variants: Res<VegetationMeshVariants>,
    cfg: Res<VegetationConfig>,
) {
    if state.finished {
        return;
    }

    let mut rng = thread_rng();
    let total_points = state.points.len();
    let end = (state.cursor + cfg.samples_per_frame).min(total_points);

    let play_r = sampler.cfg.play_radius;
    let rim_start = sampler.cfg.rim_start;
    let rim_peak = sampler.cfg.rim_peak;

    while state.cursor < end && state.spawned < cfg.max_instances {
        let base = state.points[state.cursor];
        state.cursor += 1;
        state.attempts += 1;

        // Jitter point
        let p = jitter_point(base, cfg.cell_size, &mut rng);

        // Radial mask early
        let r_mask_raw = radial_mask(p, play_r);
        if r_mask_raw <= 0.0 {
            continue;
        }

        let r_len = p.length();
        let (weight, region_inner) = region_weight(r_len, play_r, rim_start, rim_peak);

        // Enforce sparse inner quota cap
        if region_inner && state.inner_spawned >= 50 {
            continue;
        }

        let r_mask = r_mask_raw * weight;
        if r_mask <= 0.0 {
            continue;
        }

        // Noise layer
        let n_val = noise_density(&assets.perlin, p, cfg.noise_freq);
        // Quick preliminary test
        if cfg.base_density * n_val * r_mask <= cfg.threshold {
            state.early_noise_rejects += 1;
            continue;
        }

        // Surface sample (expensive)
        let (h, n) = sample_surface(&sampler, p);
        let s_mask = slope_mask(n, cfg.min_slope_normal_y);
        if s_mask <= 0.0 {
            state.slope_rejects += 1;
            continue;
        }

        // Final density
        let density = combine_density(cfg.base_density, n_val, r_mask, s_mask);
        if !decide_spawn(density, cfg.threshold) {
            continue;
        }

        // Region-specific minimum spacing
        let spacing = if r_len < play_r {
            cfg.min_spacing_inner
        } else if r_len < rim_start {
            cfg.min_spacing_slope
        } else {
            cfg.min_spacing_rim
        };

        // Spatial hash rejection
        if state.spacing_grid.too_close(p, spacing) {
            continue;
        }

        let candidate = Candidate {
            pos: p,
            height: h,
            normal: n,
            noise_norm: n_val,
            radial_mask: r_mask,
            slope_mask: s_mask,
            density,
        };

        let transform = build_transform(&candidate, &mut rng, &cfg);

        // Still using full scenes (template local offsets lost for direct mesh instancing)
        let handle = random_tree_handle(&mut rng, &assets.tree1, &assets.tree2);
        state.batch.push((
            SceneBundle {
                scene: handle,
                transform,
                ..default()
            },
            (Tree, TreeCulled(false), TreeLod { shadows_on: true }),
        ));

        if region_inner {
            state.inner_spawned += 1;
        }
        state.spacing_grid.insert(candidate.pos);
        state.spawned += 1;

        if state.batch.len() >= cfg.batch_spawn_flush {
            let drained = std::mem::take(&mut state.batch);
            commands.spawn_batch(drained.into_iter().map(|(bundle, comps)| {
                (bundle, comps.0, comps.1, comps.2)
            }));
        }
    }

    // Flush remainder
    if !state.batch.is_empty() {
        let drained = std::mem::take(&mut state.batch);
        commands.spawn_batch(drained.into_iter().map(|(bundle, comps)| {
            (bundle, comps.0, comps.1, comps.2)
        }));
    }

    // Finish condition
    if state.cursor >= total_points || state.spawned >= cfg.max_instances {
        state.finished = true;
        info!(
            "Vegetation build complete: spawned {} (inner={}) / attempts {} (early_noise_rejects={}, slope_rejects={}, points={})",
            state.spawned,
            state.inner_spawned,
            state.attempts,
            state.early_noise_rejects,
            state.slope_rejects,
            total_points
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
    if !cfg.enable_distance {
        return;
    }
    if !state.timer.tick(time.delta()).just_finished() {
        return;
    }
    let Ok(ball_t) = q_ball.get_single() else {
        return;
    };

    let origin = ball_t.translation;
    let max_d = cfg.max_distance;
    let h = cfg.hysteresis;
    let hide_r2 = (max_d + h).powi(2);
    let show_r2 = (max_d - h).max(0.0).powi(2);

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

fn tree_lod_update(
    time: Res<Time>,
    cfg: Res<VegetationLodConfig>,
    mut state: ResMut<VegetationLodState>,
    q_ball: Query<&Transform, With<Ball>>,
    mut q_trees: Query<(Entity, &Transform, &mut TreeLod, Option<&NotShadowCaster>), With<Tree>>,
    mut commands: Commands,
) {
    if !state.timer.tick(time.delta()).just_finished() {
        return;
    }
    let Ok(ball_t) = q_ball.get_single() else {
        return;
    };
    let origin = ball_t.translation;

    let on_d2 = cfg.shadows_full_on.powi(2);
    let off_d2 = cfg.shadows_full_off.powi(2);

    let enable_threshold = (cfg.shadows_full_on + cfg.hysteresis).powi(2);
    let disable_threshold = (cfg.shadows_full_off - cfg.hysteresis).powi(2);

    for (e, t, mut lod, shadow_flag) in &mut q_trees {
        let d2 = (t.translation - origin).length_squared();

        if lod.shadows_on {
            if d2 > disable_threshold {
                lod.shadows_on = false;
                if shadow_flag.is_none() {
                    commands.entity(e).insert(NotShadowCaster);
                }
            }
        } else if d2 < enable_threshold {
            lod.shadows_on = true;
            if shadow_flag.is_some() {
                commands.entity(e).remove::<NotShadowCaster>();
            }
        }

        // Hard clamp extremes
        if d2 > off_d2 && shadow_flag.is_none() {
            commands.entity(e).insert(NotShadowCaster);
            lod.shadows_on = false;
        }
        if d2 < on_d2 && shadow_flag.is_some() {
            commands.entity(e).remove::<NotShadowCaster>();
            lod.shadows_on = true;
        }
    }
}

fn vegetation_perf_tuner(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut tuner: ResMut<VegetationPerfTuner>,
    mut cull_cfg: ResMut<VegetationCullingConfig>,
    mut lod_cfg: ResMut<VegetationLodConfig>,
) {
    if !tuner.timer.tick(time.delta()).just_finished() {
        return;
    }

    let Some(fps_diag) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) else {
        return;
    };
    let Some(fps) = fps_diag.smoothed() else {
        return;
    };
    let fps = fps as f32;

    let ratio = fps / tuner.target_fps;
    if ratio < tuner.low_band {
        // Tighten
        if cull_cfg.enable_distance && cull_cfg.max_distance > tuner.min_cull {
            cull_cfg.max_distance = (cull_cfg.max_distance - tuner.adjust_step).max(tuner.min_cull);
        }
        if lod_cfg.shadows_full_on > tuner.min_shadow_on {
            lod_cfg.shadows_full_on =
                (lod_cfg.shadows_full_on - tuner.adjust_step * 0.5).max(tuner.min_shadow_on);
        }
        if lod_cfg.shadows_full_off > tuner.min_shadow_off {
            lod_cfg.shadows_full_off =
                (lod_cfg.shadows_full_off - tuner.adjust_step).max(tuner.min_shadow_off);
        }
    } else if ratio > tuner.high_band {
        // Relax
        if cull_cfg.enable_distance && cull_cfg.max_distance < tuner.default_cull {
            cull_cfg.max_distance = (cull_cfg.max_distance + tuner.adjust_step)
                .min(tuner.default_cull.min(tuner.max_cull));
        }
        if lod_cfg.shadows_full_on < tuner.default_shadow_on {
            lod_cfg.shadows_full_on = (lod_cfg.shadows_full_on + tuner.adjust_step * 0.5)
                .min(tuner.default_shadow_on.min(tuner.max_shadow_on));
        }
        if lod_cfg.shadows_full_off < tuner.default_shadow_off {
            lod_cfg.shadows_full_off = (lod_cfg.shadows_full_off + tuner.adjust_step)
                .min(tuner.default_shadow_off.min(tuner.max_shadow_off));
        }
    } else {
        // Drift toward defaults
        if cull_cfg.enable_distance && (cull_cfg.max_distance - tuner.default_cull).abs() > 1.0 {
            if cull_cfg.max_distance < tuner.default_cull {
                cull_cfg.max_distance =
                    (cull_cfg.max_distance + tuner.adjust_step * 0.5).min(tuner.default_cull);
            } else {
                cull_cfg.max_distance =
                    (cull_cfg.max_distance - tuner.adjust_step * 0.5).max(tuner.default_cull);
            }
        }
    }

    // Maintain ordering constraints & minimum separation
    if lod_cfg.shadows_full_on + 5.0 > lod_cfg.shadows_full_off {
        lod_cfg.shadows_full_off = lod_cfg.shadows_full_on + 5.0;
    }
    if cull_cfg.max_distance < lod_cfg.shadows_full_off + 10.0 {
        cull_cfg.max_distance = lod_cfg.shadows_full_off + 10.0;
    }
}
