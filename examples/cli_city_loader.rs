use clap::Parser;
use tracing::{error, info};
use tracing_subscriber;

use bevy_osm_tiles::{
    FeatureSet, OsmConfig, OsmConfigBuilder, OsmDownloader, OsmFeature, OverpassDownloader, Region,
};

#[derive(Parser)]
#[command(name = "osm-city-loader")]
#[command(about = "Load OpenStreetMap data for a city and generate grid tiles")]
struct Args {
    /// City name to load OSM data for
    #[arg(short, long)]
    city: String,

    /// Feature preset to use: urban, transportation, natural, comprehensive
    #[arg(short, long, default_value = "urban")]
    features: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Test connection only
    #[arg(short, long)]
    test: bool,
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

    // Create downloader
    let downloader = OverpassDownloader::new();

    if args.test {
        info!("🔍 Testing connection to Overpass API...");
        match downloader.test_connection().await {
            Ok(()) => {
                info!("✅ Connection test successful!");
                return Ok(());
            }
            Err(e) => {
                error!("❌ Connection test failed: {}", e);
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
            error!(
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

    // First, resolve the region
    match config.region {
        Region::City { ref name } => {
            info!("🗺️  Resolving city '{}' to coordinates...", name);
            match downloader.resolve_region(&config.region).await {
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
                    error!("❌ Failed to resolve city: {}", e);
                    return Err(e.to_string());
                }
            }
        }
        _ => {}
    }

    // Download OSM data
    info!("⬇️  Downloading OSM data...");
    match downloader.download(&config).await {
        Ok(osm_data) => {
            info!("✅ Successfully downloaded OSM data!");
            info!("📊 Data format: {:?}", osm_data.format);
            info!("📝 Data size: {} bytes", osm_data.raw_data.len());
            info!("🕒 Downloaded at: {}", osm_data.metadata.timestamp);
            info!("🌐 Source: {}", osm_data.metadata.source);

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
            error!("❌ Failed to download OSM data: {}", e);
            return Err(e.to_string());
        }
    }

    info!("🎉 City loader completed successfully!");
    Ok(())
}
