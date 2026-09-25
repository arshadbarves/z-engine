import { describe, expect, it } from "vitest";
import { contextBubble, islandLine, noticeOpensIsland, recentSteps } from "./island";
import type { LiveStatus } from "./liveStatus";
import type { ToolCallView } from "./sessionView/types";

function status(over: Partial<LiveStatus> = {}): LiveStatus {
  return {
    kind: "idle",
    tone: "quiet",
    activity: null,
    text: null,
    detail: null,
    progress: null,
    elapsed: null,
    cost: null,
    noticeId: null,
    waiting: null,
    moreWaiting: 0,
    helpers: { running: 0, pending: 0 },
    ...over,
  };
}

function tool(over: Partial<ToolCallView> = {}): ToolCallView {
  return {
    callId: "c1",
    agentId: "main",
    tool: "Read",
    title: "Read a.rs",
    input: { file_path: "src/a.rs" },
    status: "ok",
    summary: "",
    output: "",
    progress: "",
    durationMs: 10,
    startedAt: 1,
    ...over,
  };
}

describe("islandLine", () => {
  it("names the chat while idle and says nothing without one", () => {
    expect(islandLine(status(), "Fix login")).toEqual({ text: "Fix login", metric: null });
    expect(islandLine(status(), null)).toEqual({ text: null, metric: null });
  });

  it("keeps one number: the clock while working, the duration when done", () => {
    const working = status({ kind: "working", text: "Reading a.rs", elapsed: "8s", cost: "$0.01", detail: "x" });
    expect(islandLine(working, "Fix login")).toEqual({ text: "Reading a.rs", metric: "8s" });
    const done = status({ kind: "done", text: "Verified", elapsed: "1m 02s", cost: "$0.12" });
    expect(islandLine(done, null)).toEqual({ text: "Verified", metric: "1m 02s" });
  });

  it("counts down a retry and shows notices without a number", () => {
    expect(islandLine(status({ kind: "retrying", text: "Provider busy", detail: "retry 2 in 3s" }), null).metric).toBe(
      "retry 2 in 3s",
    );
    expect(islandLine(status({ kind: "notice", text: "Saved", detail: "x", elapsed: "1s" }), null)).toEqual({
      text: "Saved",
      metric: null,
    });
  });
});

describe("noticeOpensIsland", () => {
  it("opens for text that does not fit, errors and notices with actions", () => {
    expect(noticeOpensIsland({ tone: "info" }, true)).toBe(true);
    expect(noticeOpensIsland({ tone: "error" }, false)).toBe(true);
    expect(noticeOpensIsland({ tone: "ok", actions: [{ label: "Undo" }] }, false)).toBe(true);
  });

  it("stays compact for a short routine notice or none", () => {
    expect(noticeOpensIsland({ tone: "ok" }, false)).toBe(false);
    expect(noticeOpensIsland({ tone: "warn", actions: [] }, false)).toBe(false);
    expect(noticeOpensIsland(null, true)).toBe(false);
  });
});

describe("contextBubble", () => {
  it("shows the number only once the context is filling up", () => {
    expect(contextBubble({ pct: 42, level: "ok" })).toEqual({ tone: "quiet", label: null });
    expect(contextBubble({ pct: 70, level: "warn" })).toEqual({ tone: "warn", label: "70%" });
    expect(contextBubble({ pct: 91, level: "danger" })).toEqual({ tone: "danger", label: "91%" });
  });
});

describe("recentSteps", () => {
  it("lists the main agent's latest steps, newest first", () => {
    const tools = {
      a: tool({ callId: "a", startedAt: 1 }),
      b: tool({ callId: "b", tool: "Bash", input: { command: "npm test" }, status: "running", startedAt: 3 }),
      c: tool({ callId: "c", agentId: "agt_1", startedAt: 4 }),
      d: tool({ callId: "d", status: "error", startedAt: 2 }),
    };
    const steps = recentSteps(tools, 0, 3);
    expect(steps.map((s) => [s.callId, s.state])).toEqual([
      ["b", "running"],
      ["d", "failed"],
      ["a", "done"],
    ]);
    expect(steps[0].label).toBe("Running npm test");
  });

  it("only counts steps of the current turn", () => {
    expect(recentSteps({ a: tool({ startedAt: 1 }) }, 5, 3)).toEqual([]);
  });
});
