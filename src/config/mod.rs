mod builder;
mod features;
mod region;

pub use builder::*;
pub use features::*;
pub use region::*;

use serde::{Deserialize, Serialize};

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
    pub features: FeatureSet,
}

impl Default for OsmConfig {
    fn default() -> Self {
        Self {
            region: Region::city("Berlin"),
            grid_resolution: 100,
            tile_size: 10.0,
            timeout_seconds: 30,
            features: FeatureSet::default(),
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
    pub fn with_features(mut self, features: FeatureSet) -> Self {
        self.features = features;
        self
    }

    /// Create a builder for more complex configuration
    pub fn builder() -> OsmConfigBuilder {
        OsmConfigBuilder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osm_config_default() {
        let config = OsmConfig::default();

        // Check defaults
        assert_eq!(config.grid_resolution, 100);
        assert_eq!(config.tile_size, 10.0);
        assert_eq!(config.timeout_seconds, 30);

        // Should default to Berlin
        match config.region {
            Region::City { name } => assert_eq!(name, "Berlin"),
            _ => panic!("Expected City region"),
        }

        // Should have urban features by default
        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Buildings));
        assert!(config.features.contains_feature(&OsmFeature::Parks));
        assert!(config.features.contains_feature(&OsmFeature::Water));
    }

    #[test]
    fn test_osm_config_for_city() {
        let config = OsmConfig::for_city("Munich");

        match config.region {
            Region::City { name } => assert_eq!(name, "Munich"),
            _ => panic!("Expected City region"),
        }

        // Other fields should be default
        assert_eq!(config.grid_resolution, 100);
        assert_eq!(config.tile_size, 10.0);
        assert_eq!(config.timeout_seconds, 30);
    }

    #[test]
    fn test_osm_config_builder_methods() {
        let config = OsmConfig::for_city("Hamburg")
            .with_grid_resolution(150)
            .with_tile_size(7.5)
            .with_timeout(45)
            .with_features(FeatureSet::transportation());

        match config.region {
            Region::City { name } => assert_eq!(name, "Hamburg"),
            _ => panic!("Expected City region"),
        }

        assert_eq!(config.grid_resolution, 150);
        assert_eq!(config.tile_size, 7.5);
        assert_eq!(config.timeout_seconds, 45);

        // Should have transportation features
        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Highways));
        assert!(config.features.contains_feature(&OsmFeature::Railways));
        assert!(!config.features.contains_feature(&OsmFeature::Buildings));
    }

    #[test]
    fn test_osm_config_builder_creation() {
        let builder = OsmConfig::builder();
        let config = builder.city("Vienna").grid_resolution(200).build();

        match config.region {
            Region::City { name } => assert_eq!(name, "Vienna"),
            _ => panic!("Expected City region"),
        }

        assert_eq!(config.grid_resolution, 200);
    }

    #[test]
    fn test_osm_config_serialization() {
        let config = OsmConfig::for_city("Berlin")
            .with_grid_resolution(150)
            .with_tile_size(5.0)
            .with_features(FeatureSet::urban());

        // Test JSON serialization
        let json = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: OsmConfig = serde_json::from_str(&json).expect("Failed to deserialize");

        // Check that all fields are preserved
        assert_eq!(config.grid_resolution, deserialized.grid_resolution);
        assert_eq!(config.tile_size, deserialized.tile_size);
        assert_eq!(config.timeout_seconds, deserialized.timeout_seconds);

        // Check region
        match (&config.region, &deserialized.region) {
            (Region::City { name: n1 }, Region::City { name: n2 }) => assert_eq!(n1, n2),
            _ => panic!("Region types don't match"),
        }

        // Check features
        assert_eq!(config.features.features(), deserialized.features.features());
        assert_eq!(
            config.features.custom_queries(),
            deserialized.features.custom_queries()
        );
    }

    #[test]
    fn test_osm_config_complex_scenario() {
        // Test a complex configuration with all features
        let custom_queries = vec![
            OsmTagQuery::new("shop", Some("supermarket")),
            OsmTagQuery::new("emergency", Some("hospital")),
        ];

        let config = OsmConfig::builder()
            .center_radius(48.1351, 11.5820, 10.0) // Munich coordinates
            .grid_resolution(300)
            .tile_size(3.0)
            .timeout(120)
            .comprehensive_features()
            .with_feature(OsmFeature::Tourism)
            .with_custom_queries(custom_queries.clone())
            .build();

        // Check region
        match config.region {
            Region::CenterRadius {
                lat,
                lon,
                radius_km,
            } => {
                assert_eq!(lat, 48.1351);
                assert_eq!(lon, 11.5820);
                assert_eq!(radius_km, 10.0);
            }
            _ => panic!("Expected CenterRadius region"),
        }

        // Check configuration
        assert_eq!(config.grid_resolution, 300);
        assert_eq!(config.tile_size, 3.0);
        assert_eq!(config.timeout_seconds, 120);

        // Check comprehensive features
        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Highways));
        assert!(config.features.contains_feature(&OsmFeature::Buildings));
        assert!(config.features.contains_feature(&OsmFeature::Water));
        assert!(config.features.contains_feature(&OsmFeature::Railways));
        assert!(config.features.contains_feature(&OsmFeature::Amenities));
        assert!(config.features.contains_feature(&OsmFeature::Tourism)); // Added feature

        // Check custom queries
        assert_eq!(config.features.custom_queries().len(), 2);
        for query in &custom_queries {
            assert!(config.features.custom_queries().contains(query));
        }
    }

    #[test]
    fn test_osm_config_method_chaining() {
        // Test that all builder methods can be chained
        let config = OsmConfig::for_city("Test City")
            .with_grid_resolution(250)
            .with_tile_size(2.5)
            .with_timeout(60)
            .with_features(
                FeatureSet::new()
                    .with_feature(OsmFeature::Roads)
                    .with_feature(OsmFeature::Buildings)
                    .with_custom_query(OsmTagQuery::new("test", Some("value"))),
            );

        match config.region {
            Region::City { name } => assert_eq!(name, "Test City"),
            _ => panic!("Expected City region"),
        }

        assert_eq!(config.grid_resolution, 250);
        assert_eq!(config.tile_size, 2.5);
        assert_eq!(config.timeout_seconds, 60);
        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Buildings));
        assert_eq!(config.features.custom_queries().len(), 1);
    }

    #[test]
    fn test_osm_config_validation() {
        // Test that configurations can have various valid values
        let configs = vec![
            OsmConfig::for_city("A")
                .with_grid_resolution(1)
                .with_tile_size(0.1)
                .with_timeout(1),
            OsmConfig::for_city("B")
                .with_grid_resolution(1000)
                .with_tile_size(100.0)
                .with_timeout(3600),
            OsmConfig::builder()
                .bbox(-90.0, -180.0, 90.0, 180.0)
                .build(), // World bbox
        ];

        for config in configs {
            // Should be valid configurations
            assert!(config.grid_resolution > 0);
            assert!(config.tile_size > 0.0);
            assert!(config.timeout_seconds > 0);
        }
    }
}
