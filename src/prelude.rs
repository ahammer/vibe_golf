//! Convenience re-exports for frequently used types & plugins.
pub use crate::plugins::core_sim::{SimState, AutoConfig, AutoRuntime, LogState, CoreSimPlugin};
pub use crate::plugins::ball::{Ball, BallKinematic, BallPlugin};
pub use crate::plugins::target::{Target, TargetPlugin, TargetParams};
pub use crate::plugins::shooting::{ShootingPlugin};
pub use crate::plugins::game_state::{GameStatePlugin, ShotState, ShotConfig, Score, ShotMode};
pub use crate::plugins::level::{LevelPlugin, LevelDef};
pub use crate::plugins::hud::{HudPlugin, Hud};
pub use crate::plugins::camera::CameraPlugin;
pub use crate::plugins::particles::ParticlePlugin;
pub use crate::plugins::autoplay::AutoplayPlugin;
pub use crate::screenshot::{ScreenshotPlugin, ScreenshotConfig, ScreenshotState};
