mod builder;
mod features;
mod region;

pub use builder::*;
pub use features::*;
pub use region::*;

use serde::{Deserialize, Serialize};

/// Configuration for OSM data download and grid generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsmConfig {
    /// The geographic region to download data for
    pub region: Region,
    /// Grid resolution (cells per degree)
    pub grid_resolution: u32,
    /// Size of each tile in the final grid (in meters, approximately)
    pub tile_size: f32,
    /// Maximum timeout for download requests (in seconds)
    pub timeout_seconds: u64,
    /// Features to include in the grid generation
    pub features: FeatureSet,
}

impl Default for OsmConfig {
    fn default() -> Self {
        Self {
            region: Region::city("Berlin"),
            grid_resolution: 100,
            tile_size: 10.0,
            timeout_seconds: 30,
            features: FeatureSet::default(),
        }
    }
}

impl OsmConfig {
    /// Create a new configuration for a city
    pub fn for_city(city_name: impl Into<String>) -> Self {
        Self {
            region: Region::city(city_name),
            ..Default::default()
        }
    }

    /// Set the grid resolution (cells per degree)
    pub fn with_grid_resolution(mut self, resolution: u32) -> Self {
        self.grid_resolution = resolution;
        self
    }

    /// Set the tile size in meters
    pub fn with_tile_size(mut self, size: f32) -> Self {
        self.tile_size = size;
        self
    }

    /// Set the request timeout
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    /// Configure which features to include
    pub fn with_features(mut self, features: FeatureSet) -> Self {
        self.features = features;
        self
    }

    /// Create a builder for more complex configuration
    pub fn builder() -> OsmConfigBuilder {
        OsmConfigBuilder::new()
    }
}
