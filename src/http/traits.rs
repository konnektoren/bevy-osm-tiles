use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

/// Result type for HTTP operations
pub type HttpResult<T> = Result<T, HttpError>;

/// HTTP client errors
#[derive(Debug, thiserror::Error, Clone)]
pub enum HttpError {
    #[error("Request failed: {message}")]
    RequestFailed { message: String },

    #[error("HTTP error: {status}")]
    HttpStatus { status: u16 },

    #[error("Timeout after {seconds} seconds")]
    Timeout { seconds: u64 },

    #[error("Network error: {message}")]
    Network { message: String },
}

/// HTTP response
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
    pub headers: HashMap<String, String>,
}

/// Trait for HTTP clients that can be used in different environments
#[async_trait]
pub trait HttpClient: Send + Sync {
    /// Make a GET request
    async fn get(&self, url: &str) -> HttpResult<HttpResponse>;

    /// Make a POST request with form data
    async fn post_form(&self, url: &str, form_data: &[(&str, &str)]) -> HttpResult<HttpResponse>;

    /// Make a POST request with JSON body
    async fn post_json(&self, url: &str, json: &str) -> HttpResult<HttpResponse>;

    /// Test if the client can make requests (connectivity check)
    async fn test_connectivity(&self, url: &str) -> HttpResult<()>;
}

/// Configuration for HTTP clients
#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub timeout: Duration,
    pub user_agent: String,
    pub default_headers: HashMap<String, String>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            user_agent: format!("bevy-osm-tiles/{}", env!("CARGO_PKG_VERSION")),
            default_headers: HashMap::new(),
        }
    }
}

impl HttpConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.default_headers.insert(key.into(), value.into());
        self
    }
}
