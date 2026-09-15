/** Wire contract shared by taskUpdated events and task_updated journal records. */
export type TaskStatus =
  | "running"
  | "needs_verification"
  | "complete"
  | "blocked"
  | "stopped"
  | "interrupted"
  | "unassessed"
  | "stale";

export interface EvidenceArtifact {
  path: string;
  digest: string;
}

export interface CheckEvidence {
  id: string;
  spec: {
    kind: "cargo_test" | "cargo_build";
    package: string | null;
    filter: string | null;
  };
  command: string[];
  cwd: string;
  inputFingerprint: string | null;
  toolchain: string;
  startedAtMs: number;
  durationMs: number;
  exitCode: number | null;
  testsRun: number | null;
  outcome: "passed" | "failed" | "blocked" | "cancelled" | "stale";
  summary: string;
  stdout: EvidenceArtifact | null;
  stderr: EvidenceArtifact | null;
}

export interface SupervisionReport {
  continuations: number;
  maxContinuations: number;
  lastAction: "continue" | "verify" | "repair" | "complete" | "blocked" | "idle" | "stopped" | "interrupted";
  reason: string;
}

export const supervisionActionLabels: Record<SupervisionReport["lastAction"], string> = {
  continue: "Continue",
  verify: "Verify",
  repair: "Repair",
  complete: "Finalization requested",
  blocked: "Blocked",
  idle: "Idle",
  stopped: "Stopped",
  interrupted: "Interrupted",
};

export interface TaskReport {
  schemaVersion: number;
  taskId: string;
  goal: string;
  workspaceRoot: string;
  status: TaskStatus;
  requirements: { id: string; description: string }[];
  checks: CheckEvidence[];
  assessment: {
    summary: string;
    coverage: {
      requirementId: string;
      evidenceIds: string[];
      explanation: string;
    }[];
  } | null;
  blockers: string[];
  changedPaths: string[];
  supervision?: SupervisionReport;
}

export const taskStatusLabels: Record<TaskStatus, string> = {
  running: "Running",
  needs_verification: "Needs verification",
  complete: "Complete",
  blocked: "Blocked",
  stopped: "Stopped",
  interrupted: "Interrupted",
  unassessed: "Unassessed",
  stale: "Stale",
};

export const taskStatusDescriptions: Record<TaskStatus, string> = {
  running: "The task is running. Completion has not been verified.",
  needs_verification: "Completion has not been verified. Review the checks and requirement coverage.",
  complete: "The runtime verified task completion against the recorded evidence.",
  blocked: "The task is blocked. Review the blockers and check results.",
  stopped: "The task was stopped, not completed.",
  interrupted: "Execution was interrupted. No completed task was recorded.",
  unassessed: "No validated completion report was recorded for this request.",
  stale: "Recorded evidence is stale and does not verify the current workspace.",
};

/** Replay cannot revive an interrupted execution or infer that it completed. */
export function replayTaskReport(report: TaskReport): TaskReport {
  return report.status === "running" ? { ...report, status: "interrupted" } : report;
}

export function displayedTaskStatus(report: TaskReport | undefined, freshnessPending = false): TaskStatus {
  if (report?.status === "complete" && freshnessPending) return "unassessed";
  return report?.status ?? "unassessed";
}

export function evidenceFreshness(check: CheckEvidence, status: TaskStatus): string {
  if (check.outcome === "stale" || status === "stale") return "Stale — re-run required";
  if (check.inputFingerprint === null) return "Unknown — no input fingerprint recorded";
  return "Fingerprint recorded; current workspace freshness is not rechecked by this view";
}
