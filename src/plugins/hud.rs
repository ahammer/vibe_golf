use bevy::prelude::*;

use crate::plugins::core_sim::SimState;
use crate::plugins::scene::{Hud, BallKinematic};

pub struct HudPlugin;
impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_hud);
    }
}

fn update_hud(
    sim: Res<SimState>,
    q_ball: Query<&BallKinematic>,
    mut q_text: Query<&mut Text, With<Hud>>,
) {
    if let (Ok(kin), Ok(mut text)) = (q_ball.get_single(), q_text.get_single_mut()) {
        let speed = kin.vel.length();
        text.sections[0].value = format!("Tick: {} (t={:.2}s) | Speed: {:.2} m/s", sim.tick, sim.elapsed_seconds, speed);
    }
}
