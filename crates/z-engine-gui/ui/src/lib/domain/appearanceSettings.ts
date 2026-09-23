import type { TaskReportView } from "../protocol/config/TaskReportView";

export interface AppearanceOption {
  value: TaskReportView;
  label: string;
  description: string;
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
