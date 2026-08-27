import { describe, expect, it, beforeEach } from "vitest";
import {
  approvalGateStore,
  beginHydrate,
  busyStore,
  draftStore,
  handleEvent,
  queueStore,
  resetForTests,
  resetTranscript,
  setBusy,
  transcriptStore,
} from "./events";

beforeEach(() => resetForTests());

describe("session swap while a turn is live", () => {
  it("resetTranscript drops busy, approvals, queue, and draft", () => {
    setBusy(true);
    draftStore.set("Fix the failing tests");
    queueStore.push("follow up");
    handleEvent({
      type: "approvalRequired",
      id: 7,
      tool: "bash",
      inputPreview: '{"command":"ls"}',
      suggestedRule: null,
      detailPreview: null,
      canPersist: false,
      bashCommand: "ls",
    });
    expect(approvalGateStore.getSnapshot()).toBe(1);
    expect(transcriptStore.getSnapshot().some((m) => m.kind === "approval")).toBe(true);

    setBusy(true);
    resetTranscript();

    expect(busyStore.getSnapshot()).toBe(false);
    expect(approvalGateStore.getSnapshot()).toBe(0);
    expect(transcriptStore.getSnapshot()).toHaveLength(0);
    expect(queueStore.getSnapshot()).toHaveLength(0);
    expect(draftStore.getSnapshot()).toBe("");
  });

  it("hydrate lock plus reset does not leave a working turn after abort", () => {
    setBusy(true);
    handleEvent({
      type: "approvalRequired",
      id: 8,
      tool: "bash",
      inputPreview: "printf dummy",
      canPersist: true,
      bashCommand: "printf dummy",
    });
    beginHydrate();
    resetTranscript();
    handleEvent({ type: "turnAborted" });

    expect(busyStore.getSnapshot()).toBe(false);
    expect(approvalGateStore.getSnapshot()).toBe(0);
    expect(transcriptStore.getSnapshot()).toHaveLength(0);
  });
});
