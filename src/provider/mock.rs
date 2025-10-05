use async_trait::async_trait;

use super::{OsmData, OsmDataProvider, ProviderCapabilities};
use crate::{BoundingBox, OsmConfig, OsmDataFormat, OsmMetadata, OsmTilesError, Region, Result};

/// WASM-compatible mock provider for testing and development
///
/// This provider works in all environments including browsers and provides
/// predictable test data for development and testing scenarios.
pub struct MockProvider {
    /// Predefined data to return
    mock_data: String,
    /// Whether to simulate failures
    simulate_failure: bool,
}

impl MockProvider {
    /// Create a new mock provider with default test data
    pub fn new() -> Self {
        Self::with_data(Self::default_test_data())
    }

    /// Create a mock provider with custom data
    pub fn with_data(data: impl Into<String>) -> Self {
        Self {
            mock_data: data.into(),
            simulate_failure: false,
        }
    }

    /// Configure the provider to simulate failures
    pub fn with_failure(mut self) -> Self {
        self.simulate_failure = true;
        self
    }

    /// Get default test data with various OSM features
    fn default_test_data() -> String {
        r#"{
  "version": 0.6,
  "generator": "Mock Provider v1.0",
  "elements": [
    {
      "type": "way",
      "id": 123456789,
      "nodes": [1001, 1002, 1003, 1001],
      "tags": {
        "building": "residential",
        "addr:street": "Mock Street",
        "addr:housenumber": "42"
      },
      "geometry": [
        {"lat": 52.5, "lon": 13.4},
        {"lat": 52.501, "lon": 13.4},
        {"lat": 52.501, "lon": 13.401},
        {"lat": 52.5, "lon": 13.401},
        {"lat": 52.5, "lon": 13.4}
      ]
    },
    {
      "type": "way",
      "id": 987654321,
      "nodes": [2001, 2002],
      "tags": {
        "highway": "residential",
        "name": "Mock Street"
      },
      "geometry": [
        {"lat": 52.499, "lon": 13.399},
        {"lat": 52.502, "lon": 13.402}
      ]
    },
    {
      "type": "way",
      "id": 555666777,
      "nodes": [3001, 3002, 3003, 3004, 3001],
      "tags": {
        "leisure": "park",
        "name": "Mock Park"
      },
      "geometry": [
        {"lat": 52.503, "lon": 13.403},
        {"lat": 52.504, "lon": 13.403},
        {"lat": 52.504, "lon": 13.405},
        {"lat": 52.503, "lon": 13.405},
        {"lat": 52.503, "lon": 13.403}
      ]
    },
    {
      "type": "node",
      "id": 4001,
      "lat": 52.5015,
      "lon": 13.4015,
      "tags": {
        "amenity": "cafe",
        "name": "Mock Cafe"
      }
    }
  ]
}"#
        .to_string()
    }
}

#[async_trait]
impl OsmDataProvider for MockProvider {
    fn provider_type(&self) -> &'static str {
        "mock"
    }

    async fn fetch_data(&self, config: &OsmConfig) -> Result<OsmData> {
        // Simulate failure if configured
        if self.simulate_failure {
            return Err(OsmTilesError::Network(crate::NetworkError::Connection {
                message: "Simulated network failure".to_string(),
            }));
        }

        let bbox = self.resolve_region(&config.region).await?;

        let metadata = OsmMetadata::new("mock-provider", self.provider_type())
            .with_element_count(4) // Matches the default test data
            .with_processing_time(1)
            .with_extra("simulated", "true")
            .with_extra("wasm_compatible", "true")
            .with_extra("test_data", "true");

        tracing::debug!(
            "Mock provider returning {} bytes of test data",
            self.mock_data.len()
        );

        Ok(OsmData {
            raw_data: self.mock_data.clone(),
            format: OsmDataFormat::Json,
            bounding_box: bbox,
            metadata,
        })
    }

    async fn resolve_region(&self, region: &Region) -> Result<BoundingBox> {
        match region {
            Region::BoundingBox(bbox) => Ok(bbox.clone()),
            Region::CenterRadius {
                lat,
                lon,
                radius_km,
            } => {
                // Simple approximation for testing
                let delta = radius_km / 111.0; // Rough degrees per km
                Ok(BoundingBox::new(
                    lat - delta,
                    lon - delta,
                    lat + delta,
                    lon + delta,
                ))
            }
            Region::City { name } => {
                // Mock geocoding for common test cities
                let bbox = match name.to_lowercase().as_str() {
                    "berlin" => BoundingBox::new(52.3, 13.0, 52.7, 13.8),
                    "munich" | "münchen" => BoundingBox::new(48.0, 11.3, 48.3, 11.8),
                    "hamburg" => BoundingBox::new(53.4, 9.7, 53.8, 10.3),
                    "test" | "testcity" | "mock" => BoundingBox::new(52.4, 13.3, 52.6, 13.5),
                    _ => {
                        return Err(OsmTilesError::Geographic(format!(
                            "Mock provider doesn't know city: '{}'. Try: berlin, munich, hamburg, or test",
                            name
                        )));
                    }
                };
                Ok(bbox)
            }
        }
    }

    async fn test_availability(&self) -> Result<()> {
        if self.simulate_failure {
            Err(OsmTilesError::Geographic(
                "Mock failure enabled".to_string(),
            ))
        } else {
            tracing::debug!("Mock provider is always available");
            Ok(())
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_real_time: false,
            requires_network: false,
            supports_geocoding: true,
            max_area_km2: None, // No limits for mock data
            supported_formats: vec![OsmDataFormat::Json],
            rate_limit_rpm: None,
            wasm_compatible: true,
            notes: Some(
                "Mock provider for testing. Works in all environments including WASM.".to_string(),
            ),
        }
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::NetworkError;
    use crate::{FeatureSet, OsmConfigBuilder, OsmDataFormat};

    #[tokio::test]
    async fn test_mock_provider_basic() {
        let provider = MockProvider::new();
        assert_eq!(provider.provider_type(), "mock");

        let capabilities = provider.capabilities();
        assert!(!capabilities.supports_real_time);
        assert!(!capabilities.requires_network);
        assert!(capabilities.supports_geocoding);
        assert!(capabilities.wasm_compatible);
        assert!(capabilities.max_area_km2.is_none());
    }

    #[tokio::test]
    async fn test_mock_provider_with_custom_data() {
        let custom_data = r#"{"elements": [{"type": "node", "id": 1, "lat": 50.0, "lon": 8.0}]}"#;
        let provider = MockProvider::with_data(custom_data);

        let config = OsmConfigBuilder::new().city("test").build();

        let result = provider.fetch_data(&config).await.unwrap();
        assert_eq!(result.raw_data, custom_data);
        assert_eq!(result.format, OsmDataFormat::Json);
        assert_eq!(result.metadata.provider_type, "mock");
    }

    #[tokio::test]
    async fn test_mock_provider_with_failure() {
        let provider = MockProvider::new().with_failure();

        let config = OsmConfigBuilder::new().city("test").build();

        let result = provider.fetch_data(&config).await;
        assert!(result.is_err());

        if let Err(OsmTilesError::Network(NetworkError::Connection { message })) = result {
            assert_eq!(message, "Simulated network failure");
        } else {
            panic!("Expected NetworkError::Connection");
        }
    }

    #[tokio::test]
    async fn test_mock_provider_region_resolution() {
        let provider = MockProvider::new();

        // Test bounding box region
        let bbox_region = Region::bbox(52.0, 13.0, 53.0, 14.0);
        let result = provider.resolve_region(&bbox_region).await.unwrap();
        assert_eq!(result.south, 52.0);
        assert_eq!(result.west, 13.0);
        assert_eq!(result.north, 53.0);
        assert_eq!(result.east, 14.0);

        // Test center radius region
        let center_region = Region::center_radius(52.5, 13.4, 5.0);
        let result = provider.resolve_region(&center_region).await.unwrap();
        assert!(result.contains(52.5, 13.4)); // Should contain center point

        // Test known cities
        let berlin_region = Region::city("berlin");
        let result = provider.resolve_region(&berlin_region).await.unwrap();
        assert!(result.contains(52.5, 13.4)); // Berlin coordinates

        let munich_region = Region::city("munich");
        let result = provider.resolve_region(&munich_region).await.unwrap();
        assert!(result.contains(48.1, 11.5)); // Munich coordinates

        // Test unknown city
        let unknown_region = Region::city("unknown_city");
        let result = provider.resolve_region(&unknown_region).await;
        assert!(result.is_err());

        if let Err(OsmTilesError::Geographic(msg)) = result {
            assert!(msg.contains("Mock provider doesn't know city"));
        } else {
            panic!("Expected Geographic error");
        }
    }

    #[tokio::test]
    async fn test_mock_provider_test_availability() {
        // Normal provider should be available
        let provider = MockProvider::new();
        assert!(provider.test_availability().await.is_ok());

        // Provider with failure should not be available
        let failing_provider = MockProvider::new().with_failure();
        let result = failing_provider.test_availability().await;
        assert!(result.is_err());

        if let Err(OsmTilesError::Geographic(msg)) = result {
            assert_eq!(msg, "Mock failure enabled");
        } else {
            panic!("Expected Geographic error");
        }
    }

    #[tokio::test]
    async fn test_mock_provider_metadata() {
        let provider = MockProvider::new();
        let config = OsmConfigBuilder::new()
            .city("test")
            .features(FeatureSet::urban())
            .build();

        let result = provider.fetch_data(&config).await.unwrap();
        let metadata = result.metadata;

        assert_eq!(metadata.provider_type, "mock");
        assert_eq!(metadata.source, "mock-provider");
        assert_eq!(metadata.element_count, Some(4));
        assert!(metadata.processing_time_ms.is_some());
        assert_eq!(metadata.extra.get("simulated"), Some(&"true".to_string()));
        assert_eq!(
            metadata.extra.get("wasm_compatible"),
            Some(&"true".to_string())
        );
        assert_eq!(metadata.extra.get("test_data"), Some(&"true".to_string()));
    }

    #[tokio::test]
    async fn test_mock_provider_default_test_data() {
        let provider = MockProvider::new();
        let config = OsmConfigBuilder::new().city("test").build();

        let result = provider.fetch_data(&config).await.unwrap();

        // Verify the data is valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&result.raw_data).unwrap();
        assert!(parsed.get("elements").is_some());

        // Should have the expected test elements
        let elements = parsed["elements"].as_array().unwrap();
        assert_eq!(elements.len(), 4);

        // Check for expected element types
        let element_types: Vec<&str> = elements
            .iter()
            .filter_map(|e| e.get("type").and_then(|t| t.as_str()))
            .collect();
        assert!(element_types.contains(&"way"));
        assert!(element_types.contains(&"node"));
    }
}
