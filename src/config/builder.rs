use super::{FeatureSet, OsmConfig, OsmFeature, OsmTagQuery, Region};

/// Builder for creating OSM configurations with a fluent API
#[derive(Debug, Clone)]
pub struct OsmConfigBuilder {
    region: Option<Region>,
    grid_resolution: Option<u32>,
    tile_size: Option<f32>,
    timeout_seconds: Option<u64>,
    features: FeatureSet,
}

impl OsmConfigBuilder {
    /// Create a new builder with default values
    pub fn new() -> Self {
        Self {
            region: None,
            grid_resolution: None,
            tile_size: None,
            timeout_seconds: None,
            features: FeatureSet::new(),
        }
    }

    /// Set the region to download data for
    pub fn region(mut self, region: Region) -> Self {
        self.region = Some(region);
        self
    }

    /// Set the region to a city name
    pub fn city(mut self, name: impl Into<String>) -> Self {
        self.region = Some(Region::city(name));
        self
    }

    /// Set the region to a bounding box
    pub fn bbox(mut self, south: f64, west: f64, north: f64, east: f64) -> Self {
        self.region = Some(Region::bbox(south, west, north, east));
        self
    }

    /// Set the region to a center point with radius
    pub fn center_radius(mut self, lat: f64, lon: f64, radius_km: f64) -> Self {
        self.region = Some(Region::center_radius(lat, lon, radius_km));
        self
    }

    /// Set the grid resolution
    pub fn grid_resolution(mut self, resolution: u32) -> Self {
        self.grid_resolution = Some(resolution);
        self
    }

    /// Set the tile size
    pub fn tile_size(mut self, size: f32) -> Self {
        self.tile_size = Some(size);
        self
    }

    /// Set the timeout
    pub fn timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = Some(seconds);
        self
    }

    /// Use a predefined feature set
    pub fn features(mut self, features: FeatureSet) -> Self {
        self.features = features;
        self
    }

    /// Add features from a list
    pub fn with_features(mut self, features: Vec<OsmFeature>) -> Self {
        self.features = self.features.with_features(features);
        self
    }

    /// Add a single feature
    pub fn with_feature(mut self, feature: OsmFeature) -> Self {
        self.features = self.features.with_feature(feature);
        self
    }

    /// Remove a feature
    pub fn without_feature(mut self, feature: OsmFeature) -> Self {
        self.features = self.features.without_feature(&feature);
        self
    }

    /// Add custom OSM tag queries
    pub fn with_custom_queries(mut self, queries: Vec<OsmTagQuery>) -> Self {
        self.features = self.features.with_custom_queries(queries);
        self
    }

    /// Add a single custom query
    pub fn with_custom_query(
        mut self,
        key: impl Into<String>,
        value: Option<impl Into<String>>,
    ) -> Self {
        let query = OsmTagQuery::new(key, value);
        self.features = self.features.with_custom_query(query);
        self
    }

    /// Use urban feature preset (roads, buildings, parks, water)
    pub fn urban_features(mut self) -> Self {
        self.features = FeatureSet::urban();
        self
    }

    /// Use transportation feature preset
    pub fn transportation_features(mut self) -> Self {
        self.features = FeatureSet::transportation();
        self
    }

    /// Use natural feature preset
    pub fn natural_features(mut self) -> Self {
        self.features = FeatureSet::natural();
        self
    }

    /// Use comprehensive feature preset
    pub fn comprehensive_features(mut self) -> Self {
        self.features = FeatureSet::comprehensive();
        self
    }

    /// Build the final configuration
    pub fn build(self) -> OsmConfig {
        OsmConfig {
            region: self.region.unwrap_or_else(|| Region::city("Berlin")),
            grid_resolution: self.grid_resolution.unwrap_or(100),
            tile_size: self.tile_size.unwrap_or(10.0),
            timeout_seconds: self.timeout_seconds.unwrap_or(30),
            features: self.features,
        }
    }
}

impl Default for OsmConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience methods for common configurations
impl OsmConfigBuilder {
    /// Create a gaming-focused configuration with visual features
    pub fn for_gaming() -> Self {
        Self::new()
            .urban_features()
            .with_feature(OsmFeature::Amenities)
            .with_feature(OsmFeature::Tourism)
            .grid_resolution(200)
            .tile_size(5.0)
    }

    /// Create a navigation-focused configuration
    pub fn for_navigation() -> Self {
        Self::new()
            .transportation_features()
            .with_feature(OsmFeature::Buildings)
            .with_feature(OsmFeature::Amenities)
            .grid_resolution(150)
            .tile_size(8.0)
    }

    /// Create an urban planning focused configuration
    pub fn for_urban_planning() -> Self {
        Self::new()
            .comprehensive_features()
            .with_feature(OsmFeature::Boundaries)
            .with_feature(OsmFeature::Landuse)
            .grid_resolution(300)
            .tile_size(3.0)
    }

    /// Create a natural environment focused configuration
    pub fn for_environment() -> Self {
        Self::new()
            .natural_features()
            .with_feature(OsmFeature::Landuse)
            .grid_resolution(100)
            .tile_size(15.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_new() {
        let builder = OsmConfigBuilder::new();
        let config = builder.build();

        // Should use defaults
        assert_eq!(config.grid_resolution, 100);
        assert_eq!(config.tile_size, 10.0);
        assert_eq!(config.timeout_seconds, 30);

        // Should default to Berlin
        match config.region {
            Region::City { name } => assert_eq!(name, "Berlin"),
            _ => panic!("Expected City region"),
        }
    }

    #[test]
    fn test_builder_region_methods() {
        // Test city method
        let config = OsmConfigBuilder::new().city("Munich").build();
        match config.region {
            Region::City { name } => assert_eq!(name, "Munich"),
            _ => panic!("Expected City region"),
        }

        // Test bbox method
        let config = OsmConfigBuilder::new().bbox(52.0, 13.0, 53.0, 14.0).build();
        match config.region {
            Region::BoundingBox(bbox) => {
                assert_eq!(bbox.south, 52.0);
                assert_eq!(bbox.west, 13.0);
                assert_eq!(bbox.north, 53.0);
                assert_eq!(bbox.east, 14.0);
            }
            _ => panic!("Expected BoundingBox region"),
        }

        // Test center_radius method
        let config = OsmConfigBuilder::new()
            .center_radius(52.5, 13.4, 5.0)
            .build();
        match config.region {
            Region::CenterRadius {
                lat,
                lon,
                radius_km,
            } => {
                assert_eq!(lat, 52.5);
                assert_eq!(lon, 13.4);
                assert_eq!(radius_km, 5.0);
            }
            _ => panic!("Expected CenterRadius region"),
        }

        // Test region method with Region enum
        let bbox_region = Region::bbox(50.0, 10.0, 51.0, 11.0);
        let config = OsmConfigBuilder::new().region(bbox_region).build();
        match config.region {
            Region::BoundingBox(bbox) => {
                assert_eq!(bbox.south, 50.0);
                assert_eq!(bbox.west, 10.0);
            }
            _ => panic!("Expected BoundingBox region"),
        }
    }

    #[test]
    fn test_builder_configuration_methods() {
        let config = OsmConfigBuilder::new()
            .grid_resolution(200)
            .tile_size(5.0)
            .timeout(120)
            .build();

        assert_eq!(config.grid_resolution, 200);
        assert_eq!(config.tile_size, 5.0);
        assert_eq!(config.timeout_seconds, 120);
    }

    #[test]
    fn test_builder_feature_methods() {
        // Test with predefined feature set
        let config = OsmConfigBuilder::new()
            .features(FeatureSet::transportation())
            .build();

        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Highways));
        assert!(config.features.contains_feature(&OsmFeature::Railways));

        // Test with individual features
        let config = OsmConfigBuilder::new()
            .with_feature(OsmFeature::Buildings)
            .with_feature(OsmFeature::Water)
            .build();

        assert!(config.features.contains_feature(&OsmFeature::Buildings));
        assert!(config.features.contains_feature(&OsmFeature::Water));
        assert!(!config.features.contains_feature(&OsmFeature::Roads));

        // Test with multiple features
        let features = vec![OsmFeature::Roads, OsmFeature::Buildings, OsmFeature::Parks];
        let config = OsmConfigBuilder::new()
            .with_features(features.clone())
            .build();

        for feature in features {
            assert!(config.features.contains_feature(&feature));
        }
    }

    #[test]
    fn test_builder_feature_removal() {
        let config = OsmConfigBuilder::new()
            .urban_features() // Includes Roads, Buildings, Parks, Water
            .without_feature(OsmFeature::Water)
            .build();

        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Buildings));
        assert!(config.features.contains_feature(&OsmFeature::Parks));
        assert!(!config.features.contains_feature(&OsmFeature::Water));
    }

    #[test]
    fn test_builder_custom_queries() {
        let queries = vec![
            OsmTagQuery::new("shop", Some("supermarket")),
            OsmTagQuery::new("emergency", Some("hospital")),
        ];

        let config = OsmConfigBuilder::new()
            .with_custom_queries(queries.clone())
            .build();

        assert_eq!(config.features.custom_queries().len(), 2);
        for query in &queries {
            assert!(config.features.custom_queries().contains(query));
        }

        // Test single custom query
        let config = OsmConfigBuilder::new()
            .with_custom_query("shop", Some("bakery"))
            .build();

        assert_eq!(config.features.custom_queries().len(), 1);
        assert_eq!(
            config.features.custom_queries()[0],
            OsmTagQuery::new("shop", Some("bakery"))
        );
    }

    #[test]
    fn test_builder_feature_presets() {
        // Test urban preset
        let config = OsmConfigBuilder::new().urban_features().build();
        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Buildings));
        assert!(config.features.contains_feature(&OsmFeature::Parks));
        assert!(config.features.contains_feature(&OsmFeature::Water));

        // Test transportation preset
        let config = OsmConfigBuilder::new().transportation_features().build();
        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Highways));
        assert!(config.features.contains_feature(&OsmFeature::Railways));
        assert!(config.features.contains_feature(&OsmFeature::Footpaths));
        assert!(config.features.contains_feature(&OsmFeature::Parking));

        // Test natural preset
        let config = OsmConfigBuilder::new().natural_features().build();
        assert!(config.features.contains_feature(&OsmFeature::Water));
        assert!(config.features.contains_feature(&OsmFeature::Rivers));
        assert!(config.features.contains_feature(&OsmFeature::Lakes));
        assert!(config.features.contains_feature(&OsmFeature::Forests));
        assert!(config.features.contains_feature(&OsmFeature::Parks));
        assert!(config.features.contains_feature(&OsmFeature::Grassland));

        // Test comprehensive preset
        let config = OsmConfigBuilder::new().comprehensive_features().build();
        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Highways));
        assert!(config.features.contains_feature(&OsmFeature::Buildings));
        assert!(config.features.contains_feature(&OsmFeature::Residential));
        assert!(config.features.contains_feature(&OsmFeature::Commercial));
        assert!(config.features.contains_feature(&OsmFeature::Water));
        assert!(config.features.contains_feature(&OsmFeature::Parks));
        assert!(config.features.contains_feature(&OsmFeature::Forests));
        assert!(config.features.contains_feature(&OsmFeature::Railways));
        assert!(config.features.contains_feature(&OsmFeature::Amenities));
    }

    #[test]
    fn test_builder_convenience_constructors() {
        // Test for_gaming
        let config = OsmConfigBuilder::for_gaming().build();
        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Buildings));
        assert!(config.features.contains_feature(&OsmFeature::Parks));
        assert!(config.features.contains_feature(&OsmFeature::Water));
        assert!(config.features.contains_feature(&OsmFeature::Amenities));
        assert!(config.features.contains_feature(&OsmFeature::Tourism));
        assert_eq!(config.grid_resolution, 200);
        assert_eq!(config.tile_size, 5.0);

        // Test for_navigation
        let config = OsmConfigBuilder::for_navigation().build();
        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Highways));
        assert!(config.features.contains_feature(&OsmFeature::Railways));
        assert!(config.features.contains_feature(&OsmFeature::Buildings));
        assert!(config.features.contains_feature(&OsmFeature::Amenities));
        assert_eq!(config.grid_resolution, 150);
        assert_eq!(config.tile_size, 8.0);

        // Test for_urban_planning
        let config = OsmConfigBuilder::for_urban_planning().build();
        assert!(config.features.contains_feature(&OsmFeature::Boundaries));
        assert!(config.features.contains_feature(&OsmFeature::Landuse));
        assert_eq!(config.grid_resolution, 300);
        assert_eq!(config.tile_size, 3.0);

        // Test for_environment
        let config = OsmConfigBuilder::for_environment().build();
        assert!(config.features.contains_feature(&OsmFeature::Water));
        assert!(config.features.contains_feature(&OsmFeature::Forests));
        assert!(config.features.contains_feature(&OsmFeature::Landuse));
        assert_eq!(config.grid_resolution, 100);
        assert_eq!(config.tile_size, 15.0);
    }

    #[test]
    fn test_builder_chaining() {
        let config = OsmConfigBuilder::new()
            .city("Vienna")
            .grid_resolution(150)
            .tile_size(7.5)
            .timeout(45)
            .urban_features()
            .with_feature(OsmFeature::Tourism)
            .without_feature(OsmFeature::Water)
            .with_custom_query("shop", Some("bakery"))
            .build();

        // Check region
        match config.region {
            Region::City { name } => assert_eq!(name, "Vienna"),
            _ => panic!("Expected City region"),
        }

        // Check configuration
        assert_eq!(config.grid_resolution, 150);
        assert_eq!(config.tile_size, 7.5);
        assert_eq!(config.timeout_seconds, 45);

        // Check features
        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Buildings));
        assert!(config.features.contains_feature(&OsmFeature::Parks));
        assert!(config.features.contains_feature(&OsmFeature::Tourism));
        assert!(!config.features.contains_feature(&OsmFeature::Water));

        // Check custom query
        assert_eq!(config.features.custom_queries().len(), 1);
        assert!(
            config
                .features
                .custom_queries()
                .contains(&OsmTagQuery::new("shop", Some("bakery")))
        );
    }

    #[test]
    fn test_builder_default() {
        let builder1 = OsmConfigBuilder::new();
        let builder2 = OsmConfigBuilder::default();

        let config1 = builder1.build();
        let config2 = builder2.build();

        // Should be equivalent
        assert_eq!(config1.grid_resolution, config2.grid_resolution);
        assert_eq!(config1.tile_size, config2.tile_size);
        assert_eq!(config1.timeout_seconds, config2.timeout_seconds);
    }

    #[test]
    fn test_builder_complex_scenario() {
        // Simulate a complex real-world configuration
        let config = OsmConfigBuilder::for_gaming()
            .city("Tokyo")
            .grid_resolution(250)
            .with_feature(OsmFeature::Railways) // Add railways for Japan
            .with_custom_query("railway", Some("station"))
            .with_custom_query("building", Some("temple"))
            .timeout(90)
            .build();

        // Verify the complex setup
        match config.region {
            Region::City { name } => assert_eq!(name, "Tokyo"),
            _ => panic!("Expected City region"),
        }

        assert_eq!(config.grid_resolution, 250);
        assert_eq!(config.timeout_seconds, 90);

        // Should have gaming features plus railways
        assert!(config.features.contains_feature(&OsmFeature::Roads));
        assert!(config.features.contains_feature(&OsmFeature::Buildings));
        assert!(config.features.contains_feature(&OsmFeature::Amenities));
        assert!(config.features.contains_feature(&OsmFeature::Tourism));
        assert!(config.features.contains_feature(&OsmFeature::Railways));

        // Should have custom queries for Japanese-specific features
        assert_eq!(config.features.custom_queries().len(), 2);
        assert!(
            config
                .features
                .custom_queries()
                .contains(&OsmTagQuery::new("railway", Some("station")))
        );
        assert!(
            config
                .features
                .custom_queries()
                .contains(&OsmTagQuery::new("building", Some("temple")))
        );
    }
}
