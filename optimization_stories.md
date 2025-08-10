# Rendering & Performance Optimization Stories (Vibe Golf)

Scope: Reduce CPU frame time, GPU frame time, draw calls, and memory bandwidth while preserving perceived visual quality.

## 0. Baseline & Instrumentation (Status: Initial Pass Implemented)

Implemented:
- Runtime limiter: `-runtime <seconds>` (or `--runtime=<seconds>`) sets `AutoConfig.run_duration_seconds`; app auto-exits via `exit_after_runtime` system (logs `EXIT runtime reached seconds=X`).
- Headless-ish capture control: `--no-screenshot` disables first/last frame screenshot overhead.
- Frame diagnostics: `FrameTimeDiagnosticsPlugin` + `LogDiagnosticsPlugin` already emit per‑second FPS, frame_time.
- Vegetation draw call & build logs present (used for baseline counts).
- Deterministic fixed-tick (60 Hz) simulation timing.

Pending / Nice-to-have instrumentation:
- (Optional) `RenderDiagnosticsPlugin` for per-pass stats (decide after first optimization wave; adds some overhead).
- Feature flag `perf_profile` to gate extra verbose logs (draw call debug frequency throttling).
- Automated metric collector script (loop `cargo run --release -- -runtime N --no-screenshot`, parse stdout into CSV).
- Memory (RSS) sampling (external: `Get-Process vibe_golf | Select-Object WorkingSet64` on Windows, integrate into script).
- 1% low calculation: script to parse frame_time logs -> compute distribution.

Baseline Scenario Script (to automate next):
1. Launch with `-runtime 40 --no-screenshot`.
2. In input automation (future): stay idle until vegetation fully builds (~ first 20s), then trigger outward camera/character move (placeholder manual step currently).
3. (Interim) For first pass, rely on natural build-out; capture logs until exit.

Metrics to record (spreadsheet / CSV):
- FPS avg, median, 1% low (derived from frame_time logs)
- CPU frame time (from diagnostics; currently same source)
- Estimated GPU frame time (need RenderDiagnostics or external capture later)
- Entity counts snapshot (add future system to log once at t=runtime-0.5s)
- Vegetation visible trees & approx unique batches (already logged periodically)
- Terrain chunk count (add logging hook)
- Process RSS (external sample near end)
- Worst frame_time spike (max logged)

Immediate next instrumentation tasks (before first optimization changes):
- Add one-time end-of-run summary system printing: total_chunks, visible_trees, last fps avg (reduces parsing complexity).
- Add optional `--log-final-stats` flag or reuse existing runtime path (low complexity).
- (Optional) Stub script (PowerShell / Rust) to loop runs and append CSV.

After these, lock baseline snapshot and proceed to OPT-01..03.

Repeat deterministic scenario (future when input automation added):
1. Main menu 5s.
2. Idle 10s.
3. Outward expansion 30s.
4. Shot sequence (5 shots) final seconds.

Current limitation: No automated input yet; treat vegetation build + idle as provisional baseline. Update once input scripting exists.

## 1. Current Observations (from code review)

Terrain:
- Chunked generation async; good.
- Resolution 96 over 160m chunk → ~9k verts / chunk. View radius 8 ⇒ up to (2*8+1)^2 = 289 chunks worst-case if fully filled (likely not all resident simultaneously), potentially ~2.6M verts visible (heavy).
- Per‑vertex baked color attribute (adds bandwidth). Contour shading duplicated both as baked vertex colors and extended material features (possible redundancy).
- Heightfield collider per chunk (OK). Could aggregate far chunks or disable physics on distant ones.

Vegetation:
- Two modes: scene spawning vs (mesh, material) instanced-style (shared handles) via `use_instanced`.
- Approx draw call estimator present.
- Distance culling config exists but disabled (`enable_distance: false`); only shadow LOD active.
- Adaptive tuner adjusts culling radii & shadow thresholds but culling disabled so limited effect.
- Spatial hashing + progressive spawn: good.

Materials / Shading:
- ExtendedMaterial with contour extension per terrain chunk (unique material instance modifies min/max). This prevents batching across chunks (each chunk unique material → draw call per chunk).
- Trees share materials (good) when instanced mode active.

Physics:
- Rapier heightfields; no obvious bottleneck review here.

## 2. High-Level Strategy

Phase 1 (Low risk, high ROI):
- Enable & tune distance culling for vegetation.
- Reduce terrain overdraw / chunk count on screen.
- Share terrain material data via texture/atlas or global palette uniform to batch draws.
- Profile actual hotspot (validate assumptions).

Phase 2 (Medium):
- Implement real GPU instancing for trees (per-instance transform buffer) instead of separate entities (retain logical entity minimal).
- Terrain LOD (coarser mesh for far rings).

Phase 3 (Advanced / Stretch):
- Impostor (billboard or card) LOD for very far trees.
- GPU-driven indirect draw (future Bevy versions / custom pipeline).
- Async streaming unload for far vegetation & terrain to reduce memory.

## 3. Optimization Stories

Story IDs format: OPT-XX (ordered by priority & ease vs impact).

### OPT-01 Enable Vegetation Distance Culling
- Problem: All trees remain visible regardless of distance (config disabled), inflating visible batches.
- Action: Set `enable_distance = true`; tune `max_distance` to align with fog / camera horizon; start ~220 and rely on tuner.
- Acceptance: Visible tree count and approx unique batches drop ≥30% when player at origin; no obvious pop (hysteresis smooth).
- Impact: Fewer vertex submissions & fragment work.

### OPT-02 Lower Vegetation Shadow Radius Defaults
- Problem: Many distant trees still cast shadows (expensive).
- Action: Reduce `default_shadow_on` to ~80, `default_shadow_off` to ~110; widen hysteresis slightly.
- Acceptance: GPU frame time reduces ≥5% in outdoor scene; visual difference minimal.
- Impact: Shadow map fill & draw call reduction.

### OPT-03 Terrain View Radius Reduction + Adaptive
- Problem: `view_radius_chunks = 8` (160m chunk ⇒ 2560m span) very high.
- Action: Test radius 5–6; add simple distance-based hide (despawn or set `Visibility::Hidden`) for beyond radius; optional dynamic: increase during high FPS.
- Acceptance: Total resident terrain chunks cut ≥40% with negligible horizon pop.
- Impact: Fewer draw calls (materials are unique) and memory.

### OPT-04 Terrain Material Batching via Shared Uniform Texture
- Problem: Each chunk creates a unique ExtendedMaterial (different min/max). Prevents batching.
- Action: Move min/max height→ compute globally (or quantize to ring bands). Use single material per LOD ring; pass chunk min/max via vertex (pack into UV2) or instance-like buffer (requires custom pipeline) OR approximate using world y.
- Acceptance: Terrain draw calls reduce near linearly with number of combined chunks (target 4–8 calls).
- Impact: Major draw call reduction; small color precision loss acceptable.

### OPT-05 Remove Baked Vertex Colors (Compute in Shader)
- Problem: Per-vertex color attribute increases vertex bandwidth; color also derived from height & slope in code (duplicated).
- Action: Eliminate CPU-side color baking; in WGSL compute palette & contour lines using `position.y` and normal.
- Acceptance: Visual parity within small ΔE; GPU vertex fetch bytes per vertex reduced (drop COLOR attribute).
- Impact: Memory bandwidth, faster mesh build (skip color generation loop).

### OPT-06 Terrain LOD Rings
- Problem: High-res mesh wasted for far chunks.
- Action: Maintain 2–3 LOD mesh resolutions (e.g. near: 96, mid: 48, far: 24). Replace far chunk meshes asynchronously.
- Acceptance: Total vertex count reduced ≥50% while aliasing minimal.
- Impact: Vertex shading cost reduction.

### OPT-07 Vegetation True Instancing Buffer
- Problem: Current "instanced" still spawns full `PbrBundle` entities (one draw per unique (mesh,material,shadow)).
- Action: Build per-variant transform + scale + tilt buffer; custom shader reads instance data; single `draw_instanced` call per variant & shadow state split.
- Acceptance: Tree draw calls become (#variants * shadowStateCount) (e.g. 4–8) vs hundreds; FPS increase recorded.
- Impact: Large CPU & driver overhead reduction.

### OPT-08 Vegetation Far LOD Billboards
- Problem: Distant trees still render full geometry.
- Action: For distance > X replace with cross-plane billboard or single quad with normal map / baked lighting tint; group by atlas.
- Acceptance: Geometry count beyond 150m reduces by ≥80%; color/shape acceptable.
- Impact: Vertex + fragment cost reduction.

### OPT-09 Async Vegetation Unload Outside Expanded Area
- Problem: Spawn state grows without reclaim; memory pressure.
- Action: Track far zones; despawn trees beyond (half_extent + margin). Re-spawn if player returns (cache seeds for determinism).
- Acceptance: Memory plateau stable in long outward travel test.
- Impact: Memory footprint control.

### OPT-10 Parallel Terrain Normal & Color (if colors kept) Calculation
- Problem: Normals computed sequentially per chunk.
- Action: Parallelize sampling across inner loop using rayon or chunk task subdividing; or reuse gradient already used for per-vertex normal.
- Acceptance: Chunk build time reduced ≥25%.
- Impact: Faster streaming fill.

### OPT-11 Heightfield Collider Simplification for Distant Chunks
- Problem: Physics heightfields for chunks far from player rarely used.
- Action: Skip collider creation for chunks > physics_radius (e.g. 3 chunks) or create simplified (lower resolution).
- Acceptance: Rapier broadphase entities reduced; no gameplay issues (ball never reaches missing colliders).
- Impact: Physics CPU reduction.

### OPT-12 Frame Budgeted Screenshot / Diagnostics Toggle
- Problem: Diagnostics logging may spur small stutters.
- Action: Gate heavy logging (draw call debug) behind feature or runtime flag and lower frequency adaptively when FPS low.
- Acceptance: No logging-induced spikes <=1ms.
- Impact: Minor but stabilizes frame pacing.

### OPT-13 GPU Profiling & WGPU Capture Script
- Problem: Need repeatable GPU timing capture.
- Action: Add run script with `RUST_LOG=wgpu_core=warn` and environment `WGPU_TRACE=traces/traceN` for targeted 5s capture.
- Acceptance: Trace file produced; can be replayed for regression.
- Impact: Enables trustworthy measurement.

## 4. Suggested Priority & Rollout Plan

Phase A (Immediate):
1. OPT-01, OPT-02, OPT-03 (radius experiments)
2. OPT-05 (simplify terrain vertex format)
3. Rebaseline metrics

Phase B (High impact next):
4. OPT-04 (shared material) OR partially unify by quantizing min/max height to global constants
5. OPT-06 (terrain LOD)
6. OPT-07 (true instancing)

Phase C (Advanced / Visual):
7. OPT-08 (billboards)
8. OPT-11 (collider simplification)
9. OPT-09 (unload)
10. OPT-10 (parallel generation)

Phase D (Polish / Tooling):
11. OPT-12
12. OPT-13

## 5. Measurement Acceptance Template

For each story record:
- Date / Commit hash
- Scenario metrics before / after (table)
- Δ% improvement
- Screenshots / GIF (if visual)
- Notes / regressions risk

Example metric table:

| Metric | Before | After | Δ% |
|--------|--------|-------|----|
| FPS avg | 140 | 158 | +12.9 |
| GPU ms | 6.2 | 5.5 | -11.3 |
| Draw calls (terrain) | 120 | 18 | -85.0 |
| Draw calls (trees est) | 340 | 95 | -72.1 |
| Entities (trees) | 5200 | 5200 | 0 |
| VRAM (MB est) | 870 | 820 | -5.7 |

## 6. Implementation Sketches (Selected)

Terrain color in shader (replace baked vertex color):
- Remove color attribute insertion.
- In WGSL, implement palette lookup using (world_y - global_min) / (global_max - global_min).
- Compute contour line factor via fractional height; darken accordingly.

Shared material (min/max removed):
- Use global resource updated with overall observed min/max (scan once).
- Or quantize heights by known design range (e.g. 0..40m).

Vegetation instancing:
- Maintain per variant: Vec<InstanceData> (pos, rot_quat, scale, maybe packed tilt).
- Upload to storage buffer each frame (or only when dirty).
- Single entity with custom material referencing buffer + instance count.
- Remove individual `PbrBundle` tree entities or convert them into lightweight logical ECS entries (no Render components).

LOD rings:
- On finalize chunk task: decide target LOD based on chunk distance from player at that moment; schedule progressive upgrade/downgrade tasks.

## 7. Risk & Mitigations

| Area | Risk | Mitigation |
|------|------|------------|
| Removing baked colors | Visual mismatch | Side-by-side screenshot diff; tweak shader constants |
| Shared terrain material | Loss of local contrast | Add small procedural AO term from height variance |
| Instancing trees | Losing per-tree shadow LOD | Keep per-instance flag bitmask buffer or distance-based branch in shader |
| Terrain LOD swaps | Popping | Cross-fade or geometric hysteresis radius |

## 8. Stretch Ideas

- Temporal foliage animation (vertex shader wind) at no extra draw calls.
- GPU occlusion culling (Hi-Z) for dense vegetation screens.
- Clustered or forward+ lighting if adopting many dynamic lights later.
- Bindless material atlas for future props.

## 9. Summary

Immediate focus: enable existing culling / reduce material uniqueness / remove redundant vertex data. Then tackle structural reductions (LOD, instancing). Each change validated with repeatable scenario and logged metrics to ensure cumulative gains.
