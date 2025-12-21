//! Context management for conversation history and compaction.
//!
//! This module tracks conversation context and provides compaction to
//! summarize old messages when context grows too large.
//!
//! NOTE: This module is prepared for future compaction feature integration.
//! The ConversationContext will be used by agent.rs when /compact is implemented.

#![allow(dead_code)]

use crate::config::ContextConfig;
use serde_json::Value;

/// Statistics about current context usage
#[derive(Debug, Clone)]
pub struct ContextStats {
    pub system_prompt_chars: usize,
    pub summary_chars: usize,
    pub messages_chars: usize,
    pub total_chars: usize,
    pub message_count: usize,
    pub max_chars: usize,
    pub usage_ratio: f64,
    pub has_summary: bool,
}

/// Result of a compaction operation
#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub chars_before: usize,
    pub chars_after: usize,
    pub messages_before: usize,
    pub messages_after: usize,
}

/// Manages conversation context with compaction support
pub struct ConversationContext {
    /// Fixed system prompt (always sent first)
    pub system_prompt: String,
    /// Optional summary from previous compaction
    pub summary_message: Option<String>,
    /// Rolling window of recent messages
    pub messages: Vec<Value>,
    /// Configuration
    pub config: ContextConfig,
}

impl ConversationContext {
    /// Create a new context with the given system prompt and config
    pub fn new(system_prompt: String, config: ContextConfig) -> Self {
        Self {
            system_prompt,
            summary_message: None,
            messages: Vec::new(),
            config,
        }
    }

    /// Add a message to the context
    pub fn push_message(&mut self, message: Value) {
        self.messages.push(message);
    }

    /// Get current context statistics
    pub fn stats(&self) -> ContextStats {
        let system_prompt_chars = self.system_prompt.len();
        let summary_chars = self.summary_message.as_ref().map(|s| s.len()).unwrap_or(0);
        let messages_chars: usize = self.messages.iter().map(estimate_message_chars).sum();
        let total_chars = system_prompt_chars + summary_chars + messages_chars;

        ContextStats {
            system_prompt_chars,
            summary_chars,
            messages_chars,
            total_chars,
            message_count: self.messages.len(),
            max_chars: self.config.max_chars,
            usage_ratio: total_chars as f64 / self.config.max_chars as f64,
            has_summary: self.summary_message.is_some(),
        }
    }

    /// Check if compaction is needed based on threshold
    pub fn needs_compaction(&self) -> bool {
        if !self.config.auto_compact_enabled {
            return false;
        }

        let stats = self.stats();
        stats.usage_ratio >= self.config.auto_compact_threshold
    }

    /// Build messages array for LLM request
    /// Returns [system, summary?, ...messages]
    pub fn build_request_messages(&self) -> Vec<Value> {
        let mut result = Vec::with_capacity(self.messages.len() + 2);

        // System prompt
        result.push(serde_json::json!({
            "role": "system",
            "content": self.system_prompt
        }));

        // Summary if present
        if let Some(summary) = &self.summary_message {
            result.push(serde_json::json!({
                "role": "system",
                "content": format!("Previous conversation summary:\n{}", summary)
            }));
        }

        // Recent messages
        result.extend(self.messages.clone());

        result
    }

    /// Apply compaction with the given summary
    /// Replaces old messages with summary + keeps last K turns
    pub fn apply_compaction(&mut self, summary: String) -> CompactionResult {
        let chars_before = self.stats().total_chars;
        let messages_before = self.messages.len();

        // Keep the last N messages (where N is keep_last_turns * 2 for user+assistant pairs)
        let keep_count = self.config.keep_last_turns * 2;
        let new_messages = if self.messages.len() > keep_count {
            self.messages.split_off(self.messages.len() - keep_count)
        } else {
            std::mem::take(&mut self.messages)
        };

        self.messages = new_messages;
        self.summary_message = Some(summary);

        let chars_after = self.stats().total_chars;
        let messages_after = self.messages.len();

        CompactionResult {
            chars_before,
            chars_after,
            messages_before,
            messages_after,
        }
    }

    /// Clear all messages but keep system prompt and config
    pub fn clear(&mut self) {
        self.messages.clear();
        self.summary_message = None;
    }

    /// Get the messages that should be compacted (older messages, excluding recent ones)
    pub fn messages_to_compact(&self) -> Vec<Value> {
        let keep_count = self.config.keep_last_turns * 2;
        if self.messages.len() <= keep_count {
            return Vec::new();
        }

        self.messages[..self.messages.len() - keep_count].to_vec()
    }
}

/// Estimate character count for a message (JSON serialized)
fn estimate_message_chars(msg: &Value) -> usize {
    // Use JSON serialization for accurate char count
    serde_json::to_string(msg).map(|s| s.len()).unwrap_or(0)
}

/// Build a prompt asking the LLM to summarize the conversation
pub fn build_compaction_prompt(instructions: Option<&str>) -> String {
    let base = r#"Summarize this conversation for context continuation. Include:
1. Key decisions made
2. File changes performed (paths and what was changed)
3. Outstanding TODOs or next steps
4. Important commands run and their outcomes
5. Any errors encountered and how they were resolved
6. User preferences discovered

Be concise but comprehensive. The summary will replace older messages to save context space."#;

    match instructions {
        Some(instr) => format!("{}\n\nAdditional focus: {}", base, instr),
        None => base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn default_config() -> ContextConfig {
        ContextConfig::default()
    }

    #[test]
    fn test_new_context() {
        let ctx = ConversationContext::new("You are helpful.".to_string(), default_config());
        assert_eq!(ctx.system_prompt, "You are helpful.");
        assert!(ctx.summary_message.is_none());
        assert!(ctx.messages.is_empty());
    }

    #[test]
    fn test_push_message() {
        let mut ctx = ConversationContext::new("System".to_string(), default_config());
        ctx.push_message(json!({"role": "user", "content": "Hello"}));
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn test_stats() {
        let mut ctx = ConversationContext::new("System prompt".to_string(), default_config());
        ctx.push_message(json!({"role": "user", "content": "Hello world"}));

        let stats = ctx.stats();
        assert!(stats.system_prompt_chars > 0);
        assert!(stats.messages_chars > 0);
        assert_eq!(stats.message_count, 1);
        assert!(!stats.has_summary);
    }

    #[test]
    fn test_build_request_messages() {
        let mut ctx = ConversationContext::new("You are helpful.".to_string(), default_config());
        ctx.push_message(json!({"role": "user", "content": "Hi"}));
        ctx.push_message(json!({"role": "assistant", "content": "Hello!"}));

        let msgs = ctx.build_request_messages();
        assert_eq!(msgs.len(), 3); // system + 2 messages
        assert_eq!(msgs[0]["role"], "system");
    }

    #[test]
    fn test_build_request_messages_with_summary() {
        let mut ctx = ConversationContext::new("You are helpful.".to_string(), default_config());
        ctx.summary_message = Some("Previous summary here.".to_string());
        ctx.push_message(json!({"role": "user", "content": "Hi"}));

        let msgs = ctx.build_request_messages();
        assert_eq!(msgs.len(), 3); // system + summary + 1 message
        assert!(msgs[1]["content"]
            .as_str()
            .unwrap()
            .contains("Previous conversation summary"));
    }

    #[test]
    fn test_apply_compaction() {
        let mut config = default_config();
        config.keep_last_turns = 2; // Keep last 4 messages (2 turns)

        let mut ctx = ConversationContext::new("System".to_string(), config);

        // Add 10 messages
        for i in 0..10 {
            ctx.push_message(json!({"role": "user", "content": format!("Message {}", i)}));
        }

        let result = ctx.apply_compaction("Summary of old messages".to_string());

        assert!(result.messages_before > result.messages_after);
        assert_eq!(ctx.messages.len(), 4); // 2 turns * 2 = 4 messages
        assert!(ctx.summary_message.is_some());
    }

    #[test]
    fn test_clear() {
        let mut ctx = ConversationContext::new("System".to_string(), default_config());
        ctx.push_message(json!({"role": "user", "content": "Hello"}));
        ctx.summary_message = Some("Summary".to_string());

        ctx.clear();

        assert!(ctx.messages.is_empty());
        assert!(ctx.summary_message.is_none());
    }

    #[test]
    fn test_needs_compaction() {
        let mut config = default_config();
        config.max_chars = 100;
        config.auto_compact_threshold = 0.5;
        config.auto_compact_enabled = true;

        let mut ctx = ConversationContext::new("System".to_string(), config);

        // Initially should not need compaction
        assert!(!ctx.needs_compaction());

        // Add a long message to exceed threshold
        ctx.push_message(json!({
            "role": "user",
            "content": "A".repeat(200)
        }));

        assert!(ctx.needs_compaction());
    }

    #[test]
    fn test_needs_compaction_disabled() {
        let mut config = default_config();
        config.auto_compact_enabled = false;
        config.max_chars = 10;

        let mut ctx = ConversationContext::new("System".to_string(), config);
        ctx.push_message(json!({"role": "user", "content": "A".repeat(100)}));

        // Should not need compaction even if over threshold
        assert!(!ctx.needs_compaction());
    }
}
