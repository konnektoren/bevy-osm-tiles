use bevy::prelude::*;

use super::{
    LoadMapMessage, MapLoadFailedMessage, MapLoadProgressMessage, MapLoadedMessage, resources::*,
    systems::*,
};

/// Bevy plugin for loading OpenStreetMap data dynamically
pub struct OsmTilesPlugin {
    default_provider: String,
    max_concurrent_loads: usize,
}

impl OsmTilesPlugin {
    /// Create a new plugin with default settings
    pub fn new() -> Self {
        Self {
            default_provider: "overpass".to_string(),
            max_concurrent_loads: 2,
        }
    }

    /// Use mock provider by default (useful for testing/development)
    pub fn with_mock_provider(mut self) -> Self {
        self.default_provider = "mock".to_string();
        self
    }

    /// Use overpass provider by default
    pub fn with_overpass_provider(mut self) -> Self {
        self.default_provider = "overpass".to_string();
        self
    }

    /// Set maximum concurrent loading operations
    pub fn with_max_concurrent_loads(mut self, max: usize) -> Self {
        self.max_concurrent_loads = max;
        self
    }
}

impl Default for OsmTilesPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for OsmTilesPlugin {
    fn build(&self, app: &mut App) {
        app
            // Resources
            .insert_resource(MapLoadQueue {
                pending: std::collections::VecDeque::new(),
                active: std::collections::HashMap::new(),
                max_concurrent: self.max_concurrent_loads,
            })
            .insert_resource(OsmProviderRegistry {
                providers: std::collections::HashMap::new(),
                default_provider: self.default_provider.clone(),
            })
            // Messages (buffered events)
            .add_message::<LoadMapMessage>()
            .add_message::<MapLoadedMessage>()
            .add_message::<MapLoadFailedMessage>()
            .add_message::<MapLoadProgressMessage>()
            // Systems
            .add_systems(
                Update,
                (
                    handle_load_requests,
                    process_loading_tasks,
                    handle_completed_tasks,
                ),
            )
            // Setup
            .add_systems(Startup, setup_providers);
    }
}
