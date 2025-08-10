use bevy::prelude::*;

use crate::plugins::core_sim::SimState;
use crate::plugins::ball::BallKinematic;
use crate::plugins::game_state::Score;

#[derive(Component)]
pub struct Hud;

pub struct HudPlugin;
impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud_text)
            .add_systems(Update, update_hud);
    }
}

fn spawn_hud_text(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("fonts/FiraSans-Bold.ttf");
    commands.spawn((
        TextBundle::from_section(
            "Initializing...",
            TextStyle { font, font_size: 22.0, color: Color::WHITE },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(8.0),
            ..default()
        }),
        Hud,
    ));
}

fn update_hud(
    sim: Res<SimState>,
    score: Res<Score>,
    q_ball: Query<&BallKinematic>,
    mut q_text: Query<&mut Text, With<Hud>>,
) {
    if let (Ok(kin), Ok(mut text)) = (q_ball.get_single(), q_text.get_single_mut()) {
        let speed = kin.vel.length();
        if score.game_over {
            let avg_time = score.final_time / score.hits.max(1) as f32;
            let avg_shots = score.shots as f32 / score.hits.max(1) as f32;
            let best = score.high_score_time.map(|v| format!("{:.2}s", v)).unwrap_or_else(|| "--".to_string());
            text.sections[0].value = format!(
                "GAME OVER | Time: {:.2}s | Best: {best} | Holes: {} | Shots: {} | Avg T/H: {:.2}s | Avg S/H: {:.2} | Press R",
                score.final_time,
                score.hits,
                score.shots,
                avg_time,
                avg_shots,
            );
        } else {
            let current_hole = score.hits + 1;
            let avg_time = if score.hits > 0 { sim.elapsed_seconds / score.hits as f32 } else { 0.0 };
            let avg_shots = if score.hits > 0 { score.shots as f32 / score.hits as f32 } else { 0.0 };
            text.sections[0].value = format!(
                "Time: {:.2}s | Speed: {:.2} m/s | Hole: {}/{} | Shots: {} | Avg T/H: {:.2}s | Avg S/H: {:.2}",
                sim.elapsed_seconds,
                speed,
                current_hole,
                score.max_holes,
                score.shots,
                avg_time,
                avg_shots,
            );
        }
    }
}
