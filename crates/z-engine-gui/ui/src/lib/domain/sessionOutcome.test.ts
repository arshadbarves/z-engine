import { describe, expect, it } from "vitest";
import { unreadSessionOutcome } from "./sessionOutcome";

describe("sidebar unread task outcomes", () => {
  it("never presents a legacy response end as verified completion", () => {
    expect(unreadSessionOutcome("completed", false, null))
      .toEqual({ label: "Response finished · unassessed", tone: "neutral" });
    expect(unreadSessionOutcome("complete", false, null))
      .toEqual({ label: "Complete · verified task", tone: "verified" });
  });

  it.each([
    "running", "needs_verification", "blocked", "stopped", "aborted",
    "interrupted", "unassessed", "stale", "failed", "unknown-future-status",
  ])("does not show a verified indicator for %s", (outcome) => {
    const presentation = unreadSessionOutcome(outcome, false, null);
    expect(presentation).not.toBeNull();
    expect(presentation?.tone).not.toBe("verified");
    expect(presentation?.label).not.toContain("Complete");
  });

  it("hides unread metadata for opened and currently active sessions", () => {
    expect(unreadSessionOutcome("complete", true, null)).toBeNull();
    expect(unreadSessionOutcome("complete", false, "working")).toBeNull();
    expect(unreadSessionOutcome("complete", false, "approval")).toBeNull();
    expect(unreadSessionOutcome(null, false, null)).toBeNull();
  });
});
