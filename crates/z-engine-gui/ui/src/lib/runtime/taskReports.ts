import { replayTaskReport } from "../domain/taskReport";
import { isTaskReport, taskReportId } from "../domain/taskReportGuard";
import { closeThinking, endAssistant, push, update } from "./mutations";
import { rt } from "./state";

function currentUser() {
  return rt.messages.findLast((message) => message.kind === "user");
}

/** A legacy turn has no authoritative task ID, evidence, or completion status. */
export function ensureUnassessedTask() {
  const user = currentUser();
  if (!user || rt.messages.some((m) => m.kind === "task" && m.runTurn === user.runTurn)) return;
  push("task", user.text, { runTurn: user.runTurn });
}

export function applyTaskReport(value: unknown, replay = false) {
  const taskId = taskReportId(value);
  const existing = taskId
    ? rt.messages.find((m) => m.kind === "task" && m.taskId === taskId)
    : undefined;
  const user = currentUser();
  const placeholder = rt.messages.find(
    (m) => m.kind === "task" && !m.taskId && m.runTurn === user?.runTurn,
  );
  const target = existing ?? placeholder;
  const valid = isTaskReport(value);
  const report = valid ? (replay ? replayTaskReport(value) : value) : undefined;
  if (!replay && report?.status === "running" && report.supervision &&
      report.supervision.continuations > (existing?.taskReport?.supervision?.continuations ?? 0)) {
    closeThinking();
    endAssistant();
  }
  const fields = {
    taskId,
    taskReport: report,
    taskFreshnessPending: false,
    taskReportError: valid ? undefined : "Invalid or unsupported task report. Completion is unassessed.",
    text: report?.goal ?? target?.text ?? user?.text ?? "Task report",
    runTurn: target?.runTurn ?? user?.runTurn,
  };
  if (target) update(target.id, fields);
  else push("task", fields.text, fields);
}
