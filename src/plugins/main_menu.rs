// Main menu plugin: displays a simple UI with Play, Level (selector placeholder),
// High Score (read-only), and Quit. Hides itself once Play is pressed.

use bevy::prelude::*;
use crate::plugins::game_state::Score;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    Menu,
    Playing,
}

impl Default for GamePhase {
    fn default() -> Self { GamePhase::Menu }
}

#[derive(Component)]
struct MenuRoot;
#[derive(Component)]
struct PlayButton;
#[derive(Component)]
struct QuitButton;

pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GamePhase::default())
            .add_systems(Startup, spawn_main_menu)
            .add_systems(Update, (menu_button_system,));
    }
}

fn spawn_main_menu(
    mut commands: Commands,
    assets: Res<AssetServer>,
    score: Option<Res<Score>>,
) {
    // Root node (full screen overlay)
    let font = assets.load("fonts/FiraSans-Bold.ttf");
    let high_score = score
        .as_ref()
        .and_then(|s| s.high_score_time)
        .map(|v| format!("{:.2}s", v))
        .unwrap_or_else(|| "--".to_string());

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(14.0),
                    ..default()
                },
                background_color: BackgroundColor(Color::srgba(0.02, 0.02, 0.05, 0.75)),
                ..default()
            },
            MenuRoot,
        ))
        .with_children(|parent| {
            // Title
            parent.spawn(TextBundle::from_section(
                "Vibe Golf",
                TextStyle { font: font.clone(), font_size: 56.0, color: Color::srgb(0.95, 0.95, 1.0) },
            ));
            // Play Button
            spawn_button(
                parent,
                &font,
                "Play",
                Color::srgb(0.15, 0.55, 0.25),
                Some(PlayButton),
            );
            // Level selector placeholder (disabled look)
            parent.spawn(
                TextBundle::from_section(
                    "Level: 1 / 1",
                    TextStyle { font: font.clone(), font_size: 28.0, color: Color::srgb(0.75, 0.75, 0.80) },
                )
                .with_style(Style { margin: UiRect::all(Val::Px(4.0)), ..default() }),
            );
            // High score display
            parent.spawn(
                TextBundle::from_section(
                    format!("Best Time: {high_score}"),
                    TextStyle { font: font.clone(), font_size: 24.0, color: Color::srgb(0.85, 0.85, 0.90) },
                )
                .with_style(Style { margin: UiRect::all(Val::Px(2.0)), ..default() }),
            );
            // Quit Button
            spawn_button(
                parent,
                &font,
                "Quit",
                Color::srgb(0.55, 0.15, 0.15),
                Some(QuitButton),
            );
            // Footer hint
            parent.spawn(
                TextBundle::from_section(
                    "© 2025 Vibe Golf",
                    TextStyle { font: font.clone(), font_size: 16.0, color: Color::srgb(0.55, 0.55, 0.60) },
                )
                .with_style(Style { position_type: PositionType::Absolute, bottom: Val::Px(10.0), right: Val::Px(12.0), ..default() }),
            );
        });
}

fn spawn_button<T: Component>(
    parent: &mut ChildBuilder,
    font: &Handle<Font>,
    label: &str,
    base_color: Color,
    marker: Option<T>,
) {
    let mut ec = parent.spawn(ButtonBundle {
        style: Style {
            width: Val::Px(240.0),
            height: Val::Px(52.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        background_color: BackgroundColor(base_color),
        ..default()
    });
    if let Some(m) = marker {
        ec.insert(m);
    }
    ec.with_children(|b| {
        b.spawn(TextBundle::from_section(
            label,
            TextStyle {
                font: font.clone(),
                font_size: 30.0,
                color: Color::srgb(0.95, 0.95, 1.0),
            },
        ));
    });
}

fn menu_button_system(
    mut commands: Commands,
    mut phase: ResMut<GamePhase>,
    mut exit: EventWriter<AppExit>,
    q_buttons: Query<(&Interaction, Entity, Option<&PlayButton>, Option<&QuitButton>), (Changed<Interaction>, With<Button>)>,
    q_root: Query<Entity, With<MenuRoot>>,
) {
    if *phase != GamePhase::Menu {
        return;
    }
    for (interaction, _entity, play, quit) in &q_buttons {
        match *interaction {
            Interaction::Pressed => {
                if play.is_some() {
                    *phase = GamePhase::Playing;
                    // Despawn entire menu tree
                    if let Ok(root) = q_root.get_single() {
                        commands.entity(root).despawn_recursive();
                    }
                } else if quit.is_some() {
                    exit.send(AppExit::Success);
                }
            }
            _ => {}
        }
    }
}
