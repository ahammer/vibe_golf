# Architecture — Vibe Golf

Goal: Expand the rudimentary Bevy + Rapier prototype into a modular, testable 3D golf sandbox with orbit camera aiming, procedural terrain, targets, scoring metrics, and deterministic fixed-tick simulation.

## Guiding Principles
- Code-first, asset-light (generate meshes & terrain procedurally)
- Deterministic core simulation (all authoritative gameplay in FixedUpdate)
- Presentation decoupled (camera smoothing, HUD, VFX in Update)
- Small, focused plugins; feature flags for headless / debug
- Clear data flow: Input → Aim State → Fire Event → Physics → Scoring / Camera

---
## High-Level Flow
1. Player orbits camera around the ball (mouse move while not charging).
2. Player presses & holds (e.g. Left Mouse) to start aiming: we capture initial camera orientation and begin charging power (time-based or mouse drag distance).
3. On release, a `SwingEvent` is emitted with direction & power derived from camera forward (flattened & adjusted) and charge amount. 
4. FixedUpdate applies impulse to ball (physics). 
5. Camera follows ball position with a spring arm, clamped above terrain. 
6. On collision (or proximity) with target sensor, score updates; target despawns; new target spawns procedurally at a valid terrain location.
7. Terrain streamed / generated as needed (initially single chunk, expandable to chunk system).

---
## Modules / Plugins
```
src/
  main.rs               // App bootstrap, plugin registration
  components.rs         // Shared component definitions
  resources.rs          // Shared resource structs
  events.rs             // Event type declarations
  plugins/
    input.rs            // Mouse / keyboard → AimState, SwingEvent
    camera.rs           // Orbit + follow camera systems
    ball.rs             // Ball setup, swing impulse application, bounds checking
    target.rs           // Target spawn/despawn, hit detection
    terrain.rs          // Procedural heightmap, mesh + collider generation, sampling API
    scoring.rs          // ScoreState updates + metrics derivation
    hud.rs              // HUD text formatting (Update schedule)
    physics.rs          // Rapier config (gravity, tuning), friction materials
    gameplay.rs         // High-level orchestration (system sets / ordering)
  math/
    noise.rs            // Noise sampling helpers (Perlin + octaves)
```
Each file exposes a `Plugin` to keep `main.rs` slim.

---
## ECS Design
### Components
- `Ball` — marker
- `Target` — marker
- `TargetSensor` — marker (sensor collider child if using Rapier sensor)
- `MainCamera` — marker
- `CameraRig` — (optional marker if we separate camera entity from rig) 
- `TerrainChunk { chunk_coord: IVec2, size: f32 }`
- `AabbBounds` (optional for world boundary checks)

### Resources
- `SimState { tick: u64 }`
- `AimState { mode: AimMode, charge_start_tick: Option<u64>, power: f32, yaw: f32, pitch: f32 }`
  - `AimMode = Idle | Charging | CoolingDown`
- `CameraState { yaw: f32, pitch: f32, distance: f32, target_entity: Entity, smooth: f32, min_height_clearance: f32 }`
- `ScoreState { points: u32, strokes: u32, ticks_since_last_point: u64, strokes_since_last_point: u32, total_ticks: u64, last_point_tick: u64 }`
- `TerrainConfig { seed: u32, amplitude: f32, frequency: f32, octaves: u8, lacunarity: f32, gain: f32, chunk_size: f32 }`
- `SpawnConfig { min_target_distance: f32, max_target_distance: f32 }`
- `PhysicsTuning { ground_friction: f32, ball_restitution: f32, linear_damping: f32, angular_damping: f32 }`

### Events
- `SwingStartEvent`
- `SwingChargingEvent { charge_ratio: f32 }` (optional for UI feedback)
- `SwingEvent { dir: Vec3, power: f32 }`
- `TargetHitEvent { position: Vec3 }`
- `RequestSpawnTargetEvent`

> Only `SwingEvent`, `TargetHitEvent`, `RequestSpawnTargetEvent` strictly required; others help with UI.

---
## Schedules & System Sets
Use Bevy's schedule labels:
- `FixedUpdate` (authoritative sim): physics impulses, scoring updates, target spawning, terrain chunk generation scheduling
- `Update` (per frame): camera smoothing/orbit, HUD text, aim charge visual, input capture translation to resources/events

### System Ordering Sketch
```
// Update schedule
(InputCapture) -> (AimUpdate) -> (CameraOrbit) -> (HudUpdate)

// FixedUpdate schedule
(TickSim) -> (ApplySwingImpulse) -> (PhysicsStep handled by Rapier) -> (DetectTargetHit) -> (UpdateScoring) -> (HandleTargetRespawn)
```
Use `.configure_sets()` to express dependencies if needed (e.g., `ApplySwingImpulse` before Rapier step if using hooks; otherwise rely on default order if Rapier runs after FixedUpdate systems).

---
## Input & Aiming
- Mouse movement (while not charging) alters `CameraState.yaw/pitch` (clamped pitch range, e.g. [-70°, -10°]).
- Scroll wheel adjusts `CameraState.distance` (clamped).
- On LMB down: set `AimState.mode = Charging`; record `charge_start_tick`.
- While charging: compute `power = min(1.0, ticks_elapsed / charge_ticks_full)`; optional non-linear ramp (ease-out).
- On release: derive direction:
  - Base: camera forward projected onto horizontal plane (Vec3::new(f.x, 0.0, f.z).normalize()).
  - Add loft angle: e.g. apply small upward component proportional to power or constant (tunable).
  - Emit `SwingEvent { dir, power }`. Increment `ScoreState.strokes`.
  - Set mode to `CoolingDown` for N ticks to prevent accidental double-fire.

Impulse magnitude mapping: `impulse = dir * (min_power + power * (max_power - min_power))`.

---
## Ball Physics Control
- On spawn: collider with friction & restitution from `PhysicsTuning`.
- Apply impulses only in `FixedUpdate` via `ExternalImpulse` or directly modifying Rapier velocity.
- Add small linear & angular damping to stop perpetual sliding.
- World bounds system: if `|position| > BOUND_RADIUS` or y < -Y_LIMIT → reset ball near last valid position; increment strokes penalty.

### Address "accelerates off screen" Prototype Issue
Likely due to no friction + one-time impulse + free camera. We'll set:
- Friction coefficient (via Rapier material) high enough (e.g. 0.8)
- Restitution low (0.2–0.4) for rolling not bouncing
- Damping to bleed residual energy

---
## Camera Orbit + Follow (Spring Arm)
- Maintain spherical coordinates (yaw, pitch, distance)
- Desired camera position: `ball_pos + offset_from(yaw, pitch, distance)`
- Sample terrain height at projected xz; final camera y = max(desired_y, terrain_height + min_clearance)
- Smooth via: `cam_transform.translation = lerp(current, desired, 1.0 - exp(-smooth * delta_seconds))`
- Always look_at ball (optionally with slight lookahead: ball velocity * lookahead_factor)

---
## Target Lifecycle
- Each target has sensor collider (Rapier: `ActiveEvents::COLLISION_EVENTS; Sensor` flag) OR distance check in FixedUpdate.
- On `TargetHitEvent`:
  - Despawn target & sensor
  - Increment `ScoreState.points`
  - Reset `strokes_since_last_point`, `ticks_since_last_point=0`, `last_point_tick = SimState.tick`
  - Emit `RequestSpawnTargetEvent`
- Spawn logic chooses a random direction & distance within `[min_target_distance, max_target_distance]`, samples terrain height, validates slope (normal from height differences < slope threshold), ensures not colliding with ball.

---
## Procedural Terrain
Initial M0: single chunk flat (already). M1: single procedural height chunk. M2+: chunked streaming.

### Height Sampling
Use Perlin/simplex noise (crate: `noise` or custom fast function). Height function:
```
H(x,z) = amplitude * sum_{i=0}^{octaves-1} noise( (x+seed_offset) * frequency * lacunarity^i,
                                                  (z+seed_offset) * frequency * lacunarity^i ) * gain^i
```
Provide: `pub fn sample_height(x: f32, z: f32) -> f32` via `TerrainSampler` resource.

### Mesh Generation
- Grid resolution: `N x N` vertices; positions (x, H(x,z), z)
- Normals: compute via central differences or cross of adjacent triangles
- Indices: two triangles per quad

### Collider
- Rapier `Collider::trimesh(vertices, indices)` for small prototype OR `HeightField` when moving to chunks (stores heights in row-major grid).

### Chunking (future)
- `TerrainChunk` components with world origin base
- Lazy generate neighbor chunks when ball distance to edge < threshold
- Retain simple LRU to despawn far chunks

---
## Scoring Metrics
`ScoreState` fields derive metrics for HUD:
- Points: total targets collected
- Strokes: total swings
- Time (ticks) per point: `(total_ticks - last_point_tick_prev) / 60.0`
- Hits (strokes) per point: `strokes_since_last_point`
- Points since last hit: (always 0 or 1 for simple target chain; can extend if multi-point combos)
- Average strokes per point: `strokes as f32 / points as f32` (guard points>0)
- Average time per point: `total_ticks / points / 60.0`

HUD system recomputes text each frame using latest `ScoreState` + `SimState`.

---
## Data Flow Summary
```
Mouse Input ──> Input System ──> AimState (Charging) ──> Release ──> SwingEvent
SwingEvent ──(FixedUpdate)──> ApplySwingImpulse ──> Ball Physics (Rapier)
Ball + Target Sensor ──> Collision ──> TargetHitEvent ──> ScoreState & Respawn
Ball Position ──> CameraFollow (Update) ──> Updated Camera Transform
SimState.tick ──> Score/HUD
```

---
## System List (Initial Implementation)
| System | Schedule | Purpose |
|--------|----------|---------|
| `tick_sim` | FixedUpdate | Increment `SimState.tick`, `ScoreState.total_ticks` |
| `capture_input` | Update | Update yaw/pitch, start/stop charging |
| `update_aim_charge` | Update | Update `AimState.power` while charging |
| `emit_swing_event_on_release` | Update | Fire `SwingEvent` |
| `apply_swing_impulse` | FixedUpdate | Convert `SwingEvent` to physics impulse & stroke count |
| `camera_orbit_follow` | Update | Position & smooth camera |
| `detect_target_hit` | FixedUpdate | Generate `TargetHitEvent` via sensor/distance |
| `handle_target_hit` | FixedUpdate | Update score; request respawn |
| `spawn_target_system` | FixedUpdate | Spawn target on request |
| `update_hud` | Update | Render scores/metrics |
| `world_bounds_check` | FixedUpdate | Reset / penalize if ball leaves area |
| `generate_terrain_once` | Startup | Build initial mesh/collider |

---
## Testing Strategy
- Unit: aim direction math, height sampling, score metric calculations
- Integration (headless): build app with `MinimalPlugins + RapierPhysicsPlugin`; inject known noise (seed); simulate ticks applying a deterministic swing; assert target hit within expected stroke/time bounds.
- Property: for random seeds, ensure spawned target height within allowed amplitude & slope.

### Headless Mode Flag
Feature `headless` removes window & rendering plugins, adds minimal time driver; uses a loop to step fixed schedule deterministically.

---
## Planned Feature Flags
- `headless` (CI sims)
- `debug_physics` (Rapier debug render)
- `terrain_chunks` (enable streaming beyond single chunk)

---
## Immediate Next Implementation Steps (M1)
1. Refactor into plugins & modules scaffold (empty systems returning).
2. Add camera orbit & follow logic (no aiming yet).
3. Add input-driven swing (click → impulse) minimal.
4. Add target sensor + respawn & scoring.
5. Tune physics (friction/damping) so ball rolls to stop.

---
## Future Extensions
- Power curve UI arc overlay.
- Wind (adds lateral force per tick).
- Multiple simultaneous targets / combos.
- Replays (record SwingEvents + RNG seed).
- Terrain biomes (layered noise & material changes).

---
## Open Tuning Variables
| Name | Initial Guess | Notes |
|------|---------------|-------|
| min_power | 1.5 | Base impulse magnitude |
| max_power | 12.0 | Full charge impulse |
| charge_full_ticks | 120 | 2 seconds at 60 Hz |
| camera_distance | 8.0 | Starting orbital radius |
| camera_smooth | 8.0 | Higher = snappier |
| loft_angle_deg | 8–12 | Adds arc to shot |
| min_clearance | 1.5 | Camera height above terrain |
| target_min_dist | 10.0 | From ball when spawning |
| target_max_dist | 40.0 | Expansion ring |
| terrain_amplitude | 3.0 | Height variation |
| terrain_frequency | 0.08 | Base frequency |

---
## Risks & Mitigations
| Risk | Mitigation |
|------|------------|
| Terrain mesh too coarse → popping | Adaptive chunk resolution / normal recompute |
| Ball tunneling on fast shots | Enable Rapier CCD for ball when speed > threshold |
| Camera clipping into steep hills | Raycast from ball toward desired camera position to adjust distance |
| Performance regressions with large chunks | HeightField collider + chunk culling |
| Non-deterministic tests (time drift) | Drive fixed time manually in headless tests |

---
## Summary
This architecture decomposes the game into clean ECS-driven plugins, separates simulation from rendering, defines explicit data & event flows for swings and scoring, and lays a scalable path for procedural terrain and future features. Ready to scaffold modules and implement M1 (orbit + swing + target) next.
