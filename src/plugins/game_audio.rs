use bevy::prelude::*;
use bevy::audio::{AudioSource, AudioBundle, PlaybackSettings, PlaybackMode, Volume};
use crate::plugins::particles::{
    BallGroundImpactEvent,
    TargetHitEvent,
    GameOverEvent,
    ShotFiredEvent,
    BOUNCE_EFFECT_INTENSITY_MIN,
};

pub struct GameAudioPlugin;

#[derive(Resource, Clone)]
struct SfxHandles {
    bounce: Handle<AudioSource>,
    hit: Handle<AudioSource>,
    game_over: Handle<AudioSource>,
    launch: Handle<AudioSource>,
    music: Handle<AudioSource>,
}

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_audio_assets)
           .add_systems(Update, (play_event_sfx, ensure_music_loop));
    }
}

fn load_audio_assets(mut commands: Commands, assets: Res<AssetServer>) {
    // Audio assets: using MP3 only. Ensure Cargo.toml enables feature: bevy/mp3.
    // Expected files: assets/audio/{bounce,hit,game_over,launch,music}.mp3
    let handles = SfxHandles {
        bounce: assets.load("audio/bounce.mp3"),
        hit: assets.load("audio/hit.mp3"),
        game_over: assets.load("audio/game_over.mp3"),
        launch: assets.load("audio/launch.mp3"),
        music: assets.load("audio/music.mp3"),
    };
    commands.insert_resource(handles.clone());
    // Spawn looping music entity (will be respawned if despawned accidentally).
    commands.spawn((
        AudioBundle {
            source: handles.music.clone(),
            settings: PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::new(0.55),
                ..default()
            }
        },
        MusicTag,
    ));
}

#[derive(Component)]
struct MusicTag;

fn ensure_music_loop(
    mut commands: Commands,
    q_music: Query<(), With<MusicTag>>,
    sfx: Option<Res<SfxHandles>>,
) {
    if q_music.is_empty() {
        if let Some(sfx) = sfx {
            commands.spawn((
                AudioBundle {
                    source: sfx.music.clone(),
                    settings: PlaybackSettings {
                        mode: PlaybackMode::Loop,
                        volume: Volume::new(0.55),
                        ..default()
                    }
                },
                MusicTag,
            ));
        }
    }
}

fn play_event_sfx(
    sfx: Option<Res<SfxHandles>>,
    mut commands: Commands,
    mut ev_bounce: EventReader<BallGroundImpactEvent>,
    mut ev_hit: EventReader<TargetHitEvent>,
    mut ev_game_over: EventReader<GameOverEvent>,
    mut ev_shot: EventReader<ShotFiredEvent>,
) {
    let Some(sfx) = sfx else { return; };

    for e in ev_bounce.read() {
        if e.intensity < BOUNCE_EFFECT_INTENSITY_MIN {
            continue;
        }
        // Map intensity range [threshold .. ~6] -> volume [0.25 .. 1.0]
        let norm = ((e.intensity - BOUNCE_EFFECT_INTENSITY_MIN) / (6.0 - BOUNCE_EFFECT_INTENSITY_MIN)).clamp(0.0, 1.0);
        let v = 0.25 + norm * 0.75;
        commands.spawn(AudioBundle {
            source: sfx.bounce.clone(),
            settings: PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::new(v),
                ..default()
            }
        });
    }
    for _ in ev_hit.read() {
        commands.spawn(AudioBundle {
            source: sfx.hit.clone(),
            settings: PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::new(0.9),
                ..default()
            }
        });
    }
    for _ in ev_game_over.read() {
        commands.spawn(AudioBundle {
            source: sfx.game_over.clone(),
            settings: PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::new(1.0),
                ..default()
            }
        });
    }
    for e in ev_shot.read() {
        let v = (0.4 + e.power * 0.6).clamp(0.4, 1.0);
        commands.spawn(AudioBundle {
            source: sfx.launch.clone(),
            settings: PlaybackSettings {
                mode: PlaybackMode::Despawn,
                volume: Volume::new(v),
                ..default()
            }
        });
    }
}
