use serde_json::json;

use super::*;

fn call(id: &str) -> Message {
    Message::new(
        Role::Assistant,
        vec![
            ContentBlock::text("checking"),
            ContentBlock::ToolUse {
                id: CallId::from(id),
                name: "Read".into(),
                input: json!({"file_path": "a.rs"}),
            },
        ],
    )
}

fn result(id: &str) -> Message {
    Message::new(
        Role::User,
        vec![ContentBlock::tool_result(
            CallId::from(id),
            "contents",
            false,
        )],
    )
}

/// Tool-round message that also carries a steering/reminder text block.
fn result_with_reminder(id: &str) -> Message {
    Message::new(
        Role::User,
        vec![
            ContentBlock::tool_result(CallId::from(id), "contents", false),
            ContentBlock::text("<system-reminder>\nsteer\n</system-reminder>"),
        ],
    )
}

/// 0 user, 1 call, 2 result+reminder, 3 answer, 4 user, 5 call, 6 result,
/// 7 answer, 8 user, 9 answer.
fn history() -> Vec<Message> {
    vec![
        Message::user_text("first task"),
        call("a"),
        result_with_reminder("a"),
        Message::assistant_text("done a"),
        Message::user_text("second task"),
        call("b"),
        result("b"),
        Message::assistant_text("done b"),
        Message::user_text("third task"),
        Message::assistant_text("done c"),
    ]
}

#[test]
fn short_or_single_turn_histories_have_nothing_to_summarize() {
    assert_eq!(plan_summary(&[], 0), None);
    let messages = history();
    assert_eq!(plan_summary(&messages, messages.len()), None);
    assert_eq!(plan_summary(&messages, messages.len() + 3), None);
    let single_turn = &messages[..4];
    assert_eq!(plan_summary(single_turn, 0), None);
}

#[test]
fn split_is_the_latest_real_user_turn_that_keeps_enough_messages() {
    let messages = history();
    assert_eq!(plan_summary(&messages, 0), Some(SummaryPlan { split: 8 }));
    assert_eq!(plan_summary(&messages, 2), Some(SummaryPlan { split: 8 }));
    assert_eq!(plan_summary(&messages, 3), Some(SummaryPlan { split: 4 }));
    assert_eq!(plan_summary(&messages, 6), Some(SummaryPlan { split: 4 }));
    assert_eq!(plan_summary(&messages, 7), None);
}

#[test]
fn tool_rounds_with_reminder_text_are_not_turn_starts() {
    let messages = vec![
        Message::user_text("task"),
        call("a"),
        result_with_reminder("a"),
        Message::assistant_text("done"),
    ];
    assert!(
        !messages[2].is_tool_results(),
        "precondition: mixed content"
    );
    assert_eq!(plan_summary(&messages, 0), None);
}

#[test]
fn pairs_split_across_a_user_turn_block_that_split() {
    // A result delivered after a later user message still pins its call.
    let messages = vec![
        Message::user_text("task"),
        call("late"),
        Message::user_text("are you there?"),
        result("late"),
        Message::assistant_text("done"),
    ];
    assert_eq!(plan_summary(&messages, 0), None);
}

#[test]
fn apply_summary_keeps_the_tail_verbatim() {
    let messages = history();
    let plan = plan_summary(&messages, 2).unwrap();
    let summary = summary_message("Did tasks a and b.");
    let compacted = apply_summary(&messages, &plan, &summary);
    assert_eq!(compacted.len(), 3);
    assert_eq!(compacted[0], summary);
    assert_eq!(compacted[1..], messages[8..]);
    let stale = apply_summary(&messages, &SummaryPlan { split: 99 }, &summary);
    assert_eq!(stale, vec![summary]);
}

#[test]
fn summary_message_is_a_user_message_embedding_the_summary() {
    let message = summary_message("\n- Implemented the parser.\n");
    assert_eq!(message.role, Role::User);
    let text = message.text();
    assert!(text.contains("- Implemented the parser."), "{text}");
    assert!(!text.contains("{{"), "{text}");
    assert_eq!(text, text.trim());
}
