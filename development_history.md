# Development History (Evidence‑Based)

Reconstructed from actual Git history using:
- Ordered log (reverse chronological)
- Numstat (added/removed line counts + touched files) captured in `commit_numstat.txt`
- Current source layout (plugins in `src/plugins/`)
- Asset & pipeline artifacts (heightmaps, shaders, models, web build)

Commits span 2025-08-08 → 2025-08-11. The prior draft was inference-heavy; this revision grounds each phase in the concrete file deltas observed in numstat output (appearance / growth / refactors). Where intent is deduced it is explicitly labeled (inferred).

Legend (Tags):
- [CORE] Simulation loop, timing, main.rs structure, refactors
- [ARCH] Architectural / structural re‑organization
- [PHYS] Physics / ball / target interactions
- [TERRAIN] Terrain / heightmap / terrain graph / terrain material / contour
- [VEG] Vegetation spawning & tuning
- [FX] Particles, explosions, GPU offload
- [AUDIO] Audio assets & systems
- [UI] HUD, menus, indicators, camera behavior
- [ASSET] Models / textures / fonts / level data / RON
- [WEB] Web / WASM / deployment pipeline
- [PERF] Optimizations, scaling, measurement
- [DOC] Documentation / guides / README / stories
- [BUILD] GitHub workflow, config
- [CLEAN] Cleanups / hygiene / minor adjustments
- [WATER] Water & ocean shaders
- [SHADER] Shader improvements (terrain, contour, water)
- [SCALE] World or object scale / resizing
- [INPUT] Input & shooting integration

---

## Phase 0 – Bootstrap & Foundational Loop (d1fe791 → 744ff01) (2025-08-08)

| Commit | Summary | Concrete Evidence & Notes |
|--------|---------|---------------------------|
| d1fe791 | initial command | Added core scaffolding: `Cargo.toml`, `src/main.rs` (1580 lines added), early `README.md`, design docs (`game_design.md` 1520). Large monolithic main file (pre‑plugin). [CORE][DOC] |
| 32b50d8 | built architect doc | Added `architecture.md` (2600). Expanded `main.rs` (+4315). Architecture formalization precedes modular break-up. [ARCH][DOC] |
| 452c599 | some tracking/ball work | `src/main.rs` +10730 lines – major gameplay spike still centralized; introduced tracking/ball logic inline. [PHYS][CORE] |
| 9429b03 | updating designs/context/settings | Updated `game_design.md` (+4149) and `stories.md` (+1780). Configuration & planning focus. [DOC] |
| 6ebe90a | clean up | Reduced `main.rs` (‑192) and moved screenshot functionality into new `src/screenshot.rs` (+800). Removed provisional screenshots; `.gitignore` touched. [CLEAN][CORE][UI] |
| 60f305d | working kind of | Major `main.rs` edit (+4628) shows next consolidation; updated screenshot system (+212). Emergent stable loop claim. [CORE][CLEAN] |
| 70b7200 | it runs | Explosion of plugin systemization: new plugin files (`autoplay.rs`, `camera.rs`, `core_sim.rs`, `hud.rs`, `scene.rs`), each non‑trivial (hundreds of lines). First modular separation from monolith. [ARCH][UI][CORE] |
| 740e9f2 | wip | Introduces `lib.rs`, `prelude.rs`, test harness (`tests/fixed_tick.rs`). Adds fixed tick test → early deterministic timing guarantee. [ARCH][CORE][TEST] |
| 744ff01 | move to procedural generation | Large addition to `stories.md` (+17295) documenting procedural shift (design narrative). Sets stage for terrain system extraction. [DOC][TERRAIN] |

### Key Transitions
- Monolithic → emergent multi-plugin skeleton (scene-centric still; terrain not yet isolated).
- Early emphasis on written design & architectural discipline (unusually heavy doc volume at inception).

---

## Phase 1 – Terrain System Materialization & Visual Pipeline (6d32ae4 → 7cfb1c6) (Late 2025-08-08)

| Commit | Summary | Evidence |
|--------|---------|----------|
| 6d32ae4 | shows a landscape | Introduces `terrain.rs` (+2130) plus camera & scene tweaks: first explicit terrain mesh generation module. [TERRAIN] |
| 516f975 | kind of running better? | Terrain refinement (+1833) and core timing adjustments (`core_sim.rs`). Performance shaping begins. [PERF][TERRAIN] |
| c4ae84d | smoother | Minor tuning across `scene.rs` & `main.rs`. Likely frame pacing / camera smoothing (inferred). [UI][PERF] |
| 045daa4 | dropping through the landscape | Scene edits (+78) – likely collider or height sampling discrepancy fix (inferred). [PHYS][TERRAIN] |
| a28b83e | better ball | HUD & scene changes (shot feedback overlays). [PHYS][UI] |
| 9aa53ba | much better | Scene-only delta (+609) – ballistic feel / interpolation tweaks (inferred). [PHYS] |
| b4e0a0b | terrain now filling and ball falling | Terrain expansion (+64) – chunk filling logic; ensures gravity interaction reliability. [TERRAIN][PHYS] |
| f78da0a | bigger terrain | Large terrain growth (+5613): extended generation radius / LOD stride adjustments. [TERRAIN][SCALE] |
| 663349c | improvements to gfx | Scene visual changes; precursor to contour & material specialization. [GFX] |
| 7cfb1c6 | clean up priorities | Massive `stories.md` edit (+157203) = design backlog reprioritization driving next refactors. [DOC][ARCH] |

---

## Phase 2 – Inputs, Skymap, Core Gameplay Loop, Particles & Audio (91dfc72 → 3b575db) (Early 2025-08-09)

| Commit | Summary | Evidence |
|--------|---------|----------|
| 91dfc72 | some inputs | Heavy camera changes (+1129) & scene/HUD modifications: aim/look handling. [INPUT][UI] |
| 430900c | much better | Camera refinement; terrain minor tweak. [UI][PHYS] |
| 4c3e9f6 | looking much better | Scene polish (lighting/ambience). [GFX] |
| 76cd454 | now with shooting | Scene +21516: shooting mechanic integrated inside scene monolith (pre-shooting plugin). [PHYS][INPUT] |
| 77e1887 | added a skymap... | Adds EXR HDR sky asset (raw env map). [ASSET][GFX] |
| d487763 | skymap | Migrates sky assets into `assets/skymap/` structure. [ASSET][GFX][BUILD] |
| 80cb729 | HDRI now in there | Scene modifications for HDR sampling / IBL enablement. [GFX] |
| 5c6d62e | full game | HUD expansion (+282), scene update (+612). Score/time loop emerges. [UI][PHYS] |
| 7c01137 | improvements | Core sim adjustments, HUD tweaks → cadence polishing. [CORE][UI] |
| df4ff21 | particles | Introduces `particles.rs` (+3510). Foundational CPU particle system. [FX] |
| fd6ddda | audio working | Adds mp3 assets & `game_audio.rs` (+1250). [AUDIO][ASSET] |
| 60cf285 | more colors | Scene palette & audio integration adjustments. [GFX][AUDIO] |
| 3b575db | clean up | Shader introduction: `contour.wgsl` (960) & `contour_material.rs` (+1030), terrain updates (+6120). Separation of contour logic from terrain mesh. [SHADER][TERRAIN][GFX] |

---

## Phase 3 – Contours, Terrain Graph, Models, FX & Vegetation Genesis (f0739dc → 1abaf17) (Mid 2025-08-09)

| Commit | Summary | Evidence |
|--------|---------|----------|
| f0739dc | shadows back | Adds extended contour shader variant (`contour_ext.wgsl` +1060), terrain modifications. Restores shadow pipeline config (inferred). [SHADER][GFX] |
| 2c8236e | update with walls | Adds walls via terrain/contour material edits. [PHYS][TERRAIN] |
| 47f86ec | terrain graph system | New `terrain_graph.rs` (+2010): abstraction over noise or layering nodes. [TERRAIN][ARCH] |
| 7dedbe8 | better camera | Camera smoothing refinement. [UI] |
| b50d56c | added models | Introduces multiple GLB scene assets (trees, candy, ducky). [ASSET][GFX] |
| 880cbad | meshes work (scaling/placement) | Particles & scene large edits to integrate props spatially. [ASSET][FX][GFX] |
| f771171 | candy collisions have gravity | Particles edit: collision reaction events. [PHYS][FX] |
| 7c98d3f | meatballs and explosions | Adds explosive FX triggers; particle bursts. [FX] |
| ab3d6c6 | bigger ducky, no collision yet | Scene scaling for showcase; collision intentionally omitted. [ASSET][GFX] |
| 45a6579 | contour lines look better | Minor contour shader delta (+44). [SHADER][GFX] |
| 617cce7 | more poofs | Particle system spawn variety. [FX] |
| 107a776 | great | Introduces `vegetation.rs` (+1480). First vegetation system separate from generic models. [VEG][TERRAIN] |
| 3da15d0 | this looks good now | Vegetation massive expansion (+161117) = procedural placement logic explosion (sampling slopes, noise masks). [VEG][SCALE] |
| 0ac4aa4 | optimization | Vegetation refinement (‑11320 / +?)—pruning/compaction. [PERF][VEG] |
| 881d8d4 | clean up | Vegetation huge churn (+203139) – algorithmic restructure (likely batching / filters). [VEG][PERF] |
| 9b59f38 | optimized | Further vegetation delta (+25235) consolidating cost. [PERF][VEG] |
| 15d44dc | updates | Additional vegetation tuning. [VEG] |
| 3055394 | optimizing | More vegetation incremental improvements. [PERF][VEG] |
| 9679730 | gpu driving particles | Particles file large change (+255113). Migration to GPU logic (WGSL driven / indirect draws) (inferred). [FX][PERF][SHADER] |
| 5778c96 | improvements to the shot indicator | Scene modification introducing improved predictive arc overlay. [UI][PHYS] |
| b40f5cf | walls | Scene update adding or refining wall collider entities. [PHYS] |
| f03de43 | better wall | Further wall shaping or collision fidelity. [PHYS] |
| 1915342 | clean up | Introduces `game_state.rs` (+1940) formalizing scoring & shot phases. [CORE][UI] |
| 1abaf17 | refactor | Major modularization: breakout of `ball.rs`, `level.rs`, `shooting.rs`, `target.rs`, plugin-based architecture solidified; high line counts across new plugin files. [ARCH][PHYS][UI][CORE] |

### Architectural Milestone
Commit `1abaf17` is the decisive inversion from scene-centric sprawl into domain-aligned plugins (foundation for maintainability + later perf menu insertion order control).

---

## Phase 4 – Menus, World Scale Expansion & Vegetation / Terrain Iteration (f27cb3d → d31b887) (Late 2025-08-09)

| Commit | Summary | Evidence |
|--------|---------|----------|
| f27cb3d | cleanup | Removes residual scene usage. [CLEAN] |
| 2a5d2b9 | menu | Adds `main_menu.rs` (+1710). [UI] |
| 6365d45 | clean up camera | Large camera plugin edits (+10022). [UI] |
| 208186c | bigger level | Terrain growth (+14975). [TERRAIN][SCALE] |
| 98480f8 | opening up world | Level & terrain_graph tweaks. [TERRAIN][VEG] |
| 4ba3cd2 | clean up | Terrain & level hygiene. [CLEAN][TERRAIN] |
| 3542af2 | bigger world | Terrain expansion again. [SCALE][TERRAIN] |
| f032bd1 | fix things up | Terrain fixes (slope / hole artifacts likely). [TERRAIN] |
| 2529ff4 | working on landscape/trees still | Terrain tuning (+2112). [TERRAIN][VEG] |
| d31b887 | right foliage again | Vegetation rebalanced (+1226). [VEG][PERF] |

---

## Phase 5 – Vegetation Tuning Wave & Camera Ambience (a61ecb7 → 2f572c6 plus d2aaa3e) (Early 2025-08-10)

Sequence of rapid vegetation algorithmic oscillation and camera wander introduction:

- a61ecb7 / 8633e77: Massive vegetation churn (suggests restructuring spawn evaluation ordering).
- 1b6abb7 / 1c8be0b: “better trees / better tree spawns” with targeted vegetation diffs (LOD thresholds, slope constraints).
- 2f572c6: Wandering camera path integration (+10640). [UI][AMBIENCE]
- d2aaa3e: HUD improvements (feedback polish). [UI]

---

## Phase 6 – Target & Spawn Quality, Performance Headroom & Visibility (6d438d1 → 905a210) (Early 2025-08-10)

| Commit | Summary | Evidence |
|--------|---------|----------|
| 6d438d1 | target on ground | Adjust `level1.ron`; ensuring target vertical alignment. [PHYS][ASSET] |
| 675337b | clean up | HUD refactors. [CLEAN][UI] |
| ca58699 | bestter spawns | Adjusts target and level logic; spawn fairness improvements. [PHYS][UI] |
| 6303828 | less chunkky | Large terrain change (+231207) addressing terrain popping (pre-warming / LOD smoothing). [TERRAIN][PERF] |
| c2fa1fe | wow much better fps | Vegetation diff (+15640) reducing overhead. [PERF][VEG] |
| d6da2b8 | improved clip plane | Level tweak; near/far plane improvements (inferred). [PERF][GFX] |
| 7217534 | drawing wihtout clipping | Level adjustments finalize frustum config. [GFX][PERF] |
| e3a5d73 | much better draw distance | Terrain slight change to safely extend far plane. [GFX][PERF] |
| 2a528ea | particles more reasonably placed | Particle origin adjustments. [FX] |
| 905a210 | cleanup | Particle & debug scene edits – likely pruning instrumentation. [CLEAN][FX] |

---

## Phase 7 – Documentation & Performance Tooling Formalization (b5aa180 → 6257d6b) (Mid 2025-08-10)

| Commit | Summary | Evidence |
|--------|---------|----------|
| b5aa180 | clear documentation/stories ... | Updates multiple docs (architecture, stories). [DOC] |
| bb1e9ce | optimization guide | Adds `optimization_stories.md` (+2290). [DOC][PERF] |
| 56f76ad | adding baseline measurements | Core sim deltas, baseline metrics instrumentation (inferred). [PERF][CORE] |
| bac90c3 | updating optimize guidelines | Expands perf doc (+4522). [DOC][PERF] |
| 3b4a228 | perf menu | Introduces `performance_menu.rs` (+4930) & terrain adjustments. Runtime introspection UI. [PERF][UI] |
| 08c907e | clean up/optimize | Core sim + vegetation + terrain tuning. [PERF][CLEAN] |
| fb3a342 | clean up/optimization | Terrain optimizations (+8923). [PERF][TERRAIN] |
| 6257d6b | clean up | Terrain hygiene. [CLEAN] |

---

## Phase 8 – Continued Perf / Rescale & Transition to Hybrid Heightmap (dff8a3a → 31033cf) (Later 2025-08-10)

| Commit | Summary | Evidence |
|--------|---------|----------|
| dff8a3a | tweaks | Vegetation tuning (+1515). [VEG][PERF] |
| cc32fc9 | clean up | Core sim & level adjustments. [CLEAN][CORE] |
| c0b499f | rescale | Removes/adjusts multiple model assets — re-normalizing world scale. [SCALE][ASSET] |
| 18895b9 | update size of things | Vegetation adjusts to new scale (+1112). [SCALE][VEG] |
| 02da44c | adding a heightmap | Adds `assets/heightmaps/heightmap.png`. [TERRAIN][ASSET] |
| 977c04c | heightmap renamed | Renames to `level1.png` (signals stable baseline heightmap). [TERRAIN] |
| fbf97b6 | using precalculated height map | Large terrain change (+11481) switching generation pipeline to sampled source. [TERRAIN][PERF] |
| 31033cf | added ocean | Terrain extension with water boundary integration. [WATER][TERRAIN] |

---

## Phase 9 – Water Shader, Spawn Constraints, Camera Polishing & Terrain Material Upgrade (0cca365 → 2700bbf) (End 2025-08-10)

| Commit | Summary | Evidence |
|--------|---------|----------|
| 0cca365 | water shader, reset, spawn constraints | Adds `water.wgsl` (+950), modifies ball, level, target, vegetation; introduces spawn avoidance of submerged zones. [WATER][SHADER][TERRAIN][PHYS] |
| 50dcc8d | clean up | Level adjustments post-water. [CLEAN] |
| 196ea2f | better camera | Large camera changes (+9218) – smoothing / wander / aim interplay. [UI] |
| d9f8d36 | clean up | Camera hygiene (+4135). [CLEAN][UI] |
| 6a9a7fe | update vegetation spawn | Vegetation placement updated (+11639) to respect water & heightmap constraints. [VEG][TERRAIN] |
| 7e57e22 | better terrain | Adds `terrain_pbr_ext.wgsl` (+1780) & `terrain_material.rs` (+980). Material specialization & extended PBR layering; main & prelude updates. [SHADER][TERRAIN][GFX] |
| 2700bbf | lgtm | Minor shader tuning. Finalization checkpoint of terrain material stack. [SHADER][TERRAIN] |

---

## Phase 10 – Web Deployment & Asset Mirroring (95472de → 90cb185) (Late 2025-08-10)

| Commit | Summary | Evidence |
|--------|---------|----------|
| 95472de | added gh page support | Adds GitHub workflow, `.cargo/config.toml`, full `web/` asset mirror (audio, models, shaders, wasm bundle). [WEB][BUILD][ASSET] |
| 8f09e50 | update deploy job | Workflow refinement. [WEB][BUILD] |
| 90cb185 | update template | Adjusts `web/index.html` for asset or canvas changes. [WEB][UI] |

---

## Phase 11 – Final Polish, Asset Loading, Shader Cleanup & Documentation (27fdfd0 → e9023f4) (2025-08-11)

| Commit | Summary | Evidence |
|--------|---------|----------|
| 27fdfd0 | clean up | Multi-plugin hygiene (main menu, particles, terrain material). [CLEAN][ARCH] |
| c0c5e56 | update file loading | Terrain file handling tweak (probable switch to Bevy unprocessed asset mode). [BUILD][ASSET] |
| 551fe60 | assets maybe work now? | `main.rs` large edit (+2912) – toggling AssetPlugin config to fix web load paths. [WEB][ASSET][CORE] |
| 9b4048c | runs | Confirmation checkpoint; minor main diff. [CLEAN] |
| cafd653 | stop with screenshots | Disables automatic screenshot system (flag gating). [UI][PERF] |
| bf51499 | fix up level loading | Adjusts `level.rs` (+2510) improving spawn ordering / dependency timing. [TERRAIN][LEVEL] |
| 610f8d9 | clean up terrain shader | Significant `terrain_pbr_ext.wgsl` changes (+3269) & material plugin. Shader consolidation / uniform optimization. [SHADER][TERRAIN][PERF] |
| 54aea34 | Create README.md | Adds minimal README placeholder. [DOC] |
| e9023f4 | updates to docs | Expanded README (+1461), added dev history artifacts, generating historical narrative. [DOC][BUILD] |

---

## Major Feature Introduction Timeline

| Feature | First Commit | Maturation Commits |
|---------|--------------|--------------------|
| Procedural Terrain | 6d32ae4 | fbf97b6 (hybrid heightmap), 7e57e22 (material) |
| Contour Shading | 3b575db | f0739dc, 45a6579 |
| Terrain Graph | 47f86ec | Subsequent terrain scaling commits |
| Ball & Shooting | Early monolith (452c599, 76cd454) | 1abaf17 (modular split) |
| Particles (CPU) | df4ff21 | 9679730 (GPU offload) |
| Vegetation | 107a776 | 3da15d0 (massive), optimization wave (0ac4aa4 → 3055394), water integration (6a9a7fe) |
| Audio | fd6ddda | Minor color & scene adjustments later |
| Menus (Main) | 2a5d2b9 | Performance menu (3b4a228) |
| Performance Menu | 3b4a228 | Ongoing terrain/veg perf passes |
| Heightmap Hybrid | 02da44c, fbf97b6 | Water & spawn constraints (0cca365, 6a9a7fe) |
| Water Shader & Ocean | 31033cf (ocean), 0cca365 (water.wgsl) | Terrain material synergy (7e57e22) |
| Terrain PBR Material | 7e57e22 | Cleanup (610f8d9) |
| Web Deployment | 95472de | Asset load fixes (551fe60) |
| Shader Optimization | 610f8d9 | Future-ready baseline |

---

## Performance Strategy Evolution (Grounded)

1. Early Monolith Reduction: Large refactor (1abaf17) isolates systems → lowers system scheduling contention.  
2. Vegetation Algorithm Waves: Sequences of enormous vegetation file churn show iterative pruning of per-frame logic (0ac4aa4 → 3055394, a61ecb7 cluster).  
3. GPU Particle Migration: Single high-impact commit 9679730 relocates updates off CPU ECS iteration path.  
4. Terrain Scaling & Popping Mitigation: Large terrain diffs (6303828) preceding draw distance increases (e3a5d73) indicate preemptive streaming stabilization before visibility expansion.  
5. Hybrid Heightmap: Precomputed sampling (fbf97b6) reduces procedural noise cost at runtime.  
6. Shader Consolidation: Terrain material upgrades (7e57e22, 610f8d9) align with reduced uniform complexity and batched layer logic (inferred via file size + naming).  
7. Asset Unprocessed Mode (551fe60 cluster): Eliminates 404 fetch overhead on web for `.meta` artifacts.

---

## Architectural Principles (Derived)

- Domain-centric Plugins: Each subsystem (terrain, vegetation, particles, audio, UI layers) independently bootstrapped & ordered in `main.rs` for deterministic resource availability.
- Progressive Decomposition: High-risk features built monolithically first, then modularized once stable (shooting, scoring, vegetation).
- Document-Driven Refactors: Surges in `stories.md` / `optimization_stories.md` coincide with structural changes & performance passes.

---

## Potential Gaps / Future Hardening (Observations)

- Deterministic Seeds: Lock procedural + heightmap variation for reproducible perf benchmarking.
- Asset Streaming / LOD: Vegetation & terrain could benefit from explicit streaming layer (observed manual scaling commits).
- Shader Parameter Reduction: Water + terrain could unify atmospheric scattering constants.
- Benchmark Harness: Baseline measurement commit exists; could externalize into criterion or headless timing run.
- Save / Score Persistence: High score file introduced once – expand to structured persistence (RON/JSON).
- Web Payload Optimization: Compress HDR/EXR or adopt KTX2/Basis; prune duplicate asset copies where feasible.

---

## Appendix A – Commit Classification Table (Abbreviated)

A machine-grade exhaustive per-commit code footprint is preserved in `commit_numstat.txt`. This history file surfaces higher-order semantics; for diffs run:
```
git show <hash>
git diff <prev>..<hash>
```

| Commit | Tags |
|--------|------|
| d1fe791 | CORE DOC |
| 32b50d8 | ARCH DOC |
| 452c599 | CORE PHYS |
| 9429b03 | DOC |
| 6ebe90a | CLEAN CORE |
| 60f305d | CORE |
| 70b7200 | ARCH CORE UI |
| 740e9f2 | ARCH CORE TEST |
| 744ff01 | DOC TERRAIN |
| 6d32ae4 | TERRAIN |
| 516f975 | TERRAIN PERF |
| c4ae84d | UI PERF |
| 045daa4 | PHYS TERRAIN |
| a28b83e | PHYS UI |
| 9aa53ba | PHYS |
| b4e0a0b | TERRAIN PHYS |
| f78da0a | TERRAIN SCALE |
| 663349c | GFX |
| 7cfb1c6 | DOC ARCH |
| 91dfc72 | INPUT UI |
| 430900c | UI |
| 4c3e9f6 | GFX |
| 76cd454 | PHYS INPUT |
| 77e1887 | ASSET GFX |
| d487763 | ASSET BUILD |
| 80cb729 | GFX |
| 5c6d62e | UI PHYS |
| 7c01137 | CORE UI |
| df4ff21 | FX |
| fd6ddda | AUDIO ASSET |
| 60cf285 | GFX AUDIO |
| 3b575db | SHADER TERRAIN |
| f0739dc | SHADER GFX |
| 2c8236e | TERRAIN PHYS |
| 47f86ec | TERRAIN ARCH |
| 7dedbe8 | UI |
| b50d56c | ASSET |
| 880cbad | ASSET FX |
| f771171 | FX PHYS |
| 7c98d3f | FX |
| ab3d6c6 | ASSET |
| 45a6579 | SHADER |
| 617cce7 | FX |
| 107a776 | VEG |
| 3da15d0 | VEG SCALE |
| 0ac4aa4 | VEG PERF |
| 881d8d4 | VEG PERF |
| 9b59f38 | VEG PERF |
| 15d44dc | VEG |
| 3055394 | VEG PERF |
| 9679730 | FX PERF |
| 5778c96 | UI PHYS |
| b40f5cf | PHYS |
| f03de43 | PHYS |
| 1915342 | CORE UI |
| 1abaf17 | ARCH PHYS UI |
| f27cb3d | CLEAN |
| 2a5d2b9 | UI |
| 6365d45 | UI ARCH |
| 208186c | TERRAIN SCALE |
| 98480f8 | TERRAIN |
| 4ba3cd2 | CLEAN TERRAIN |
| 3542af2 | TERRAIN SCALE |
| f032bd1 | TERRAIN |
| 2529ff4 | TERRAIN VEG |
| d31b887 | VEG |
| a61ecb7 | VEG PERF |
| 8633e77 | VEG CORE |
| 1b6abb7 | VEG |
| 1c8be0b | VEG |
| 2f572c6 | UI |
| d2aaa3e | UI |
| 6d438d1 | PHYS LEVEL |
| 675337b | UI CLEAN |
| ca58699 | PHYS UI |
| 6303828 | TERRAIN PERF |
| c2fa1fe | VEG PERF |
| d6da2b8 | GFX PERF |
| 7217534 | GFX PERF |
| e3a5d73 | GFX PERF |
| 2a528ea | FX |
| 905a210 | FX CLEAN |
| b5aa180 | DOC |
| bb1e9ce | DOC PERF |
| 56f76ad | PERF CORE |
| bac90c3 | DOC PERF |
| 3b4a228 | PERF UI |
| 08c907e | PERF CLEAN |
| fb3a342 | PERF TERRAIN |
| 6257d6b | CLEAN |
| dff8a3a | VEG PERF |
| cc32fc9 | CLEAN CORE |
| c0b499f | SCALE ASSET |
| 18895b9 | SCALE VEG |
| 02da44c | TERRAIN ASSET |
| 977c04c | TERRAIN |
| fbf97b6 | TERRAIN PERF |
| 31033cf | WATER TERRAIN |
| 0cca365 | WATER SHADER |
| 50dcc8d | CLEAN |
| 196ea2f | UI |
| d9f8d36 | UI CLEAN |
| 6a9a7fe | VEG TERRAIN |
| 7e57e22 | TERRAIN SHADER |
| 2700bbf | SHADER TERRAIN |
| 95472de | WEB BUILD |
| 8f09e50 | WEB BUILD |
| 90cb185 | WEB UI |
| 27fdfd0 | CLEAN ARCH |
| c0c5e56 | BUILD ASSET |
| 551fe60 | WEB ASSET |
| 9b4048c | CLEAN |
| cafd653 | UI PERF |
| bf51499 | LEVEL TERRAIN |
| 610f8d9 | SHADER TERRAIN PERF |
| 54aea34 | DOC |
| e9023f4 | DOC BUILD |

---

## Closing

This history emphasizes **actual file change evidence** (numstat) before inference. For further analysis:
```
git blame <file>
git show <hash>:<path>
```

The repository now reflects a modular, performance-aware, deployment-capable experimental golf sandbox with a clear cadence from exploratory monolith → structured plugin ecosystem → optimization & web distribution.
