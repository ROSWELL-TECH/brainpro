//! Privacy module for Zero Data Retention (ZDR) enforcement.
//!
//! This module provides:
//! - Privacy level classification (Standard, Sensitive, Strict)
//! - Prompt scanning for sensitive patterns
//! - Auto-escalation to Strict when sensitive data detected
//! - ZDR-aware provider filtering

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::OnceLock;

/// Privacy level for a request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PrivacyLevel {
    /// Default - any provider is acceptable
    #[default]
    Standard,
    /// Prefer ZDR providers, warn if non-ZDR used
    Sensitive,
    /// ZDR-only providers, fail if unavailable
    Strict,
}

impl PrivacyLevel {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "standard" => Some(Self::Standard),
            "sensitive" => Some(Self::Sensitive),
            "strict" => Some(Self::Strict),
            _ => None,
        }
    }

    /// Get as string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Sensitive => "sensitive",
            Self::Strict => "strict",
        }
    }

    /// Check if ZDR is required
    pub fn requires_zdr(&self) -> bool {
        *self == PrivacyLevel::Strict
    }

    /// Check if ZDR is preferred
    pub fn prefers_zdr(&self) -> bool {
        matches!(self, PrivacyLevel::Sensitive | PrivacyLevel::Strict)
    }
}

/// Privacy configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PrivacyConfig {
    /// Default privacy level for all requests
    #[serde(default)]
    pub default_level: PrivacyLevel,

    /// Patterns that auto-escalate to Strict
    #[serde(default = "default_strict_patterns")]
    pub strict_patterns: Vec<String>,

    /// Whether to audit ZDR violations
    #[serde(default)]
    pub audit_zdr_violations: bool,

    /// Prefer local provider for sensitive data
    #[serde(default)]
    pub prefer_local_for_sensitive: bool,
}

fn default_strict_patterns() -> Vec<String> {
    vec![
        r"password".to_string(),
        r"secret".to_string(),
        r"\bkey\b".to_string(),
        r"token".to_string(),
        r"api[_-]?key".to_string(),
        r"ssn".to_string(),
        r"social.?security".to_string(),
        r"credit.?card".to_string(),
        r"cvv".to_string(),
        r"private.?key".to_string(),
        r"-----BEGIN".to_string(),
        r"bearer\s".to_string(),
    ]
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            default_level: PrivacyLevel::Standard,
            strict_patterns: default_strict_patterns(),
            audit_zdr_violations: true,
            prefer_local_for_sensitive: true,
        }
    }
}

/// Compiled patterns for efficient scanning
struct CompiledPatterns {
    patterns: Vec<Regex>,
}

impl CompiledPatterns {
    fn new(patterns: &[String]) -> Self {
        let compiled: Vec<Regex> = patterns
            .iter()
            .filter_map(|p| {
                Regex::new(&format!("(?i){}", p))
                    .map_err(|e| eprintln!("[privacy] Invalid pattern '{}': {}", p, e))
                    .ok()
            })
            .collect();
        Self { patterns: compiled }
    }

    fn matches(&self, text: &str) -> bool {
        self.patterns.iter().any(|p| p.is_match(text))
    }

    fn find_matches(&self, text: &str) -> Vec<String> {
        self.patterns
            .iter()
            .filter(|p| p.is_match(text))
            .map(|p| p.to_string())
            .collect()
    }
}

/// Global compiled patterns cache
static COMPILED_PATTERNS: OnceLock<CompiledPatterns> = OnceLock::new();

fn get_compiled_patterns(config: &PrivacyConfig) -> &'static CompiledPatterns {
    COMPILED_PATTERNS.get_or_init(|| CompiledPatterns::new(&config.strict_patterns))
}

/// Result of privacy scanning
#[derive(Debug, Clone)]
pub struct PrivacyScanResult {
    /// Detected privacy level
    pub level: PrivacyLevel,
    /// Whether patterns were matched
    pub sensitive_detected: bool,
    /// Matched patterns (if any)
    pub matched_patterns: Vec<String>,
    /// Whether privacy was escalated from baseline
    pub escalated: bool,
}

/// Privacy scanner for analyzing prompts
pub struct PrivacyScanner {
    config: PrivacyConfig,
}

impl PrivacyScanner {
    /// Create a new scanner with the given config
    pub fn new(config: PrivacyConfig) -> Self {
        Self { config }
    }

    /// Scan a prompt and determine privacy level
    pub fn scan(&self, prompt: &str) -> PrivacyScanResult {
        let patterns = get_compiled_patterns(&self.config);
        let matched = patterns.find_matches(prompt);
        let sensitive_detected = !matched.is_empty();

        let (level, escalated) = if sensitive_detected {
            // Auto-escalate to Strict when patterns detected
            if self.config.default_level == PrivacyLevel::Strict {
                (PrivacyLevel::Strict, false)
            } else {
                (PrivacyLevel::Strict, true)
            }
        } else {
            (self.config.default_level, false)
        };

        PrivacyScanResult {
            level,
            sensitive_detected,
            matched_patterns: matched,
            escalated,
        }
    }

    /// Check if a backend is acceptable for a given privacy level
    pub fn is_backend_acceptable(&self, backend_zdr: bool, level: PrivacyLevel) -> bool {
        match level {
            PrivacyLevel::Standard => true,
            PrivacyLevel::Sensitive => true, // Warn but allow
            PrivacyLevel::Strict => backend_zdr,
        }
    }

    /// Get config reference
    pub fn config(&self) -> &PrivacyConfig {
        &self.config
    }
}

/// ZDR violation audit record
#[derive(Debug, Clone, Serialize)]
pub struct ZdrViolation {
    pub timestamp: u64,
    pub privacy_level: PrivacyLevel,
    pub backend: String,
    pub backend_has_zdr: bool,
    pub matched_patterns: Vec<String>,
}

/// Audit log for ZDR violations
#[derive(Debug, Default)]
pub struct PrivacyAuditLog {
    violations: Vec<ZdrViolation>,
}

impl PrivacyAuditLog {
    /// Record a potential ZDR violation
    pub fn record_violation(
        &mut self,
        privacy_level: PrivacyLevel,
        backend: &str,
        backend_has_zdr: bool,
        matched_patterns: Vec<String>,
    ) {
        // Only record if non-ZDR backend used for sensitive/strict data
        if privacy_level.prefers_zdr() && !backend_has_zdr {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            self.violations.push(ZdrViolation {
                timestamp,
                privacy_level,
                backend: backend.to_string(),
                backend_has_zdr,
                matched_patterns,
            });

            eprintln!(
                "[privacy:audit] ZDR violation: {} data sent to non-ZDR backend '{}'",
                privacy_level.as_str(),
                backend
            );
        }
    }

    /// Get all violations
    pub fn violations(&self) -> &[ZdrViolation] {
        &self.violations
    }

    /// Get recent violations (last N)
    pub fn recent(&self, n: usize) -> &[ZdrViolation] {
        let len = self.violations.len();
        if len <= n {
            &self.violations
        } else {
            &self.violations[len - n..]
        }
    }
}

/// Filter backends by ZDR status
pub fn filter_zdr_backends(
    backends: &[String],
    backend_zdr_map: &std::collections::HashMap<String, bool>,
    require_zdr: bool,
) -> Vec<String> {
    if !require_zdr {
        return backends.to_vec();
    }

    backends
        .iter()
        .filter(|b| backend_zdr_map.get(*b).copied().unwrap_or(false))
        .cloned()
        .collect()
}

/// Get ZDR-compliant backends from a list
pub fn get_zdr_backends(
    backends: &HashSet<String>,
    backend_zdr_map: &std::collections::HashMap<String, bool>,
) -> Vec<String> {
    backends
        .iter()
        .filter(|b| backend_zdr_map.get(*b).copied().unwrap_or(false))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_privacy_level_parsing() {
        assert_eq!(PrivacyLevel::from_str("standard"), Some(PrivacyLevel::Standard));
        assert_eq!(PrivacyLevel::from_str("SENSITIVE"), Some(PrivacyLevel::Sensitive));
        assert_eq!(PrivacyLevel::from_str("strict"), Some(PrivacyLevel::Strict));
        assert_eq!(PrivacyLevel::from_str("invalid"), None);
    }

    #[test]
    fn test_privacy_level_zdr_requirements() {
        assert!(!PrivacyLevel::Standard.requires_zdr());
        assert!(!PrivacyLevel::Sensitive.requires_zdr());
        assert!(PrivacyLevel::Strict.requires_zdr());

        assert!(!PrivacyLevel::Standard.prefers_zdr());
        assert!(PrivacyLevel::Sensitive.prefers_zdr());
        assert!(PrivacyLevel::Strict.prefers_zdr());
    }

    #[test]
    fn test_scan_clean_prompt() {
        let scanner = PrivacyScanner::new(PrivacyConfig::default());
        let result = scanner.scan("Please refactor this function");

        assert!(!result.sensitive_detected);
        assert!(result.matched_patterns.is_empty());
        assert!(!result.escalated);
    }

    #[test]
    fn test_scan_sensitive_prompt() {
        let scanner = PrivacyScanner::new(PrivacyConfig::default());
        let result = scanner.scan("Store the API_KEY in the config");

        assert!(result.sensitive_detected);
        assert!(!result.matched_patterns.is_empty());
        assert!(result.escalated);
        assert_eq!(result.level, PrivacyLevel::Strict);
    }

    #[test]
    fn test_scan_password_prompt() {
        let scanner = PrivacyScanner::new(PrivacyConfig::default());
        let result = scanner.scan("Please update the password field");

        assert!(result.sensitive_detected);
        assert!(result.escalated);
        assert_eq!(result.level, PrivacyLevel::Strict);
    }

    #[test]
    fn test_backend_acceptability() {
        let scanner = PrivacyScanner::new(PrivacyConfig::default());

        // Standard accepts all
        assert!(scanner.is_backend_acceptable(true, PrivacyLevel::Standard));
        assert!(scanner.is_backend_acceptable(false, PrivacyLevel::Standard));

        // Sensitive accepts all (warns but allows)
        assert!(scanner.is_backend_acceptable(true, PrivacyLevel::Sensitive));
        assert!(scanner.is_backend_acceptable(false, PrivacyLevel::Sensitive));

        // Strict requires ZDR
        assert!(scanner.is_backend_acceptable(true, PrivacyLevel::Strict));
        assert!(!scanner.is_backend_acceptable(false, PrivacyLevel::Strict));
    }

    #[test]
    fn test_filter_zdr_backends() {
        let mut zdr_map = std::collections::HashMap::new();
        zdr_map.insert("claude".to_string(), true);
        zdr_map.insert("chatgpt".to_string(), false);
        zdr_map.insert("ollama".to_string(), true);

        let backends = vec![
            "claude".to_string(),
            "chatgpt".to_string(),
            "ollama".to_string(),
        ];

        let filtered = filter_zdr_backends(&backends, &zdr_map, true);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&"claude".to_string()));
        assert!(filtered.contains(&"ollama".to_string()));
        assert!(!filtered.contains(&"chatgpt".to_string()));

        // Without ZDR requirement, all pass
        let all = filter_zdr_backends(&backends, &zdr_map, false);
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_audit_log() {
        let mut audit = PrivacyAuditLog::default();

        // Standard level doesn't record
        audit.record_violation(PrivacyLevel::Standard, "chatgpt", false, vec![]);
        assert!(audit.violations().is_empty());

        // Sensitive level records when non-ZDR
        audit.record_violation(
            PrivacyLevel::Sensitive,
            "chatgpt",
            false,
            vec!["password".to_string()],
        );
        assert_eq!(audit.violations().len(), 1);

        // ZDR backend doesn't record
        audit.record_violation(PrivacyLevel::Strict, "claude", true, vec![]);
        assert_eq!(audit.violations().len(), 1);
    }
}
