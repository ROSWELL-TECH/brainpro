//! ActivateSkill tool for model-invoked skill activation.

use super::SchemaOptions;
use serde_json::{json, Value};

/// Get the ActivateSkill tool schema
pub fn schema(opts: &SchemaOptions) -> Value {
    if opts.optimize {
        json!({
            "type": "function",
            "function": {
                "name": "ActivateSkill",
                "description": "Activate skill pack",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "reason": { "type": "string" }
                    },
                    "required": ["name"]
                }
            }
        })
    } else {
        json!({
            "type": "function",
            "function": {
                "name": "ActivateSkill",
                "description": "Activate a skill pack to gain specialized instructions and optionally restrict available tools. Use when the task matches a skill's description. View available skills in the 'Available skill packs:' section of the system prompt.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Name of the skill pack to activate"
                        },
                        "reason": {
                            "type": "string",
                            "description": "Brief reason for activating this skill (optional)"
                        }
                    },
                    "required": ["name"]
                }
            }
        })
    }
}
