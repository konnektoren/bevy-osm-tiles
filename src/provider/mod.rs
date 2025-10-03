mod mock;
mod overpass;

pub use mock::*;
pub use overpass::*;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{BoundingBox, OsmConfig, Region, Result};

/// Raw OSM data response from a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsmData {
    /// Raw response data (XML or JSON)
    pub raw_data: String,
    /// Format of the data (xml, json)
    pub format: OsmDataFormat,
    /// The bounding box that was actually fetched
    pub bounding_box: BoundingBox,
    /// Metadata about the request
    pub metadata: OsmMetadata,
}

/// Format of OSM data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OsmDataFormat {
    /// OSM XML format
    Xml,
    /// Overpass JSON format
    Json,
}

/// Metadata about an OSM data request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsmMetadata {
    /// Timestamp when data was fetched
    pub timestamp: String,
    /// Data source (e.g., "overpass-api.de", "mock")
    pub source: String,
    /// Provider type identifier
    pub provider_type: String,
    /// Number of elements returned
    pub element_count: Option<u32>,
    /// Processing time in milliseconds
    pub processing_time_ms: Option<u64>,
    /// Additional metadata from the API/source
    pub extra: HashMap<String, String>,
}

impl OsmMetadata {
    /// Create new metadata with basic information
    pub fn new(source: impl Into<String>, provider_type: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            source: source.into(),
            provider_type: provider_type.into(),
            element_count: None,
            processing_time_ms: None,
            extra: HashMap::new(),
        }
    }

    /// Set the number of elements
    pub fn with_element_count(mut self, count: u32) -> Self {
        self.element_count = Some(count);
        self
    }

    /// Set the processing time
    pub fn with_processing_time(mut self, ms: u64) -> Self {
        self.processing_time_ms = Some(ms);
        self
    }

    /// Add extra metadata
    pub fn with_extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }
}

/// Trait for providing OpenStreetMap data from various WASM-compatible sources
///
/// This trait abstracts the data source, allowing for different implementations
/// such as HTTP APIs, in-memory data, or mock data for testing.
#[async_trait]
pub trait OsmDataProvider: Send + Sync {
    /// Get the provider type identifier (e.g., "overpass", "mock")
    fn provider_type(&self) -> &'static str;

    /// Fetch OSM data for the given configuration
    ///
    /// Implementations should:
    /// - Resolve regions to bounding boxes if needed
    /// - Make appropriate API calls or retrieve cached data
    /// - Handle rate limiting and retries where applicable
    /// - Return structured OSM data with proper metadata
    async fn fetch_data(&self, config: &OsmConfig) -> Result<OsmData>;

    /// Resolve a region to a concrete bounding box
    ///
    /// For city names, this typically involves geocoding.
    /// For other region types, this may involve coordinate transformation.
    async fn resolve_region(&self, region: &Region) -> Result<BoundingBox>;

    /// Test connectivity/availability of the data source
    ///
    /// This might ping an API, check cache status, or validate configuration
    async fn test_availability(&self) -> Result<()>;

    /// Get provider-specific capabilities and limitations
    fn capabilities(&self) -> ProviderCapabilities;
}

/// Describes the capabilities and limitations of a data provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    /// Whether this provider supports real-time data fetching
    pub supports_real_time: bool,
    /// Whether this provider requires internet connectivity
    pub requires_network: bool,
    /// Whether this provider supports geocoding (city name resolution)
    pub supports_geocoding: bool,
    /// Maximum recommended bounding box area in square kilometers
    pub max_area_km2: Option<f64>,
    /// Supported data formats
    pub supported_formats: Vec<OsmDataFormat>,
    /// Rate limiting information (requests per minute)
    pub rate_limit_rpm: Option<u32>,
    /// Whether this provider works in WASM environments
    pub wasm_compatible: bool,
    /// Additional notes about the provider
    pub notes: Option<String>,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            supports_real_time: false,
            requires_network: false,
            supports_geocoding: false,
            max_area_km2: None,
            supported_formats: vec![OsmDataFormat::Json],
            rate_limit_rpm: None,
            wasm_compatible: true,
            notes: None,
        }
    }
}

/// Provider factory for creating different types of WASM-compatible OSM data providers
pub struct ProviderFactory;

impl ProviderFactory {
    /// Create an Overpass API provider with default settings
    pub fn overpass() -> OverpassProvider {
        OverpassProvider::new()
    }

    /// Create an Overpass API provider with custom endpoint
    pub fn overpass_with_url(url: impl Into<String>) -> OverpassProvider {
        OverpassProvider::with_base_url(url)
    }

    /// Create a mock provider for testing
    pub fn mock() -> MockProvider {
        MockProvider::new()
    }

    /// Create a mock provider with predefined data
    pub fn mock_with_data(data: impl Into<String>) -> MockProvider {
        MockProvider::with_data(data)
    }

    /// Create a mock provider that simulates network conditions
    pub fn mock_with_delay(delay_ms: u64) -> MockProvider {
        MockProvider::new().with_delay(std::time::Duration::from_millis(delay_ms))
    }

    /// Get a list of all available provider types
    pub fn available_providers() -> Vec<&'static str> {
        vec!["overpass", "mock"]
    }

    /// Create a provider by name with default settings
    pub fn create_provider(name: &str) -> Result<Box<dyn OsmDataProvider>> {
        match name {
            "overpass" => Ok(Box::new(Self::overpass())),
            "mock" => Ok(Box::new(Self::mock())),
            _ => Err(crate::OsmTilesError::Config(format!(
                "Unknown provider: '{}'. Available providers: {:?}",
                name,
                Self::available_providers()
            ))),
        }
    }
}
