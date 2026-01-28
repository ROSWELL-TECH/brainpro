//! Provider health tracking for LLM backends.
//!
//! Tracks health metrics per provider including:
//! - Success/failure rates
//! - Average latency
//! - Cooldown periods after failures
//! - Integration with circuit breaker state

#![allow(dead_code)]

use crate::circuit_breaker::{CircuitBreakerRegistry, CircuitState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Health state of a provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    /// Provider is responding normally
    #[default]
    Healthy,
    /// Provider is experiencing intermittent issues
    Degraded,
    /// Provider is not responding or failing consistently
    Unhealthy,
}

/// Health metrics for a single provider
#[derive(Debug, Clone, Serialize)]
pub struct ProviderHealth {
    pub backend: String,
    pub state: HealthState,
    pub consecutive_failures: u32,
    pub avg_latency_ms: f64,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub last_success: Option<u64>,   // Unix timestamp
    pub last_failure: Option<u64>,   // Unix timestamp
    pub cooldown_until: Option<u64>, // Unix timestamp
}

/// Configuration for health tracking
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthConfig {
    /// Latency threshold for degraded state (ms)
    #[serde(default = "default_degraded_latency_ms")]
    pub degraded_latency_ms: u64,
    /// Failure count threshold for degraded state
    #[serde(default = "default_degraded_failure_count")]
    pub degraded_failure_count: u32,
    /// Failure count threshold for unhealthy state
    #[serde(default = "default_unhealthy_failure_count")]
    pub unhealthy_failure_count: u32,
    /// Cooldown duration after unhealthy (seconds)
    #[serde(default = "default_cooldown_secs")]
    pub cooldown_secs: u64,
    /// Window size for latency averaging
    #[serde(default = "default_latency_window")]
    pub latency_window: usize,
}

fn default_degraded_latency_ms() -> u64 {
    5000 // 5 seconds
}
fn default_degraded_failure_count() -> u32 {
    2
}
fn default_unhealthy_failure_count() -> u32 {
    5
}
fn default_cooldown_secs() -> u64 {
    60
}
fn default_latency_window() -> usize {
    10
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            degraded_latency_ms: default_degraded_latency_ms(),
            degraded_failure_count: default_degraded_failure_count(),
            unhealthy_failure_count: default_unhealthy_failure_count(),
            cooldown_secs: default_cooldown_secs(),
            latency_window: default_latency_window(),
        }
    }
}

/// Internal mutable state for a provider
#[derive(Debug, Default)]
struct ProviderState {
    consecutive_failures: u32,
    consecutive_successes: u32,
    recent_latencies: Vec<u64>, // Circular buffer
    latency_index: usize,
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    last_success: Option<Instant>,
    last_failure: Option<Instant>,
    cooldown_until: Option<Instant>,
}

impl ProviderState {
    fn add_latency(&mut self, latency_ms: u64, window_size: usize) {
        if self.recent_latencies.len() < window_size {
            self.recent_latencies.push(latency_ms);
        } else {
            self.recent_latencies[self.latency_index % window_size] = latency_ms;
            self.latency_index += 1;
        }
    }

    fn avg_latency(&self) -> f64 {
        if self.recent_latencies.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.recent_latencies.iter().sum();
        sum as f64 / self.recent_latencies.len() as f64
    }

    fn compute_state(&self, config: &HealthConfig) -> HealthState {
        // Check if in cooldown
        if let Some(cooldown) = self.cooldown_until {
            if Instant::now() < cooldown {
                return HealthState::Unhealthy;
            }
        }

        // Check consecutive failures
        if self.consecutive_failures >= config.unhealthy_failure_count {
            return HealthState::Unhealthy;
        }

        if self.consecutive_failures >= config.degraded_failure_count {
            return HealthState::Degraded;
        }

        // Check latency
        let avg_latency = self.avg_latency();
        if avg_latency > config.degraded_latency_ms as f64 {
            return HealthState::Degraded;
        }

        HealthState::Healthy
    }

    fn is_in_cooldown(&self) -> bool {
        if let Some(cooldown) = self.cooldown_until {
            Instant::now() < cooldown
        } else {
            false
        }
    }
}

/// Registry of provider health tracking
#[derive(Debug)]
pub struct ProviderHealthRegistry {
    config: HealthConfig,
    providers: Arc<RwLock<HashMap<String, ProviderState>>>,
    circuit_breakers: Option<Arc<CircuitBreakerRegistry>>,
}

impl ProviderHealthRegistry {
    /// Create a new health registry
    pub fn new(config: HealthConfig) -> Self {
        Self {
            config,
            providers: Arc::new(RwLock::new(HashMap::new())),
            circuit_breakers: None,
        }
    }

    /// Set the circuit breaker registry for integration
    pub fn with_circuit_breakers(mut self, cb: Arc<CircuitBreakerRegistry>) -> Self {
        self.circuit_breakers = Some(cb);
        self
    }

    /// Record a successful request
    pub fn record_success(&self, backend: &str, latency_ms: u64) {
        let mut providers = self.providers.write().unwrap();
        let state = providers.entry(backend.to_string()).or_default();

        state.total_requests += 1;
        state.successful_requests += 1;
        state.consecutive_successes += 1;
        state.consecutive_failures = 0;
        state.last_success = Some(Instant::now());
        state.add_latency(latency_ms, self.config.latency_window);

        // Clear cooldown on success
        state.cooldown_until = None;

        // Notify circuit breaker
        if let Some(cb) = &self.circuit_breakers {
            cb.record_success(backend);
        }
    }

    /// Record a failed request
    pub fn record_failure(&self, backend: &str) {
        let mut providers = self.providers.write().unwrap();
        let state = providers.entry(backend.to_string()).or_default();

        state.total_requests += 1;
        state.failed_requests += 1;
        state.consecutive_failures += 1;
        state.consecutive_successes = 0;
        state.last_failure = Some(Instant::now());

        // Set cooldown if becoming unhealthy
        if state.consecutive_failures >= self.config.unhealthy_failure_count {
            state.cooldown_until =
                Some(Instant::now() + Duration::from_secs(self.config.cooldown_secs));
        }

        // Notify circuit breaker
        if let Some(cb) = &self.circuit_breakers {
            cb.record_failure(backend);
        }
    }

    /// Get health state for a backend
    pub fn get_health(&self, backend: &str) -> HealthState {
        let providers = self.providers.read().unwrap();
        providers
            .get(backend)
            .map(|s| s.compute_state(&self.config))
            .unwrap_or(HealthState::Healthy)
    }

    /// Get detailed health info for a backend
    pub fn get_health_info(&self, backend: &str) -> ProviderHealth {
        let providers = self.providers.read().unwrap();
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(state) = providers.get(backend) {
            ProviderHealth {
                backend: backend.to_string(),
                state: state.compute_state(&self.config),
                consecutive_failures: state.consecutive_failures,
                avg_latency_ms: state.avg_latency(),
                total_requests: state.total_requests,
                successful_requests: state.successful_requests,
                failed_requests: state.failed_requests,
                last_success: state.last_success.map(|_| now_unix),
                last_failure: state.last_failure.map(|_| now_unix),
                cooldown_until: state
                    .cooldown_until
                    .map(|c| now_unix + c.saturating_duration_since(Instant::now()).as_secs()),
            }
        } else {
            ProviderHealth {
                backend: backend.to_string(),
                state: HealthState::Healthy,
                consecutive_failures: 0,
                avg_latency_ms: 0.0,
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                last_success: None,
                last_failure: None,
                cooldown_until: None,
            }
        }
    }

    /// Get health info for all backends
    pub fn all_health_info(&self) -> Vec<ProviderHealth> {
        let providers = self.providers.read().unwrap();
        providers.keys().map(|k| self.get_health_info(k)).collect()
    }

    /// Check if a backend is available (healthy or degraded, not in cooldown)
    pub fn is_available(&self, backend: &str) -> bool {
        // Check circuit breaker first
        if let Some(cb) = &self.circuit_breakers {
            if cb.is_open(backend) {
                return false;
            }
        }

        let providers = self.providers.read().unwrap();
        if let Some(state) = providers.get(backend) {
            // Not available if in cooldown
            if state.is_in_cooldown() {
                return false;
            }
            // Available if healthy or degraded
            let health = state.compute_state(&self.config);
            health != HealthState::Unhealthy
        } else {
            // Unknown backend is assumed available
            true
        }
    }

    /// Get list of available backends from a list of candidates
    pub fn filter_available(&self, backends: &[String]) -> Vec<String> {
        backends
            .iter()
            .filter(|b| self.is_available(b))
            .cloned()
            .collect()
    }

    /// Get combined health and circuit breaker status
    pub fn get_status(&self, backend: &str) -> ProviderStatus {
        let health = self.get_health(backend);
        let circuit_state = self
            .circuit_breakers
            .as_ref()
            .and_then(|cb| cb.stats(backend))
            .map(|s| s.state)
            .unwrap_or(CircuitState::Closed);

        ProviderStatus {
            backend: backend.to_string(),
            health,
            circuit_state,
            available: self.is_available(backend),
        }
    }
}

impl Default for ProviderHealthRegistry {
    fn default() -> Self {
        Self::new(HealthConfig::default())
    }
}

/// Combined status for a provider
#[derive(Debug, Clone, Serialize)]
pub struct ProviderStatus {
    pub backend: String,
    pub health: HealthState,
    pub circuit_state: CircuitState,
    pub available: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starts_healthy() {
        let registry = ProviderHealthRegistry::default();
        assert_eq!(registry.get_health("test"), HealthState::Healthy);
        assert!(registry.is_available("test"));
    }

    #[test]
    fn test_success_tracking() {
        let registry = ProviderHealthRegistry::default();
        registry.record_success("test", 100);
        registry.record_success("test", 200);

        let info = registry.get_health_info("test");
        assert_eq!(info.total_requests, 2);
        assert_eq!(info.successful_requests, 2);
        assert_eq!(info.failed_requests, 0);
        assert_eq!(info.avg_latency_ms, 150.0);
    }

    #[test]
    fn test_degraded_on_failures() {
        let config = HealthConfig {
            degraded_failure_count: 2,
            unhealthy_failure_count: 5,
            ..Default::default()
        };
        let registry = ProviderHealthRegistry::new(config);

        registry.record_failure("test");
        assert_eq!(registry.get_health("test"), HealthState::Healthy);

        registry.record_failure("test");
        assert_eq!(registry.get_health("test"), HealthState::Degraded);

        // Success resets
        registry.record_success("test", 100);
        assert_eq!(registry.get_health("test"), HealthState::Healthy);
    }

    #[test]
    fn test_unhealthy_on_many_failures() {
        let config = HealthConfig {
            degraded_failure_count: 2,
            unhealthy_failure_count: 4,
            cooldown_secs: 60,
            ..Default::default()
        };
        let registry = ProviderHealthRegistry::new(config);

        for _ in 0..4 {
            registry.record_failure("test");
        }

        assert_eq!(registry.get_health("test"), HealthState::Unhealthy);
        assert!(!registry.is_available("test"));
    }

    #[test]
    fn test_degraded_on_high_latency() {
        let config = HealthConfig {
            degraded_latency_ms: 1000, // 1 second
            latency_window: 3,
            ..Default::default()
        };
        let registry = ProviderHealthRegistry::new(config);

        // High latency requests
        registry.record_success("test", 2000);
        registry.record_success("test", 2000);
        registry.record_success("test", 2000);

        assert_eq!(registry.get_health("test"), HealthState::Degraded);
    }

    #[test]
    fn test_filter_available() {
        let config = HealthConfig {
            unhealthy_failure_count: 2,
            cooldown_secs: 60,
            ..Default::default()
        };
        let registry = ProviderHealthRegistry::new(config);

        // Make backend2 unhealthy
        registry.record_failure("backend2");
        registry.record_failure("backend2");

        let backends = vec![
            "backend1".to_string(),
            "backend2".to_string(),
            "backend3".to_string(),
        ];
        let available = registry.filter_available(&backends);

        assert!(available.contains(&"backend1".to_string()));
        assert!(!available.contains(&"backend2".to_string()));
        assert!(available.contains(&"backend3".to_string()));
    }
}
