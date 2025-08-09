// Procedural vegetation spawning (trees) using a simple density field + Perlin noise mask.
//
// Approach (PCG steps mirrored from Unreal style workflows):
// 1. Define a uniform grid over the terrain bounds.
// 2. For each grid cell, sample a base density (constant) and modulate by Perlin noise (0..1).
// 3. Apply additional masks:
//      - Steep slope rejection (skip if surface normal too tilted)
//      - Radial falloff toward inner play area if desired (light thinning near center)
// 4. Jitter the spawn point inside the cell for variation.
// 5. Threshold the final density to decide whether to spawn a tree.
// 6. Randomize between multiple tree model variants and apply slight scale / rotation variance.
//
// This keeps logic deterministic per run (but not strictly seed stable yet because of thread_rng;
// could be switched to a seeded RNG if reproducibility is needed).

use bevy::prelude::*;
use noise::{Perlin, NoiseFn};
use rand::{Rng, thread_rng};
use crate::plugins::terrain::TerrainSampler;

pub struct VegetationPlugin;

impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_trees);
    }
}

#[derive(Component)]
pub struct Tree;

fn spawn_trees(
    mut commands: Commands,
    assets: Res<AssetServer>,
    sampler: Res<TerrainSampler>,
) {
    let cfg = &sampler.clone().cfg; // (cfg not public fields? we rely on public fields)
    let half = cfg.chunk_size * 0.5;

    // Grid spacing: controls overall density baseline
    let cell = 10.0_f32;
    let perlin = Perlin::new(cfg.seed.wrapping_add(917_331));

    // Noise frequency (world units -> noise domain)
    let noise_freq = 0.035_f64;
    // Density threshold: only spawn if final density > threshold
    let threshold = 0.58_f32;
    // Base density multiplier
    let base_density = 0.85_f32;

    // Optional interior thinning near very center (keep play area clearer)
    let clear_radius = cfg.play_radius * 0.35;
    let fade_radius = cfg.play_radius * 0.85; // start to fully allow after this
    let fade_range = (fade_radius - clear_radius).max(1.0);

    let mut rng = thread_rng();

    // Iterate grid
    let mut count = 0usize;
    let mut attempts = 0usize;
    let max_instances = 1200usize; // safety cap

    let mut z = -half;
    while z <= half {
        let mut x = -half;
        while x <= half {
            attempts += 1;

            // Random jitter inside the cell for less obvious grid patterns
            let jx = rng.gen_range(-0.45..0.45) * cell;
            let jz = rng.gen_range(-0.45..0.45) * cell;
            let px = x + jx;
            let pz = z + jz;

            // Inside terrain bounds, sample height & normal
            let h = sampler.height(px, pz);
            let n = sampler.normal(px, pz);

            // Reject steep slopes
            if n.y < 0.72 {
                x += cell;
                continue;
            }

            // Radial thinning near center
            let r = Vec2::new(px, pz).length();
            let center_mask = if r <= clear_radius {
                0.0
            } else if r >= fade_radius {
                1.0
            } else {
                ((r - clear_radius) / fade_range).clamp(0.0, 1.0)
            };

            // Noise (Perlin in [-1,1]) -> [0,1]
            let noise_val = perlin.get([px as f64 * noise_freq, pz as f64 * noise_freq]);
            let noise_norm = ((noise_val as f32) * 0.5 + 0.5).clamp(0.0, 1.0);

            // Combine densities
            let mut density = base_density * noise_norm * center_mask;

            // Light additional modulation based on slope (flatter slightly higher)
            density *= n.y.clamp(0.6, 1.0);

            if density > threshold {
                // Randomly pick one of the tree variants
                let variant = if rng.gen_bool(0.5) { 1 } else { 2 };
                let scene_path = format!("models/tree_{}.glb#Scene0", variant);

                // Random uniform rotation around Y
                let rot = Quat::from_rotation_y(rng.gen_range(0.0..std::f32::consts::TAU));
                // Slight non-uniform scale variation
                let scale_base = rng.gen_range(0.85..1.35);
                let scale_variation = Vec3::new(
                    scale_base * rng.gen_range(0.95..1.05),
                    scale_base * rng.gen_range(0.95..1.10),
                    scale_base * rng.gen_range(0.95..1.05),
                );

                commands.spawn((
                    SceneBundle {
                        scene: assets.load(scene_path),
                        transform: Transform {
                            translation: Vec3::new(px, h, pz),
                            rotation: rot,
                            scale: scale_variation,
                        },
                        ..default()
                    },
                    Tree,
                ));

                count += 1;
                if count >= max_instances {
                    break;
                }
            }

            x += cell;
        }
        if count >= max_instances {
            break;
        }
        z += cell;
    }

    info!("Vegetation: spawned {} trees after {} samples.", count, attempts);
}
