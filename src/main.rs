//// Migration Notes:
//// R1/P0: Fixed 60 Hz gameplay tick reinstated (see architecture docs).
//// R1/P1: Modularization – systems split into focused plugins under src/plugins/ (core_sim, level, ball, target, shooting, autoplay, hud, camera, terrain, particles, audio, vegetation, etc).

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};

use vibe_golf::plugins::{
    core_sim::CoreSimPlugin,
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
    contour_material::ContourMaterialPlugin,
    main_menu::MainMenuPlugin,
};

use vibe_golf::screenshot::{ScreenshotPlugin, ScreenshotConfig};

fn main() {
    let screenshot_enabled = !std::env::args().any(|a| a == "--no-screenshot");
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.52, 0.80, 0.92)))
        .insert_resource(Msaa::Sample4)
        .insert_resource(AmbientLight {
            color: Color::srgb(0.55, 0.55, 0.60),
            brightness: 800.0,
        })
        .insert_resource(ScreenshotConfig::new(screenshot_enabled))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Vibe Golf".into(), ..default() }),
            ..default()
        }))
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        // Gameplay & rendering plugins (order preserved)
        .add_plugins(CoreSimPlugin)         // timing + shared resources
        .add_plugins(ContourMaterialPlugin) // custom contour material (shader)
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
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins(LogDiagnosticsPlugin::default())
        .run();
}
// Tests for core simulation now reside implicitly in plugin code if needed; keeping a lightweight smoke test here optional.
