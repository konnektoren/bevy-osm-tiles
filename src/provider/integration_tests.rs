#[cfg(test)]
mod integration_tests {
    use super::super::*;
    use crate::{FeatureSet, OsmConfigBuilder, Region};

    #[tokio::test]
    async fn test_provider_comparison() {
        // Test that both providers can handle the same configuration
        let config = OsmConfigBuilder::new()
            .city("test")
            .features(FeatureSet::urban())
            .build();

        // Mock provider should always work
        let mock_provider = ProviderFactory::mock();
        let mock_result = mock_provider.fetch_data(&config).await.unwrap();
        assert_eq!(mock_result.metadata.provider_type, "mock");
        assert!(!mock_result.raw_data.is_empty());

        // Both providers should resolve the same region type
        let region = Region::bbox(52.0, 13.0, 53.0, 14.0);

        let mock_bbox = mock_provider.resolve_region(&region).await.unwrap();
        let overpass_provider = ProviderFactory::overpass();
        let overpass_bbox = overpass_provider.resolve_region(&region).await.unwrap();

        // Bounding boxes should be identical for bbox regions
        assert_eq!(mock_bbox.south, overpass_bbox.south);
        assert_eq!(mock_bbox.west, overpass_bbox.west);
        assert_eq!(mock_bbox.north, overpass_bbox.north);
        assert_eq!(mock_bbox.east, overpass_bbox.east);
    }

    #[tokio::test]
    async fn test_provider_capabilities_differences() {
        let mock_provider = ProviderFactory::mock();
        let overpass_provider = ProviderFactory::overpass();

        let mock_caps = mock_provider.capabilities();
        let overpass_caps = overpass_provider.capabilities();

        // Both should be WASM compatible
        assert!(mock_caps.wasm_compatible);
        assert!(overpass_caps.wasm_compatible);

        // Overpass requires network, mock doesn't
        assert!(!mock_caps.requires_network);
        assert!(overpass_caps.requires_network);

        // Overpass provides real-time data, mock doesn't
        assert!(!mock_caps.supports_real_time);
        assert!(overpass_caps.supports_real_time);

        // Both support geocoding
        assert!(mock_caps.supports_geocoding);
        assert!(overpass_caps.supports_geocoding);
    }

    #[tokio::test]
    async fn test_provider_factory_consistency() {
        // Test that factory methods create consistent providers
        let provider1 = ProviderFactory::overpass();
        let provider2 = ProviderFactory::overpass();

        // Should have same configuration
        assert_eq!(provider1.base_url, provider2.base_url);
        assert_eq!(provider1.provider_type(), provider2.provider_type());

        // Test custom URL consistency
        let custom_url = "https://test.example.com/api";
        let custom1 = ProviderFactory::overpass_with_url(custom_url);
        let custom2 = ProviderFactory::overpass_with_url(custom_url);

        assert_eq!(custom1.base_url, custom2.base_url);
        assert_eq!(custom1.base_url, custom_url);
    }

    #[tokio::test]
    async fn test_provider_error_handling() {
        // Test mock provider with failure
        let failing_provider = ProviderFactory::mock().with_failure();
        let config = OsmConfigBuilder::new().city("test").build();

        let result = failing_provider.fetch_data(&config).await;
        assert!(result.is_err());

        let availability = failing_provider.test_availability().await;
        assert!(availability.is_err());

        // Test invalid city with mock provider
        let provider = ProviderFactory::mock();
        let result = provider
            .resolve_region(&Region::city("invalid_city_name"))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_provider_metadata_consistency() {
        let provider = ProviderFactory::mock();
        let config = OsmConfigBuilder::new()
            .city("test")
            .features(FeatureSet::transportation())
            .build();

        let result = provider.fetch_data(&config).await.unwrap();
        let metadata = result.metadata;

        // Check required metadata fields
        assert!(!metadata.timestamp.is_empty());
        assert!(!metadata.source.is_empty());
        assert!(!metadata.provider_type.is_empty());

        // Check consistency
        assert_eq!(metadata.provider_type, provider.provider_type());

        // Timestamp should be recent (within last minute)
        let timestamp = chrono::DateTime::parse_from_rfc3339(&metadata.timestamp).unwrap();
        let now = chrono::Utc::now();
        let diff = now.signed_duration_since(timestamp.with_timezone(&chrono::Utc));
        assert!(diff.num_seconds() < 60);
    }

    #[tokio::test]
    async fn test_different_feature_sets() {
        let provider = ProviderFactory::mock();

        let configs = vec![
            OsmConfigBuilder::new().urban_features().build(),
            OsmConfigBuilder::new().transportation_features().build(),
            OsmConfigBuilder::new().natural_features().build(),
            OsmConfigBuilder::new().comprehensive_features().build(),
        ];

        for config in configs {
            let result = provider.fetch_data(&config).await.unwrap();
            assert!(!result.raw_data.is_empty());
            assert_eq!(result.metadata.provider_type, "mock");
            // Mock provider should handle all feature sets
        }
    }

    #[tokio::test]
    async fn test_region_type_handling() {
        let provider = ProviderFactory::mock();

        let regions = vec![
            Region::bbox(52.0, 13.0, 53.0, 14.0),
            Region::center_radius(52.5, 13.4, 10.0),
            Region::city("berlin"),
        ];

        for region in regions {
            let config = OsmConfigBuilder::new().region(region).build();

            let result = provider.fetch_data(&config).await.unwrap();
            assert!(!result.raw_data.is_empty());

            // Bounding box should be reasonable
            let bbox = result.bounding_box;
            assert!(bbox.north > bbox.south);
            assert!(bbox.east > bbox.west);
            assert!(bbox.area_km2() > 0.0);
        }
    }
}
