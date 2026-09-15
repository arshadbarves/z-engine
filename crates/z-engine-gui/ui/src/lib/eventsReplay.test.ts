import { beforeEach, describe, expect, it } from "vitest";
import {
  beginHydrate, endHydrate, handleEvent, replaySession, resetForTests,
  resetTranscript, sessionStore, submitLocal, toastStore, transcriptStore,
} from "./events";

const msgs = () => transcriptStore.getSnapshot();
beforeEach(() => resetForTests());

describe("session replay", () => {
  // Shapes mirror the snake_case tags emitted by SessionEvent's serde contract.
  it("rebuilds transcript cards and an unassessed task from legacy session events", () => {
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
    expect(msgs().map((m) => m.kind)).toEqual(["user", "assistant", "tool", "task"]);
    const tool = msgs()[2];
    expect(tool.toolName).toBe("read_file");
    expect(tool.preview).toBe("Cargo.toml");
    expect(tool.streaming).toBe(false);
    expect(tool.summary).toContain("line");
    expect(tool.output).toContain("line");
    expect(msgs()[0].runTurn).toBe(0);
    expect(msgs()[3].taskReport).toBeUndefined();
  });

  it("assigns sequential runTurn so restored prompts can be edited", () => {
    replaySession([
      { type: "user_msg", text: "one" },
      { type: "assistant_msg", content: "ok" },
      { type: "user_msg", text: "two" },
    ]);
    const users = msgs().filter((m) => m.kind === "user");
    expect(users.map((m) => m.runTurn)).toEqual([0, 1]);
  });

  it("regression: camelCase tags match nothing (serde emits snake_case)", () => {
    replaySession([{ type: "user_msg", text: "real shape" }]);
    expect(msgs().filter((m) => m.kind === "user").map((m) => m.text)).toEqual(["real shape"]);
    replaySession([{ type: "userMsg", text: "legacy shape" }]);
    expect(msgs()).toHaveLength(0);
  });

  it("resetTranscript clears everything", () => {
    submitLocal("hi");
    resetTranscript();
    expect(msgs()).toHaveLength(0);
  });
});

describe("session hydrate lock", () => {
  it("swallows turnAborted so a swap cannot toast over the restored chat", () => {
    replaySession([{ type: "user_msg", text: "kept" }]);
    const before = msgs();
    beginHydrate();
    handleEvent({ type: "turnAborted" });
    handleEvent({ type: "tokenDelta", text: "stale" });
    expect(msgs()).toEqual(before);
    expect(toastStore.getSnapshot().some((t) => t.text.toLowerCase().includes("abort"))).toBe(false);
    endHydrate();
  });

  it("still records sessionChanged while locked", () => {
    beginHydrate();
    handleEvent({ type: "sessionChanged", ulid: "01NEW" });
    expect(sessionStore.getSnapshot()).toBe("01NEW");
    endHydrate();
  });

  it("replays attached images on user_msg", () => {
    replaySession([{ type: "user_msg", text: "see this", images: ["data:image/png;base64,xx"] }]);
    expect(msgs()[0].images).toEqual(["data:image/png;base64,xx"]);
  });
});
