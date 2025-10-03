use geo::{Destination, Distance, Haversine, Point};
use serde::{Deserialize, Serialize};

/// Represents a geographic bounding box for OSM data requests
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    /// Southern latitude boundary
    pub south: f64,
    /// Western longitude boundary
    pub west: f64,
    /// Northern latitude boundary
    pub north: f64,
    /// Eastern longitude boundary
    pub east: f64,
}

impl BoundingBox {
    /// Create a new bounding box from coordinates
    pub fn new(south: f64, west: f64, north: f64, east: f64) -> Self {
        Self {
            south,
            west,
            north,
            east,
        }
    }

    /// Get the center point of the bounding box
    pub fn center(&self) -> (f64, f64) {
        let lat = (self.south + self.north) / 2.0;
        let lon = (self.west + self.east) / 2.0;
        (lat, lon)
    }

    /// Get the width of the bounding box in degrees longitude
    pub fn width(&self) -> f64 {
        self.east - self.west
    }

    /// Get the height of the bounding box in degrees latitude
    pub fn height(&self) -> f64 {
        self.north - self.south
    }

    /// Get the approximate area in square kilometers using geographic calculations
    pub fn area_km2(&self) -> f64 {
        let center = self.center();

        // Calculate distances using Haversine formula
        let width_km = {
            let west_point = Point::new(self.west, center.0);
            let east_point = Point::new(self.east, center.0);
            Haversine.distance(west_point, east_point) / 1000.0 // Convert to km
        };

        let height_km = {
            let south_point = Point::new(center.1, self.south);
            let north_point = Point::new(center.1, self.north);
            Haversine.distance(south_point, north_point) / 1000.0 // Convert to km
        };

        width_km * height_km
    }

    /// Check if this bounding box contains a point
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        lat >= self.south && lat <= self.north && lon >= self.west && lon <= self.east
    }

    /// Expand the bounding box by a given distance in kilometers
    pub fn expand_by_km(&self, distance_km: f64) -> BoundingBox {
        let center = self.center();
        let distance_meters = distance_km * 1000.0;

        // Calculate new bounds by moving from the current bounds outward
        let current_north = Point::new(center.1, self.north);
        let new_north = Haversine.destination(current_north, 0.0, distance_meters); // 0° = North

        let current_south = Point::new(center.1, self.south);
        let new_south = Haversine.destination(current_south, 180.0, distance_meters); // 180° = South

        let current_east = Point::new(self.east, center.0);
        let new_east = Haversine.destination(current_east, 90.0, distance_meters); // 90° = East

        let current_west = Point::new(self.west, center.0);
        let new_west = Haversine.destination(current_west, 270.0, distance_meters); // 270° = West

        BoundingBox::new(
            new_south.y(), // latitude
            new_west.x(),  // longitude
            new_north.y(), // latitude
            new_east.x(),  // longitude
        )
    }
}

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
    pub features: FeatureFilter,
}

/// Represents different ways to specify a geographic region
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Region {
    /// A named city that will be resolved to coordinates
    City { name: String },
    /// A custom bounding box with explicit coordinates
    BoundingBox(BoundingBox),
    /// A center point with radius (in kilometers)
    CenterRadius { lat: f64, lon: f64, radius_km: f64 },
}

impl Region {
    /// Create a region from a city name
    pub fn city(name: impl Into<String>) -> Self {
        Self::City { name: name.into() }
    }

    /// Create a region from a bounding box
    pub fn bbox(south: f64, west: f64, north: f64, east: f64) -> Self {
        Self::BoundingBox(BoundingBox::new(south, west, north, east))
    }

    /// Create a region from center point and radius
    pub fn center_radius(lat: f64, lon: f64, radius_km: f64) -> Self {
        Self::CenterRadius {
            lat,
            lon,
            radius_km,
        }
    }
}

/// Specifies which OSM features to include in grid generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFilter {
    /// Include roads and pathways
    pub roads: bool,
    /// Include buildings and structures
    pub buildings: bool,
    /// Include water features (rivers, lakes, etc.)
    pub water: bool,
    /// Include green spaces (parks, forests, etc.)
    pub green_spaces: bool,
    /// Include railway infrastructure
    pub railways: bool,
    /// Custom OSM tags to include
    pub custom_tags: Vec<String>,
}

impl Default for FeatureFilter {
    fn default() -> Self {
        Self {
            roads: true,
            buildings: true,
            water: true,
            green_spaces: true,
            railways: false,
            custom_tags: Vec::new(),
        }
    }
}

impl Default for OsmConfig {
    fn default() -> Self {
        Self {
            region: Region::city("Berlin"),
            grid_resolution: 100,
            tile_size: 10.0,
            timeout_seconds: 30,
            features: FeatureFilter::default(),
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
    pub fn with_features(mut self, features: FeatureFilter) -> Self {
        self.features = features;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounding_box_creation() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        assert_eq!(bbox.center(), (52.5, 13.5));
        assert_eq!(bbox.width(), 1.0);
        assert_eq!(bbox.height(), 1.0);
    }

    #[test]
    fn test_bounding_box_contains() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);

        assert!(bbox.contains(52.5, 13.5)); // Center
        assert!(bbox.contains(52.0, 13.0)); // Southwest corner
        assert!(bbox.contains(53.0, 14.0)); // Northeast corner

        assert!(!bbox.contains(51.9, 13.5)); // Too far south
        assert!(!bbox.contains(52.5, 12.9)); // Too far west
        assert!(!bbox.contains(53.1, 13.5)); // Too far north
        assert!(!bbox.contains(52.5, 14.1)); // Too far east
    }

    #[test]
    fn test_bounding_box_area() {
        let bbox = BoundingBox::new(52.0, 13.0, 52.1, 13.1); // Small box around Berlin
        let area = bbox.area_km2();

        // This should be roughly 11km x 7km = ~77 km²
        assert!(
            area > 70.0 && area < 85.0,
            "Area should be around 77 km², got {}",
            area
        );
    }

    #[test]
    fn test_bounding_box_expand() {
        let bbox = BoundingBox::new(52.5, 13.4, 52.6, 13.5); // Small box
        let expanded = bbox.expand_by_km(1.0); // Expand by 1km

        // Original box should be contained in expanded box
        assert!(expanded.contains(bbox.south, bbox.west));
        assert!(expanded.contains(bbox.north, bbox.east));

        // Expanded box should be larger
        assert!(expanded.height() > bbox.height());
        assert!(expanded.width() > bbox.width());

        // Centers should be approximately the same
        let original_center = bbox.center();
        let expanded_center = expanded.center();
        assert!((original_center.0 - expanded_center.0).abs() < 0.01);
        assert!((original_center.1 - expanded_center.1).abs() < 0.01);
    }

    #[test]
    fn test_region_creation() {
        let city = Region::city("Berlin");
        match city {
            Region::City { name } => assert_eq!(name, "Berlin"),
            _ => panic!("Expected City variant"),
        }

        let bbox = Region::bbox(52.0, 13.0, 53.0, 14.0);
        match bbox {
            Region::BoundingBox(b) => {
                assert_eq!(b.south, 52.0);
                assert_eq!(b.north, 53.0);
            }
            _ => panic!("Expected BoundingBox variant"),
        }
    }

    #[test]
    fn test_config_builder() {
        let config = OsmConfig::for_city("Munich")
            .with_grid_resolution(200)
            .with_tile_size(5.0)
            .with_timeout(60);

        match config.region {
            Region::City { name } => assert_eq!(name, "Munich"),
            _ => panic!("Expected city region"),
        }
        assert_eq!(config.grid_resolution, 200);
        assert_eq!(config.tile_size, 5.0);
        assert_eq!(config.timeout_seconds, 60);
    }
}
