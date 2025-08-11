# Development History

This document reconstructs the evolution of `vibe_golf` across 116 commits (2025-08-08 → 2025-08-11).  
Source data: commit chronology (hash | date | subject).  
Because only subjects were available (no diffs in this narrative build step), deeper descriptions are *inferred* from naming patterns, subsystem emergence order, and typical Bevy / gameplay iteration workflows. Each commit is documented; adjacent thematic commits are grouped mentally into phases but still listed individually.

Legend:
- Core = simulation loop, timing, physics, state management.
- Env = terrain, vegetation, heightmaps, sky, water.
- GFX = rendering, shaders, visual polish (contours, shadows, color, particles).
- UX = menus, HUD, camera behavior, user input, shot interaction.
- Perf = optimization, scaling, asset load strategy.
- Build/Deploy = repository hygiene, docs, GitHub Pages, WASM adjustments.

---

## Phase 1: Bootstrap & Procedural Terrain Foundations (2025-08-08)

| Commit | Subject | Expanded Notes |
|--------|---------|----------------|
| d1fe791 | initial command | Repo initialized; likely added Cargo.toml skeleton + minimal Bevy app entry point. |
| 32b50d8 | built architect doc | Early architectural intent; probably drafted guiding principles (later reinforced by modular plugins). |
| 452c599 | some tracking/ball work | First gameplay artifact: a physics body or placeholder entity representing the ball; possibly Rapier integration start. |
| 9429b03 | updating designs/context/settings | Adjusted initial constants (gravity, units, camera baseline) and/or README / design notes. |
| 6ebe90a | clean up | Removed scaffolding / unused prototypes from early spike. |
| 60f305d | working kind of | First end-to-end “runs without crashing” state; minimal render loop stable. |
| 70b7200 | it runs | Confirmation of stable loop; may have introduced a simple update system. |
| 740e9f2 | wip | Transitional partial changes: maybe preparing for terrain shift. |
| 744ff01 | move to procedural generation | Pivot to runtime procedural terrain (noise-based); replaced static mesh with generated height data. |
| 6d32ae4 | shows a landscape | Visual validation of generated terrain pipeline integrated into Bevy mesh assets. |
| 516f975 | kind of running better? | Minor performance tweaks (batching, fewer allocations). |
| c4ae84d | smoother | Frame pacing improvements; possibly MSAA or framerate diagnostics enabled. |
| 045daa4 | dropping through the landscape | Identified physics vs. heightmap collision mismatch; perhaps added debugging colliders. |
| a28b83e | better ball | Refined ball collider (sphere radius sync), restitution, damping. |
| 9aa53ba | much better | Stabilized physics integration; corrected transform sync. |
| b4e0a0b | terrain now filling and ball falling | Terrain tiles / chunk fill logic confirmed; ball now interacts properly (collision shape fixed). |
| f78da0a | bigger terrain | Expanded world bounds or chunk streaming radius. |
| 663349c | improvements to gfx | Basic material tuning (albedo tweaks, light, fog, tone mapping). |
| 7cfb1c6 | clean up | Removed temporary logging / redundant systems. |

## Phase 2: Core Interaction & Sky / Gameplay Loop Emergence (2025-08-09 start)

| Commit | Subject | Expanded Notes |
| 91dfc72 | some inputs | Input mapping for aim/shoot or camera orbit initiated. |
| 430900c | much better | First responsive controls; maybe smoothing of camera follow. |
| 4c3e9f6 | looking much better | Visual polish (lighting direction, environment color). |
| 76cd454 | now with shooting | Canonical gameplay mechanic implemented: shot charge / release applying impulse to ball. |
| 77e1887 | added a skymap, basic gameplay working | Environment map / HDR sky integrated; cohesive loop (aim → shoot → observe). |
| d487763 | skymap | Additional refinement (correct mip chain or exposure). |
| 80cb729 | HDRI now in there | Validated HDR format loading (feature flags: hdr). |
| 5c6d62e | full game | Minimal ruleset (scoring / target / hole detection) probably first pass. |
| 7c01137 | improvements | Iterated on HUD or physics feel. |
| df4ff21 | particles | Introduced GPU/CPU particle system scaffolding. |
| fd6ddda | audio working | SFX/music pipeline integrated (mp3 feature in Bevy). |
| 60cf285 | more colors | Palette adjustments; maybe contour coloration seeds. |
| 3b575db | clean up | Refactor pass. |
| f0739dc | shadows back | Re-enabled shadow maps after perf tuning. |
| 2c8236e | update with walls | Added boundary colliders to keep ball in bounds. |
| 47f86ec | terrain graph system | Abstraction layer for procedural pipeline (node graph or layering noise modules). |
| 7dedbe8 | better camera | Follow/orbit smoothing, look-ahead or spring damping. |
| b50d56c | added models | First static prop GLBs added (trees, decorative). |
| 880cbad | nice, meshes work (but need scaling and placement) | Confirmed asset loading; placement heuristics pending. |
| f771171 | candy collisions have gravity | Prop physics bodies enabled. |
| 7c98d3f | meatballs and explosions | Novel FX or particle burst upon collisions. |
| ab3d6c6 | bigger ducky, no collision yet | Adjusted scale; left collision disabled intentionally (maybe collectible or passive). |
| 45a6579 | contour lines look better | Shader iteration—added banding logic or improved normal-based blending. |
| 617cce7 | more poofs | Enhanced particle spawn variety/event triggers. |
| 107a776 | great | General polish checkpoint. |
| 3da15d0 | this looks good now | Visual milestone—terrain + props + FX cohesive. |
| 0ac4aa4 | optimization | Start of systematic performance focus (see later perf burst). |
| 881d8d4 | clean up | Remove instrumentation. |
| 9b59f38 | optimized | Concrete wins (fewer entities, instancing, frustum culling). |
| 15d44dc | updates | Minor tuning. |
| 3055394 | optimizing | Further iteration before GPU particle migration. |
| 9679730 | gpu driving particles | Shift from CPU tick to GPU compute/material for particles. |
| 5778c96 | improvements to the shot indicator | UI overlay / trajectory prediction line refinement. |
| b40f5cf | walls | Additional geometry or improved boundary colliders. |
| f03de43 | better wall | Collision shape or art pass improvement. |
| 1915342 | clean up | Refactor. |
| 1abaf17 | refactor | Structural re-org (prelude or plugin separation beginnings). |
| f27cb3d | cleanup | Code hygiene after refactor. |
| 2a5d2b9 | menu | First Main Menu (Play / maybe Quit). |
| 6365d45 | clean up camera | Consolidated camera system responsibilities. |
| 208186c | bigger level | Extended playfield scale. |
| 98480f8 | opening up world | Reduced occlusion / raised clip distances. |
| 4ba3cd2 | clean up | Hygiene. |
| 3542af2 | bigger world | Another expansion / density adjustments. |
| f032bd1 | fix things up | Stabilization (crash/asset path). |
| 2529ff4 | working on landscape/trees still | Iterating spawn distribution algorithm. |
| d31b887 | right foliage again | Corrected spawn jitter / slope filtering. |

## Phase 3: Vegetation, Camera Wandering, Stability & Visual Distance (2025-08-10 early)

| Commit | Subject | Expanded Notes |
| a61ecb7 | clean up | Routine hygiene. |
| 8633e77 | cleaning | Same. |
| 1b6abb7 | better trees | Improved LOD, scale variance, or placement gradient with height. |
| 1c8be0b | better tree spawns | Added noise-based exclusion or slope threshold. |
| 2f572c6 | better wandering camera | Idle attractor / demo camera path (spline or noise-driven). |
| d2aaa3e | better | Minor quality improvements. |
| 6d438d1 | target on ground | Adjusted target vertical offset. |
| 675337b | clean up | Hygiene. |
| ca58699 | bestter spawns | Further spawn distribution smoothing (typo in subject). |
| 6303828 | less chunkky | Reduced popping (streaming prefetch or mesh LOD). |
| c2fa1fe | wow much better fps | Key perf breakthrough: batching, removal of expensive per-frame queries. |
| d6da2b8 | improved clip plane | Adjusted camera near/far for depth precision & performance. |
| 7217534 | drawing wihtout clipping | Visual frustum correct; improved culling volumes. |
| e3a5d73 | much better draw distance | Extended far plane after perf headroom gained. |
| 2a528ea | particles more reasonably placed | Adjusted spawn origins (surface normal alignment). |
| 905a210 | cleanup | Hygiene. |

## Phase 4: Documentation + Systematic Optimization Workflow

| Commit | Subject | Expanded Notes |
| b5aa180 | clear documentation/stories ... | Added narrative to guide further refactors / agent usage. |
| bb1e9ce | optimization guide | Authored `optimization_stories.md` or similar process doc. |
| 56f76ad | adding baseline measurements | Logged FPS frame budgets / entity counts for comparison. |
| bac90c3 | updating optimize guidelines | Iterated guidance with new findings. |
| 3b4a228 | perf menu | Introduced PerformanceMenuPlugin (runtime toggles, stats). |
| 08c907e | clean up/optimize | Applied insights (removed overdraw sources). |
| fb3a342 | clean up/optimization | Incremental improvements. |
| 6257d6b | clean up | Hygiene. |
| dff8a3a | tweaks | Parameter nudges (lod distances, particle counts). |
| cc32fc9 | clean up | Hygiene. |
| c0b499f | rescale | Global scale normalization (physics ↔ visuals). |
| 18895b9 | update size of things | Follow-up re-scaling individual assets. |
| 02da44c | adding a heightmap | Introduced static heightmap file (level1.png). |
| 977c04c | heightmap renamed | Naming consistency (maybe to level1). |
| fbf97b6 | using precalculated height map | Shift from fully procedural to hybrid precomputed terrain. |
| 31033cf | added ocean | Water plane or infinite grid. |
| 0cca365 | water shader, reset, spawn constraints | WGSL water material; adjusted spawn to avoid submersion. |
| 50dcc8d | clean up | Hygiene. |
| 196ea2f | better camera | Polished transitions / smoothing constants. |
| d9f8d36 | clean up | Hygiene. |
| 6a9a7fe | update vegetation spawn | Adapt vegetation to new heightmap vs noise pipeline. |
| 7e57e22 | better terrain | Material layering / normal refinement. |
| 2700bbf | lgtm | Checkpoint acceptance. |

## Phase 5: Deployment Enablement & Web (GitHub Pages)

| Commit | Subject | Expanded Notes |
| 95472de | added gh page support | Added GitHub Pages workflow; wasm build target. |
| 8f09e50 | update deploy job | Refined CI yaml - asset paths or cache. |
| 90cb185 | update template | Adjusted web/index.html or js glue to load assets. |

## Phase 6: Late Polish, Asset Loading Robustness (2025-08-11)

| Commit | Subject | Expanded Notes |
| 27fdfd0 | clean up | Hygiene. |
| c0c5e56 | update file loading | Adjust AssetPlugin config (Unprocessed mode for web). |
| 551fe60 | assets maybe work now? | Fixing path or mime issues on GH Pages. |
| 9b4048c | runs | Verified working deployment build. |
| cafd653 | stop with screenshots | Disabled auto-screenshot spam (flag-gated). |
| bf51499 | fix up level loading | Ensured RON/world definitions load before gameplay start. |
| 610f8d9 | clean up terrain shader | Consolidated WGSL includes; optimized uniforms. |
| 54aea34 | Create README.md | Initial README with play link stub. |

---

## Thematic Evolution Summary

1. Procedural Genesis → Heightmap Hybrid: Began pure noise, later augmented by precomputed heightmap for control + performance.
2. Physics & Core Loop: Early struggle aligning ball & terrain colliders; stabilized quickly; introduced shooting mechanic early.
3. Visual Stack: Progressive layering—terrain material, contour lines, sky HDRI, shadows, particles, water, vegetation.
4. Performance Culture: Explicit baseline commits, optimization guides, perf menu plugin; multiple passes on culling, scaling, GPU offload for particles.
5. World Enlargement & Streaming: Several commits boosting terrain/world size with follow-up performance and spawn adjustments.
6. UX & Menus: Core game first, then menu, HUD, performance menu; later camera polish and wandering/demo mode.
7. Deployment & WASM: Asset mode tweaks, GH Pages pipeline, path + loader stabilization.
8. Modular Architecture: Refactor into plugin-per-domain culminating in structured `main.rs` insertion order.

---

## Plugin Architecture (Inferred Final State)

- CoreSimPlugin: Fixed timestep / global resources.
- TerrainMaterialPlugin / TerrainPlugin: Mesh + shader layering.
- VegetationPlugin: Procedural spawn with ecological rules.
- ParticlePlugin: GPU-accelerated FX.
- GameAudioPlugin: SFX/music event-driven triggers.
- GameStatePlugin: Score, shots, game phases.
- LevelPlugin: Heightmap + RON entity definitions.
- BallPlugin / ShootingPlugin: Input → impulse application + trajectory UI.
- TargetPlugin: Moving target hit detection.
- HudPlugin / MainMenuPlugin / PerformanceMenuPlugin: UI surfaces.
- CameraPlugin: Follow/orbit + idle wander.
- ScreenshotPlugin (flag-gated): On-demand capture.

---

## Key Performance Strategies (Observed via Commit Cadence)

- Iterative micro-optimizations interleaved with feature growth (avoid late giant rewrite).
- Early adoption of GPU particles reduced CPU system churn.
- Scale normalization commit cluster prevented exponential physics/LOD cost.
- Asset unprocessed mode for WASM to prevent .meta fetch overhead.
- Rendering distance increases only after perf headroom reclaimed.

---

## Potential Future Directions

(Not commits—forward-looking suggestions derived from trajectory)

- Deterministic seed management for reproducible worlds.
- Save/restore of best scores & session metrics.
- Dynamic difficulty (target movement variance, wind).
- Multi-hole course sequencing.
- Web-specific asset compression (KTX2, basis).

---

All commits have been enumerated with inferred context. For exact change sets, consult `git show <hash>` as needed.
