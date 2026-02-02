//! Agent persona system.
//!
//! This module provides distinct agent personas with separate behaviors:
//! - MrCode: Direct CLI, focused coding assistant with minimal toolset
//! - MrBot: Gateway/Docker path, conversational bot with SOUL.md support

pub mod hooks;
pub mod loader;
pub mod mrbot;
pub mod mrcode;

#[allow(unused_imports)] // Used by library consumers (yo binary)
pub use loader::{load_persona, PersonaConfig, WorkspaceContext};

use crate::agent::TurnResult;
use crate::cli::Context;
use crate::config::PermissionMode;
use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

/// Context for building system prompts
#[derive(Debug, Clone, Default)]
pub struct PromptContext {
    /// Working directory for the agent
    pub working_dir: PathBuf,
    /// Currently active skill packs
    pub active_skills: Vec<String>,
    /// Whether in plan mode
    pub plan_mode: bool,
    /// Whether optimize mode is enabled (-O flag)
    pub optimize_mode: bool,
    /// SOUL.md content for MrBot persona
    #[allow(dead_code)] // Used by library consumers
    pub soul_content: Option<String>,
    /// Workspace MEMORY.md content
    pub workspace_memory: Option<String>,
    /// Daily notes (filename, content) - today and yesterday
    pub daily_notes: Vec<(String, String)>,
    /// WORKING.md content - current task state
    pub working_state: Option<String>,
    /// BOOTSTRAP.md content - project onboarding/context
    pub bootstrap_content: Option<String>,
    /// Whether this is a subagent session (reduced context)
    pub is_subagent: bool,
}

impl PromptContext {
    /// Create a new prompt context from the CLI context
    pub fn from_context(ctx: &Context) -> Self {
        let active_skills = ctx
            .active_skills
            .borrow()
            .list()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let plan_mode = ctx.plan_mode.borrow().phase == crate::plan::PlanPhase::Planning;

        // Load workspace context from .brainpro/ directory
        let ws = loader::load_workspace_context(&ctx.root);

        Self {
            working_dir: ctx.root.clone(),
            active_skills,
            plan_mode,
            optimize_mode: ctx.args.optimize,
            soul_content: None,
            workspace_memory: ws.memory,
            daily_notes: ws.daily_notes,
            working_state: ws.working_state,
            bootstrap_content: ws.bootstrap,
            is_subagent: false,
        }
    }

    /// Set the SOUL content
    #[allow(dead_code)] // Used by library consumers
    pub fn with_soul(mut self, soul: Option<String>) -> Self {
        self.soul_content = soul;
        self
    }

    /// Set as subagent session (reduced context)
    pub fn into_subagent(mut self) -> Self {
        self.is_subagent = true;
        // Clear workspace context for subagents
        self.workspace_memory = None;
        self.daily_notes = Vec::new();
        self.working_state = None;
        self.bootstrap_content = None;
        self
    }
}

/// Trait defining an agent persona
#[allow(dead_code)] // Used by library consumers (yo binary)
pub trait Persona: Send + Sync {
    /// Persona identifier
    fn name(&self) -> &str;

    /// Get the persona configuration (loaded from files)
    fn config(&self) -> &PersonaConfig;

    /// Build the system prompt for this persona
    fn build_system_prompt(&self, ctx: &PromptContext) -> String;

    /// Run the agent loop (each persona has its own implementation)
    fn run_turn(
        &self,
        ctx: &Context,
        user_input: &str,
        messages: &mut Vec<Value>,
    ) -> Result<TurnResult>;

    /// Get available tools for this persona
    fn available_tools(&self) -> &[&str];

    /// Default permission mode for this persona
    fn permission_mode(&self) -> PermissionMode;
}

/// Get persona by name
#[allow(dead_code)] // Used by library consumers
pub fn get_persona(name: &str) -> Option<Box<dyn Persona>> {
    match name.to_lowercase().as_str() {
        "mrcode" => Some(Box::new(mrcode::MrCode::new())),
        "mrbot" => Some(Box::new(mrbot::MrBot::new())),
        _ => None,
    }
}

/// Get the default MrCode persona
#[allow(dead_code)] // Used by library consumers
pub fn mrcode() -> mrcode::MrCode {
    mrcode::MrCode::new()
}

/// Get the default MrBot persona
#[allow(dead_code)] // Used by library consumers
pub fn mrbot() -> mrbot::MrBot {
    mrbot::MrBot::new()
}
