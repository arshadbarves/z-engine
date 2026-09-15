import { beforeEach, describe, expect, it } from "vitest";
import { taskReportFixture } from "../domain/taskReportFixture";
import {
  busyStore, handleEvent, replaySession, resetForTests, setBusy,
  submitLocal, transcriptStore,
} from "../events";

beforeEach(() => resetForTests());

const continuing = () => taskReportFixture({
  status: "running",
  supervision: {
    continuations: 1,
    maxContinuations: 3,
    lastAction: "verify",
    reason: "Current verification is missing.",
  },
});

describe("supervised response boundaries", () => {
  it("starts a separate assistant response without clearing task busy state", () => {
    submitLocal("fix the bug");
    setBusy(true);
    handleEvent({ type: "taskUpdated", report: taskReportFixture({ status: "running" }) });
    handleEvent({ type: "tokenDelta", text: "Initial answer" });
    handleEvent({ type: "taskUpdated", report: continuing() });
    expect(busyStore.getSnapshot()).toBe(true);
    handleEvent({ type: "tokenDelta", text: "Continuing with verification" });
    const assistants = transcriptStore.getSnapshot().filter((message) => message.kind === "assistant");
    expect(assistants.map((message) => message.text))
      .toEqual(["Initial answer", "Continuing with verification"]);
    expect(assistants[0].streaming).toBe(false);
    expect(assistants[1].streaming).toBe(true);
    expect(transcriptStore.getSnapshot().filter((message) => message.kind === "task")).toHaveLength(1);
  });

  it("does not break streaming on duplicate continuation projections", () => {
    submitLocal("fix the bug");
    handleEvent({ type: "taskUpdated", report: continuing() });
    handleEvent({ type: "tokenDelta", text: "One " });
    handleEvent({ type: "taskUpdated", report: continuing() });
    handleEvent({ type: "tokenDelta", text: "response" });
    expect(transcriptStore.getSnapshot().filter((message) => message.kind === "assistant").map((message) => message.text))
      .toEqual(["One response"]);
  });

  it("replays supervision as history rather than restarting execution", () => {
    replaySession([
      { type: "user_msg", text: "fix the bug" },
      { type: "assistant_msg", content: "Initial answer" },
      { type: "task_updated", report: continuing() },
      { type: "assistant_msg", content: "Continuing with verification" },
    ]);
    const tasks = transcriptStore.getSnapshot().filter((message) => message.kind === "task");
    expect(tasks).toHaveLength(1);
    expect(tasks[0].taskReport?.status).toBe("interrupted");
    expect(tasks[0].taskReport?.supervision?.continuations).toBe(1);
    expect(busyStore.getSnapshot()).toBe(false);
  });
});
