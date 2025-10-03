use async_trait::async_trait;
use std::time::Instant;

use super::{
    GeneratorCapabilities, GridGenerator, OsmElement, OsmParser, Tile, TileGrid, TileType,
};
use crate::{OsmConfig, OsmData, OsmTilesError, Result};

/// Default grid generator implementation
pub struct DefaultGridGenerator {
    /// Parser for OSM data
    parser: OsmParser,
    /// Maximum grid size to prevent memory issues
    max_grid_size: (usize, usize),
}

impl DefaultGridGenerator {
    /// Create a new default grid generator
    pub fn new() -> Self {
        Self {
            parser: OsmParser,
            max_grid_size: (5000, 5000),
        }
    }

    /// Create a grid generator with custom maximum grid size
    pub fn with_max_size(max_width: usize, max_height: usize) -> Self {
        Self {
            parser: OsmParser,
            max_grid_size: (max_width, max_height),
        }
    }

    /// Calculate grid dimensions based on config and bounding box
    fn calculate_grid_dimensions(
        &self,
        config: &OsmConfig,
        osm_data: &OsmData,
    ) -> Result<(usize, usize)> {
        let bbox = &osm_data.bounding_box;

        // Calculate grid size based on resolution and area
        let width_deg = bbox.width();
        let height_deg = bbox.height();

        // Grid resolution is cells per degree
        let grid_width = (width_deg * config.grid_resolution as f64).ceil() as usize;
        let grid_height = (height_deg * config.grid_resolution as f64).ceil() as usize;

        // Enforce minimum size
        let grid_width = grid_width.max(10);
        let grid_height = grid_height.max(10);

        // Enforce maximum size
        let grid_width = grid_width.min(self.max_grid_size.0);
        let grid_height = grid_height.min(self.max_grid_size.1);

        Ok((grid_width, grid_height))
    }

    /// Calculate approximate meters per tile
    fn calculate_meters_per_tile(
        &self,
        config: &OsmConfig,
        osm_data: &OsmData,
        grid_dims: (usize, usize),
    ) -> f32 {
        let bbox = &osm_data.bounding_box;

        // Use the area and grid size to estimate meters per tile
        let area_km2 = bbox.area_km2();
        let total_tiles = grid_dims.0 * grid_dims.1;
        let km2_per_tile = area_km2 / total_tiles as f64;
        let m2_per_tile = km2_per_tile * 1_000_000.0; // Convert km² to m²
        let meters_per_tile = (m2_per_tile.sqrt()) as f32; // Approximate side length

        // Also consider the configured tile size as a hint
        (meters_per_tile + config.tile_size) / 2.0
    }

    /// Rasterize an OSM element onto the grid
    fn rasterize_element(&self, element: &OsmElement, grid: &mut TileGrid) -> Result<u32> {
        let tile_type = element.to_tile_type();

        // Skip empty tile types
        if matches!(tile_type, TileType::Empty) {
            return Ok(0);
        }

        let metadata = element.to_tile_metadata();
        let tile = Tile::with_metadata(tile_type, metadata);

        let mut tiles_updated = 0;

        match element.geometry.len() {
            0 => {
                // No geometry, skip
                return Ok(0);
            }
            1 => {
                // Point geometry - place at single location
                let (lat, lon) = element.geometry[0];
                if let Some((x, y)) = grid.geo_to_grid(lat, lon) {
                    if grid
                        .set_tile_with_priority(x, y, tile)
                        .map_err(|e| OsmTilesError::GridGeneration(e))?
                    {
                        tiles_updated += 1;
                    }
                }
            }
            _ => {
                // Line or polygon geometry - rasterize along the path
                tiles_updated += self.rasterize_line(&element.geometry, tile, grid)?;
            }
        }

        Ok(tiles_updated)
    }

    /// Rasterize a line or polygon onto the grid using Bresenham-like algorithm
    fn rasterize_line(
        &self,
        geometry: &[(f64, f64)],
        tile: Tile,
        grid: &mut TileGrid,
    ) -> Result<u32> {
        let mut tiles_updated = 0;

        // For polygons (closed ways), also fill the interior for certain tile types
        let should_fill = matches!(
            tile.tile_type,
            TileType::Building
                | TileType::Water
                | TileType::GreenSpace
                | TileType::Parking
                | TileType::Residential
                | TileType::Commercial
                | TileType::Industrial
        );

        // First, rasterize the outline
        for window in geometry.windows(2) {
            let (lat1, lon1) = window[0];
            let (lat2, lon2) = window[1];

            if let (Some((x1, y1)), Some((x2, y2))) =
                (grid.geo_to_grid(lat1, lon1), grid.geo_to_grid(lat2, lon2))
            {
                tiles_updated += self.draw_line(x1, y1, x2, y2, tile.clone(), grid)?;
            }
        }

        // For filled shapes, use a simple flood fill approach
        if should_fill && geometry.len() >= 3 {
            tiles_updated += self.fill_polygon(geometry, tile, grid)?;
        }

        Ok(tiles_updated)
    }

    /// Draw a line between two points using Bresenham's algorithm
    fn draw_line(
        &self,
        x1: usize,
        y1: usize,
        x2: usize,
        y2: usize,
        tile: Tile,
        grid: &mut TileGrid,
    ) -> Result<u32> {
        let mut tiles_updated = 0;

        let dx = (x2 as i32 - x1 as i32).abs();
        let dy = (y2 as i32 - y1 as i32).abs();
        let sx = if x1 < x2 { 1 } else { -1 };
        let sy = if y1 < y2 { 1 } else { -1 };
        let mut err = dx - dy;

        let mut x = x1 as i32;
        let mut y = y1 as i32;

        loop {
            if x >= 0 && y >= 0 {
                let ux = x as usize;
                let uy = y as usize;
                if grid
                    .set_tile_with_priority(ux, uy, tile.clone())
                    .map_err(|e| OsmTilesError::GridGeneration(e))?
                {
                    tiles_updated += 1;
                }
            }

            if x == x2 as i32 && y == y2 as i32 {
                break;
            }

            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }

        Ok(tiles_updated)
    }

    /// Fill a polygon using scanline algorithm (simplified)
    fn fill_polygon(
        &self,
        geometry: &[(f64, f64)],
        tile: Tile,
        grid: &mut TileGrid,
    ) -> Result<u32> {
        let mut tiles_updated = 0;

        // Find bounding box of polygon in grid coordinates
        let mut min_x = usize::MAX;
        let mut max_x = 0;
        let mut min_y = usize::MAX;
        let mut max_y = 0;

        for (lat, lon) in geometry {
            if let Some((x, y)) = grid.geo_to_grid(*lat, *lon) {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }

        if min_x == usize::MAX {
            return Ok(0); // No valid points
        }

        // Simple point-in-polygon test for each tile in bounding box
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if let Some((lat, lon)) = grid.grid_to_geo(x, y) {
                    if self.point_in_polygon(lat, lon, geometry) {
                        if grid
                            .set_tile_with_priority(x, y, tile.clone())
                            .map_err(|e| OsmTilesError::GridGeneration(e))?
                        {
                            tiles_updated += 1;
                        }
                    }
                }
            }
        }

        Ok(tiles_updated)
    }

    /// Test if a point is inside a polygon using ray casting algorithm
    fn point_in_polygon(&self, lat: f64, lon: f64, polygon: &[(f64, f64)]) -> bool {
        let mut inside = false;
        let mut j = polygon.len() - 1;

        for i in 0..polygon.len() {
            let (lat_i, lon_i) = polygon[i];
            let (lat_j, lon_j) = polygon[j];

            if ((lat_i > lat) != (lat_j > lat))
                && (lon < (lon_j - lon_i) * (lat - lat_i) / (lat_j - lat_i) + lon_i)
            {
                inside = !inside;
            }
            j = i;
        }

        inside
    }
}

impl Default for DefaultGridGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GridGenerator for DefaultGridGenerator {
    async fn generate_grid(&self, osm_data: &OsmData, config: &OsmConfig) -> Result<TileGrid> {
        let start_time = Instant::now();

        tracing::info!("Generating grid from OSM data");

        // Parse OSM data
        let elements = self.parser.parse(osm_data)?;
        tracing::debug!("Parsed {} OSM elements", elements.len());

        // Calculate grid dimensions
        let (grid_width, grid_height) = self.calculate_grid_dimensions(config, osm_data)?;
        let meters_per_tile =
            self.calculate_meters_per_tile(config, osm_data, (grid_width, grid_height));

        tracing::info!(
            "Creating {}x{} grid ({} tiles, ~{:.1}m per tile)",
            grid_width,
            grid_height,
            grid_width * grid_height,
            meters_per_tile
        );

        // Create empty grid
        let mut grid = TileGrid::new(
            grid_width,
            grid_height,
            osm_data.bounding_box.clone(),
            meters_per_tile,
        );

        // Rasterize each element onto the grid
        let mut total_tiles_updated = 0;
        for element in &elements {
            let tiles_updated = self.rasterize_element(element, &mut grid)?;
            total_tiles_updated += tiles_updated;
        }

        let generation_time = start_time.elapsed().as_millis() as u64;

        // Update grid metadata
        grid.metadata.elements_processed = elements.len() as u32;
        grid.metadata.tiles_populated = total_tiles_updated as usize;
        grid.metadata.generation_time_ms = generation_time;
        grid.metadata.algorithm = "default_rasterization".to_string();
        grid.metadata
            .extra
            .insert("grid_width".to_string(), grid_width.to_string());
        grid.metadata
            .extra
            .insert("grid_height".to_string(), grid_height.to_string());
        grid.metadata
            .extra
            .insert("meters_per_tile".to_string(), meters_per_tile.to_string());

        tracing::info!(
            "Grid generation complete: {}/{} tiles populated in {:.1}s",
            total_tiles_updated,
            grid_width * grid_height,
            generation_time as f64 / 1000.0
        );

        Ok(grid)
    }

    fn capabilities(&self) -> GeneratorCapabilities {
        GeneratorCapabilities {
            max_grid_size: Some(self.max_grid_size),
            supported_crs: vec!["EPSG:4326".to_string()],
            supports_parallel: false,
            notes: Some("Default rasterization-based grid generator".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoundingBox, OsmConfigBuilder, OsmDataFormat, OsmMetadata};

    fn create_test_osm_data() -> OsmData {
        let json_data = r#"{
            "elements": [
                {
                    "type": "way",
                    "id": 1,
                    "tags": {"highway": "residential"},
                    "geometry": [
                        {"lat": 52.5, "lon": 13.4},
                        {"lat": 52.501, "lon": 13.401}
                    ]
                },
                {
                    "type": "way",
                    "id": 2,
                    "tags": {"building": "yes"},
                    "geometry": [
                        {"lat": 52.500, "lon": 13.400},
                        {"lat": 52.500, "lon": 13.401},
                        {"lat": 52.501, "lon": 13.401},
                        {"lat": 52.501, "lon": 13.400},
                        {"lat": 52.500, "lon": 13.400}
                    ]
                },
                {
                    "type": "node",
                    "id": 3,
                    "lat": 52.5005,
                    "lon": 13.4005,
                    "tags": {
                        "amenity": "cafe"
                    }
                },
                {
                    "type": "way",
                    "id": 4,
                    "tags": {"natural": "water"},
                    "geometry": [
                        {"lat": 52.502, "lon": 13.402},
                        {"lat": 52.502, "lon": 13.403},
                        {"lat": 52.503, "lon": 13.403},
                        {"lat": 52.503, "lon": 13.402},
                        {"lat": 52.502, "lon": 13.402}
                    ]
                }
            ]
        }"#;

        OsmData {
            raw_data: json_data.to_string(),
            format: OsmDataFormat::Json,
            bounding_box: BoundingBox::new(52.49, 13.39, 52.51, 13.41),
            metadata: OsmMetadata::new("test", "test"),
        }
    }

    #[tokio::test]
    async fn test_grid_generation() {
        let generator = DefaultGridGenerator::new();
        let osm_data = create_test_osm_data();
        let config = OsmConfigBuilder::new().grid_resolution(100).build();

        let grid = generator.generate_grid(&osm_data, &config).await.unwrap();

        // Check basic properties
        let (width, height) = grid.dimensions();
        assert!(width > 0);
        assert!(height > 0);
        assert!(grid.tile_count() > 0);

        // Check metadata
        assert_eq!(grid.metadata.elements_processed, 4); // Updated for 4 elements
        assert!(grid.metadata.tiles_populated > 0);
        assert_eq!(grid.metadata.algorithm, "default_rasterization");
    }

    #[tokio::test]
    async fn test_grid_coordinates_conversion() {
        let generator = DefaultGridGenerator::new();
        let osm_data = create_test_osm_data();
        let config = OsmConfigBuilder::new().grid_resolution(50).build();

        let grid = generator.generate_grid(&osm_data, &config).await.unwrap();

        // Test coordinate conversion
        let test_lat = 52.5;
        let test_lon = 13.4;

        if let Some((x, y)) = grid.geo_to_grid(test_lat, test_lon) {
            let (lat_back, lon_back) = grid.grid_to_geo(x, y).unwrap();

            // Should be approximately the same (within grid resolution)
            assert!((lat_back - test_lat).abs() < 0.01);
            assert!((lon_back - test_lon).abs() < 0.01);
        }
    }

    #[tokio::test]
    async fn test_tile_types_generated() {
        let generator = DefaultGridGenerator::new();
        let osm_data = create_test_osm_data();
        let config = OsmConfigBuilder::new().grid_resolution(200).build(); // Higher resolution for better coverage

        let grid = generator.generate_grid(&osm_data, &config).await.unwrap();
        let stats = grid.statistics();

        println!("Generated tile types: {:?}", stats.tile_type_counts);
        println!("Total tiles populated: {}", stats.non_empty_tiles);

        // Should have some tiles populated
        assert!(stats.non_empty_tiles > 0);

        // Check that we have various tile types (at least some should be present)
        let has_road = stats.tile_type_counts.contains_key(&TileType::Road);
        let has_building = stats.tile_type_counts.contains_key(&TileType::Building);
        let has_water = stats.tile_type_counts.contains_key(&TileType::Water);
        let has_amenity = stats.tile_type_counts.contains_key(&TileType::Amenity);

        // At least one of these should be present
        assert!(
            has_road || has_building || has_water || has_amenity,
            "Expected at least one non-empty tile type, got: {:?}",
            stats.tile_type_counts
        );

        // Should have more empty tiles than filled
        let empty_count = stats.tile_type_counts.get(&TileType::Empty).unwrap_or(&0);
        assert!(*empty_count > stats.non_empty_tiles);
    }

    #[tokio::test]
    async fn test_specific_tile_placement() {
        let generator = DefaultGridGenerator::new();
        let osm_data = create_test_osm_data();
        let config = OsmConfigBuilder::new().grid_resolution(200).build();

        let grid = generator.generate_grid(&osm_data, &config).await.unwrap();

        // Test that elements are actually placed
        let elements = generator.parser.parse(&osm_data).unwrap();
        assert_eq!(elements.len(), 4);

        // Check element types
        let element_types: Vec<TileType> = elements.iter().map(|e| e.to_tile_type()).collect();
        println!("Element tile types: {:?}", element_types);

        // Verify tile type mapping
        assert!(element_types.contains(&TileType::Road));
        assert!(element_types.contains(&TileType::Building));
        assert!(element_types.contains(&TileType::Water));
        assert!(element_types.contains(&TileType::Amenity));

        // Check that tiles were actually placed by examining specific coordinates
        // The road should be at 52.5, 13.4 and 52.501, 13.401
        if let Some((x, y)) = grid.geo_to_grid(52.5, 13.4) {
            println!("Road tile at ({}, {}) -> {:?}", x, y, grid.get_tile(x, y));
        }

        // Count non-empty tiles
        let mut non_empty_count = 0;
        for (x, y, tile) in grid.iter_tiles() {
            if !matches!(tile.tile_type, TileType::Empty) {
                non_empty_count += 1;
                println!("Non-empty tile at ({}, {}): {:?}", x, y, tile.tile_type);
            }
        }

        println!("Total non-empty tiles found: {}", non_empty_count);
        assert!(
            non_empty_count > 0,
            "Expected some non-empty tiles to be placed"
        );
    }

    #[test]
    fn test_point_in_polygon() {
        let generator = DefaultGridGenerator::new();

        // Square polygon
        let polygon = vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];

        // Point inside
        assert!(generator.point_in_polygon(0.5, 0.5, &polygon));

        // Point outside
        assert!(!generator.point_in_polygon(1.5, 0.5, &polygon));

        // Point on edge (may vary depending on implementation)
        // assert!(!generator.point_in_polygon(0.0, 0.5, &polygon));
    }

    #[test]
    fn test_grid_dimensions_calculation() {
        let generator = DefaultGridGenerator::new();
        let osm_data = create_test_osm_data();
        let config = OsmConfigBuilder::new().grid_resolution(100).build();

        let (width, height) = generator
            .calculate_grid_dimensions(&config, &osm_data)
            .unwrap();

        // Should be reasonable size
        assert!(width >= 10);
        assert!(height >= 10);
        assert!(width <= 5000);
        assert!(height <= 5000);
    }

    #[tokio::test]
    async fn test_generator_capabilities() {
        let generator = DefaultGridGenerator::new();
        let capabilities = generator.capabilities();

        assert_eq!(capabilities.max_grid_size, Some((5000, 5000)));
        assert!(
            capabilities
                .supported_crs
                .contains(&"EPSG:4326".to_string())
        );
        assert!(!capabilities.supports_parallel);
    }

    #[test]
    fn test_tile_type_mapping() {
        let generator = DefaultGridGenerator::new();
        let osm_data = create_test_osm_data();
        let elements = generator.parser.parse(&osm_data).unwrap();

        // Test individual element mapping
        for element in &elements {
            let tile_type = element.to_tile_type();
            println!(
                "Element {} with tags {:?} -> {:?}",
                element.id, element.tags, tile_type
            );

            match element.id {
                1 => assert_eq!(tile_type, TileType::Road), // highway: residential
                2 => assert_eq!(tile_type, TileType::Building), // building: yes
                3 => assert_eq!(tile_type, TileType::Amenity), // amenity: cafe
                4 => assert_eq!(tile_type, TileType::Water), // natural: water
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_element_rasterization() {
        let generator = DefaultGridGenerator::new();
        let osm_data = create_test_osm_data();
        let config = OsmConfigBuilder::new().grid_resolution(100).build();

        // Create a grid
        let (grid_width, grid_height) = generator
            .calculate_grid_dimensions(&config, &osm_data)
            .unwrap();
        let meters_per_tile =
            generator.calculate_meters_per_tile(&config, &osm_data, (grid_width, grid_height));
        let mut grid = TileGrid::new(
            grid_width,
            grid_height,
            osm_data.bounding_box.clone(),
            meters_per_tile,
        );

        // Parse elements
        let elements = generator.parser.parse(&osm_data).unwrap();

        // Test rasterizing individual elements
        for element in &elements {
            let tiles_updated = generator.rasterize_element(element, &mut grid).unwrap();
            println!("Element {} updated {} tiles", element.id, tiles_updated);

            if !matches!(element.to_tile_type(), TileType::Empty) {
                assert!(
                    tiles_updated > 0,
                    "Expected non-empty element to update some tiles"
                );
            }
        }

        // Verify final grid has some non-empty tiles
        let stats = grid.statistics();
        assert!(stats.non_empty_tiles > 0);
    }
}
