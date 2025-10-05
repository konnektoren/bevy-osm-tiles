mod integration_tests;
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OsmConfigBuilder;

    #[test]
    fn test_osm_metadata_creation() {
        let metadata = OsmMetadata::new("test-source", "test-provider");

        assert_eq!(metadata.source, "test-source");
        assert_eq!(metadata.provider_type, "test-provider");
        assert!(metadata.element_count.is_none());
        assert!(metadata.processing_time_ms.is_none());
        assert!(metadata.extra.is_empty());
        assert!(!metadata.timestamp.is_empty());
    }

    #[test]
    fn test_osm_metadata_builder() {
        let metadata = OsmMetadata::new("source", "provider")
            .with_element_count(42)
            .with_processing_time(1500)
            .with_extra("key1", "value1")
            .with_extra("key2", "value2");

        assert_eq!(metadata.element_count, Some(42));
        assert_eq!(metadata.processing_time_ms, Some(1500));
        assert_eq!(metadata.extra.get("key1"), Some(&"value1".to_string()));
        assert_eq!(metadata.extra.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_provider_capabilities_default() {
        let capabilities = ProviderCapabilities::default();

        assert!(!capabilities.supports_real_time);
        assert!(!capabilities.requires_network);
        assert!(!capabilities.supports_geocoding);
        assert!(capabilities.max_area_km2.is_none());
        assert_eq!(capabilities.supported_formats, vec![OsmDataFormat::Json]);
        assert!(capabilities.rate_limit_rpm.is_none());
        assert!(capabilities.wasm_compatible);
        assert!(capabilities.notes.is_none());
    }

    #[test]
    fn test_provider_factory_available_providers() {
        let providers = ProviderFactory::available_providers();
        assert_eq!(providers, vec!["overpass", "mock"]);
    }

    #[test]
    fn test_provider_factory_create_overpass() {
        let provider = ProviderFactory::overpass();
        assert_eq!(provider.provider_type(), "overpass");
    }

    #[test]
    fn test_provider_factory_create_overpass_with_url() {
        let custom_url = "https://custom.overpass.api/interpreter";
        let provider = ProviderFactory::overpass_with_url(custom_url);
        assert_eq!(provider.base_url, custom_url);
    }

    #[test]
    fn test_provider_factory_create_mock() {
        let provider = ProviderFactory::mock();
        assert_eq!(provider.provider_type(), "mock");
    }

    #[test]
    fn test_provider_factory_create_mock_with_data() {
        let custom_data = r#"{"custom": "data"}"#;
        let provider = ProviderFactory::mock_with_data(custom_data);
        // We can't easily test the internal data without accessing private fields,
        // but we can verify it was created
        assert_eq!(provider.provider_type(), "mock");
    }

    #[test]
    fn test_provider_factory_create_provider_by_name() {
        // Test valid provider names
        let overpass = ProviderFactory::create_provider("overpass").unwrap();
        assert_eq!(overpass.provider_type(), "overpass");

        let mock = ProviderFactory::create_provider("mock").unwrap();
        assert_eq!(mock.provider_type(), "mock");

        // Test invalid provider name
        let result = ProviderFactory::create_provider("invalid");
        assert!(result.is_err());

        if let Err(crate::OsmTilesError::Config(msg)) = result {
            assert!(msg.contains("Unknown provider: 'invalid'"));
            assert!(msg.contains("Available providers:"));
        } else {
            panic!("Expected Config error");
        }
    }

    #[test]
    fn test_osm_data_format_serialization() {
        // Test that formats can be serialized/deserialized
        let xml_format = OsmDataFormat::Xml;
        let json_format = OsmDataFormat::Json;

        let xml_json = serde_json::to_string(&xml_format).unwrap();
        let json_json = serde_json::to_string(&json_format).unwrap();

        assert!(xml_json.contains("Xml"));
        assert!(json_json.contains("Json"));

        let xml_deserialized: OsmDataFormat = serde_json::from_str(&xml_json).unwrap();
        let json_deserialized: OsmDataFormat = serde_json::from_str(&json_json).unwrap();

        assert!(matches!(xml_deserialized, OsmDataFormat::Xml));
        assert!(matches!(json_deserialized, OsmDataFormat::Json));
    }

    #[test]
    fn test_osm_data_serialization() {
        use crate::BoundingBox;

        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        let metadata = OsmMetadata::new("test", "test");
        let osm_data = OsmData {
            raw_data: "test data".to_string(),
            format: OsmDataFormat::Json,
            bounding_box: bbox,
            metadata,
        };

        // Should be serializable
        let json = serde_json::to_string(&osm_data).unwrap();
        assert!(!json.is_empty());

        // Should be deserializable
        let deserialized: OsmData = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.raw_data, "test data");
        assert!(matches!(deserialized.format, OsmDataFormat::Json));
    }

    #[tokio::test]
    async fn test_provider_trait_methods() {
        // Test that we can use providers through the trait
        let provider: Box<dyn OsmDataProvider> = Box::new(ProviderFactory::mock());

        assert_eq!(provider.provider_type(), "mock");

        let capabilities = provider.capabilities();
        assert!(capabilities.wasm_compatible);

        // Test availability
        let availability = provider.test_availability().await;
        assert!(availability.is_ok());

        // Test region resolution
        let region = Region::city("test");
        let bbox = provider.resolve_region(&region).await.unwrap();
        assert!(bbox.contains(52.5, 13.4)); // Should be in the test range
    }

    #[tokio::test]
    async fn test_provider_trait_with_config() {
        let provider: Box<dyn OsmDataProvider> = Box::new(ProviderFactory::mock());
        let config = OsmConfigBuilder::new().city("test").build();

        let result = provider.fetch_data(&config).await.unwrap();

        assert!(!result.raw_data.is_empty());
        assert!(matches!(result.format, OsmDataFormat::Json));
        assert_eq!(result.metadata.provider_type, "mock");
    }
}
