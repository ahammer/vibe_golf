// Ball components & simple custom kinematic physics (terrain + world bounds).
use bevy::prelude::*;
use crate::plugins::terrain::TerrainSampler;
use crate::plugins::level::LevelDef;
use crate::plugins::particles::BallGroundImpactEvent;

#[derive(Component)]
pub struct Ball;

#[derive(Component)]
pub struct BallKinematic {
    pub collider_radius: f32,
    pub visual_radius: f32,
    pub vel: Vec3,
    pub angular_vel: Vec3,
}

pub struct BallPlugin;
impl Plugin for BallPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, ball_physics);
    }
}

fn ball_physics(
    mut q: Query<(&mut Transform, &mut BallKinematic), With<Ball>>,
    sampler: Res<TerrainSampler>,
    level: Option<Res<LevelDef>>,
    mut ev_impact: EventWriter<BallGroundImpactEvent>,
) {
    let Ok((mut t, mut kin)) = q.get_single_mut() else { return; };
    let dt = 1.0 / 60.0;
    let g = -9.81;

    kin.vel.y += g * dt;
    t.translation += kin.vel * dt;

    // Removed world boundary bounce (open world)

    // Terrain interaction
    let h = sampler.height(t.translation.x, t.translation.z);
    let surface_y = h + kin.collider_radius;

    if t.translation.y <= surface_y {
        t.translation.y = surface_y;

        let n = sampler.normal(t.translation.x, t.translation.z);

        let vn = kin.vel.dot(n);
        if vn < 0.0 {
            let impact_intensity = (-vn).max(0.0);
            if impact_intensity > 0.1 {
                ev_impact.send(BallGroundImpactEvent {
                    pos: t.translation,
                    intensity: impact_intensity,
                });
            }
            kin.vel -= vn * n;
        }

        let g_vec = Vec3::Y * g;
        let g_parallel = g_vec - n * g_vec.dot(n);
        kin.vel += g_parallel * dt;

        let mut tangential = kin.vel - n * kin.vel.dot(n);
        let speed = tangential.length();
        if speed > 1e-5 {
            let friction_coeff = 0.25;
            let decel = friction_coeff * -g;
            let drop = decel * dt;
            if drop >= speed {
                kin.vel -= tangential;
                tangential = Vec3::ZERO;
            } else {
                let new_speed = speed - drop;
                kin.vel += tangential.normalize() * (new_speed - speed);
                tangential = kin.vel - n * kin.vel.dot(n);
            }
        }

        // Rolling angular velocity smoothing
        let speed = tangential.length();
        if speed > 1e-5 {
            let axis = n.cross(tangential).normalize_or_zero();
            if axis.length_squared() > 0.0 {
                let desired_mag = speed / kin.visual_radius;
                let desired = axis * desired_mag;
                kin.angular_vel = if kin.angular_vel.length_squared() > 0.0 {
                    kin.angular_vel.lerp(desired, 0.35)
                } else {
                    desired
                };
            }
        } else {
            kin.angular_vel *= 0.85;
            if kin.angular_vel.length_squared() < 1e-6 {
                kin.angular_vel = Vec3::ZERO;
            }
        }
        let omega = kin.angular_vel;
        let omega_len = omega.length();
        if omega_len > 1e-6 {
            t.rotate_local(Quat::from_axis_angle(omega.normalize(), omega_len * dt));
        }
    }
}
