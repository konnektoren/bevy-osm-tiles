use super::MapLoadRequest;
use crate::{OsmDataProvider, ProviderFactory};
use bevy::prelude::*;
use std::collections::{HashMap, VecDeque};

/// Resource managing the map loading queue
#[derive(Resource)]
pub struct MapLoadQueue {
    pub pending: VecDeque<MapLoadRequest>,
    pub active: HashMap<String, Entity>, // city_name -> entity with LoadingTask
    pub max_concurrent: usize,
}

/// Resource managing available OSM data providers
#[derive(Resource)]
pub struct OsmProviderRegistry {
    pub providers: HashMap<String, Box<dyn OsmDataProvider>>,
    pub default_provider: String,
}

impl OsmProviderRegistry {
    /// Get a provider by name
    pub fn get_provider(&self, name: &str) -> Option<&dyn OsmDataProvider> {
        self.providers.get(name).map(|p| p.as_ref())
    }

    /// Get the default provider
    pub fn get_default_provider(&self) -> &dyn OsmDataProvider {
        self.providers
            .get(&self.default_provider)
            .map(|p| p.as_ref())
            .expect("Default provider should always be available")
    }
}

/// Setup the provider registry with default providers
pub fn setup_providers(mut registry: ResMut<OsmProviderRegistry>) {
    // Add available providers
    registry.providers.insert(
        "overpass".to_string(),
        Box::new(ProviderFactory::overpass()),
    );
    registry
        .providers
        .insert("mock".to_string(), Box::new(ProviderFactory::mock()));
}
