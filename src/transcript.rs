use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Transcript {
    pub path: PathBuf,
    session_id: String,
    cwd: PathBuf,
    file: File,
}

#[derive(Serialize)]
struct Event<'a> {
    ts: DateTime<Utc>,
    session_id: &'a str,
    cwd: &'a Path,
    #[serde(rename = "type")]
    event_type: &'a str,
    #[serde(flatten)]
    data: serde_json::Value,
}

impl Transcript {
    pub fn new(path: &Path, session_id: &str, cwd: &Path) -> Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;

        Ok(Self {
            path: path.to_path_buf(),
            session_id: session_id.to_string(),
            cwd: cwd.to_path_buf(),
            file,
        })
    }

    pub fn log(&mut self, event_type: &str, data: serde_json::Value) -> Result<()> {
        let event = Event {
            ts: Utc::now(),
            session_id: &self.session_id,
            cwd: &self.cwd,
            event_type,
            data,
        };
        let line = serde_json::to_string(&event)?;
        writeln!(self.file, "{}", line)?;
        self.file.flush()?;
        Ok(())
    }

    pub fn user_message(&mut self, content: &str) -> Result<()> {
        self.log("user_message", serde_json::json!({ "content": content }))
    }

    pub fn assistant_message(&mut self, content: &str) -> Result<()> {
        self.log(
            "assistant_message",
            serde_json::json!({ "content": content }),
        )
    }

    pub fn tool_call(&mut self, tool: &str, args: &serde_json::Value) -> Result<()> {
        self.log(
            "tool_call",
            serde_json::json!({ "tool": tool, "args": args }),
        )
    }

    pub fn tool_result(&mut self, tool: &str, ok: bool, result: &serde_json::Value) -> Result<()> {
        self.log(
            "tool_result",
            serde_json::json!({ "tool": tool, "ok": ok, "result": result }),
        )
    }

    #[allow(dead_code)]
    pub fn permission(&mut self, tool: &str, allowed: bool) -> Result<()> {
        self.log(
            "permission",
            serde_json::json!({ "tool": tool, "allowed": allowed }),
        )
    }

    /// Log a policy decision for a tool call
    pub fn policy_decision(
        &mut self,
        tool: &str,
        decision: &str,
        rule_matched: Option<&str>,
    ) -> Result<()> {
        self.log(
            "policy_decision",
            serde_json::json!({
                "tool": tool,
                "decision": decision,
                "rule_matched": rule_matched,
            }),
        )
    }

    /// Log a context compaction event
    #[allow(dead_code)]
    pub fn compact_event(
        &mut self,
        chars_before: usize,
        chars_after: usize,
        messages_before: usize,
        messages_after: usize,
    ) -> Result<()> {
        self.log(
            "compact_event",
            serde_json::json!({
                "chars_before": chars_before,
                "chars_after": chars_after,
                "messages_before": messages_before,
                "messages_after": messages_after,
            }),
        )
    }

    /// Log settings loaded at startup
    #[allow(dead_code)]
    pub fn settings_loaded(
        &mut self,
        config_files: &[String],
        permission_mode: &str,
        allow_rules: usize,
        ask_rules: usize,
        deny_rules: usize,
    ) -> Result<()> {
        self.log(
            "settings_loaded",
            serde_json::json!({
                "config_files": config_files,
                "permission_mode": permission_mode,
                "allow_rules": allow_rules,
                "ask_rules": ask_rules,
                "deny_rules": deny_rules,
            }),
        )
    }

    #[allow(dead_code)]
    pub fn error(&mut self, message: &str) -> Result<()> {
        self.log("error", serde_json::json!({ "message": message }))
    }
}
