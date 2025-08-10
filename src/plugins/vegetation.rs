// Procedural vegetation spawning & runtime management.
// Aggressive optimizations applied:
//  - Early rejection before expensive surface sampling
//  - Progressive streaming spawn (frame‑budgeted)
//  - Config resources (runtime tunable)
//  - Preloaded scene handles (no per-instance path formatting)
//  - Batched entity creation (spawn_batch)
//  - Distance culling with hysteresis + timed passes
//  - Shadow LOD: disable shadows for distant trees (no quality loss near player)
//  - Adaptive update timers (independent for culling & shadow LOD)
//
// Future potential (not yet):
//  - Real GPU instancing with extracted meshes
//  - Billboard / impostor far LOD
//  - Streaming unload + spatial partition
//  - Parallel sampling via task pool
//
// NOTE: For determinism you could replace thread_rng with a seeded RNG from cfg.seed.

use bevy::prelude::*;
use bevy::pbr::NotShadowCaster;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use noise::{NoiseFn, Perlin};
use rand::{thread_rng, Rng};
use crate::plugins::terrain::TerrainSampler;
use crate::plugins::scene::Ball;

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
                timer: Timer::from_seconds(VegetationCullingConfig::default().update_interval, TimerMode::Repeating),
            })
            .insert_resource(VegetationLodState {
                timer: Timer::from_seconds(VegetationLodConfig::default().update_interval, TimerMode::Repeating),
            })
            .add_systems(Update, (
                extract_tree_mesh_variants.before(progressive_spawn_trees),
                progressive_spawn_trees,
                cull_trees.after(progressive_spawn_trees),
                tree_lod_update.after(cull_trees),
                vegetation_perf_tuner.after(tree_lod_update),
            ));
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
            cell_size: 6.0,          // slightly finer to allow more candidate coverage (full view)
            noise_freq: 0.035,
            base_density: 1.0,
            threshold: 0.50,         // allow a few more candidates; spacing still governs clustering
            max_instances: 2600,     // higher cap to fill entire visible small level
            min_slope_normal_y: 0.70,
            scale_min: 5.0,
            scale_max: 10.0,
            samples_per_frame: 700,  // finish population quickly
            batch_spawn_flush: 256,
            min_spacing_inner: 22.0, // inner area very sparse
            min_spacing_slope: 12.0, // moderate spacing on slopes
            min_spacing_rim: 8.0,    // rim denser but spaced to avoid clumps
        }
    }
}

// Distance culling configuration
#[derive(Resource)]
pub struct VegetationCullingConfig {
    pub max_distance: f32,     // soft visibility radius (ignored if distance culling disabled)
    pub hysteresis: f32,       // +/- band to avoid popping
    pub update_interval: f32,  // seconds between passes
    pub enable_distance: bool, // if false, no distance-based hide (full population always visible)
}
impl Default for VegetationCullingConfig {
    fn default() -> Self {
        Self {
            max_distance: 1200.0, // large enough to cover full small level
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

// Shadow LOD config (keeps visual quality near player; disables distant shadow casters)
#[derive(Resource)]
pub struct VegetationLodConfig {
    pub shadows_full_on: f32,     // within this distance: always shadowed
    pub shadows_full_off: f32,    // beyond this distance: shadows disabled
    pub hysteresis: f32,          // distance band to prevent flicker
    pub update_interval: f32,     // seconds between checks
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

// Adaptive performance tuner – dynamically adjusts vegetation-related distances to approach target FPS.
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
            low_band: 0.92,   // tighten below ~110 FPS
            high_band: 1.05,  // relax above ~126 FPS
            default_cull: 200.0,       // updated to match increased draw distance
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
// Once ready we spawn PbrBundle instances instead of full SceneBundle hierarchies.
#[derive(Resource, Default)]
struct VegetationMeshVariants {
    ready: bool,
    variants: Vec<(Handle<Mesh>, Handle<StandardMaterial>)>,
}

#[derive(Component)]
struct TreeTemplate;

// Extract meshes + materials from hidden template scene instances.
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

    // Recursive descent
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

    // Keep first two distinct variants (if more collected)
    if !collected.is_empty() {
        collected.truncate(2);
        if !collected.is_empty() {
            variants.variants = collected;
            variants.ready = true;
            // Despawn templates now that we have raw mesh/material handles
            for root in q_templates.iter() {
                commands.entity(root).despawn_recursive();
            }
            info!("Vegetation instancing: extracted {} tree mesh variants", variants.variants.len());
        }
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
    inner_spawned: usize, // count of accepted inner play-area trees
    finished: bool,
    batch: Vec<(SceneBundle, (Tree, TreeCulled, TreeLod))>, // reusable batch buffer
    accepted_positions: Vec<Vec2>, // for spacing rejection
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
    if normal.y < min_slope_normal_y {
        0.0
    } else {
        normal.y.clamp(0.6, 1.0)
    }
}

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
    if rng.gen_bool(0.5) {
        a.clone()
    } else {
        b.clone()
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
    // Clone handles so we can both store them in the resource and still use the local copies
    commands.insert_resource(VegetationAssets { tree1: tree1.clone(), tree2: tree2.clone(), perlin });
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
        accepted_positions: Vec::new(),
    });

    // Spawn hidden template scenes to extract mesh/material variants for instancing.
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

fn progressive_spawn_trees(
    mut commands: Commands,
    sampler: Res<TerrainSampler>,
    mut state: ResMut<VegetationSpawnState>,
    assets: Res<VegetationAssets>,
    variants: Res<VegetationMeshVariants>,
    cfg: Res<VegetationConfig>,
) {
    if state.finished {
        return;
    }

    let mut rng = thread_rng();

    let total_points = state.points.len();
    let end = (state.cursor + cfg.samples_per_frame).min(total_points);

    while state.cursor < end && state.spawned < cfg.max_instances {
        let base = state.points[state.cursor];
        state.cursor += 1;
        state.attempts += 1;

        // Jitter
        let p = jitter_point(base, cfg.cell_size, &mut rng);

        // Cheap masks first
        // Radial base mask (clears very center smoothly)
        let r_mask_raw = radial_mask(p, sampler.cfg.play_radius);
        if r_mask_raw <= 0.0 {
            continue;
        }

        let r_len = p.length();
        let play_r = sampler.cfg.play_radius;
        let rim_start = sampler.cfg.rim_start;
        let rim_peak = sampler.cfg.rim_peak;

        // Region weighting strategy:
        //  - Inner deep center (< 0.5 * play_r): none
        //  - Inner play area (0.5*play_r .. play_r): sparse (target ~40–50 total)
        //  - Slope band (play_r .. rim_start): moderate increasing density
        //  - Rim band (rim_start .. rim_peak): highest density
        let mut region_inner = false;
        let weight = if r_len < play_r * 0.5 {
            0.0
        } else if r_len < play_r {
            region_inner = true;
            0.10 // sparse inner area
        } else if r_len < rim_start {
            let t = ((r_len - play_r) / (rim_start - play_r)).clamp(0.0, 1.0);
            let smooth = t * t * (3.0 - 2.0 * t);
            0.35 + 0.35 * smooth // 0.35 -> 0.70 across slope
        } else {
            let t = ((r_len - rim_start) / (rim_peak - rim_start)).clamp(0.0, 1.0);
            let smooth = t * t * (3.0 - 2.0 * t);
            0.70 + 0.30 * smooth // 0.70 -> 1.0 across rim band
        };

        // Enforce sparse inner quota cap (~50)
        if weight > 0.0 && region_inner && state.inner_spawned >= 50 {
            continue;
        }

        let r_mask = r_mask_raw * weight;
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
            radial_mask: r_mask, // already includes ring emphasis & inner suppression
            slope_mask: s_mask,
            density,
        };

        // Region-specific minimum spacing
        let spacing = if r_len < play_r {
            cfg.min_spacing_inner
        } else if r_len < rim_start {
            cfg.min_spacing_slope
        } else {
            cfg.min_spacing_rim
        };

        // Simple O(n) blue-noise style rejection (counts are low enough)
        let mut too_close = false;
        if spacing > 0.0 {
            let spacing2 = spacing * spacing;
            for prev in &state.accepted_positions {
                if prev.distance_squared(candidate.pos) < spacing2 {
                    too_close = true;
                    break;
                }
            }
        }
        if too_close {
            continue;
        }

        if decide_spawn(candidate.density, cfg.threshold) {
            let transform = build_transform(&candidate, &mut rng, &cfg);
            if variants.ready && !variants.variants.is_empty() {
                // Use instanced mesh/material variant
                let (mesh, material) = &variants.variants[rng.gen_range(0..variants.variants.len())];
                commands.spawn((
                    PbrBundle {
                        mesh: mesh.clone(),
                        material: material.clone(),
                        transform,
                        ..default()
                    },
                    Tree,
                    TreeCulled(false),
                    TreeLod { shadows_on: true },
                ));
            } else {
                // Fallback: spawn full scene (pre-extraction)
                let handle = random_tree_handle(&mut rng, &assets.tree1, &assets.tree2);
                state.batch.push((
                    SceneBundle {
                        scene: handle,
                        transform,
                        ..default()
                    },
                    (Tree, TreeCulled(false), TreeLod { shadows_on: true }),
                ));
            }
            if region_inner {
                state.inner_spawned += 1;
            }
            state.accepted_positions.push(candidate.pos);
            state.spawned += 1;
        }

        if state.batch.len() >= cfg.batch_spawn_flush {
            let drained = std::mem::take(&mut state.batch);
            // Flatten tuple structure for spawn_batch
            commands.spawn_batch(drained.into_iter().map(|(bundle, comps)| {
                (
                    bundle,
                    comps.0, // Tree
                    comps.1, // TreeCulled
                    comps.2, // TreeLod
                )
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

    // Finished condition
    if state.cursor >= total_points || state.spawned >= cfg.max_instances {
        state.finished = true;
        info!(
            "Vegetation build complete: spawned {} (inner={}) / attempts {} (early_noise_rejects={}, slope_rejects={}, points={})",
            state.spawned, state.inner_spawned, state.attempts, state.early_noise_rejects, state.slope_rejects, total_points
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
    // If distance culling disabled we keep everything visible (visibility managed only by Bevy frustum).
    if !cfg.enable_distance {
        return;
    }
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
    let Ok(ball_t) = q_ball.get_single() else { return; };
    let origin = ball_t.translation;

    let on_d2 = cfg.shadows_full_on * cfg.shadows_full_on;
    let off_d2 = cfg.shadows_full_off * cfg.shadows_full_off;
    let hysteresis = cfg.hysteresis;

    // Outer thresholds with hysteresis
    let enable_threshold = (cfg.shadows_full_on + hysteresis).powi(2);
    let disable_threshold = (cfg.shadows_full_off - hysteresis).powi(2);

    for (e, t, mut lod, shadow_flag) in &mut q_trees {
        let d2 = (t.translation - origin).length_squared();
        // If currently with shadows
        if lod.shadows_on {
            // Past disable range -> turn off
            if d2 > disable_threshold {
                lod.shadows_on = false;
                if shadow_flag.is_none() {
                    commands.entity(e).insert(NotShadowCaster);
                }
            }
        } else {
            // Return to shadowed if well within enable range
            if d2 < enable_threshold {
                lod.shadows_on = true;
                if shadow_flag.is_some() {
                    commands.entity(e).remove::<NotShadowCaster>();
                }
            }
        }
        // Hard cut: outside extreme off distance always remove shadows
        if d2 > off_d2 && shadow_flag.is_none() {
            commands.entity(e).insert(NotShadowCaster);
            lod.shadows_on = false;
        }
        // Inside sure-on distance always ensure shadows (overrides above)
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

    // Pull smoothed FPS
    let Some(fps_diag) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) else { return; };
    let Some(fps) = fps_diag.smoothed() else { return; };
    let fps = fps as f32; // convert f64 -> f32 for config comparison

    let ratio = fps / tuner.target_fps;
    // Decide direction
    if ratio < tuner.low_band {
        // Tighten: reduce cull distance & shadow ranges
        if cull_cfg.enable_distance && cull_cfg.max_distance > tuner.min_cull {
            cull_cfg.max_distance = (cull_cfg.max_distance - tuner.adjust_step).max(tuner.min_cull);
        }
        if lod_cfg.shadows_full_on > tuner.min_shadow_on {
            lod_cfg.shadows_full_on = (lod_cfg.shadows_full_on - tuner.adjust_step * 0.5).max(tuner.min_shadow_on);
        }
        if lod_cfg.shadows_full_off > tuner.min_shadow_off {
            lod_cfg.shadows_full_off = (lod_cfg.shadows_full_off - tuner.adjust_step).max(tuner.min_shadow_off);
        }
    } else if ratio > tuner.high_band {
        // Relax toward defaults (not past maxima)
        if cull_cfg.enable_distance && cull_cfg.max_distance < tuner.default_cull {
            cull_cfg.max_distance = (cull_cfg.max_distance + tuner.adjust_step).min(tuner.default_cull.min(tuner.max_cull));
        }
        if lod_cfg.shadows_full_on < tuner.default_shadow_on {
            lod_cfg.shadows_full_on = (lod_cfg.shadows_full_on + tuner.adjust_step * 0.5).min(tuner.default_shadow_on.min(tuner.max_shadow_on));
        }
        if lod_cfg.shadows_full_off < tuner.default_shadow_off {
            lod_cfg.shadows_full_off = (lod_cfg.shadows_full_off + tuner.adjust_step).min(tuner.default_shadow_off.min(tuner.max_shadow_off));
        }
    } else {
        // In band: gentle drift back toward defaults
        if cull_cfg.enable_distance && (cull_cfg.max_distance - tuner.default_cull).abs() > 1.0 {
            if cull_cfg.max_distance < tuner.default_cull {
                cull_cfg.max_distance = (cull_cfg.max_distance + tuner.adjust_step * 0.5).min(tuner.default_cull);
            } else {
                cull_cfg.max_distance = (cull_cfg.max_distance - tuner.adjust_step * 0.5).max(tuner.default_cull);
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
