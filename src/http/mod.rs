mod traits;

#[cfg(feature = "reqwest-client")]
mod reqwest_client;

#[cfg(feature = "ehttp-client")]
mod ehttp_client;

pub use traits::*;

#[cfg(feature = "reqwest-client")]
pub use reqwest_client::*;

#[cfg(feature = "ehttp-client")]
pub use ehttp_client::*;

use std::sync::Arc;

/// Create a default HTTP client based on enabled features
pub fn create_default_client() -> Result<Arc<dyn HttpClient>, String> {
    #[cfg(feature = "reqwest-client")]
    {
        ReqwestClient::new()
            .map(|client| Arc::new(client) as Arc<dyn HttpClient>)
            .map_err(|e| format!("Failed to create reqwest client: {}", e))
    }

    #[cfg(all(feature = "ehttp-client", not(feature = "reqwest-client")))]
    {
        Ok(Arc::new(EhttpClient::new()) as Arc<dyn HttpClient>)
    }

    #[cfg(not(any(feature = "reqwest-client", feature = "ehttp-client")))]
    {
        Err(
            "No HTTP client feature enabled. Enable either 'reqwest-client' or 'ehttp-client'"
                .to_string(),
        )
    }
}

/// Create an HTTP client with custom configuration
pub fn create_client_with_config(config: HttpConfig) -> Result<Arc<dyn HttpClient>, String> {
    #[cfg(feature = "reqwest-client")]
    {
        ReqwestClient::with_config(config)
            .map(|client| Arc::new(client) as Arc<dyn HttpClient>)
            .map_err(|e| format!("Failed to create reqwest client: {}", e))
    }

    #[cfg(all(feature = "ehttp-client", not(feature = "reqwest-client")))]
    {
        Ok(Arc::new(EhttpClient::with_config(config)) as Arc<dyn HttpClient>)
    }

    #[cfg(not(any(feature = "reqwest-client", feature = "ehttp-client")))]
    {
        Err(
            "No HTTP client feature enabled. Enable either 'reqwest-client' or 'ehttp-client'"
                .to_string(),
        )
    }
}
