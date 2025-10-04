use super::super::resources::MapLoadQueue;
use super::super::{LoadMapMessage, LoadingStage, MapLoading};
use bevy::prelude::*;

/// System to handle new map load requests
pub fn handle_load_requests(
    mut load_events: MessageReader<LoadMapMessage>,
    mut queue: ResMut<MapLoadQueue>,
    mut commands: Commands,
) {
    for event in load_events.read() {
        let request = event.request.clone();

        // Add loading component to target entity if specified
        if let Some(entity) = request.target_entity {
            commands.entity(entity).insert(MapLoading {
                request: request.clone(),
                stage: LoadingStage::ResolvingCity,
                progress: 0.0,
                started_at: std::time::Instant::now(),
            });
        }

        // Queue the request
        queue.pending.push_back(request);
    }
}
