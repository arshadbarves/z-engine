import { beforeEach, describe, expect, it } from "vitest";
import { taskReportFixture } from "../domain/taskReportFixture";
import {
  activateSession, beginHydrate, endHydrate, handleEvent, parkCurrentAndReset,
  resetForTests, submitLocal, transcriptStore,
} from "../events";

const tasks = () => transcriptStore.getSnapshot().filter((m) => m.kind === "task");
beforeEach(() => resetForTests());

describe("session-scoped task report projection", () => {
  it("updates a parked task without contaminating the foreground session", () => {
    activateSession("A");
    submitLocal("first session");
    handleEvent({ type: "taskUpdated", sessionId: "A", report: taskReportFixture({ status: "running" }) });
    const original = tasks()[0].taskReport;
    activateSession("B");
    submitLocal("second session");
    handleEvent({
      type: "taskUpdated", sessionId: "B",
      report: taskReportFixture({ taskId: "task-B", status: "blocked" }),
    });
    handleEvent({ type: "taskUpdated", sessionId: "A", report: taskReportFixture() });
    expect(tasks().map((m) => m.taskReport?.taskId)).toEqual(["task-B"]);
    expect(tasks()[0].taskReport?.status).toBe("blocked");
    activateSession("A");
    expect(tasks()).toHaveLength(1);
    expect(tasks()[0].taskReport?.status).toBe("complete");
    expect(original?.status).toBe("running");
  });

  it("retains parked updates while a different session is hydrating", () => {
    activateSession("A");
    submitLocal("first");
    activateSession("B");
    const generation = beginHydrate();
    handleEvent({ type: "taskUpdated", sessionId: "A", report: taskReportFixture() });
    expect(tasks()).toHaveLength(0);
    endHydrate(generation);
    activateSession("A");
    expect(tasks()[0].taskReport?.status).toBe("complete");
  });

  it("keeps background updates out of a blank new-chat view", () => {
    activateSession("A");
    submitLocal("first");
    parkCurrentAndReset();
    handleEvent({ type: "taskUpdated", sessionId: "A", report: taskReportFixture() });
    expect(transcriptStore.getSnapshot()).toHaveLength(0);
    activateSession("A");
    expect(tasks()[0].taskReport?.status).toBe("complete");
  });

  it("does not reinterpret a live running snapshot as interrupted when swapping", () => {
    activateSession("A");
    handleEvent({ type: "taskUpdated", report: taskReportFixture({ status: "running" }) });
    activateSession("B");
    activateSession("A");
    expect(tasks()[0].taskReport?.status).toBe("running");
  });
});
