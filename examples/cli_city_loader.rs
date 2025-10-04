use clap::Parser;
use tracing::{error, info, warn};
use tracing_subscriber;

use bevy_osm_tiles::{
    DefaultGridGenerator, FeatureSet, GridGenerator, OsmConfigBuilder, OsmDataProvider, OsmFeature,
    ProviderFactory, TileGrid, TileType,
};

use image::{ImageBuffer, Rgb, RgbImage};

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

    /// Grid resolution (cells per degree)
    #[arg(short, long, default_value = "100")]
    grid_resolution: u32,

    /// Output PNG file path (optional)
    #[arg(short, long)]
    output: Option<String>,

    /// Skip grid generation (only fetch OSM data)
    #[arg(long)]
    skip_grid: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Test connection only
    #[arg(short, long)]
    test: bool,

    /// Simulate network delay (for mock provider, in milliseconds)
    #[arg(long)]
    delay: Option<u64>,

    /// Show detailed grid statistics
    #[arg(long)]
    grid_stats: bool,
}

/// Configuration for loading city data
#[derive(Clone, Debug)]
pub struct CityLoadConfig {
    pub city: String,
    pub features: String,
    pub provider: String,
    pub grid_resolution: u32,
    pub delay: Option<u64>,
    pub verbose: bool,
    pub timeout: u32,
}

impl Default for CityLoadConfig {
    fn default() -> Self {
        Self {
            city: String::new(),
            features: "urban".to_string(),
            provider: "mock".to_string(),
            grid_resolution: 100,
            delay: None,
            verbose: false,
            timeout: 60,
        }
    }
}

impl From<&Args> for CityLoadConfig {
    fn from(args: &Args) -> Self {
        Self {
            city: args.city.clone(),
            features: args.features.clone(),
            provider: args.provider.clone(),
            grid_resolution: args.grid_resolution,
            delay: args.delay,
            verbose: args.verbose,
            timeout: 60,
        }
    }
}

/// Load city data and return the generated grid
pub async fn load_city_data(config: CityLoadConfig) -> Result<TileGrid, String> {
    if config.verbose {
        info!("🌍 Loading city data for: {}", config.city);
        info!("🎯 Feature preset: {}", config.features);
        info!("🔌 Provider: {}", config.provider);
        info!(
            "🔢 Grid resolution: {} cells/degree",
            config.grid_resolution
        );
    }

    // Create the appropriate provider
    let provider: Box<dyn OsmDataProvider> = match config.provider.as_str() {
        "overpass" => Box::new(ProviderFactory::overpass()),
        "mock" => {
            if let Some(delay_ms) = config.delay {
                Box::new(ProviderFactory::mock_with_delay(delay_ms))
            } else {
                Box::new(ProviderFactory::mock())
            }
        }
        _ => {
            let available = ProviderFactory::available_providers();
            error!(
                "Unknown provider: {}. Available providers: {:?}",
                config.provider, available
            );
            return Err("Invalid provider".to_string());
        }
    };

    // Show provider capabilities if verbose
    if config.verbose {
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
    }

    // Create configuration with selected feature preset
    let feature_set = match config.features.as_str() {
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
                config.features
            );
            FeatureSet::urban()
        }
    };

    let osm_config = OsmConfigBuilder::new()
        .city(&config.city)
        .features(feature_set)
        .grid_resolution(config.grid_resolution)
        .timeout(config.timeout.into())
        .build();

    if config.verbose {
        info!("⚙️  Configuration created successfully");
        info!(
            "📊 Features included: {:?}",
            osm_config.features.features().iter().collect::<Vec<_>>()
        );
        info!(
            "🔧 Custom queries: {}",
            osm_config.features.custom_queries().len()
        );
    }

    // Resolve the region
    if config.verbose {
        info!("🗺️  Resolving region...");
    }
    let bbox = match provider.resolve_region(&osm_config.region).await {
        Ok(bbox) => {
            if config.verbose {
                info!("📍 Resolved to bounding box: {:?}", bbox);
                info!(
                    "📏 Area: {:.3}° x {:.3}° (lat x lon)",
                    bbox.height(),
                    bbox.width()
                );
                info!("🎯 Center: {:.3}, {:.3}", bbox.center().0, bbox.center().1);
                info!("📐 Approximate area: {:.2} km²", bbox.area_km2());
            }
            bbox
        }
        Err(e) => {
            error!("❌ Failed to resolve region: {}", e);
            return Err(e.to_string());
        }
    };

    // Fetch OSM data
    if config.verbose {
        info!("⬇️  Fetching OSM data...");
    }
    let osm_data = match provider.fetch_data(&osm_config).await {
        Ok(osm_data) => {
            if config.verbose {
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
            }
            osm_data
        }
        Err(e) => {
            error!("❌ Failed to fetch OSM data: {}", e);
            return Err(e.to_string());
        }
    };

    // Generate grid
    if config.verbose {
        info!("🔲 Generating tile grid...");
    }

    let generator = DefaultGridGenerator::new();
    if config.verbose {
        let generator_caps = generator.capabilities();
        info!("🔧 Grid generator capabilities:");
        if let Some(max_size) = generator_caps.max_grid_size {
            info!("  - Max grid size: {}x{}", max_size.0, max_size.1);
        }
        info!("  - Supported CRS: {:?}", generator_caps.supported_crs);
        info!(
            "  - Parallel processing: {}",
            generator_caps.supports_parallel
        );
        if let Some(notes) = &generator_caps.notes {
            info!("  - Notes: {}", notes);
        }
    }

    match generator.generate_grid(&osm_data, &osm_config).await {
        Ok(grid) => {
            if config.verbose {
                info!("✅ Grid generation completed!");
                let (width, height) = grid.dimensions();
                info!(
                    "📐 Grid dimensions: {}x{} ({} total tiles)",
                    width,
                    height,
                    grid.tile_count()
                );
                info!("📏 Meters per tile: ~{:.1}m", grid.meters_per_tile);

                // Show grid metadata
                info!("📊 Grid metadata:");
                info!(
                    "  - Elements processed: {}",
                    grid.metadata.elements_processed
                );
                info!("  - Tiles populated: {}", grid.metadata.tiles_populated);
                info!(
                    "  - Generation time: {:.1}s",
                    grid.metadata.generation_time_ms as f64 / 1000.0
                );
                info!("  - Algorithm: {}", grid.metadata.algorithm);

                // Show grid statistics
                let stats = grid.statistics();
                info!("📈 Grid statistics:");
                info!("  - Coverage ratio: {:.1}%", stats.coverage_ratio * 100.0);
                info!(
                    "  - Non-empty tiles: {}/{}",
                    stats.non_empty_tiles, stats.total_tiles
                );
            }
            Ok(grid)
        }
        Err(e) => {
            error!("❌ Failed to generate grid: {}", e);
            Err(e.to_string())
        }
    }
}

/// Show detailed grid statistics
pub fn show_grid_stats(grid: &TileGrid) {
    let stats = grid.statistics();

    info!("🎨 Tile type distribution:");
    let mut type_counts: Vec<_> = stats.tile_type_counts.iter().collect();
    type_counts.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending

    for (tile_type, count) in type_counts.iter().take(10) {
        // Show top 10
        let percentage = **count as f64 / stats.total_tiles as f64 * 100.0;
        if **count > 0 {
            let color = tile_type.default_color();
            info!(
                "  - {:12}: {:6} tiles ({:5.1}%) [RGB: {:3},{:3},{:3}]",
                tile_type.name(),
                count,
                percentage,
                color.0,
                color.1,
                color.2
            );
        }
    }
}

/// Show detailed grid analysis including sample areas and tile locations
pub fn show_detailed_grid_analysis(grid: &TileGrid) {
    info!("🔍 Detailed grid analysis:");

    // Find some interesting tiles
    let road_tiles = grid.tiles_of_type(&TileType::Road);
    let building_tiles = grid.tiles_of_type(&TileType::Building);
    let water_tiles = grid.tiles_of_type(&TileType::Water);

    if !road_tiles.is_empty() {
        let (x, y, _) = road_tiles[0];
        if let Some((lat, lon)) = grid.grid_to_geo(x, y) {
            info!(
                "  - First road tile at grid ({}, {}) -> geo ({:.6}, {:.6})",
                x, y, lat, lon
            );
        }
    }

    if !building_tiles.is_empty() {
        let (x, y, tile) = &building_tiles[0];
        if let Some((lat, lon)) = grid.grid_to_geo(*x, *y) {
            info!(
                "  - First building tile at grid ({}, {}) -> geo ({:.6}, {:.6})",
                x, y, lat, lon
            );
            if let Some(metadata) = &tile.metadata {
                info!("    - OSM IDs: {:?}", metadata.osm_ids);
                if !metadata.tags.is_empty() {
                    info!("    - Tags: {:?}", metadata.tags);
                }
            }
        }
    }

    if !water_tiles.is_empty() {
        info!("  - Water tiles found: {}", water_tiles.len());
    }

    // Show a small sample area around center
    let (grid_width, grid_height) = grid.dimensions();
    let center_x = grid_width / 2;
    let center_y = grid_height / 2;

    let sample_size = 5;
    let start_x = center_x.saturating_sub(sample_size / 2);
    let start_y = center_y.saturating_sub(sample_size / 2);

    if let Some(area) = grid.get_area(start_x, start_y, sample_size, sample_size) {
        info!(
            "  - {}x{} sample area around center:",
            sample_size, sample_size
        );
        for (y, row) in area.iter().enumerate() {
            let row_str: String = row
                .iter()
                .map(|tile| match tile.tile_type {
                    TileType::Empty => ".",
                    TileType::Road => "R",
                    TileType::Building => "B",
                    TileType::Water => "W",
                    TileType::GreenSpace => "G",
                    TileType::Railway => "T",
                    TileType::Parking => "P",
                    TileType::Amenity => "A",
                    TileType::Tourism => "U",
                    TileType::Industrial => "I",
                    TileType::Residential => "H",
                    TileType::Commercial => "C",
                    TileType::Custom(_) => "X",
                })
                .collect();
            info!("    [{}]: {}", start_y + y, row_str);
        }
    }
}

/// Save grid as JSON file
pub fn save_grid_as_json(grid: &TileGrid, city_name: &str) -> Result<String, String> {
    match serde_json::to_string_pretty(grid) {
        Ok(json) => {
            let filename = format!("{}_grid.json", city_name.replace(" ", "_").to_lowercase());
            match std::fs::write(&filename, json) {
                Ok(()) => {
                    info!("💾 Grid saved to: {}", filename);
                    Ok(filename)
                }
                Err(e) => {
                    warn!("⚠️  Failed to save grid: {}", e);
                    Err(e.to_string())
                }
            }
        }
        Err(e) => {
            warn!("⚠️  Failed to serialize grid: {}", e);
            Err(e.to_string())
        }
    }
}

/// Generate PNG image from grid
pub fn generate_png(grid: &TileGrid, output_path: &str) -> Result<(), String> {
    let (grid_width, grid_height) = grid.dimensions();

    info!("🖼️  Generating {}x{} PNG image", grid_width, grid_height);

    // Create image buffer - one pixel per grid cell
    let mut img: RgbImage = ImageBuffer::new(grid_width as u32, grid_height as u32);

    // Draw grid tiles
    for (x, y, tile) in grid.iter_tiles() {
        let color = tile.tile_type.default_color();
        let rgb = Rgb([color.0, color.1, color.2]);
        img.put_pixel(x as u32, y as u32, rgb);
    }

    // Save image
    img.save(output_path)
        .map_err(|e| format!("Failed to save PNG: {}", e))?;

    info!("💾 PNG saved to: {}", output_path);
    Ok(())
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

    // Test connection only
    if args.test {
        let config = CityLoadConfig::from(&args);
        let provider: Box<dyn OsmDataProvider> = match config.provider.as_str() {
            "overpass" => Box::new(ProviderFactory::overpass()),
            "mock" => Box::new(ProviderFactory::mock()),
            _ => return Err("Invalid provider".to_string()),
        };

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

    // Skip grid generation (only fetch OSM data)
    if args.skip_grid {
        info!("⏭️  Skipping grid generation");
        return Ok(());
    }

    // Load city data using the exported function
    let config = CityLoadConfig::from(&args);
    let grid = load_city_data(config).await?;

    // Show statistics
    show_grid_stats(&grid);

    // Generate PNG if requested
    #[cfg(feature = "cli")]
    if let Some(output_path) = &args.output {
        if let Err(e) = generate_png(&grid, output_path) {
            error!("❌ Failed to generate PNG: {}", e);
        }
    }

    #[cfg(not(feature = "cli"))]
    if args.output.is_some() {
        warn!("⚠️  PNG output requires 'cli' feature to be enabled");
    }

    // Detailed grid statistics if requested
    if args.grid_stats {
        show_detailed_grid_analysis(&grid);
    }

    // Save grid to file if verbose
    if args.verbose {
        let _ = save_grid_as_json(&grid, &args.city);
    }

    info!("🎉 City loader completed successfully!");
    Ok(())
}
