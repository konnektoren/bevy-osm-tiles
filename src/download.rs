use async_trait::async_trait;
use geo::{Destination, Haversine, Point};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{BoundingBox, NetworkError, OsmConfig, OsmTilesError, Region, Result};

/// Raw OSM data response from a downloader
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
    /// Data source (e.g., "overpass-api.de")
    pub source: String,
    /// Number of elements returned
    pub element_count: Option<u32>,
    /// Additional metadata from the API
    pub extra: HashMap<String, String>,
}

/// Trait for downloading OpenStreetMap data
///
/// This trait abstracts the data source, allowing for different implementations
/// such as Overpass API, local files, or mock data for testing.
#[async_trait]
pub trait OsmDownloader: Send + Sync {
    /// Download OSM data for the given configuration
    ///
    /// Implementations should:
    /// - Resolve regions to bounding boxes if needed
    /// - Make appropriate API calls
    /// - Handle rate limiting and retries
    /// - Return structured OSM data
    async fn download(&self, config: &OsmConfig) -> Result<OsmData>;

    /// Resolve a region to a concrete bounding box
    ///
    /// For city names, this typically involves geocoding.
    /// For other region types, this may involve coordinate transformation.
    async fn resolve_region(&self, region: &Region) -> Result<BoundingBox>;

    /// Test connectivity to the data source
    async fn test_connection(&self) -> Result<()>;
}

/// HTTP-based downloader using the Overpass API
pub struct OverpassDownloader {
    /// Base URL for the Overpass API
    pub base_url: String,
    /// HTTP client for making requests
    client: reqwest::Client,
    /// User agent string for requests
    user_agent: String,
}

impl OverpassDownloader {
    /// Create a new Overpass API downloader
    pub fn new() -> Self {
        Self::with_base_url("https://overpass-api.de/api/interpreter")
    }

    /// Create a new downloader with a custom Overpass API endpoint
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url: base_url.into(),
            client,
            user_agent: format!("bevy-osm-tiles/{}", env!("CARGO_PKG_VERSION")),
        }
    }

    /// Build an Overpass QL query for the given bounding box and features
    fn build_overpass_query(&self, bbox: &BoundingBox, config: &OsmConfig) -> String {
        let bbox_str = format!("{},{},{},{}", bbox.south, bbox.west, bbox.north, bbox.east);

        let mut query = "[out:json][timeout:25];\n(\n".to_string();

        // Get all OSM tag queries from the feature set
        let tag_queries = config.features.to_osm_queries();

        for tag_query in tag_queries {
            // Add way queries
            let filter = match &tag_query.value {
                Some(value) => format!("[\"{}\"][\"{}\"]", tag_query.key, value),
                None => format!("[\"{}\"]", tag_query.key),
            };
            query.push_str(&format!("  way{}({});\n", filter, bbox_str));

            // Add relation queries for some feature types
            if tag_query.key == "building"
                || tag_query.key == "natural"
                || tag_query.key == "landuse"
            {
                query.push_str(&format!("  relation{}({});\n", filter, bbox_str));
            }
        }

        query.push_str(");\nout geom;");
        query
    }

    /// Convert a radius in kilometers to a bounding box around a center point
    ///
    /// Uses proper geographic calculations via the geo library
    fn radius_to_bbox(center_lat: f64, center_lon: f64, radius_km: f64) -> BoundingBox {
        let center = Point::new(center_lon, center_lat); // Point uses (lon, lat)
        let distance_meters = radius_km * 1000.0;

        // Calculate the four corner points by moving in cardinal directions
        let north_point = Haversine.destination(center, 0.0, distance_meters); // 0° = North
        let south_point = Haversine.destination(center, 180.0, distance_meters); // 180° = South
        let east_point = Haversine.destination(center, 90.0, distance_meters); // 90° = East
        let west_point = Haversine.destination(center, 270.0, distance_meters); // 270° = West

        BoundingBox::new(
            south_point.y(), // latitude
            west_point.x(),  // longitude
            north_point.y(), // latitude
            east_point.x(),  // longitude
        )
    }
}

#[async_trait]
impl OsmDownloader for OverpassDownloader {
    async fn download(&self, config: &OsmConfig) -> Result<OsmData> {
        tracing::info!("Downloading OSM data with config: {:?}", config);

        // First resolve the region to a bounding box
        let bbox = self.resolve_region(&config.region).await?;
        tracing::debug!("Resolved region to bounding box: {:?}", bbox);

        // Build the Overpass query
        let query = self.build_overpass_query(&bbox, config);
        tracing::debug!("Overpass query: {}", query);

        // Make the HTTP request
        let response = self
            .client
            .post(&self.base_url)
            .header("User-Agent", &self.user_agent)
            .form(&[("data", query)])
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    NetworkError::Timeout {
                        seconds: config.timeout_seconds,
                    }
                } else if e.is_connect() {
                    NetworkError::Connection {
                        message: e.to_string(),
                    }
                } else {
                    NetworkError::Connection {
                        message: e.to_string(),
                    }
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(OsmTilesError::Network(NetworkError::HttpError {
                status: status.as_u16(),
            }));
        }

        let raw_data = response
            .text()
            .await
            .map_err(|e| NetworkError::Connection {
                message: e.to_string(),
            })?;

        let metadata = OsmMetadata {
            timestamp: chrono::Utc::now().to_rfc3339(),
            source: self.base_url.clone(),
            element_count: None, // TODO: Parse this from response
            extra: HashMap::new(),
        };

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
                // For now, we'll use a simple geocoding approach via Nominatim
                // In a production system, you might want to use a dedicated geocoding service
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
                        message: e.to_string(),
                    })?;

                let geocode_results: Vec<serde_json::Value> = response
                    .json()
                    .await
                    .map_err(|e| OsmTilesError::Parse(e.to_string()))?;

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
                        "Invalid bounding box format".to_string(),
                    ));
                }

                let south: f64 = bbox_array[0]
                    .as_str()
                    .ok_or_else(|| OsmTilesError::Parse("Invalid latitude".to_string()))?
                    .parse()
                    .map_err(|_| OsmTilesError::Parse("Invalid latitude format".to_string()))?;

                let north: f64 = bbox_array[1]
                    .as_str()
                    .ok_or_else(|| OsmTilesError::Parse("Invalid latitude".to_string()))?
                    .parse()
                    .map_err(|_| OsmTilesError::Parse("Invalid latitude format".to_string()))?;

                let west: f64 = bbox_array[2]
                    .as_str()
                    .ok_or_else(|| OsmTilesError::Parse("Invalid longitude".to_string()))?
                    .parse()
                    .map_err(|_| OsmTilesError::Parse("Invalid longitude format".to_string()))?;

                let east: f64 = bbox_array[3]
                    .as_str()
                    .ok_or_else(|| OsmTilesError::Parse("Invalid longitude".to_string()))?
                    .parse()
                    .map_err(|_| OsmTilesError::Parse("Invalid longitude format".to_string()))?;

                Ok(BoundingBox::new(south, west, north, east))
            }
        }
    }

    async fn test_connection(&self) -> Result<()> {
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
                message: e.to_string(),
            })?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(OsmTilesError::Network(NetworkError::HttpError {
                status: response.status().as_u16(),
            }))
        }
    }
}

impl Default for OverpassDownloader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{Distance, Haversine};

    #[test]
    fn test_overpass_query_building() {
        let downloader = OverpassDownloader::new();
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        let config = OsmConfig::default();

        let query = downloader.build_overpass_query(&bbox, &config);

        assert!(query.contains("52,13,53,14"));
        assert!(query.contains("way[\"highway\"]"));
        assert!(query.contains("way[\"building\"]"));
        assert!(query.contains("out geom"));
    }

    #[tokio::test]
    async fn test_region_resolution_bbox() {
        let downloader = OverpassDownloader::new();
        let region = Region::bbox(52.0, 13.0, 53.0, 14.0);

        let result = downloader.resolve_region(&region).await.unwrap();
        assert_eq!(result.south, 52.0);
        assert_eq!(result.west, 13.0);
        assert_eq!(result.north, 53.0);
        assert_eq!(result.east, 14.0);
    }

    #[tokio::test]
    async fn test_region_resolution_center_radius() {
        let downloader = OverpassDownloader::new();
        let lat = 52.5_f64;
        let lon = 13.4_f64;
        let radius_km = 5.0_f64;
        let region = Region::center_radius(lat, lon, radius_km);

        let result = downloader.resolve_region(&region).await.unwrap();

        // Verify the center is approximately correct
        let center = result.center();
        assert!(
            (center.0 - lat).abs() < 0.01_f64,
            "Center latitude should be close to original: expected {}, got {}",
            lat,
            center.0
        );
        assert!(
            (center.1 - lon).abs() < 0.01_f64,
            "Center longitude should be close to original: expected {}, got {}",
            lon,
            center.1
        );

        // Verify the bounding box makes sense
        assert!(
            result.north > result.south,
            "North should be greater than south"
        );
        assert!(
            result.east > result.west,
            "East should be greater than west"
        );

        // Check that the distance from center to corners is approximately the radius
        let center_point = Point::new(lon, lat);
        let corner_distances = [
            Haversine.distance(center_point, Point::new(result.west, result.south)) / 1000.0_f64,
            Haversine.distance(center_point, Point::new(result.east, result.south)) / 1000.0_f64,
            Haversine.distance(center_point, Point::new(result.west, result.north)) / 1000.0_f64,
            Haversine.distance(center_point, Point::new(result.east, result.north)) / 1000.0_f64,
        ];

        for distance in corner_distances {
            assert!(
                distance >= radius_km - 0.1_f64 && distance <= radius_km * 1.5_f64,
                "Corner distance {} should be approximately the radius {}",
                distance,
                radius_km
            );
        }
    }

    #[test]
    fn test_radius_to_bbox_calculation() {
        // Test the geographic calculation directly
        let lat = 52.5_f64; // Berlin latitude
        let lon = 13.4_f64; // Berlin longitude
        let radius_km = 10.0_f64;

        let bbox = OverpassDownloader::radius_to_bbox(lat, lon, radius_km);

        // Center should be preserved
        let center = bbox.center();
        assert!(
            (center.0 - lat).abs() < 0.001_f64,
            "Center latitude should be preserved: expected {}, got {}",
            lat,
            center.0
        );
        assert!(
            (center.1 - lon).abs() < 0.001_f64,
            "Center longitude should be preserved: expected {}, got {}",
            lon,
            center.1
        );

        // Verify we get a reasonable bounding box
        assert!(
            bbox.height() > 0.15_f64 && bbox.height() < 0.2_f64,
            "Height should be reasonable for 10km radius, got {}",
            bbox.height()
        );
        assert!(
            bbox.width() > 0.2_f64 && bbox.width() < 0.3_f64,
            "Width should be reasonable for 10km radius at this latitude, got {}",
            bbox.width()
        );
    }

    #[test]
    fn test_radius_to_bbox_edge_cases() {
        // Test at equator - should be more symmetric
        let bbox_equator = OverpassDownloader::radius_to_bbox(0.0, 0.0, 10.0);
        let height_width_ratio = bbox_equator.height() / bbox_equator.width();
        assert!(
            height_width_ratio > 0.9_f64 && height_width_ratio < 1.1_f64,
            "At equator, height/width ratio should be close to 1, got {}",
            height_width_ratio
        );

        // Test small radius
        let bbox_small = OverpassDownloader::radius_to_bbox(52.5, 13.4, 1.0);
        assert!(
            bbox_small.height() > 0.01_f64 && bbox_small.height() < 0.03_f64,
            "Small radius should give small bounding box"
        );

        // Test large radius
        let bbox_large = OverpassDownloader::radius_to_bbox(52.5, 13.4, 50.0);
        assert!(
            bbox_large.height() > 0.8_f64 && bbox_large.height() < 1.0_f64,
            "Large radius should give large bounding box"
        );
    }

    #[test]
    fn test_geo_integration() {
        // Verify that geo crate is working correctly
        use geo::{Destination, Distance, Haversine, Point};

        let berlin = Point::new(13.4_f64, 52.5_f64); // (lon, lat)
        let distance_meters = 10000.0_f64; // 10km

        // Move 10km north from Berlin
        let north_point = Haversine.destination(berlin, 0.0, distance_meters);
        assert!(
            north_point.y() > berlin.y(),
            "Moving north should increase latitude"
        );
        assert!(
            (north_point.x() - berlin.x()).abs() < 0.001_f64,
            "Moving north should not change longitude significantly"
        );

        // Move 10km east from Berlin
        let east_point = Haversine.destination(berlin, 90.0, distance_meters);
        assert!(
            east_point.x() > berlin.x(),
            "Moving east should increase longitude"
        );
        assert!(
            (east_point.y() - berlin.y()).abs() < 0.001_f64,
            "Moving east should not change latitude significantly"
        );

        // Verify distance calculation
        let calculated_distance = Haversine.distance(berlin, north_point);
        assert!(
            (calculated_distance - distance_meters).abs() < 1.0_f64,
            "Distance calculation should be accurate"
        );
    }
}
