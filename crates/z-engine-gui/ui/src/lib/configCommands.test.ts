import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { saveGeneral } from "./commands";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));

beforeEach(() => vi.clearAllMocks());

describe("supervision settings IPC", () => {
  it.each([0, 3, 10])("preserves the %i limit in the camelCase request", async (maxTaskContinuations) => {
    await saveGeneral({ maxTaskContinuations });
    expect(invoke).toHaveBeenCalledWith("save_general", {
      model: null,
      baseUrl: null,
      maxContextTokens: null,
      review: null,
      maxTaskContinuations,
      taskReportView: null,
    });
  });

  it("leaves the continuation limit unchanged for unrelated settings", async () => {
    await saveGeneral({ review: false });
    expect(invoke).toHaveBeenCalledWith("save_general", expect.objectContaining({
      review: false, maxTaskContinuations: null,
    }));
  });
});
