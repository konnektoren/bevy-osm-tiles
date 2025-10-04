use bevy::{
    ecs::{system::SystemState, world::CommandQueue},
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future},
};
use bevy_osm_tiles::{
    DefaultGridGenerator, FeatureSet, GridGenerator, OsmConfigBuilder, OsmDataProvider, OsmFeature,
    ProviderFactory, TileGrid, TileType,
};
use clap::Parser;

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
    #[arg(short, long, default_value = "mock")]
    provider: String,

    /// Grid resolution (cells per degree)
    #[arg(short, long, default_value = "50")]
    grid_resolution: u32,

    /// Simulate network delay (for mock provider, in milliseconds)
    #[arg(long)]
    delay: Option<u64>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    println!("🌍 Loading 3D city map for: {}", args.city);
    println!("🎯 Feature preset: {}", args.features);
    println!("🔌 Provider: {}", args.provider);
    println!("🔢 Grid resolution: {} cells/degree", args.grid_resolution);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("3D City Map: {}", args.city),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_map_loading_tasks, update_camera))
        .insert_resource(CityArgs {
            city: args.city,
            features: args.features,
            provider: args.provider,
            grid_resolution: args.grid_resolution,
            delay: args.delay,
            verbose: args.verbose,
        })
        .init_resource::<MapState>()
        .run();
}

#[derive(Resource)]
struct CityArgs {
    city: String,
    features: String,
    provider: String,
    grid_resolution: u32,
    delay: Option<u64>,
    verbose: bool,
}

#[derive(Resource, Default)]
struct MapState {
    loading: bool,
    loaded: bool,
    rendered_entities: Vec<Entity>,
}

#[derive(Component)]
struct MapTile {
    tile_type: TileType,
    grid_pos: (usize, usize),
}

#[derive(Component)]
struct CameraController;

#[derive(Component)]
struct MapLoadingTask(Task<CommandQueue>);

#[derive(Component)]
struct LoadingText;

#[derive(Component)]
struct StatusText;

fn setup(mut commands: Commands, city_args: Res<CityArgs>) {
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

    // Create loading UI
    commands.spawn((
        Text::new(format!("Loading 3D map for {}...", city_args.city)),
        Node {
            position_type: PositionType::Absolute,
            top: px(20.0),
            left: px(20.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 1.0)),
        LoadingText,
    ));

    // Create status text
    commands.spawn((
        Text::new("Fetching OSM data..."),
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
        Text::new("Controls: WASD to move, Alt + Mouse to look around"),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(20.0),
            left: px(20.0),
            ..default()
        },
        TextColor(Color::srgb(0.6, 0.6, 0.6)),
    ));

    // Start loading the city data immediately
    start_loading(&mut commands, &city_args);
}

fn start_loading(commands: &mut Commands, city_args: &CityArgs) {
    let thread_pool = AsyncComputeTaskPool::get();
    let loading_entity = commands.spawn_empty().id();

    // Clone the args for the async task
    let city = city_args.city.clone();
    let features = city_args.features.clone();
    let provider = city_args.provider.clone();
    let grid_resolution = city_args.grid_resolution;
    let delay = city_args.delay;
    let verbose = city_args.verbose;

    let task = thread_pool.spawn(async move {
        // Load the city data asynchronously
        let grid_result =
            load_city_data(&city, &features, &provider, grid_resolution, delay, verbose).await;

        let mut command_queue = CommandQueue::default();

        // Create commands to apply the loaded data
        command_queue.push(move |world: &mut World| {
            match grid_result {
                Some(ref grid) => {
                    println!("✅ Successfully loaded city data, rendering 3D map...");
                    render_3d_map(world, &grid, loading_entity);

                    // Update status
                    let mut system_state = SystemState::<(
                        Query<&mut Text, With<LoadingText>>,
                        Query<&mut Text, (With<StatusText>, Without<LoadingText>)>,
                    )>::new(world);
                    let (mut loading_query, mut status_query) = system_state.get_mut(world);

                    if let Ok(mut loading_text) = loading_query.single_mut() {
                        **loading_text = format!("3D City Map: {}", city);
                    }

                    if let Ok(mut status_text) = status_query.single_mut() {
                        let (width, height) = grid.dimensions();
                        **status_text = format!(
                            "Grid: {}x{} tiles | Rendered: {} objects",
                            width,
                            height,
                            grid.tile_count()
                        );
                    }
                }
                None => {
                    println!("❌ Failed to load city data");

                    // Update status to show error
                    let mut system_state = SystemState::<(
                        Query<&mut Text, With<LoadingText>>,
                        Query<&mut Text, (With<StatusText>, Without<LoadingText>)>,
                    )>::new(world);
                    let (mut loading_query, mut status_query) = system_state.get_mut(world);

                    if let Ok(mut loading_text) = loading_query.single_mut() {
                        **loading_text = "Failed to load city data".to_string();
                    }

                    if let Ok(mut status_text) = status_query.single_mut() {
                        **status_text = "Error: Could not fetch or process OSM data".to_string();
                    }
                }
            }

            // Update map state
            let mut map_state = world.resource_mut::<MapState>();
            map_state.loading = false;
            map_state.loaded = grid_result.is_some();
        });

        command_queue
    });

    commands.entity(loading_entity).insert(MapLoadingTask(task));
}

async fn load_city_data(
    city_name: &str,
    features: &str,
    provider_name: &str,
    grid_resolution: u32,
    delay: Option<u64>,
    verbose: bool,
) -> Option<TileGrid> {
    println!("🌍 Loading map data for: {}", city_name);

    // Create the appropriate provider
    let provider: Box<dyn OsmDataProvider> = match provider_name {
        "overpass" => Box::new(ProviderFactory::overpass()),
        "mock" => {
            if let Some(delay_ms) = delay {
                Box::new(ProviderFactory::mock_with_delay(delay_ms))
            } else {
                Box::new(ProviderFactory::mock())
            }
        }
        _ => {
            eprintln!("❌ Unknown provider: {}", provider_name);
            return None;
        }
    };

    // Create feature set
    let feature_set = match features {
        "urban" => FeatureSet::urban(),
        "transportation" => FeatureSet::transportation(),
        "natural" => FeatureSet::natural(),
        "comprehensive" => FeatureSet::comprehensive(),
        "gaming" => FeatureSet::urban()
            .with_feature(OsmFeature::Amenities)
            .with_feature(OsmFeature::Tourism),
        _ => {
            println!(
                "⚠️  Unknown feature preset: {}. Using 'urban' instead.",
                features
            );
            FeatureSet::urban()
        }
    };

    // Create configuration
    let config = OsmConfigBuilder::new()
        .city(city_name)
        .features(feature_set)
        .grid_resolution(grid_resolution)
        .timeout(60)
        .build();

    if verbose {
        println!(
            "📊 Features included: {:?}",
            config.features.features().iter().collect::<Vec<_>>()
        );
    }

    // Fetch OSM data
    println!("⬇️  Fetching OSM data...");
    let osm_data = match provider.fetch_data(&config).await {
        Ok(osm_data) => {
            println!(
                "✅ Successfully fetched OSM data! ({} bytes)",
                osm_data.raw_data.len()
            );
            osm_data
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch OSM data: {}", e);
            return None;
        }
    };

    // Generate grid
    println!("🔲 Generating tile grid...");
    let generator = DefaultGridGenerator::new();
    match generator.generate_grid(&osm_data, &config).await {
        Ok(grid) => {
            let (width, height) = grid.dimensions();
            println!(
                "✅ Grid generation completed! ({}x{} tiles, {} populated)",
                width,
                height,
                grid.tile_count()
            );

            if verbose {
                let stats = grid.statistics();
                println!("📈 Coverage ratio: {:.1}%", stats.coverage_ratio * 100.0);

                // Show tile type distribution
                let mut type_counts: Vec<_> = stats.tile_type_counts.iter().collect();
                type_counts.sort_by(|a, b| b.1.cmp(a.1));

                println!("🎨 Top tile types:");
                for (tile_type, count) in type_counts.iter().take(5) {
                    if **count > 0 {
                        let percentage = **count as f64 / stats.total_tiles as f64 * 100.0;
                        println!(
                            "  - {}: {} tiles ({:.1}%)",
                            tile_type.name(),
                            count,
                            percentage
                        );
                    }
                }
            }

            Some(grid)
        }
        Err(e) => {
            eprintln!("❌ Failed to generate grid: {}", e);
            None
        }
    }
}

fn handle_map_loading_tasks(
    mut commands: Commands,
    mut loading_tasks: Query<(Entity, &mut MapLoadingTask)>,
) {
    for (entity, mut task) in &mut loading_tasks {
        if let Some(mut command_queue) = block_on(future::poll_once(&mut task.0)) {
            // Execute the commands that were prepared in the async task
            commands.append(&mut command_queue);
            // Remove the completed task
            commands.entity(entity).remove::<MapLoadingTask>();
        }
    }
}

fn render_3d_map(world: &mut World, grid: &TileGrid, _loading_entity: Entity) {
    let (grid_width, grid_height) = grid.dimensions();
    let tile_size = 2.0; // Make tiles a bit larger for better visibility

    // Get mesh and material assets
    let (cube_mesh, road_mesh, building_mesh, water_mesh) = {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        (
            meshes.add(Cuboid::new(tile_size, 1.0, tile_size)),
            meshes.add(Cuboid::new(tile_size, 0.2, tile_size)),
            meshes.add(Cuboid::new(tile_size, 4.0, tile_size)),
            meshes.add(Cuboid::new(tile_size, 0.1, tile_size)),
        )
    };

    println!("🎨 Rendering 3D map: {}x{} tiles", grid_width, grid_height);

    let mut rendered_count = 0;
    let mut rendered_entities = Vec::new();

    for x in 0..grid_width {
        for z in 0..grid_height {
            if let Some(tile) = grid.get_tile(x, z) {
                let (mesh_handle, height, color) = match tile.tile_type {
                    TileType::Empty => continue,
                    TileType::Building => (
                        building_mesh.clone(),
                        2.0,
                        Color::srgb(0.6, 0.4, 0.2), // Brown
                    ),
                    TileType::Road => (
                        road_mesh.clone(),
                        0.1,
                        Color::srgb(0.3, 0.3, 0.3), // Dark gray
                    ),
                    TileType::Water => (
                        water_mesh.clone(),
                        0.05,
                        Color::srgb(0.2, 0.6, 1.0), // Blue
                    ),
                    TileType::GreenSpace => (
                        road_mesh.clone(),
                        0.2,
                        Color::srgb(0.2, 0.8, 0.2), // Green
                    ),
                    TileType::Railway => (
                        road_mesh.clone(),
                        0.15,
                        Color::srgb(0.5, 0.3, 0.1), // Brown
                    ),
                    TileType::Parking => (
                        road_mesh.clone(),
                        0.05,
                        Color::srgb(0.4, 0.4, 0.4), // Light gray
                    ),
                    TileType::Amenity => (
                        cube_mesh.clone(),
                        1.0,
                        Color::srgb(1.0, 0.6, 0.0), // Orange
                    ),
                    TileType::Tourism => (
                        cube_mesh.clone(),
                        1.5,
                        Color::srgb(1.0, 0.2, 0.8), // Pink
                    ),
                    TileType::Industrial => (
                        building_mesh.clone(),
                        3.0,
                        Color::srgb(0.5, 0.0, 0.5), // Purple
                    ),
                    TileType::Residential => (
                        building_mesh.clone(),
                        1.8,
                        Color::srgb(1.0, 1.0, 0.0), // Yellow
                    ),
                    TileType::Commercial => (
                        building_mesh.clone(),
                        2.5,
                        Color::srgb(1.0, 0.0, 0.0), // Red
                    ),
                    TileType::Custom(_) => (
                        cube_mesh.clone(),
                        0.8,
                        Color::srgb(0.8, 0.8, 0.8), // Light gray
                    ),
                };

                // Calculate world position
                let world_x = (x as f32 - grid_width as f32 / 2.0) * tile_size;
                let world_z = (z as f32 - grid_height as f32 / 2.0) * tile_size;

                // Create material
                let material_handle = {
                    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
                    materials.add(StandardMaterial {
                        base_color: color,
                        metallic: match tile.tile_type {
                            TileType::Water => 0.8,
                            TileType::Building | TileType::Commercial | TileType::Industrial => 0.2,
                            _ => 0.1,
                        },
                        ..default()
                    })
                };

                // Spawn the tile entity
                let entity = world
                    .spawn((
                        Mesh3d(mesh_handle),
                        MeshMaterial3d(material_handle),
                        Transform::from_xyz(world_x, height, world_z),
                        MapTile {
                            tile_type: tile.tile_type.clone(),
                            grid_pos: (x, z),
                        },
                    ))
                    .id();

                rendered_entities.push(entity);
                rendered_count += 1;
            }
        }
    }

    // Update the map state with the new entities
    let mut map_state = world.resource_mut::<MapState>();
    map_state.rendered_entities = rendered_entities;

    println!("✅ Rendered {} 3D tiles", rendered_count);
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
