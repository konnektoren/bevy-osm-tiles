use bevy::prelude::*;
use clap::Parser;
use tracing::{error, info, warn};
use tracing_subscriber;

use bevy_osm_tiles::{
    DefaultGridGenerator, FeatureSet, GridGenerator, OsmConfigBuilder, OsmDataProvider, OsmFeature,
    ProviderFactory, TileGrid, TileType,
};

#[derive(Parser)]
#[command(name = "osm-3d-city-loader")]
#[command(about = "Load OpenStreetMap data for a city and display it as a 3D visualization")]
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

    /// Skip grid generation (only fetch OSM data)
    #[arg(long)]
    skip_grid: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Test connection only
    #[arg(short, long)]
    test: bool,

    /// Simulate network delay (for mock provider, in milliseconds)
    #[arg(long)]
    delay: Option<u64>,

    /// Show detailed grid statistics
    #[arg(long)]
    grid_stats: bool,
}

/// Configuration for loading city data
#[derive(Resource, Clone, Debug)]
pub struct CityLoadConfig {
    pub city: String,
    pub features: String,
    pub provider: String,
    pub grid_resolution: u32,
    pub delay: Option<u64>,
    pub verbose: bool,
    pub timeout: u32,
}

impl Default for CityLoadConfig {
    fn default() -> Self {
        Self {
            city: String::new(),
            features: "urban".to_string(),
            provider: "mock".to_string(),
            grid_resolution: 100,
            delay: None,
            verbose: false,
            timeout: 60,
        }
    }
}

impl From<&Args> for CityLoadConfig {
    fn from(args: &Args) -> Self {
        Self {
            city: args.city.clone(),
            features: args.features.clone(),
            provider: args.provider.clone(),
            grid_resolution: args.grid_resolution,
            delay: args.delay,
            verbose: args.verbose,
            timeout: 60,
        }
    }
}

fn main() {
    let args = Args::parse();

    // Initialize tracing only if not already initialized
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

    // Handle test-only mode
    if args.test {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            let config = CityLoadConfig::from(&args);
            test_provider_availability(config).await
        });
        match result {
            Ok(()) => {
                info!("✅ Provider is available!");
                return;
            }
            Err(e) => {
                error!("❌ Provider test failed: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Skip grid generation mode
    if args.skip_grid {
        info!("⏭️  Skipping grid generation");
        return;
    }

    info!("🌍 Loading 3D city map for: {}", args.city);
    info!("🎯 Feature preset: {}", args.features);
    info!("🔌 Provider: {}", args.provider);
    info!("🔢 Grid resolution: {} cells/degree", args.grid_resolution);

    // Pre-load the data before starting Bevy to avoid Tokio runtime issues
    let rt = tokio::runtime::Runtime::new().unwrap();
    let grid_result = rt.block_on(async {
        let config = CityLoadConfig::from(&args);
        load_city_data(config).await
    });

    match grid_result {
        Ok(grid) => {
            // Show statistics
            show_grid_stats(&grid);

            // Start Bevy with pre-loaded grid
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
                .add_systems(Startup, setup)
                .add_systems(Update, update_camera)
                .insert_resource(LoadedGrid(grid))
                .insert_resource(CityLoadConfig::from(&args))
                .run();
        }
        Err(e) => {
            error!("❌ Failed to load city data: {}", e);
            std::process::exit(1);
        }
    }
}

#[derive(Resource)]
struct LoadedGrid(TileGrid);

#[derive(Component)]
struct MapTile {
    tile_type: TileType,
    grid_pos: (usize, usize),
}

#[derive(Component)]
struct CameraController;

fn setup(
    mut commands: Commands,
    config: Res<CityLoadConfig>,
    grid: Res<LoadedGrid>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
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

    // Create UI
    commands.spawn((
        Text::new(format!("3D City Map: {}", config.city)),
        Node {
            position_type: PositionType::Absolute,
            top: px(20.0),
            left: px(20.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 1.0)),
    ));

    // Create status text
    let (width, height) = grid.0.dimensions();
    commands.spawn((
        Text::new(format!(
            "Grid: {}x{} tiles | Objects: {} | Coverage: {:.1}%",
            width,
            height,
            grid.0.tile_count(),
            grid.0.statistics().coverage_ratio * 100.0
        )),
        Node {
            position_type: PositionType::Absolute,
            top: px(50.0),
            left: px(20.0),
            ..default()
        },
        TextColor(Color::srgb(0.8, 0.8, 0.8)),
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

    // Render the 3D map immediately
    render_3d_map(&mut commands, &grid.0, &mut meshes, &mut materials);
}

/// Load city data and return the generated grid
pub async fn load_city_data(config: CityLoadConfig) -> Result<TileGrid, String> {
    if config.verbose {
        info!("🌍 Loading city data for: {}", config.city);
        info!("🎯 Feature preset: {}", config.features);
        info!("🔌 Provider: {}", config.provider);
        info!(
            "🔢 Grid resolution: {} cells/degree",
            config.grid_resolution
        );
    }

    // Create the appropriate provider
    let provider: Box<dyn OsmDataProvider> = match config.provider.as_str() {
        "overpass" => Box::new(ProviderFactory::overpass()),
        "mock" => {
            if let Some(delay_ms) = config.delay {
                Box::new(ProviderFactory::mock_with_delay(delay_ms))
            } else {
                Box::new(ProviderFactory::mock())
            }
        }
        _ => {
            let available = ProviderFactory::available_providers();
            error!(
                "Unknown provider: {}. Available providers: {:?}",
                config.provider, available
            );
            return Err("Invalid provider".to_string());
        }
    };

    // Show provider capabilities if verbose
    if config.verbose {
        let capabilities = provider.capabilities();
        info!("🔧 Provider capabilities:");
        info!("  - Real-time data: {}", capabilities.supports_real_time);
        info!("  - Requires network: {}", capabilities.requires_network);
        info!(
            "  - Supports geocoding: {}",
            capabilities.supports_geocoding
        );
        info!("  - WASM compatible: {}", capabilities.wasm_compatible);
    }

    // Create configuration with selected feature preset
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

    let osm_config = OsmConfigBuilder::new()
        .city(&config.city)
        .features(feature_set)
        .grid_resolution(config.grid_resolution)
        .timeout(config.timeout.into())
        .build();

    if config.verbose {
        info!("⚙️  Configuration created successfully");
        info!(
            "📊 Features included: {:?}",
            osm_config.features.features().iter().collect::<Vec<_>>()
        );
    }

    // Resolve the region
    if config.verbose {
        info!("🗺️  Resolving region...");
    }
    let _bbox = match provider.resolve_region(&osm_config.region).await {
        Ok(bbox) => {
            if config.verbose {
                info!("📍 Resolved to bounding box: {:?}", bbox);
                info!("📐 Approximate area: {:.2} km²", bbox.area_km2());
            }
            bbox
        }
        Err(e) => {
            error!("❌ Failed to resolve region: {}", e);
            return Err(e.to_string());
        }
    };

    // Fetch OSM data
    if config.verbose {
        info!("⬇️  Fetching OSM data...");
    }
    let osm_data = match provider.fetch_data(&osm_config).await {
        Ok(osm_data) => {
            if config.verbose {
                info!("✅ Successfully fetched OSM data!");
                info!(
                    "📝 Data size: {} bytes ({:.1} KB)",
                    osm_data.raw_data.len(),
                    osm_data.raw_data.len() as f64 / 1024.0
                );
            }
            osm_data
        }
        Err(e) => {
            error!("❌ Failed to fetch OSM data: {}", e);
            return Err(e.to_string());
        }
    };

    // Generate grid
    if config.verbose {
        info!("🔲 Generating tile grid...");
    }

    let generator = DefaultGridGenerator::new();
    match generator.generate_grid(&osm_data, &osm_config).await {
        Ok(grid) => {
            if config.verbose {
                info!("✅ Grid generation completed!");
                let (width, height) = grid.dimensions();
                info!(
                    "📐 Grid dimensions: {}x{} ({} total tiles)",
                    width,
                    height,
                    grid.tile_count()
                );
            }
            Ok(grid)
        }
        Err(e) => {
            error!("❌ Failed to generate grid: {}", e);
            Err(e.to_string())
        }
    }
}

/// Show detailed grid statistics
pub fn show_grid_stats(grid: &TileGrid) {
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

async fn test_provider_availability(config: CityLoadConfig) -> Result<(), String> {
    let provider: Box<dyn OsmDataProvider> = match config.provider.as_str() {
        "overpass" => Box::new(ProviderFactory::overpass()),
        "mock" => Box::new(ProviderFactory::mock()),
        _ => return Err("Invalid provider".to_string()),
    };

    info!("🔍 Testing provider availability...");
    provider
        .test_availability()
        .await
        .map_err(|e| e.to_string())
}

fn render_3d_map(
    commands: &mut Commands,
    grid: &TileGrid,
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

// Helper function for converting pixels to Val
fn px(value: f32) -> Val {
    Val::Px(value)
}
