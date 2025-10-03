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
