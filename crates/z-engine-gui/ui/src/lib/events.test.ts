import { describe, expect, it, beforeEach } from "vitest";
import {
  attachmentStore,
  handleEvent,
  replaySession,
  resetForTests,
  resetTranscript,
  resolveApproval,
  sessionStore,
  submitLocal,
  tailLines,
  transcriptStore,
  usageStore,
  type Msg,
} from "./events";

function msgs(): Msg[] {
  return transcriptStore.getSnapshot();
}

beforeEach(() => resetForTests());

describe("tokenDelta", () => {
  it("accumulates into a single streaming assistant message", () => {
    handleEvent({ type: "turnStarted" });
    handleEvent({ type: "tokenDelta", text: "Hel" });
    handleEvent({ type: "tokenDelta", text: "lo" });
    expect(msgs()).toHaveLength(1);
    expect(msgs()[0].kind).toBe("assistant");
    expect(msgs()[0].text).toBe("Hello");
    expect(msgs()[0].streaming).toBe(true);
  });

  it("ends the assistant message on turnCompleted", () => {
    handleEvent({ type: "tokenDelta", text: "hi" });
    handleEvent({ type: "turnCompleted", promptTokens: 10, completionTokens: 2 });
    expect(msgs()[0].streaming).toBe(false);
  });
});

describe("thinking", () => {
  it("streams reasoning as a growing thinking message", () => {
    handleEvent({ type: "reasoningDelta", text: "abcd" });
    handleEvent({ type: "reasoningDelta", text: "efg" });
    const m = msgs()[0];
    expect(m.kind).toBe("thinking");
    expect(m.thinkingBody).toBe("abcdefg");
    expect(m.streaming).toBe(true);
  });

  it("auto-collapses on next token but retains the body for toggling", () => {
    handleEvent({ type: "reasoningDelta", text: "secret plan" });
    handleEvent({ type: "tokenDelta", text: "Answer" });
    const thinking = msgs().find((m) => m.kind === "thinking")!;
    expect(thinking).toBeDefined();
    expect(thinking.streaming).toBe(false);
    expect(thinking.collapsed).toBe(true);
    expect(thinking.thinkingBody).toBe("secret plan");
    expect(thinking.text).toContain("(11 chars)");
    // assistant continues on its own card
    expect(msgs().find((m) => m.kind === "assistant")!.text).toBe("Answer");
  });

  it("toggleCollapsed flips collapsed without losing the body", () => {
    handleEvent({ type: "reasoningDelta", text: "abc" });
    handleEvent({ type: "tokenDelta", text: "x" });
    const id = msgs().find((m) => m.kind === "thinking")!.id;
    handleEvent({ type: "toggleThinking", id });
    let thinking = msgs().find((m) => m.kind === "thinking")!;
    expect(thinking.collapsed).toBe(false);
    handleEvent({ type: "toggleThinking", id });
    thinking = msgs().find((m) => m.kind === "thinking")!;
    expect(thinking.collapsed).toBe(true);
    expect(thinking.thinkingBody).toBe("abc");
  });
});

describe("tool cards", () => {
  it("starts a running card with name, preview and startedAt", () => {
    const before = Date.now();
    handleEvent({ type: "toolCallStarted", name: "bash", preview: "cargo test" });
    const m = msgs()[0];
    expect(m.kind).toBe("tool");
    expect(m.toolName).toBe("bash");
    expect(m.preview).toBe("cargo test");
    expect(m.streaming).toBe(true);
    expect(m.startedAt!).toBeGreaterThanOrEqual(before);
  });

  it("appends live output deltas while running", () => {
    handleEvent({ type: "toolCallStarted", name: "bash", preview: "ls" });
    handleEvent({ type: "toolOutputDelta", toolName: "bash", text: "line1\nline2\n" });
    const m = msgs()[0];
    expect(m.output).toBe("line1\nline2\n");
    expect(tailLines(m.output!)).toEqual(["line1", "line2"]);
  });

  it("keeps only the last 10 tail lines", () => {
    handleEvent({ type: "toolCallStarted", name: "bash", preview: "seq" });
    const lines = Array.from({ length: 14 }, (_, i) => `L${i}`).join("\n");
    handleEvent({ type: "toolOutputDelta", toolName: "bash", text: lines });
    const m = msgs()[0];
    expect(m.output!.split("\n")).toHaveLength(14);
    expect(tailLines(m.output!)[0]).toBe("L4");
    expect(tailLines(m.output!)).toHaveLength(10);
  });

  it("finish records ok, durationMs and summary; full output kept for expand", () => {
    handleEvent({ type: "toolCallStarted", name: "bash", preview: "ls" });
    handleEvent({ type: "toolOutputDelta", toolName: "bash", text: "out" });
    handleEvent({
      type: "toolCallFinished",
      name: "bash",
      ok: true,
      durationMs: 1234,
      summary: "3 lines",
    });
    const m = msgs()[0];
    expect(m.streaming).toBe(false);
    expect(m.ok).toBe(true);
    expect(m.durationMs).toBe(1234);
    expect(m.summary).toBe("3 lines");
    expect(m.output).toBe("out");
  });

  it("overlapping same-name tools each get their own finish (no infinite spinner)", () => {
    handleEvent({ type: "toolCallStarted", name: "read_file", preview: "A.toml" });
    handleEvent({ type: "toolCallStarted", name: "read_file", preview: "B.lock" });
    handleEvent({
      type: "toolCallFinished",
      name: "read_file",
      ok: true,
      durationMs: 5,
      summary: "A.toml (lines 1–61)",
    });
    let cards = msgs().filter((m) => m.kind === "tool");
    // first finish closes the most recent open card of that name…
    expect(cards[0].streaming).toBe(true);
    expect(cards[1].streaming).toBe(false);
    expect(cards[1].summary).toBe("A.toml (lines 1–61)");
    handleEvent({
      type: "toolCallFinished",
      name: "read_file",
      ok: true,
      durationMs: 7,
      summary: "B.lock (lines 1–2000)",
    });
    cards = msgs().filter((m) => m.kind === "tool");
    expect(cards[0].streaming).toBe(false);
    expect(cards[0].summary).toBe("B.lock (lines 1–2000)");
    // no phantom orphan notices
    expect(msgs().some((m) => m.kind === "notice" && m.text.includes("read_file"))).toBe(false);
  });

  it("live output routes to the newest open card of that tool", () => {
    handleEvent({ type: "toolCallStarted", name: "bash", preview: "one" });
    handleEvent({ type: "toolCallStarted", name: "bash", preview: "two" });
    handleEvent({ type: "toolOutputDelta", toolName: "bash", text: "tick" });
    expect(msgs()[1].output).toBe("tick");
    expect(msgs()[0].output).toBe("");
  });

  it("ignores orphan deltas when no tool is open", () => {
    handleEvent({ type: "toolOutputDelta", toolName: "bash", text: "x" });
    expect(msgs()).toHaveLength(0);
  });

  it("finish without start falls back to a notice card", () => {
    handleEvent({
      type: "toolCallFinished",
      name: "grep",
      ok: false,
      durationMs: 5,
      summary: "no match",
    });
    const m = msgs()[0];
    expect(m.kind).toBe("error");
    expect(m.text).toContain("grep");
  });
});

describe("approval", () => {
  it("captures id, scopes and diff detailPreview", () => {
    handleEvent({
      type: "approvalRequired",
      id: 7,
      tool: "edit_file",
      inputPreview: "src/lib.rs",
      suggestedRule: null,
      detailPreview: "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new",
      canPersist: true,
      bashCommand: null,
    });
    const m = msgs()[0];
    expect(m.kind).toBe("approval");
    expect(m.approvalId).toBe(7);
    expect(m.canPersist).toBe(true);
    expect(m.detailPreview).toContain("@@ -1");
  });
});

describe("usage", () => {
  it("usageUpdated feeds the usage store", () => {
    handleEvent({ type: "usageUpdated", promptTokens: 1000, completionTokens: 50 });
    expect(usageStore.getSnapshot().promptTokens).toBe(1000);
    expect(usageStore.getSnapshot().completionTokens).toBe(50);
  });

  it("turnCompleted also updates usage", () => {
    handleEvent({ type: "turnCompleted", promptTokens: 2000, completionTokens: 20 });
    expect(usageStore.getSnapshot().promptTokens).toBe(2000);
  });
});

describe("user + command echo", () => {
  it("submitLocal pushes a user card", async () => {
    const { submitLocal } = await import("./events");
    submitLocal("fix it");
    expect(msgs()[0].kind).toBe("user");
    expect(msgs()[0].text).toBe("fix it");
  });
});

describe("approval resolution (A3)", () => {
  it("collapses the card to an approved notice in place", () => {
    handleEvent({
      type: "approvalRequired",
      id: 3,
      tool: "bash",
      inputPreview: "cargo test",
      suggestedRule: "cargo test*",
      detailPreview: null,
      canPersist: true,
      bashCommand: "cargo test",
    });
    resolveApproval(3, "once");
    const m = msgs()[0];
    expect(m.kind).toBe("notice");
    expect(m.text).toContain("✓ approved");
    expect(m.text).toContain("cargo test");
    expect(m.approvalId).toBe(3); // id retained so re-resolution is a no-op-safe
  });

  it("records session/persist scope and denials", () => {
    handleEvent({
      type: "approvalRequired",
      id: 4,
      tool: "bash",
      inputPreview: "rm x",
      suggestedRule: null,
      detailPreview: null,
      canPersist: false,
      bashCommand: "rm x",
    });
    resolveApproval(4, "session");
    expect(msgs()[0].text).toContain("session rule");
    handleEvent({
      type: "approvalRequired",
      id: 5,
      tool: "edit_file",
      inputPreview: "a.rs",
      suggestedRule: null,
      detailPreview: null,
      canPersist: false,
      bashCommand: null,
    });
    resolveApproval(5, "deny");
    expect(msgs()[1].text).toContain("✗ denied");
  });

  it("ignores unknown approval ids", () => {
    resolveApproval(999, "once");
    expect(msgs()).toHaveLength(0);
  });
});

describe("turn markers (A6)", () => {
  it("turnCompleted pushes ✓ done", () => {
    handleEvent({ type: "turnCompleted", promptTokens: 1, completionTokens: 1 });
    expect(msgs().some((m) => m.kind === "notice" && m.text.includes("✓ done"))).toBe(true);
  });
  it("turnAborted pushes ■ aborted", () => {
    handleEvent({ type: "turnAborted" });
    expect(msgs().some((m) => m.kind === "notice" && m.text.includes("■ aborted"))).toBe(true);
  });
});

describe("status note routing (A5)", () => {
  it("unknown notes become inline notices, not toasts", () => {
    handleEvent({ type: "statusNote", text: "reviewer: looks good" });
    const m = msgs()[0];
    expect(m.kind).toBe("notice");
    expect(m.text).toBe("reviewer: looks good");
  });
  it("shell echoes and context-pressure events stay inline", () => {
    handleEvent({ type: "statusNote", text: "$ ls" });
    handleEvent({ type: "statusNote", text: "context at 95% of budget" });
    expect(msgs().map((m) => m.text)).toEqual(["$ ls", "context at 95% of budget"]);
    expect(msgs()).toHaveLength(2); // durable transcript notices, no toast
  });
});

describe("session replay", () => {
  // Shapes below mirror exactly what serde produces for
  // z_engine_core::session::SessionEvent (snake_case variant tags).
  it("rebuilds transcript cards from serde-tagged session JSONL events", () => {
    resetTranscript();
    replaySession([
      { type: "meta", model: "m", project_root: "/" },
      { type: "user_msg", text: "fix it" },
      {
        type: "assistant_msg",
        content: "Reading the file.",
        tool_calls: [{ id: "t1", name: "read_file", arguments: '{"path":"Cargo.toml"}' }],
      },
      { type: "tool_result", tool_call_id: "t1", content: "--- stdout ---\nline" },
      { type: "note", text: "compacted" },
    ]);
    expect(msgs().map((m) => m.kind)).toEqual(["user", "assistant", "tool", "notice"]);
    const tool = msgs()[2];
    expect(tool.toolName).toBe("read_file");
    expect(tool.preview).toBe("Cargo.toml");
    expect(tool.streaming).toBe(false);
    expect(tool.summary).toContain("line");
    expect(tool.output).toContain("line");
    expect(msgs()[3].text).toBe("compacted");
  });

  it("regression: camelCase tags match nothing (serde emits snake_case)", () => {
    resetTranscript();
    replaySession([{ type: "user_msg", text: "real shape" }]);
    expect(msgs().map((m) => m.text)).toEqual(["real shape"]);
    replaySession([{ type: "userMsg", text: "legacy shape" }]);
    expect(msgs()).toHaveLength(0);
  });

  it("resetTranscript clears everything", () => {
    submitLocal("hi");
    resetTranscript();
    expect(msgs()).toHaveLength(0);
  });
});

describe("transcriptTrimmed", () => {
  it("drops the kept user message and everything after it", () => {
    submitLocal("first");
    handleEvent({ type: "tokenDelta", text: "ok" });
    handleEvent({ type: "turnCompleted", promptTokens: 1, completionTokens: 1 });
    submitLocal("second");
    handleEvent({ type: "transcriptTrimmed", keepTurn: 1 });
    expect(msgs().filter((m) => m.kind === "user").map((m) => m.text)).toEqual(["first"]);
    expect(msgs().some((m) => m.text === "second")).toBe(false);
  });

  it("is a no-op when the keep turn is not in the transcript", () => {
    submitLocal("only");
    handleEvent({ type: "transcriptTrimmed", keepTurn: 9 });
    expect(msgs()).toHaveLength(1);
  });
});

describe("attachments (B)", () => {
  it("add/remove/clear maintain the list", () => {
    attachmentStore.add("src/lib.rs");
    attachmentStore.add("README.md");
    attachmentStore.add("src/lib.rs"); // dedupe
    expect(attachmentStore.getSnapshot()).toEqual(["src/lib.rs", "README.md"]);
    attachmentStore.remove("src/lib.rs");
    expect(attachmentStore.getSnapshot()).toEqual(["README.md"]);
    attachmentStore.clear();
    expect(attachmentStore.getSnapshot()).toEqual([]);
  });

  it("sessionChanged records the active session ulid", () => {
    handleEvent({ type: "sessionChanged", ulid: "01ABC" });
    expect(sessionStore.getSnapshot()).toBe("01ABC");
  });
});
