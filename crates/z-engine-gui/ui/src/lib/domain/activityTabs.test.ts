import { describe, expect, it } from "vitest";
import {
  nextActivityTabIndex,
  resolveActivityTab,
  shouldRestoreProcessTrigger,
} from "./activityTabs";

describe("nextActivityTabIndex", () => {
  it("wraps horizontal arrow navigation across visible tabs", () => {
    expect(nextActivityTabIndex(0, 3, "ArrowLeft")).toBe(2);
    expect(nextActivityTabIndex(2, 3, "ArrowRight")).toBe(0);
  });

  it("supports Home and End without moving for unrelated keys", () => {
    expect(nextActivityTabIndex(2, 4, "Home")).toBe(0);
    expect(nextActivityTabIndex(1, 4, "End")).toBe(3);
    expect(nextActivityTabIndex(1, 4, "Enter")).toBe(1);
  });
});

describe("resolveActivityTab", () => {
  it("restores focus only when a focused active tab disappears", () => {
    expect(resolveActivityTab("files", ["all", "reason"], true)).toEqual({
      active: "all",
      restoreFocus: true,
    });
    expect(resolveActivityTab("files", ["all", "reason"], false)).toEqual({
      active: "all",
      restoreFocus: false,
    });
  });

  it("keeps a surviving active tab without requesting focus", () => {
    expect(resolveActivityTab("reason", ["all", "reason"], true)).toEqual({
      active: "reason",
      restoreFocus: false,
    });
  });
});

describe("shouldRestoreProcessTrigger", () => {
  it("restores focus only when streaming closes a focused inspector", () => {
    expect(shouldRestoreProcessTrigger(true, false, true)).toBe(true);
    expect(shouldRestoreProcessTrigger(true, false, false)).toBe(false);
    expect(shouldRestoreProcessTrigger(false, false, true)).toBe(false);
    expect(shouldRestoreProcessTrigger(true, true, true)).toBe(false);
  });
});
