use bevy::prelude::*;
use bevy_rapier3d::prelude::Velocity;

use crate::plugins::core_sim::SimState;
use crate::plugins::scene::Hud;

pub struct HudPlugin;
impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_hud);
    }
}

fn update_hud(
    sim: Res<SimState>,
    q_vel: Query<&Velocity>,
    mut q_text: Query<&mut Text, With<Hud>>,
) {
    if let (Ok(vel), Ok(mut text)) = (q_vel.get_single(), q_text.get_single_mut()) {
        let speed = vel.linvel.length();
        text.sections[0].value = format!("Tick: {} (t={:.2}s) | Speed: {:.2} m/s", sim.tick, sim.elapsed_seconds, speed);
    }
}
