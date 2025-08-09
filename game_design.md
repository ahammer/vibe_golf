# Vibe Golf — Bevy (Rust) Design (code-only)

Goal: build a rudimentary 3D golf sandbox with minimal glue, fixed-tick simulation, decoupled rendering, and only code-generated assets (no DCC or external scene files). Designed to be easy to automate/test.

## Tech stack
- Rust + Bevy 0.14 (ECS, renderer, UI, assets, schedules)
- Rapier 3D via `bevy_rapier3d` 0.26 (rigid bodies, colliders, contact events)
- Code-only meshes: `Sphere`, `Cuboid` (no glTF)
- Minimal single font in `assets/fonts/` for on-screen text

Why Bevy: pure code workflow, fixed timestep support, first-class ECS, easy CI/headless, good Windows support.

## High-level requirements
- Landscape (flat for M0), dynamic ball, fixed target cube, HUD text
- Fixed simulation tick (e.g., 60 Hz) with renderer decoupled from sim
- Prefer built-ins and code-only assets; minimal setup
- Headless-friendly for automation

## Architecture

### Schedules
- FixedUpdate (60 Hz): deterministic game logic and physics-authoring actions
- Update (per frame): presentation-only work (HUD text, camera smoothing)

Configure: `app.insert_resource(Time::<Fixed>::from_hz(60.0));`

### Entities & components
- Ground: `RigidBody::Fixed`, `Collider::cuboid(w,h,d)`, PBR material (green)
- Ball: `RigidBody::Dynamic`, `Collider::ball(r)`, `Restitution`, `Damping`, marker `Ball`
- Target: `RigidBody::Fixed`, `Collider::cuboid(0.5)`, optionally a sensor child for goal detection, marker `Target`
- Camera: `Camera3d`, optional `CameraFollow` marker
- HUD: UI `TextBundle`, marker `Hud`

### Resources
```rust
#[derive(Resource, Default)]
struct SimState {
	tick: u64,
	strokes: u32,
	rng_seed: u64, // for deterministic randomness if needed
}
```

### Events
- `HitEvent { dir: Vec3, power: f32 }` — describes a swing impulse to apply to the ball (emitted by input, consumed in FixedUpdate)

### Systems
- Startup:
	- `setup_scene` — camera, light, ground, ball, target; meshes from `Sphere`/`Cuboid`
	- `setup_ui` — HUD text with a bundled font
- FixedUpdate (simulation):
	- `tick_state` — increment `SimState.tick`
	- `apply_hits` — convert `HitEvent` into `ExternalImpulse` on ball
	- `win_check` — detect overlap with target sensor; update state
- Update (render/UI):
	- `update_hud` — show tick, ball speed, strokes
	- `camera_follow` — optional smooth follow of ball

Physics is provided by Rapier plugin; we author bodies/colliders and read `Velocity`/events.

## Rendering & assets (code-only)
- Meshes: `Mesh::from(Sphere { radius })` and `Mesh::from(Cuboid::from_size(Vec3))`
- Materials: simple colors via `StandardMaterial` (PBR)
- Lighting: one directional light
- Text: `TextBundle` (needs a `.ttf` in `assets/fonts/`)

## Input (minimal first)
- M0: auto-nudge with a small impulse on the first fixed tick
- M1: keyboard (space to “hit”), then mouse drag to set direction/power

## Determinism & timing
- Use `FixedUpdate` at 60 Hz for game logic; avoid side effects in `Update`
- Seed all randomness via `rng_seed` if any is needed
- Rapier determinism: keep gravity and solver default; minimize frame-dependent logic by authoring forces in FixedUpdate only

## Headless/CI mode
- Windowed app for local dev using `DefaultPlugins`
- Headless for automation: switch to `MinimalPlugins`, skip `WindowPlugin`/renderer, keep `RapierPhysicsPlugin`
- Example idea (feature flag `headless`):
```rust
#[cfg(not(feature = "headless"))]
app.add_plugins(DefaultPlugins);
#[cfg(feature = "headless")]
app.add_plugins(MinimalPlugins);
```
- In tests, step fixed time manually and assert on ball position/velocity

## Minimal code skeleton
```rust
fn main() {
	App::new()
		.insert_resource(Time::<Fixed>::from_hz(60.0))
		.insert_resource(ClearColor(Color::srgb(0.52, 0.80, 0.92)))
		.insert_resource(SimState::default())
		.add_plugins(DefaultPlugins)
		.add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
		// .add_plugins(RapierDebugRenderPlugin::default())
		.add_event::<HitEvent>()
		.add_systems(Startup, (setup_scene, setup_ui))
		.add_systems(FixedUpdate, (tick_state, apply_hits, win_check))
		.add_systems(Update, (update_hud,))
		.run();
}
```

## Tests (examples)
- Unit: `SimState` increments
- Integration: build an `App`, spawn ball/target, simulate N fixed ticks, assert the ball moves/hits zone

Pseudo-test pattern:
```rust
#[test]
fn sim_advances_in_fixed_steps() {
	let mut app = App::new();
	app.insert_resource(Time::<Fixed>::from_hz(60.0));
	// add minimal plugins and systems, then:
	for _ in 0..120 { // 2 seconds @ 60 Hz
		app.update(); // will advance Fixed schedule
	}
	// assert on state/queries
}
```

## Project layout
```
src/
	main.rs            // app bootstrap
	sim.rs             // resources, events, fixed systems
	scene.rs           // setup_scene(), meshes/materials
	hud.rs             // text UI systems
assets/
	fonts/YourFont.ttf
```

## Milestones
- M0: Flat ground, ball, target cube, HUD text, auto-nudge; fixed tick + decoupled render
- M1: Keyboard swing; simple win condition (ball in target sensor)
- M2: Mouse drag aiming/power, stroke counter & reset
- M3: Simple height variation (procedural bumps) and camera follow
- M4: Headless test that validates reaching the target within N strokes

## Definition of done (M0)
- Runs on Windows; shows ground, ball, cube, HUD
- FixedUpdate at 60 Hz; Update for HUD; physics responds to a single initial impulse
- No external assets except one font; meshes are code-generated
- Project compiles with `cargo build`, runs with `cargo run`, tests pass (`cargo test`)

## Notes
- Keep gameplay effects (forces/impulses) in FixedUpdate for stability
- Prefer sensors + events for goal detection to avoid rigid contact corner cases
- For CI, prefer headless feature and pure integration tests stepping FixedUpdate

