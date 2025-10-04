# justfile for bevy-osm-tiles

# Default recipe - show available commands
default:
    @just --list

# Variables
default_city := "Friedrichshain, Berlin"
default_resolution := "2500"
default_features := "urban"

# CLI example - load OSM data and generate PNG
cli city=default_city features=default_features resolution=default_resolution output="output.png":
    cargo run --example cli_city_loader --features cli,reqwest-client -- --city "{{city}}" --features {{features}} --grid-resolution {{resolution}} --output {{output}}

# 3D example - load OSM data and display in 3D
viz3d city=default_city resolution=default_resolution provider="overpass":
    cargo run --example city_loader_3d --features=bevy,cli -- --city "{{city}}" --grid-resolution {{resolution}} --provider {{provider}}

# 3D example with mock data for testing
viz3d-mock city=default_city resolution=default_resolution:
    cargo run --example city_loader_3d --features=bevy,cli -- --city "{{city}}" --grid-resolution {{resolution}} --mock

# Specific examples from your commands
friedrichshain:
    cargo run --example cli_city_loader --features cli,reqwest-client -- --city "Berlin Friedrichshain" --features urban --grid-resolution 10000 --output friedrichshain.png

bad-vilbel:
    cargo run --example city_loader_3d --features=bevy,cli -- --city "Bad Vilbel" --grid-resolution 2500

# High-resolution examples
hires-berlin:
    cargo run --example cli_city_loader --features cli,reqwest-client -- --city "Berlin" --features comprehensive --grid-resolution 5000 --output berlin_hires.png

hires-viz3d city="Munich":
    cargo run --example city_loader_3d --features=bevy,cli -- --city "{{city}}" --grid-resolution 5000

# Different feature sets
transportation city=default_city:
    cargo run --example cli_city_loader --features cli,reqwest-client -- --city "{{city}}" --features transportation --grid-resolution 2000 --output {{city}}_transport.png

natural city=default_city:
    cargo run --example cli_city_loader --features cli,reqwest-client -- --city "{{city}}" --features natural --grid-resolution 1500 --output {{city}}_natural.png

comprehensive city=default_city:
    cargo run --example cli_city_loader --features cli,reqwest-client -- --city "{{city}}" --features comprehensive --grid-resolution 3000 --output {{city}}_comprehensive.png

# Test with different HTTP clients
test-ehttp city=default_city:
    cargo run --example city_loader_3d --no-default-features --features=bevy,cli,ehttp-client -- --city "{{city}}" --grid-resolution 1000

test-reqwest city=default_city:
    cargo run --example city_loader_3d --no-default-features --features=bevy,cli,reqwest-client -- --city "{{city}}" --grid-resolution 1000

# Development commands
build:
    cargo build

build-release:
    cargo build --release

# Test different feature combinations
test:
    cargo test

test-features:
    cargo test --features reqwest-client
    cargo test --features ehttp-client
    cargo test --features bevy,cli

# Check code
check:
    cargo check

clippy:
    cargo clippy -- -D warnings

fmt:
    cargo fmt

# Clean
clean:
    cargo clean
    rm -f *.png

# WASM build (if you want to support it later)
build-wasm:
    cargo build --target wasm32-unknown-unknown --no-default-features --features=ehttp-client

# Documentation
doc:
    cargo doc --open

# Batch processing - generate multiple cities
batch-cities:
    @echo "Generating maps for multiple cities..."
    @just cli "Berlin" "urban" "2000" "berlin_urban.png"
    @just cli "Munich" "urban" "2000" "munich_urban.png"
    @just cli "Hamburg" "urban" "2000" "hamburg_urban.png"
    @just cli "Frankfurt" "urban" "2000" "frankfurt_urban.png"

# Quick test with mock data
quick-test:
    cargo run --example city_loader_3d --features=bevy,cli -- --city "Test" --mock --grid-resolution 500 --verbose

# Debug mode with verbose output
debug city=default_city:
    cargo run --example city_loader_3d --features=bevy,cli -- --city "{{city}}" --grid-resolution 1000 --verbose

# Release builds for examples
release-cli city=default_city:
    cargo run --release --example cli_city_loader --features cli,reqwest-client -- --city "{{city}}" --features urban --grid-resolution 2000 --output {{city}}_release.png

release-viz3d city=default_city:
    cargo run --release --example city_loader_3d --features=bevy,cli -- --city "{{city}}" --grid-resolution 2000

# Help for common cities and parameters
help:
    @echo "Common usage patterns:"
    @echo ""
    @echo "Basic CLI generation:"
    @echo "  just cli 'City Name' urban 1000 output.png"
    @echo ""
    @echo "3D visualization:"
    @echo "  just viz3d 'City Name' 2000"
    @echo ""
    @echo "Feature sets: urban, transportation, natural, comprehensive"
    @echo "Grid resolution: 100-10000 (higher = more detail, slower)"
    @echo ""
    @echo "Predefined examples:"
    @echo "  just friedrichshain    # High-res Berlin district"
    @echo "  just bad-vilbel        # 3D Bad Vilbel"
    @echo "  just quick-test        # Fast test with mock data"
