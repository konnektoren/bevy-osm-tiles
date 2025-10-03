use clap::Parser;
use tracing::{error, info, warn};
use tracing_subscriber;

use bevy_osm_tiles::{
    FeatureSet, OsmConfig, OsmConfigBuilder, OsmDataProvider, OsmFeature, ProviderFactory, Region,
};

#[derive(Parser)]
#[command(name = "osm-city-loader")]
#[command(about = "Load OpenStreetMap data for a city and generate grid tiles")]
struct Args {
    /// City name to load OSM data for
    #[arg(short, long)]
    city: String,

    /// Feature preset to use: urban, transportation, natural, comprehensive, gaming
    #[arg(short, long, default_value = "urban")]
    features: String,

    /// Data provider: overpass, mock
    #[arg(short, long, default_value = "overpass")]
    provider: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Test connection only
    #[arg(short, long)]
    test: bool,

    /// Simulate network delay (for mock provider, in milliseconds)
    #[arg(long)]
    delay: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = Args::parse();

    // Initialize tracing
    let level = if args.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt().with_max_level(level).init();

    info!("🌍 OSM City Loader starting...");
    info!("📍 Target city: {}", args.city);
    info!("🎯 Feature preset: {}", args.features);
    info!("🔌 Provider: {}", args.provider);

    // Create the appropriate provider
    let provider: Box<dyn OsmDataProvider> = match args.provider.as_str() {
        "overpass" => Box::new(ProviderFactory::overpass()),
        "mock" => {
            let mut mock_provider = ProviderFactory::mock();
            if let Some(delay_ms) = args.delay {
                mock_provider = ProviderFactory::mock_with_delay(delay_ms);
            }
            Box::new(mock_provider)
        }
        _ => {
            error!(
                "Unknown provider: {}. Available providers: {:?}",
                args.provider,
                ProviderFactory::available_providers()
            );
            return Err("Invalid provider".to_string());
        }
    };

    // Show provider capabilities
    let capabilities = provider.capabilities();
    info!("🔧 Provider capabilities:");
    info!("  - Real-time data: {}", capabilities.supports_real_time);
    info!("  - Requires network: {}", capabilities.requires_network);
    info!(
        "  - Supports geocoding: {}",
        capabilities.supports_geocoding
    );
    info!("  - WASM compatible: {}", capabilities.wasm_compatible);
    if let Some(max_area) = capabilities.max_area_km2 {
        info!("  - Max area: {:.1} km²", max_area);
    }
    if let Some(rate_limit) = capabilities.rate_limit_rpm {
        info!("  - Rate limit: {} requests/minute", rate_limit);
    }
    if let Some(notes) = &capabilities.notes {
        info!("  - Notes: {}", notes);
    }

    if args.test {
        info!("🔍 Testing provider availability...");
        match provider.test_availability().await {
            Ok(()) => {
                info!("✅ Provider is available!");
                return Ok(());
            }
            Err(e) => {
                error!("❌ Provider test failed: {}", e);
                return Err(e.to_string());
            }
        }
    }

    // Create configuration with selected feature preset
    let feature_set = match args.features.as_str() {
        "urban" => FeatureSet::urban(),
        "transportation" => FeatureSet::transportation(),
        "natural" => FeatureSet::natural(),
        "comprehensive" => FeatureSet::comprehensive(),
        "gaming" => FeatureSet::urban()
            .with_feature(OsmFeature::Amenities)
            .with_feature(OsmFeature::Tourism),
        _ => {
            warn!(
                "Unknown feature preset: {}. Using 'urban' instead.",
                args.features
            );
            FeatureSet::urban()
        }
    };

    let config = OsmConfigBuilder::new()
        .city(&args.city)
        .features(feature_set)
        .grid_resolution(50)
        .timeout(60)
        .build();

    info!("⚙️  Configuration created successfully");
    info!(
        "📊 Features included: {:?}",
        config.features.features().iter().collect::<Vec<_>>()
    );
    info!(
        "🔧 Custom queries: {}",
        config.features.custom_queries().len()
    );

    // Resolve the region
    info!("🗺️  Resolving region...");
    match provider.resolve_region(&config.region).await {
        Ok(bbox) => {
            info!("📍 Resolved to bounding box: {:?}", bbox);
            info!(
                "📏 Area: {:.3}° x {:.3}° (lat x lon)",
                bbox.height(),
                bbox.width()
            );
            info!("🎯 Center: {:.3}, {:.3}", bbox.center().0, bbox.center().1);
            info!("📐 Approximate area: {:.2} km²", bbox.area_km2());
        }
        Err(e) => {
            error!("❌ Failed to resolve region: {}", e);
            return Err(e.to_string());
        }
    }

    // Fetch OSM data
    info!("⬇️  Fetching OSM data...");
    match provider.fetch_data(&config).await {
        Ok(osm_data) => {
            info!("✅ Successfully fetched OSM data!");
            info!("📊 Data format: {:?}", osm_data.format);
            info!(
                "📝 Data size: {} bytes ({:.1} KB)",
                osm_data.raw_data.len(),
                osm_data.raw_data.len() as f64 / 1024.0
            );
            info!("🕒 Fetched at: {}", osm_data.metadata.timestamp);
            info!("🌐 Source: {}", osm_data.metadata.source);
            info!("🔌 Provider: {}", osm_data.metadata.provider_type);

            if let Some(count) = osm_data.metadata.element_count {
                info!("📊 Elements: {}", count);
            }

            if let Some(time) = osm_data.metadata.processing_time_ms {
                info!("⏱️  Processing time: {:.1}s", time as f64 / 1000.0);
            }

            if args.verbose {
                info!("📄 First 200 characters of data:");
                let preview = if osm_data.raw_data.len() > 200 {
                    &osm_data.raw_data[..200]
                } else {
                    &osm_data.raw_data
                };
                info!("{}", preview);
            }
        }
        Err(e) => {
            error!("❌ Failed to fetch OSM data: {}", e);
            return Err(e.to_string());
        }
    }

    info!("🎉 City loader completed successfully!");
    Ok(())
}
