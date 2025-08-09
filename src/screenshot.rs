use std::path::Path;
use std::fs;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::render::view::screenshot::ScreenshotManager;

use crate::plugins::core_sim::{SimState, AutoConfig};

#[derive(Resource)]
pub struct ScreenshotConfig {
    pub enabled: bool,
    pub first_frame_path: String,
    pub last_frame_path: String,
    pub legacy_last_run_path: String, // kept for backwards compatibility
}
impl ScreenshotConfig {
    pub fn new(enabled: bool) -> Self { Self { enabled, first_frame_path: "screenshots/first_frame.png".into(), last_frame_path: "screenshots/last_frame.png".into(), legacy_last_run_path: "screenshots/last_run.png".into() } }
}

#[derive(Resource, Default)]
pub struct ScreenshotState {
    pub first_requested: bool,
    pub first_saved: bool,
    pub last_requested: bool,
    pub last_saved: bool,
}

pub struct ScreenshotPlugin;

impl Plugin for ScreenshotPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenshotState>()
            .add_systems(Startup, cleanup_previous_screenshots)
            .add_systems(Update, capture_screenshot);
    }
}

fn cleanup_previous_screenshots(cfg: Option<Res<ScreenshotConfig>>) {
    let Some(cfg) = cfg else { return; };
    if !cfg.enabled { return; };
    if let Some(dir) = Path::new(&cfg.first_frame_path).parent() {
        if let Ok(entries) = fs::read_dir(dir) {
            let mut removed = 0u32;
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() { if !ft.is_file() { continue; } }
                let path = entry.path();
                if let Some(ext) = path.extension() { if ext == "png" { if fs::remove_file(&path).is_ok() { removed += 1; } } }
            }
            if removed > 0 { info!("SCREENSHOT cleanup removed={}", removed); }
        }
        // ensure directory exists
        let _ = fs::create_dir_all(dir);
    }
}

fn capture_screenshot(
    sim: Res<SimState>,
    auto: Res<AutoConfig>,
    cfg: Option<Res<ScreenshotConfig>>,
    mut state: ResMut<ScreenshotState>,
    mut screenshot_manager: ResMut<ScreenshotManager>,
    q_window: Query<(Entity, &Window), With<PrimaryWindow>>,
) {
    let Some(cfg) = cfg else { return; }; // config not inserted yet
    if !cfg.enabled { return; }

    // Ensure directory exists once
    if !(state.first_requested || state.last_requested) {
        if let Some(parent) = Path::new(&cfg.first_frame_path).parent() {
            if let Err(e) = fs::create_dir_all(parent) { warn!("SCREENSHOT dir create failed error={}", e); }
        }
    }

    // Capture first frame (after at least one fixed tick so initial render occurred)
    if sim.tick >= 1 && !state.first_requested {
        if let Ok((window_entity, _)) = q_window.get_single() {
            let _ = screenshot_manager.save_screenshot_to_disk(window_entity, cfg.first_frame_path.clone());
            state.first_requested = true;
        }
    }
    if state.first_requested && !state.first_saved {
        if let Ok(meta) = fs::metadata(&cfg.first_frame_path) { if meta.len() > 0 { state.first_saved = true; info!("SCREENSHOT first_frame path={}", cfg.first_frame_path); } }
    }

    // Capture last frame after run duration reached
    if sim.elapsed_seconds >= auto.run_duration_seconds && !state.last_requested {
        if let Ok((window_entity, _)) = q_window.get_single() {
            let _ = screenshot_manager.save_screenshot_to_disk(window_entity, cfg.last_frame_path.clone());
            state.last_requested = true;
        }
    }
    if state.last_requested && !state.last_saved {
        if let Ok(meta) = fs::metadata(&cfg.last_frame_path) { if meta.len() > 0 { state.last_saved = true; 
            // Copy / replace legacy path for tooling expecting last_run.png
            let _ = fs::copy(&cfg.last_frame_path, &cfg.legacy_last_run_path);
            if let Ok((_entity, w)) = q_window.get_single() { info!("SCREENSHOT last_frame path={} size={}x{}", cfg.last_frame_path, w.physical_width(), w.physical_height()); } else { info!("SCREENSHOT last_frame path={}", cfg.last_frame_path); }
        }}
    }
}
