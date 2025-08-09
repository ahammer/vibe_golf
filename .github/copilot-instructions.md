# AI Coding Agent Instructions — Vibe Golf

Concise, project-specific guidance so an AI agent can contribute productively. Focus on THIS repo's actual patterns (Rust + Bevy 0.14 + Rapier 3D). Keep additions deterministic & modular.

## Core Vision
Minimal 3D golf sandbox emphasizing:
- Deterministic fixed-tick simulation (60 Hz) for gameplay & physics authoring
- Decoupled per-frame presentation (camera, HUD, smoothing) from fixed logic
- Code-only assets: meshes procedurally created, single font in `assets/fonts/`
- Path toward modular plugins (architecture docs describe future breakdown)

## Current State (M0+ Autoplay Prototype)
Implemented in a single file: `src/main.rs` with systems for scene setup, scripted ball impulses, HUD, and auto-exit. Architecture docs (`architecture.md`, `game_design.md`) outline future modularization (plugins, terrain, scoring). Treat them as authoritative design references when refactoring.

## Key Files
- `src/main.rs` — Bootstrap, resources, components, systems (FixedUpdate + Update)
- `architecture.md` / `game_design.md` — Target decomposition into plugins & ECS data model
- `assets/fonts/` — Must contain `FiraSans-Bold.ttf` (or any .ttf at that path) for text

## Build & Run
ALWAYS RUN, Autorun will print logs for 10 seconds to return results
- Build: `cargo run --quiet`
- Test: `cargo test` (currently unit test for `SimState` only)

## Deterministic Simulation Pattern
- Fixed tick set via `Time::<Fixed>::from_hz(60.0)`.
- Author gameplay impulses / state mutation ONLY in `FixedUpdate` systems (e.g., `scripted_autoplay`).
- Per-frame (`Update`) systems should be side-effect-free except for presentation (HUD text, camera smoothing, future VFX).

## Resources & Components in Use
- `SimState { tick }` — monotonically increments in `tick_state` (FixedUpdate) and drives timing / logging.
- `AutoConfig` — automation parameters (intervals, impulse magnitude) enabling headless / CI runs.
- Components: `Ball`, `Hud` markers to query entities; physics via Rapier components (`RigidBody`, `Collider`, `ExternalImpulse`, `Velocity`).

## System Responsibilities (Existing)
FixedUpdate:
1. `tick_state` — increment simulation tick.
2. `scripted_autoplay` — periodic `ExternalImpulse` insertion (simulate swings).
3. `debug_log_each_second` — logs every 60 ticks.
4. `exit_on_duration` — sends `AppExit` after configured ticks.
Update:
- `update_hud` — reads `Velocity` + `SimState` to format HUD text.
Startup:
- `setup_scene`, `setup_ui` — create camera, light, ground, ball, target, HUD text.

## Extending Toward Planned Architecture
When adding features (camera orbit, input-driven swings, scoring, terrain):
- Create new modules under `src/` (or `src/plugins/`) each exporting a Bevy `Plugin` to keep `main.rs` slim.
- Port ECS data definitions (components/resources/events) from docs rather than inventing new names. Example: implement `AimState`, `ScoreState`, `SwingEvent` exactly as specified.
- Keep authoritative physics impulses in a FixedUpdate system `apply_swing_impulse` that consumes `SwingEvent` events.

## Event & Data Flow (Target Model)
Input -> `AimState` update (Update) -> release generates `SwingEvent` -> FixedUpdate system applies impulse -> Rapier updates `Velocity` -> Target collision triggers `TargetHitEvent` -> scoring & respawn.
Implement incrementally; unused planned events should not be stubbed without usage.

## Patterns to Preserve
- Insert impulses by adding `ExternalImpulse` component (Rapier consumes & clears it).
- Logging cadence keyed to ticks (multiples of 60 for seconds) to avoid log spam.
- HUD text updated in Update using preloaded font path; keep font path stable.

## Testing / Headless Direction
- Future headless mode: replace `DefaultPlugins` with `MinimalPlugins` + physics when a cargo feature (e.g., `headless`) is enabled. Ensure fixed tick still advances via `app.update()` in tests.
- Add integration tests that build a minimal App, step N frames, and assert on entity state (ball moved, tick advanced).

## Safe Refactor Steps
1. Move systems into appropriately named modules without changing logic.
2. Introduce events/resources from docs one at a time; wire producer/consumer in same PR.
3. Maintain behavior parity (autoplay still works) while layering manual input features.
4. Keep `SimState.tick` monotonic and single-writer for determinism.

## Style & Conventions
- Use Bevy 0.14 APIs (no speculative future API changes).
- Group system additions by schedule: `.add_systems(FixedUpdate, (...))` etc.
- Derive `Resource`, `Component`, `Event` as needed; prefer `Default` for zero init.
- Avoid premature abstraction: only create a plugin once a feature has ≥2 systems or data types.

## Do NOT
- Introduce external asset pipelines (glTF, images) or large dependencies without necessity.
- Put gameplay-affecting logic in `Update` schedule.
- Rename documented planned types/events unless design doc updated.

## Small Example (Adding AimState Resource Skeleton)
```rust
#[derive(Resource, Default)]
pub struct AimState { pub mode: AimMode, pub power: f32 }
#[derive(Default)] pub enum AimMode { #[default] Idle, Charging }
```
Add in `main.rs` (temporarily) or a new `input.rs` module + plugin registering Update systems that mutate `AimState` and emit `SwingEvent` when releasing.

---
Questions / clarifications: open an issue or request updates to architecture docs before diverging.
