import { describe, expect, it } from "vitest";
import type { TaskUpdatedEvent, TaskUpdatedReplayEvent } from "../types";
import {
  displayedTaskStatus, evidenceFreshness, replayTaskReport, supervisionActionLabels, taskStatusLabels,
} from "./taskReport";
import { taskReportFixture } from "./taskReportFixture";
import { isTaskReport } from "./taskReportGuard";

describe("S1 task report serde contract", () => {
  it("accepts the same camelCase report under live and snake_case journal tags", () => {
    const report = taskReportFixture();
    const live: TaskUpdatedEvent = { type: "taskUpdated", report, sessionId: "session-1" };
    const journal: TaskUpdatedReplayEvent = { type: "task_updated", report };
    const decodedLive = JSON.parse(JSON.stringify(live));
    const decodedJournal = JSON.parse(JSON.stringify(journal));
    expect(isTaskReport(decodedLive.report)).toBe(true);
    expect(isTaskReport(decodedJournal.report)).toBe(true);
    expect(decodedLive.report).toEqual(decodedJournal.report);
    expect(decodedLive.report.checks[0]).toMatchObject({
      inputFingerprint: "sha256:source", startedAtMs: 1_780_000_000_000,
      durationMs: 420, exitCode: 0, testsRun: 3,
    });
  });

  it("accepts nullable Cargo build evidence and no assessment", () => {
    const report = taskReportFixture({ status: "needs_verification", assessment: null });
    report.checks[0] = {
      ...report.checks[0],
      spec: { kind: "cargo_build", package: null, filter: null },
      inputFingerprint: null, exitCode: null, testsRun: null, stdout: null, stderr: null,
      outcome: "blocked",
    };
    expect(isTaskReport(report)).toBe(true);
  });

  it.each([
    null, {}, { status: "complete" },
    { ...taskReportFixture(), schemaVersion: 2 },
    { ...taskReportFixture(), taskId: "" },
    { ...taskReportFixture(), status: "done" },
    { ...taskReportFixture(), changedPaths: [1] },
    { ...taskReportFixture(), requirements: [{ id: "r1" }] },
    { ...taskReportFixture(), assessment: { summary: "Done", coverage: [null] } },
    { ...taskReportFixture(), checks: [{ ...taskReportFixture().checks[0], stdout: {} }] },
    { ...taskReportFixture(), checks: [{ ...taskReportFixture().checks[0], exitCode: "0" }] },
    { ...taskReportFixture(), checks: [{ ...taskReportFixture().checks[0], testsRun: -1 }] },
    { ...taskReportFixture(), checks: [{ ...taskReportFixture().checks[0], durationMs: Infinity }] },
    { ...taskReportFixture(), checks: [{ ...taskReportFixture().checks[0], outcome: "complete" }] },
    { ...taskReportFixture(), checks: [{ ...taskReportFixture().checks[0], spec: { kind: "shell" } }] },
  ])("rejects malformed and unsupported reports (%#)", (value) => {
    expect(isTaskReport(value)).toBe(false);
  });

  it("rejects snake_case report fields rather than guessing a contract", () => {
    const { schemaVersion, taskId, ...rest } = taskReportFixture();
    expect(isTaskReport({ ...rest, schema_version: schemaVersion, task_id: taskId })).toBe(false);
  });

  it("rejects duplicate evidence IDs that would make references ambiguous", () => {
    const report = taskReportFixture();
    report.checks.push({ ...report.checks[0] });
    expect(isTaskReport(report)).toBe(false);
  });
});

describe("presentation does not establish completion", () => {
  it("only labels the authoritative complete status Complete", () => {
    expect(Object.entries(taskStatusLabels).filter(([, label]) => label === "Complete"))
      .toEqual([["complete", "Complete"]]);
  });

  describe("bounded supervision report contract", () => {
    const supervision = {
      continuations: 1, maxContinuations: 3, lastAction: "verify", reason: "Missing current checks.",
    } as const;

    it("accepts legacy reports without supervision", () => {
      const legacy = taskReportFixture();
      expect(legacy).not.toHaveProperty("supervision");
      expect(isTaskReport(legacy)).toBe(true);
    });

    it.each([
      "continue", "verify", "repair", "complete", "blocked", "idle", "stopped", "interrupted",
    ] as const)("accepts %s under live and journal tags", (lastAction) => {
      const report = taskReportFixture({ supervision: { ...supervision, lastAction } });
      for (const type of ["taskUpdated", "task_updated"]) {
        const decoded = JSON.parse(JSON.stringify({ type, report }));
        expect(isTaskReport(decoded.report)).toBe(true);
        expect(decoded.report.supervision).toEqual({ ...supervision, lastAction });
      }
      expect(replayTaskReport(report).supervision).toEqual(report.supervision);
    });

    it.each([0, 3, 10])("accepts %i as a continuation bound", (maxContinuations) => {
      expect(isTaskReport(taskReportFixture({
        supervision: { ...supervision, continuations: maxContinuations, maxContinuations },
      }))).toBe(true);
    });

    it.each([
      null, undefined, [], {}, "verify",
      { ...supervision, continuations: -1 },
      { ...supervision, continuations: 1.5 },
      { ...supervision, continuations: "1" },
      { ...supervision, continuations: 4 },
      { ...supervision, continuations: Infinity },
      { ...supervision, continuations: NaN },
      { ...supervision, maxContinuations: -1 },
      { ...supervision, maxContinuations: 3.5 },
      { ...supervision, maxContinuations: "3" },
      { ...supervision, maxContinuations: 11 },
      { ...supervision, maxContinuations: Infinity },
      { ...supervision, lastAction: "done" },
      { ...supervision, lastAction: null },
      { ...supervision, reason: null },
      { ...supervision, reason: 42 },
      { continuations: 1, maxContinuations: 3, reason: "No action" },
      { continuations: 1, lastAction: "verify", reason: "No bound" },
      { maxContinuations: 3, lastAction: "verify", reason: "No count" },
      { continuations: 1, maxContinuations: 3, lastAction: "verify" },
      { continuations: 1, max_continuations: 3, last_action: "verify", reason: "Wrong casing" },
    ])("rejects malformed supervision (%#)", (value) => {
      expect(isTaskReport({ ...taskReportFixture(), supervision: value })).toBe(false);
    });

    it("does not change the authoritative status based on the last action", () => {
      const report = taskReportFixture({
        status: "running", supervision: { ...supervision, lastAction: "complete" },
      });
      expect(isTaskReport(report)).toBe(true);
      expect(displayedTaskStatus(report)).toBe("running");
      expect(supervisionActionLabels.complete).toBe("Finalization requested");
      expect(Object.values(supervisionActionLabels)).not.toContain("Complete");
      const replayed = replayTaskReport(report);
      expect(replayed.supervision).toEqual(report.supervision);
      expect(report.status).toBe("running");
    });
  });

  it("never presents an unvalidated parked completion as Complete", () => {
    expect(displayedTaskStatus(taskReportFixture(), true)).toBe("unassessed");
    expect(displayedTaskStatus(taskReportFixture(), false)).toBe("complete");
    expect(displayedTaskStatus(taskReportFixture({ status: "stale" }), true)).toBe("stale");
  });

  it("projects running reports as interrupted on replay without mutating the payload", () => {
    const report = taskReportFixture({ status: "running" });
    expect(replayTaskReport(report).status).toBe("interrupted");
    expect(report.status).toBe("running");
    const complete = taskReportFixture();
    expect(replayTaskReport(complete)).toBe(complete);
  });

  it("does not claim freshness from a fingerprint or a successful exit", () => {
    const check = taskReportFixture().checks[0];
    expect(evidenceFreshness(check, "complete")).toContain("not rechecked");
    expect(evidenceFreshness(check, "stale")).toContain("Stale");
    expect(evidenceFreshness({ ...check, outcome: "stale" }, "needs_verification"))
      .toContain("Stale");
    expect(evidenceFreshness({ ...check, inputFingerprint: null }, "needs_verification"))
      .toContain("Unknown");
  });
});
