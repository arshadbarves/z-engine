import { beforeEach, describe, expect, it } from "vitest";
import { groupTurns } from "../activity";
import { taskReportFixture } from "../domain/taskReportFixture";
import type { TaskUpdatedEvent } from "../types";
import {
  busyStore, handleEvent, replaySession, resetForTests, resetTranscript,
  setBusy, submitLocal, transcriptStore, trimTranscript,
} from "../events";

const messages = () => transcriptStore.getSnapshot();
const tasks = () => messages().filter((m) => m.kind === "task");
beforeEach(() => resetForTests());

describe("task evidence and response lifecycle", () => {
  it("finishes streaming without inferring task completion", () => {
    submitLocal("fix tests");
    setBusy(true);
    handleEvent({ type: "taskUpdated", report: taskReportFixture({ status: "needs_verification" }) });
    handleEvent({ type: "tokenDelta", text: "All done!" });
    handleEvent({ type: "turnCompleted", promptTokens: 10, completionTokens: 2 });
    expect(busyStore.getSnapshot()).toBe(false);
    expect(messages().find((m) => m.kind === "assistant")?.streaming).toBe(false);
    expect(tasks()[0].taskReport?.status).toBe("needs_verification");
    expect(messages().find((m) => m.kind === "status"))
      .toMatchObject({ text: expect.stringContaining("Response finished") });
    expect(messages().find((m) => m.kind === "status")?.ok).toBeUndefined();
    expect(groupTurns(messages()).map((b) => b.type)).toContain("task");
    expect(groupTurns(messages()).map((b) => b.type)).toContain("status");
  });

  it("shows unassessed rather than success when a response has no report", () => {
    submitLocal("legacy");
    handleEvent({ type: "turnCompleted" });
    expect(tasks()).toHaveLength(1);
    expect(tasks()[0].taskReport).toBeUndefined();
  });

  it("replaces one task card in place and preserves detailed evidence", () => {
    submitLocal("fix tests");
    handleEvent({ type: "taskUpdated", report: taskReportFixture({ status: "running" }) });
    const id = tasks()[0].id;
    const update: TaskUpdatedEvent = { type: "taskUpdated", report: taskReportFixture() };
    handleEvent(update);
    handleEvent(update);
    expect(tasks()).toHaveLength(1);
    expect(tasks()[0]).toMatchObject({ id, taskReport: taskReportFixture() });
  });

  it.each(["failed", "blocked", "stale", "cancelled"] as const)(
    "does not turn %s evidence into completion at response end", (outcome) => {
      const report = taskReportFixture({ status: outcome === "stale" ? "stale" : "blocked" });
      report.checks[0] = { ...report.checks[0], outcome, exitCode: outcome === "failed" ? 1 : null };
      submitLocal("fix");
      handleEvent({ type: "taskUpdated", report });
      handleEvent({ type: "turnCompleted" });
      expect(tasks()[0].taskReport).toEqual(report);
      expect(tasks()[0].taskReport?.status).not.toBe("complete");
    },
  );

  it("invalidates an existing complete card when its next report is malformed", () => {
    submitLocal("fix");
    handleEvent({ type: "taskUpdated", report: taskReportFixture() });
    handleEvent({ type: "taskUpdated", report: { taskId: "task-1", status: "complete" } });
    expect(tasks()).toHaveLength(1);
    expect(tasks()[0].taskReport).toBeUndefined();
    expect(tasks()[0].taskReportError).toContain("unassessed");
    handleEvent({ type: "taskUpdated", report: taskReportFixture({ status: "blocked" }) });
    expect(tasks()).toHaveLength(1);
    expect(tasks()[0].taskReportError).toBeUndefined();
  });

  it("does not locally stop or complete a report on abort", () => {
    submitLocal("fix");
    handleEvent({ type: "taskUpdated", report: taskReportFixture({ status: "running" }) });
    handleEvent({ type: "turnAborted" });
    expect(tasks()[0].taskReport?.status).toBe("running");
    handleEvent({ type: "taskUpdated", report: taskReportFixture({ status: "stopped" }) });
    expect(tasks()[0].taskReport?.status).toBe("stopped");
  });
});

describe("journal task projection", () => {
  it("honors the last report for each task without repeating cards", () => {
    replaySession([
      { type: "user_msg", text: "first" },
      { type: "task_updated", report: taskReportFixture({ status: "running" }) },
      { type: "assistant_msg", content: "First answer" },
      { type: "task_updated", report: taskReportFixture() },
      { type: "user_msg", text: "second" },
      { type: "task_updated", report: taskReportFixture({ taskId: "task-2", status: "running" }) },
      { type: "task_updated", report: taskReportFixture({ status: "stale" }) },
    ]);
    expect(tasks()).toHaveLength(2);
    expect(tasks().map((m) => [m.runTurn, m.taskReport?.status]))
      .toEqual([[0, "stale"], [1, "interrupted"]]);
  });

  it("leaves every reportless legacy user turn unassessed", () => {
    replaySession([
      { type: "user_msg", text: "first" },
      { type: "assistant_msg", content: "Done and verified!" },
      { type: "user_msg", text: "second" },
      { type: "assistant_msg", content: "All done." },
    ]);
    expect(tasks().map((m) => [m.runTurn, m.text, m.taskReport]))
      .toEqual([[0, "first", undefined], [1, "second", undefined]]);
  });

  it("keeps mixed old and new tasks distinct", () => {
    replaySession([
      { type: "user_msg", text: "legacy" },
      { type: "user_msg", text: "new" },
      { type: "task_updated", report: taskReportFixture() },
    ]);
    expect(tasks()).toHaveLength(2);
    expect(tasks()[0].taskReport).toBeUndefined();
    expect(tasks()[1].taskReport?.status).toBe("complete");
  });

  it("fails closed for malformed persisted reports", () => {
    replaySession([
      { type: "user_msg", text: "bad report" },
      { type: "task_updated", report: { ...taskReportFixture(), schemaVersion: 99 } },
    ]);
    expect(tasks()).toHaveLength(1);
    expect(tasks()[0].taskReport).toBeUndefined();
    expect(tasks()[0].taskReportError).toContain("Invalid or unsupported");
  });

  it("trims report cards with their original task even after late updates", () => {
    replaySession([
      { type: "user_msg", text: "keep" },
      { type: "task_updated", report: taskReportFixture() },
      { type: "user_msg", text: "remove" },
      { type: "task_updated", report: taskReportFixture({ taskId: "task-2" }) },
      { type: "task_updated", report: taskReportFixture({ status: "stale" }) },
    ]);
    trimTranscript(1);
    expect(tasks()).toHaveLength(1);
    expect(tasks()[0].taskReport?.status).toBe("stale");
    expect(tasks()[0].runTurn).toBe(0);
    resetTranscript();
    expect(tasks()).toHaveLength(0);
  });
});
