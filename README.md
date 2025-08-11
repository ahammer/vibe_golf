# Vibe Golf

A fast‑iterated experimental 3D golf sandbox built with Rust + Bevy + Rapier. Procedural terrain, vegetation, particles, HDR sky, water shader, GPU driven FX, and a light arcade scoring loop — all optimized for both native and WebAssembly (GitHub Pages deployment).

Play Now  
https://ahammer.github.io/vibe_golf/

---

## Core Features

- Procedural / heightmap hybrid terrain + custom terrain material & contour lines
- Vegetation spawning (slope + noise filtered)
- Water plane + shader
- Ball physics using Rapier3D
- Shooting mechanic with trajectory / shot indicator
- Moving target + scoring / basic game state
- Particles & GPU driven FX (impact, poofs, explosions)
- Decorative models (candy, duck, trees, etc.)
- HDR sky environment
- Performance menu (runtime toggles & diagnostics)
- Main menu + HUD
- Screenshot capture (flag-gated)
- Deterministic fixed 60 Hz simulation core (see code comments)

---

## Controls (Default)

- Mouse / Drag: Aim (camera orbit or shot direction)
- Left Click / Press: Charge & release shot
- ESC: Menu
- Gear Icon: Performance menu
- (Idle) Camera may wander for ambience

---

## Runtime Flags

- `--runtime <seconds>`  Auto-exit after duration (useful for benchmarking / CI)
- `--screenshot` Enable screenshot capture systems (otherwise disabled to reduce overhead)

Example:  
`cargo run --release -- --runtime 30 --screenshot`

---

## Build (Native)

Requirements: Rust (stable), cargo.

```
cargo run
```

Optimized:
```
cargo run --release
```

---

## Build (WebAssembly)

1. Add target:
```
rustup target add wasm32-unknown-unknown
```
2. Release build:
```
cargo build --release --target wasm32-unknown-unknown
```
3. (If rebuilding artifacts) Run `wasm-bindgen` on produced wasm (the repo already includes generated `web/` artifacts):
```
wasm-bindgen target/wasm32-unknown-unknown/release/vibe_golf.wasm -–out-dir web --no-modules --no-typescript
```
4. Serve the `web/` directory (any static file server).  
5. Assets are loaded in unprocessed mode (see `AssetPlugin` config in `main.rs`).

---

## Architecture Overview

Each gameplay / rendering concern is encapsulated as a Bevy plugin:

- CoreSimPlugin: fixed timestep / shared timing resources
- TerrainMaterialPlugin + TerrainPlugin: mesh generation + material & contour shader
- VegetationPlugin: procedural tree / prop placement
- ParticlePlugin: GPU / FX systems
- GameAudioPlugin: music + SFX events
- GameStatePlugin: scoring & shot state
- LevelPlugin: heightmap + RON level data
- BallPlugin: ball physics + integration
- TargetPlugin: moving target + hit detection
- ShootingPlugin: input → impulse & shot indicator
- HudPlugin / MainMenuPlugin / PerformanceMenuPlugin: UI layers
- CameraPlugin: follow / orbit / idle wander
- ScreenshotPlugin (conditional): manual capture

Insertion order (see `src/main.rs`) deliberately groups simulation → world gen → FX → UI.

---

## Performance Notes

Key optimizations (reflected in commit cadence):

- GPU particles replacing per-entity CPU updates
- Scale normalization & collider tuning
- Asset unprocessed mode for wasm (avoid meta fetch 404s)
- Iterative culling / draw distance adjustments after gains
- Heightmap precomputation to stabilize terrain cost

Use the performance menu (gear icon) and frame diagnostics for profiling.

---

## Development History

A full commit-by-commit narrative (116 commits) is documented in `development_history.md` including inferred intent, phases, and architectural evolution.

---

## Screenshots

See `/screenshots` for early frames. (Screenshots disabled unless `--screenshot` flag supplied.)

---

## Future Ideas (Non‑binding)

- Course sequencing / multiple targets
- Persistent high scores
- Seeded world generation / reproducibility
- Wind or dynamic difficulty modifiers
- Asset compression (KTX2 / Basis) for web payload reduction

---

## License

TBD.

---

## Contributing

Fast experimental project: open small, focused PRs (feature or perf). Maintain plugin isolation & fixed tick integrity.
