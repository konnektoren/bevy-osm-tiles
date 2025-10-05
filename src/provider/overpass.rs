use async_trait::async_trait;
use geo::{Destination, Haversine, Point};
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use super::{OsmData, OsmDataProvider, ProviderCapabilities};
use crate::http::{HttpClient, HttpConfig, HttpError};
use crate::{
    BoundingBox, NetworkError, OsmConfig, OsmDataFormat, OsmMetadata, OsmTilesError, Region, Result,
};

/// WASM-compatible HTTP-based provider using the Overpass API
pub struct OverpassProvider {
    pub base_url: String,
    http_client: Arc<dyn HttpClient>,
    custom_timeout: Option<u64>, // Changed from Duration to u64
}

impl OverpassProvider {
    /// Create a new Overpass API provider with default client
    pub fn new() -> Self {
        Self::with_base_url("https://overpass-api.de/api/interpreter")
    }

    /// Create a new provider with a custom Overpass API endpoint
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let http_client = crate::http::create_default_client()
            .expect("Failed to create HTTP client - check that either 'reqwest-client' or 'ehttp-client' feature is enabled");

        Self {
            base_url: base_url.into(),
            http_client,
            custom_timeout: None,
        }
    }

    /// Create a new provider with custom configuration
    pub fn with_config(base_url: impl Into<String>, config: HttpConfig) -> Self {
        let http_client = crate::http::create_client_with_config(config)
            .expect("Failed to create HTTP client with config");

        Self {
            base_url: base_url.into(),
            http_client,
            custom_timeout: None,
        }
    }

    /// Create a provider with a custom HTTP client
    pub fn with_http_client(base_url: impl Into<String>, http_client: Arc<dyn HttpClient>) -> Self {
        Self {
            base_url: base_url.into(),
            http_client,
            custom_timeout: None,
        }
    }

    /// Create a provider optimized for WASM environments
    #[cfg(feature = "ehttp-client")]
    pub fn for_wasm() -> Self {
        let config = HttpConfig::default();
        let http_client = Arc::new(crate::http::EhttpClient::with_config(config));

        Self {
            base_url: "https://overpass-api.de/api/interpreter".to_string(),
            http_client,
            custom_timeout: None,
        }
    }

    /// Create a provider optimized for native environments
    #[cfg(feature = "reqwest-client")]
    pub fn for_native() -> Self {
        let config = HttpConfig::default();
        let http_client = Arc::new(
            crate::http::ReqwestClient::with_config(config)
                .expect("Failed to create reqwest client"),
        );

        Self {
            base_url: "https://overpass-api.de/api/interpreter".to_string(),
            http_client,
            custom_timeout: None,
        }
    }

    /// Set a custom timeout for requests
    pub fn with_timeout_secs(mut self, timeout_seconds: u64) -> Self {
        self.custom_timeout = Some(timeout_seconds);
        self
    }

    /// Build an Overpass QL query for the given bounding box and features
    fn build_overpass_query(&self, bbox: &BoundingBox, config: &OsmConfig) -> String {
        let bbox_str = format!("{},{},{},{}", bbox.south, bbox.west, bbox.north, bbox.east);

        let timeout = self.custom_timeout.unwrap_or(config.timeout_seconds);

        let mut query = format!("[out:json][timeout:{}];\n(\n", timeout);

        // Get all OSM tag queries from the feature set
        let tag_queries = config.features.to_osm_queries();

        for tag_query in tag_queries {
            // Build the filter string
            let filter = match &tag_query.value {
                Some(value) => format!("[\"{}\"][\"{}\"]", tag_query.key, value),
                None => format!("[\"{}\"]", tag_query.key),
            };

            // Add way queries
            query.push_str(&format!("  way{}({});\n", filter, bbox_str));

            // Add relation queries for some feature types that commonly use relations
            if self.should_include_relations(&tag_query.key) {
                query.push_str(&format!("  relation{}({});\n", filter, bbox_str));
            }

            // Add node queries for specific features like amenities
            if self.should_include_nodes(&tag_query.key) {
                query.push_str(&format!("  node{}({});\n", filter, bbox_str));
            }
        }

        query.push_str(");\nout geom;");
        query
    }

    /// Determine if relations should be included for a given OSM key
    fn should_include_relations(&self, key: &str) -> bool {
        matches!(
            key,
            "building" | "natural" | "landuse" | "leisure" | "boundary" | "waterway"
        )
    }

    /// Determine if nodes should be included for a given OSM key
    fn should_include_nodes(&self, key: &str) -> bool {
        matches!(key, "amenity" | "tourism" | "power")
    }

    /// Convert a radius in kilometers to a bounding box around a center point
    fn radius_to_bbox(center_lat: f64, center_lon: f64, radius_km: f64) -> BoundingBox {
        let center = Point::new(center_lon, center_lat); // Point uses (lon, lat)
        let distance_meters = radius_km * 1000.0;

        // Calculate the four corner points by moving in cardinal directions
        let north_point = Haversine.destination(center, 0.0, distance_meters);
        let south_point = Haversine.destination(center, 180.0, distance_meters);
        let east_point = Haversine.destination(center, 90.0, distance_meters);
        let west_point = Haversine.destination(center, 270.0, distance_meters);

        BoundingBox::new(
            south_point.y(), // latitude
            west_point.x(),  // longitude
            north_point.y(), // latitude
            east_point.x(),  // longitude
        )
    }

    /// Parse element count from Overpass JSON response
    fn parse_element_count(json_data: &str) -> Option<u32> {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_data) {
            if let Some(elements) = value.get("elements").and_then(|e| e.as_array()) {
                return Some(elements.len() as u32);
            }
        }
        None
    }

    /// Convert HTTP error to our network error
    fn convert_http_error(err: HttpError) -> NetworkError {
        match err {
            HttpError::RequestFailed { message } => NetworkError::Connection { message },
            HttpError::HttpStatus { status } => NetworkError::HttpError { status },
            HttpError::Timeout { seconds } => NetworkError::Timeout { seconds },
            HttpError::Network { message } => NetworkError::Connection { message },
        }
    }
}

#[async_trait]
impl OsmDataProvider for OverpassProvider {
    fn provider_type(&self) -> &'static str {
        "overpass"
    }

    async fn fetch_data(&self, config: &OsmConfig) -> Result<OsmData> {
        // Conditional timing for non-WASM targets
        #[cfg(not(target_arch = "wasm32"))]
        let start_time = Instant::now();

        tracing::info!(
            "Fetching OSM data via Overpass API with config: {:?}",
            config
        );

        // Resolve the region to a bounding box
        let bbox = self.resolve_region(&config.region).await?;
        tracing::debug!("Resolved region to bounding box: {:?}", bbox);

        // Validate bounding box size for Overpass API limits
        let area_km2 = bbox.area_km2();
        if area_km2 > 1000.0 {
            tracing::warn!(
                "Large area requested: {:.2} km² - this may take a while or fail",
                area_km2
            );
        }
        if area_km2 > 5000.0 {
            return Err(OsmTilesError::Config(format!(
                "Area too large: {:.2} km². Overpass API typically limits requests to ~1000 km²",
                area_km2
            )));
        }

        // Build the Overpass query
        let query = self.build_overpass_query(&bbox, config);
        tracing::debug!("Overpass query: {}", query);

        // Make the HTTP request using our trait
        let response = self
            .http_client
            .post_form(&self.base_url, &[("data", &query)])
            .await
            .map_err(Self::convert_http_error)?;

        if response.status != 200 {
            return Err(OsmTilesError::Network(NetworkError::HttpError {
                status: response.status,
            }));
        }

        let raw_data = response.body;

        // Calculate processing time conditionally
        let processing_time = {
            #[cfg(not(target_arch = "wasm32"))]
            {
                start_time.elapsed().as_millis() as u64
            }
            #[cfg(target_arch = "wasm32")]
            {
                1u64 // Default value for WASM
            }
        };

        let element_count = Self::parse_element_count(&raw_data);

        let mut metadata = OsmMetadata::new(&self.base_url, self.provider_type())
            .with_processing_time(processing_time);

        if let Some(count) = element_count {
            metadata = metadata.with_element_count(count);
        }

        metadata = metadata
            .with_extra("query_size", raw_data.len().to_string())
            .with_extra("area_km2", format!("{:.2}", area_km2))
            .with_extra(
                "bbox",
                format!("{},{},{},{}", bbox.south, bbox.west, bbox.north, bbox.east),
            )
            .with_extra("http_client", "trait_based");

        // Conditional logging with timing info
        #[cfg(not(target_arch = "wasm32"))]
        tracing::info!(
            "Successfully fetched OSM data: {} elements, {:.2} KB, {:.1}s",
            element_count.unwrap_or(0),
            raw_data.len() as f64 / 1024.0,
            processing_time as f64 / 1000.0
        );

        #[cfg(target_arch = "wasm32")]
        tracing::info!(
            "Successfully fetched OSM data: {} elements, {:.2} KB",
            element_count.unwrap_or(0),
            raw_data.len() as f64 / 1024.0,
        );

        Ok(OsmData {
            raw_data,
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
            } => Ok(Self::radius_to_bbox(*lat, *lon, *radius_km)),

            Region::City { name } => {
                tracing::debug!("Geocoding city: {}", name);

                let nominatim_url = format!(
                    "https://nominatim.openstreetmap.org/search?q={}&format=json&limit=1&addressdetails=1",
                    urlencoding::encode(name)
                );

                let response = self
                    .http_client
                    .get(&nominatim_url)
                    .await
                    .map_err(Self::convert_http_error)?;

                if response.status != 200 {
                    return Err(OsmTilesError::Network(NetworkError::HttpError {
                        status: response.status,
                    }));
                }

                let geocode_results: Vec<serde_json::Value> = serde_json::from_str(&response.body)
                    .map_err(|e| {
                        OsmTilesError::Parse(format!("Failed to parse geocoding response: {}", e))
                    })?;

                if geocode_results.is_empty() {
                    return Err(OsmTilesError::Geographic(format!(
                        "Could not find city: {}",
                        name
                    )));
                }

                let result = &geocode_results[0];
                let bbox_array = result["boundingbox"].as_array().ok_or_else(|| {
                    OsmTilesError::Geographic(format!("No bounding box found for city: {}", name))
                })?;

                if bbox_array.len() != 4 {
                    return Err(OsmTilesError::Geographic(
                        "Invalid bounding box format from geocoding service".to_string(),
                    ));
                }

                let parse_coord = |idx: usize, coord_type: &str| -> Result<f64> {
                    bbox_array[idx]
                        .as_str()
                        .ok_or_else(|| OsmTilesError::Parse(format!("Invalid {}", coord_type)))?
                        .parse()
                        .map_err(|_| OsmTilesError::Parse(format!("Invalid {} format", coord_type)))
                };

                let south = parse_coord(0, "south latitude")?;
                let north = parse_coord(1, "north latitude")?;
                let west = parse_coord(2, "west longitude")?;
                let east = parse_coord(3, "east longitude")?;

                tracing::debug!(
                    "Geocoded '{}' to bbox: {},{},{},{}",
                    name,
                    south,
                    west,
                    north,
                    east
                );
                Ok(BoundingBox::new(south, west, north, east))
            }
        }
    }

    async fn test_availability(&self) -> Result<()> {
        tracing::debug!("Testing Overpass API availability");

        let test_query = "[out:json][timeout:5];\nnode(0,0,0.001,0.001);\nout;";

        let response = self
            .http_client
            .post_form(&self.base_url, &[("data", test_query)])
            .await
            .map_err(Self::convert_http_error)?;

        if response.status == 200 {
            tracing::debug!("Overpass API is available");
            Ok(())
        } else {
            Err(OsmTilesError::Network(NetworkError::HttpError {
                status: response.status,
            }))
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_real_time: true,
            requires_network: true,
            supports_geocoding: true,
            max_area_km2: Some(1000.0), // Conservative limit for Overpass API
            supported_formats: vec![OsmDataFormat::Json],
            rate_limit_rpm: Some(60), // Conservative estimate
            wasm_compatible: true,
            notes: Some("Trait-based HTTP client for maximum compatibility".to_string()),
        }
    }
}

impl Default for OverpassProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FeatureSet, OsmConfigBuilder, OsmFeature};

    #[test]
    fn test_overpass_provider_basic() {
        let provider = OverpassProvider::new();
        assert_eq!(provider.provider_type(), "overpass");
        assert_eq!(provider.base_url, "https://overpass-api.de/api/interpreter");

        let capabilities = provider.capabilities();
        assert!(capabilities.supports_real_time);
        assert!(capabilities.requires_network);
        assert!(capabilities.supports_geocoding);
        assert!(capabilities.wasm_compatible);
        assert_eq!(capabilities.max_area_km2, Some(1000.0));
        assert_eq!(capabilities.rate_limit_rpm, Some(60));
    }

    #[test]
    fn test_overpass_provider_custom_url() {
        let custom_url = "https://lz4.overpass-api.de/api/interpreter";
        let provider = OverpassProvider::with_base_url(custom_url);
        assert_eq!(provider.base_url, custom_url);
    }

    #[test]
    fn test_build_overpass_query() {
        let provider = OverpassProvider::new();
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        let config = OsmConfigBuilder::new()
            .features(FeatureSet::urban())
            .build();

        let query = provider.build_overpass_query(&bbox, &config);

        // Should contain basic structure
        assert!(query.contains("[out:json]"));
        assert!(query.contains("[timeout:"));
        assert!(query.contains("52,13,53,14")); // bbox coordinates
        assert!(query.contains("out geom"));

        // Should contain feature queries
        assert!(query.contains("way[\"highway\"]"));
        assert!(query.contains("way[\"building\"]"));
        assert!(query.contains("way[\"leisure\"]"));
        assert!(query.contains("way[\"natural\"]"));
    }

    #[test]
    fn test_build_overpass_query_with_custom_features() {
        let provider = OverpassProvider::new();
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        let config = OsmConfigBuilder::new()
            .features(
                FeatureSet::new()
                    .with_feature(OsmFeature::Buildings)
                    .with_feature(OsmFeature::Railways),
            )
            .build();

        let query = provider.build_overpass_query(&bbox, &config);

        // Should contain building queries
        assert!(query.contains("way[\"building\"]"));
        assert!(query.contains("relation[\"building\"]")); // Buildings use relations

        // Should contain railway queries
        assert!(query.contains("way[\"railway\"]"));

        // Should not contain roads (not in feature set)
        assert!(!query.contains("way[\"highway\"]"));
    }

    #[test]
    fn test_should_include_relations() {
        let provider = OverpassProvider::new();

        // These should include relations
        assert!(provider.should_include_relations("building"));
        assert!(provider.should_include_relations("natural"));
        assert!(provider.should_include_relations("landuse"));
        assert!(provider.should_include_relations("leisure"));
        assert!(provider.should_include_relations("boundary"));
        assert!(provider.should_include_relations("waterway"));

        // These should not
        assert!(!provider.should_include_relations("highway"));
        assert!(!provider.should_include_relations("amenity"));
        assert!(!provider.should_include_relations("unknown"));
    }

    #[test]
    fn test_should_include_nodes() {
        let provider = OverpassProvider::new();

        // These should include nodes
        assert!(provider.should_include_nodes("amenity"));
        assert!(provider.should_include_nodes("tourism"));
        assert!(provider.should_include_nodes("power"));

        // These should not
        assert!(!provider.should_include_nodes("building"));
        assert!(!provider.should_include_nodes("highway"));
        assert!(!provider.should_include_nodes("unknown"));
    }

    #[test]
    fn test_radius_to_bbox() {
        let center_lat = 52.5;
        let center_lon = 13.4;
        let radius_km = 10.0;

        let bbox = OverpassProvider::radius_to_bbox(center_lat, center_lon, radius_km);

        // Center should be preserved (approximately)
        let center = bbox.center();
        assert!((center.0 - center_lat).abs() < 0.01);
        assert!((center.1 - center_lon).abs() < 0.01);

        // Bounding box should be reasonable size
        assert!(bbox.height() > 0.1); // At least 0.1 degrees
        assert!(bbox.width() > 0.1);
        assert!(bbox.height() < 0.5); // Less than 0.5 degrees
        assert!(bbox.width() < 0.5);

        // Should contain the center point
        assert!(bbox.contains(center_lat, center_lon));
    }

    #[test]
    fn test_parse_element_count() {
        // Valid JSON with elements
        let json_with_elements = r#"{"elements": [{"type": "node"}, {"type": "way"}]}"#;
        assert_eq!(
            OverpassProvider::parse_element_count(json_with_elements),
            Some(2)
        );

        // Empty elements array
        let json_empty = r#"{"elements": []}"#;
        assert_eq!(OverpassProvider::parse_element_count(json_empty), Some(0));

        // No elements field
        let json_no_elements = r#"{"version": 0.6}"#;
        assert_eq!(
            OverpassProvider::parse_element_count(json_no_elements),
            None
        );

        // Invalid JSON
        let invalid_json = "not json";
        assert_eq!(OverpassProvider::parse_element_count(invalid_json), None);
    }

    #[tokio::test]
    async fn test_resolve_region_bounding_box() {
        let provider = OverpassProvider::new();
        let region = Region::bbox(52.0, 13.0, 53.0, 14.0);

        let result = provider.resolve_region(&region).await.unwrap();
        assert_eq!(result.south, 52.0);
        assert_eq!(result.west, 13.0);
        assert_eq!(result.north, 53.0);
        assert_eq!(result.east, 14.0);
    }

    #[tokio::test]
    async fn test_resolve_region_center_radius() {
        let provider = OverpassProvider::new();
        let region = Region::center_radius(52.5, 13.4, 5.0);

        let result = provider.resolve_region(&region).await.unwrap();

        // Should contain the center point
        assert!(result.contains(52.5, 13.4));

        // Should be a reasonable size for 5km radius
        assert!(result.height() > 0.05); // At least 0.05 degrees
        assert!(result.width() > 0.05);
        assert!(result.height() < 0.2); // Less than 0.2 degrees
        assert!(result.width() < 0.2);
    }

    // Note: We can't easily test the actual network calls without mocking
    // or using integration tests, but we can test the error handling logic

    #[test]
    fn test_area_validation() {
        // This would be tested in the fetch_data method
        // Large areas should be rejected or warned about
        let large_bbox = BoundingBox::new(50.0, 10.0, 55.0, 15.0); // ~500km x 500km
        let area = large_bbox.area_km2();
        assert!(area > 5000.0); // Should trigger our validation
    }

    #[test]
    fn test_timeout_calculation() {
        let provider = OverpassProvider::new();
        let config = OsmConfigBuilder::new().timeout(120).build();
        let bbox = BoundingBox::new(52.0, 13.0, 52.1, 13.1);

        let query = provider.build_overpass_query(&bbox, &config);
        assert!(query.contains("[timeout:120]"));

        // Test custom timeout override
        let provider_with_timeout = OverpassProvider::new().with_timeout_secs(90);
        let query = provider_with_timeout.build_overpass_query(&bbox, &config);
        assert!(query.contains("[timeout:90]"));
    }
}
