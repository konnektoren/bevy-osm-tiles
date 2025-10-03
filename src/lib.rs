//! A Rust library for downloading OpenStreetMap data and converting it to grid-based tile maps.
//!
//! This library provides a trait-based architecture for fetching OSM data and generating
//! grid representations suitable for games and visualizations. The core library is
//! WASM-compatible and has optional Bevy integration.

pub mod config;
pub mod error;
pub mod generator;
pub mod provider;

pub use config::*;
pub use error::*;
pub use generator::*;
pub use provider::*;
