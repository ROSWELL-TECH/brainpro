//! Venice pricing cache module.
//!
//! Fetches and caches Venice model pricing from their API.
//! Cache is refreshed if older than 1 week.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::cost::ModelPricing;

/// Cache expiry duration (1 week)
const CACHE_MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;

/// Venice API endpoint for listing models
const VENICE_MODELS_URL: &str = "https://api.venice.ai/api/v1/models";

/// Cached pricing data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VenicePricingCache {
    /// Unix timestamp when cache was fetched
    pub fetched_at: u64,
    /// Model pricing: model_id -> (input_price, output_price) per 1M tokens
    pub models: HashMap<String, ModelPricing>,
}

impl VenicePricingCache {
    /// Check if cache is still valid (less than 1 week old)
    pub fn is_valid(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.fetched_at) < CACHE_MAX_AGE_SECS
    }
}

/// Response structure from Venice /models endpoint
#[derive(Debug, Deserialize)]
struct VeniceModelsResponse {
    data: Vec<VeniceModel>,
}

#[derive(Debug, Deserialize)]
struct VeniceModel {
    id: String,
    model_spec: Option<VeniceModelSpec>,
    #[serde(rename = "type")]
    model_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VeniceModelSpec {
    pricing: Option<VenicePricing>,
}

#[derive(Debug, Deserialize)]
struct VenicePricing {
    input: Option<VenicePriceValue>,
    output: Option<VenicePriceValue>,
}

#[derive(Debug, Deserialize)]
struct VenicePriceValue {
    usd: Option<f64>,
}

/// Get the cache file path (~/.yo/venice_pricing.json)
fn cache_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".yo").join("venice_pricing.json"))
}

/// Load cached pricing from disk
pub fn load_cache() -> Option<VenicePricingCache> {
    let path = cache_path()?;
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save pricing cache to disk
fn save_cache(cache: &VenicePricingCache) -> anyhow::Result<()> {
    let path = cache_path().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;

    // Ensure ~/.yo/ directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(cache)?;
    fs::write(&path, content)?;
    Ok(())
}

/// Fetch current pricing from Venice API
fn fetch_from_api() -> anyhow::Result<HashMap<String, ModelPricing>> {
    let agent = ureq::Agent::new();
    let resp: VeniceModelsResponse = agent
        .get(VENICE_MODELS_URL)
        .timeout(Duration::from_secs(10))
        .call()?
        .into_json()?;

    let mut models = HashMap::new();

    for model in resp.data {
        // Only include text models with pricing info
        if model.model_type.as_deref() != Some("text") {
            continue;
        }

        if let Some(spec) = model.model_spec {
            if let Some(pricing) = spec.pricing {
                let input = pricing.input.and_then(|p| p.usd).unwrap_or(0.0);
                let output = pricing.output.and_then(|p| p.usd).unwrap_or(0.0);

                // Skip models with zero pricing (likely errors or special cases)
                if input > 0.0 || output > 0.0 {
                    models.insert(model.id, ModelPricing::new(input, output));
                }
            }
        }
    }

    Ok(models)
}

/// Get Venice pricing, using cache if valid or fetching fresh data.
/// Returns None if both cache and API fail.
pub fn get_venice_pricing() -> Option<HashMap<String, ModelPricing>> {
    // Try to load from cache first
    if let Some(cache) = load_cache() {
        if cache.is_valid() {
            return Some(cache.models);
        }
    }

    // Cache is stale or missing, try to fetch fresh data
    match fetch_from_api() {
        Ok(models) => {
            let cache = VenicePricingCache {
                fetched_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                models: models.clone(),
            };

            // Save cache (ignore errors - not critical)
            let _ = save_cache(&cache);

            Some(models)
        }
        Err(_) => {
            // API failed, try to use stale cache if available
            load_cache().map(|c| c.models)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_validity() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Fresh cache should be valid
        let fresh = VenicePricingCache {
            fetched_at: now,
            models: HashMap::new(),
        };
        assert!(fresh.is_valid());

        // Old cache should be invalid
        let old = VenicePricingCache {
            fetched_at: now - CACHE_MAX_AGE_SECS - 1,
            models: HashMap::new(),
        };
        assert!(!old.is_valid());
    }
}
