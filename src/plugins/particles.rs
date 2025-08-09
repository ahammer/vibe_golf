// Particle & FX systems for atmosphere dust, impact dust clouds, target explosions, and game-over confetti.
use bevy::prelude::*;
use bevy::math::primitives::{Sphere, Cuboid};
use rand::prelude::*;

pub struct ParticlePlugin;

// Events emitted by gameplay code
#[derive(Event)]
pub struct BallGroundImpactEvent {
    pub pos: Vec3,
    pub intensity: f32, // impact speed or magnitude
}

#[derive(Event)]
pub struct TargetHitEvent {
    pub pos: Vec3,
}

#[derive(Event)]
pub struct GameOverEvent {
    pub pos: Vec3,
}

#[derive(Event)]
pub struct ShotFiredEvent {
    pub pos: Vec3,
    pub power: f32,
}

// Internal particle variants
#[derive(Component)]
enum ParticleKind {
    DustAtmos,      // persistent atmospheric dust (recycled)
    DustBurst,      // short dust puff on ground impact
    Explosion,      // bright fast particles
    Confetti,       // falling colorful squares
}

#[derive(Component)]
struct Particle {
    lifetime: f32,
    age: f32,
    fade: bool,
    gravity: f32,
    vel: Vec3,
    angular_vel: Vec3,
}

#[derive(Resource)]
struct AtmosDustConfig {
    count: usize,
    half_extent: f32,
    min_y: f32,
    max_y: f32,
    rise_speed: f32,
}
impl Default for AtmosDustConfig {
    fn default() -> Self {
        Self {
            count: 220,
            half_extent: 140.0,
            min_y: 1.0,
            max_y: 24.0,
            rise_speed: 0.15,
        }
    }
}

#[derive(Resource)]
pub struct ParticleMaterials {
    dust: Handle<StandardMaterial>,
    dust_burst: Handle<StandardMaterial>,
    explosion: Handle<StandardMaterial>,
    confetti_colors: Vec<Handle<StandardMaterial>>,
}

impl FromWorld for ParticleMaterials {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let dust = materials.add(StandardMaterial {
            base_color: Color::srgba(0.85, 0.80, 0.70, 0.20),
            perceptual_roughness: 1.0,
            metallic: 0.0,
            ..default()
        });
        let dust_burst = materials.add(StandardMaterial {
            base_color: Color::srgba(0.90, 0.85, 0.75, 0.55),
            perceptual_roughness: 1.0,
            ..default()
        });
        let explosion = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.65, 0.2, 0.85),
            emissive: LinearRgba::new(4.0, 2.0, 0.6, 1.0),
            ..default()
        });
        let confetti_palette = [
            Color::srgba(0.95, 0.2, 0.2, 1.0),
            Color::srgba(0.2, 0.95, 0.3, 1.0),
            Color::srgba(0.2, 0.4, 0.95, 1.0),
            Color::srgba(0.95, 0.9, 0.2, 1.0),
            Color::srgba(0.9, 0.2, 0.85, 1.0),
        ];
        let mut confetti_colors = Vec::new();
        for c in confetti_palette {
            confetti_colors.push(materials.add(StandardMaterial {
                base_color: c,
                double_sided: true,
                ..default()
            }));
        }
        Self { dust, dust_burst, explosion, confetti_colors }
    }
}

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AtmosDustConfig::default())
            .init_resource::<ParticleMaterials>()
            .add_event::<BallGroundImpactEvent>()
            .add_event::<TargetHitEvent>()
            .add_event::<GameOverEvent>()
            .add_event::<ShotFiredEvent>()
            .add_systems(Startup, setup_atmospheric_dust)
            .add_systems(Update, (
                recycle_atmospheric_dust,
                spawn_dust_on_impact,
                spawn_explosion_on_hit,
                spawn_confetti_on_game_over,
                update_particles,
            ));
    }
}

// -------- Atmospheric Dust (persistent) --------
fn setup_atmospheric_dust(
    mut commands: Commands,
    cfg: Res<AtmosDustConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<ParticleMaterials>,
) {
    let mut rng = StdRng::from_entropy();
    // Small sphere mesh reused
    let dust_mesh = meshes.add(Mesh::from(Sphere { radius: 0.18 }));
    for _ in 0..cfg.count {
        let x = rng.gen_range(-cfg.half_extent..=cfg.half_extent);
        let y = rng.gen_range(cfg.min_y..=cfg.max_y);
        let z = rng.gen_range(-cfg.half_extent..=cfg.half_extent);
        commands.spawn((
            PbrBundle {
                mesh: dust_mesh.clone(),
                material: mats.dust.clone(),
                transform: Transform::from_translation(Vec3::new(x, y, z)),
                ..default()
            },
            ParticleKind::DustAtmos,
        ));
    }
}

 
// We need a dedicated query; re-implement with filtering.
fn recycle_atmospheric_dust(
    mut q: Query<&mut Transform, (With<ParticleKind>, Without<Particle>)>,
    cfg: Res<AtmosDustConfig>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds();
    for mut t in &mut q {
        t.translation.y += cfg.rise_speed * dt;
        if t.translation.y > cfg.max_y {
            t.translation.y = cfg.min_y;
        }
    }
}

// -------- Impact Dust --------
fn spawn_dust_on_impact(
    mut ev: EventReader<BallGroundImpactEvent>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<ParticleMaterials>,
) {
    let mesh = meshes.add(Mesh::from(Sphere { radius: 0.15 }));
    for e in ev.read() {
        if e.intensity < 1.0 { continue; }
        let count = (6.0 + e.intensity * 4.0).clamp(6.0, 40.0) as usize;
        let mut rng = thread_rng();
        for _ in 0..count {
            // random outward direction (hemisphere)
            let dir = {
                let mut d;
                loop {
                    d = Vec3::new(rng.gen_range(-1.0..1.0), rng.gen_range(0.0..1.0), rng.gen_range(-1.0..1.0));
                    if d.length_squared() > 0.01 { break; }
                }
                d.normalize()
            };
            let speed = rng.gen_range(0.5..2.5) * (0.4 + e.intensity * 0.6);
            commands.spawn((
                PbrBundle {
                    mesh: mesh.clone(),
                    material: mats.dust_burst.clone(),
                    transform: Transform::from_translation(e.pos + Vec3::Y * 0.05),
                    ..default()
                },
                ParticleKind::DustBurst,
                Particle {
                    lifetime: rng.gen_range(0.6..1.2),
                    age: 0.0,
                    fade: true,
                    gravity: -2.5,
                    vel: dir * speed,
                    angular_vel: Vec3::new(
                        rng.gen_range(-2.0..2.0),
                        rng.gen_range(-2.0..2.0),
                        rng.gen_range(-2.0..2.0),
                    ),
                },
            ));
        }
    }
}

// -------- Target Explosion --------
fn spawn_explosion_on_hit(
    mut ev: EventReader<TargetHitEvent>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<ParticleMaterials>,
) {
    let mesh = meshes.add(Mesh::from(Sphere { radius: 0.25 }));
    for e in ev.read() {
        let mut rng = thread_rng();
        let count = 60;
        for _ in 0..count {
            let dir = {
                let mut d;
                loop {
                    d = Vec3::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0));
                    if d.length_squared() > 0.05 { break; }
                }
                d.normalize()
            };
            let speed = rng.gen_range(5.0..14.0);
            commands.spawn((
                PbrBundle {
                    mesh: mesh.clone(),
                    material: mats.explosion.clone(),
                    transform: Transform::from_translation(e.pos),
                    ..default()
                },
                ParticleKind::Explosion,
                Particle {
                    lifetime: rng.gen_range(0.5..1.0),
                    age: 0.0,
                    fade: true,
                    gravity: -9.0,
                    vel: dir * speed,
                    angular_vel: Vec3::new(
                        rng.gen_range(-6.0..6.0),
                        rng.gen_range(-6.0..6.0),
                        rng.gen_range(-6.0..6.0),
                    ),
                },
            ));
        }
    }
}

// -------- Game Over Confetti --------
fn spawn_confetti_on_game_over(
    mut ev: EventReader<GameOverEvent>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<ParticleMaterials>,
) {
    let mesh = meshes.add(Mesh::from(Cuboid::from_size(Vec3::splat(0.12))));
    for e in ev.read() {
        let mut rng = thread_rng();
        let count = 400;
        for _ in 0..count {
            let color_mat = mats.confetti_colors.choose(&mut rng).unwrap().clone();
            let pos = e.pos + Vec3::new(
                rng.gen_range(-8.0..8.0),
                rng.gen_range(4.0..12.0),
                rng.gen_range(-8.0..8.0),
            );
            let vel = Vec3::new(
                rng.gen_range(-2.5..2.5),
                rng.gen_range(0.5..3.0),
                rng.gen_range(-2.5..2.5),
            );
            commands.spawn((
                PbrBundle {
                    mesh: mesh.clone(),
                    material: color_mat,
                    transform: Transform::from_translation(pos)
                        .with_rotation(Quat::from_euler(
                            EulerRot::XYZ,
                            rng.gen_range(0.0..std::f32::consts::TAU),
                            rng.gen_range(0.0..std::f32::consts::TAU),
                            rng.gen_range(0.0..std::f32::consts::TAU),
                        )),
                    ..default()
                },
                ParticleKind::Confetti,
                Particle {
                    lifetime: rng.gen_range(3.5..6.0),
                    age: 0.0,
                    fade: true,
                    gravity: -6.0,
                    vel,
                    angular_vel: Vec3::new(
                        rng.gen_range(-3.0..3.0),
                        rng.gen_range(-3.0..3.0),
                        rng.gen_range(-3.0..3.0),
                    ),
                },
            ));
        }
    }
}

// -------- Particle Update --------
fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Particle, &Handle<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_seconds();
    for (e, mut t, mut p, mat_handle) in &mut q {
        p.age += dt;
        let rdt = dt;
        // Integrate motion
        p.vel.y += p.gravity * rdt;
        t.translation += p.vel * rdt;
        // Simple angular rotation
        let ang = p.angular_vel * rdt;
        if ang.length_squared() > 0.0 {
            let qrot = Quat::from_euler(EulerRot::XYZ, ang.x, ang.y, ang.z);
            t.rotate_local(qrot);
        }
        if p.age >= p.lifetime {
            commands.entity(e).despawn_recursive();
            continue;
        }
        if p.fade {
            let remain = 1.0 - (p.age / p.lifetime);
            if let Some(mat) = materials.get_mut(mat_handle) {
                let mut c = mat.base_color.to_linear();
                c.set_alpha(remain);
                mat.base_color = c.into();
            }
        }
    }
}
