//! Convenience re-exports for frequently used types & plugins.
pub use crate::plugins::core_sim::{SimState, AutoConfig, AutoRuntime, LogState, CoreSimPlugin};
pub use crate::plugins::scene::{Ball, Hud, CameraFollow, ScenePlugin};
pub use crate::plugins::autoplay::AutoplayPlugin;
pub use crate::plugins::hud::HudPlugin;
pub use crate::plugins::camera::CameraPlugin;
pub use crate::screenshot::{ScreenshotPlugin, ScreenshotConfig, ScreenshotState};
