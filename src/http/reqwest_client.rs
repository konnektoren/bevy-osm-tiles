use super::{HttpClient, HttpConfig, HttpError, HttpResponse, HttpResult};
use async_trait::async_trait;
use std::collections::HashMap;

/// Standard reqwest-based HTTP client for general use
pub struct ReqwestClient {
    client: reqwest::Client,
}

impl ReqwestClient {
    /// Create a new reqwest client with default configuration
    pub fn new() -> HttpResult<Self> {
        Self::with_config(HttpConfig::default())
    }

    /// Create a new reqwest client with custom configuration
    pub fn with_config(config: HttpConfig) -> HttpResult<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        let timeout = std::time::Duration::from_secs(config.timeout_seconds);

        #[cfg(target_arch = "wasm32")]
        let timeout = 30;

        let mut builder = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(&config.user_agent);

        // Add default headers
        let mut headers = reqwest::header::HeaderMap::new();
        for (key, value) in &config.default_headers {
            let header_name =
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    HttpError::RequestFailed {
                        message: format!("Invalid header name '{}': {}", key, e),
                    }
                })?;
            let header_value = reqwest::header::HeaderValue::from_str(value).map_err(|e| {
                HttpError::RequestFailed {
                    message: format!("Invalid header value '{}': {}", value, e),
                }
            })?;
            headers.insert(header_name, header_value);
        }

        if !headers.is_empty() {
            builder = builder.default_headers(headers);
        }

        let client = builder.build().map_err(|e| HttpError::RequestFailed {
            message: format!("Failed to create HTTP client: {}", e),
        })?;

        Ok(Self { client })
    }

    /// Convert reqwest error to our error type
    fn convert_error(err: reqwest::Error) -> HttpError {
        if err.is_timeout() {
            HttpError::Timeout { seconds: 60 } // Default fallback
        } else if err.is_connect() {
            HttpError::Network {
                message: format!("Connection failed: {}", err),
            }
        } else if let Some(status) = err.status() {
            HttpError::HttpStatus {
                status: status.as_u16(),
            }
        } else {
            HttpError::RequestFailed {
                message: err.to_string(),
            }
        }
    }

    /// Convert reqwest response to our response type
    async fn convert_response(response: reqwest::Response) -> HttpResult<HttpResponse> {
        let status = response.status().as_u16();

        // Extract headers
        let mut headers = HashMap::new();
        for (name, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(name.to_string(), value_str.to_string());
            }
        }

        // Get body
        let body = response.text().await.map_err(Self::convert_error)?;

        Ok(HttpResponse {
            status,
            body,
            headers,
        })
    }

    /// Get access to the underlying reqwest client
    pub fn reqwest_client(&self) -> &reqwest::Client {
        &self.client
    }
}

#[async_trait]
impl HttpClient for ReqwestClient {
    async fn get(&self, url: &str) -> HttpResult<HttpResponse> {
        tracing::debug!("GET {}", url);

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(Self::convert_error)?;

        Self::convert_response(response).await
    }

    async fn post_form(&self, url: &str, form_data: &[(&str, &str)]) -> HttpResult<HttpResponse> {
        tracing::debug!("POST {} (form data with {} fields)", url, form_data.len());

        let response = self
            .client
            .post(url)
            .form(form_data)
            .send()
            .await
            .map_err(Self::convert_error)?;

        Self::convert_response(response).await
    }

    async fn post_json(&self, url: &str, json: &str) -> HttpResult<HttpResponse> {
        tracing::debug!("POST {} (JSON, {} bytes)", url, json.len());

        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .body(json.to_string())
            .send()
            .await
            .map_err(Self::convert_error)?;

        Self::convert_response(response).await
    }

    async fn test_connectivity(&self, url: &str) -> HttpResult<()> {
        tracing::debug!("Testing connectivity to {}", url);

        let response = self
            .client
            .head(url)
            .send()
            .await
            .map_err(Self::convert_error)?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(HttpError::HttpStatus {
                status: response.status().as_u16(),
            })
        }
    }
}

impl Default for ReqwestClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default reqwest client")
    }
}
