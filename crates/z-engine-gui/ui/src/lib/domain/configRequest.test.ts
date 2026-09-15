import { describe, expect, it } from "vitest";
import { shouldApplyConfigResponse } from "./configRequest";

describe("config request freshness", () => {
  const initial = { taskReportView: "quiet" };

  it("accepts a response while mounted when the store is unchanged", () => {
    expect(shouldApplyConfigResponse(true, initial, initial)).toBe(true);
  });

  it("rejects a response after the component unmounts", () => {
    expect(shouldApplyConfigResponse(false, initial, initial)).toBe(false);
  });

  it("rejects a response when a newer config replaced the request snapshot", () => {
    expect(shouldApplyConfigResponse(true, initial, { taskReportView: "detailed" })).toBe(false);
  });
});
