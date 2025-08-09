use bevy::prelude::*;
use bevy_rapier3d::prelude::Velocity;
use crate::plugins::scene::{Ball, CameraFollow};

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, camera_follow);
    }
}

fn camera_follow(
    q_ball: Query<(&Transform, Option<&Velocity>), (With<Ball>, Without<CameraFollow>)>,
    mut q_cam: Query<(&mut Transform, &CameraFollow), Without<Ball>>,
) {
    let Ok((ball_t, vel_opt)) = q_ball.get_single() else { return; };
    let Ok((mut cam_t, follow)) = q_cam.get_single_mut() else { return; };

    let forward = vel_opt
        .and_then(|v| {
            let horiz = Vec3::new(v.linvel.x, 0.0, v.linvel.z);
            if horiz.length_squared() > 0.05 { Some(horiz.normalize()) } else { None }
        })
        .unwrap_or_else(|| {
            let rel = (ball_t.translation - cam_t.translation) * Vec3::new(1.0, 0.0, 1.0);
            if rel.length_squared() > 0.01 { rel.normalize() } else { Vec3::Z }
        });

    let desired = ball_t.translation - forward * follow.distance + Vec3::Y * follow.height;
    cam_t.translation = cam_t.translation.lerp(desired, follow.lerp_factor);
    cam_t.look_at(ball_t.translation + Vec3::Y * 0.3, Vec3::Y);
}
