// Migration Notes:
// R1/P0: Fixed 60 Hz gameplay tick reinstated (see architecture docs).
// R1/P1: Modularization – systems split into plugins under src/plugins/ (core_sim, scene, autoplay, hud, camera).

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

pub mod plugins {
    pub mod core_sim;
    pub mod scene;
    pub mod autoplay;
    pub mod hud;
    pub mod camera;
    pub mod terrain;
}
use plugins::core_sim::CoreSimPlugin;
use plugins::scene::ScenePlugin;
use plugins::autoplay::AutoplayPlugin;
use plugins::hud::HudPlugin;
use plugins::camera::CameraPlugin;
use plugins::terrain::TerrainPlugin;

mod screenshot;
use screenshot::{ScreenshotPlugin, ScreenshotConfig};

fn main() {
    let screenshot_enabled = !std::env::args().any(|a| a == "--no-screenshot");
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.52, 0.80, 0.92)))
        .insert_resource(ScreenshotConfig::new(screenshot_enabled))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Vibe Golf".into(), ..default() }),
            ..default()
        }))
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(RapierDebugRenderPlugin::default())
        .add_plugins(CoreSimPlugin)      // timing + shared resources
        .add_plugins(TerrainPlugin)      // procedural terrain
        .add_plugins(ScenePlugin)        // world & entities
        .add_plugins(AutoplayPlugin)     // scripted swings & telemetry
        .add_plugins(HudPlugin)          // HUD update
        .add_plugins(CameraPlugin)       // camera follow
        .add_plugins(ScreenshotPlugin)   // screenshot capture
        .run();
}
// Tests for core simulation now reside implicitly in plugin code if needed; keeping a lightweight smoke test here optional.
