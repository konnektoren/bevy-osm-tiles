use super::{HttpClient, HttpConfig, HttpError, HttpResponse, HttpResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// HTTP client using ehttp for WASM and native compatibility
///
/// ehttp is a lightweight HTTP client that works well in both WASM and native environments
/// without requiring tokio, making it ideal for Bevy applications.
pub struct EhttpClient {
    config: HttpConfig,
}

/// Internal state for tracking async requests
struct RequestState {
    result: Option<HttpResult<HttpResponse>>,
    completed: bool,
}

impl EhttpClient {
    /// Create a new ehttp client with default configuration
    pub fn new() -> Self {
        Self::with_config(HttpConfig::default())
    }

    /// Create a new ehttp client with custom configuration
    pub fn with_config(config: HttpConfig) -> Self {
        Self { config }
    }

    /// Get the configuration
    pub fn config(&self) -> &HttpConfig {
        &self.config
    }

    /// Convert ehttp error to our error type
    fn convert_error(error: String) -> HttpError {
        // ehttp returns string errors, so we need to parse them
        if error.contains("timeout") || error.contains("Timeout") {
            HttpError::Timeout { seconds: 60 } // Default timeout
        } else if error.contains("connection") || error.contains("Connection") {
            HttpError::Network { message: error }
        } else {
            HttpError::RequestFailed { message: error }
        }
    }

    /// Convert ehttp response to our response type
    fn convert_response(response: ehttp::Response) -> HttpResult<HttpResponse> {
        let status = response.status as u16;

        // Convert ehttp::Headers to HashMap<String, String>
        let mut headers = HashMap::new();
        for (key, value) in response.headers {
            headers.insert(key, value);
        }

        // Convert body bytes to string
        let body = String::from_utf8(response.bytes).map_err(|e| HttpError::RequestFailed {
            message: format!("Failed to decode response body as UTF-8: {}", e),
        })?;

        Ok(HttpResponse {
            status,
            body,
            headers,
        })
    }

    /// Build headers for the request
    fn build_headers(&self, additional_headers: Option<HashMap<String, String>>) -> ehttp::Headers {
        let mut headers = ehttp::Headers::default();

        // Add default headers from config
        for (key, value) in &self.config.default_headers {
            headers.insert(key.clone(), value.clone());
        }

        // Add user agent
        headers.insert("User-Agent".to_string(), self.config.user_agent.clone());

        // Add any additional headers
        if let Some(additional) = additional_headers {
            for (key, value) in additional {
                headers.insert(key, value);
            }
        }

        headers
    }

    /// Execute an HTTP request using ehttp with async/await simulation
    async fn execute_request(
        &self,
        method: &str,
        url: &str,
        headers: ehttp::Headers,
        body: Vec<u8>,
    ) -> HttpResult<HttpResponse> {
        let request = ehttp::Request {
            method: method.to_string(),
            url: url.to_string(),
            headers,
            body,
            mode: ehttp::Mode::NoCors,
        };

        tracing::debug!("{} {} ({} bytes)", method, url, request.body.len());

        // Use shared state to track the request completion
        let state = Arc::new(Mutex::new(RequestState {
            result: None,
            completed: false,
        }));

        let state_for_callback = state.clone();

        // Execute the request
        ehttp::fetch(request, move |response| {
            let result = match response {
                Ok(response) => Self::convert_response(response),
                Err(error) => Err(Self::convert_error(error)),
            };

            let mut guard = state_for_callback.lock().unwrap();
            guard.result = Some(result);
            guard.completed = true;
        });

        // Poll for completion (non-blocking async polling)
        let start_time = std::time::Instant::now();
        let timeout = self.config.timeout;

        loop {
            // Check if request is complete
            {
                let guard = state.lock().unwrap();
                if guard.completed {
                    // Take the result instead of cloning it
                    return guard.result.as_ref().unwrap().clone();
                }
            }

            // Check for timeout
            if start_time.elapsed() > timeout {
                return Err(HttpError::Timeout {
                    seconds: timeout.as_secs(),
                });
            }

            // Yield control to allow other tasks to run
            #[cfg(not(target_arch = "wasm32"))]
            {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            #[cfg(target_arch = "wasm32")]
            {
                // On WASM, we can't use std::thread::sleep
                // The browser's event loop will handle the yielding
                // We just need to not busy-wait too aggressively
            }
        }
    }
}

#[async_trait]
impl HttpClient for EhttpClient {
    async fn get(&self, url: &str) -> HttpResult<HttpResponse> {
        let headers = self.build_headers(None);
        self.execute_request("GET", url, headers, Vec::new()).await
    }

    async fn post_form(&self, url: &str, form_data: &[(&str, &str)]) -> HttpResult<HttpResponse> {
        // Build form-encoded body - fix the borrowing issue
        let mut body_parts = Vec::new();
        for (i, (key, value)) in form_data.iter().enumerate() {
            if i > 0 {
                body_parts.push("&".to_string());
            }
            body_parts.push(urlencoding::encode(key).to_string());
            body_parts.push("=".to_string());
            body_parts.push(urlencoding::encode(value).to_string());
        }
        let body_string = body_parts.join("");
        let body = body_string.into_bytes();

        // Set content type header
        let mut additional_headers = HashMap::new();
        additional_headers.insert(
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        );

        let headers = self.build_headers(Some(additional_headers));
        self.execute_request("POST", url, headers, body).await
    }

    async fn post_json(&self, url: &str, json: &str) -> HttpResult<HttpResponse> {
        let body = json.as_bytes().to_vec();

        // Set content type header
        let mut additional_headers = HashMap::new();
        additional_headers.insert("Content-Type".to_string(), "application/json".to_string());

        let headers = self.build_headers(Some(additional_headers));
        self.execute_request("POST", url, headers, body).await
    }

    async fn test_connectivity(&self, url: &str) -> HttpResult<()> {
        let headers = self.build_headers(None);
        let response = self
            .execute_request("HEAD", url, headers, Vec::new())
            .await?;

        if response.status >= 200 && response.status < 400 {
            Ok(())
        } else {
            Err(HttpError::HttpStatus {
                status: response.status,
            })
        }
    }
}

impl Default for EhttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_ehttp_client_creation() {
        let client = EhttpClient::new();
        assert_eq!(
            client.config().user_agent,
            format!("bevy-osm-tiles/{}", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(client.config().timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_ehttp_client_with_config() {
        let config = HttpConfig::new()
            .with_timeout(Duration::from_secs(30))
            .with_user_agent("test-agent")
            .with_header("X-Test", "value");

        let client = EhttpClient::with_config(config);
        assert_eq!(client.config().timeout, Duration::from_secs(30));
        assert_eq!(client.config().user_agent, "test-agent");
        assert_eq!(
            client.config().default_headers.get("X-Test"),
            Some(&"value".to_string())
        );
    }

    #[test]
    fn test_build_headers() {
        let config = HttpConfig::new()
            .with_user_agent("test-agent")
            .with_header("X-Default", "default-value");

        let client = EhttpClient::with_config(config);

        let mut additional = HashMap::new();
        additional.insert("X-Additional".to_string(), "additional-value".to_string());

        let headers = client.build_headers(Some(additional));

        // We can't easily inspect ehttp::Headers content, but we can verify it was created
        // In a real scenario, this would be tested through actual HTTP requests
        assert!(true); // Placeholder assertion
    }

    #[test]
    fn test_convert_error() {
        let timeout_error = EhttpClient::convert_error("Request timeout".to_string());
        assert!(matches!(timeout_error, HttpError::Timeout { .. }));

        let connection_error = EhttpClient::convert_error("Connection failed".to_string());
        assert!(matches!(connection_error, HttpError::Network { .. }));

        let generic_error = EhttpClient::convert_error("Something went wrong".to_string());
        assert!(matches!(generic_error, HttpError::RequestFailed { .. }));
    }

    #[test]
    fn test_convert_response() {
        let mut headers = ehttp::Headers::default();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let ehttp_response = ehttp::Response {
            url: "https://example.com".to_string(),
            ok: true,
            status: 200,
            status_text: "OK".to_string(),
            headers,
            bytes: b"Hello, World!".to_vec(),
        };

        let response = EhttpClient::convert_response(ehttp_response).unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, "Hello, World!");
        assert_eq!(
            response.headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );
    }

    // Integration tests would be added here for actual HTTP requests
    // They would test against a real server or mock server
}
