use super::super::resources::{MapLoadQueue, OsmProviderRegistry};
use super::super::{
    LoadingStage, LoadingTask, MapLoadFailedMessage, MapLoadProgressMessage, MapLoadedMessage,
    MapLoading, MapTiles,
};
use crate::{DefaultGridGenerator, GridGenerator, OsmConfigBuilder, ProviderFactory};
use bevy::{
    ecs::{system::SystemState, world::CommandQueue},
    prelude::*,
    tasks::AsyncComputeTaskPool,
};

/// System to start new loading tasks using Bevy's AsyncComputeTaskPool
pub fn process_loading_tasks(
    mut queue: ResMut<MapLoadQueue>,
    registry: Res<OsmProviderRegistry>,
    mut progress_events: MessageWriter<MapLoadProgressMessage>,
    mut commands: Commands,
) {
    let thread_pool = AsyncComputeTaskPool::get();

    // Start new tasks if we have capacity
    while queue.active.len() < queue.max_concurrent && !queue.pending.is_empty() {
        if let Some(request) = queue.pending.pop_front() {
            let city_key = request.city_name.clone();

            // Skip if already loading this city
            if queue.active.contains_key(&city_key) {
                continue;
            }

            // Get provider type
            let provider_name = request
                .provider_override
                .as_ref()
                .unwrap_or(&registry.default_provider);

            let provider_type = provider_name.clone();
            let request_clone = request.clone();

            // Spawn new task on the AsyncComputeTaskPool - exactly like Bevy example
            let task = thread_pool.spawn(async move {
                // Do the async work
                let result = load_map_async(request_clone.clone(), provider_type).await;

                let mut command_queue = CommandQueue::default();

                // Use a raw command queue to pass results back to be applied in a deferred manner
                command_queue.push(move |world: &mut World| {
                    // Create a system state to access the ECS resources we need
                    let mut system_state = SystemState::<(
                        MessageWriter<MapLoadedMessage>,
                        MessageWriter<MapLoadFailedMessage>,
                        Query<&mut MapLoading>,
                        Commands,
                    )>::new(world);

                    let (mut loaded_events, mut failed_events, mut loading_query, mut commands) =
                        system_state.get_mut(world);

                    match result {
                        Ok(grid) => {
                            // Send loaded event
                            loaded_events.write(MapLoadedMessage {
                                request: request_clone.clone(),
                                grid: grid.clone(),
                                entity: request_clone.target_entity,
                            });

                            // Update entity if specified
                            if let Some(target_entity) = request_clone.target_entity {
                                if loading_query.get_mut(target_entity).is_ok() {
                                    commands
                                        .entity(target_entity)
                                        .remove::<MapLoading>()
                                        .insert(MapTiles {
                                            grid,
                                            request: request_clone,
                                            #[cfg(not(target_arch = "wasm32"))]
                                            loaded_at: std::time::Instant::now(),
                                        });
                                }
                            }
                        }
                        Err(error) => {
                            // Send failed event
                            failed_events.write(MapLoadFailedMessage {
                                request: request_clone.clone(),
                                error: error.to_string(),
                            });

                            // Remove loading component from entity
                            if let Some(target_entity) = request_clone.target_entity {
                                commands.entity(target_entity).remove::<MapLoading>();
                            }
                        }
                    }
                });

                command_queue
            });

            // Spawn entity to track the task
            let task_entity = commands
                .spawn(LoadingTask {
                    request: request.clone(),
                    task,
                    #[cfg(not(target_arch = "wasm32"))]
                    started_at: std::time::Instant::now(),
                })
                .id();

            queue.active.insert(city_key, task_entity);

            progress_events.write(MapLoadProgressMessage {
                request,
                stage: LoadingStage::ResolvingCity,
                progress: 0.0,
            });
        }
    }
}

/// Async loading function - uses only the providers that are already WASM-compatible
async fn load_map_async(
    request: super::super::MapLoadRequest,
    provider_type: String,
) -> crate::Result<crate::TileGrid> {
    // Create provider (this is cheap, providers are stateless)
    let provider: Box<dyn crate::OsmDataProvider> = match provider_type.as_str() {
        "overpass" => Box::new(ProviderFactory::overpass()),
        "mock" => Box::new(ProviderFactory::mock()),
        _ => Box::new(ProviderFactory::mock()), // fallback
    };

    // Build config from request
    let config = OsmConfigBuilder::new()
        .city(&request.city_name)
        .features(request.features)
        .grid_resolution(request.grid_resolution)
        .build();

    // Fetch OSM data - this uses reqwest with wasm features, which is WASM-compatible
    let osm_data = provider.fetch_data(&config).await?;

    // Generate grid - this is pure computation
    let generator = DefaultGridGenerator::new();
    let grid = generator.generate_grid(&osm_data, &config).await?;

    Ok(grid)
}
