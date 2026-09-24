import type { LiveActivity, LiveStatus, LiveTone } from "./liveStatus";

export type CompanionLevel = "lively" | "calm" | "off";

export type CompanionMood =
  | "idle"
  | "listening"
  | "curious"
  | "sleepy"
  | "greeting"
  | "thinking"
  | "reading"
  | "focused"
  | "busy"
  | "attention"
  | "glance"
  | "tired"
  | "happy"
  | "worried";

export type Gaze = "center" | "down" | "up" | "left" | "scan" | "pointer";
export type Particles = "none" | "thought" | "sparkles" | "sleep";

/** What the user is doing, from DOM events; see `lib/stores/userSignals.svelte.ts`. */
export interface UserSignals {
  typingAt: number | null;
  scrolledBack: boolean;
  hovering: boolean;
  lastActiveAt: number;
  returnedAt: number | null;
}

export interface CompanionPose {
  mood: CompanionMood;
  gaze: Gaze;
  particles: Particles;
  tone: LiveTone;
}

/** Keystrokes count as "listening" this long after the last one. */
export const TYPING_MS = 1600;
/** Nothing from the user or the agent for this long, and the companion dozes. */
export const SLEEPY_MS = 3 * 60_000;
/** How long the wave lasts after the user comes back. */
export const GREETING_MS = 4000;

function pose(mood: CompanionMood, gaze: Gaze, tone: LiveTone, particles: Particles = "none"): CompanionPose {
  return { mood, gaze, particles, tone };
}

const WORK_POSE: Partial<Record<LiveActivity, [CompanionMood, Gaze, Particles]>> = {
  think: ["thinking", "up", "thought"],
  reply: ["focused", "down", "none"],
  read: ["reading", "scan", "none"],
  search: ["reading", "scan", "none"],
  web: ["reading", "scan", "none"],
  edit: ["focused", "down", "none"],
  write: ["focused", "down", "none"],
  todo: ["focused", "down", "none"],
  plan: ["focused", "down", "none"],
};

function agentPose(status: LiveStatus): CompanionPose | null {
  switch (status.kind) {
    case "attention":
      return pose("attention", "center", "attention");
    case "retrying":
      return pose("tired", "center", "attention");
    case "working": {
      const [mood, gaze, particles] = (status.activity && WORK_POSE[status.activity]) ?? ["busy", "center", "none"];
      return pose(mood, gaze, "working", particles);
    }
    case "notice":
    case "done":
      if (status.tone === "ok") return pose("happy", "center", "ok", "sparkles");
      if (status.tone === "danger" || status.tone === "attention") return pose("worried", "down", status.tone);
      return status.kind === "done" ? pose("idle", "center", "quiet") : null;
    default:
      return null;
  }
}

function userPose(signals: UserSignals, now: number): CompanionPose | null {
  if (signals.returnedAt !== null && now - signals.returnedAt < GREETING_MS) return pose("greeting", "center", "quiet");
  if (signals.typingAt !== null && now - signals.typingAt < TYPING_MS) return pose("listening", "down", "quiet");
  if (signals.scrolledBack) return pose("curious", "up", "quiet");
  if (!signals.hovering && now - signals.lastActiveAt > SLEEPY_MS) return pose("sleepy", "down", "quiet", "sleep");
  return null;
}

/**
 * The companion's body language. The agent's state always wins; at the
 * lively level the eyes still follow what the user is doing (the pointer,
 * the composer while typing), and an idle agent reacts to the user.
 */
export function companionPose(
  status: LiveStatus,
  signals: UserSignals,
  level: CompanionLevel,
  now: number,
): CompanionPose | null {
  if (level === "off") return null;
  const lively = level === "lively";
  const base =
    agentPose(status) ??
    (lively ? userPose(signals, now) : null) ??
    (status.waiting ? pose("glance", "left", "attention") : pose("idle", "center", "quiet"));
  if (!lively || base.mood === "sleepy" || base.mood === "greeting") return base;
  if (signals.hovering) return { ...base, gaze: "pointer" };
  if (signals.typingAt !== null && now - signals.typingAt < TYPING_MS) return { ...base, gaze: "down" };
  return base;
}
