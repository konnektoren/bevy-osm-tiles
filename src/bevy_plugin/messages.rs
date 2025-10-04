use crate::TileGrid;
use bevy::prelude::*;

/// Event to request loading a map
#[derive(Message, Clone, Debug)]
pub struct LoadMapMessage {
    pub request: MapLoadRequest,
}

/// Request configuration for loading a map
#[derive(Clone, Debug)]
pub struct MapLoadRequest {
    pub city_name: String,
    pub features: crate::FeatureSet,
    pub grid_resolution: u32,
    pub target_entity: Option<Entity>,
    pub provider_override: Option<String>,
}

impl MapLoadRequest {
    /// Create a new map load request for a city
    pub fn new(city_name: impl Into<String>) -> Self {
        Self {
            city_name: city_name.into(),
            features: crate::FeatureSet::urban(),
            grid_resolution: 200,
            target_entity: None,
            provider_override: None,
        }
    }

    /// Set the features to include in the map
    pub fn with_features(mut self, features: crate::FeatureSet) -> Self {
        self.features = features;
        self
    }

    /// Set the grid resolution
    pub fn with_resolution(mut self, resolution: u32) -> Self {
        self.grid_resolution = resolution;
        self
    }

    /// Associate this request with a specific entity
    pub fn for_entity(mut self, entity: Entity) -> Self {
        self.target_entity = Some(entity);
        self
    }

    /// Override the provider for this request
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider_override = Some(provider.into());
        self
    }
}

/// Event sent when a map has been successfully loaded
#[derive(Message, Debug)]
pub struct MapLoadedMessage {
    pub request: MapLoadRequest,
    pub grid: TileGrid,
    pub entity: Option<Entity>,
}

/// Event sent when map loading fails
#[derive(Message, Debug)]
pub struct MapLoadFailedMessage {
    pub request: MapLoadRequest,
    pub error: String,
}

/// Event sent to report loading progress
#[derive(Message, Debug)]
pub struct MapLoadProgressMessage {
    pub request: MapLoadRequest,
    pub stage: LoadingStage,
    pub progress: f32, // 0.0 to 1.0
}

/// Stages of the loading process
#[derive(Debug, Clone, PartialEq)]
pub enum LoadingStage {
    ResolvingCity,
    FetchingData,
    GeneratingGrid,
    Complete,
}

/// Helper trait for loading maps
pub trait MapLoadingExt {
    /// Request loading a map for a city
    fn load_map(&mut self, city_name: impl Into<String>) -> MapLoadRequest;

    /// Request loading a map with a specific request
    fn load_map_with_request(&mut self, request: MapLoadRequest);
}

impl MapLoadingExt for MessageWriter<'_, LoadMapMessage> {
    fn load_map(&mut self, city_name: impl Into<String>) -> MapLoadRequest {
        let request = MapLoadRequest::new(city_name);
        self.write(LoadMapMessage {
            request: request.clone(),
        });
        request
    }

    fn load_map_with_request(&mut self, request: MapLoadRequest) {
        self.write(LoadMapMessage { request });
    }
}
