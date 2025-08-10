//! Library entry for integration tests & external tooling.
//! Exposes plugin modules and a prelude for common types.

pub mod plugins {
    pub mod core_sim;
    pub mod game_state;
    pub mod level;
    pub mod ball;
    pub mod target;
    pub mod shooting;
    pub mod autoplay;
    pub mod hud;
    pub mod camera;
    pub mod terrain;
    pub mod particles;
    pub mod game_audio;
    pub mod contour_material;
    pub mod terrain_graph;
    pub mod vegetation;
}
pub mod screenshot;
pub mod prelude;
