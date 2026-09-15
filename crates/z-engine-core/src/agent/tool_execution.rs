use std::time::Instant;

use tokio::sync::mpsc::UnboundedSender;
use z_engine_provider::ToolCall;

use super::events::Event;
use super::execute::parse_input;
use crate::tools::{ToolCtx, ToolError, ToolOutput, ToolRegistry};

pub(super) fn input_preview(input: &serde_json::Value) -> String {
    let text = input.to_string();
    let mut preview: String = text.chars().take(160).collect();
    if text.chars().count() > 160 {
        preview.push('\u{2026}');
    }
    preview
}

pub(super) async fn run_one(
    call: ToolCall,
    ctx: &ToolCtx,
    registry: &ToolRegistry,
    ev_tx: &UnboundedSender<Event>,
) -> String {
    let started = Instant::now();
    let input = parse_input(&call.function.arguments);
    let name = call.function.name;
    let _ = ev_tx.send(Event::ToolCallStarted {
        name: name.clone(),
        preview: input_preview(&input),
    });
    let input_hook = input.clone();
    let result: Result<ToolOutput, ToolError> = if ctx.aborted() {
        Err(ToolError::Failed("Operation cancelled".into()))
    } else if let Err(error) = super::operation_tracking::before_call(&name, &input, ctx) {
        Err(ToolError::Failed(error.to_string()))
    } else {
        match registry.get(&name) {
            Some(tool) => tool.run(input, ctx).await,
            None => Err(ToolError::Failed(format!("unknown tool: {name}"))),
        }
    };
    let duration_ms = started.elapsed().as_millis() as u64;
    let mut out = match result {
        Ok(out) => out,
        Err(error) => {
            if matches!(name.as_str(), "run_verification" | "assess_completion") {
                if let Err(state_error) =
                    super::task_completion::block(ctx, format!("{name} failed: {error}"))
                {
                    tracing::error!(%state_error, "could not record verification failure");
                }
            }
            let _ = ev_tx.send(Event::ToolCallFinished {
                name,
                ok: false,
                duration_ms,
                summary: error.to_string(),
            });
            return format!("ERROR: {error}");
        }
    };
    crate::tools::lsp_tools::maybe_attach_diagnostics(
        &name,
        out.ok,
        &input_hook,
        ctx,
        &mut out.result,
    )
    .await;
    let _ = ev_tx.send(Event::ToolCallFinished {
        name,
        ok: out.ok,
        duration_ms,
        summary: out.summary,
    });
    out.result
}
