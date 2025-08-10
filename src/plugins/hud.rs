use bevy::prelude::*;

use crate::plugins::core_sim::SimState;
use crate::plugins::ball::{BallKinematic, Ball};
use crate::plugins::game_state::Score;
use crate::plugins::target::Target;
use crate::plugins::camera::OrbitCameraState;

#[derive(Component)]
pub struct Hud;

#[derive(Component)]
pub struct CompassHud;

pub struct HudPlugin;
impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_hud_text, spawn_compass_text))
            .add_systems(Update, (update_hud, update_compass));
    }
}

fn spawn_hud_text(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("fonts/FiraSans-Bold.ttf");
    commands.spawn((
        TextBundle::from_section(
            "Initializing...",
            TextStyle { font: font.clone(), font_size: 22.0, color: Color::WHITE },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(8.0),
            ..default()
        }),
        Hud,
    ));

    // (Kept for clarity; compass spawns in separate system to keep responsibilities split)
}

fn spawn_compass_text(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load("fonts/FiraSans-Bold.ttf");
    commands.spawn((
        TextBundle::from_section(
            "Compass...",
            TextStyle { font, font_size: 18.0, color: Color::WHITE },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(32.0),
            ..default()
        }),
        CompassHud,
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

fn update_compass(
    score: Res<Score>,
    state: Option<Res<OrbitCameraState>>,
    q_ball_t: Query<&Transform, With<Ball>>,
    q_target_t: Query<&Transform, (With<Target>, Without<Ball>)>,
    mut q_text: Query<&mut Text, With<CompassHud>>,
) {
    if score.game_over {
        return;
    }
    let (Some(state), Ok(ball_t), Ok(target_t), Ok(mut text)) =
        (state, q_ball_t.get_single(), q_target_t.get_single(), q_text.get_single_mut()) else {
            return;
        };

    // Horizontal vector and distance
    let to_target = target_t.translation - ball_t.translation;
    let horiz = Vec3::new(to_target.x, 0.0, to_target.z);
    let dist = horiz.length();

    if dist < 0.001 {
        text.sections[0].value = "At Target".to_string();
        return;
    }

    // World bearing: angle from +X toward +Z (matches target placement logic using cos/sin).
    let bearing_world = horiz.z.atan2(horiz.x);

    // Camera yaw
    let cam_yaw = state.yaw;

    // Relative angle target vs camera forward
    let mut rel = bearing_world - cam_yaw;
    const TAU: f32 = std::f32::consts::TAU;
    while rel > std::f32::consts::PI { rel -= TAU; }
    while rel < -std::f32::consts::PI { rel += TAU; }

    // Build compass bar
    const LEN: usize = 41; // odd for center marker
    let mut chars = [' '; LEN];
    let center = LEN / 2;
    chars[center] = '|';

    let angle_to_index = |angle: f32| -> usize {
        // angle in [-PI, PI]
        let scale = (LEN as f32 - 1.0) / 2.0;
        let offs = (angle / std::f32::consts::PI) * scale;
        let idx = center as f32 + offs;
        idx.round().clamp(0.0, (LEN - 1) as f32) as usize
    };

    // Target marker
    let t_idx = angle_to_index(rel);
    chars[t_idx] = 'T';

    // Cardinal directions relative to camera
    // World bearings
    let cardinals = [
        (0.0_f32, 'E'),
        (std::f32::consts::FRAC_PI_2, 'N'),
        (std::f32::consts::PI, 'W'),
        (-std::f32::consts::FRAC_PI_2, 'S'),
    ];
    for (world_bearing, ch) in cardinals {
        let mut r = world_bearing - cam_yaw;
        while r > std::f32::consts::PI { r -= TAU; }
        while r < -std::f32::consts::PI { r += TAU; }
        let idx = angle_to_index(r);
        if chars[idx] == ' ' {
            chars[idx] = ch;
        }
    }

    let compass: String = chars.iter().collect();
    text.sections[0].value = format!("{compass}  Dist:{:.1}m", dist);
}
