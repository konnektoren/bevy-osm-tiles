use serde_json::Value;
use std::collections::HashMap;

use super::{TileMetadata, TileType};
use crate::{OsmData, OsmDataFormat, OsmTilesError, Result};

/// Represents a parsed OSM element
#[derive(Debug, Clone)]
pub struct OsmElement {
    pub id: i64,
    pub element_type: OsmElementType,
    pub tags: HashMap<String, String>,
    pub geometry: Vec<(f64, f64)>, // (lat, lon) pairs
}

/// Type of OSM element
#[derive(Debug, Clone, PartialEq)]
pub enum OsmElementType {
    Node,
    Way,
    Relation,
}

impl OsmElement {
    /// Determine the tile type for this OSM element based on its tags
    pub fn to_tile_type(&self) -> TileType {
        // Priority-based matching - more specific first

        // Buildings
        if self.tags.contains_key("building") {
            match self.tags.get("building").map(|s| s.as_str()) {
                Some("residential") => TileType::Residential,
                Some("commercial") | Some("retail") => TileType::Commercial,
                Some("industrial") => TileType::Industrial,
                _ => TileType::Building,
            }
        }
        // Highways and roads
        else if self.tags.contains_key("highway") {
            TileType::Road
        }
        // Water features
        else if self.tags.contains_key("waterway")
            || self.tags.get("natural") == Some(&"water".to_string())
        {
            TileType::Water
        }
        // Green spaces
        else if self.tags.get("leisure") == Some(&"park".to_string())
            || self.tags.get("leisure") == Some(&"garden".to_string())
            || self.tags.get("landuse") == Some(&"forest".to_string())
            || self.tags.get("natural") == Some(&"wood".to_string())
            || self.tags.get("landuse") == Some(&"grass".to_string())
        {
            TileType::GreenSpace
        }
        // Railways
        else if self.tags.contains_key("railway") {
            TileType::Railway
        }
        // Parking
        else if self.tags.get("amenity") == Some(&"parking".to_string())
            || self.tags.get("landuse") == Some(&"parking".to_string())
        {
            TileType::Parking
        }
        // Amenities
        else if self.tags.contains_key("amenity") {
            TileType::Amenity
        }
        // Tourism
        else if self.tags.contains_key("tourism") {
            TileType::Tourism
        }
        // Land use
        else if let Some(landuse) = self.tags.get("landuse") {
            match landuse.as_str() {
                "residential" => TileType::Residential,
                "commercial" | "retail" => TileType::Commercial,
                "industrial" => TileType::Industrial,
                _ => TileType::Custom(format!("landuse_{}", landuse)),
            }
        }
        // Default
        else {
            TileType::Empty
        }
    }

    /// Create tile metadata from this element
    pub fn to_tile_metadata(&self) -> TileMetadata {
        TileMetadata {
            osm_ids: vec![self.id],
            tags: self.tags.clone(),
            confidence: 1.0,
        }
    }

    /// Get the center point of this element's geometry
    pub fn center_point(&self) -> Option<(f64, f64)> {
        if self.geometry.is_empty() {
            return None;
        }

        let lat_sum: f64 = self.geometry.iter().map(|(lat, _)| lat).sum();
        let lon_sum: f64 = self.geometry.iter().map(|(_, lon)| lon).sum();
        let count = self.geometry.len() as f64;

        Some((lat_sum / count, lon_sum / count))
    }

    /// Get the bounding box of this element
    pub fn bounding_box(&self) -> Option<(f64, f64, f64, f64)> {
        if self.geometry.is_empty() {
            return None;
        }

        let mut min_lat = f64::INFINITY;
        let mut max_lat = f64::NEG_INFINITY;
        let mut min_lon = f64::INFINITY;
        let mut max_lon = f64::NEG_INFINITY;

        for (lat, lon) in &self.geometry {
            min_lat = min_lat.min(*lat);
            max_lat = max_lat.max(*lat);
            min_lon = min_lon.min(*lon);
            max_lon = max_lon.max(*lon);
        }

        Some((min_lat, min_lon, max_lat, max_lon))
    }
}

/// Parser for OSM data
pub struct OsmParser;

impl OsmParser {
    /// Parse OSM data into a list of elements
    pub fn parse(&self, osm_data: &OsmData) -> Result<Vec<OsmElement>> {
        match osm_data.format {
            OsmDataFormat::Json => self.parse_json(&osm_data.raw_data),
            OsmDataFormat::Xml => self.parse_xml(&osm_data.raw_data),
        }
    }

    /// Parse Overpass JSON format
    fn parse_json(&self, json_data: &str) -> Result<Vec<OsmElement>> {
        let parsed: Value = serde_json::from_str(json_data)
            .map_err(|e| OsmTilesError::Parse(format!("Invalid JSON: {}", e)))?;

        let elements = parsed
            .get("elements")
            .and_then(|e| e.as_array())
            .ok_or_else(|| OsmTilesError::Parse("No 'elements' array found in JSON".to_string()))?;

        let mut osm_elements = Vec::new();

        for element in elements {
            if let Some(osm_element) = self.parse_json_element(element)? {
                osm_elements.push(osm_element);
            }
        }

        Ok(osm_elements)
    }

    /// Parse a single JSON element
    fn parse_json_element(&self, element: &Value) -> Result<Option<OsmElement>> {
        let id = element
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| OsmTilesError::Parse("Element missing 'id'".to_string()))?;

        let element_type_str = element
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OsmTilesError::Parse("Element missing 'type'".to_string()))?;

        let element_type = match element_type_str {
            "node" => OsmElementType::Node,
            "way" => OsmElementType::Way,
            "relation" => OsmElementType::Relation,
            _ => return Ok(None), // Skip unknown types
        };

        // Parse tags
        let mut tags = HashMap::new();
        if let Some(tags_obj) = element.get("tags").and_then(|v| v.as_object()) {
            for (key, value) in tags_obj {
                if let Some(value_str) = value.as_str() {
                    tags.insert(key.clone(), value_str.to_string());
                }
            }
        }

        // Parse geometry
        let geometry = match element_type {
            OsmElementType::Node => {
                // For nodes, use lat/lon directly
                let lat = element
                    .get("lat")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| OsmTilesError::Parse("Node missing 'lat'".to_string()))?;
                let lon = element
                    .get("lon")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| OsmTilesError::Parse("Node missing 'lon'".to_string()))?;
                vec![(lat, lon)]
            }
            OsmElementType::Way | OsmElementType::Relation => {
                // For ways and relations, use geometry array
                if let Some(geometry_array) = element.get("geometry").and_then(|v| v.as_array()) {
                    let mut coords = Vec::new();
                    for coord in geometry_array {
                        let lat = coord.get("lat").and_then(|v| v.as_f64()).ok_or_else(|| {
                            OsmTilesError::Parse("Geometry point missing 'lat'".to_string())
                        })?;
                        let lon = coord.get("lon").and_then(|v| v.as_f64()).ok_or_else(|| {
                            OsmTilesError::Parse("Geometry point missing 'lon'".to_string())
                        })?;
                        coords.push((lat, lon));
                    }
                    coords
                } else {
                    // If no geometry, try to use lat/lon (for some nodes)
                    if let (Some(lat), Some(lon)) = (
                        element.get("lat").and_then(|v| v.as_f64()),
                        element.get("lon").and_then(|v| v.as_f64()),
                    ) {
                        vec![(lat, lon)]
                    } else {
                        Vec::new() // No geometry available
                    }
                }
            }
        };

        // Skip elements without geometry or tags
        if geometry.is_empty() && tags.is_empty() {
            return Ok(None);
        }

        Ok(Some(OsmElement {
            id,
            element_type,
            tags,
            geometry,
        }))
    }

    /// Parse XML format (basic implementation)
    fn parse_xml(&self, _xml_data: &str) -> Result<Vec<OsmElement>> {
        // For now, return an error - XML parsing is more complex
        // In a full implementation, you'd use an XML parser like `quick-xml`
        Err(OsmTilesError::Parse(
            "XML parsing not yet implemented - use JSON format".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OsmMetadata;

    fn create_test_osm_data() -> OsmData {
        let json_data = r#"{
            "elements": [
                {
                    "type": "node",
                    "id": 1001,
                    "lat": 52.5,
                    "lon": 13.4,
                    "tags": {
                        "amenity": "cafe",
                        "name": "Test Cafe"
                    }
                },
                {
                    "type": "way",
                    "id": 2001,
                    "tags": {
                        "highway": "residential",
                        "name": "Test Street"
                    },
                    "geometry": [
                        {"lat": 52.499, "lon": 13.399},
                        {"lat": 52.501, "lon": 13.401}
                    ]
                },
                {
                    "type": "way",
                    "id": 3001,
                    "tags": {
                        "building": "residential"
                    },
                    "geometry": [
                        {"lat": 52.500, "lon": 13.400},
                        {"lat": 52.500, "lon": 13.401},
                        {"lat": 52.501, "lon": 13.401},
                        {"lat": 52.501, "lon": 13.400},
                        {"lat": 52.500, "lon": 13.400}
                    ]
                }
            ]
        }"#;

        OsmData {
            raw_data: json_data.to_string(),
            format: OsmDataFormat::Json,
            bounding_box: crate::BoundingBox::new(52.0, 13.0, 53.0, 14.0),
            metadata: OsmMetadata::new("test", "test"),
        }
    }

    #[test]
    fn test_parse_json() {
        let parser = OsmParser;
        let osm_data = create_test_osm_data();

        let elements = parser.parse(&osm_data).unwrap();
        assert_eq!(elements.len(), 3);

        // Check cafe (node)
        let cafe = &elements[0];
        assert_eq!(cafe.id, 1001);
        assert_eq!(cafe.element_type, OsmElementType::Node);
        assert_eq!(cafe.tags.get("amenity"), Some(&"cafe".to_string()));
        assert_eq!(cafe.geometry.len(), 1);
        assert_eq!(cafe.geometry[0], (52.5, 13.4));

        // Check street (way)
        let street = &elements[1];
        assert_eq!(street.id, 2001);
        assert_eq!(street.element_type, OsmElementType::Way);
        assert_eq!(street.tags.get("highway"), Some(&"residential".to_string()));
        assert_eq!(street.geometry.len(), 2);

        // Check building (way)
        let building = &elements[2];
        assert_eq!(building.id, 3001);
        assert_eq!(building.element_type, OsmElementType::Way);
        assert_eq!(
            building.tags.get("building"),
            Some(&"residential".to_string())
        );
        assert_eq!(building.geometry.len(), 5); // Closed polygon
    }

    #[test]
    fn test_tile_type_mapping() {
        let mut element = OsmElement {
            id: 1,
            element_type: OsmElementType::Way,
            tags: HashMap::new(),
            geometry: vec![(52.5, 13.4)],
        };

        // Test building
        element
            .tags
            .insert("building".to_string(), "yes".to_string());
        assert_eq!(element.to_tile_type(), TileType::Building);

        // Test residential building
        element
            .tags
            .insert("building".to_string(), "residential".to_string());
        assert_eq!(element.to_tile_type(), TileType::Residential);

        // Test road
        element.tags.clear();
        element
            .tags
            .insert("highway".to_string(), "residential".to_string());
        assert_eq!(element.to_tile_type(), TileType::Road);

        // Test water
        element.tags.clear();
        element
            .tags
            .insert("natural".to_string(), "water".to_string());
        assert_eq!(element.to_tile_type(), TileType::Water);

        // Test amenity
        element.tags.clear();
        element
            .tags
            .insert("amenity".to_string(), "cafe".to_string());
        assert_eq!(element.to_tile_type(), TileType::Amenity);
    }

    #[test]
    fn test_element_center_point() {
        let element = OsmElement {
            id: 1,
            element_type: OsmElementType::Way,
            tags: HashMap::new(),
            geometry: vec![(52.0, 13.0), (52.1, 13.1), (52.2, 13.2)],
        };

        let center = element.center_point().unwrap();
        assert!((center.0 - 52.1).abs() < 0.001);
        assert!((center.1 - 13.1).abs() < 0.001);
    }

    #[test]
    fn test_element_bounding_box() {
        let element = OsmElement {
            id: 1,
            element_type: OsmElementType::Way,
            tags: HashMap::new(),
            geometry: vec![(52.0, 13.0), (52.2, 13.2), (52.1, 13.1)],
        };

        let bbox = element.bounding_box().unwrap();
        assert_eq!(bbox, (52.0, 13.0, 52.2, 13.2)); // (min_lat, min_lon, max_lat, max_lon)
    }
}
