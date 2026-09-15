use super::*;
use z_engine_provider::{FunctionCall, ToolCall};

fn assistant_with_tools(id: &str) -> ChatMessage {
    ChatMessage::Assistant {
        content: None,
        tool_calls: vec![ToolCall {
            id: id.into(),
            function: FunctionCall {
                name: "read_file".into(),
                arguments: "{}".into(),
            },
        }],
    }
}

fn sample() -> Vec<ChatMessage> {
    vec![
        ChatMessage::user("early task"),
        ChatMessage::assistant_text("early answer"),
        assistant_with_tools("c1"),
        ChatMessage::tool_result("c1", "big output A"),
        ChatMessage::user("mid task"),
        ChatMessage::assistant_text("mid answer"),
        assistant_with_tools("c2"),
        ChatMessage::tool_result("c2", "big output B"),
        ChatMessage::user("recent task"),
        ChatMessage::assistant_text("recent answer"),
    ]
}

#[test]
fn tail_stays_verbatim_and_head_elides() {
    let tmp = tempfile::tempdir().unwrap();
    let msgs = sample();
    let out = compact(&msgs, 4, tmp.path()).unwrap();
    assert_eq!(out.messages.len(), 6);
    assert!(
        matches!(&out.messages[0], ChatMessage::Assistant { tool_calls, .. } if !tool_calls.is_empty())
    );
    let elided_c1 = match &out.messages[1] {
        ChatMessage::Tool { content, .. } => content,
        other => panic!("{other:?}"),
    };
    let path = existing_spill_path(elided_c1).unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "big output A");
    assert!(
        matches!(&out.messages[3], ChatMessage::Tool { content, .. } if content == "big output B")
    );
    assert!(matches!(&out.messages[4], ChatMessage::User { content } if content == "recent task"));
    assert!(matches!(&out.messages[5], ChatMessage::Assistant { .. }));
    assert!(out.summarize_input.contains("[user] early task"));
    assert!(out.summarize_input.contains("[assistant] early answer"));
    assert!(out.summarize_input.contains("[user] mid task"));
    assert!(out.summarize_input.contains("[assistant] mid answer"));
    assert!(!out.summarize_input.contains("recent task"));
    assert_eq!(out.elided_tool_outputs, 1);
    assert_eq!(out.dropped_prose_messages, 4);
}

#[test]
fn tail_never_starts_on_orphaned_tool_reply() {
    let msgs = vec![
        ChatMessage::user("u1"),
        assistant_with_tools("x"),
        ChatMessage::tool_result("x", "out"),
        ChatMessage::user("u2"),
    ];
    let tmp = tempfile::tempdir().unwrap();
    let out = compact(&msgs, 2, tmp.path()).unwrap();
    assert_eq!(out.messages.len(), 3);
    assert!(
        matches!(out.messages.first(), Some(ChatMessage::Assistant { tool_calls, .. }) if !tool_calls.is_empty())
    );
    assert!(matches!(out.messages.get(2), Some(ChatMessage::User { content }) if content == "u2"));
}

#[test]
fn small_list_is_untouched() {
    let msgs = sample();
    let tmp = tempfile::tempdir().unwrap();
    let out = compact(&msgs, 50, tmp.path()).unwrap();
    assert_eq!(
        serde_json::to_value(out.messages).unwrap(),
        serde_json::to_value(msgs).unwrap()
    );
    assert_eq!(out.elided_tool_outputs, 0);
    assert!(out.summarize_input.is_empty());
}

#[test]
fn oversized_prose_stays_original_instead_of_being_clipped_by_the_summarizer() {
    let tmp = tempfile::tempdir().unwrap();
    let huge = "\u{1f980}".repeat(MAX_SUMMARIZE_CHARS);
    let msgs = vec![
        ChatMessage::user(&huge),
        ChatMessage::assistant_text("small"),
    ];
    let out = compact(&msgs, 0, tmp.path()).unwrap();
    assert_eq!(out.summarize_input, "[assistant] small");
    assert_eq!(out.dropped_prose_messages, 1);
    assert!(matches!(&out.messages[0], ChatMessage::User { content } if content == &huge));
}

#[test]
fn summary_input_keeps_whole_messages_at_the_character_ceiling() {
    let tmp = tempfile::tempdir().unwrap();
    let first = "\u{1f980}".repeat(MAX_SUMMARIZE_CHARS - "[user] ".chars().count());
    let msgs = vec![ChatMessage::user(&first), ChatMessage::user("must remain")];
    let out = compact(&msgs, 0, tmp.path()).unwrap();
    assert_eq!(out.summarize_input.chars().count(), MAX_SUMMARIZE_CHARS);
    assert_eq!(out.dropped_prose_messages, 1);
    assert!(matches!(&out.messages[0], ChatMessage::User { content } if content == "must remain"));
}

#[test]
fn images_and_tool_call_narration_are_not_silently_discarded() {
    let tmp = tempfile::tempdir().unwrap();
    let mut anchor = assistant_with_tools("c1");
    if let ChatMessage::Assistant { content, .. } = &mut anchor {
        *content = Some("Important narration not sent for summarization".into());
    }
    let image =
        ChatMessage::user_with_images("Important image", &["data:image/png;base64,AA==".into()]);
    let msgs = vec![image, anchor, ChatMessage::tool_result("c1", "output")];
    let out = compact(&msgs, 0, tmp.path()).unwrap();
    assert_eq!(
        serde_json::to_value(&out.messages[..2]).unwrap(),
        serde_json::to_value(&msgs[..2]).unwrap()
    );
    assert!(out.summarize_input.is_empty());
    assert_eq!(out.dropped_prose_messages, 0);
}
