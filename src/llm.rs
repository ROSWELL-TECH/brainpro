//! LLM client with retry logic, jittered backoff, and connection pooling.
//!
//! Uses reqwest::blocking for synchronous HTTP calls with built-in
//! connection pooling and timeout handling.

use anyhow::{anyhow, Result};
use rand::Rng;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::thread;
use std::time::Duration;

// Retry configuration for rate limiting and transient errors
const MAX_RETRIES: u32 = 5;
const INITIAL_BACKOFF_MS: u64 = 1000; // 1 second
const MAX_BACKOFF_MS: u64 = 60000; // 60 seconds
const JITTER_FACTOR: f64 = 0.3; // ±30% jitter

/// Check if an HTTP status code is retryable (429 rate limit or 5xx server error)
fn is_retryable_status(code: u16) -> bool {
    code == 429 || (500..600).contains(&code)
}

/// Calculate jittered backoff delay
fn jittered_backoff(base_ms: u64) -> u64 {
    let mut rng = rand::thread_rng();
    let jitter = rng.gen_range(0.0..JITTER_FACTOR) * base_ms as f64;
    let jittered = base_ms as f64 + jitter;
    (jittered as u64).min(MAX_BACKOFF_MS)
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
}

/// Token usage statistics from the API response
#[derive(Debug, Deserialize, Default, Clone)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: Message,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Result of an LLM call including timing and retry info
#[derive(Debug)]
pub struct LlmCallResult {
    pub response: ChatResponse,
    pub latency_ms: u64,
    pub retries: u32,
}

/// Trait for LLM clients to allow mocking and abstraction
pub trait LlmClient {
    /// Synchronous chat call (may internally use async)
    fn chat(&self, request: &ChatRequest) -> Result<ChatResponse>;

    /// Chat call that returns additional metadata (latency, retries)
    fn chat_with_metadata(&self, request: &ChatRequest) -> Result<LlmCallResult> {
        let start = std::time::Instant::now();
        let response = self.chat(request)?;
        Ok(LlmCallResult {
            response,
            latency_ms: start.elapsed().as_millis() as u64,
            retries: 0,
        })
    }
}

pub struct Client {
    base_url: String,
    /// API key wrapped in SecretString for secure memory handling.
    /// Will be zeroized on drop and won't leak via Debug/Display.
    api_key: SecretString,
    http_client: reqwest::blocking::Client,
}

impl Client {
    /// Create a new LLM client.
    /// The API key is stored as a SecretString for secure memory handling.
    pub fn new(base_url: &str, api_key: SecretString) -> Self {
        let http_client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .pool_max_idle_per_host(10)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            http_client,
        }
    }

    /// Internal sync implementation with retry logic
    fn chat_sync(&self, request: &ChatRequest) -> Result<LlmCallResult> {
        let url = format!("{}/chat/completions", self.base_url);
        let start = std::time::Instant::now();

        let mut attempt = 0;
        let mut backoff_ms = INITIAL_BACKOFF_MS;
        let mut total_retries = 0;

        loop {
            attempt += 1;

            let resp = self
                .http_client
                .post(&url)
                .header(
                    "Authorization",
                    format!("Bearer {}", self.api_key.expose_secret()),
                )
                .header("Content-Type", "application/json")
                .json(request)
                .send();

            match resp {
                Ok(response) => {
                    let status = response.status();

                    if status.is_success() {
                        let body: ChatResponse = response.json()?;
                        return Ok(LlmCallResult {
                            response: body,
                            latency_ms: start.elapsed().as_millis() as u64,
                            retries: total_retries,
                        });
                    }

                    let code = status.as_u16();
                    if is_retryable_status(code) {
                        if attempt >= MAX_RETRIES {
                            let body = response.text().unwrap_or_default();
                            return Err(anyhow!(
                                "API error {} after {} retries: {}",
                                code,
                                MAX_RETRIES,
                                body
                            ));
                        }

                        // Check for Retry-After header (common in 429 responses)
                        let retry_after = response
                            .headers()
                            .get("Retry-After")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok())
                            .map(|s| s * 1000); // Convert seconds to ms

                        let wait_ms = retry_after.unwrap_or_else(|| jittered_backoff(backoff_ms));

                        eprintln!(
                            "[llm] {} error, retrying in {}ms (attempt {}/{})",
                            code, wait_ms, attempt, MAX_RETRIES
                        );

                        thread::sleep(Duration::from_millis(wait_ms));
                        backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
                        total_retries += 1;
                    } else {
                        // Non-retryable HTTP error (4xx except 429)
                        let body = response.text().unwrap_or_default();
                        return Err(anyhow!("API error {}: {}", code, body));
                    }
                }
                Err(e) => {
                    // Connection/network error - retryable
                    if attempt >= MAX_RETRIES {
                        return Err(anyhow!(
                            "Connection error after {} retries: {}",
                            MAX_RETRIES,
                            e
                        ));
                    }

                    let wait_ms = jittered_backoff(backoff_ms);
                    eprintln!(
                        "[llm] Connection error, retrying in {}ms (attempt {}/{}): {}",
                        wait_ms, attempt, MAX_RETRIES, e
                    );

                    thread::sleep(Duration::from_millis(wait_ms));
                    backoff_ms = (backoff_ms * 2).min(MAX_BACKOFF_MS);
                    total_retries += 1;
                }
            }
        }
    }
}

impl LlmClient for Client {
    fn chat(&self, request: &ChatRequest) -> Result<ChatResponse> {
        let result = self.chat_sync(request)?;
        Ok(result.response)
    }

    fn chat_with_metadata(&self, request: &ChatRequest) -> Result<LlmCallResult> {
        self.chat_sync(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jittered_backoff() {
        // Run multiple times to verify jitter is applied
        let base = 1000u64;
        for _ in 0..10 {
            let result = jittered_backoff(base);
            assert!(result >= base);
            assert!(result <= base + (base as f64 * JITTER_FACTOR) as u64);
        }
    }

    #[test]
    fn test_is_retryable_status() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(502));
        assert!(is_retryable_status(503));
        assert!(!is_retryable_status(400));
        assert!(!is_retryable_status(401));
        assert!(!is_retryable_status(404));
    }
}
