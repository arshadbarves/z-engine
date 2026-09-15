import { describe, expect, it } from "vitest";
import { DEFAULT_TASK_CONTINUATIONS, MAX_TASK_CONTINUATIONS, taskContinuationLimitError } from "./configSupervision";

describe("task continuation settings", () => {
  it("matches the runtime default and cap", () => {
    expect(DEFAULT_TASK_CONTINUATIONS).toBe(3);
    expect(MAX_TASK_CONTINUATIONS).toBe(10);
  });

  it.each([0, 1, 3, 10])("accepts %i", (value) => {
    expect(taskContinuationLimitError(value)).toBeNull();
  });

  it.each([undefined, -1, 1.5, 11, NaN, Infinity])("rejects %s", (value) => {
    expect(taskContinuationLimitError(value)).toContain("whole number from 0 to 10");
  });
});
