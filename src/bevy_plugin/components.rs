use super::{LoadingStage, MapLoadRequest};
use crate::TileGrid;
use bevy::{ecs::world::CommandQueue, prelude::*, tasks::Task};

/// Component to hold loaded map data
#[derive(Component, Debug)]
pub struct MapTiles {
    pub grid: TileGrid,
    pub request: MapLoadRequest,
    pub loaded_at: std::time::Instant,
}

/// Component indicating a map is currently being loaded
#[derive(Component, Debug)]
pub struct MapLoading {
    pub request: MapLoadRequest,
    pub stage: LoadingStage,
    pub progress: f32,
    pub started_at: std::time::Instant,
}

/// Component for async loading task - exactly like the Bevy example
#[derive(Component)]
pub struct LoadingTask {
    pub request: MapLoadRequest,
    pub task: Task<CommandQueue>,
    pub started_at: std::time::Instant,
}
