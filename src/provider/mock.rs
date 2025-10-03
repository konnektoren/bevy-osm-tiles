use async_trait::async_trait;
use std::time::Duration;

use super::{OsmData, OsmDataProvider, ProviderCapabilities};
use crate::{BoundingBox, OsmConfig, OsmDataFormat, OsmMetadata, OsmTilesError, Region, Result};

/// WASM-compatible mock provider for testing and development
///
/// This provider works in all environments including browsers and provides
/// predictable test data for development and testing scenarios.
pub struct MockProvider {
    /// Predefined data to return
    mock_data: String,
    /// Simulated delay for network requests
    simulated_delay: Option<Duration>,
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
            simulated_delay: None,
            simulate_failure: false,
        }
    }

    /// Add a simulated network delay (useful for testing loading states)
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.simulated_delay = Some(delay);
        self
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
        // Simulate network delay if configured
        if let Some(delay) = self.simulated_delay {
            tracing::debug!("Simulating network delay: {:?}", delay);

            // Use platform-appropriate sleep
            #[cfg(target_arch = "wasm32")]
            {
                // In WASM, we need to use a WASM-compatible sleep
                let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
                    web_sys::window()
                        .unwrap()
                        .set_timeout_with_callback_and_timeout_and_arguments_0(
                            &resolve,
                            delay.as_millis() as i32,
                        )
                        .unwrap();
                };
                let promise = js_sys::Promise::new(&mut cb);
                wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
            }

            #[cfg(not(target_arch = "wasm32"))]
            {
                //tokio::time::sleep(delay).await;
            }
        }

        // Simulate failure if configured
        if self.simulate_failure {
            return Err(OsmTilesError::Network(crate::NetworkError::Connection {
                message: "Simulated network failure".to_string(),
            }));
        }

        let bbox = self.resolve_region(&config.region).await?;

        let metadata = OsmMetadata::new("mock-provider", self.provider_type())
            .with_element_count(4) // Matches the default test data
            .with_processing_time(
                self.simulated_delay
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(1),
            )
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
