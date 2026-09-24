import type { SessionActivity, UnreadMark } from "./sessions";

export interface UnreadSessionOutcome {
  label: string;
  tone: "neutral" | "verified" | "warn";
}

/** Sidebar dot for a turn that finished while the chat was in the background. */
export function unreadSessionOutcome(
  mark: UnreadMark | null | undefined,
  active: boolean,
  activity: SessionActivity | null,
): UnreadSessionOutcome | null {
  if (active || activity || !mark) return null;
  switch (mark.outcome.type) {
    case "failed":
      return { label: "Response failed", tone: "warn" };
    case "budgetExhausted":
      return { label: "Stopped · budget reached", tone: "warn" };
    case "interrupted":
      return { label: "Interrupted", tone: "warn" };
    case "cancelled":
      return { label: "Stopped", tone: "neutral" };
    default:
      break;
  }
  switch (mark.verification.status) {
    case "verified":
      return { label: "Finished · verified", tone: "verified" };
    case "failed":
      return { label: "Finished · checks failed", tone: "warn" };
    case "unverified":
      return { label: "Finished · unverified", tone: "neutral" };
    default:
      return { label: "Finished", tone: "neutral" };
  }
}
