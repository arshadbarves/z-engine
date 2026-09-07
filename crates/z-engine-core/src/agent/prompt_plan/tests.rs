//! Prompt-assembly tests: that the manifest and the wire request are
//! one decision, that it is deterministic, and that an unbounded
//! prompt is refused rather than sent.

use super::*;
use crate::evidence::{BlobHandle, EvidenceRecord};
use crate::governance::WorkOrder;
use z_engine_provider::ToolDef;

fn tools() -> Vec<ToolDef> {
    vec![ToolDef::function(
        "read_file",
        "Read a file",
        serde_json::json!({"type": "object"}),
    )]
}

fn order() -> ActiveWorkOrder {
    let record = EvidenceRecord::new(
        "src/lib.rs",
        Some((1, 3)),
        "0".repeat(64),
        BlobHandle::of(b"fn parse() {}"),
        "read_file",
        "working-tree",
    );
    ActiveWorkOrder::for_test(
        WorkOrder {
            id: "wo-1".into(),
            goal: "make parse fallible".into(),
            writable_paths: vec!["src/lib.rs".into()],
            target_symbols: vec!["parse".into()],
            evidence_ids: vec![record.id.clone()],
            acceptance_commands: Vec::new(),
        },
        vec![record],
    )
}

fn materials<'a>(
    order: Option<&'a ActiveWorkOrder>,
    working: &'a [ChatMessage],
    tools: &'a [ToolDef],
    budget: u64,
) -> Materials<'a> {
    Materials {
        instructions: vec!["L0 instructions".into(), "# Repository symbol map".into()],
        order,
        working,
        tools,
        budget_tokens: budget,
    }
}

fn text(msg: &ChatMessage) -> String {
    role_and_content(msg).1
}

/// Every section the manifest lists is on the wire, and everything on
/// the wire is a section. That equivalence is what makes the manifest
/// authoritative rather than descriptive.
#[test]
fn the_manifest_sections_and_the_wire_messages_are_the_same_decision() {
    let order = order();
    let working = vec![
        ChatMessage::user("go"),
        ChatMessage::assistant_text("on it"),
    ];
    let tools = tools();
    let plan = bounded(&materials(Some(&order), &working, &tools, 100_000)).unwrap();
    let manifest = plan.manifest.as_ref().unwrap();

    let labels: Vec<&str> = manifest.sections.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "system-instructions",
            "order-digest",
            "evidence-0",
            "working-0",
            "working-1",
            "tool-0"
        ]
    );
    // Pinned sections, in order, are the prefix the wire carries.
    assert_eq!(plan.prefix_len, 4, "L0, repo map, digest, evidence");
    assert_eq!(text(&plan.messages[0]), "L0 instructions");
    assert_eq!(text(&plan.messages[1]), "# Repository symbol map");
    assert!(text(&plan.messages[2]).starts_with("# Active work order"));
    assert!(text(&plan.messages[3]).starts_with(EVIDENCE_HEADER));
    // …and the manifest measured exactly those strings.
    let joined = format!("{}\n\n{}", "L0 instructions", "# Repository symbol map");
    assert_eq!(manifest.sections[0].content, joined);
    assert_eq!(manifest.sections[1].content, text(&plan.messages[2]));
    assert_eq!(manifest.sections[2].content, text(&plan.messages[3]));
    // The conversation follows, untrimmed and in order.
    assert_eq!(plan.messages.len(), 6);
    assert_eq!(text(&plan.messages[4]), "go");
}

/// Determinism: the same materials produce the same manifest *and*
/// the same bytes, every time.
#[test]
fn the_same_materials_assemble_the_same_request_twice() {
    let order = order();
    let working = vec![ChatMessage::user("go")];
    let tools = tools();
    let a = bounded(&materials(Some(&order), &working, &tools, 100_000)).unwrap();
    let b = bounded(&materials(Some(&order), &working, &tools, 100_000)).unwrap();
    assert_eq!(a.manifest, b.manifest);
    assert_eq!(a.prefix_len, b.prefix_len);
    assert_eq!(
        a.messages.iter().map(text).collect::<Vec<_>>(),
        b.messages.iter().map(text).collect::<Vec<_>>()
    );
}

/// The overflow that invariant 7 is about: pinned content alone over
/// budget yields no request at all, so nothing can be posted.
#[test]
fn pinned_content_over_budget_yields_no_request() {
    let order = order();
    let working = vec![ChatMessage::user("go")];
    let tools = tools();
    let overflow = bounded(&materials(Some(&order), &working, &tools, 1))
        .expect_err("a prompt that cannot be bounded must not be assembled");
    assert!(overflow.required > 1);
    assert_eq!(overflow.budget, 1);
}

/// A trimmed conversation is trimmed on the wire too, with the same
/// marker the manifest reports — the model sees that history is
/// missing rather than reading a doctored transcript.
#[test]
fn trimming_drops_the_same_messages_from_the_manifest_and_the_wire() {
    let working: Vec<ChatMessage> = (0..12)
        .map(|i| ChatMessage::user(format!("m{i}: {}", "x".repeat(60))))
        .collect();
    let tools: Vec<ToolDef> = Vec::new();
    let plan = bounded(&materials(None, &working, &tools, 120)).unwrap();
    let manifest = plan.manifest.as_ref().unwrap();

    let kept: Vec<usize> = manifest
        .sections
        .iter()
        .filter_map(|s| s.label.strip_prefix("working-"))
        .filter_map(|i| i.parse::<usize>().ok())
        .collect();
    assert!(!kept.is_empty() && kept.len() < working.len(), "{kept:?}");
    assert_eq!(*kept.last().unwrap(), 11, "the newest message survives");

    let marker = plan
        .messages
        .iter()
        .map(text)
        .find(|t| t.contains("omitted to fit the context budget"))
        .expect("the wire must carry the omission marker the manifest reports");
    assert!(marker.contains(&kept[0].to_string()), "{marker}");

    // The wire carries the prefix, the marker, and exactly the kept run.
    assert_eq!(plan.messages.len(), plan.prefix_len + kept.len());
    assert_eq!(
        text(&plan.messages[plan.prefix_len]),
        text(&working[kept[0]])
    );
    assert!(manifest.estimated_tokens <= 120);
}

/// Trimming may only ever drop more, never leave a tool result whose
/// assistant message went with the trim — providers reject the pair.
#[test]
fn a_trim_never_leaves_an_orphan_tool_result_at_the_front() {
    let mut working: Vec<ChatMessage> = (0..8)
        .map(|i| ChatMessage::user(format!("m{i}: {}", "x".repeat(60))))
        .collect();
    working.push(ChatMessage::tool_result("call-1", "x".repeat(60)));
    working.push(ChatMessage::tool_result("call-2", "x".repeat(60)));
    working.push(ChatMessage::user("the newest message"));
    let tools: Vec<ToolDef> = Vec::new();

    let plan = bounded(&materials(None, &working, &tools, 90)).unwrap();
    let first = &plan.messages[plan.prefix_len..]
        .iter()
        .find(|m| !matches!(m, ChatMessage::System { .. }))
        .expect("something must survive");
    assert!(
        !matches!(first, ChatMessage::Tool { .. }),
        "a kept run may not begin with an unpaired tool result"
    );
}

/// The unguarded request is what it always was: no evidence block, no
/// marker, every message present, and an overflow is a log line.
#[test]
fn the_unguarded_request_is_unchanged_and_never_refused() {
    let order = order();
    let working: Vec<ChatMessage> = (0..6).map(|i| ChatMessage::user(format!("m{i}"))).collect();
    let tools = tools();
    let plan = legacy(&materials(Some(&order), &working, &tools, 1));

    assert!(
        plan.manifest.is_none(),
        "an over-budget unguarded prompt is reported, not truncated"
    );
    assert_eq!(
        plan.prefix_len, 3,
        "L0, repo map, digest — no evidence block"
    );
    assert_eq!(plan.messages.len(), 3 + working.len());
    assert!(
        plan.messages
            .iter()
            .map(text)
            .all(|t| !t.contains("omitted")),
        "an unguarded prompt is never trimmed here"
    );
}

/// With no order there is nothing to pin beyond the instructions, and
/// the guarded assembly says so rather than pinning an empty section.
#[test]
fn no_order_pins_no_digest_and_no_evidence() {
    let working = vec![ChatMessage::user("go")];
    let tools: Vec<ToolDef> = Vec::new();
    let plan = bounded(&materials(None, &working, &tools, 100_000)).unwrap();
    assert_eq!(plan.prefix_len, 2);
    let labels: Vec<&str> = plan
        .manifest
        .as_ref()
        .unwrap()
        .sections
        .iter()
        .map(|s| s.label.as_str())
        .collect();
    assert_eq!(labels, ["system-instructions", "working-0"]);
}
