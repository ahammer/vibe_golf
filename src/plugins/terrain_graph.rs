use std::sync::Arc;
use noise::{Perlin, NoiseFn};
use bevy::prelude::*;

/// Context passed during node sampling.
pub struct GraphContext<'a> {
    pub perlin: &'a Perlin,
    pub cfg: &'a crate::plugins::terrain::TerrainConfig,
    pub seed_offset: Vec2,
}

/// Trait for all height graph nodes.
pub trait HeightNode: Send + Sync {
    fn sample(&self, x: f32, z: f32, ctx: &GraphContext) -> f32;
}

pub type NodeRef = Arc<dyn HeightNode>;

/// Simple Perlin noise node.
pub struct NoiseNode {
    pub frequency: f64,
    pub amplitude: f32,
}
impl HeightNode for NoiseNode {
    fn sample(&self, x: f32, z: f32, ctx: &GraphContext) -> f32 {
        let nx = (x + ctx.seed_offset.x) as f64 * self.frequency;
        let nz = (z + ctx.seed_offset.y) as f64 * self.frequency;
        (ctx.perlin.get([nx, nz]) as f32) * self.amplitude
    }
}

/// Fractal fBm noise.
pub struct FbmNode {
    pub base_frequency: f64,
    pub octaves: u8,
    pub lacunarity: f64,
    pub gain: f32,
    pub amplitude: f32,
}
impl HeightNode for FbmNode {
    fn sample(&self, x: f32, z: f32, ctx: &GraphContext) -> f32 {
        let mut freq = self.base_frequency;
        let mut amp = 1.0_f32;
        let mut sum = 0.0_f32;
        for _ in 0..self.octaves {
            let nx = (x + ctx.seed_offset.x) as f64 * freq;
            let nz = (z + ctx.seed_offset.y) as f64 * freq;
            let n = ctx.perlin.get([nx, nz]) as f32;
            sum += n * amp;
            freq *= self.lacunarity;
            amp *= self.gain;
        }
        sum * self.amplitude
    }
}

/// Ridge transform (1 - |n|)^2 applied to an input node.
pub struct RidgeNode {
    pub input: NodeRef,
    pub amplitude: f32,
}
impl HeightNode for RidgeNode {
    fn sample(&self, x: f32, z: f32, ctx: &GraphContext) -> f32 {
        let v = self.input.sample(x, z, ctx);
        let ridge = (1.0 - v.abs()).max(0.0).powi(2);
        ridge * self.amplitude
    }
}

/// Multiply input by scalar.
pub struct ScaleNode {
    pub input: NodeRef,
    pub scale: f32,
}
impl HeightNode for ScaleNode {
    fn sample(&self, x: f32, z: f32, ctx: &GraphContext) -> f32 {
        self.input.sample(x, z, ctx) * self.scale
    }
}

/// Add two inputs.
pub struct AddNode {
    pub a: NodeRef,
    pub b: NodeRef,
}
impl HeightNode for AddNode {
    fn sample(&self, x: f32, z: f32, ctx: &GraphContext) -> f32 {
        self.a.sample(x, z, ctx) + self.b.sample(x, z, ctx)
    }
}

/// Domain warp (modifies coordinates before sampling child).
pub struct DomainWarpNode {
    pub child: NodeRef,
    pub warp_frequency: f64,
    pub warp_amplitude: f32,
}
impl HeightNode for DomainWarpNode {
    fn sample(&self, x: f32, z: f32, ctx: &GraphContext) -> f32 {
        let wx = ctx.perlin.get([
            (x + ctx.seed_offset.x) as f64 * self.warp_frequency,
            (z + ctx.seed_offset.y + 57.31) as f64 * self.warp_frequency,
        ]) as f32;
        let wz = ctx.perlin.get([
            (x + ctx.seed_offset.x + 103.7) as f64 * self.warp_frequency,
            (z + ctx.seed_offset.y) as f64 * self.warp_frequency,
        ]) as f32;
        let warped_x = x + wx * self.warp_amplitude;
        let warped_z = z + wz * self.warp_amplitude;
        self.child.sample(warped_x, warped_z, ctx)
    }
}

/// Crater containment shaping applied after noise combination (uses original world coords).
pub struct CraterShapeNode {
    pub input: NodeRef,
}
impl HeightNode for CraterShapeNode {
    fn sample(&self, x: f32, z: f32, ctx: &GraphContext) -> f32 {
        let cfg = ctx.cfg;
        let base_val = self.input.sample(x, z, ctx);

        let r = Vec2::new(x, z).length();
        // smoothstep helper
        let smooth = |e0: f32, e1: f32, v: f32| {
            if e1 == e0 {
                return 0.0;
            }
            let mut t = ((v - e0) / (e1 - e0)).clamp(0.0, 1.0);
            t = t * t * (3.0 - 2.0 * t);
            t
        };

        let inner_flat = 1.0 - (r / cfg.play_radius).clamp(0.0, 1.0);
        let rim_t = smooth(cfg.rim_start, cfg.rim_peak, r);

        let noise_scale = 0.55 + 0.45 * rim_t;
        let flat_reduction = 0.5 * inner_flat;

        let mut shaped = base_val;
        shaped *= noise_scale * (1.0 - flat_reduction);
        shaped += rim_t.powf(1.25) * cfg.rim_height;
        shaped -= inner_flat.powf(2.0) * 1.2;

        shaped
    }
}

/// Build the procedural height graph replicating the legacy procedural combination
/// but in a compositional form:
/// final = crater_shape( domain_warp( base*0.6 + detail*0.35 + ridge(base)*0.8 ) )
pub fn build_terrain_graph(cfg: &crate::plugins::terrain::TerrainConfig) -> NodeRef {
    // Shared base noise (low frequency)
    let base = Arc::new(NoiseNode {
        frequency: cfg.base_frequency,
        amplitude: 1.0,
    }) as NodeRef;

    // Ridge from base
    let ridge = Arc::new(RidgeNode {
        input: base.clone(),
        amplitude: 0.8,
    }) as NodeRef;

    // Base scaled
    let base_scaled = Arc::new(ScaleNode {
        input: base.clone(),
        scale: 0.6,
    }) as NodeRef;

    // Detail fBm
    let detail = Arc::new(FbmNode {
        base_frequency: cfg.detail_frequency,
        octaves: cfg.detail_octaves,
        lacunarity: cfg.lacunarity,
        gain: cfg.gain as f32,
        amplitude: 0.35,
    }) as NodeRef;

    // base_scaled + detail
    let base_plus_detail = Arc::new(AddNode {
        a: base_scaled,
        b: detail,
    }) as NodeRef;

    // (base_scaled + detail) + ridge
    let combined = Arc::new(AddNode {
        a: base_plus_detail,
        b: ridge,
    }) as NodeRef;

    // Domain warp on combined
    let warped = Arc::new(DomainWarpNode {
        child: combined,
        warp_frequency: cfg.warp_frequency,
        warp_amplitude: cfg.warp_amplitude,
    }) as NodeRef;

    // No crater shaping (open world)
    warped
}
