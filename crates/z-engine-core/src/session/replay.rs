use super::{Replayed, SessionEvent};

/// Rebuild provider messages and durable notes, excluding verification reports.
pub fn replay(events: &[SessionEvent]) -> Replayed {
    use z_engine_provider::{ChatMessage, FunctionCall, ToolCall};

    let mut working: Vec<ChatMessage> = Vec::new();
    let mut notes_replayed = Vec::new();
    let mut pending_rounds: Vec<usize> = Vec::new();

    for ev in events {
        match ev {
            SessionEvent::UserMsg { text, images } => {
                working.push(ChatMessage::user_with_images(text.clone(), images));
            }
            SessionEvent::AssistantMsg {
                content,
                tool_calls,
            } => {
                let converted: Vec<ToolCall> = tool_calls
                    .iter()
                    .map(|tc| ToolCall {
                        id: tc.id.clone(),
                        function: FunctionCall {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        },
                    })
                    .collect();
                if !tool_calls.is_empty() {
                    pending_rounds.push(working.len());
                }
                for tc in tool_calls {
                    if tc.name == "update_context_notes" {
                        notes_replayed.push(tc.arguments.clone());
                    }
                }
                working.push(ChatMessage::Assistant {
                    content: content.clone(),
                    tool_calls: converted,
                });
            }
            SessionEvent::ToolResult {
                tool_call_id,
                content,
            } => {
                pending_rounds.pop();
                working.push(ChatMessage::tool_result(
                    tool_call_id.clone(),
                    content.clone(),
                ));
            }
            SessionEvent::Note { text } => notes_replayed.push(text.clone()),
            SessionEvent::Meta { .. }
            | SessionEvent::Title { .. }
            | SessionEvent::TurnEnd { .. }
            | SessionEvent::TaskUpdated { .. }
            | SessionEvent::Ack => {}
        }
    }

    // Providers reject trailing assistant tool calls without their replies.
    while let Some(&idx) = pending_rounds.last() {
        working.truncate(idx);
        pending_rounds.pop();
    }
    Replayed {
        working,
        notes_replayed,
    }
}
