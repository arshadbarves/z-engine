import { beforeEach, describe, expect, it, vi } from "vitest";
import { startSession, type StartSessionResult } from "./commands";
import { displayedTaskStatus } from "./domain/taskReport";
import { taskReportFixture } from "./domain/taskReportFixture";
import {
  activateSession, handleEvent, resetForTests, setBusy, submitLocal, transcriptStore,
} from "./events";
import { hydrateOpenSession } from "./sessionOpen";
import { markTaskReportsPending } from "./runtime/taskReportRefresh";
import type { TaskReport } from "./types";

vi.mock("./commands", async (importOriginal) => ({
  ...await importOriginal<typeof import("./commands")>(),
  startSession: vi.fn(),
}));

const task = () => transcriptStore.getSnapshot().find((message) => message.kind === "task")!;
function response(report: TaskReport): StartSessionResult {
  return {
    ulid: "A", path: "/sessions/A.jsonl", alreadyLive: true,
    events: [{ type: "user_msg", text: "original" }, { type: "task_updated", report }],
  };
}
function parkedCompletion() {
  activateSession("A");
  submitLocal("original");
  handleEvent({ type: "taskUpdated", report: taskReportFixture() });
  const id = task().id;
  activateSession("B");
  return id;
}
function delayedResponse() {
  let resolve!: (value: StartSessionResult) => void;
  const promise = new Promise<StartSessionResult>((done) => { resolve = done; });
  vi.mocked(startSession).mockReturnValueOnce(promise);
  return resolve;
}

beforeEach(() => {
  resetForTests();
  vi.clearAllMocks();
});

describe("revalidating parked completion evidence", () => {
  it("shows freshness unknown immediately on activation without changing the recorded report", () => {
    const id = parkedCompletion();
    activateSession("A");
    expect(task().id).toBe(id);
    expect(task().taskReport?.status).toBe("complete");
    expect(displayedTaskStatus(task().taskReport, task().taskFreshnessPending)).toBe("unassessed");
  });

  it("uses refreshed stale reports even when start_session reports alreadyLive", async () => {
    const id = parkedCompletion();
    const resolve = delayedResponse();
    const opening = hydrateOpenSession("/sessions/A.jsonl");
    expect(displayedTaskStatus(task().taskReport, task().taskFreshnessPending)).toBe("unassessed");
    resolve(response(taskReportFixture({ status: "stale", blockers: ["Inputs changed"] })));
    await opening;
    expect(task().id).toBe(id);
    expect(task().taskReport?.status).toBe("stale");
    expect(task().taskReport?.blockers).toEqual(["Inputs changed"]);
    expect(transcriptStore.getSnapshot().filter((message) => message.kind === "user")).toHaveLength(1);
  });

  it("only restores Complete after backend freshness revalidation", async () => {
    parkedCompletion();
    vi.mocked(startSession).mockResolvedValueOnce(response(taskReportFixture()));
    await hydrateOpenSession("/sessions/A.jsonl");
    expect(task().taskFreshnessPending).toBe(false);
    expect(displayedTaskStatus(task().taskReport, task().taskFreshnessPending)).toBe("complete");
  });

  it("keeps freshness unknown when refresh fails or omits a report", async () => {
    parkedCompletion();
    vi.mocked(startSession).mockRejectedValueOnce(new Error("cannot validate workspace"));
    await hydrateOpenSession("/sessions/A.jsonl");
    expect(displayedTaskStatus(task().taskReport, task().taskFreshnessPending)).toBe("unassessed");
    vi.mocked(startSession).mockResolvedValueOnce({ ulid: "A", events: [], alreadyLive: true });
    await hydrateOpenSession("/sessions/A.jsonl");
    expect(displayedTaskStatus(task().taskReport, task().taskFreshnessPending)).toBe("unassessed");
  });

  it("does not overwrite a newer live update with an older refresh response", async () => {
    parkedCompletion();
    const resolve = delayedResponse();
    const opening = hydrateOpenSession("/sessions/A.jsonl");
    const live = taskReportFixture({ status: "blocked", blockers: ["New live state"] });
    handleEvent({ type: "taskUpdated", sessionId: "A", report: live });
    resolve(response(taskReportFixture()));
    await opening;
    expect(task().taskReport).toEqual(live);
  });

  it("does not restore green after a newer request invalidates the pending snapshot", async () => {
    parkedCompletion();
    const resolve = delayedResponse();
    const opening = hydrateOpenSession("/sessions/A.jsonl");
    markTaskReportsPending();
    submitLocal("new work");
    resolve(response(taskReportFixture()));
    await opening;
    expect(displayedTaskStatus(task().taskReport, task().taskFreshnessPending)).toBe("unassessed");
  });

  it("applies a delayed refresh to the parked session, not a newly visible session", async () => {
    parkedCompletion();
    const resolve = delayedResponse();
    const opening = hydrateOpenSession("/sessions/A.jsonl");
    activateSession("B");
    submitLocal("other session");
    const visible = transcriptStore.getSnapshot();
    resolve(response(taskReportFixture({ status: "stale" })));
    await opening;
    expect(transcriptStore.getSnapshot()).toEqual(visible);
    activateSession("A");
    expect(task().taskReport?.status).toBe("stale");
  });

  it("never interrupts a running in-memory task using a disk-only projection", async () => {
    activateSession("A");
    submitLocal("running");
    setBusy(true);
    handleEvent({ type: "taskUpdated", report: taskReportFixture({ status: "running" }) });
    activateSession("B");
    vi.mocked(startSession).mockResolvedValueOnce(response(taskReportFixture({ status: "interrupted" })));
    await hydrateOpenSession("/sessions/A.jsonl");
    expect(task().taskReport?.status).toBe("running");
  });

  it("cold replay still consumes the backend-refreshed report", async () => {
    vi.mocked(startSession).mockResolvedValueOnce(response(taskReportFixture({ status: "stale" })));
    await hydrateOpenSession("/sessions/A.jsonl");
    expect(task().taskReport?.status).toBe("stale");
  });
});
