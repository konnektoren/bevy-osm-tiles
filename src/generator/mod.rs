mod grid_builder;
mod osm_parser;
mod tile_grid;

pub use grid_builder::*;
pub use osm_parser::*;
pub use tile_grid::*;

use crate::{OsmConfig, OsmData, Result};
use async_trait::async_trait;

/// Trait for generating tile grids from OSM data
#[async_trait]
pub trait GridGenerator: Send + Sync {
    /// Generate a tile grid from OSM data
    async fn generate_grid(&self, osm_data: &OsmData, config: &OsmConfig) -> Result<TileGrid>;

    /// Get the generator's capabilities and settings
    fn capabilities(&self) -> GeneratorCapabilities;
}

/// Describes the capabilities and settings of a grid generator
#[derive(Debug, Clone)]
pub struct GeneratorCapabilities {
    /// Maximum grid size supported (width × height)
    pub max_grid_size: Option<(usize, usize)>,
    /// Supported coordinate reference systems
    pub supported_crs: Vec<String>,
    /// Whether the generator supports multi-threading
    pub supports_parallel: bool,
    /// Additional notes about the generator
    pub notes: Option<String>,
}

impl Default for GeneratorCapabilities {
    fn default() -> Self {
        Self {
            max_grid_size: Some((10000, 10000)),
            supported_crs: vec!["EPSG:4326".to_string()], // WGS84
            supports_parallel: false,
            notes: None,
        }
    }
}
