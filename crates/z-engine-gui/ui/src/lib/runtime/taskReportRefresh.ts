import type { Msg } from "../types";
import { taskReportId } from "../domain/taskReportGuard";
import { parkedEntries } from "../sessionSnaps";
import { emitChange, rt } from "./state";
import { applyTaskReport } from "./taskReports";

export type ReportSnapshot = Map<string, Msg>;

/** A parked completion is historical evidence until the backend rechecks it. */
export function markTaskReportsPending(): ReportSnapshot {
  const reports: ReportSnapshot = new Map();
  rt.messages = rt.messages.map((message) => {
    if (message.kind !== "task" || !message.taskId) return message;
    const pending = message.taskReport?.status === "complete"
      ? { ...message, taskFreshnessPending: true }
      : message;
    reports.set(message.taskId, pending);
    return pending;
  });
  emitChange(parkedEntries());
  return reports;
}

/** Merge only the last durable report, without rewinding a newer live event. */
export function mergeRefreshedTaskReports(events: unknown[], expected: ReportSnapshot) {
  const latest = new Map<string, unknown>();
  for (const event of events) {
    if (typeof event !== "object" || event === null || !("type" in event) ||
      event.type !== "task_updated" || !("report" in event)) continue;
    const id = taskReportId(event.report);
    if (id) latest.set(id, event.report);
  }
  for (const [id, prior] of expected) {
    const current = rt.messages.find((message) => message.kind === "task" && message.taskId === id);
    if (!current || current !== prior || !latest.has(id)) continue;
    // A disk projection cannot interrupt an execution that is still live.
    if (prior.taskReport?.status === "running" || prior.taskReport?.status === "needs_verification") continue;
    applyTaskReport(latest.get(id));
  }
}
