use super::super::resources::HttpLoadingState;
use super::super::{MapLoadFailedMessage, MapLoadedMessage, MapLoading, MapTiles};
use bevy::prelude::*;

/// System to process completed HTTP requests and send appropriate events
pub fn process_http_loading_state(
    mut http_state: ResMut<HttpLoadingState>,
    mut loaded_events: MessageWriter<MapLoadedMessage>,
    mut failed_events: MessageWriter<MapLoadFailedMessage>,
    mut loading_query: Query<&mut MapLoading>,
    mut commands: Commands,
) {
    let completed_requests = http_state.take_completed_requests();

    for (request, result) in completed_requests {
        match result {
            Ok(grid) => {
                // Send loaded event
                loaded_events.write(MapLoadedMessage {
                    request: request.clone(),
                    grid: grid.clone(),
                    entity: request.target_entity,
                });

                // Update entity if specified
                if let Some(target_entity) = request.target_entity {
                    if loading_query.get_mut(target_entity).is_ok() {
                        commands
                            .entity(target_entity)
                            .remove::<MapLoading>()
                            .insert(MapTiles {
                                grid,
                                request,
                                #[cfg(not(target_arch = "wasm32"))]
                                loaded_at: std::time::Instant::now(),
                            });
                    }
                }
            }
            Err(error) => {
                // Send failed event
                failed_events.write(MapLoadFailedMessage {
                    request: request.clone(),
                    error,
                });

                // Remove loading component from entity
                if let Some(target_entity) = request.target_entity {
                    commands.entity(target_entity).remove::<MapLoading>();
                }
            }
        }
    }
}

/// System to clean up old HTTP requests to prevent memory leaks
pub fn cleanup_old_http_requests(mut http_state: ResMut<HttpLoadingState>) {
    // Clean up requests older than 5 minutes
    http_state.cleanup_old_requests(300);
}
