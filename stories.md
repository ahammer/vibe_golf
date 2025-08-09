# Core Interaction Backlog (Orbit + Aim + Shoot Only)

Purpose: Focus exclusively on the essential player interaction loop — orbiting the camera around the ball, charging a shot with the mouse, and releasing to launch the ball. All prior terrain / streaming / analytics / auxiliary backlog items are intentionally removed for clarity and execution speed.

Scope (In This Backlog):
- Camera orbit & zoom (mouse)
- Shot charging (press / hold / release)
- Applying shot impulse (direction + power + optional loft)
- Basic gating (ball rest / cooldown)
- Minimal HUD feedback (power, state, strokes)
- (Optional P1) Scoring + target hit + spin / loft refinement
- (Optional P2) Quality-of-life preview & undo

Out of Scope (Explicitly Deferred):
- Terrain streaming / LOD
- Async generation
- Advanced physics tuning
- Telemetry / persistence
- Multiplayer / networking
- Gamepad / accessibility (except placeholder story)
- Visual debug gizmos, trails
- Mulligans beyond single undo
- Trajectory accuracy beyond simple approximation

---

## Release G1 – Core Interaction Loop

### P0: Orbit Camera (Mouse Pivot)
Done

### P0: Aim State & Charging (Mouse Hold)
Done

### P0: Shot Release & Ball Impulse
Done

### P0: Cooldown & Re-Aim Gating
As a player I should not be able to spam shots while ball is obviously still rolling.
AC:
- After shot: mode=Cooldown.
- Transition Cooldown → Idle when (ball speed < rest_threshold) OR (elapsed > max_cooldown_force_release).
- HUD shows either READY, CHARGING xx%, or COOLDOWN.
Artifacts:
- Extend AimConfig { max_cooldown_force_release }

### P0: Minimal HUD Integration
As a player I want clear feedback on current aim state & strokes.
AC:
- HUD line: `Strokes: N | State: (READY | CHARGING nn% | COOLDOWN)`
- Updates only when underlying values change (avoid per-frame string churn if desired).
Artifacts:
- HUD system reading AimState & StrokeState.

### P0: Deterministic Tick Alignment
As a maintainer I want charging & cooldown timers deterministic.
AC:
- Power progression & cooldown decrement in FixedUpdate using fixed dt (e.g. 1/60).
- Visual smoothing (if any) isolated to Update layer (does not affect logical state).

---

## P1 Enhancements (Post-Loop Polishing)

### P1: Loft / Elevation Control
As a player I want to add controlled loft to my shot.
AC:
- While Charging, mouse vertical delta (or Shift + Scroll) adjusts loft within [0, loft_max_deg].
- Loft reflected in event (loft_deg) & HUD.
Artifacts:
- Extend AimState { loft_deg }
- Config: loft_max_deg

### P1: Target Hit & Simple Scoring
As a player I want a goal to shoot toward.
AC:
- Proximity or collider overlap with target emits TargetHitEvent.
- ScoreState { strokes_total, points } updated; simple formula points += max(1, base - strokes_since_last_target).
- On target hit: (Optional) Soft particle or flash feedback.
Artifacts:
- Event: TargetHitEvent
- Resource: ScoreState (merge StrokeState if desired)

### P1: Basic Shot Log (Debug)
As a developer I want to inspect recent shots for tuning.
AC:
- Keep ring buffer last N entries (power, resulting speed, loft, dir.xz, tick).
- Optional debug print `[SHOT] power=... speed=...`.

### P1: Spin / Advanced Loft (Deferred Mechanics)
AC:
- Placeholder only: extend impulse with lateral/vertical modifiers (not implemented unless needed).

---

## P2 Exploratory / Nice-To-Have

### P2: Trajectory Preview (While Charging)
As a player I want to see an approximate path before releasing.
AC:
- While Charging, show dotted arc or line segments for first T seconds (sampled at fixed intervals).
- Updates at ≤10 Hz.

### P2: Undo Last Shot (Mulligan)
As a player I want a limited rewind.
AC:
- Press R while Idle → restore pre-shot position & velocity (store snapshot each shot).
- Limited to M uses (config).

### P2: Accessibility & Gamepad Mapping
As a player using a gamepad I want parity controls.
AC:
- Right stick orbit, triggers for charge/release, bumpers for loft.

---

## Implementation Order (Lean)

1. OrbitCameraState + input transform application (P0)
2. AimState + charging logic (P0)
3. ShotEvent + impulse application + StrokeState (P0)
4. Cooldown gating (P0)
5. HUD integration (P0)
6. (Optional immediate) Deterministic tick verification tests
7. Loft control (P1)
8. Target + scoring (P1)
9. Shot log (P1)
10. Trajectory preview (P2)
11. Mulligan (P2)
12. Gamepad mapping (P2)

---

## Config Summary (Initial Defaults)

```text
OrbitCameraConfig:
  pitch_min = -10°
  pitch_max = 65°
  radius_min = 4.0
  radius_max = 18.0
  zoom_speed = 1.0
  orbit_sensitivity = (yaw=0.005, pitch=0.005)

AimConfig:
  charge_seconds = 1.2
  cooldown_seconds = 0.4
  max_cooldown_force_release = 1.5
  rest_speed_threshold = 0.05
  base_impulse = 12.0
  loft_max_deg (P1) = 25.0
  default_loft_deg (P0) = 5.0 (or 0.0 if flat)

ShotLog:
  capacity = 32 (P1)
```

---

## Ready Next (Actionable P0)
Implement OrbitCameraState + mouse input + transform update (no aim yet).
Then add AimState + charging logic + ShotEvent emission + impulse application.
