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

#[cfg(feature = "cli")]
fn generate_png(grid: &TileGrid, output_path: &str) -> Result<(), String> {
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
    info!("📍 Target city: {}", args.city);
    info!("🎯 Feature preset: {}", args.features);
    info!("🔌 Provider: {}", args.provider);
    info!("🔢 Grid resolution: {} cells/degree", args.grid_resolution);

    if let Some(ref output) = args.output {
        info!("🖼️  PNG output: {}", output);
    }

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
        .grid_resolution(args.grid_resolution)
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
    let bbox = match provider.resolve_region(&config.region).await {
        Ok(bbox) => {
            info!("📍 Resolved to bounding box: {:?}", bbox);
            info!(
                "📏 Area: {:.3}° x {:.3}° (lat x lon)",
                bbox.height(),
                bbox.width()
            );
            info!("🎯 Center: {:.3}, {:.3}", bbox.center().0, bbox.center().1);
            info!("📐 Approximate area: {:.2} km²", bbox.area_km2());
            bbox
        }
        Err(e) => {
            error!("❌ Failed to resolve region: {}", e);
            return Err(e.to_string());
        }
    };

    // Fetch OSM data
    info!("⬇️  Fetching OSM data...");
    let osm_data = match provider.fetch_data(&config).await {
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

            osm_data
        }
        Err(e) => {
            error!("❌ Failed to fetch OSM data: {}", e);
            return Err(e.to_string());
        }
    };

    // Generate grid if not skipped
    if !args.skip_grid {
        info!("🔲 Generating tile grid...");

        // Create grid generator
        let generator = DefaultGridGenerator::new();
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

        // Generate the grid
        match generator.generate_grid(&osm_data, &config).await {
            Ok(grid) => {
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

                // Show tile type distribution
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

                    // Show coverage in different areas
                    let center = bbox.center();
                    if let Some((center_x, center_y)) = grid.geo_to_grid(center.0, center.1) {
                        info!(
                            "  - Center tile ({}, {}) type: {:?}",
                            center_x,
                            center_y,
                            grid.get_tile(center_x, center_y).map(|t| &t.tile_type)
                        );
                    }

                    // Show a small sample area around center
                    if let Some((center_x, center_y)) = grid.geo_to_grid(center.0, center.1) {
                        let sample_size = 5;
                        let start_x = center_x.saturating_sub(sample_size / 2);
                        let start_y = center_y.saturating_sub(sample_size / 2);

                        if let Some(area) =
                            grid.get_area(start_x, start_y, sample_size, sample_size)
                        {
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
                }

                // Save grid to file if verbose
                if args.verbose {
                    match serde_json::to_string_pretty(&grid) {
                        Ok(json) => {
                            let filename =
                                format!("{}_grid.json", args.city.replace(" ", "_").to_lowercase());
                            match std::fs::write(&filename, json) {
                                Ok(()) => info!("💾 Grid saved to: {}", filename),
                                Err(e) => warn!("⚠️  Failed to save grid: {}", e),
                            }
                        }
                        Err(e) => warn!("⚠️  Failed to serialize grid: {}", e),
                    }
                }
            }
            Err(e) => {
                error!("❌ Failed to generate grid: {}", e);
                return Err(e.to_string());
            }
        }
    } else {
        info!("⏭️  Skipping grid generation");
    }

    info!("🎉 City loader completed successfully!");
    Ok(())
}
