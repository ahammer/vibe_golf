// Particle & FX systems now using candy_1 / candy_2 glb models for burst/explosion/confetti effects.
use bevy::prelude::*;
use bevy::math::primitives::Sphere;
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

// Minimum impact intensity required to spawn bounce dust & play bounce SFX.
pub const BOUNCE_EFFECT_INTENSITY_MIN: f32 = 2.0;

// Internal particle variants
#[derive(Component)]
enum ParticleKind {
    DustAtmos,      // persistent atmospheric dust (recycled primitive spheres)
    DustBurst,      // short dust puff on ground impact (candy models now)
    ShotBlast,      // burst when player launches the ball
    Explosion,      // bright fast particles (target hit)
    Confetti,       // game-over candy rain (candy models)
}

#[derive(Component)]
struct Particle {
    lifetime: f32,
    age: f32,
    gravity: f32,
    vel: Vec3,
    angular_vel: Vec3,
    start_scale: Vec3,
    end_scale: Vec3,
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
    dust: Handle<StandardMaterial>, // only used for atmospheric dust now
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
        Self { dust }
    }
}

 // Snowflake model handle for sky particles
#[derive(Resource)]
pub struct SnowflakeModel {
    handle: Handle<Scene>,
}
impl FromWorld for SnowflakeModel {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            handle: assets.load("models/snowflake.glb#Scene0"),
        }
    }
}

// Candy model handles
#[derive(Resource)]
pub struct CandyModels {
    candy: [Handle<Scene>; 2],
}
impl FromWorld for CandyModels {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            candy: [
                assets.load("models/candy_1.glb#Scene0"),
                assets.load("models/candy_2.glb#Scene0"),
            ],
        }
    }
}

// GPU instancing variants extracted from candy scenes
#[derive(Resource, Default)]
struct CandyMeshVariants {
    ready: bool,
    variants: Vec<(Handle<Mesh>, Handle<StandardMaterial>)>,
}

#[derive(Component)]
struct CandyTemplate;

fn spawn_candy_templates(mut commands: Commands, candy: Res<CandyModels>) {
    for (i, handle) in candy.candy.iter().enumerate() {
        commands.spawn((
            SceneBundle {
                scene: handle.clone(),
                visibility: Visibility::Hidden,
                ..default()
            },
            CandyTemplate,
            Name::new(format!("CandyTemplate{}", i)),
        ));
    }
}

fn extract_candy_variants(
    mut commands: Commands,
    mut variants: ResMut<CandyMeshVariants>,
    q_templates: Query<Entity, With<CandyTemplate>>,
    q_children: Query<&Children>,
    q_mesh_mats: Query<(&Handle<Mesh>, &Handle<StandardMaterial>)>,
) {
    if variants.ready {
        return;
    }
    fn visit(
        e: Entity,
        q_children: &Query<&Children>,
        q_mesh_mats: &Query<(&Handle<Mesh>, &Handle<StandardMaterial>)>,
        out: &mut Vec<(Handle<Mesh>, Handle<StandardMaterial>)>,
    ) {
        if let Ok((m, mat)) = q_mesh_mats.get(e) {
            out.push((m.clone(), mat.clone()));
        }
        if let Ok(children) = q_children.get(e) {
            for &c in children.iter() {
                visit(c, q_children, q_mesh_mats, out);
            }
        }
    }
    let mut collected = Vec::new();
    for root in q_templates.iter() {
        visit(root, &q_children, &q_mesh_mats, &mut collected);
    }
    if !collected.is_empty() {
        collected.truncate(4); // keep a few variants
        variants.variants = collected;
        variants.ready = true;
        for root in q_templates.iter() {
            commands.entity(root).despawn_recursive();
        }
        info!("Particle instancing: extracted {} candy mesh variants", variants.variants.len());
    }
}

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AtmosDustConfig::default())
.init_resource::<ParticleMaterials>()
            .init_resource::<SnowflakeModel>()
            .init_resource::<CandyModels>()
            .insert_resource(CandyMeshVariants::default())
            .add_event::<BallGroundImpactEvent>()
            .add_event::<TargetHitEvent>()
            .add_event::<GameOverEvent>()
            .add_event::<ShotFiredEvent>()
            .add_systems(Startup, (setup_atmospheric_dust, spawn_candy_templates))
            .add_systems(Update, (
                extract_candy_variants.before(recycle_atmospheric_dust),
                recycle_atmospheric_dust,
                spawn_dust_on_impact,
                spawn_shot_blast,
                spawn_explosion_on_hit,
                spawn_confetti_on_game_over,
                update_particles,
            ));
    }
}

// -------- Atmospheric Dust (persistent primitive spheres) --------
fn setup_atmospheric_dust(
    mut commands: Commands,
    cfg: Res<AtmosDustConfig>,
    snow: Res<SnowflakeModel>,
) {
    let mut rng = thread_rng();
    for _ in 0..cfg.count {
        let x = rng.gen_range(-cfg.half_extent..=cfg.half_extent);
        let y = rng.gen_range(40.0..80.0); // lowered altitude
        let z = rng.gen_range(-cfg.half_extent..=cfg.half_extent);
        let max_scale = rng.gen_range(8.75..17.5); // half previous size (was 17.5..35.0)
        let angular = Vec3::new(
            rng.gen_range(-0.4..0.4),
            rng.gen_range(-0.4..0.4),
            rng.gen_range(-0.4..0.4),
        );
        commands.spawn((
            SceneBundle {
                scene: snow.handle.clone(),
                transform: Transform::from_translation(Vec3::new(x, y, z))
                    .with_scale(Vec3::splat(0.0)),
                ..default()
            },
            ParticleKind::DustAtmos,
            Particle {
                lifetime: 20.0,
                age: 0.0,
                gravity: 0.0,
                vel: Vec3::ZERO,
                angular_vel: angular,
                start_scale: Vec3::ZERO,
                end_scale: Vec3::splat(max_scale),
            },
        ));
    }
}

// Recycle rising atmospheric dust
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

// Helper: pick candy handle
fn random_candy<'a>(rng: &mut impl Rng, candy: &'a [Handle<Scene>; 2]) -> Handle<Scene> {
    if rng.gen_bool(0.5) {
        candy[0].clone()
    } else {
        candy[1].clone()
    }
}

// -------- Impact Dust (now candy chunks) --------
fn spawn_dust_on_impact(
    mut ev: EventReader<BallGroundImpactEvent>,
    mut commands: Commands,
    candy_models: Res<CandyModels>,
    variants: Res<CandyMeshVariants>,
) {
    for e in ev.read() {
        if e.intensity < BOUNCE_EFFECT_INTENSITY_MIN { continue; }
        let count = (6.0 + e.intensity * 4.0).clamp(6.0, 40.0) as usize;
        let mut rng = thread_rng();
        for _ in 0..count {
            // random outward hemisphere direction
            let dir = {
                let mut d;
                loop {
                    d = Vec3::new(rng.gen_range(-1.0..1.0), rng.gen_range(0.0..1.0), rng.gen_range(-1.0..1.0));
                    if d.length_squared() > 0.01 { break; }
                }
                d.normalize()
            };
            let speed = rng.gen_range(0.45..1.6) * (0.35 + e.intensity * 0.5); // keep mid explosive velocity
            let scale = rng.gen_range(0.18..0.28); // larger than current, still smaller than original max 0.30
            let angular = Vec3::new(
                rng.gen_range(-2.2..2.2),
                rng.gen_range(-2.2..2.2),
                rng.gen_range(-2.2..2.2),
            );
            let transform = Transform::from_translation(e.pos + Vec3::Y * 0.03)
                .with_scale(Vec3::splat(scale))
                .with_rotation(Quat::from_euler(
                    EulerRot::XYZ,
                    rng.gen_range(0.0..std::f32::consts::TAU),
                    rng.gen_range(0.0..std::f32::consts::TAU),
                    rng.gen_range(0.0..std::f32::consts::TAU),
                ));
            if variants.ready && !variants.variants.is_empty() {
                let (mesh, material) = &variants.variants[rng.gen_range(0..variants.variants.len())];
                commands.spawn((
                    PbrBundle {
                        mesh: mesh.clone(),
                        material: material.clone(),
                        transform,
                        ..default()
                    },
                    ParticleKind::DustBurst,
                    Particle {
                        lifetime: 10.0,
                        age: 0.0,
                        gravity: -9.8,
                        vel: dir * speed,
                        angular_vel: angular,
                        start_scale: Vec3::splat(scale),
                        end_scale: Vec3::splat(scale * 2.2),
                    },
                ));
            } else {
                commands.spawn((
                    SceneBundle {
                        scene: random_candy(&mut rng, &candy_models.candy),
                        transform,
                        ..default()
                    },
                    ParticleKind::DustBurst,
                    Particle {
                        lifetime: 10.0,
                        age: 0.0,
                        gravity: -9.8,
                        vel: dir * speed,
                        angular_vel: angular,
                        start_scale: Vec3::splat(scale),
                        end_scale: Vec3::splat(scale * 2.2),
                    },
                ));
            }
        }
    }
}

fn spawn_shot_blast(
    mut ev: EventReader<ShotFiredEvent>,
    mut commands: Commands,
    candy_models: Res<CandyModels>,
    variants: Res<CandyMeshVariants>,
) {
    for e in ev.read() {
        let mut rng = thread_rng();
        // Scale count with shot power (power 0..1)
        let count = (14.0 + e.power * 40.0).round() as usize;
        for _ in 0..count {
            // Sample direction in upper hemisphere biased slightly upward.
            let dir = {
                let mut d;
                loop {
                    d = Vec3::new(
                        rng.gen_range(-1.0..1.0),
                        rng.gen_range(0.0..1.0),
                        rng.gen_range(-1.0..1.0),
                    );
                    if d.length_squared() > 0.05 { break; }
                }
                // Add mild upward bias then normalize.
                let mut d2 = d + Vec3::Y * 0.35;
                d2 = d2.normalize();
                d2
            };
            // Speed scales with power; keep within a pleasing arc
            let speed = rng.gen_range(4.0..8.5) * (0.45 + 0.65 * e.power);
            let scale = rng.gen_range(0.16..0.30);
            let transform = Transform::from_translation(e.pos + Vec3::Y * 0.15)
                .with_scale(Vec3::splat(scale))
                .with_rotation(Quat::from_euler(
                    EulerRot::XYZ,
                    rng.gen_range(0.0..std::f32::consts::TAU),
                    rng.gen_range(0.0..std::f32::consts::TAU),
                    rng.gen_range(0.0..std::f32::consts::TAU),
                ));
            let particle = Particle {
                lifetime: rng.gen_range(0.45..0.85),
                age: 0.0,
                gravity: -9.5,
                vel: dir * speed,
                angular_vel: Vec3::new(
                    rng.gen_range(-5.0..5.0),
                    rng.gen_range(-5.0..5.0),
                    rng.gen_range(-5.0..5.0),
                ),
                start_scale: Vec3::splat(scale),
                end_scale: Vec3::splat(scale * rng.gen_range(1.0..1.4)),
            };
            if variants.ready && !variants.variants.is_empty() {
                let (mesh, material) = &variants.variants[rng.gen_range(0..variants.variants.len())];
                commands.spawn((
                    PbrBundle {
                        mesh: mesh.clone(),
                        material: material.clone(),
                        transform,
                        ..default()
                    },
                    ParticleKind::ShotBlast,
                    particle,
                ));
            } else {
                commands.spawn((
                    SceneBundle {
                        scene: random_candy(&mut rng, &candy_models.candy),
                        transform,
                        ..default()
                    },
                    ParticleKind::ShotBlast,
                    particle,
                ));
            }
        }
    }
}

// -------- Target Explosion (candy shrapnel) --------
fn spawn_explosion_on_hit(
    mut ev: EventReader<TargetHitEvent>,
    mut commands: Commands,
    candy_models: Res<CandyModels>,
    variants: Res<CandyMeshVariants>,
) {
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
            let scale = rng.gen_range(0.20..0.40);
            let transform = Transform::from_translation(e.pos)
                .with_scale(Vec3::splat(scale))
                .with_rotation(Quat::from_euler(
                    EulerRot::XYZ,
                    rng.gen_range(0.0..std::f32::consts::TAU),
                    rng.gen_range(0.0..std::f32::consts::TAU),
                    rng.gen_range(0.0..std::f32::consts::TAU),
                ));
            let particle = Particle {
                lifetime: rng.gen_range(0.5..1.0),
                age: 0.0,
                gravity: -9.0,
                vel: dir * speed,
                angular_vel: Vec3::new(
                    rng.gen_range(-6.0..6.0),
                    rng.gen_range(-6.0..6.0),
                    rng.gen_range(-6.0..6.0),
                ),
                start_scale: Vec3::splat(scale),
                end_scale: Vec3::splat(scale),
            };
            if variants.ready && !variants.variants.is_empty() {
                let (mesh, material) = &variants.variants[rng.gen_range(0..variants.variants.len())];
                commands.spawn((
                    PbrBundle {
                        mesh: mesh.clone(),
                        material: material.clone(),
                        transform,
                        ..default()
                    },
                    ParticleKind::Explosion,
                    particle,
                ));
            } else {
                commands.spawn((
                    SceneBundle {
                        scene: random_candy(&mut rng, &candy_models.candy),
                        transform,
                        ..default()
                    },
                    ParticleKind::Explosion,
                    particle,
                ));
            }
        }
    }
}

// -------- Game Over Confetti (candy rain) --------
fn spawn_confetti_on_game_over(
    mut ev: EventReader<GameOverEvent>,
    mut commands: Commands,
    candy_models: Res<CandyModels>,
    variants: Res<CandyMeshVariants>,
) {
    for e in ev.read() {
        let mut rng = thread_rng();
        let count = 300;
        for _ in 0..count {
            let pos = e.pos + Vec3::new(
                rng.gen_range(-8.0..8.0),
                rng.gen_range(4.0..14.0),
                rng.gen_range(-8.0..8.0),
            );
            let vel = Vec3::new(
                rng.gen_range(-2.5..2.5),
                rng.gen_range(0.5..3.0),
                rng.gen_range(-2.5..2.5),
            );
            let scale = rng.gen_range(0.12..0.22);
            let transform = Transform::from_translation(pos)
                .with_scale(Vec3::splat(scale))
                .with_rotation(Quat::from_euler(
                    EulerRot::XYZ,
                    rng.gen_range(0.0..std::f32::consts::TAU),
                    rng.gen_range(0.0..std::f32::consts::TAU),
                    rng.gen_range(0.0..std::f32::consts::TAU),
                ));
            let particle = Particle {
                lifetime: rng.gen_range(3.5..6.0),
                age: 0.0,
                gravity: -6.0,
                vel,
                angular_vel: Vec3::new(
                    rng.gen_range(-3.0..3.0),
                    rng.gen_range(-3.0..3.0),
                    rng.gen_range(-3.0..3.0),
                ),
                start_scale: Vec3::splat(scale),
                end_scale: Vec3::splat(scale),
            };
            if variants.ready && !variants.variants.is_empty() {
                let (mesh, material) = &variants.variants[rng.gen_range(0..variants.variants.len())];
                commands.spawn((
                    PbrBundle {
                        mesh: mesh.clone(),
                        material: material.clone(),
                        transform,
                        ..default()
                    },
                    ParticleKind::Confetti,
                    particle,
                ));
            } else {
                commands.spawn((
                    SceneBundle {
                        scene: random_candy(&mut rng, &candy_models.candy),
                        transform,
                        ..default()
                    },
                    ParticleKind::Confetti,
                    particle,
                ));
            }
        }
    }
}

// -------- Particle Update --------
fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Particle, &ParticleKind)>,
) {
    let dt = time.delta_seconds();
    for (e, mut t, mut p, kind) in &mut q {
        p.age += dt;
        // Integrate motion (all manual now)
        p.vel.y += p.gravity * dt;
        t.translation += p.vel * dt;

        // Angular rotation
        let ang = p.angular_vel * dt;
        if ang.length_squared() > 0.0 {
            let qrot = Quat::from_euler(EulerRot::XYZ, ang.x, ang.y, ang.z);
            t.rotate_local(qrot);
        }
        // Scale over lifetime:
        // - DustAtmos (sky snowflakes): scale in (0->max) first half, out (max->0) second half
        // - Others: linear lerp start->end
        let progress = (p.age / p.lifetime).clamp(0.0, 1.0);
        if matches!(kind, ParticleKind::DustAtmos) {
            let phase = if progress < 0.5 {
                progress / 0.5
            } else {
                (1.0 - progress) / 0.5
            };
            t.scale = p.end_scale * phase;
        } else {
            t.scale = p.start_scale.lerp(p.end_scale, progress);
        }

        if p.age >= p.lifetime {
            commands.entity(e).despawn_recursive();
            continue;
        }
        // (Fade skipped for glb candy models)
    }
}
