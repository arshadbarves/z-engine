import { beforeEach, describe, expect, it, vi } from "vitest";
import { startSession, submit } from "./commands";
import {
  activateSession, busyStore, handleEvent, hydrateStore, queueStore, resetForTests,
  sessionStore, submitLocal, transcriptStore,
} from "./events";
import { displayedTaskStatus } from "./domain/taskReport";
import { taskReportFixture } from "./domain/taskReportFixture";
import { submitTask } from "./sessionSubmit";
import { workspaceStore } from "./workspaces";

vi.mock("./commands", async (importOriginal) => ({
  ...await importOriginal<typeof import("./commands")>(),
  startSession: vi.fn(),
  submit: vi.fn(),
}));

beforeEach(() => {
  resetForTests();
  vi.clearAllMocks();
  workspaceStore.setActive("/workspace/project");
  vi.mocked(startSession).mockResolvedValue({
    ulid: "recorded", path: "/sessions/recorded.jsonl", events: [],
  });
  vi.mocked(submit).mockResolvedValue(undefined);
});

describe("recorder-backed task submission", () => {
  it.each(["", "boot"])("creates a durable session before submitting from %j", async (initial) => {
    if (initial) activateSession(initial);
    vi.mocked(submit).mockImplementation(async (_text, _images, id) => {
      expect(id).toBe("recorded");
      expect(hydrateStore.getSnapshot()).toBe(false);
      handleEvent({ type: "taskUpdated", sessionId: id, report: taskReportFixture({ status: "running" }) });
    });
    expect(await submitTask("verify parser", ["image"])).toBe(true);
    expect(startSession).toHaveBeenCalledWith(null, "/workspace/project");
    expect(submit).toHaveBeenCalledWith("verify parser", ["image"], "recorded");
    expect(sessionStore.getSnapshot()).toBe("recorded");
    expect(transcriptStore.getSnapshot().find((m) => m.kind === "task")?.taskReport?.status)
      .toBe("running");
  });

  it("shares creation across rapid submissions and queues the second request", async () => {
    const first = submitTask("first");
    const second = submitTask("second");
    await Promise.all([first, second]);
    expect(startSession).toHaveBeenCalledTimes(1);
    expect(submit).toHaveBeenCalledTimes(1);
    expect(queueStore.getSnapshot().map((item) => item.text)).toEqual(["second"]);
  });

  it("does not fall back to the boot loop when creation lacks a durable path", async () => {
    vi.mocked(startSession).mockResolvedValue({ ulid: "recorded", events: [], path: null });
    expect(await submitTask("verify")).toBe(false);
    expect(await submitTask("retry")).toBe(false);
    expect(submit).not.toHaveBeenCalled();
  });

  it("preserves original task cards but invalidates their display freshness for a new goal", async () => {
    activateSession("recorded");
    submitLocal("original");
    handleEvent({ type: "taskUpdated", report: taskReportFixture() });
    const original = transcriptStore.getSnapshot().find((m) => m.kind === "task")!;
    await submitTask("new goal");
    const kept = transcriptStore.getSnapshot().find((m) => m.id === original.id)!;
    expect(kept.taskReport).toEqual(original.taskReport);
    expect(displayedTaskStatus(kept.taskReport, kept.taskFreshnessPending)).toBe("unassessed");
    expect(transcriptStore.getSnapshot().filter((m) => m.kind === "user")).toHaveLength(2);
    expect(startSession).not.toHaveBeenCalled();
  });

  it("routes a rejected background submission without clearing another session's busy state", async () => {
    activateSession("A");
    activateSession("B");
    await submitTask("visible work", [], "B");
    vi.mocked(submit).mockRejectedValueOnce(new Error("submit rejected"));
    expect(await submitTask("background", [], "A")).toBe(false);
    expect(busyStore.getSnapshot()).toBe(true);
    expect(transcriptStore.getSnapshot().some((m) => m.kind === "error")).toBe(false);
    activateSession("A");
    expect(busyStore.getSnapshot()).toBe(false);
    expect(transcriptStore.getSnapshot().some((m) => m.kind === "error")).toBe(true);
  });
});
