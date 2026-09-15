import type { HarnessConfig, TaskReportView } from "../commands";

export interface AppearanceOption {
  value: TaskReportView;
  label: string;
  description: string;
}

export interface TaskReportViewUpdate {
  previous: HarnessConfig;
  optimistic: HarnessConfig;
}

export type CompensationOutcome = "not-needed" | "succeeded" | "failed";

export interface TaskReportViewRecovery {
  initialSavePersisted: boolean;
  compensation: CompensationOutcome;
  reconciled: HarnessConfig | null;
}

export interface TaskReportViewRecoveryDecision {
  config: HarnessConfig;
  durabilityConfirmed: boolean;
  restoredPrevious: boolean;
}

export const APPEARANCE_OPTIONS: readonly AppearanceOption[] = [
  {
    value: "quiet",
    label: "Quiet",
    description: "Shows the final result, with verification tucked away until you open it.",
  },
  {
    value: "compact",
    label: "Compact",
    description: "Adds a short outcome and check count while keeping supporting evidence collapsed.",
  },
  {
    value: "detailed",
    label: "Detailed",
    description: "Keeps verification checks and supporting evidence visible for every completed task.",
  },
];

export function beginTaskReportViewUpdate(
  current: HarnessConfig,
  next: TaskReportView,
): TaskReportViewUpdate {
  return {
    previous: current,
    optimistic: { ...current, taskReportView: next },
  };
}

export function mergeTaskReportView(
  latest: HarnessConfig | null,
  fallback: HarnessConfig,
  taskReportView: TaskReportView,
): HarnessConfig {
  return { ...(latest ?? fallback), taskReportView };
}

export function resolveTaskReportViewRecovery(
  update: TaskReportViewUpdate,
  recovery: TaskReportViewRecovery,
): TaskReportViewRecoveryDecision {
  if (recovery.reconciled) {
    return {
      config: recovery.reconciled,
      durabilityConfirmed: true,
      restoredPrevious:
        recovery.reconciled.taskReportView === update.previous.taskReportView,
    };
  }
  if (!recovery.initialSavePersisted) {
    return {
      config: update.previous,
      durabilityConfirmed: false,
      restoredPrevious: true,
    };
  }
  const restoredPrevious = recovery.compensation === "succeeded";
  return {
    config: restoredPrevious ? update.previous : update.optimistic,
    durabilityConfirmed: false,
    restoredPrevious,
  };
}
