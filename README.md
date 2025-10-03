# bevy-osm-tiles

A Rust library for downloading OpenStreetMap data and converting it to grid-based tile maps, with optional Bevy game engine integration.

## Overview

`bevy-osm-tiles` provides a WASM-compatible library for fetching OpenStreetMap data and converting it into grid-based representations suitable for games, simulations, and visualizations. The library follows a trait-based architecture that separates core functionality from rendering, making it usable in various contexts beyond Bevy.

## Features

- **Pure Rust Core**: Core tile generation logic with no game engine dependencies
- **WASM Compatible**: Runs in browsers and native environments
- **Trait-Based Architecture**: Pluggable downloaders, renderers, and data processors
- **Optional Bevy Integration**: ECS components, systems, and plugin for seamless Bevy integration
- **Async-First**: Non-blocking data fetching and processing
- **Configurable Grid Generation**: Customizable tile size, zoom levels, and data filtering

## Example: City Map Loader

This example demonstrates a simple Bevy application that allows users to input a city name, downloads the corresponding OpenStreetMap data, converts it to a grid, and renders it as a 2D tile map.

### Features Demonstrated

1. **Text Input System**: User can type a city name in a text field
2. **Async Data Loading**: On Enter key press, triggers async download of OSM data for the specified city
3. **Grid Conversion**: Raw OSM data is processed into a structured grid representation
4. **Visual Rendering**: Grid tiles are rendered as colored sprites representing different terrain types (roads, buildings, water, parks, etc.)
5. **Loading States**: Visual feedback during data download and processing

### Expected Behavior

```
[Text Input: "Berlin"] [Press Enter]
         ↓
[Loading indicator...]
         ↓
[Download OSM data for Berlin]
         ↓
[Convert to grid: roads=gray, buildings=brown, water=blue, parks=green]
         ↓
[Render 2D tile map in Bevy window]
```

### Usage

```bash
cargo run --example city_loader --features bevy
```

Type a city name in the input field and press Enter to load and visualize the city's map data as a grid-based tile representation.

## Architecture

### Core Library (`bevy-osm-tiles`)
- `TileGrid`: Core data structure representing the map grid
- `OsmDownloader`: Trait for fetching OpenStreetMap data
- `GridGenerator`: Converts raw OSM data to structured grids
- `TileType`: Enumeration of different terrain/feature types

### Bevy Integration (`bevy-osm-tiles` with `bevy` feature)
- `OsmTilesPlugin`: Bevy plugin providing ECS integration
- `MapLoader`: Component for managing async map loading state
- `TileRenderer`: System for converting grids to visual sprites
- `InputHandler`: System for processing user input and triggering downloads

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bevy-osm-tiles = "0.1"

# For Bevy integration
bevy-osm-tiles = { version = "0.1", features = ["bevy"] }
```

## License

[MIT](LICENSE)
