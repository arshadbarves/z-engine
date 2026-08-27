import { describe, expect, it, beforeEach } from "vitest";
import {
  activateSession,
  busyStore,
  handleEvent,
  hasSessionRuntime,
  queueStore,
  resetForTests,
  resolveApproval,
  sessionActivityStore,
  setBusy,
  submitLocal,
  transcriptStore,
} from "./events";

beforeEach(() => resetForTests());

function approvalEvent(id: number) {
  return {
    type: "approvalRequired",
    id,
    tool: "bash",
    inputPreview: "npm test",
    suggestedRule: null,
    detailPreview: null,
    canPersist: false,
    bashCommand: "npm test",
  };
}

describe("background sessions", () => {
  it("keeps the previous chat generating after switching away", () => {
    handleEvent({ type: "sessionChanged", ulid: "AAA" });
    submitLocal("fix tests");
    setBusy(true);
    handleEvent({ type: "tokenDelta", text: "working on it" });

    activateSession("BBB");
    expect(transcriptStore.getSnapshot()).toHaveLength(0);
    expect(busyStore.getSnapshot()).toBe(false);
    expect(hasSessionRuntime("AAA")).toBe(true);
    expect(sessionActivityStore.getSnapshot()["AAA"]).toBe("working");

    handleEvent({ type: "tokenDelta", text: " still", sessionId: "AAA" });
    expect(transcriptStore.getSnapshot()).toHaveLength(0);

    activateSession("AAA");
    const text = transcriptStore.getSnapshot().map((m) => m.text).join(" ");
    expect(text).toContain("working on it still");
    expect(busyStore.getSnapshot()).toBe(true);
  });

  it("parks the follow-up queue with its session", () => {
    handleEvent({ type: "sessionChanged", ulid: "AAA" });
    setBusy(true);
    queueStore.push("follow up");
    activateSession("BBB");
    expect(queueStore.getSnapshot()).toHaveLength(0);
    activateSession("AAA");
    expect(queueStore.getSnapshot()[0]?.text).toBe("follow up");
  });

  it("does not wipe a parked session when a background turn finishes", () => {
    handleEvent({ type: "sessionChanged", ulid: "AAA" });
    submitLocal("fix tests");
    setBusy(true);
    activateSession("BBB");
    handleEvent({ type: "turnCompleted", sessionId: "AAA", promptTokens: 1, completionTokens: 1 });
    expect(sessionActivityStore.getSnapshot()["AAA"]).toBeUndefined();
    activateSession("AAA");
    expect(transcriptStore.getSnapshot().some((m) => m.kind === "user")).toBe(true);
    expect(transcriptStore.getSnapshot().some((m) => m.kind === "status")).toBe(true);
    expect(busyStore.getSnapshot()).toBe(false);
  });

  it("returns a stable working-ids snapshot so React does not loop", () => {
    const a = sessionActivityStore.getSnapshot();
    const b = sessionActivityStore.getSnapshot();
    expect(a).toBe(b);
    handleEvent({ type: "sessionChanged", ulid: "AAA" });
    setBusy(true);
    const c = sessionActivityStore.getSnapshot();
    const d = sessionActivityStore.getSnapshot();
    expect(c).toBe(d);
    expect(c).not.toBe(a);
    expect(c["AAA"]).toBe("working");
  });

  it("keeps the indicator on as approval-pending instead of going dark", () => {
    handleEvent({ type: "sessionChanged", ulid: "AAA" });
    submitLocal("run tests");
    setBusy(true);
    handleEvent(approvalEvent(1));
    expect(busyStore.getSnapshot()).toBe(false);
    expect(sessionActivityStore.getSnapshot()["AAA"]).toBe("approval");
  });

  it("flips back to working once the last approval is decided", () => {
    handleEvent({ type: "sessionChanged", ulid: "AAA" });
    submitLocal("run tests");
    setBusy(true);
    handleEvent(approvalEvent(1));
    resolveApproval(1, "once");
    expect(busyStore.getSnapshot()).toBe(true);
    expect(sessionActivityStore.getSnapshot()["AAA"]).toBe("working");
    handleEvent({ type: "turnCompleted", promptTokens: 1, completionTokens: 1 });
    expect(sessionActivityStore.getSnapshot()["AAA"]).toBeUndefined();
  });

  it("marks a parked session awaiting approval as approval, not idle", () => {
    handleEvent({ type: "sessionChanged", ulid: "AAA" });
    submitLocal("run tests");
    setBusy(true);
    handleEvent(approvalEvent(1));
    activateSession("BBB");
    expect(sessionActivityStore.getSnapshot()["AAA"]).toBe("approval");
  });

  it("expires a stale approval card when the turn aborts", () => {
    handleEvent({ type: "sessionChanged", ulid: "AAA" });
    setBusy(true);
    handleEvent(approvalEvent(1));
    handleEvent({ type: "turnAborted" });
    expect(sessionActivityStore.getSnapshot()["AAA"]).toBeUndefined();
    const kinds = transcriptStore.getSnapshot().map((m) => m.kind);
    expect(kinds).not.toContain("approval");
    expect(kinds).toContain("notice");
  });
});
