use thiserror::Error;

/// Errors that can occur during OSM data processing
#[derive(Error, Debug)]
pub enum OsmTilesError {
    /// Network-related errors during data download
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),

    /// Errors parsing OSM data
    #[error("Parse error: {0}")]
    Parse(String),

    /// Configuration validation errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Geographic coordinate or region resolution errors
    #[error("Geographic error: {0}")]
    Geographic(String),

    /// Grid generation errors
    #[error("Grid generation error: {0}")]
    GridGeneration(String),
}

/// Network-specific errors
#[derive(Error, Debug)]
pub enum NetworkError {
    /// HTTP request failed
    #[error("HTTP request failed: {status}")]
    HttpError { status: u16 },

    /// Request timeout
    #[error("Request timed out after {seconds} seconds")]
    Timeout { seconds: u64 },

    /// Connection error
    #[error("Connection error: {message}")]
    Connection { message: String },

    /// Invalid URL
    #[error("Invalid URL: {url}")]
    InvalidUrl { url: String },
}

pub type Result<T> = std::result::Result<T, OsmTilesError>;
