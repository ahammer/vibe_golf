//! Convenience re-exports for frequently used types & plugins.
//!
//! Grouped roughly by layer. Importing `prelude::*` in integration tests or
//! auxiliary binaries simplifies access to core game types/plugins.
//!
//! Note: Keep this lean; avoid dumping every internal type here – prefer the
//! most commonly used building blocks.

/// Core simulation / timing
pub use crate::plugins::core_sim::{SimState, AutoConfig, AutoRuntime, LogState, CoreSimPlugin};

/// Gameplay domain types
pub use crate::plugins::ball::{Ball, BallKinematic, BallPlugin};
pub use crate::plugins::target::{Target, TargetPlugin, TargetParams};
pub use crate::plugins::shooting::ShootingPlugin;
pub use crate::plugins::game_state::{GameStatePlugin, ShotState, ShotConfig, Score, ShotMode};
pub use crate::plugins::level::{LevelPlugin, LevelDef};

/// World / environment
pub use crate::plugins::terrain::{TerrainPlugin, TerrainSampler, TerrainConfig};
pub use crate::plugins::vegetation::{
    VegetationPlugin, VegetationConfig, VegetationCullingConfig, VegetationLodConfig,
};
pub use crate::plugins::contour_material::ContourMaterialPlugin;
pub use crate::plugins::terrain_material::TerrainMaterialPlugin;

/// Presentation / UX
pub use crate::plugins::hud::{HudPlugin, Hud};
pub use crate::plugins::camera::CameraPlugin;
pub use crate::plugins::particles::ParticlePlugin;
pub use crate::plugins::game_audio::GameAudioPlugin;
pub use crate::plugins::main_menu::MainMenuPlugin;

/// Optional utilities
pub use crate::plugins::autoplay::AutoplayPlugin;
pub use crate::screenshot::{ScreenshotPlugin, ScreenshotConfig, ScreenshotState};
