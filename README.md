# bevy-osm-tiles

A Rust library for downloading OpenStreetMap data and converting it to grid-based tile maps, with Bevy game engine integration for interactive 3D visualization.

## 🌍 Overview

`bevy-osm-tiles` provides a WASM-compatible library for fetching OpenStreetMap data and converting it into grid-based representations suitable for games, simulations, and visualizations. The library features a trait-based architecture that separates data fetching, processing, and rendering, making it flexible and extensible.

## ✨ Features

- **🦀 Pure Rust Core**: Core tile generation logic with optional game engine integration
- **🌐 WASM Compatible**: Runs in browsers and native environments
- **🔌 Trait-Based Architecture**: Pluggable data providers, generators, and processors
- **🕊️ Bevy Integration**: ECS components, systems, and plugin for seamless Bevy integration
- **⚡ Async-First**: Non-blocking data fetching and processing with progress tracking
- **⚙️ Configurable**: Customizable grid resolution, feature sets, and data filtering
- **🎮 Interactive**: Real-time city loading with dynamic feature selection
- **📡 Multiple Data Sources**: Overpass API integration with mock provider for testing

## 🎮 Interactive Examples

### Interactive City Loader (3D)

A complete 3D interactive application that allows users to explore OpenStreetMap data in real-time.

**🎯 Features:**
- **Text Input**: Type any city name to load OSM data
- **Feature Cycling**: Click to cycle through different feature sets (Urban, Transportation, Natural, Comprehensive, Gaming)
- **Resolution Control**: Adjust grid resolution from 250 to 8000 for different detail levels
- **3D Visualization**: Navigate through cities with WASD + mouse controls
- **Real-time Loading**: Visual progress indicators and status updates
- **Performance Warnings**: Color-coded resolution settings with performance guidance

**🕹️ Controls:**
- Type city names directly in the interface
- `Enter` or `Load` button to fetch data
- `🎯 Feature` button: Cycles through feature presets
- `📏 Resolution` button: Doubles resolution (250→500→1000→2000→4000→8000→250)
- `←→↑↓`: Camera movement
- `Alt + Mouse`: Look around
- `Page Up/Down`: Vertical movement
- `Ctrl`: Speed boost

**🚀 Try it online:** [bevy-osm-tiles Demo](https://konnektoren.github.io/bevy-osm-tiles/)

### Usage

```bash
# Run locally
cargo run --example interactive_city_loader

# Build for web
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo build --release --target wasm32-unknown-unknown --example interactive_city_loader
```

## 🏗️ Architecture

### Core Library
```
bevy-osm-tiles/
├── config/          # Configuration and feature definitions
├── provider/        # Data providers (Overpass API, Mock)
├── generator/       # Grid generation and OSM parsing
├── bevy_plugin/     # Bevy ECS integration
└── error/          # Error handling
```

**Key Components:**
- `TileGrid`: Core data structure representing map grids with metadata
- `OsmDataProvider`: Trait for fetching OpenStreetMap data from various sources
- `GridGenerator`: Converts raw OSM data to structured tile grids
- `TileType`: Rich enumeration of terrain/feature types with rendering hints
- `OsmConfig`: Flexible configuration system for regions, features, and generation parameters

### Bevy Integration
- `OsmTilesPlugin`: Complete Bevy plugin with async loading and ECS integration
- `MapLoadRequest`/`MapLoadedMessage`: Event-driven loading system
- `LoadingStage`: Progress tracking for multi-stage loading process
- Component-based architecture for managing map state and rendering

### WASM Compatibility
- Uses `reqwest` with WASM features for HTTP requests
- Compatible with browser environments
- Proper async handling for web deployment
- GitHub Pages deployment workflow included

## 🔧 Configuration

### Feature Sets
```rust
use bevy_osm_tiles::{FeatureSet, OsmFeature};

// Predefined feature sets
let urban = FeatureSet::urban();           // Roads, buildings, parks, water
let transport = FeatureSet::transportation(); // Roads, highways, railways, parking
let natural = FeatureSet::natural();       // Water, forests, parks, grassland
let comprehensive = FeatureSet::comprehensive(); // All available features

// Custom feature sets
let custom = FeatureSet::new()
    .with_feature(OsmFeature::Buildings)
    .with_feature(OsmFeature::Roads)
    .with_custom_query(OsmTagQuery::new("amenity", Some("restaurant")));
```

### Grid Configuration
```rust
use bevy_osm_tiles::{OsmConfigBuilder, Region};

let config = OsmConfigBuilder::new()
    .city("Berlin")
    .grid_resolution(500)           // Higher = more detail
    .features(FeatureSet::urban())
    .build();

// Or with bounding box
let config = OsmConfigBuilder::new()
    .bbox(52.4, 13.3, 52.6, 13.5)  // South, West, North, East
    .grid_resolution(1000)
    .comprehensive_features()
    .build();
```

## 📦 Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bevy-osm-tiles = { git = "https://github.com/konnektoren/bevy-osm-tiles" }
bevy = "0.17"
```

For WASM builds, also add:
```toml
getrandom = { version = "0.3", features = ["wasm_js"] }
```

## 🌐 Web Deployment

This project includes a complete GitHub Actions workflow for automatic deployment to GitHub Pages:

1. **Automatic builds** on push to main
2. **WASM compilation** with proper browser compatibility
3. **GitHub Pages deployment** with custom HTML interface
4. **Asset optimization** and caching

See `.github/workflows/deploy.yml` for the complete setup.

## 🎨 Tile Types & Visualization

The library supports rich tile type classification with default colors and rendering hints:

| Tile Type | Color | Height | Use Case |
|-----------|-------|--------|----------|
| Building | Brown | 2.0m | Residential/commercial structures |
| Road | Gray | 0.1m | Streets and paths |
| Water | Blue | 0.05m | Rivers, lakes, fountains |
| GreenSpace | Green | 0.2m | Parks, forests, grass |
| Railway | Brown | 0.15m | Train tracks and stations |
| Amenity | Orange | 1.0m | Shops, restaurants, services |
| Tourism | Pink | 1.5m | Hotels, attractions, monuments |

## 🤝 Contributing

Contributions are welcome! Areas for improvement:
- Additional data providers (local files, other APIs)
- Enhanced tile type classification
- Performance optimizations for large datasets
- Additional rendering backends
- More example applications

## 📄 License

MIT License - see [LICENSE](LICENSE) for details.

## 🙏 Acknowledgments

- **OpenStreetMap**: For providing free, open geographic data
- **Overpass API**: For powerful OSM data querying capabilities
- **Bevy Engine**: For the excellent game engine and ECS framework
- **Rust Community**: For the amazing ecosystem and tools

---

**🎮 [Try the Interactive Demo](https://konnektoren.github.io/bevy-osm-tiles/)**
