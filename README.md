# Vibe Golf (Bevy + Rapier)

Minimal 3D golf sandbox with:
- Fixed-tick simulation (60 Hz) via `Time::<Fixed>`
- Decoupled rendering
- Ground, ball (dynamic), target cube (fixed)
- On-screen text with tick & ball speed

## Prereqs
- Rust toolchain (Windows): install via winget or rustup

## Setup
1. Add a font for UI text:
   - Create `assets/fonts/`
   - Download a font (e.g., Fira Sans Bold) into `assets/fonts/FiraSans-Bold.ttf`

## Run
```powershell
cargo run
```

## Test
```powershell
cargo test
```

## Notes
- Toggle Rapier debug lines by enabling `RapierDebugRenderPlugin` in `main.rs`.
- Adjust fixed tick Hz in `Time::<Fixed>::from_hz(…)`.
- Renderer runs freely and is decoupled from the fixed simulation.
