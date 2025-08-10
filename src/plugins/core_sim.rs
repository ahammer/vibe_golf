use bevy::prelude::*;
use bevy::app::AppExit;
use bevy::time::Fixed;
use bevy_rapier3d::prelude::{Velocity, RigidBody};
use crate::plugins::game_state::Score;

// Core simulation timing & shared gameplay configuration/types.
#[derive(Resource, Default, Debug)]
pub struct SimState {
    pub tick: u64,
    pub elapsed_seconds: f32,
}
impl SimState {
    pub fn advance_fixed(&mut self) {
        self.tick += 1;
        self.elapsed_seconds = self.tick as f32 / 60.0;
    }
}

#[derive(Resource)]
pub struct AutoConfig {
    pub run_duration_seconds: f32,
    pub swing_interval_seconds: f32,
    pub base_impulse: f32,
    pub upward_factor: f32,
}
impl Default for AutoConfig {
    fn default() -> Self {
        Self { run_duration_seconds: 20.0, swing_interval_seconds: 3.0, base_impulse: 6.0, upward_factor: 0.0 }
    }
}

#[derive(Resource, Default)]
pub struct AutoRuntime { pub next_swing_tick: u64 }
#[derive(Resource, Default)]
pub struct LogState { pub last_logged_second: u64 }

#[derive(Resource, Default)]
pub struct ExitState { pub triggered: bool }

pub struct CoreSimPlugin;
impl Plugin for CoreSimPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SimState::default())
            .init_resource::<AutoConfig>() // respect pre-inserted AutoConfig (e.g. from -runtime flag)
            .insert_resource(AutoRuntime::default())
            .insert_resource(LogState::default())
            .insert_resource(ExitState::default())
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(FixedUpdate, tick_state)
            .add_systems(Update, apply_custom_gravity)
            .add_systems(Update, exit_after_runtime);
    }
}

fn tick_state(mut sim: ResMut<SimState>, score: Option<Res<Score>>) {
    if let Some(score) = score {
        if score.game_over {
            return; // freeze simulation timing after game over
        }
    }
    sim.advance_fixed();
}

fn apply_custom_gravity(mut q: Query<(&RigidBody, &mut Velocity)>) {
    // Manual gravity because default Rapier gravity appears absent.
    let dt = 1.0 / 60.0;
    let g = -9.81;
    for (rb, mut vel) in q.iter_mut() {
        if matches!(*rb, RigidBody::Dynamic) {
            vel.linvel.y += g * dt;
        }
    }
}

fn exit_after_runtime(
    sim: Res<SimState>,
    auto: Res<AutoConfig>,
    mut exit_state: ResMut<ExitState>,
    mut ev_exit: EventWriter<AppExit>,
) {
    if exit_state.triggered { return; }
    if sim.elapsed_seconds >= auto.run_duration_seconds {
        info!("EXIT runtime reached seconds={}", sim.elapsed_seconds);
        exit_state.triggered = true;
        ev_exit.send(AppExit::Success);
    }
}


pub use AutoConfig as AutoConfigExport;
pub use SimState as SimStateExport;
