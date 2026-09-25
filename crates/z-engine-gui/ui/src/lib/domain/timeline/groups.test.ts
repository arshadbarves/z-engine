import { describe, expect, it } from "vitest";
import { groupTurnItems, runLine, summarizeRun, type RunCall } from "./groups";
import type { TimelineItem } from "./turns";

const tool = (id: string, name: string, input: Record<string, unknown> = {}): TimelineItem => ({
  kind: "tool",
  key: `tool:${id}`,
  use: { callId: id, name, input: input as never },
});
const text = (id: string): TimelineItem => ({ kind: "text", key: id, messageId: id, text: "hi" });

function call(name: string, input: Record<string, unknown> = {}, status: RunCall["status"] = "ok"): RunCall {
  return { name, input: input as never, status };
}

describe("groupTurnItems", () => {
  it("folds consecutive tool calls into one run and keeps text between runs", () => {
    const blocks = groupTurnItems([tool("a", "Read"), tool("b", "Grep"), text("t"), tool("c", "Bash"), tool("d", "Edit")]);
    expect(blocks.map((b) => (b.kind === "run" ? `run:${b.uses.map((u) => u.callId).join("")}` : b.item.kind))).toEqual([
      "run:ab",
      "text",
      "run:cd",
    ]);
  });

  it("leaves a lone call as its own card", () => {
    expect(groupTurnItems([tool("a", "Read"), text("t")]).map((b) => b.kind)).toEqual(["item", "item"]);
  });

  it("never folds agents, questions or plans, which stand on their own", () => {
    const blocks = groupTurnItems([tool("a", "Read"), tool("b", "Agent"), tool("c", "Read"), tool("d", "Grep")]);
    expect(blocks.map((b) => (b.kind === "run" ? "run" : b.item.kind))).toEqual(["tool", "tool", "run"]);
  });

  it("keys a run by its first call so it stays put while it grows", () => {
    const first = groupTurnItems([tool("a", "Read"), tool("b", "Read")]);
    const later = groupTurnItems([tool("a", "Read"), tool("b", "Read"), tool("c", "Bash")]);
    expect(first[0].key).toBe(later[0].key);
  });
});

describe("summarizeRun", () => {
  it("describes the work in plain words, counting distinct files", () => {
    const summary = summarizeRun([
      call("Read", { file_path: "a.rs" }),
      call("Read", { file_path: "b.rs" }),
      call("Read", { file_path: "a.rs" }),
      call("Grep", { pattern: "x" }),
      call("Grep", { pattern: "y" }),
      call("Edit", { file_path: "a.rs" }),
      call("Bash", { command: "npm test" }),
    ]);
    expect(summary.parts).toEqual(["read 2 files", "searched 2×", "edited 1 file", "ran 1 command"]);
    expect(runLine(summary)).toBe("Read 2 files · searched 2× · edited 1 file · ran 1 command");
    expect(summary.state).toBe("ok");
  });

  it("names the step in progress while the run is live", () => {
    const summary = summarizeRun([call("Read", { file_path: "a.rs" }), call("Bash", { command: "cargo test" }, "running")]);
    expect(summary).toMatchObject({ state: "running", current: "Running cargo test" });
  });

  it("never hides a failure or a denial behind the fold", () => {
    const summary = summarizeRun([
      call("Bash", { command: "a" }, "error"),
      call("Edit", { file_path: "x" }, "denied"),
      call("Read", { file_path: "y" }),
    ]);
    expect(summary).toMatchObject({ state: "failed", failed: 1, denied: 1 });
  });

  it("groups MCP calls by server and keeps other tools generic", () => {
    const summary = summarizeRun([call("mcp__github__list_prs"), call("mcp__github__get_pr"), call("Verify"), call("TodoWrite")]);
    expect(summary.parts).toEqual(["used github 2×", "used 1 other tool", "updated the plan"]);
  });
});
