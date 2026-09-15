import type { TaskReport } from "./taskReport";

export type TaskReportView = "quiet" | "compact" | "detailed";

export interface TaskReportPresentation {
  mode: TaskReportView;
  details: "disclosure" | "inline";
  showCheckCount: boolean;
  summary: string | null;
}

function taskReportView(config: unknown): TaskReportView {
  if (!config || typeof config !== "object" || !("taskReportView" in config)) return "quiet";
  const value = config.taskReportView;
  return value === "compact" || value === "detailed" || value === "quiet" ? value : "quiet";
}

export function projectTaskReportPresentation(
  config: unknown,
  report: TaskReport | undefined,
): TaskReportPresentation {
  const mode = taskReportView(config);
  if (mode === "compact") {
    return {
      mode,
      details: "disclosure",
      showCheckCount: true,
      summary: report?.blockers[0] ?? report?.assessment?.summary ?? null,
    };
  }
  if (mode === "detailed") {
    return { mode, details: "inline", showCheckCount: true, summary: null };
  }
  return { mode, details: "disclosure", showCheckCount: false, summary: null };
}
