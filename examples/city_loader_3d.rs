use bevy::prelude::*;
use clap::Parser;
use tracing::{error, info, warn};
use tracing_subscriber;

use bevy_osm_tiles::{FeatureSet, OsmFeature, TileType, bevy_plugin::*};

#[derive(Parser)]
#[command(name = "osm-3d-city-loader-plugin")]
#[command(about = "Load OpenStreetMap data using the plugin and display as 3D visualization")]
struct Args {
    /// City name to load OSM data for
    #[arg(short, long)]
    city: String,

    /// Feature preset to use: urban, transportation, natural, comprehensive, gaming
    #[arg(short, long, default_value = "urban")]
    features: String,

    /// Data provider: overpass, mock
    #[arg(short, long, default_value = "overpass")]
    provider: String,

    /// Grid resolution (cells per degree)
    #[arg(short, long, default_value = "100")]
    grid_resolution: u32,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Use mock provider for testing
    #[arg(long)]
    mock: bool,

    /// Simulate network delay (for mock provider, in milliseconds)
    #[arg(long)]
    delay: Option<u64>,
}

/// Configuration for the app
#[derive(Resource, Clone, Debug)]
pub struct AppConfig {
    pub city: String,
    pub features: String,
    pub provider: String,
    pub grid_resolution: u32,
    pub delay: Option<u64>,
    pub verbose: bool,
}

impl From<&Args> for AppConfig {
    fn from(args: &Args) -> Self {
        Self {
            city: args.city.clone(),
            features: args.features.clone(),
            provider: if args.mock {
                "mock".to_string()
            } else {
                args.provider.clone()
            },
            grid_resolution: args.grid_resolution,
            delay: args.delay,
            verbose: args.verbose,
        }
    }
}

fn main() {
    let args = Args::parse();

    // Initialize tracing
    if tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_max_level(if args.verbose {
                tracing::Level::DEBUG
            } else {
                tracing::Level::INFO
            })
            .finish(),
    )
    .is_err()
    {
        // Tracing already initialized, that's fine
    }

    info!("🌍 Starting 3D city map loader for: {}", args.city);
    info!("🎯 Feature preset: {}", args.features);
    info!(
        "🔌 Provider: {}",
        if args.mock {
            "mock"
        } else {
            args.provider.as_str()
        }
    );
    info!("🔢 Grid resolution: {} cells/degree", args.grid_resolution);

    let config = AppConfig::from(&args);

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: format!("3D City Map: {}", args.city),
                        resolution: (1280, 720).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    level: bevy::log::Level::WARN, // Reduce Bevy's logging to avoid conflicts
                    ..default()
                }),
        )
        // Add the OSM tiles plugin
        .add_plugins(if args.mock || args.provider == "mock" {
            OsmTilesPlugin::new().with_mock_provider()
        } else {
            OsmTilesPlugin::new().with_overpass_provider()
        })
        .insert_resource(config)
        .add_systems(Startup, (setup, request_map_load.after(setup)))
        .add_systems(
            Update,
            (
                handle_map_loaded,
                handle_map_failed,
                update_camera,
                update_loading_ui,
            ),
        )
        .run();
}

#[derive(Component)]
struct MapContainer;

#[derive(Component)]
struct CameraController;

#[derive(Component)]
struct LoadingText;

#[derive(Component)]
struct StatusText;

fn setup(mut commands: Commands) {
    // Create camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 50.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
        CameraController,
    ));

    // Add light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            intensity: 10000.0,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0),
    ));

    // Create map container entity
    commands.spawn((
        Transform::default(),
        Visibility::default(),
        MapContainer,
        Name::new("MapContainer"),
    ));

    // Create loading UI
    commands.spawn((
        Text::new("Loading map data..."),
        Node {
            position_type: PositionType::Absolute,
            top: px(20.0),
            left: px(20.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 1.0)),
        LoadingText,
    ));

    // Create status text (initially empty)
    commands.spawn((
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            top: px(50.0),
            left: px(20.0),
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.8, 0.8)),
        StatusText,
    ));

    // Create controls text
    commands.spawn((
        Text::new("Controls: WASD to move, Alt + Mouse to look around, Ctrl for boost"),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(20.0),
            left: px(20.0),
            ..default()
        },
        TextColor(Color::srgb(0.6, 0.6, 0.6)),
    ));
}

fn request_map_load(
    config: Res<AppConfig>,
    mut load_writer: MessageWriter<LoadMapMessage>,
    map_container: Query<Entity, With<MapContainer>>,
) {
    let container_entity = match map_container.single() {
        Ok(entity) => entity,
        Err(_) => {
            error!("Expected exactly one MapContainer entity");
            return;
        }
    };

    // Parse feature preset
    let feature_set = match config.features.as_str() {
        "urban" => FeatureSet::urban(),
        "transportation" => FeatureSet::transportation(),
        "natural" => FeatureSet::natural(),
        "comprehensive" => FeatureSet::comprehensive(),
        "gaming" => FeatureSet::urban()
            .with_feature(OsmFeature::Amenities)
            .with_feature(OsmFeature::Tourism),
        _ => {
            warn!(
                "Unknown feature preset: {}. Using 'urban' instead.",
                config.features
            );
            FeatureSet::urban()
        }
    };

    // Create load request
    let mut request = MapLoadRequest::new(&config.city)
        .with_features(feature_set)
        .with_resolution(config.grid_resolution)
        .for_entity(container_entity);

    if config.provider != "overpass" {
        request = request.with_provider(&config.provider);
    }

    // Send the load request
    load_writer.load_map_with_request(request);

    info!("📨 Sent map load request for: {}", config.city);
}

fn handle_map_loaded(
    mut loaded_reader: MessageReader<MapLoadedMessage>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut loading_text: Query<&mut Text, (With<LoadingText>, Without<StatusText>)>,
    mut status_text: Query<&mut Text, (With<StatusText>, Without<LoadingText>)>,
    config: Res<AppConfig>,
) {
    for message in loaded_reader.read() {
        info!(
            "🎉 Map loaded for {}: {}x{} grid",
            message.request.city_name,
            message.grid.dimensions().0,
            message.grid.dimensions().1
        );

        // Update UI
        if let Ok(mut text) = loading_text.single_mut() {
            **text = format!("3D City Map: {}", config.city);
        }

        let (width, height) = message.grid.dimensions();
        if let Ok(mut text) = status_text.single_mut() {
            **text = format!(
                "Grid: {}x{} tiles | Objects: {} | Coverage: {:.1}%",
                width,
                height,
                message.grid.tile_count(),
                message.grid.statistics().coverage_ratio * 100.0
            );
        }

        // Show grid statistics
        show_grid_stats(&message.grid);

        // Spawn 3D visualization
        render_3d_map(&mut commands, &message.grid, &mut meshes, &mut materials);
    }
}

fn handle_map_failed(
    mut failed_reader: MessageReader<MapLoadFailedMessage>,
    mut loading_text: Query<&mut Text, With<LoadingText>>,
) {
    for message in failed_reader.read() {
        error!(
            "❌ Failed to load map for {}: {}",
            message.request.city_name, message.error
        );

        // Update UI to show error
        if let Ok(mut text) = loading_text.single_mut() {
            **text = format!("❌ Failed to load map: {}", message.error);
        }
    }
}

fn update_loading_ui(
    mut progress_reader: MessageReader<MapLoadProgressMessage>,
    mut loading_text: Query<&mut Text, With<LoadingText>>,
) {
    for message in progress_reader.read() {
        if let Ok(mut text) = loading_text.single_mut() {
            let stage_text = match message.stage {
                LoadingStage::ResolvingCity => "Resolving city location...",
                LoadingStage::FetchingData => "Fetching OSM data...",
                LoadingStage::GeneratingGrid => "Generating grid...",
                LoadingStage::Complete => "Complete!",
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

fn show_grid_stats(grid: &bevy_osm_tiles::TileGrid) {
    let stats = grid.statistics();
    info!("🎨 Tile type distribution:");
    let mut type_counts: Vec<_> = stats.tile_type_counts.iter().collect();
    type_counts.sort_by(|a, b| b.1.cmp(a.1));

    for (tile_type, count) in type_counts.iter().take(10) {
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
}

fn render_3d_map(
    commands: &mut Commands,
    grid: &bevy_osm_tiles::TileGrid,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let (grid_width, grid_height) = grid.dimensions();
    let tile_size = 2.0;

    // Get mesh assets
    let (cube_mesh, road_mesh, building_mesh, water_mesh) = (
        meshes.add(Cuboid::new(tile_size, 1.0, tile_size)),
        meshes.add(Cuboid::new(tile_size, 0.2, tile_size)),
        meshes.add(Cuboid::new(tile_size, 4.0, tile_size)),
        meshes.add(Cuboid::new(tile_size, 0.1, tile_size)),
    );

    info!("🎨 Rendering 3D map: {}x{} tiles", grid_width, grid_height);

    let mut rendered_count = 0;

    for x in 0..grid_width {
        for z in 0..grid_height {
            if let Some(tile) = grid.get_tile(x, z) {
                let (mesh_handle, height, color) = match tile.tile_type {
                    TileType::Empty => continue,
                    TileType::Building => (building_mesh.clone(), 2.0, Color::srgb(0.6, 0.4, 0.2)),
                    TileType::Road => (road_mesh.clone(), 0.1, Color::srgb(0.3, 0.3, 0.3)),
                    TileType::Water => (water_mesh.clone(), 0.05, Color::srgb(0.2, 0.6, 1.0)),
                    TileType::GreenSpace => (road_mesh.clone(), 0.2, Color::srgb(0.2, 0.8, 0.2)),
                    TileType::Railway => (road_mesh.clone(), 0.15, Color::srgb(0.5, 0.3, 0.1)),
                    TileType::Parking => (road_mesh.clone(), 0.05, Color::srgb(0.4, 0.4, 0.4)),
                    TileType::Amenity => (cube_mesh.clone(), 1.0, Color::srgb(1.0, 0.6, 0.0)),
                    TileType::Tourism => (cube_mesh.clone(), 1.5, Color::srgb(1.0, 0.2, 0.8)),
                    TileType::Industrial => {
                        (building_mesh.clone(), 3.0, Color::srgb(0.5, 0.0, 0.5))
                    }
                    TileType::Residential => {
                        (building_mesh.clone(), 1.8, Color::srgb(1.0, 1.0, 0.0))
                    }
                    TileType::Commercial => {
                        (building_mesh.clone(), 2.5, Color::srgb(1.0, 0.0, 0.0))
                    }
                    TileType::Custom(_) => (cube_mesh.clone(), 0.8, Color::srgb(0.8, 0.8, 0.8)),
                };

                // Calculate world position
                let world_x = (x as f32 - grid_width as f32 / 2.0) * tile_size;
                let world_z = (z as f32 - grid_height as f32 / 2.0) * tile_size;

                // Create material
                let material_handle = materials.add(StandardMaterial {
                    base_color: color,
                    metallic: match tile.tile_type {
                        TileType::Water => 0.8,
                        TileType::Building | TileType::Commercial | TileType::Industrial => 0.2,
                        _ => 0.1,
                    },
                    ..default()
                });

                // Spawn the tile entity
                commands.spawn((
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material_handle),
                    Transform::from_xyz(world_x, height, world_z),
                    MapTile {
                        tile_type: tile.tile_type.clone(),
                        grid_pos: (x, z),
                    },
                ));

                rendered_count += 1;
            }
        }
    }

    info!("✅ Rendered {} 3D tiles", rendered_count);
}

fn update_camera(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut mouse_motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut camera_query: Query<&mut Transform, With<CameraController>>,
    time: Res<Time>,
) {
    if let Ok(mut camera_transform) = camera_query.single_mut() {
        let speed = 30.0 * time.delta_secs();
        let rotation_speed = 3.0 * time.delta_secs();

        // Handle WASD movement
        let mut movement = Vec3::ZERO;

        if keyboard.pressed(KeyCode::KeyW) {
            movement += *camera_transform.forward();
        }
        if keyboard.pressed(KeyCode::KeyS) {
            movement += *camera_transform.back();
        }
        if keyboard.pressed(KeyCode::KeyA) {
            movement += *camera_transform.left();
        }
        if keyboard.pressed(KeyCode::KeyD) {
            movement += *camera_transform.right();
        }
        if keyboard.pressed(KeyCode::Space) {
            movement += Vec3::Y;
        }
        if keyboard.pressed(KeyCode::ShiftLeft) {
            movement -= Vec3::Y;
        }

        // Boost speed when holding Ctrl
        let speed_multiplier = if keyboard.pressed(KeyCode::ControlLeft) {
            3.0
        } else {
            1.0
        };
        camera_transform.translation += movement * speed * speed_multiplier;

        // Handle mouse look
        for mouse_delta in mouse_motion.read() {
            if keyboard.pressed(KeyCode::AltLeft) {
                let yaw = -mouse_delta.delta.x * rotation_speed * 0.1;
                let pitch = -mouse_delta.delta.y * rotation_speed * 0.1;

                camera_transform.rotate_y(yaw);
                camera_transform.rotate_local_x(pitch);
            }
        }
    }
}

#[derive(Component)]
struct MapTile {
    tile_type: TileType,
    grid_pos: (usize, usize),
}

// Helper function for converting pixels to Val
fn px(value: f32) -> Val {
    Val::Px(value)
}
