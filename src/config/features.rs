use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Standard OSM feature types that can be included in grid generation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OsmFeature {
    // Transportation
    Roads,
    Highways,
    Footpaths,
    Railways,

    // Buildings & Structures
    Buildings,
    Residential,
    Commercial,
    Industrial,

    // Natural Features
    Water,
    Rivers,
    Lakes,
    Forests,
    Parks,
    Grassland,

    // Urban Features
    Parking,
    Amenities,
    Tourism,

    // Infrastructure
    PowerLines,
    Boundaries,
    Landuse,
}

impl OsmFeature {
    /// Get the OSM tag queries for this feature
    pub fn to_osm_queries(&self) -> Vec<OsmTagQuery> {
        match self {
            Self::Roads => vec![
                OsmTagQuery::new("highway", Some("primary")),
                OsmTagQuery::new("highway", Some("secondary")),
                OsmTagQuery::new("highway", Some("tertiary")),
                OsmTagQuery::new("highway", Some("residential")),
                OsmTagQuery::new("highway", Some("unclassified")),
            ],
            Self::Highways => vec![
                OsmTagQuery::new("highway", Some("motorway")),
                OsmTagQuery::new("highway", Some("trunk")),
                OsmTagQuery::new("highway", Some("primary")),
            ],
            Self::Footpaths => vec![
                OsmTagQuery::new("highway", Some("footway")),
                OsmTagQuery::new("highway", Some("path")),
                OsmTagQuery::new("highway", Some("pedestrian")),
                OsmTagQuery::new("highway", Some("steps")),
            ],
            Self::Railways => vec![OsmTagQuery::new("railway", None::<String>)],
            Self::Buildings => vec![OsmTagQuery::new("building", None::<String>)],
            Self::Residential => vec![
                OsmTagQuery::new("building", Some("residential")),
                OsmTagQuery::new("landuse", Some("residential")),
            ],
            Self::Commercial => vec![
                OsmTagQuery::new("building", Some("commercial")),
                OsmTagQuery::new("landuse", Some("commercial")),
                OsmTagQuery::new("building", Some("retail")),
            ],
            Self::Industrial => vec![
                OsmTagQuery::new("building", Some("industrial")),
                OsmTagQuery::new("landuse", Some("industrial")),
            ],
            Self::Water => vec![
                OsmTagQuery::new("natural", Some("water")),
                OsmTagQuery::new("waterway", None::<String>),
            ],
            Self::Rivers => vec![
                OsmTagQuery::new("waterway", Some("river")),
                OsmTagQuery::new("waterway", Some("stream")),
            ],
            Self::Lakes => vec![
                OsmTagQuery::new("natural", Some("water")),
                OsmTagQuery::new("water", Some("lake")),
            ],
            Self::Forests => vec![
                OsmTagQuery::new("natural", Some("wood")),
                OsmTagQuery::new("landuse", Some("forest")),
            ],
            Self::Parks => vec![
                OsmTagQuery::new("leisure", Some("park")),
                OsmTagQuery::new("leisure", Some("garden")),
            ],
            Self::Grassland => vec![
                OsmTagQuery::new("landuse", Some("grass")),
                OsmTagQuery::new("natural", Some("grassland")),
            ],
            Self::Parking => vec![
                OsmTagQuery::new("amenity", Some("parking")),
                OsmTagQuery::new("landuse", Some("parking")),
            ],
            Self::Amenities => vec![OsmTagQuery::new("amenity", None::<String>)],
            Self::Tourism => vec![OsmTagQuery::new("tourism", None::<String>)],
            Self::PowerLines => vec![
                OsmTagQuery::new("power", Some("line")),
                OsmTagQuery::new("power", Some("tower")),
            ],
            Self::Boundaries => vec![OsmTagQuery::new("boundary", None::<String>)],
            Self::Landuse => vec![OsmTagQuery::new("landuse", None::<String>)],
        }
    }

    /// Get a human-readable description of this feature
    pub fn description(&self) -> &'static str {
        match self {
            Self::Roads => "Local roads and streets",
            Self::Highways => "Major highways and motorways",
            Self::Footpaths => "Walking paths and pedestrian areas",
            Self::Railways => "Railway lines and stations",
            Self::Buildings => "All building structures",
            Self::Residential => "Residential buildings and areas",
            Self::Commercial => "Commercial buildings and retail areas",
            Self::Industrial => "Industrial buildings and zones",
            Self::Water => "All water features",
            Self::Rivers => "Rivers and streams",
            Self::Lakes => "Lakes and ponds",
            Self::Forests => "Forests and wooded areas",
            Self::Parks => "Parks and recreational areas",
            Self::Grassland => "Grass and meadow areas",
            Self::Parking => "Parking areas and lots",
            Self::Amenities => "Public amenities and services",
            Self::Tourism => "Tourist attractions and facilities",
            Self::PowerLines => "Power lines and electrical infrastructure",
            Self::Boundaries => "Administrative and other boundaries",
            Self::Landuse => "General land use classifications",
        }
    }
}

/// Represents an OSM tag query
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OsmTagQuery {
    pub key: String,
    pub value: Option<String>,
}

impl OsmTagQuery {
    pub fn new(key: impl Into<String>, value: Option<impl Into<String>>) -> Self {
        Self {
            key: key.into(),
            value: value.map(|v| v.into()),
        }
    }

    /// Convert to Overpass QL format
    pub fn to_overpass_filter(&self) -> String {
        match &self.value {
            Some(value) => format!("[\"{}\"][\"{}\"]", self.key, value),
            None => format!("[\"{}\"]", self.key),
        }
    }
}

/// A set of features to include in OSM data fetching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureSet {
    /// Standard features to include
    features: HashSet<OsmFeature>,
    /// Custom OSM tag queries
    custom_queries: Vec<OsmTagQuery>,
}

impl FeatureSet {
    /// Create a new empty feature set
    pub fn new() -> Self {
        Self {
            features: HashSet::new(),
            custom_queries: Vec::new(),
        }
    }

    /// Create a feature set with basic urban features
    pub fn urban() -> Self {
        Self::new().with_features(vec![
            OsmFeature::Roads,
            OsmFeature::Buildings,
            OsmFeature::Parks,
            OsmFeature::Water,
        ])
    }

    /// Create a feature set for transportation analysis
    pub fn transportation() -> Self {
        Self::new().with_features(vec![
            OsmFeature::Roads,
            OsmFeature::Highways,
            OsmFeature::Railways,
            OsmFeature::Footpaths,
            OsmFeature::Parking,
        ])
    }

    /// Create a feature set for natural features
    pub fn natural() -> Self {
        Self::new().with_features(vec![
            OsmFeature::Water,
            OsmFeature::Rivers,
            OsmFeature::Lakes,
            OsmFeature::Forests,
            OsmFeature::Parks,
            OsmFeature::Grassland,
        ])
    }

    /// Create a comprehensive feature set with most common features
    pub fn comprehensive() -> Self {
        Self::new().with_features(vec![
            OsmFeature::Roads,
            OsmFeature::Highways,
            OsmFeature::Buildings,
            OsmFeature::Residential,
            OsmFeature::Commercial,
            OsmFeature::Water,
            OsmFeature::Parks,
            OsmFeature::Forests,
            OsmFeature::Railways,
            OsmFeature::Amenities,
        ])
    }

    /// Add features to this set
    pub fn with_features(mut self, features: Vec<OsmFeature>) -> Self {
        self.features.extend(features);
        self
    }

    /// Add a single feature
    pub fn with_feature(mut self, feature: OsmFeature) -> Self {
        self.features.insert(feature);
        self
    }

    /// Add custom OSM tag queries
    pub fn with_custom_queries(mut self, queries: Vec<OsmTagQuery>) -> Self {
        self.custom_queries.extend(queries);
        self
    }

    /// Add a single custom query
    pub fn with_custom_query(mut self, query: OsmTagQuery) -> Self {
        self.custom_queries.push(query);
        self
    }

    /// Remove a feature from this set
    pub fn without_feature(mut self, feature: &OsmFeature) -> Self {
        self.features.remove(feature);
        self
    }

    /// Check if a feature is included
    pub fn contains_feature(&self, feature: &OsmFeature) -> bool {
        self.features.contains(feature)
    }

    /// Get all OSM tag queries for this feature set
    pub fn to_osm_queries(&self) -> Vec<OsmTagQuery> {
        let mut queries = Vec::new();

        // Add queries from standard features
        for feature in &self.features {
            queries.extend(feature.to_osm_queries());
        }

        // Add custom queries
        queries.extend(self.custom_queries.clone());

        // Remove duplicates
        queries.sort_by(|a, b| a.key.cmp(&b.key).then(a.value.cmp(&b.value)));
        queries.dedup();

        queries
    }

    /// Get the list of included features
    pub fn features(&self) -> &HashSet<OsmFeature> {
        &self.features
    }

    /// Get the custom queries
    pub fn custom_queries(&self) -> &[OsmTagQuery] {
        &self.custom_queries
    }

    /// Check if the feature set is empty
    pub fn is_empty(&self) -> bool {
        self.features.is_empty() && self.custom_queries.is_empty()
    }

    /// Get the total number of features and custom queries
    pub fn len(&self) -> usize {
        self.features.len() + self.custom_queries.len()
    }
}

impl Default for FeatureSet {
    fn default() -> Self {
        Self::urban()
    }
}

impl From<Vec<OsmFeature>> for FeatureSet {
    fn from(features: Vec<OsmFeature>) -> Self {
        Self::new().with_features(features)
    }
}

impl From<OsmFeature> for FeatureSet {
    fn from(feature: OsmFeature) -> Self {
        Self::new().with_feature(feature)
    }
}
