# Product & Technical User Stories (Backlog)

Purpose: actionable, incremental stories to evolve the prototype (single-file Bevy 0.14 + Rapier) toward the architecture vision (deterministic fixed-tick gameplay core, modular plugins, automation-friendly). Each story has clear acceptance criteria (AC) and lightweight implementation notes.

Priority legend: P0 (next), P1 (soon), P2 (later exploration)
Release groupings are illustrative (can reorder if dependencies shift).

---
## Release R1 – Automation & Determinism Foundations

<!-- P0 Automated Screenshot Capture implemented (screenshot.rs plugin, --no-screenshot flag) and removed from active backlog. -->

### P0: Reinstate Fixed 60 Hz Gameplay Tick
<!-- As a gameplay engineer I want a deterministic fixed simulation tick separate from variable render frames to ensure reproducible physics authoring & tests. Done -->


### P1: Modularize Current Systems Into Plugins
As a maintainer I want the monolith split so new features land in cohesive modules.
AC:
- Create `src/plugins/` with: `core_sim.rs`, `autoplay.rs`, `camera.rs`, `hud.rs`, `scene.rs`.
- Each exposes a `Plugin` adding its systems/resources; `main.rs` slim (just app wiring).
- Behavior & logs unchanged (verified by comparing representative log lines before/after over a 10s run).
- No new gameplay logic introduced in this step.
Notes:
- Shared types (e.g., `SimState`, `AutoConfig`) placed in `core_sim.rs` and re-exported.
- Ensure ordering: `core_sim` before `autoplay` (needs SimState), camera/hud after scene spawn.

### P1: Headless Mode Skeleton
As a CI runner I want to execute simulation with no window for faster, reliable tests.
AC:
- Cargo feature `headless` switches plugins to `MinimalPlugins` + required subsets (physics, asset server as needed) & disables screenshot capture.
- A new integration test builds an `App` under headless feature, steps N fixed ticks, asserts `SimState.tick == N`.
- README documents usage.
Notes:
- Keep default (non-headless) path unchanged.

### P1: Deterministic Autoplay Angle Function Extracted
As a developer I want the swing direction logic isolated for unit testing.
AC:
- Pure function `fn autoplay_direction(swing_index: u64) -> Vec3` returns normalized XZ direction.
- Existing runtime uses it; unit test covers a few indices (0,1,5) & length ~1.
- No behavior change (exact impulses match previous log).

---
## Release R2 – Player Interaction & Scoring

### P0: Aim State Resource + Charging Input
As a player I want to hold input to charge power and release to swing.
AC:
- `AimState { mode: Idle|Charging, power: f32 }` resource added (per architecture doc pattern).
- Holding Space (desktop) transitions to Charging (power ramps 0..1 over e.g. 1.5s); releasing emits `SwingEvent { power }`.
- Autoplay can be disabled via `AutoConfig.autoplay_enabled`.
- HUD shows either `AUTO` or `CHARGE xx%` when charging.
Notes:
- Power ramp deterministic: increase only in `FixedUpdate`.

### P0: Apply Swing Impulse System
As a gameplay system I want swings applied on the fixed tick consuming events.
AC:
- `SwingEvent` defined; `FixedUpdate` system reads queued events once per tick.
- Impulse magnitude scales with event.power * configurable base.
- Autoplay and manual swings share same impulse application path (event-based).

### P1: Target Hit Event & Basic Scoring
As a player I want feedback when the ball hits the red target.
AC:
- Collision detection emits `TargetHitEvent` (Rapier contact event filtered for ball+target entities).
- `ScoreState { strokes: u32, hits: u32 }` resource increments `hits` on event.
- HUD adds `Hits: N` section.
- Non-blocking if multiple contacts in same frame (count once per distinct swing? – initial version: count every first contact per swing index).

### P2: Ball Respawn On Target Hit
As a player I want the ball reset after a successful target hit to continue practicing.
AC:
- On `TargetHitEvent` ball transform & linear/angular velocity reset to original spawn.
- Stroke count increments.
- Minimal cooldown (e.g., 0.5s) before next swing allowed.

---
## Release R3 – Camera & UX Polish

### P1: Orbit Camera Input
As a user I want to rotate the camera around the ball with the mouse while maintaining follow behavior.
AC:
- Holding Right Mouse + drag adjusts yaw (clamped pitch) stored in `CameraFollowState` resource.
- Camera system uses state yaw/pitch instead of velocity-derived forward when available.
- Autoplay unaffected when no input.

### P2: Motion Trail / Velocity Gizmo (Debug Only)
As a developer I want a quick visual of ball velocity.
AC:
- Behind `debug_visuals` feature: line or arrow mesh updated each frame, disabled otherwise.
- No performance regression when disabled.

---
## Release R4 – Persistence & Analytics

### P2: Run Telemetry JSON Output
As a developer I want structured output to compare runs between commits.
AC:
- On exit writes `run_telemetry.json` with: total_time, frames, swings, hits, average_speed, max_speed.
- Schema documented in README.

### P2: Rolling Screenshot History
As a developer I want multiple past screenshots to inspect progress.
AC:
- Configurable `screenshot_history = N` keeps `last_run.png` plus timestamped copies for previous N runs.
- Oldest file pruned automatically.

---
## Cross-Cutting Technical Debt Stories

### P1: Logging Consistency Pass
As a maintainer I want structured log prefixes for easier grep.
AC:
- All info logs start with category token `[SIM]`, `[SWING]`, `[AUTO]`, `[SHOT]`, etc.
- No excessive per-frame logs.

### P2: Clippy & Rustfmt CI Gate
As a maintainer I want style & lint checks automated.
AC:
- GitHub Action runs `cargo fmt -- --check` and `cargo clippy -- -D warnings`.
- Badge added to README.

### P2: Physics Parameter Tuning Table
As a designer I want a centralized place to adjust physics constants.
AC:
- Resource or module listing restitution, damping, impulse scaling, easily tweakable.
- Changes propagate without search-replace across code.

---
## Story Dependency Highlights
- Screenshot capture depends only on current rendering pipeline (R1 can start immediately).
- Fixed tick reinstatement should precede Aim / Swing event logic (provides deterministic timing).
- Modularization before larger feature growth reduces merge friction.
- Event-based swing application unifies autoplay & manual input early, reducing divergence.

---
## Initial Implementation Order Suggestion
1. Screenshot capture (quick win + CI artifact).  
2. Fixed tick reinstatement.  
3. Modularization into plugins.  
4. Autoplay refactor to events + extracted direction fn.  
5. Aim input & swing events.  
6. Target collision -> scoring.  
7. Headless mode + integration test.  
8. Camera orbit & further polish.

---
## Open Questions (Track / Clarify Before Implementing)
- Acceptable screenshot resolution? (Default to window size.)
- Need alpha channel in PNG? (Assume opaque.)
- Should hits count once per swing or per contact cluster? (Start simple: per contact event; refine later.)
- For deterministic tests, is physics step reproducible cross-platform with Rapier defaults? (If not, pin deterministic params or tolerance in assertions.)

---
## Ready Next: Screenshot Capture (R1/P0)
Reference this story and proceed to implement a minimal plugin stub plus config flag, then add a follow-up PR for headless exclusion & history retention.
