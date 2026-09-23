import type { Effort } from "../protocol/Effort";
import type { PermissionMode } from "../protocol/PermissionMode";

export interface ModeMeta {
  id: PermissionMode;
  label: string;
  description: string;
  /** Shown before switching into a mode that removes guardrails. */
  warning?: string;
}

export const MODES: ModeMeta[] = [
  { id: "default", label: "Ask", description: "Ask before gated edits and commands" },
  { id: "acceptEdits", label: "Auto-accept edits", description: "Apply edits in allowed folders without asking" },
  { id: "plan", label: "Plan", description: "Read-only research; propose a plan before changing files" },
  {
    id: "bypass",
    label: "Bypass",
    description: "Approve everything except explicit deny rules",
    warning: "Edits and commands run without asking. Use it only in a sandbox or a disposable checkout.",
  },
];

export function modeMeta(mode: PermissionMode): ModeMeta {
  return MODES.find((m) => m.id === mode) ?? MODES[0];
}

/** Shift+Tab cycle; bypass is never entered by accident. */
export function nextMode(mode: PermissionMode): PermissionMode {
  const cycle: PermissionMode[] = ["default", "acceptEdits", "plan"];
  const index = cycle.indexOf(mode);
  return cycle[(index + 1) % cycle.length];
}

export interface EffortMeta {
  id: Effort;
  description: string;
}

export const EFFORTS: EffortMeta[] = [
  { id: "low", description: "Fast and cheap" },
  { id: "medium", description: "Balanced default" },
  { id: "high", description: "Thorough thinking" },
  { id: "max", description: "Maximum depth" },
];
