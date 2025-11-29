//! HTTP client abstraction for external API communication.
//!
//! This module provides a trait-based abstraction over HTTP clients, enabling
//! dependency injection and easy mocking in tests.

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

/// Trait for HTTP communication with external APIs.
///
/// This abstraction allows injecting mock HTTP clients for testing without
/// making real network requests.
///
/// # Example
///
/// ```ignore
/// use abiogenesis::http_client::{HttpClient, ReqwestHttpClient};
///
/// let client = ReqwestHttpClient::new();
/// let response = client.post_json(
///     "https://api.example.com/endpoint",
///     &[("Content-Type", "application/json")],
///     &serde_json::json!({"key": "value"}),
/// ).await?;
/// ```
#[async_trait]
pub trait HttpClient: Send + Sync {
    /// Sends a POST request with JSON body and returns the response text.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to send the request to
    /// * `headers` - Key-value pairs of headers to include
    /// * `body` - The JSON body to send
    ///
    /// # Returns
    ///
    /// The response body as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be read.
    async fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<String>;
}

/// HTTP client implementation using reqwest.
///
/// This is the default production implementation that makes real HTTP requests.
pub struct ReqwestHttpClient {
    client: Client,
}

impl ReqwestHttpClient {
    /// Creates a new HTTP client with default configuration.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for ReqwestHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn post_json(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        body: &serde_json::Value,
    ) -> Result<String> {
        let mut request = self.client.post(url);

        for (key, value) in headers {
            request = request.header(*key, *value);
        }

        let response = request.json(body).send().await?;
        Ok(response.text().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mock HTTP client for testing.
    ///
    /// Returns a predetermined response without making network requests.
    pub struct MockHttpClient {
        response: Mutex<String>,
    }

    impl MockHttpClient {
        /// Creates a mock client that always returns the given response.
        pub fn new(response: &str) -> Self {
            Self {
                response: Mutex::new(response.to_string()),
            }
        }
    }

    #[async_trait]
    impl HttpClient for MockHttpClient {
        async fn post_json(
            &self,
            _url: &str,
            _headers: &[(&str, &str)],
            _body: &serde_json::Value,
        ) -> Result<String> {
            Ok(self.response.lock().unwrap().clone())
        }
    }

    #[test]
    fn test_mock_http_client_returns_response() {
        let client = MockHttpClient::new("test response");
        let response = client.response.lock().unwrap().clone();
        assert_eq!(response, "test response");
    }
}