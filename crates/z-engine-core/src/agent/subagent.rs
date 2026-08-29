//! Isolated sub-agent loop: read-only toolset, own transcript, bounded
//! rounds; returns the final assistant text only.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use z_engine_provider::{
    AccumulatedToolCall, ChatMessage, ChatProvider, ChatRequest, StreamEvent, ToolCall,
    ToolCallAccumulator,
};

use crate::perms::PolicyEngine;
use crate::tools::{ToolCtx, ToolRegistry};

use super::execute::parse_input;

/// Isolated sub-agent loop (spec section 9 v0.7): read-only toolset, own
/// transcript, bounded rounds; returns the final assistant text only.
#[allow(clippy::too_many_arguments)]
pub(super) async fn run_isolated(
    client: Arc<dyn ChatProvider>,
    model: String,
    project_root: PathBuf,
    tmp_dir: PathBuf,
    abort: Arc<AtomicBool>,
    prompt: &str,
    max_rounds: u32,
    max_output_tokens: u32,
) -> Result<String, String> {
    let perms = Arc::new(Mutex::new(PolicyEngine::new(Vec::new())));
    let mut ctx = ToolCtx::new(project_root.clone(), Arc::clone(&perms), tmp_dir);
    ctx.abort = Arc::clone(&abort);
    let registry = ToolRegistry::readonly_subset();

    let mut messages = vec![
        ChatMessage::system(crate::prompts::SUBAGENT),
        ChatMessage::user(prompt.to_string()),
    ];

    for round in 1..=max_rounds {
        if abort.load(Ordering::Relaxed) {
            return Err("aborted".into());
        }
        tracing::debug!(round, "sub-agent round");
        let request = ChatRequest::new(model.clone(), messages.clone())
            .with_tools(registry.defs())
            .with_max_tokens(max_output_tokens);
        let mut stream = client.stream_chat(&request, Arc::clone(&abort));

        let mut text = String::new();
        let mut acc = ToolCallAccumulator::default();
        // No command watching: sub-agents die with the parent's flag.
        loop {
            tokio::select! {
                item = stream.recv() => match item {
                    None => break,
                    Some(Err(e)) => return Err(format!("provider error in sub-agent: {e}")),
                    Some(Ok(StreamEvent::TextDelta(t))) => text.push_str(&t),
                    Some(Ok(StreamEvent::ReasoningDelta(_))) => {}
                    Some(Ok(StreamEvent::ToolCallDelta { index, id, name, args_delta })) => {
                        acc.absorb(index, id.as_deref(), name.as_deref(), &args_delta);
                    }
                    Some(Ok(StreamEvent::Usage(_))) => {}
                    Some(Ok(StreamEvent::Finish(_))) => {}
                    Some(Ok(StreamEvent::Done)) => break,
                },
                _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {
                    if abort.load(Ordering::Relaxed) {
                        return Err("aborted".into());
                    }
                }
            }
        }

        let finalized = acc.finish();
        let mut complete_calls: Vec<ToolCall> = Vec::new();
        let mut synthetic_errors: Vec<(String, String)> = Vec::new();
        for call in finalized {
            match call {
                AccumulatedToolCall::Complete(c) => complete_calls.push(c),
                AccumulatedToolCall::MalformedArguments {
                    id,
                    name,
                    raw_arguments,
                    reason,
                } => {
                    let raw_short: String = raw_arguments.chars().take(160).collect();
                    synthetic_errors.push((
                        id.clone(),
                        format!(
                            "ERROR: arguments not valid JSON ({reason}). You sent: {raw_short}"
                        ),
                    ));
                    // Keep the call on the wire so the error pairs up.
                    complete_calls.push(ToolCall {
                        id,
                        function: z_engine_provider::FunctionCall {
                            name: name.unwrap_or_default(),
                            arguments: raw_arguments,
                        },
                    });
                }
                AccumulatedToolCall::MissingId { index } => {
                    messages.push(ChatMessage::user(format!(
                        "[harness] tool call index {index} had no id; skipped."
                    )));
                }
            }
        }

        // Assistant message must precede its tool results on the wire.
        messages.push(ChatMessage::Assistant {
            content: (!text.is_empty()).then_some(text),
            tool_calls: complete_calls.clone(),
        });
        for (id, err) in synthetic_errors {
            messages.push(ChatMessage::tool_result(id, err));
        }

        if complete_calls.is_empty() {
            return Ok(messages
                .last()
                .and_then(|m| match m {
                    ChatMessage::Assistant {
                        content: Some(c), ..
                    } => Some(c.clone()),
                    _ => None,
                })
                .unwrap_or_default());
        }

        for call in &complete_calls {
            if abort.load(Ordering::Relaxed) {
                return Err("aborted".into());
            }
            let input = parse_input(&call.function.arguments);
            let content = match registry.get(&call.function.name) {
                Some(tool) => match tool.run(input, &ctx).await {
                    Ok(out) => out.result,
                    Err(e) => format!("ERROR: {e}"),
                },
                None => format!("ERROR: unknown tool {}", call.function.name),
            };
            messages.push(ChatMessage::tool_result(call.id.clone(), content));
        }
    }

    Err(format!(
        "sub-agent hit its {max_rounds}-round limit without concluding"
    ))
}
