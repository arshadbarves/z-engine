import type { TaskReport } from "./taskReport";

/** Serde-shaped S1 payload: event tags differ, report fields do not. */
export function taskReportFixture(overrides: Partial<TaskReport> = {}): TaskReport {
  return {
    schemaVersion: 1,
    taskId: "task-1",
    goal: "Fix the parser and verify its regression test",
    workspaceRoot: "/workspace/project",
    status: "complete",
    requirements: [{ id: "req-1", description: "The parser regression is covered" }],
    checks: [{
      id: "check-1",
      spec: { kind: "cargo_test", package: "z-engine-core", filter: "parser" },
      command: ["cargo", "test", "-p", "z-engine-core", "parser"],
      cwd: "/workspace/project",
      inputFingerprint: "sha256:source",
      toolchain: "cargo 1.89.0",
      startedAtMs: 1_780_000_000_000,
      durationMs: 420,
      exitCode: 0,
      testsRun: 3,
      outcome: "passed",
      summary: "3 tests passed",
      stdout: { path: "/workspace/evidence/check-1.stdout", digest: "sha256:stdout" },
      stderr: { path: "/workspace/evidence/check-1.stderr", digest: "sha256:stderr" },
    }],
    assessment: {
      summary: "The regression was verified",
      coverage: [{
        requirementId: "req-1",
        evidenceIds: ["check-1"],
        explanation: "The parser regression suite passed against the recorded inputs",
      }],
    },
    blockers: [],
    changedPaths: ["src/parser.rs"],
    ...overrides,
  };
}
