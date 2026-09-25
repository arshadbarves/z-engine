import { describe, expect, it } from "vitest";
import { needsOnboarding, nextStep, prevStep, setupChecklist, stepIndex } from "./onboarding";

describe("needsOnboarding", () => {
  it("welcomes a brand-new install only", () => {
    expect(needsOnboarding({ projects: 0, chats: 0 })).toBe(true);
    expect(needsOnboarding({ projects: 1, chats: 0 })).toBe(false);
    expect(needsOnboarding({ projects: 0, chats: 3 })).toBe(false);
  });
});

describe("steps", () => {
  it("walks forward and back and stops at the ends", () => {
    expect(nextStep("welcome")).toBe("model");
    expect(nextStep("style")).toBe("ready");
    expect(nextStep("ready")).toBe("ready");
    expect(prevStep("model")).toBe("welcome");
    expect(prevStep("welcome")).toBe("welcome");
    expect(stepIndex("project")).toBe(2);
  });
});

describe("setupChecklist", () => {
  it("lists what is left and counts what is done", () => {
    const list = setupChecklist({ modelReady: true, trusted: false, hasInstructions: null });
    expect(list.items.map((i) => [i.id, i.done])).toEqual([
      ["model", true],
      ["trust", false],
    ]);
    expect(list).toMatchObject({ done: 1, total: 2, complete: false });
  });

  it("is complete when everything that applies is done", () => {
    const list = setupChecklist({ modelReady: true, trusted: true, hasInstructions: true });
    expect(list).toMatchObject({ done: 3, total: 3, complete: true });
  });
});
