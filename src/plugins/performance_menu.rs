use bevy::prelude::*;

use crate::plugins::terrain::TerrainConfig;
use crate::plugins::vegetation::{VegetationConfig, VegetationCullingConfig, VegetationLodConfig};
use crate::plugins::particles::AtmosDustConfig;

#[derive(Resource, Default)]
struct PerfMenuState {
    open: bool,
}

#[derive(Component)]
struct PerfMenuRoot;
#[derive(Component)]
struct PerfMenuPanel;
#[derive(Component)]
struct GearButton;
#[derive(Component)]
struct ParamRow;
#[derive(Component)]
struct ParamValueText {
    kind: ParamKind,
}
#[derive(Component)]
struct ParamAdjustButton {
    kind: ParamKind,
    delta: f32,
}
#[derive(Component)]
struct ToggleButton {
    kind: ParamKind,
}
#[derive(Component)]
struct CloseButton;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParamKind {
    TerrainAmplitude,
    TerrainViewRadius,
    VegetationMaxInstances,
    VegetationSamplesPerFrame,
    VegetationInstancedToggle,
    VegetationDrawCallDebugToggle,
    VegetationCullingEnableToggle,
    VegetationCullingMaxDistance,
    VegetationShadowOn,
    VegetationShadowOff,
    AmbientBrightness,
    AtmosDustCount,
    AtmosDustRiseSpeed,
}

pub struct PerformanceMenuPlugin;
impl Plugin for PerformanceMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PerfMenuState>()
            .add_systems(Startup, spawn_perf_menu_ui)
            .add_systems(Update, (
                gear_button_interaction,
                close_button_interaction,
                param_adjust_buttons,
                toggle_buttons,
                refresh_param_texts,
                sync_panel_visibility,
            ));
    }
}

fn spawn_perf_menu_ui(
    mut commands: Commands,
    assets: Res<AssetServer>,
) {
    let font = assets.load("fonts/FiraSans-Bold.ttf");

    // Root overlay node
    commands.spawn((
        NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            background_color: BackgroundColor(Color::NONE),
            ..default()
        },
        PerfMenuRoot,
    )).with_children(|root| {
        // Gear button (bottom-right)
        root.spawn((
            ButtonBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(12.0),
                    right: Val::Px(12.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    ..default()
                },
                background_color: BackgroundColor(Color::srgb(0.12, 0.12, 0.18)),
                ..default()
            },
            GearButton,
        )).with_children(|b| {
            b.spawn(TextBundle::from_section(
                "⚙",
                TextStyle { font: font.clone(), font_size: 28.0, color: Color::WHITE }
            ));
        });

        // Panel (hidden initially)
        root.spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(60.0),
                    right: Val::Px(12.0),
                    width: Val::Px(360.0),
                    max_height: Val::Px(640.0),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::clip_y(),
                    row_gap: Val::Px(4.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                background_color: BackgroundColor(Color::srgba(0.04, 0.05, 0.08, 0.92)),
                visibility: Visibility::Hidden,
                ..default()
            },
            PerfMenuPanel,
        )).with_children(|panel| {
            // Header
            panel.spawn(TextBundle::from_section(
                "Performance / Tweaks",
                TextStyle { font: font.clone(), font_size: 22.0, color: Color::srgb(0.95,0.95,1.0) }
            ));

            spawn_close_button(panel, &font);

            panel.spawn(TextBundle::from_section(
                "Terrain",
                TextStyle { font: font.clone(), font_size: 18.0, color: Color::srgb(0.80,0.90,1.0) }
            ));

            spawn_param_row(panel, &font, "Amplitude", ParamKind::TerrainAmplitude, 0.25, -0.25, 0.25);
            spawn_param_row(panel, &font, "View Radius (chunks)", ParamKind::TerrainViewRadius, 1.0, -1.0, 1.0);

            panel.spawn(TextBundle::from_section(
                "Vegetation",
                TextStyle { font: font.clone(), font_size: 18.0, color: Color::srgb(0.80,0.90,1.0) }
            ));
            spawn_toggle_row(panel, &font, "Instanced Mode", ParamKind::VegetationInstancedToggle);
            spawn_toggle_row(panel, &font, "DrawCall Debug", ParamKind::VegetationDrawCallDebugToggle);
            spawn_param_row(panel, &font, "Max Instances", ParamKind::VegetationMaxInstances, 500.0, -500.0, 500.0);
            spawn_param_row(panel, &font, "Samples / Frame", ParamKind::VegetationSamplesPerFrame, 100.0, -100.0, 100.0);

            panel.spawn(TextBundle::from_section(
                "Culling & Shadows",
                TextStyle { font: font.clone(), font_size: 18.0, color: Color::srgb(0.80,0.90,1.0) }
            ));
            spawn_toggle_row(panel, &font, "Distance Culling", ParamKind::VegetationCullingEnableToggle);
            spawn_param_row(panel, &font, "Cull Distance", ParamKind::VegetationCullingMaxDistance, 50.0, -50.0, 50.0);
            spawn_param_row(panel, &font, "Shadow On Dist", ParamKind::VegetationShadowOn, 5.0, -5.0, 5.0);
            spawn_param_row(panel, &font, "Shadow Off Dist", ParamKind::VegetationShadowOff, 5.0, -5.0, 5.0);

            panel.spawn(TextBundle::from_section(
                "Lighting",
                TextStyle { font: font.clone(), font_size: 18.0, color: Color::srgb(0.80,0.90,1.0) }
            ));
            spawn_param_row(panel, &font, "Ambient Bright", ParamKind::AmbientBrightness, 50.0, -50.0, 50.0);

            panel.spawn(TextBundle::from_section(
                "Particles",
                TextStyle { font: font.clone(), font_size: 18.0, color: Color::srgb(0.80,0.90,1.0) }
            ));
            spawn_param_row(panel, &font, "Dust Count", ParamKind::AtmosDustCount, 20.0, -20.0, 20.0);
            spawn_param_row(panel, &font, "Dust Rise Speed", ParamKind::AtmosDustRiseSpeed, 0.02, -0.02, 0.02);
        });
    });
}

fn spawn_close_button(parent: &mut ChildBuilder, font: &Handle<Font>) {
    parent.spawn((
        ButtonBundle {
            style: Style {
                align_self: AlignSelf::FlexEnd,
                margin: UiRect::bottom(Val::Px(6.0)),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                ..default()
            },
            background_color: BackgroundColor(Color::srgb(0.30,0.10,0.10)),
            ..default()
        },
        CloseButton,
    )).with_children(|b| {
        b.spawn(TextBundle::from_section(
            "Close",
            TextStyle { font: font.clone(), font_size: 16.0, color: Color::WHITE }
        ));
    });
}

fn spawn_param_row(
    parent: &mut ChildBuilder,
    font: &Handle<Font>,
    label: &str,
    kind: ParamKind,
    step_pos: f32,
    step_neg: f32,
    _display_step: f32,
) {
    parent.spawn((
        NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            ..default()
        },
        ParamRow,
    )).with_children(|row| {
        row.spawn(TextBundle::from_section(
            label,
            TextStyle { font: font.clone(), font_size: 14.0, color: Color::srgb(0.85,0.90,1.0) }
        ));
        // minus
        row.spawn((
            ButtonBundle {
                style: Style {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    ..default()
                },
                background_color: BackgroundColor(Color::srgb(0.20,0.15,0.15)),
                ..default()
            },
            ParamAdjustButton { kind, delta: step_neg },
        )).with_children(|b| {
            b.spawn(TextBundle::from_section(
                "-",
                TextStyle { font: font.clone(), font_size: 16.0, color: Color::WHITE }
            ));
        });
        // value text
        row.spawn((
            TextBundle::from_section(
                "--",
                TextStyle { font: font.clone(), font_size: 14.0, color: Color::WHITE }
            ),
            ParamValueText { kind },
        ));
        // plus
        row.spawn((
            ButtonBundle {
                style: Style {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    ..default()
                },
                background_color: BackgroundColor(Color::srgb(0.15,0.25,0.20)),
                ..default()
            },
            ParamAdjustButton { kind, delta: step_pos },
        )).with_children(|b| {
            b.spawn(TextBundle::from_section(
                "+",
                TextStyle { font: font.clone(), font_size: 16.0, color: Color::WHITE }
            ));
        });
    });
}

fn spawn_toggle_row(
    parent: &mut ChildBuilder,
    font: &Handle<Font>,
    label: &str,
    kind: ParamKind,
) {
    parent.spawn((
        NodeBundle {
            style: Style {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            ..default()
        },
        ParamRow,
    )).with_children(|row| {
        row.spawn(TextBundle::from_section(
            label,
            TextStyle { font: font.clone(), font_size: 14.0, color: Color::srgb(0.85,0.90,1.0) }
        ));
        row.spawn((
            ButtonBundle {
                style: Style {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    ..default()
                },
                background_color: BackgroundColor(Color::srgb(0.18,0.18,0.30)),
                ..default()
            },
            ToggleButton { kind },
        )).with_children(|b| {
            b.spawn((
                TextBundle::from_section(
                    "Toggle",
                    TextStyle { font: font.clone(), font_size: 14.0, color: Color::WHITE }
                ),
            ));
        });
        row.spawn((
            TextBundle::from_section(
                "--",
                TextStyle { font: font.clone(), font_size: 14.0, color: Color::WHITE }
            ),
            ParamValueText { kind },
        ));
    });
}

fn gear_button_interaction(
    mut state: ResMut<PerfMenuState>,
    mut q_button: Query<&Interaction, (Changed<Interaction>, With<GearButton>)>,
) {
    for interaction in q_button.iter_mut() {
        if *interaction == Interaction::Pressed {
            state.open = !state.open;
        }
    }
}

fn close_button_interaction(
    mut state: ResMut<PerfMenuState>,
    mut q_button: Query<&Interaction, (Changed<Interaction>, With<CloseButton>)>,
) {
    for interaction in q_button.iter_mut() {
        if *interaction == Interaction::Pressed {
            state.open = false;
        }
    }
}

fn sync_panel_visibility(
    state: Res<PerfMenuState>,
    mut q_panel: Query<&mut Visibility, With<PerfMenuPanel>>,
) {
    if !state.is_changed() { return; }
    if let Ok(mut vis) = q_panel.get_single_mut() {
        *vis = if state.open { Visibility::Inherited } else { Visibility::Hidden };
    }
}

fn param_adjust_buttons(
    mut q_buttons: Query<(&Interaction, &ParamAdjustButton), (Changed<Interaction>, With<Button>)>,
    mut terrain_cfg: Option<ResMut<TerrainConfig>>,
    mut veg_cfg: Option<ResMut<VegetationConfig>>,
    mut cull_cfg: Option<ResMut<VegetationCullingConfig>>,
    mut lod_cfg: Option<ResMut<VegetationLodConfig>>,
    mut ambient: ResMut<AmbientLight>,
    mut atmos: Option<ResMut<AtmosDustConfig>>,
) {
    for (interaction, btn) in q_buttons.iter_mut() {
        if *interaction != Interaction::Pressed { continue; }
        match btn.kind {
            ParamKind::TerrainAmplitude => {
                if let Some(ref mut c) = terrain_cfg {
                    c.amplitude = (c.amplitude + btn.delta).clamp(0.25, 12.0);
                }
            }
            ParamKind::TerrainViewRadius => {
                if let Some(ref mut c) = terrain_cfg {
                    let mut v = c.view_radius_chunks as f32 + btn.delta;
                    v = v.clamp(2.0, 12.0);
                    c.view_radius_chunks = v.round() as i32;
                }
            }
            ParamKind::VegetationMaxInstances => {
                if let Some(ref mut c) = veg_cfg {
                    let mut v = c.max_instances as f32 + btn.delta;
                    v = v.clamp(500.0, 20000.0);
                    c.max_instances = v.round() as usize;
                }
            }
            ParamKind::VegetationSamplesPerFrame => {
                if let Some(ref mut c) = veg_cfg {
                    let mut v = c.samples_per_frame as f32 + btn.delta;
                    v = v.clamp(50.0, 4000.0);
                    c.samples_per_frame = v.round() as usize;
                }
            }
            ParamKind::VegetationCullingMaxDistance => {
                if let Some(ref mut c) = cull_cfg {
                    let mut v = c.max_distance + btn.delta;
                    v = v.clamp(50.0, 4000.0);
                    c.max_distance = v;
                }
            }
            ParamKind::VegetationShadowOn => {
                if let Some(ref mut c) = lod_cfg {
                    let mut v = c.shadows_full_on + btn.delta;
                    v = v.clamp(20.0, 300.0);
                    c.shadows_full_on = v;
                    if c.shadows_full_on + 5.0 > c.shadows_full_off {
                        c.shadows_full_off = c.shadows_full_on + 5.0;
                    }
                }
            }
            ParamKind::VegetationShadowOff => {
                if let Some(ref mut c) = lod_cfg {
                    let mut v = c.shadows_full_off + btn.delta;
                    v = v.clamp(30.0, 400.0);
                    c.shadows_full_off = v.max(c.shadows_full_on + 5.0);
                }
            }
            ParamKind::AmbientBrightness => {
                ambient.brightness = (ambient.brightness + btn.delta).clamp(50.0, 2000.0);
            }
            ParamKind::AtmosDustCount => {
                if let Some(ref mut c) = atmos {
                    let mut v = c.count as f32 + btn.delta;
                    v = v.clamp(0.0, 2000.0);
                    c.count = v.round() as usize;
                }
            }
            ParamKind::AtmosDustRiseSpeed => {
                if let Some(ref mut c) = atmos {
                    c.rise_speed = (c.rise_speed + btn.delta).clamp(0.0, 2.0);
                }
            }
            _ => {}
        }
    }
}

fn toggle_buttons(
    mut q_buttons: Query<(&Interaction, &ToggleButton), (Changed<Interaction>, With<Button>)>,
    mut veg_cfg: Option<ResMut<VegetationConfig>>,
    mut cull_cfg: Option<ResMut<VegetationCullingConfig>>,
) {
    for (interaction, btn) in q_buttons.iter_mut() {
        if *interaction != Interaction::Pressed { continue; }
        match btn.kind {
            ParamKind::VegetationInstancedToggle => {
                if let Some(ref mut c) = veg_cfg { c.use_instanced = !c.use_instanced; }
            }
            ParamKind::VegetationDrawCallDebugToggle => {
                if let Some(ref mut c) = veg_cfg { c.debug_draw_calls = !c.debug_draw_calls; }
            }
            ParamKind::VegetationCullingEnableToggle => {
                if let Some(ref mut c) = cull_cfg { c.enable_distance = !c.enable_distance; }
            }
            _ => {}
        }
    }
}

fn refresh_param_texts(
    terrain_cfg: Option<Res<TerrainConfig>>,
    veg_cfg: Option<Res<VegetationConfig>>,
    cull_cfg: Option<Res<VegetationCullingConfig>>,
    lod_cfg: Option<Res<VegetationLodConfig>>,
    ambient: Option<Res<AmbientLight>>,
    atmos: Option<Res<AtmosDustConfig>>,
    mut q_values: Query<(&mut Text, &ParamValueText)>,
) {
    for (mut text, tag) in &mut q_values {
        let v = match tag.kind {
            ParamKind::TerrainAmplitude => terrain_cfg.as_ref().map(|c| format!("{:.2}", c.amplitude)),
            ParamKind::TerrainViewRadius => terrain_cfg.as_ref().map(|c| format!("{}", c.view_radius_chunks)),
            ParamKind::VegetationMaxInstances => veg_cfg.as_ref().map(|c| format!("{}", c.max_instances)),
            ParamKind::VegetationSamplesPerFrame => veg_cfg.as_ref().map(|c| format!("{}", c.samples_per_frame)),
            ParamKind::VegetationInstancedToggle => veg_cfg.as_ref().map(|c| if c.use_instanced { "On".into() } else { "Off".into() }),
            ParamKind::VegetationDrawCallDebugToggle => veg_cfg.as_ref().map(|c| if c.debug_draw_calls { "On".into() } else { "Off".into() }),
            ParamKind::VegetationCullingEnableToggle => cull_cfg.as_ref().map(|c| if c.enable_distance { "On".into() } else { "Off".into() }),
            ParamKind::VegetationCullingMaxDistance => cull_cfg.as_ref().map(|c| format!("{:.0}", c.max_distance)),
            ParamKind::VegetationShadowOn => lod_cfg.as_ref().map(|c| format!("{:.0}", c.shadows_full_on)),
            ParamKind::VegetationShadowOff => lod_cfg.as_ref().map(|c| format!("{:.0}", c.shadows_full_off)),
            ParamKind::AmbientBrightness => ambient.as_ref().map(|c| format!("{:.0}", c.brightness)),
            ParamKind::AtmosDustCount => atmos.as_ref().map(|c| format!("{}", c.count)),
            ParamKind::AtmosDustRiseSpeed => atmos.as_ref().map(|c| format!("{:.3}", c.rise_speed)),
        };
        if let Some(s) = v {
            if text.sections[0].value != s {
                text.sections[0].value = s;
            }
        }
    }
}
