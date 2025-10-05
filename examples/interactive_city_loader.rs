use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use tracing_subscriber;

use bevy_osm_tiles::{FeatureSet, OsmFeature, TileType, bevy_plugin::*};

fn main() {
    // Initialize tracing
    if tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .finish(),
    )
    .is_err()
    {
        // Tracing already initialized, that's fine
    }

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Interactive OSM City Loader".to_string(),
                        resolution: (1400, 900).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    level: bevy::log::Level::WARN,
                    ..default()
                }),
        )
        // Add the OSM tiles plugin
        .add_plugins(OsmTilesPlugin::new().with_overpass_provider())
        .init_resource::<AppState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                handle_keyboard_input,
                handle_ui_interactions,
                handle_map_loaded,
                handle_map_failed,
                update_loading_ui,
                update_camera,
                update_input_display,
            ),
        )
        .run();
}

#[derive(Resource)]
struct AppState {
    current_input: String,
    loading: bool,
    last_loaded_city: Option<String>,
    current_feature_set: FeaturePreset,
    current_resolution: u32,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_input: "Berlin".to_string(),
            loading: false,
            last_loaded_city: None,
            current_feature_set: FeaturePreset::Urban,
            current_resolution: 250,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum FeaturePreset {
    Urban,
    Transportation,
    Natural,
    Comprehensive,
    Gaming,
}

impl FeaturePreset {
    fn name(&self) -> &'static str {
        match self {
            Self::Urban => "Urban",
            Self::Transportation => "Transport",
            Self::Natural => "Natural",
            Self::Comprehensive => "Complete",
            Self::Gaming => "Gaming",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::Urban => "Roads, buildings, parks, water",
            Self::Transportation => "Roads, highways, railways, parking",
            Self::Natural => "Water, rivers, forests, parks",
            Self::Comprehensive => "All features available",
            Self::Gaming => "Urban + amenities + tourism",
        }
    }

    fn to_feature_set(&self) -> FeatureSet {
        match self {
            Self::Urban => FeatureSet::urban(),
            Self::Transportation => FeatureSet::transportation(),
            Self::Natural => FeatureSet::natural(),
            Self::Comprehensive => FeatureSet::comprehensive(),
            Self::Gaming => FeatureSet::urban()
                .with_feature(OsmFeature::Amenities)
                .with_feature(OsmFeature::Tourism),
        }
    }

    fn next(&self) -> Self {
        match self {
            Self::Urban => Self::Transportation,
            Self::Transportation => Self::Natural,
            Self::Natural => Self::Comprehensive,
            Self::Comprehensive => Self::Gaming,
            Self::Gaming => Self::Urban,
        }
    }

    fn color(&self) -> Color {
        match self {
            Self::Urban => Color::srgb(0.3, 0.5, 0.8),
            Self::Transportation => Color::srgb(0.8, 0.5, 0.3),
            Self::Natural => Color::srgb(0.3, 0.8, 0.3),
            Self::Comprehensive => Color::srgb(0.8, 0.3, 0.8),
            Self::Gaming => Color::srgb(0.8, 0.8, 0.3),
        }
    }
}

impl AppState {
    fn next_resolution(&mut self) {
        self.current_resolution = match self.current_resolution {
            250 => 500,
            500 => 1000,
            1000 => 2000,
            2000 => 4000,
            4000 => 8000,
            _ => 250,
        };
    }

    fn resolution_color(&self) -> Color {
        match self.current_resolution {
            250 => Color::srgb(0.4, 0.8, 0.4),  // Green - fast
            500 => Color::srgb(0.6, 0.8, 0.4),  // Yellow-green
            1000 => Color::srgb(0.8, 0.8, 0.4), // Yellow
            2000 => Color::srgb(0.8, 0.6, 0.4), // Orange
            4000 => Color::srgb(0.8, 0.4, 0.4), // Red-orange
            8000 => Color::srgb(0.8, 0.2, 0.2), // Red - slow
            _ => Color::srgb(0.5, 0.5, 0.5),    // Gray
        }
    }

    fn resolution_warning(&self) -> Option<&'static str> {
        match self.current_resolution {
            250..=500 => None,
            1000 => Some("⚠️ Medium detail"),
            2000 => Some("⚠️ High detail - slower"),
            4000 => Some("🐌 Very high detail - slow"),
            8000 => Some("🐌 Ultra detail - very slow"),
            _ => None,
        }
    }
}

#[derive(Component)]
struct MapContainer;

#[derive(Component)]
struct CameraController;

#[derive(Component)]
struct InputDisplay;

#[derive(Component)]
struct StatusDisplay;

#[derive(Component)]
struct LoadButton;

#[derive(Component)]
struct FeatureButton;

#[derive(Component)]
struct ResolutionButton;

fn setup(mut commands: Commands) {
    setup_camera(&mut commands);
    setup_lighting(&mut commands);
    setup_map_container(&mut commands);
    setup_ui(&mut commands);
}

fn setup_camera(commands: &mut Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 50.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
        CameraController,
    ));
}

fn setup_lighting(commands: &mut Commands) {
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 10000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.5, 0.0)),
    ));
}

fn setup_map_container(commands: &mut Commands) {
    commands.spawn((
        Transform::default(),
        Visibility::default(),
        MapContainer,
        Name::new("MapContainer"),
    ));
}

fn setup_ui(commands: &mut Commands) {
    commands.spawn(create_root_node()).with_children(|spawner| {
        setup_top_panel(spawner);
        setup_status_panel(spawner);
        setup_controls_panel(spawner);
    });
}

fn create_root_node() -> (Node, BackgroundColor) {
    (
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(Color::NONE),
    )
}

fn setup_top_panel(spawner: &mut ChildSpawnerCommands) {
    spawner
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(180.0), // Increased height for better layout
                padding: UiRect::all(Val::Px(20.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
        ))
        .with_children(|spawner| {
            setup_title(spawner);
            setup_input_area(spawner);
            setup_button_area(spawner);
        });
}

fn setup_title(spawner: &mut ChildSpawnerCommands) {
    spawner.spawn((
        Text::new("Interactive OSM City Loader"),
        TextLayout::new_with_justify(Justify::Center),
        TextFont {
            font_size: 32.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            margin: UiRect::bottom(Val::Px(10.0)),
            ..default()
        },
    ));
}

fn setup_input_area(spawner: &mut ChildSpawnerCommands) {
    spawner
        .spawn((Node {
            width: Val::Percent(100.0),
            height: Val::Px(60.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(15.0),
            margin: UiRect::bottom(Val::Px(15.0)),
            ..default()
        },))
        .with_children(|spawner| {
            setup_input_label(spawner);
            setup_input_display(spawner);
            setup_load_button(spawner);
        });
}

fn setup_button_area(spawner: &mut ChildSpawnerCommands) {
    spawner
        .spawn((Node {
            width: Val::Percent(100.0),
            height: Val::Px(60.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(20.0),
            ..default()
        },))
        .with_children(|spawner| {
            setup_feature_button(spawner);
            setup_resolution_button(spawner);
        });
}

fn setup_input_label(spawner: &mut ChildSpawnerCommands) {
    spawner.spawn((
        Text::new("City: "),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::WHITE),
    ));
}

fn setup_input_display(spawner: &mut ChildSpawnerCommands) {
    spawner.spawn((
        Text::new("Berlin"),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::srgb(0.8, 1.0, 0.8)),
        Node {
            padding: UiRect::all(Val::Px(8.0)),
            flex_grow: 1.0,
            ..default()
        },
        BackgroundColor(Color::srgba(0.2, 0.2, 0.2, 0.8)),
        InputDisplay,
    ));
}

fn setup_load_button(spawner: &mut ChildSpawnerCommands) {
    spawner
        .spawn((
            Button,
            Node {
                width: Val::Px(100.0),
                height: Val::Px(40.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.2, 0.7, 0.3)),
            LoadButton,
        ))
        .with_children(|spawner| {
            spawner.spawn((
                Text::new("Load"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn setup_feature_button(spawner: &mut ChildSpawnerCommands) {
    spawner
        .spawn((
            Button,
            Node {
                width: Val::Px(200.0),
                height: Val::Px(50.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(FeaturePreset::Urban.color()),
            FeatureButton,
        ))
        .with_children(|spawner| {
            spawner.spawn((
                Text::new("🎯 Urban"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(2.0)),
                    ..default()
                },
            ));
            spawner.spawn((
                Text::new("Click to cycle features"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
            ));
        });
}

fn setup_resolution_button(spawner: &mut ChildSpawnerCommands) {
    spawner
        .spawn((
            Button,
            Node {
                width: Val::Px(150.0),
                height: Val::Px(50.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.4, 0.8, 0.4)),
            ResolutionButton,
        ))
        .with_children(|spawner| {
            spawner.spawn((
                Text::new("📏 250"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(2.0)),
                    ..default()
                },
            ));
            spawner.spawn((
                Text::new("Click to double"),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
            ));
        });
}

fn setup_status_panel(spawner: &mut ChildSpawnerCommands) {
    spawner
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(80.0), // Increased height for warning text
                padding: UiRect::all(Val::Px(20.0)),
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|spawner| {
            spawner.spawn((
                Text::new("Type a city name and press Enter or click Load"),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.8)),
                StatusDisplay,
            ));
        });
}

fn setup_controls_panel(spawner: &mut ChildSpawnerCommands) {
    spawner
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(20.0),
                left: Val::Px(20.0),
                right: Val::Px(20.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        ))
        .with_children(|spawner| {
            spawner.spawn((
                Text::new("Controls: Type to enter city name | Enter/Load to load | Feature/Resolution buttons to cycle | ←→↑↓ to move camera | Alt+Mouse to look | PageUp/PageDown up/down | Ctrl speed boost"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ));
        });
}

// Simplified keyboard input handling using Bevy's text input pattern
fn handle_keyboard_input(
    mut app_state: ResMut<AppState>,
    mut keyboard_input_reader: MessageReader<KeyboardInput>,
    mut load_writer: MessageWriter<LoadMapMessage>,
    map_container: Query<Entity, With<MapContainer>>,
) {
    if app_state.loading {
        return;
    }

    for keyboard_input in keyboard_input_reader.read() {
        if !keyboard_input.state.is_pressed() {
            continue;
        }

        match (&keyboard_input.logical_key, &keyboard_input.text) {
            (Key::Enter, _) => {
                if !app_state.current_input.trim().is_empty() {
                    load_city(&mut app_state, &mut load_writer, &map_container);
                }
            }
            (Key::Backspace, _) => {
                app_state.current_input.pop();
            }
            (Key::Escape, _) => {
                app_state.current_input.clear();
            }
            (_, Some(text)) => {
                if text.chars().all(is_printable_char) {
                    app_state.current_input.push_str(text);
                }
            }
            _ => {}
        }
    }
}

// Simple printable character check (from Bevy examples)
fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);

    !is_in_private_use_area && !chr.is_ascii_control()
}

fn handle_ui_interactions(
    mut app_state: ResMut<AppState>,
    mut interaction_query: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            Option<&LoadButton>,
            Option<&FeatureButton>,
            Option<&ResolutionButton>,
            &Children,
        ),
        (Changed<Interaction>, With<Button>),
    >,
    mut load_writer: MessageWriter<LoadMapMessage>,
    map_container: Query<Entity, With<MapContainer>>,
    mut text_query: Query<&mut Text>,
) {
    for (interaction, mut color, load_btn, feature_btn, resolution_btn, children) in
        &mut interaction_query
    {
        handle_button_interaction(
            interaction,
            &mut color,
            load_btn,
            feature_btn,
            resolution_btn,
            children,
            &mut app_state,
            &mut load_writer,
            &map_container,
            &mut text_query,
        );
    }
}

fn handle_button_interaction(
    interaction: &Interaction,
    color: &mut Mut<BackgroundColor>,
    load_btn: Option<&LoadButton>,
    feature_btn: Option<&FeatureButton>,
    resolution_btn: Option<&ResolutionButton>,
    children: &Children,
    app_state: &mut ResMut<AppState>,
    load_writer: &mut MessageWriter<LoadMapMessage>,
    map_container: &Query<Entity, With<MapContainer>>,
    text_query: &mut Query<&mut Text>,
) {
    match *interaction {
        Interaction::Pressed => {
            handle_button_press(
                load_btn,
                feature_btn,
                resolution_btn,
                children,
                app_state,
                load_writer,
                map_container,
                text_query,
            );
        }
        Interaction::Hovered => {
            update_button_color_hover(color, load_btn, feature_btn, resolution_btn, app_state);
        }
        Interaction::None => {
            update_button_color_normal(color, load_btn, feature_btn, resolution_btn, app_state);
        }
    }
}

fn handle_button_press(
    load_btn: Option<&LoadButton>,
    feature_btn: Option<&FeatureButton>,
    resolution_btn: Option<&ResolutionButton>,
    children: &Children,
    app_state: &mut ResMut<AppState>,
    load_writer: &mut MessageWriter<LoadMapMessage>,
    map_container: &Query<Entity, With<MapContainer>>,
    text_query: &mut Query<&mut Text>,
) {
    if load_btn.is_some() && !app_state.loading && !app_state.current_input.trim().is_empty() {
        load_city(app_state, load_writer, map_container);
    } else if feature_btn.is_some() {
        cycle_feature_preset(app_state, children, text_query);
    } else if resolution_btn.is_some() {
        cycle_resolution(app_state, children, text_query);
    }
}

fn cycle_feature_preset(
    app_state: &mut ResMut<AppState>,
    children: &Children,
    text_query: &mut Query<&mut Text>,
) {
    app_state.current_feature_set = app_state.current_feature_set.next();

    // Update button text (first child is the main text)
    if let Some(&first_child) = children.first() {
        if let Ok(mut text) = text_query.get_mut(first_child) {
            **text = format!("🎯 {}", app_state.current_feature_set.name());
        }
    }

    info!(
        "🎯 Switched to {} features: {}",
        app_state.current_feature_set.name(),
        app_state.current_feature_set.description()
    );
}

fn cycle_resolution(
    app_state: &mut ResMut<AppState>,
    children: &Children,
    text_query: &mut Query<&mut Text>,
) {
    app_state.next_resolution();

    // Update button text (first child is the main text)
    if let Some(&first_child) = children.first() {
        if let Ok(mut text) = text_query.get_mut(first_child) {
            **text = format!("📏 {}", app_state.current_resolution);
        }
    }

    let warning = app_state
        .resolution_warning()
        .unwrap_or("✅ Good performance");
    info!(
        "📏 Resolution set to {} - {}",
        app_state.current_resolution, warning
    );
}

fn update_button_color_hover(
    color: &mut Mut<BackgroundColor>,
    load_btn: Option<&LoadButton>,
    feature_btn: Option<&FeatureButton>,
    resolution_btn: Option<&ResolutionButton>,
    app_state: &ResMut<AppState>,
) {
    if load_btn.is_some() {
        **color = BackgroundColor(Color::srgb(0.25, 0.75, 0.35));
    } else if feature_btn.is_some() {
        let base_color = app_state.current_feature_set.color();
        **color = BackgroundColor(brighten_color(base_color, 0.2));
    } else if resolution_btn.is_some() {
        let base_color = app_state.resolution_color();
        **color = BackgroundColor(brighten_color(base_color, 0.2));
    }
}

fn update_button_color_normal(
    color: &mut Mut<BackgroundColor>,
    load_btn: Option<&LoadButton>,
    feature_btn: Option<&FeatureButton>,
    resolution_btn: Option<&ResolutionButton>,
    app_state: &ResMut<AppState>,
) {
    if load_btn.is_some() {
        **color = BackgroundColor(Color::srgb(0.2, 0.7, 0.3));
    } else if feature_btn.is_some() {
        **color = BackgroundColor(app_state.current_feature_set.color());
    } else if resolution_btn.is_some() {
        **color = BackgroundColor(app_state.resolution_color());
    }
}

fn brighten_color(color: Color, amount: f32) -> Color {
    let [r, g, b, a] = color.to_srgba().to_f32_array();
    Color::srgba(
        (r + amount).min(1.0),
        (g + amount).min(1.0),
        (b + amount).min(1.0),
        a,
    )
}

fn load_city(
    app_state: &mut ResMut<AppState>,
    load_writer: &mut MessageWriter<LoadMapMessage>,
    map_container: &Query<Entity, With<MapContainer>>,
) {
    if let Ok(container_entity) = map_container.single() {
        let request = MapLoadRequest::new(&app_state.current_input)
            .with_features(app_state.current_feature_set.to_feature_set())
            .with_resolution(app_state.current_resolution)
            .for_entity(container_entity);

        load_writer.load_map_with_request(request);
        app_state.loading = true;

        let warning = app_state.resolution_warning().unwrap_or("");
        info!(
            "🚀 Loading city: {} with {} features at {}x resolution {}",
            app_state.current_input,
            app_state.current_feature_set.name(),
            app_state.current_resolution,
            warning
        );
    }
}

fn update_input_display(
    app_state: Res<AppState>,
    mut input_display: Query<&mut Text, With<InputDisplay>>,
) {
    if let Ok(mut text) = input_display.single_mut() {
        let display_text = if app_state.current_input.is_empty() {
            "Type city name...".to_string()
        } else {
            format!("{}|", app_state.current_input)
        };
        **text = display_text;
    }
}

fn handle_map_loaded(
    mut loaded_reader: MessageReader<MapLoadedMessage>,
    mut app_state: ResMut<AppState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut status_display: Query<&mut Text, With<StatusDisplay>>,
    existing_tiles: Query<Entity, With<MapTile>>,
) {
    for message in loaded_reader.read() {
        process_map_loaded(
            message,
            &mut app_state,
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut status_display,
            &existing_tiles,
        );
    }
}

fn process_map_loaded(
    message: &MapLoadedMessage,
    app_state: &mut ResMut<AppState>,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    status_display: &mut Query<&mut Text, With<StatusDisplay>>,
    existing_tiles: &Query<Entity, With<MapTile>>,
) {
    app_state.loading = false;
    app_state.last_loaded_city = Some(message.request.city_name.clone());

    clear_existing_tiles(commands, existing_tiles);
    update_status_display(message, status_display, app_state);
    show_detailed_grid_stats(&message.grid); // Add detailed stats
    render_3d_map(commands, &message.grid, meshes, materials);

    let (width, height) = message.grid.dimensions();
    info!(
        "🎉 Map loaded for {}: {}×{} grid",
        message.request.city_name, width, height
    );
}

fn clear_existing_tiles(commands: &mut Commands, existing_tiles: &Query<Entity, With<MapTile>>) {
    for entity in existing_tiles {
        commands.entity(entity).despawn();
    }
}

fn update_status_display(
    message: &MapLoadedMessage,
    status_display: &mut Query<&mut Text, With<StatusDisplay>>,
    app_state: &ResMut<AppState>,
) {
    let (width, height) = message.grid.dimensions();
    let stats = message.grid.statistics();
    let warning = app_state.resolution_warning().unwrap_or("");

    if let Ok(mut text) = status_display.single_mut() {
        **text = format!(
            "✅ Loaded {}: {}×{} grid | {} tiles | {:.1}% coverage | {} features {}",
            message.request.city_name,
            width,
            height,
            stats.non_empty_tiles,
            stats.coverage_ratio * 100.0,
            app_state.current_feature_set.name(),
            warning
        );
    }
}

fn show_detailed_grid_stats(grid: &bevy_osm_tiles::TileGrid) {
    let stats = grid.statistics();
    info!("🎨 Tile type distribution:");
    let mut type_counts: Vec<_> = stats.tile_type_counts.iter().collect();
    type_counts.sort_by(|a, b| b.1.cmp(a.1));

    for (tile_type, count) in type_counts.iter().take(15) {
        let percentage = **count as f64 / stats.total_tiles as f64 * 100.0;
        if **count > 0 {
            let color = tile_type.default_color();
            info!(
                "  - {:12}: {:6} tiles ({:5.1}%) [RGB: {:3},{:3},{:3}]",
                tile_type.name(),
                count,
                percentage,
                color.0,
                color.1,
                color.2
            );
        }
    }

    // Additional debugging info
    info!("🔍 Grid debug info:");
    info!("  - Total area: {:.2} km²", stats.area_km2);
    info!("  - Meters per tile: {:.1}m", stats.meters_per_tile);
    info!(
        "  - Grid dimensions: {}×{}",
        stats.dimensions.0, stats.dimensions.1
    );
}

fn handle_map_failed(
    mut failed_reader: MessageReader<MapLoadFailedMessage>,
    mut app_state: ResMut<AppState>,
    mut status_display: Query<&mut Text, With<StatusDisplay>>,
) {
    for message in failed_reader.read() {
        app_state.loading = false;

        if let Ok(mut text) = status_display.single_mut() {
            **text = format!(
                "❌ Failed to load {}: {}",
                message.request.city_name, message.error
            );
        }

        error!(
            "❌ Failed to load map for {}: {}",
            message.request.city_name, message.error
        );
    }
}

fn update_loading_ui(
    mut progress_reader: MessageReader<MapLoadProgressMessage>,
    mut status_display: Query<&mut Text, With<StatusDisplay>>,
) {
    for message in progress_reader.read() {
        if let Ok(mut text) = status_display.single_mut() {
            let stage_text = match message.stage {
                LoadingStage::ResolvingCity => "🔍 Resolving city location...",
                LoadingStage::FetchingData => "📡 Fetching OSM data...",
                LoadingStage::GeneratingGrid => "🏗️ Generating grid...",
                LoadingStage::Complete => "✅ Complete!",
            };

            **text = format!(
                "Loading {}: {} ({:.0}%)",
                message.request.city_name,
                stage_text,
                message.progress * 100.0
            );
        }
    }
}

fn render_3d_map(
    commands: &mut Commands,
    grid: &bevy_osm_tiles::TileGrid,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let (grid_width, grid_height) = grid.dimensions();
    let tile_size = 2.0;

    let mesh_handles = create_mesh_handles(meshes, tile_size);

    info!("🎨 Rendering 3D map: {}×{} tiles", grid_width, grid_height);

    let mut rendered_count = 0;

    for x in 0..grid_width {
        for z in 0..grid_height {
            if let Some(tile) = grid.get_tile(x, z) {
                if let Some((mesh_handle, height, color)) =
                    get_tile_render_info(tile, &mesh_handles)
                {
                    spawn_tile_entity(
                        commands,
                        mesh_handle,
                        height,
                        color,
                        x,
                        z,
                        grid_width,
                        grid_height,
                        tile_size,
                        tile,
                        materials,
                    );
                    rendered_count += 1;
                }
            }
        }
    }

    info!("✅ Rendered {} 3D tiles", rendered_count);
}

struct MeshHandles {
    cube: Handle<Mesh>,
    road: Handle<Mesh>,
    building: Handle<Mesh>,
    water: Handle<Mesh>,
}

fn create_mesh_handles(meshes: &mut ResMut<Assets<Mesh>>, tile_size: f32) -> MeshHandles {
    MeshHandles {
        cube: meshes.add(Cuboid::new(tile_size, 1.0, tile_size)),
        road: meshes.add(Cuboid::new(tile_size, 0.2, tile_size)),
        building: meshes.add(Cuboid::new(tile_size, 4.0, tile_size)),
        water: meshes.add(Cuboid::new(tile_size, 0.1, tile_size)),
    }
}

fn get_tile_render_info(
    tile: &bevy_osm_tiles::Tile,
    mesh_handles: &MeshHandles,
) -> Option<(Handle<Mesh>, f32, Color)> {
    match tile.tile_type {
        TileType::Empty => None,
        TileType::Building => Some((
            mesh_handles.building.clone(),
            2.0,
            Color::srgb(0.6, 0.4, 0.2),
        )),
        TileType::Road => Some((mesh_handles.road.clone(), 0.1, Color::srgb(0.3, 0.3, 0.3))), // Darker roads to match CLI example
        TileType::Water => Some((mesh_handles.water.clone(), 0.05, Color::srgb(0.2, 0.6, 1.0))),
        TileType::GreenSpace => Some((mesh_handles.road.clone(), 0.2, Color::srgb(0.2, 0.8, 0.2))),
        TileType::Railway => Some((mesh_handles.road.clone(), 0.15, Color::srgb(0.5, 0.3, 0.1))),
        TileType::Parking => Some((mesh_handles.road.clone(), 0.05, Color::srgb(0.4, 0.4, 0.4))),
        TileType::Amenity => Some((mesh_handles.cube.clone(), 1.0, Color::srgb(1.0, 0.6, 0.0))),
        TileType::Tourism => Some((mesh_handles.cube.clone(), 1.5, Color::srgb(1.0, 0.2, 0.8))),
        TileType::Industrial => Some((
            mesh_handles.building.clone(),
            3.0,
            Color::srgb(0.5, 0.0, 0.5),
        )),
        TileType::Residential => Some((
            mesh_handles.building.clone(),
            1.8,
            Color::srgb(1.0, 1.0, 0.0), // Yellow buildings like CLI example
        )),
        TileType::Commercial => Some((
            mesh_handles.building.clone(),
            2.5,
            Color::srgb(1.0, 0.0, 0.0), // Red buildings like CLI example
        )),
        TileType::Custom(_) => Some((mesh_handles.cube.clone(), 0.8, Color::srgb(0.7, 0.7, 0.7))),
    }
}

fn spawn_tile_entity(
    commands: &mut Commands,
    mesh_handle: Handle<Mesh>,
    height: f32,
    color: Color,
    x: usize,
    z: usize,
    grid_width: usize,
    grid_height: usize,
    tile_size: f32,
    tile: &bevy_osm_tiles::Tile,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let world_x = (x as f32 - grid_width as f32 / 2.0) * tile_size;
    let world_z = (z as f32 - grid_height as f32 / 2.0) * tile_size;

    let material_handle = materials.add(StandardMaterial {
        base_color: color,
        metallic: get_tile_metallic(&tile.tile_type),
        perceptual_roughness: get_tile_roughness(&tile.tile_type),
        ..default()
    });

    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material_handle),
        Transform::from_xyz(world_x, height, world_z),
        MapTile {
            tile_type: tile.tile_type.clone(),
            grid_pos: (x, z),
        },
    ));
}

fn get_tile_metallic(tile_type: &TileType) -> f32 {
    match tile_type {
        TileType::Water => 0.8, // Match CLI example
        TileType::Building | TileType::Commercial | TileType::Industrial => 0.2, // Match CLI example
        _ => 0.1,
    }
}

fn get_tile_roughness(tile_type: &TileType) -> f32 {
    match tile_type {
        TileType::Water => 0.1,
        _ => 0.8,
    }
}

// Changed camera controls to use arrow keys instead of WASD
fn update_camera(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut camera_query: Query<&mut Transform, With<CameraController>>,
    time: Res<Time>,
) {
    if let Ok(mut camera_transform) = camera_query.single_mut() {
        update_camera_movement(&mut camera_transform, &keyboard, &time);
        update_camera_rotation(&mut camera_transform, &mut mouse_motion, &keyboard, &time);
    }
}

fn update_camera_movement(
    camera_transform: &mut Transform,
    keyboard: &Res<ButtonInput<KeyCode>>,
    time: &Res<Time>,
) {
    let speed = 30.0 * time.delta_secs(); // Increased speed to match CLI example
    let mut movement = Vec3::ZERO;

    // Changed to arrow keys
    if keyboard.pressed(KeyCode::ArrowUp) {
        movement += *camera_transform.forward();
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        movement += *camera_transform.back();
    }
    if keyboard.pressed(KeyCode::ArrowLeft) {
        movement += *camera_transform.left();
    }
    if keyboard.pressed(KeyCode::ArrowRight) {
        movement += *camera_transform.right();
    }
    if keyboard.pressed(KeyCode::PageUp) {
        movement += Vec3::Y;
    }
    if keyboard.pressed(KeyCode::PageDown) {
        movement -= Vec3::Y;
    }

    let speed_multiplier = if keyboard.pressed(KeyCode::ControlLeft) {
        3.0
    } else {
        1.0
    };
    camera_transform.translation += movement * speed * speed_multiplier;
}

fn update_camera_rotation(
    camera_transform: &mut Transform,
    mouse_motion: &mut MessageReader<bevy::input::mouse::MouseMotion>,
    keyboard: &Res<ButtonInput<KeyCode>>,
    time: &Res<Time>,
) {
    if keyboard.pressed(KeyCode::AltLeft) {
        let rotation_speed = 3.0 * time.delta_secs(); // Match CLI example

        for mouse_delta in mouse_motion.read() {
            let yaw = -mouse_delta.delta.x * rotation_speed * 0.1;
            let pitch = -mouse_delta.delta.y * rotation_speed * 0.1;

            camera_transform.rotate_y(yaw);
            camera_transform.rotate_local_x(pitch);
        }
    }
}

#[derive(Component)]
struct MapTile {
    tile_type: TileType,
    grid_pos: (usize, usize),
}
