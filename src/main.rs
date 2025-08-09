// Migration Notes:
// R1/P0: Fixed 60 Hz gameplay tick reinstated (see architecture docs).
// R1/P1: Modularization – systems split into plugins under src/plugins/ (core_sim, scene, autoplay, hud, camera).

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};

pub mod plugins {
    pub mod core_sim;
    pub mod scene;
    pub mod autoplay;
    pub mod hud;
    pub mod camera;
    pub mod terrain;
    pub mod particles;
    pub mod game_audio;
    pub mod contour_material;
}
use plugins::core_sim::CoreSimPlugin;
use plugins::scene::ScenePlugin;
use plugins::hud::HudPlugin;
use plugins::camera::CameraPlugin;
use plugins::terrain::TerrainPlugin;
use plugins::particles::ParticlePlugin;
use plugins::game_audio::GameAudioPlugin;
use plugins::contour_material::ContourMaterialPlugin;

mod screenshot;
use screenshot::{ScreenshotPlugin, ScreenshotConfig};

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
        .add_plugins(CoreSimPlugin)      // timing + shared resources
        .add_plugins(ContourMaterialPlugin) // custom contour material (shader)
        .add_plugins(TerrainPlugin)      // procedural terrain
        .add_plugins(ParticlePlugin)     // particle & FX systems (register events before scene systems use them)
        .add_plugins(GameAudioPlugin)    // game audio (music + sfx)
        .add_plugins(ScenePlugin)        // world & entities
        // .add_plugins(AutoplayPlugin)     // disabled: no impulses, simple vertical drop test
        .add_plugins(HudPlugin)          // HUD update
        .add_plugins(CameraPlugin)       // camera follow
        .add_plugins(ScreenshotPlugin)   // screenshot capture
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins(LogDiagnosticsPlugin::default())
        .run();
}
// Tests for core simulation now reside implicitly in plugin code if needed; keeping a lightweight smoke test here optional.
