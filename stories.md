# Product & Technical User Stories (Backlog)

Purpose: actionable, incremental stories to evolve the prototype toward the architecture vision with a NEW emphasis on procedural terrain (noise-based heightfield, mesh generation, chunk streaming) as the next milestone. Each story lists acceptance criteria (AC) and minimal notes.

Priority legend: P0 (next), P1 (soon), P2 (later / exploratory)
Release groupings are thematic; reorder if dependencies require.

---
## Completed Foundations (Historical)
<!-- P0 Automated Screenshot Capture implemented (screenshot.rs plugin, --no-screenshot flag) -->
<!-- P0 Fixed 60 Hz Tick reinstated -->
<!-- P1 Modularization into plugins completed -->

These are retained for traceability but no longer active backlog items.

---
## Release RT1 – Procedural Terrain Core

Primary goal: replace static plane with deterministic noise-driven height mesh & enable forward path to infinite (chunked) generation.

### P0: TerrainConfig Resource & Deterministic Noise Sampler
As a systems engineer I want a centralized `TerrainConfig` and noise sampler so terrain & tests are reproducible.
AC:
- `TerrainConfig { seed, amplitude, frequency, octaves, lacunarity, gain, chunk_size }` resource created (matches architecture doc).
- `TerrainSampler` (or function) produces stable height values given (x,z) & seed.
- Unit test: sampling (x,z) twice returns identical value; different seeds differ.
- Logged once at startup: `[TERRAIN] seed=...`.

### P0: Generate Single Procedural Height Mesh + Collider
As a player I want a rolling hilly surface instead of a flat plane.
AC:
- Startup system builds a single N x N grid (configurable resolution) sized by `chunk_size` around origin.
- Heights from sampler; normals computed (central differences or cross).
- Rapier collider created (trimesh OK for single chunk).
- Old flat ground removed.
- Ball spawns at sampled height at (0,0) + small offset.

### P0: Camera Terrain Clearance Integration
As a user I want the camera to stay above hills smoothly.
AC:
- Camera follow/orbit system queries height under desired XZ and clamps Y >= height + clearance.
- No visible clipping into ground when traversing slopes.

### P1: Terrain Sampling API for Spawns
As a gameplay system I want to place targets on valid terrain.
AC:
- Provide `sample_height(x,z)` and `sample_normal(x,z)` helpers.
- Target spawn (existing / future) uses slope threshold: reject if |normal.y| < min_y (too steep).
- Logged spawn decisions at debug level with `[SPAWN]` prefix when `RUST_LOG=debug`.

### P1: Basic Target Respawn Using Terrain
As a player I want targets to appear embedded on hills at varying distances.
AC:
- On (manual or existing autoplay) start, one target spawns at distance within `[min_target_distance, max_target_distance]` using `SpawnConfig`.
- Height sampling ensures target sits flush (no floating / clipping > tolerance 0.05).
- Reuses existing target marker & collider approach (or sensor) unchanged otherwise.

### P1: Physics Tuning Pass For Slopes
As a physics tuner I want rolling to feel natural on inclines.
AC:
- Introduce `PhysicsTuning` resource (friction, restitution, damping) if not present.
- Adjust values so ball comes to rest within 6–8 seconds after full impulse on moderate slope.
- Document chosen constants inline & in README tuning section.

### P2: HeightField Optimization Option
As an engine developer I want a path to more efficient colliders.
AC:
- Feature flag (e.g. `terrain_heightfield`) swaps trimesh for Rapier `HeightField`.
- Benchmark note (log) prints vertex count vs heightfield dimensions.

### P2: Slope-Based Spawn Refinement
As a designer I want to avoid overly steep or concave areas for target placement.
AC:
- Evaluate slope variance in a local 3x3 sample; reject if max deviation > threshold.
- Fallback to additional random attempts (max 10) before giving up (log warning if fail).

### P2: Noise Parameter Hot-Reload (Debug)
As a developer I want to tweak noise live.
AC:
- When `debug_terrain` feature active: pressing numeric keys cycles amplitude/frequency presets.
- Mesh regenerates next frame (single chunk only) without crash.

---
## Release RT2 – Infinite Terrain (Chunk Streaming)

### P0: TerrainChunk Component & 3x3 Ring Generation
As a player I want terrain beyond the initial chunk so the ball doesn’t reach a hard edge quickly.
AC:
- Define `TerrainChunk { chunk_coord: IVec2, size: f32 }` component.
- On startup generate center + 8 neighbors (3x3).
- Shared sampler ensures seamless borders (height continuity within floating precision tolerance < 0.01).
- Collider per chunk (still trimesh acceptable initially).

### P0: Chunk Existence Manager
As a systems engineer I want to maintain a moving ring of chunks around the ball.
AC:
- FixedUpdate system: if ball within threshold of outer ring edge, spawn newly required ring (expand center coordinate) and despawn farthest old ring beyond retention radius.
- Max active chunk count logged each change.

### P1: Spawn & Target Validation Across Chunks
As a gameplay system I want target spawning to include newly generated chunks.
AC:
- Spawn logic samples candidate chunk set within distance range; ignores chunks not yet generated (triggers generation if needed optional P2).
- Works seamlessly as ball travels (no panic / missing sampler cases).

### P1: Simple Chunk LRU Despawn
As an engine maintainer I want memory bounded.
AC:
- Keep at most K (configurable) chunks; oldest outside ball radius removed.
- Log `[TERRAIN] despawn chunk (x,y)` events at info level.

### P2: Async Mesh Generation (Preparation)
As a performance engineer I want to avoid frame spikes.
AC:
- Stub system that would push generation tasks to thread pool (Bevy tasks); currently still sync but code structured to split sampling from entity spawn.
- Document hook points.

### P2: Collider LOD Experiment
As a researcher I want to evaluate coarser colliders for distant chunks.
AC:
- Distant (>2 rings) chunks use half resolution grid.
- Visual continuity preserved (normals acceptable, no obvious seams from standard camera height).

---
## Release G1 – Aiming & Swing (Reordered After Terrain Core)

### P0: Aim State Resource + Charging Input
As a player I want to hold input to charge power and release to swing.
AC:
- `AimState { mode: Idle|Charging|CoolingDown, power: f32 }` (or per architecture doc) added.
- Holding Space (temp binding) transitions to Charging (power ramps 0..1 deterministic in FixedUpdate via tick durations).
- Release emits `SwingEvent { power }`.
- HUD displays `CHARGE xx%` while charging else `IDLE` or `COOLDOWN`.

### P0: Apply Swing Impulse System
As a gameplay system I want swings applied deterministically.
AC:
- `SwingEvent` consumed once per FixedUpdate; impulse magnitude = base * power.
- Autoplay path (if enabled) also emits events (unified pipeline).

### P1: Target Hit Event & Basic Scoring
As a player I want feedback when the ball hits the target.
AC:
- Rapier contact (ball + target sensor) emits `TargetHitEvent`.
- `ScoreState { strokes, points }` increments points.
- HUD shows Points & Strokes.

### P2: Ball Respawn On Target Hit
As a player I want rapid iteration after scoring.
AC:
- On `TargetHitEvent` ball resets position & velocity after brief cooldown.
- Stroke count increments before reset.

---
## Release UX – Camera & Visual Debug

### P1: Orbit Camera Input
As a user I want to rotate the camera around the ball.
AC:
- Right Mouse drag adjusts yaw/pitch with clamps.
- Integrated with terrain clearance.

### P2: Motion Trail / Velocity Gizmo (Debug Feature)
As a developer I want to visualize ball velocity.
AC:
- Feature gated; minimal mesh updated per frame.

---
## Release A – Analytics & Persistence

### P2: Run Telemetry JSON Output
As a developer I want structured run data.
AC:
- On exit writes `run_telemetry.json` with time, swings, points, avg speeds.

### P2: Rolling Screenshot History
As a developer I want visual regression artifacts.
AC:
- Keep N previous screenshots, prune oldest.

---
## Cross-Cutting Technical Debt / Quality

### P1: Logging Consistency Pass
AC:
- Category prefixes `[SIM]`, `[TERRAIN]`, `[SWING]`, `[SPAWN]`, `[CHUNK]` standardized.

### P1: Physics Parameter Tuning Table (if not satisfied earlier)
AC:
- Central constants module/resource; single source of truth.

### P2: Clippy & Rustfmt CI Gate
AC:
- GH Action enforces formatting & warns-as-errors.

### P2: Headless Mode (Feature Flag)
AC:
- `headless` feature swaps to `MinimalPlugins`; deterministic stepping in tests.

---
## Story Dependency Highlights
- TerrainConfig & sampler precede any terrain mesh & chunk logic.
- Single procedural mesh precedes chunk streaming.
- Chunk streaming precedes long-distance spawn logic.
- Aim & swing can proceed in parallel after single mesh exists but before chunk streaming completes.
- Physics tuning feeds into slope & spawn validation quality.

---
## Suggested Updated Implementation Order
1. TerrainConfig + sampler (RT1 P0)
2. Single procedural mesh + collider (RT1 P0)
3. Camera terrain clearance (RT1 P0)
4. Sampling API + target spawn integration (RT1 P1)
5. Physics tuning pass (RT1 P1)
6. 3x3 chunk ring + chunk component (RT2 P0)
7. Chunk existence manager (RT2 P0)
8. Aim & swing events (G1 P0)
9. Apply swing impulse (G1 P0)
10. Target hit event & scoring (G1 P1)
11. Orbit camera input (UX P1)
12. LRU despawn + spawn across chunks (RT2 P1)
13. Logging consistency + tuning table (Cross P1)
14. Remaining P2 explorations (heightfield, async gen, gizmos, telemetry, headless, etc.)

---
## Open Questions (Track / Clarify)
- Grid resolution vs performance target for initial mesh? (Pick N=128 default; adjust later.)
- Acceptable chunk size for streaming prototype? (Start 64x64 world units.)
- Need LOD in first pass or acceptable to keep uniform resolution? (Uniform first.)
- Noise crate dependency vs custom lightweight implementation? (Prefer crate `noise` for speed unless binary size critical.)
- Deterministic RNG: use `rand_pcg` or `SmallRng` seeded from TerrainConfig.seed? (Decide P0.)

---
## Ready Next: TerrainConfig & Noise Sampler (RT1/P0)
Implement `TerrainConfig` + deterministic height sampler with unit test; then proceed to procedural mesh generation story.
