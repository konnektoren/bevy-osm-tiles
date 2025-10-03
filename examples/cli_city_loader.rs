use clap::Parser;
use tracing::info;
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "osm-city-loader")]
#[command(about = "Load OpenStreetMap data for a city and generate grid tiles")]
struct Args {
    /// City name to load OSM data for
    #[arg(short, long)]
    city: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
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

    Ok(())
}
