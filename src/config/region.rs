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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounding_box_creation() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        assert_eq!(bbox.south, 52.0);
        assert_eq!(bbox.west, 13.0);
        assert_eq!(bbox.north, 53.0);
        assert_eq!(bbox.east, 14.0);
    }

    #[test]
    fn test_bounding_box_center() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        let center = bbox.center();
        assert_eq!(center, (52.5, 13.5));

        // Test with non-symmetric bbox
        let bbox2 = BoundingBox::new(50.0, 10.0, 52.0, 15.0);
        let center2 = bbox2.center();
        assert_eq!(center2, (51.0, 12.5));
    }

    #[test]
    fn test_bounding_box_dimensions() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        assert_eq!(bbox.width(), 1.0);
        assert_eq!(bbox.height(), 1.0);

        let bbox2 = BoundingBox::new(50.0, 10.0, 52.5, 15.5);
        assert_eq!(bbox2.width(), 5.5);
        assert_eq!(bbox2.height(), 2.5);
    }

    #[test]
    fn test_bounding_box_contains() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);

        // Points inside
        assert!(bbox.contains(52.5, 13.5)); // Center
        assert!(bbox.contains(52.0, 13.0)); // Southwest corner
        assert!(bbox.contains(53.0, 14.0)); // Northeast corner
        assert!(bbox.contains(52.1, 13.9)); // Inside

        // Points outside
        assert!(!bbox.contains(51.9, 13.5)); // Too far south
        assert!(!bbox.contains(52.5, 12.9)); // Too far west
        assert!(!bbox.contains(53.1, 13.5)); // Too far north
        assert!(!bbox.contains(52.5, 14.1)); // Too far east
    }

    #[test]
    fn test_bounding_box_area() {
        // Small box around Berlin
        let bbox = BoundingBox::new(52.4, 13.3, 52.6, 13.5);
        let area = bbox.area_km2();

        // Should be roughly 22km x 14km = ~308 km²
        assert!(
            area > 250.0 && area < 350.0,
            "Area should be around 308 km², got {}",
            area
        );
    }

    #[test]
    fn test_bounding_box_expand() {
        let bbox = BoundingBox::new(52.5, 13.4, 52.6, 13.5);
        let expanded = bbox.expand_by_km(1.0);

        // Original box should be contained in expanded box
        assert!(expanded.contains(bbox.south, bbox.west));
        assert!(expanded.contains(bbox.north, bbox.east));
        assert!(expanded.contains(52.55, 13.45)); // Center should still be contained

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
    fn test_bounding_box_expand_zero() {
        let bbox = BoundingBox::new(52.5, 13.4, 52.6, 13.5);
        let expanded = bbox.expand_by_km(0.0);

        // Should be approximately the same
        assert!((bbox.south - expanded.south).abs() < 0.001);
        assert!((bbox.west - expanded.west).abs() < 0.001);
        assert!((bbox.north - expanded.north).abs() < 0.001);
        assert!((bbox.east - expanded.east).abs() < 0.001);
    }

    #[test]
    fn test_region_city_creation() {
        let region = Region::city("Berlin");
        match region {
            Region::City { name } => assert_eq!(name, "Berlin"),
            _ => panic!("Expected City variant"),
        }

        let region2 = Region::city(String::from("Munich"));
        match region2 {
            Region::City { name } => assert_eq!(name, "Munich"),
            _ => panic!("Expected City variant"),
        }
    }

    #[test]
    fn test_region_bbox_creation() {
        let region = Region::bbox(52.0, 13.0, 53.0, 14.0);
        match region {
            Region::BoundingBox(bbox) => {
                assert_eq!(bbox.south, 52.0);
                assert_eq!(bbox.west, 13.0);
                assert_eq!(bbox.north, 53.0);
                assert_eq!(bbox.east, 14.0);
            }
            _ => panic!("Expected BoundingBox variant"),
        }
    }

    #[test]
    fn test_region_center_radius_creation() {
        let region = Region::center_radius(52.5, 13.4, 5.0);
        match region {
            Region::CenterRadius {
                lat,
                lon,
                radius_km,
            } => {
                assert_eq!(lat, 52.5);
                assert_eq!(lon, 13.4);
                assert_eq!(radius_km, 5.0);
            }
            _ => panic!("Expected CenterRadius variant"),
        }
    }

    #[test]
    fn test_bounding_box_serialization() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);

        // Test JSON serialization
        let json = serde_json::to_string(&bbox).expect("Failed to serialize");
        let deserialized: BoundingBox = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(bbox, deserialized);
    }

    #[test]
    fn test_region_serialization() {
        let regions = vec![
            Region::city("Berlin"),
            Region::bbox(52.0, 13.0, 53.0, 14.0),
            Region::center_radius(52.5, 13.4, 5.0),
        ];

        for region in regions {
            let json = serde_json::to_string(&region).expect("Failed to serialize");
            let deserialized: Region = serde_json::from_str(&json).expect("Failed to deserialize");

            // Compare by debug representation since Region doesn't implement PartialEq
            assert_eq!(format!("{:?}", region), format!("{:?}", deserialized));
        }
    }

    #[test]
    fn test_bounding_box_edge_cases() {
        // Test with very small bbox
        let tiny_bbox = BoundingBox::new(52.5, 13.4, 52.5001, 13.4001);
        let area = tiny_bbox.area_km2();
        assert!(
            area > 0.0 && area < 0.01,
            "Tiny area should be very small but positive"
        );

        // Test with bbox crossing prime meridian (longitude 0)
        let cross_meridian = BoundingBox::new(51.0, -1.0, 52.0, 1.0);
        assert_eq!(cross_meridian.width(), 2.0);
        assert!(cross_meridian.contains(51.5, 0.0));

        // Test with bbox crossing equator
        let cross_equator = BoundingBox::new(-1.0, 10.0, 1.0, 11.0);
        assert_eq!(cross_equator.height(), 2.0);
        assert!(cross_equator.contains(0.0, 10.5));
    }

    #[test]
    fn test_bounding_box_invalid_coordinates() {
        // These are technically invalid but should still work
        let inverted_lat = BoundingBox::new(53.0, 13.0, 52.0, 14.0); // south > north
        assert_eq!(inverted_lat.height(), -1.0);

        let inverted_lon = BoundingBox::new(52.0, 14.0, 53.0, 13.0); // west > east
        assert_eq!(inverted_lon.width(), -1.0);
    }
}
