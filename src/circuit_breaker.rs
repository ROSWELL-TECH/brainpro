//! Circuit breaker pattern for protecting against cascading failures.
//!
//! States: Closed -> Open -> HalfOpen -> Closed
//! - Closed: Normal operation, requests pass through
//! - Open: Failures exceeded threshold, requests are rejected immediately
//! - HalfOpen: Recovery period, limited requests allowed to probe health

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CircuitState {
    #[default]
    Closed,
    Open,
    HalfOpen,
}

/// Configuration for circuit breaker behavior
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening circuit
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    /// Duration to keep circuit open before trying half-open
    #[serde(default = "default_recovery_timeout_secs")]
    pub recovery_timeout_secs: u64,
    /// Number of successful probes in half-open before closing
    #[serde(default = "default_half_open_probes")]
    pub half_open_probes: u32,
    /// Enable circuit breaker (can be disabled via config)
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_failure_threshold() -> u32 {
    5
}
fn default_recovery_timeout_secs() -> u64 {
    30
}
fn default_half_open_probes() -> u32 {
    3
}
fn default_enabled() -> bool {
    true
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: default_failure_threshold(),
            recovery_timeout_secs: default_recovery_timeout_secs(),
            half_open_probes: default_half_open_probes(),
            enabled: default_enabled(),
        }
    }
}

/// Internal state for a single circuit breaker
#[derive(Debug)]
struct CircuitBreakerState {
    state: CircuitState,
    consecutive_failures: u32,
    consecutive_successes: u32,
    last_failure_time: Option<Instant>,
    total_failures: u64,
    total_successes: u64,
    total_rejections: u64,
}

impl Default for CircuitBreakerState {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
            last_failure_time: None,
            total_failures: 0,
            total_successes: 0,
            total_rejections: 0,
        }
    }
}

/// Result of a circuit breaker check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitBreakerDecision {
    /// Request should proceed
    Allow,
    /// Request rejected due to open circuit
    Reject,
    /// Request allowed as a probe in half-open state
    Probe,
}

/// A single circuit breaker instance
#[derive(Debug)]
pub struct CircuitBreaker {
    name: String,
    config: CircuitBreakerConfig,
    state: RwLock<CircuitBreakerState>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given name and config
    pub fn new(name: &str, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.to_string(),
            config,
            state: RwLock::new(CircuitBreakerState::default()),
        }
    }

    /// Get current state
    pub fn state(&self) -> CircuitState {
        self.state.read().unwrap().state
    }

    /// Check if request should be allowed
    pub fn check(&self) -> CircuitBreakerDecision {
        if !self.config.enabled {
            return CircuitBreakerDecision::Allow;
        }

        let mut state = self.state.write().unwrap();

        match state.state {
            CircuitState::Closed => CircuitBreakerDecision::Allow,
            CircuitState::Open => {
                // Check if recovery timeout has passed
                if let Some(last_failure) = state.last_failure_time {
                    let recovery_duration = Duration::from_secs(self.config.recovery_timeout_secs);
                    if last_failure.elapsed() >= recovery_duration {
                        // Transition to half-open
                        state.state = CircuitState::HalfOpen;
                        state.consecutive_successes = 0;
                        eprintln!(
                            "[circuit_breaker:{}] Transitioning to half-open after {}s recovery",
                            self.name, self.config.recovery_timeout_secs
                        );
                        return CircuitBreakerDecision::Probe;
                    }
                }
                state.total_rejections += 1;
                CircuitBreakerDecision::Reject
            }
            CircuitState::HalfOpen => CircuitBreakerDecision::Probe,
        }
    }

    /// Record a successful request
    pub fn record_success(&self) {
        if !self.config.enabled {
            return;
        }

        let mut state = self.state.write().unwrap();
        state.total_successes += 1;
        state.consecutive_failures = 0;

        match state.state {
            CircuitState::Closed => {
                // Already closed, nothing to do
            }
            CircuitState::HalfOpen => {
                state.consecutive_successes += 1;
                if state.consecutive_successes >= self.config.half_open_probes {
                    // Close the circuit
                    state.state = CircuitState::Closed;
                    state.consecutive_successes = 0;
                    eprintln!(
                        "[circuit_breaker:{}] Circuit closed after {} successful probes",
                        self.name, self.config.half_open_probes
                    );
                }
            }
            CircuitState::Open => {
                // Shouldn't happen, but close circuit on success
                state.state = CircuitState::Closed;
            }
        }
    }

    /// Record a failed request
    pub fn record_failure(&self) {
        if !self.config.enabled {
            return;
        }

        let mut state = self.state.write().unwrap();
        state.total_failures += 1;
        state.consecutive_failures += 1;
        state.consecutive_successes = 0;
        state.last_failure_time = Some(Instant::now());

        match state.state {
            CircuitState::Closed => {
                if state.consecutive_failures >= self.config.failure_threshold {
                    state.state = CircuitState::Open;
                    eprintln!(
                        "[circuit_breaker:{}] Circuit opened after {} consecutive failures",
                        self.name, state.consecutive_failures
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Failure in half-open immediately reopens
                state.state = CircuitState::Open;
                eprintln!(
                    "[circuit_breaker:{}] Circuit reopened after probe failure",
                    self.name
                );
            }
            CircuitState::Open => {
                // Already open, just track the failure
            }
        }
    }

    /// Get statistics for this circuit breaker
    pub fn stats(&self) -> CircuitBreakerStats {
        let state = self.state.read().unwrap();
        CircuitBreakerStats {
            name: self.name.clone(),
            state: state.state,
            consecutive_failures: state.consecutive_failures,
            total_failures: state.total_failures,
            total_successes: state.total_successes,
            total_rejections: state.total_rejections,
        }
    }
}

/// Statistics for a circuit breaker
#[derive(Debug, Clone, Serialize)]
pub struct CircuitBreakerStats {
    pub name: String,
    pub state: CircuitState,
    pub consecutive_failures: u32,
    pub total_failures: u64,
    pub total_successes: u64,
    pub total_rejections: u64,
}

/// Registry of circuit breakers per backend
#[derive(Debug)]
pub struct CircuitBreakerRegistry {
    config: CircuitBreakerConfig,
    breakers: Arc<RwLock<HashMap<String, Arc<CircuitBreaker>>>>,
}

impl CircuitBreakerRegistry {
    /// Create a new registry with the given config
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            breakers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a circuit breaker for a backend
    pub fn get(&self, backend: &str) -> Arc<CircuitBreaker> {
        let breakers = self.breakers.read().unwrap();
        if let Some(cb) = breakers.get(backend) {
            return cb.clone();
        }
        drop(breakers);

        // Create new circuit breaker
        let mut breakers = self.breakers.write().unwrap();
        // Double-check after acquiring write lock
        if let Some(cb) = breakers.get(backend) {
            return cb.clone();
        }

        let cb = Arc::new(CircuitBreaker::new(backend, self.config.clone()));
        breakers.insert(backend.to_string(), cb.clone());
        cb
    }

    /// Check if a request to backend should be allowed
    pub fn check(&self, backend: &str) -> CircuitBreakerDecision {
        self.get(backend).check()
    }

    /// Record success for a backend
    pub fn record_success(&self, backend: &str) {
        self.get(backend).record_success();
    }

    /// Record failure for a backend
    pub fn record_failure(&self, backend: &str) {
        self.get(backend).record_failure();
    }

    /// Get statistics for all circuit breakers
    pub fn all_stats(&self) -> Vec<CircuitBreakerStats> {
        let breakers = self.breakers.read().unwrap();
        breakers.values().map(|cb| cb.stats()).collect()
    }

    /// Get statistics for a specific backend
    pub fn stats(&self, backend: &str) -> Option<CircuitBreakerStats> {
        let breakers = self.breakers.read().unwrap();
        breakers.get(backend).map(|cb| cb.stats())
    }

    /// Check if circuit is open for a backend
    pub fn is_open(&self, backend: &str) -> bool {
        let breakers = self.breakers.read().unwrap();
        breakers
            .get(backend)
            .map(|cb| cb.state() == CircuitState::Open)
            .unwrap_or(false)
    }
}

impl Default for CircuitBreakerRegistry {
    fn default() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::new("test", CircuitBreakerConfig::default());
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.check(), CircuitBreakerDecision::Allow);
    }

    #[test]
    fn test_circuit_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout_secs: 30,
            half_open_probes: 2,
            enabled: true,
        };
        let cb = CircuitBreaker::new("test", config);

        // First two failures keep circuit closed
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        // Third failure opens circuit
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert_eq!(cb.check(), CircuitBreakerDecision::Reject);
    }

    #[test]
    fn test_success_resets_failure_count() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout_secs: 30,
            half_open_probes: 2,
            enabled: true,
        };
        let cb = CircuitBreaker::new("test", config);

        cb.record_failure();
        cb.record_failure();
        cb.record_success(); // Resets consecutive failures
        cb.record_failure();
        cb.record_failure();

        // Still closed because success reset the count
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_disabled() {
        let config = CircuitBreakerConfig {
            enabled: false,
            ..Default::default()
        };
        let cb = CircuitBreaker::new("test", config);

        // Record many failures
        for _ in 0..10 {
            cb.record_failure();
        }

        // Circuit should still allow requests when disabled
        assert_eq!(cb.check(), CircuitBreakerDecision::Allow);
    }

    #[test]
    fn test_half_open_recovery() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout_secs: 0, // Immediate recovery for testing
            half_open_probes: 2,
            enabled: true,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for recovery (timeout is 0)
        std::thread::sleep(Duration::from_millis(10));

        // Should transition to half-open on check
        let decision = cb.check();
        assert_eq!(decision, CircuitBreakerDecision::Probe);
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Successful probes close the circuit
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_half_open_failure_reopens() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout_secs: 0,
            half_open_probes: 2,
            enabled: true,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open circuit and transition to half-open
        cb.record_failure();
        cb.record_failure();
        std::thread::sleep(Duration::from_millis(10));
        cb.check(); // Triggers transition to half-open

        // Failure in half-open reopens circuit
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_registry_per_backend() {
        let registry = CircuitBreakerRegistry::default();

        // Different backends get different circuit breakers
        let cb1 = registry.get("backend1");
        let _cb2 = registry.get("backend2");

        cb1.record_failure();
        cb1.record_failure();
        cb1.record_failure();
        cb1.record_failure();
        cb1.record_failure();

        // Only backend1 should be affected
        assert!(registry.is_open("backend1"));
        assert!(!registry.is_open("backend2"));
    }

    #[test]
    fn test_stats() {
        let cb = CircuitBreaker::new("test", CircuitBreakerConfig::default());

        cb.record_success();
        cb.record_success();
        cb.record_failure();

        let stats = cb.stats();
        assert_eq!(stats.name, "test");
        assert_eq!(stats.total_successes, 2);
        assert_eq!(stats.total_failures, 1);
        assert_eq!(stats.state, CircuitState::Closed);
    }
}
