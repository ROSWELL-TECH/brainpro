//! Agent loop for processing user input and executing tool calls.

use crate::{cli::Context, llm, policy::Decision, tools};
use anyhow::Result;
use serde_json::{json, Value};

const MAX_ITERATIONS: usize = 12;

const SYSTEM_PROMPT: &str = r#"You are an agentic coding assistant running locally.
You can only access files via tools. All paths are relative to the project root.
Use Glob/Grep to find files before Read. Before Edit/Write, explain what you will change.
Use Bash for running builds, tests, formatters, and git operations.
Never use curl or wget - they are blocked by policy.
Keep edits minimal and precise."#;

fn trace(ctx: &Context, label: &str, content: &str) {
    if *ctx.tracing.borrow() {
        eprintln!("[TRACE:{}] {}", label, content);
    }
}

fn verbose(ctx: &Context, message: &str) {
    if ctx.args.verbose || ctx.args.debug {
        eprintln!("[VERBOSE] {}", message);
    }
}

pub fn run_turn(ctx: &Context, user_input: &str, messages: &mut Vec<Value>) -> Result<()> {
    let _ = ctx.transcript.borrow_mut().user_message(user_input);

    messages.push(json!({
        "role": "user",
        "content": user_input
    }));

    // Resolve target for current skill
    let skill = ctx.current_skill.borrow().clone();
    let config = ctx.config.borrow();
    let target = config
        .resolve_skill(&skill)
        .or_else(|| config.default_target())
        .ok_or_else(|| anyhow::anyhow!("No target configured for skill: {}", skill))?;
    let bash_config = config.bash.clone();
    drop(config);

    trace(ctx, "TARGET", &format!("{} (skill: {})", target, skill));

    let tool_schemas = tools::schemas();

    // Use max_turns from CLI if provided, otherwise default
    let max_iterations = ctx.args.max_turns.unwrap_or(MAX_ITERATIONS);

    for iteration in 1..=max_iterations {
        trace(ctx, "ITER", &format!("Starting iteration {}", iteration));

        // Get client for target's backend (lazy-loaded)
        let response = {
            let mut backends = ctx.backends.borrow_mut();
            let client = backends.get_client(&target.backend)?;

            let mut req_messages = vec![json!({
                "role": "system",
                "content": SYSTEM_PROMPT
            })];
            req_messages.extend(messages.clone());

            let request = llm::ChatRequest {
                model: target.model.clone(),
                messages: req_messages,
                tools: Some(tool_schemas.clone()),
                tool_choice: Some("auto".to_string()),
            };

            client.chat(&request)?
        };

        if response.choices.is_empty() {
            println!("No response from model");
            break;
        }

        let choice = &response.choices[0];
        let msg = &choice.message;

        if let Some(content) = &msg.content {
            if !content.is_empty() {
                println!("{}", content);
                let _ = ctx.transcript.borrow_mut().assistant_message(content);
            }
        }

        let tool_calls = match &msg.tool_calls {
            Some(tc) if !tc.is_empty() => {
                // Trace thinking when there's content along with tool calls
                if let Some(content) = &msg.content {
                    if !content.is_empty() {
                        trace(ctx, "THINK", content);
                    }
                }
                tc
            }
            _ => {
                messages.push(json!({
                    "role": "assistant",
                    "content": msg.content
                }));
                break;
            }
        };

        let assistant_msg = json!({
            "role": "assistant",
            "content": msg.content,
            "tool_calls": tool_calls
        });
        messages.push(assistant_msg);

        for tc in tool_calls {
            let name = &tc.function.name;
            let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));

            trace(
                ctx,
                "CALL",
                &format!(
                    "{}({})",
                    name,
                    serde_json::to_string_pretty(&args).unwrap_or_default()
                ),
            );

            verbose(
                ctx,
                &format!("Tool call: {}({})", name, tc.function.arguments),
            );

            let _ = ctx.transcript.borrow_mut().tool_call(name, &args);

            // Use PolicyEngine for permission decisions
            let (allowed, decision, matched_rule) =
                ctx.policy.borrow().check_permission(name, &args);

            // Log policy decision to transcript
            let decision_str = match decision {
                Decision::Allow => "allowed",
                Decision::Deny => "denied",
                Decision::Ask => {
                    if allowed {
                        "prompted_yes"
                    } else {
                        "prompted_no"
                    }
                }
            };
            let _ = ctx.transcript.borrow_mut().policy_decision(
                name,
                decision_str,
                matched_rule.as_deref(),
            );

            let result = if allowed {
                tools::execute(name, args.clone(), &ctx.root, &bash_config)?
            } else {
                let reason = match decision {
                    Decision::Deny => "Denied by policy",
                    _ => "User denied permission",
                };
                json!({ "error": { "code": "permission_denied", "message": reason } })
            };

            let ok = result.get("error").is_none();
            let _ = ctx.transcript.borrow_mut().tool_result(name, ok, &result);

            trace(
                ctx,
                "RESULT",
                &format!(
                    "{}: {}",
                    name,
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                ),
            );

            verbose(ctx, &format!("Tool result: {} ok={}", name, ok));

            messages.push(json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": serde_json::to_string(&result)?
            }));
        }
    }

    Ok(())
}
