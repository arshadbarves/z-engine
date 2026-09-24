import { describe, expect, it } from "vitest";
import { companionPose, GREETING_MS, SLEEPY_MS, TYPING_MS, type UserSignals } from "./companion";
import type { LiveStatus } from "./liveStatus";

const NOW = 1_000_000;

function status(over: Partial<LiveStatus> = {}): LiveStatus {
  return {
    kind: "idle",
    tone: "quiet",
    activity: null,
    text: null,
    detail: null,
    progress: null,
    elapsed: null,
    cost: null,
    noticeId: null,
    waiting: null,
    moreWaiting: 0,
    helpers: { running: 0, pending: 0 },
    ...over,
  };
}

function signals(over: Partial<UserSignals> = {}): UserSignals {
  return { typingAt: null, scrolledBack: false, hovering: false, lastActiveAt: NOW, returnedAt: null, ...over };
}

const working = (activity: LiveStatus["activity"]) => status({ kind: "working", tone: "working", activity });

describe("companionPose", () => {
  it("shows the agent's work through posture", () => {
    expect(companionPose(working("think"), signals(), "lively", NOW)).toMatchObject({ mood: "thinking", gaze: "up", particles: "thought" });
    expect(companionPose(working("read"), signals(), "lively", NOW)).toMatchObject({ mood: "reading", gaze: "scan" });
    expect(companionPose(working("edit"), signals(), "lively", NOW)?.mood).toBe("focused");
    expect(companionPose(working("bash"), signals(), "lively", NOW)).toMatchObject({ mood: "busy", tone: "working" });
  });

  it("looks at the user when it needs them, and droops or cheers at the end", () => {
    expect(companionPose(status({ kind: "attention", tone: "attention" }), signals(), "calm", NOW)).toMatchObject({
      mood: "attention",
      gaze: "center",
      tone: "attention",
    });
    expect(companionPose(status({ kind: "done", tone: "ok" }), signals(), "calm", NOW)).toMatchObject({ mood: "happy", particles: "sparkles" });
    expect(companionPose(status({ kind: "done", tone: "danger" }), signals(), "calm", NOW)?.mood).toBe("worried");
    expect(companionPose(status({ kind: "retrying", tone: "attention" }), signals(), "calm", NOW)?.mood).toBe("tired");
  });

  it("reacts to the user only when lively and the agent is idle", () => {
    const typing = signals({ typingAt: NOW - TYPING_MS + 1 });
    expect(companionPose(status(), typing, "lively", NOW)).toMatchObject({ mood: "listening", gaze: "down" });
    expect(companionPose(status(), typing, "calm", NOW)?.mood).toBe("idle");
    expect(companionPose(status(), signals({ scrolledBack: true }), "lively", NOW)).toMatchObject({ mood: "curious", gaze: "up" });
    expect(companionPose(status(), signals({ returnedAt: NOW - GREETING_MS + 1 }), "lively", NOW)?.mood).toBe("greeting");
  });

  it("dozes after a quiet spell but wakes when hovered", () => {
    const quiet = signals({ lastActiveAt: NOW - SLEEPY_MS - 1 });
    expect(companionPose(status(), quiet, "lively", NOW)).toMatchObject({ mood: "sleepy", particles: "sleep" });
    expect(companionPose(status(), { ...quiet, hovering: true }, "lively", NOW)).toMatchObject({ mood: "idle", gaze: "pointer" });
  });

  it("keeps working but glances at the pointer or the composer", () => {
    expect(companionPose(working("bash"), signals({ hovering: true }), "lively", NOW)).toMatchObject({ mood: "busy", gaze: "pointer" });
    expect(companionPose(working("bash"), signals({ typingAt: NOW }), "lively", NOW)?.gaze).toBe("down");
    expect(companionPose(working("bash"), signals({ hovering: true }), "calm", NOW)?.gaze).toBe("center");
  });

  it("glances toward a waiting chat, and disappears when off", () => {
    const waiting = status({ waiting: { sessionId: "S2", title: "Fix CI" } });
    expect(companionPose(waiting, signals(), "calm", NOW)).toMatchObject({ mood: "glance", gaze: "left" });
    expect(companionPose(working("bash"), signals(), "off", NOW)).toBeNull();
  });
});
