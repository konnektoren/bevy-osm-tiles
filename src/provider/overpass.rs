use async_trait::async_trait;
use geo::{Destination, Haversine, Point};
use std::time::Instant;

use super::{OsmData, OsmDataProvider, ProviderCapabilities};
use crate::{
    BoundingBox, NetworkError, OsmConfig, OsmDataFormat, OsmMetadata, OsmTilesError, Region, Result,
};

/// WASM-compatible HTTP-based provider using the Overpass API
///
/// This provider uses reqwest with WASM-compatible features to fetch
/// OpenStreetMap data from Overpass API endpoints. It supports all
/// modern browsers and server environments.
pub struct OverpassProvider {
    /// Base URL for the Overpass API
    pub base_url: String,
    /// HTTP client for making requests (WASM-compatible)
    client: reqwest::Client,
    /// User agent string for requests
    user_agent: String,
    /// Custom timeout override
    custom_timeout: Option<std::time::Duration>,
}

impl OverpassProvider {
    /// Create a new Overpass API provider with default endpoint
    ///
    /// Uses the main Overpass API instance, but you can also use:
    /// - https://lz4.overpass-api.de/api/interpreter (LZ4 compressed)
    /// - https://z.overpass-api.de/api/interpreter (Gzip compressed)
    pub fn new() -> Self {
        Self::with_base_url("https://overpass-api.de/api/interpreter")
    }

    /// Create a new provider with a custom Overpass API endpoint
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url: base_url.into(),
            client,
            user_agent: format!("bevy-osm-tiles/{}", env!("CARGO_PKG_VERSION")),
            custom_timeout: None,
        }
    }

    /// Set a custom timeout for requests
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.custom_timeout = Some(timeout);
        self
    }

    /// Set a custom user agent
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Build an Overpass QL query for the given bounding box and features
    fn build_overpass_query(&self, bbox: &BoundingBox, config: &OsmConfig) -> String {
        let bbox_str = format!("{},{},{},{}", bbox.south, bbox.west, bbox.north, bbox.east);

        let timeout = self
            .custom_timeout
            .or_else(|| Some(std::time::Duration::from_secs(config.timeout_seconds)))
            .unwrap()
            .as_secs();

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
}

#[async_trait]
impl OsmDataProvider for OverpassProvider {
    fn provider_type(&self) -> &'static str {
        "overpass"
    }

    async fn fetch_data(&self, config: &OsmConfig) -> Result<OsmData> {
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

        // Determine timeout
        let timeout = self
            .custom_timeout
            .unwrap_or_else(|| std::time::Duration::from_secs(config.timeout_seconds));

        // Make the HTTP request
        let response = self
            .client
            .post(&self.base_url)
            .header("User-Agent", &self.user_agent)
            .form(&[("data", query)])
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    NetworkError::Timeout {
                        seconds: timeout.as_secs(),
                    }
                } else if e.is_connect() {
                    NetworkError::Connection {
                        message: format!("Failed to connect to Overpass API: {}", e),
                    }
                } else {
                    NetworkError::Connection {
                        message: e.to_string(),
                    }
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(OsmTilesError::Network(NetworkError::HttpError {
                status: status.as_u16(),
            }));
        }

        let raw_data = response
            .text()
            .await
            .map_err(|e| NetworkError::Connection {
                message: format!("Failed to read response: {}", e),
            })?;

        let processing_time = start_time.elapsed().as_millis() as u64;
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
            .with_extra("wasm_compatible", "true");

        tracing::info!(
            "Successfully fetched OSM data: {} elements, {:.2} KB, {:.1}s",
            element_count.unwrap_or(0),
            raw_data.len() as f64 / 1024.0,
            processing_time as f64 / 1000.0
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
                    .client
                    .get(&nominatim_url)
                    .header("User-Agent", &self.user_agent)
                    .send()
                    .await
                    .map_err(|e| NetworkError::Connection {
                        message: format!("Geocoding failed: {}", e),
                    })?;

                let geocode_results: Vec<serde_json::Value> =
                    response.json().await.map_err(|e| {
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
            .client
            .post(&self.base_url)
            .header("User-Agent", &self.user_agent)
            .form(&[("data", test_query)])
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| NetworkError::Connection {
                message: format!("Overpass API test failed: {}", e),
            })?;

        if response.status().is_success() {
            tracing::debug!("Overpass API is available");
            Ok(())
        } else {
            Err(OsmTilesError::Network(NetworkError::HttpError {
                status: response.status().as_u16(),
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
            notes: Some(
                "Real-time OSM data via Overpass API. WASM-compatible using reqwest.".to_string(),
            ),
        }
    }
}

impl Default for OverpassProvider {
    fn default() -> Self {
        Self::new()
    }
}
