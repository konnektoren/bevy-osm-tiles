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
