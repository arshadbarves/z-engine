import { describe, expect, it } from "vitest";
import { taskReportFixture } from "./taskReportFixture";
import { projectTaskReportPresentation } from "./taskReportPresentation";

describe("task report presentation projections", () => {
  it.each([
    ["quiet", { mode: "quiet", details: "disclosure", showCheckCount: false, summary: null }],
    ["compact", {
      mode: "compact",
      details: "disclosure",
      showCheckCount: true,
      summary: "The regression was verified",
    }],
    ["detailed", { mode: "detailed", details: "inline", showCheckCount: true, summary: null }],
  ] as const)("projects the %s mode", (taskReportView, expected) => {
    expect(projectTaskReportPresentation({ taskReportView }, taskReportFixture())).toEqual(expected);
  });

  it("prefers the first blocker for a compact blocked report", () => {
    const report = taskReportFixture({
      status: "blocked",
      blockers: ["API credentials are unavailable", "Network access is disabled"],
    });

    expect(projectTaskReportPresentation({ taskReportView: "compact" }, report).summary)
      .toBe("API credentials are unavailable");
  });

  it.each([
    undefined,
    null,
    {},
    { taskReportView: null },
    { taskReportView: "dense" },
    { taskReportView: 1 },
  ])("falls back malformed or missing config to quiet (%#)", (config) => {
    expect(projectTaskReportPresentation(config, taskReportFixture())).toMatchObject({
      mode: "quiet",
      details: "disclosure",
      showCheckCount: false,
      summary: null,
    });
  });

  it("does not mutate authoritative status or evidence", () => {
    const report = taskReportFixture();
    const before = structuredClone(report);

    projectTaskReportPresentation({ taskReportView: "compact" }, report);

    expect(report).toEqual(before);
  });
});
