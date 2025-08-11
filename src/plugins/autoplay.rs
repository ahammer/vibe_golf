use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::plugins::core_sim::{SimState, AutoConfig, AutoRuntime, LogState};
use crate::screenshot::{ScreenshotConfig, ScreenshotState};
use crate::plugins::ball::Ball;

pub struct AutoplayPlugin;
impl Plugin for AutoplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (scripted_autoplay, debug_log_each_second, exit_on_duration));
    }
}

fn scripted_autoplay(
    sim: Res<SimState>,
    mut runtime: ResMut<AutoRuntime>,
    cfg: Res<AutoConfig>,
    mut commands: Commands,
    q_ball: Query<(Entity, &Transform), With<Ball>>,
) {
    if sim.tick < runtime.next_swing_tick { return; }
    let interval_ticks = (cfg.swing_interval_seconds * 60.0) as u64;
    if let Ok((entity, transform)) = q_ball.get_single() {
        let swings_done = if runtime.next_swing_tick == 0 { 0 } else { runtime.next_swing_tick / interval_ticks.max(1) };
        let angle = (swings_done as f32 * 13.0).to_radians();
        let dir_flat = Vec3::new(angle.cos(), 0.0, angle.sin()).normalize();
        let impulse = dir_flat * cfg.base_impulse + Vec3::Y * (cfg.base_impulse * cfg.upward_factor);
        commands.entity(entity).insert(ExternalImpulse { impulse, torque_impulse: Vec3::ZERO });
        info!("AUTOPLAY swing t={:.2}s tick={} swing={} pos=({:.2},{:.2},{:.2}) impulse=({:.2},{:.2},{:.2})",
            sim.elapsed_seconds, sim.tick, swings_done,
            transform.translation.x, transform.translation.y, transform.translation.z,
            impulse.x, impulse.y, impulse.z);
    }
    runtime.next_swing_tick += interval_ticks.max(1);
}

fn debug_log_each_second(
    sim: Res<SimState>,
    mut log_state: ResMut<LogState>,
    q_ball: Query<(&Transform, &Velocity), With<Ball>>,
) {
    if sim.tick == 0 || sim.tick % 60 != 0 { return; }
    let current_second = sim.tick / 60;
    if current_second == 0 || current_second == log_state.last_logged_second { return; }
    log_state.last_logged_second = current_second;
    if let Ok((t, vel)) = q_ball.get_single() {
        info!("T+{}s tick={} ball=({:.2},{:.2},{:.2}) speed={:.2}",
            current_second, sim.tick,
            t.translation.x, t.translation.y, t.translation.z,
            vel.linvel.length());
    }
}

fn exit_on_duration(
    sim: Res<SimState>,
    cfg: Res<AutoConfig>,
    screenshot_cfg: Option<Res<ScreenshotConfig>>,
    screenshot_state: Option<Res<ScreenshotState>>,
    mut exit: EventWriter<AppExit>,
) {
    let target_ticks = (cfg.run_duration_seconds * 60.0) as u64;
    if sim.tick < target_ticks { return; }
    if let (Some(c), Some(state)) = (screenshot_cfg, screenshot_state) {
        if c.enabled && !state.last_saved { return; }
    }
    exit.send(AppExit::Success);
}
