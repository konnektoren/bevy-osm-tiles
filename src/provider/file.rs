use async_trait::async_trait;
use std::path::Path;

use super::{OsmData, OsmDataProvider, ProviderCapabilities};
use crate::{BoundingBox, OsmConfig, OsmDataFormat, OsmMetadata, OsmTilesError, Region, Result};

/// File-based provider for loading OSM data from local files
pub struct FileProvider {
    /// Path to the OSM data file
    file_path: String,
    /// Optional bounding box if known
    known_bbox: Option<BoundingBox>,
}

impl FileProvider {
    /// Create a new file provider
    pub fn new(file_path: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            known_bbox: None,
        }
    }

    /// Set a known bounding box for the file data
    pub fn with_bbox(mut self, bbox: BoundingBox) -> Self {
        self.known_bbox = Some(bbox);
        self
    }

    /// Detect file format from extension
    fn detect_format(&self) -> OsmDataFormat {
        let path = Path::new(&self.file_path);
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("xml") | Some("osm") => OsmDataFormat::Xml,
            Some("json") => OsmDataFormat::Json,
            _ => OsmDataFormat::Json, // Default to JSON
        }
    }

    /// Extract bounding box from Overpass JSON data
    fn extract_bbox_from_json(&self, data: &str) -> Option<BoundingBox> {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
            if let Some(elements) = value.get("elements").and_then(|e| e.as_array()) {
                let mut min_lat = f64::INFINITY;
                let mut max_lat = f64::NEG_INFINITY;
                let mut min_lon = f64::INFINITY;
                let mut max_lon = f64::NEG_INFINITY;

                for element in elements {
                    if let Some(geometry) = element.get("geometry").and_then(|g| g.as_array()) {
                        for point in geometry {
                            if let (Some(lat), Some(lon)) = (
                                point.get("lat").and_then(|v| v.as_f64()),
                                point.get("lon").and_then(|v| v.as_f64()),
                            ) {
                                min_lat = min_lat.min(lat);
                                max_lat = max_lat.max(lat);
                                min_lon = min_lon.min(lon);
                                max_lon = max_lon.max(lon);
                            }
                        }
                    }
                }

                if min_lat != f64::INFINITY {
                    return Some(BoundingBox::new(min_lat, min_lon, max_lat, max_lon));
                }
            }
        }
        None
    }
}

#[async_trait]
impl OsmDataProvider for FileProvider {
    fn provider_type(&self) -> &'static str {
        "file"
    }

    async fn fetch_data(&self, _config: &OsmConfig) -> Result<OsmData> {
        let data = tokio::fs::read_to_string(&self.file_path)
            .await
            .map_err(|e| {
                OsmTilesError::Config(format!("Failed to read file '{}': {}", self.file_path, e))
            })?;

        let format = self.detect_format();

        // Try to determine bounding box
        let bbox = self
            .known_bbox
            .clone()
            .or_else(|| {
                if matches!(format, OsmDataFormat::Json) {
                    self.extract_bbox_from_json(&data)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                // Default bbox if we can't determine it
                BoundingBox::new(0.0, 0.0, 1.0, 1.0)
            });

        let metadata = OsmMetadata::new(&self.file_path, self.provider_type())
            .with_extra("file_size", data.len().to_string())
            .with_extra("format", format!("{:?}", format));

        Ok(OsmData {
            raw_data: data,
            format,
            bounding_box: bbox,
            metadata,
        })
    }

    async fn resolve_region(&self, region: &Region) -> Result<BoundingBox> {
        match region {
            Region::BoundingBox(bbox) => Ok(bbox.clone()),
            _ => {
                // File provider can't resolve cities or center+radius
                // Return the known bbox or a default
                Ok(self
                    .known_bbox
                    .clone()
                    .unwrap_or_else(|| BoundingBox::new(0.0, 0.0, 1.0, 1.0)))
            }
        }
    }

    async fn test_availability(&self) -> Result<()> {
        if tokio::fs::metadata(&self.file_path).await.is_ok() {
            Ok(())
        } else {
            Err(OsmTilesError::Config(format!(
                "File not found: {}",
                self.file_path
            )))
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_real_time: false,
            requires_network: false,
            supports_geocoding: false,
            max_area_km2: None,
            supported_formats: vec![OsmDataFormat::Json, OsmDataFormat::Xml],
            rate_limit_rpm: None,
            notes: Some("Local file-based OSM data".to_string()),
        }
    }
}
