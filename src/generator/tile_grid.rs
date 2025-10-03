use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::BoundingBox;

/// Represents a single tile in the grid
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TileType {
    /// Empty/unknown tile
    Empty,
    /// Road or highway
    Road,
    /// Building
    Building,
    /// Water feature (river, lake, etc.)
    Water,
    /// Green space (park, forest, etc.)
    GreenSpace,
    /// Railway
    Railway,
    /// Parking area
    Parking,
    /// Amenity (cafe, shop, etc.)
    Amenity,
    /// Tourism feature
    Tourism,
    /// Industrial area
    Industrial,
    /// Residential area
    Residential,
    /// Commercial area
    Commercial,
    /// Custom tile type with name
    Custom(String),
}

impl Default for TileType {
    fn default() -> Self {
        Self::Empty
    }
}

impl TileType {
    /// Get a human-readable name for this tile type
    pub fn name(&self) -> &str {
        match self {
            Self::Empty => "empty",
            Self::Road => "road",
            Self::Building => "building",
            Self::Water => "water",
            Self::GreenSpace => "green_space",
            Self::Railway => "railway",
            Self::Parking => "parking",
            Self::Amenity => "amenity",
            Self::Tourism => "tourism",
            Self::Industrial => "industrial",
            Self::Residential => "residential",
            Self::Commercial => "commercial",
            Self::Custom(name) => name,
        }
    }

    /// Get a suggested color for this tile type (RGB)
    pub fn default_color(&self) -> (u8, u8, u8) {
        match self {
            Self::Empty => (240, 240, 240),     // Light gray
            Self::Road => (128, 128, 128),      // Gray
            Self::Building => (139, 69, 19),    // Brown
            Self::Water => (30, 144, 255),      // Blue
            Self::GreenSpace => (34, 139, 34),  // Green
            Self::Railway => (105, 105, 105),   // Dark gray
            Self::Parking => (169, 169, 169),   // Light gray
            Self::Amenity => (255, 165, 0),     // Orange
            Self::Tourism => (255, 20, 147),    // Pink
            Self::Industrial => (128, 0, 128),  // Purple
            Self::Residential => (255, 255, 0), // Yellow
            Self::Commercial => (255, 0, 0),    // Red
            Self::Custom(_) => (200, 200, 200), // Default gray
        }
    }

    /// Check if this tile type represents a navigable area
    pub fn is_navigable(&self) -> bool {
        matches!(self, Self::Road | Self::Empty | Self::Parking)
    }

    /// Check if this tile type represents a structure
    pub fn is_structure(&self) -> bool {
        matches!(self, Self::Building | Self::Amenity | Self::Tourism)
    }

    /// Get priority for tile placement (higher priority overwrites lower)
    pub fn priority(&self) -> u8 {
        match self {
            Self::Empty => 0,
            Self::GreenSpace => 1,
            Self::Water => 2,
            Self::Residential => 3,
            Self::Commercial => 4,
            Self::Industrial => 5,
            Self::Parking => 6,
            Self::Road => 7,
            Self::Railway => 8,
            Self::Building => 9,
            Self::Amenity => 10,
            Self::Tourism => 11,
            Self::Custom(_) => 5,
        }
    }
}

/// Additional metadata for a tile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileMetadata {
    /// OSM element IDs that contributed to this tile
    pub osm_ids: Vec<i64>,
    /// OSM tags from the elements
    pub tags: HashMap<String, String>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
}

impl Default for TileMetadata {
    fn default() -> Self {
        Self {
            osm_ids: Vec::new(),
            tags: HashMap::new(),
            confidence: 1.0,
        }
    }
}

/// A tile with its type and optional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tile {
    pub tile_type: TileType,
    pub metadata: Option<TileMetadata>,
}

impl Default for Tile {
    fn default() -> Self {
        Self {
            tile_type: TileType::Empty,
            metadata: None,
        }
    }
}

impl Tile {
    /// Create a new tile with the given type
    pub fn new(tile_type: TileType) -> Self {
        Self {
            tile_type,
            metadata: None,
        }
    }

    /// Create a new tile with type and metadata
    pub fn with_metadata(tile_type: TileType, metadata: TileMetadata) -> Self {
        Self {
            tile_type,
            metadata: Some(metadata),
        }
    }

    /// Check if this tile can be overwritten by another tile
    pub fn can_be_overwritten_by(&self, other: &Tile) -> bool {
        other.tile_type.priority() > self.tile_type.priority()
    }
}

/// A grid of tiles representing a geographic area
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileGrid {
    /// The actual grid data (stored as Vec<Vec<Tile>> for better serialization)
    tiles: Vec<Vec<Tile>>,
    /// Grid width
    width: usize,
    /// Grid height
    height: usize,
    /// Geographic bounding box this grid represents
    pub bounding_box: BoundingBox,
    /// Meters per tile (approximately)
    pub meters_per_tile: f32,
    /// Grid generation metadata
    pub metadata: GridMetadata,
}

/// Metadata about grid generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridMetadata {
    /// Timestamp when grid was generated
    pub generated_at: String,
    /// Number of OSM elements processed
    pub elements_processed: u32,
    /// Number of tiles populated (non-empty)
    pub tiles_populated: usize,
    /// Generation time in milliseconds
    pub generation_time_ms: u64,
    /// Algorithm used for generation
    pub algorithm: String,
    /// Additional metadata
    pub extra: HashMap<String, String>,
}

impl TileGrid {
    /// Create a new tile grid
    pub fn new(
        width: usize,
        height: usize,
        bounding_box: BoundingBox,
        meters_per_tile: f32,
    ) -> Self {
        // Initialize grid with empty tiles
        let mut tiles = Vec::with_capacity(height);
        for _ in 0..height {
            let mut row = Vec::with_capacity(width);
            for _ in 0..width {
                row.push(Tile::default());
            }
            tiles.push(row);
        }

        Self {
            tiles,
            width,
            height,
            bounding_box,
            meters_per_tile,
            metadata: GridMetadata {
                generated_at: chrono::Utc::now().to_rfc3339(),
                elements_processed: 0,
                tiles_populated: 0,
                generation_time_ms: 0,
                algorithm: "default".to_string(),
                extra: HashMap::new(),
            },
        }
    }

    /// Get the grid dimensions (width, height)
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Get the total number of tiles
    pub fn tile_count(&self) -> usize {
        self.width * self.height
    }

    /// Get the number of rows
    pub fn rows(&self) -> usize {
        self.height
    }

    /// Get the number of columns
    pub fn cols(&self) -> usize {
        self.width
    }

    /// Get a tile at the given grid coordinates
    pub fn get_tile(&self, x: usize, y: usize) -> Option<&Tile> {
        if x < self.width && y < self.height {
            Some(&self.tiles[y][x])
        } else {
            None
        }
    }

    /// Get a mutable reference to a tile at the given grid coordinates
    pub fn get_tile_mut(&mut self, x: usize, y: usize) -> Option<&mut Tile> {
        if x < self.width && y < self.height {
            Some(&mut self.tiles[y][x])
        } else {
            None
        }
    }

    /// Set a tile at the given grid coordinates
    pub fn set_tile(&mut self, x: usize, y: usize, tile: Tile) -> Result<(), String> {
        if x >= self.width || y >= self.height {
            return Err(format!(
                "Coordinates ({}, {}) out of bounds for grid {}x{}",
                x, y, self.width, self.height
            ));
        }

        self.tiles[y][x] = tile;
        Ok(())
    }

    /// Set a tile only if it has higher priority than the existing tile
    pub fn set_tile_with_priority(
        &mut self,
        x: usize,
        y: usize,
        tile: Tile,
    ) -> Result<bool, String> {
        if x >= self.width || y >= self.height {
            return Err(format!(
                "Coordinates ({}, {}) out of bounds for grid {}x{}",
                x, y, self.width, self.height
            ));
        }

        let current_tile = &self.tiles[y][x];
        if current_tile.can_be_overwritten_by(&tile) {
            self.tiles[y][x] = tile;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Convert geographic coordinates (lat, lon) to grid coordinates (x, y)
    pub fn geo_to_grid(&self, lat: f64, lon: f64) -> Option<(usize, usize)> {
        if !self.bounding_box.contains(lat, lon) {
            return None;
        }

        let width_deg = self.bounding_box.width();
        let height_deg = self.bounding_box.height();

        let x_ratio = (lon - self.bounding_box.west) / width_deg;
        let y_ratio = (self.bounding_box.north - lat) / height_deg; // Flip Y axis

        let x = (x_ratio * self.width as f64) as usize;
        let y = (y_ratio * self.height as f64) as usize;

        // Clamp to grid bounds
        let x = x.min(self.width - 1);
        let y = y.min(self.height - 1);

        Some((x, y))
    }

    /// Convert grid coordinates (x, y) to geographic coordinates (lat, lon)
    pub fn grid_to_geo(&self, x: usize, y: usize) -> Option<(f64, f64)> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let width_deg = self.bounding_box.width();
        let height_deg = self.bounding_box.height();

        let x_ratio = (x as f64 + 0.5) / self.width as f64; // Center of tile
        let y_ratio = (y as f64 + 0.5) / self.height as f64;

        let lon = self.bounding_box.west + x_ratio * width_deg;
        let lat = self.bounding_box.north - y_ratio * height_deg; // Flip Y axis

        Some((lat, lon))
    }

    /// Get all tiles of a specific type
    pub fn tiles_of_type(&self, tile_type: &TileType) -> Vec<(usize, usize, &Tile)> {
        let mut results = Vec::new();

        for y in 0..self.height {
            for x in 0..self.width {
                let tile = &self.tiles[y][x];
                if tile.tile_type == *tile_type {
                    results.push((x, y, tile));
                }
            }
        }

        results
    }

    /// Count tiles by type
    pub fn count_tiles_by_type(&self) -> HashMap<TileType, usize> {
        let mut counts = HashMap::new();

        for y in 0..self.height {
            for x in 0..self.width {
                let tile_type = &self.tiles[y][x].tile_type;
                *counts.entry(tile_type.clone()).or_insert(0) += 1;
            }
        }

        counts
    }

    /// Get statistics about the grid
    pub fn statistics(&self) -> GridStatistics {
        let counts = self.count_tiles_by_type();
        let total_tiles = self.tile_count();
        let non_empty_tiles = total_tiles - counts.get(&TileType::Empty).unwrap_or(&0);

        GridStatistics {
            total_tiles,
            non_empty_tiles,
            tile_type_counts: counts,
            coverage_ratio: non_empty_tiles as f64 / total_tiles as f64,
            dimensions: self.dimensions(),
            area_km2: self.bounding_box.area_km2(),
            meters_per_tile: self.meters_per_tile,
        }
    }

    /// Iterate over all tiles with their coordinates
    pub fn iter_tiles(&self) -> impl Iterator<Item = (usize, usize, &Tile)> {
        (0..self.height).flat_map(move |y| (0..self.width).map(move |x| (x, y, &self.tiles[y][x])))
    }

    /// Get a slice of the grid for a specific area
    pub fn get_area(
        &self,
        x_start: usize,
        y_start: usize,
        width: usize,
        height: usize,
    ) -> Option<Vec<Vec<&Tile>>> {
        if x_start + width > self.width || y_start + height > self.height {
            return None;
        }

        let mut result = Vec::with_capacity(height);
        for y in y_start..y_start + height {
            let mut row = Vec::with_capacity(width);
            for x in x_start..x_start + width {
                row.push(&self.tiles[y][x]);
            }
            result.push(row);
        }

        Some(result)
    }

    /// Get raw access to the tiles data (for advanced use)
    pub fn tiles(&self) -> &Vec<Vec<Tile>> {
        &self.tiles
    }

    /// Get mutable raw access to the tiles data (for advanced use)
    pub fn tiles_mut(&mut self) -> &mut Vec<Vec<Tile>> {
        &mut self.tiles
    }
}

/// Statistics about a tile grid
#[derive(Debug, Clone)]
pub struct GridStatistics {
    /// Total number of tiles
    pub total_tiles: usize,
    /// Number of non-empty tiles
    pub non_empty_tiles: usize,
    /// Count of each tile type
    pub tile_type_counts: HashMap<TileType, usize>,
    /// Ratio of non-empty to total tiles
    pub coverage_ratio: f64,
    /// Grid dimensions (width, height)
    pub dimensions: (usize, usize),
    /// Total area covered in km²
    pub area_km2: f64,
    /// Approximate meters per tile
    pub meters_per_tile: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_grid_creation() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        let grid = TileGrid::new(100, 100, bbox, 10.0);

        assert_eq!(grid.dimensions(), (100, 100));
        assert_eq!(grid.tile_count(), 10000);
        assert_eq!(grid.meters_per_tile, 10.0);
    }

    #[test]
    fn test_tile_access() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        let mut grid = TileGrid::new(10, 10, bbox, 10.0);

        // Test getting empty tile
        let tile = grid.get_tile(5, 5).unwrap();
        assert_eq!(tile.tile_type, TileType::Empty);

        // Test setting tile
        let new_tile = Tile::new(TileType::Building);
        grid.set_tile(5, 5, new_tile).unwrap();

        let tile = grid.get_tile(5, 5).unwrap();
        assert_eq!(tile.tile_type, TileType::Building);
    }

    #[test]
    fn test_coordinate_conversion() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        let grid = TileGrid::new(100, 100, bbox, 10.0);

        // Test point in center of bbox
        let (x, y) = grid.geo_to_grid(52.5, 13.5).unwrap();
        assert!(x > 40 && x < 60);
        assert!(y > 40 && y < 60);

        // Test conversion back
        let (lat, lon) = grid.grid_to_geo(x, y).unwrap();
        assert!((lat - 52.5).abs() < 0.02);
        assert!((lon - 13.5).abs() < 0.02);
    }

    #[test]
    fn test_tile_priorities() {
        let empty = Tile::new(TileType::Empty);
        let building = Tile::new(TileType::Building);
        let road = Tile::new(TileType::Road);

        assert!(empty.can_be_overwritten_by(&building));
        assert!(empty.can_be_overwritten_by(&road));
        assert!(road.can_be_overwritten_by(&building));
        assert!(!building.can_be_overwritten_by(&road));
    }

    #[test]
    fn test_set_tile_with_priority() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        let mut grid = TileGrid::new(10, 10, bbox, 10.0);

        // Set a road tile
        let road_tile = Tile::new(TileType::Road);
        assert!(grid.set_tile_with_priority(5, 5, road_tile).unwrap());

        // Try to overwrite with empty (should fail)
        let empty_tile = Tile::new(TileType::Empty);
        assert!(!grid.set_tile_with_priority(5, 5, empty_tile).unwrap());

        // Overwrite with building (should succeed)
        let building_tile = Tile::new(TileType::Building);
        assert!(grid.set_tile_with_priority(5, 5, building_tile).unwrap());

        let final_tile = grid.get_tile(5, 5).unwrap();
        assert_eq!(final_tile.tile_type, TileType::Building);
    }

    #[test]
    fn test_tile_counting() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        let mut grid = TileGrid::new(10, 10, bbox, 10.0);

        // Add some different tile types
        grid.set_tile(0, 0, Tile::new(TileType::Building)).unwrap();
        grid.set_tile(1, 0, Tile::new(TileType::Building)).unwrap();
        grid.set_tile(2, 0, Tile::new(TileType::Road)).unwrap();

        let counts = grid.count_tiles_by_type();
        assert_eq!(*counts.get(&TileType::Building).unwrap(), 2);
        assert_eq!(*counts.get(&TileType::Road).unwrap(), 1);
        assert_eq!(*counts.get(&TileType::Empty).unwrap(), 97);
    }

    #[test]
    fn test_grid_statistics() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        let mut grid = TileGrid::new(10, 10, bbox, 10.0);

        // Add some tiles
        grid.set_tile(0, 0, Tile::new(TileType::Building)).unwrap();
        grid.set_tile(1, 0, Tile::new(TileType::Road)).unwrap();

        let stats = grid.statistics();
        assert_eq!(stats.total_tiles, 100);
        assert_eq!(stats.non_empty_tiles, 2);
        assert_eq!(stats.coverage_ratio, 0.02);
        assert_eq!(stats.dimensions, (10, 10));
    }

    #[test]
    fn test_serialization() {
        let bbox = BoundingBox::new(52.0, 13.0, 53.0, 14.0);
        let mut grid = TileGrid::new(5, 5, bbox, 10.0);

        // Add some tiles
        grid.set_tile(0, 0, Tile::new(TileType::Building)).unwrap();
        grid.set_tile(1, 1, Tile::new(TileType::Road)).unwrap();

        // Test serialization
        let json = serde_json::to_string(&grid).unwrap();
        assert!(!json.is_empty());

        // Test deserialization
        let deserialized: TileGrid = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.dimensions(), (5, 5));
        assert_eq!(
            deserialized.get_tile(0, 0).unwrap().tile_type,
            TileType::Building
        );
        assert_eq!(
            deserialized.get_tile(1, 1).unwrap().tile_type,
            TileType::Road
        );
    }
}
