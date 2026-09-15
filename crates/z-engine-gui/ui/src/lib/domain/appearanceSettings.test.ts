import { describe, expect, it } from "vitest";
import {
  APPEARANCE_OPTIONS,
  beginTaskReportViewUpdate,
  mergeTaskReportView,
  resolveTaskReportViewRecovery,
} from "./appearanceSettings";

describe("appearance settings", () => {
  it("offers the three report density modes in increasing detail", () => {
    expect(APPEARANCE_OPTIONS.map(({ value, label }) => ({ value, label }))).toEqual([
      { value: "quiet", label: "Quiet" },
      { value: "compact", label: "Compact" },
      { value: "detailed", label: "Detailed" },
    ]);
    expect(APPEARANCE_OPTIONS.every((option) => option.description.length > 20)).toBe(true);
  });

  it("creates an immutable optimistic config update", () => {
    const current = {
      model: "openrouter/auto",
      maxContextTokens: 128_000,
      taskReportView: "quiet" as const,
    };

    const update = beginTaskReportViewUpdate(current, "detailed");

    expect(update.previous).toBe(current);
    expect(update.optimistic).toEqual({ ...current, taskReportView: "detailed" });
    expect(update.optimistic).not.toBe(current);
    expect(current.taskReportView).toBe("quiet");
  });

  it("merges a recovered view into the latest config snapshot", () => {
    const fallback = {
      model: "old-model",
      maxContextTokens: 128_000,
      taskReportView: "quiet" as const,
    };
    const latest = {
      ...fallback,
      model: "concurrently-updated-model",
      maxContextTokens: 64_000,
    };

    expect(mergeTaskReportView(latest, fallback, "detailed")).toEqual({
      ...latest,
      taskReportView: "detailed",
    });
    expect(latest.taskReportView).toBe("quiet");
  });

  it("uses the supplied fallback when the config store is empty", () => {
    const fallback = {
      model: "openrouter/auto",
      maxContextTokens: 128_000,
      taskReportView: "compact" as const,
    };

    expect(mergeTaskReportView(null, fallback, "quiet")).toEqual({
      ...fallback,
      taskReportView: "quiet",
    });
  });

  it("uses reconciled durable config after a failed compensating save", () => {
    const previous = {
      model: "openrouter/auto",
      maxContextTokens: 128_000,
      taskReportView: "quiet" as const,
    };
    const update = beginTaskReportViewUpdate(previous, "detailed");
    const durable = { ...previous, taskReportView: "compact" as const };

    expect(resolveTaskReportViewRecovery(update, {
      initialSavePersisted: true,
      compensation: "failed",
      reconciled: durable,
    })).toEqual({
      config: durable,
      durabilityConfirmed: true,
      restoredPrevious: false,
    });
  });

  it("keeps the optimistic value as best-known durable state when rollback is unconfirmed", () => {
    const previous = {
      model: "openrouter/auto",
      maxContextTokens: 128_000,
      taskReportView: "quiet" as const,
    };
    const update = beginTaskReportViewUpdate(previous, "detailed");

    expect(resolveTaskReportViewRecovery(update, {
      initialSavePersisted: true,
      compensation: "failed",
      reconciled: null,
    })).toEqual({
      config: update.optimistic,
      durabilityConfirmed: false,
      restoredPrevious: false,
    });
  });

  it("uses the prior value as best-known state when compensation succeeded but refresh failed", () => {
    const previous = {
      model: "openrouter/auto",
      maxContextTokens: 128_000,
      taskReportView: "compact" as const,
    };
    const update = beginTaskReportViewUpdate(previous, "detailed");

    expect(resolveTaskReportViewRecovery(update, {
      initialSavePersisted: true,
      compensation: "succeeded",
      reconciled: null,
    })).toEqual({
      config: previous,
      durabilityConfirmed: false,
      restoredPrevious: true,
    });
  });

  it("uses the prior config as unconfirmed best-known state when the initial save throws", () => {
    const previous = {
      model: "openrouter/auto",
      maxContextTokens: 128_000,
      taskReportView: "compact" as const,
    };
    const update = beginTaskReportViewUpdate(previous, "quiet");

    expect(resolveTaskReportViewRecovery(update, {
      initialSavePersisted: false,
      compensation: "not-needed",
      reconciled: null,
    })).toEqual({
      config: previous,
      durabilityConfirmed: false,
      restoredPrevious: true,
    });
  });

  it("uses reconciled config when an initial save reports failure after writing", () => {
    const previous = {
      model: "openrouter/auto",
      maxContextTokens: 128_000,
      taskReportView: "compact" as const,
    };
    const update = beginTaskReportViewUpdate(previous, "detailed");

    expect(resolveTaskReportViewRecovery(update, {
      initialSavePersisted: false,
      compensation: "not-needed",
      reconciled: update.optimistic,
    })).toEqual({
      config: update.optimistic,
      durabilityConfirmed: true,
      restoredPrevious: false,
    });
  });
});
