import type { SessionActivity } from "../types";

export interface UnreadSessionOutcome {
  label: string;
  tone: "neutral" | "verified" | "warn";
}

/** Sidebar metadata distinguishes task verification from legacy response endings. */
export function unreadSessionOutcome(
  outcome: string | null | undefined,
  active: boolean,
  activity: SessionActivity | null,
): UnreadSessionOutcome | null {
  if (active || activity || !outcome) return null;
  switch (outcome) {
    case "complete": return { label: "Complete · verified task", tone: "verified" };
    case "completed": return { label: "Response finished · unassessed", tone: "neutral" };
    case "aborted":
    case "stopped": return { label: "Stopped", tone: "neutral" };
    case "running": return { label: "Running", tone: "neutral" };
    case "needs_verification": return { label: "Needs verification", tone: "warn" };
    case "blocked": return { label: "Blocked", tone: "warn" };
    case "stale": return { label: "Stale evidence", tone: "warn" };
    case "interrupted": return { label: "Interrupted", tone: "warn" };
    case "failed": return { label: "Response failed", tone: "warn" };
    default: return { label: "Unassessed", tone: "neutral" };
  }
}
