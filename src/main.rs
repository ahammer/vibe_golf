/// Migration Notes:
/// R1/P0: Fixed 60 Hz gameplay tick reinstated (see architecture docs).
/// R1/P1: Modularization – systems split into focused plugins under src/plugins/ (core_sim, level, ball, target, shooting, autoplay, hud, camera, terrain, particles, audio, vegetation, etc).
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
use bevy::asset::{AssetPlugin, AssetMode};

use vibe_golf::plugins::{
    core_sim::{CoreSimPlugin, AutoConfig},
    game_state::GameStatePlugin,
    level::LevelPlugin,
    ball::BallPlugin,
    target::TargetPlugin,
    shooting::ShootingPlugin,
    hud::HudPlugin,
    camera::CameraPlugin,
    terrain::TerrainPlugin,
    vegetation::VegetationPlugin,
    particles::ParticlePlugin,
    game_audio::GameAudioPlugin,
    terrain_material::TerrainMaterialPlugin,
    main_menu::MainMenuPlugin,
    performance_menu::PerformanceMenuPlugin,
};

use vibe_golf::screenshot::{ScreenshotPlugin, ScreenshotConfig};

fn main() {
    // Better panic messages in the browser console when running under WebAssembly.
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    let args: Vec<String> = std::env::args().collect();
    let screenshot_enabled = !args.iter().any(|a| a == "--no-screenshot");
    // Parse -runtime / --runtime flags (supports -runtime 30, --runtime 30, -runtime=30, --runtime=30)
    // Also detect whether the flag was supplied to enable auto-exit behavior.
    let mut runtime_flag: Option<f32> = None;
    for (i, a) in args.iter().enumerate() {
        if a == "-runtime" || a == "--runtime" {
            if let Some(val) = args.get(i + 1) {
                if let Ok(f) = val.parse::<f32>() { runtime_flag = Some(f); }
            }
        } else if let Some(stripped) = a.strip_prefix("-runtime=").or_else(|| a.strip_prefix("--runtime=")) {
            if let Ok(f) = stripped.parse::<f32>() { runtime_flag = Some(f); }
        }
    }
    let exit_enabled = runtime_flag.is_some();
    let runtime_seconds = runtime_flag.unwrap_or(20.0);

    // Build the app in stages to allow cfg-gated plugin insertion without illegal attributes in method chains.
    let mut app = App::new();
    app.insert_resource(AutoConfig { exit_enabled, run_duration_seconds: runtime_seconds, ..Default::default() })
        .insert_resource(ClearColor(Color::srgb(0.52, 0.80, 0.92)))
        .insert_resource(Msaa::Sample4)
        .insert_resource(AmbientLight {
            color: Color::srgb(0.55, 0.55, 0.60),
            brightness: 800.0,
        })
        .insert_resource(ScreenshotConfig::new(screenshot_enabled))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Vibe Golf".into(),
                        #[cfg(target_arch = "wasm32")]
                        canvas: Some("#bevy-canvas".into()),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    // On web we don't ship processed .meta files; use unprocessed mode to avoid 404s.
                    mode: AssetMode::Unprocessed,
                    file_path: "assets".into(),
                    ..default()
                })
        );


    app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        // Gameplay & rendering plugins (order preserved)
        .add_plugins(CoreSimPlugin)         // timing + shared resources
        .add_plugins(TerrainMaterialPlugin) // realistic terrain material (shader)
        .add_plugins(TerrainPlugin)         // procedural terrain
        .add_plugins(VegetationPlugin)      // procedural vegetation (trees)
        .add_plugins(ParticlePlugin)        // particle & FX systems
        .add_plugins(GameAudioPlugin)       // game audio (music + sfx)
        .add_plugins(GameStatePlugin)       // shot state, scoring
        .add_plugins(MainMenuPlugin)        // main menu (Play/Quit/High Score)
        .add_plugins(LevelPlugin)           // level loading & world entities
        .add_plugins(BallPlugin)            // ball physics
        .add_plugins(TargetPlugin)          // target motion + hit detection
        .add_plugins(ShootingPlugin)        // shooting input & trajectory UI
        // .add_plugins(AutoplayPlugin)     // optional automated swings
        .add_plugins(HudPlugin)             // HUD (score/time)
        .add_plugins(CameraPlugin)          // camera follow/orbit
        .add_plugins(ScreenshotPlugin)      // screenshot capture
        .add_plugins(PerformanceMenuPlugin) // realtime performance menu (gear icon)
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins(LogDiagnosticsPlugin::default())
        .run();
}
// Tests for core simulation now reside implicitly in plugin code if needed; keeping a lightweight smoke test here optional.
