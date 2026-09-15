import type { CheckEvidence, EvidenceArtifact, SupervisionReport, TaskReport } from "./taskReport";

function object(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function string(value: unknown): value is string {
  return typeof value === "string";
}

function strings(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(string);
}

function nullableString(value: unknown): value is string | null {
  return value === null || string(value);
}

function unsigned(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function artifact(value: unknown): value is EvidenceArtifact | null {
  return value === null || (object(value) && string(value.path) && string(value.digest));
}

function check(value: unknown): value is CheckEvidence {
  return (
    object(value) &&
    string(value.id) && value.id.length > 0 &&
    object(value.spec) &&
    (value.spec.kind === "cargo_test" || value.spec.kind === "cargo_build") &&
    nullableString(value.spec.package) &&
    nullableString(value.spec.filter) &&
    strings(value.command) &&
    string(value.cwd) &&
    nullableString(value.inputFingerprint) &&
    string(value.toolchain) &&
    unsigned(value.startedAtMs) &&
    unsigned(value.durationMs) &&
    (value.exitCode === null ||
      (typeof value.exitCode === "number" && Number.isSafeInteger(value.exitCode))) &&
    (value.testsRun === null || unsigned(value.testsRun)) &&
    (value.outcome === "passed" || value.outcome === "failed" ||
      value.outcome === "blocked" || value.outcome === "cancelled" || value.outcome === "stale") &&
    string(value.summary) &&
    artifact(value.stdout) &&
    artifact(value.stderr)
  );
}

function requirement(value: unknown): value is TaskReport["requirements"][number] {
  return object(value) && string(value.id) && value.id.length > 0 && string(value.description);
}

function coverage(value: unknown): value is NonNullable<TaskReport["assessment"]>["coverage"][number] {
  return (
    object(value) &&
    string(value.requirementId) &&
    strings(value.evidenceIds) &&
    string(value.explanation)
  );
}

function uniqueIds(values: { id: string }[]): boolean {
  return new Set(values.map((value) => value.id)).size === values.length;
}

function supervision(value: unknown): value is SupervisionReport {
  return (
    object(value) &&
    unsigned(value.continuations) &&
    unsigned(value.maxContinuations) &&
    value.continuations <= value.maxContinuations &&
    value.maxContinuations <= 10 &&
    (value.lastAction === "continue" || value.lastAction === "verify" ||
      value.lastAction === "repair" || value.lastAction === "complete" ||
      value.lastAction === "blocked" || value.lastAction === "idle" ||
      value.lastAction === "stopped" || value.lastAction === "interrupted") &&
    string(value.reason)
  );
}

/** Validate at both live and journal boundaries; unsupported versions fail closed. */
export function isTaskReport(value: unknown): value is TaskReport {
  return (
    object(value) &&
    value.schemaVersion === 1 &&
    string(value.taskId) && value.taskId.length > 0 &&
    string(value.goal) &&
    string(value.workspaceRoot) &&
    (value.status === "running" || value.status === "needs_verification" ||
      value.status === "complete" || value.status === "blocked" || value.status === "stopped" ||
      value.status === "interrupted" || value.status === "unassessed" || value.status === "stale") &&
    Array.isArray(value.requirements) && value.requirements.every(requirement) &&
    uniqueIds(value.requirements) &&
    Array.isArray(value.checks) && value.checks.every(check) && uniqueIds(value.checks) &&
    (value.assessment === null ||
      (object(value.assessment) && string(value.assessment.summary) &&
        Array.isArray(value.assessment.coverage) && value.assessment.coverage.every(coverage))) &&
    strings(value.blockers) &&
    strings(value.changedPaths) &&
    (!("supervision" in value) || supervision(value.supervision))
  );
}

export function taskReportId(value: unknown): string | undefined {
  return object(value) && string(value.taskId) && value.taskId.length > 0
    ? value.taskId
    : undefined;
}
