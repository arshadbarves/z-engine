import type { CtxMeter } from "./contextMeter";
import type { LiveStatus } from "./liveStatus";
import { MAIN_AGENT, type ToolCallView } from "./sessionView/types";
import { activityLabel } from "./tools/activityLabel";

/** The compact island: one line of words and at most one number. */
export interface IslandLine {
  text: string | null;
  metric: string | null;
}

export function islandLine(status: LiveStatus, chatTitle: string | null): IslandLine {
  const metric =
    status.kind === "working" || status.kind === "done"
      ? status.elapsed
      : status.kind === "retrying"
        ? status.detail
        : null;
  return { text: status.text ?? chatTitle, metric };
}

/** A notice opens the island by itself when the line cannot hold it, it is an error, or it offers actions. */
export function noticeOpensIsland(notice: { tone: string; actions?: unknown[] } | null, truncated: boolean): boolean {
  if (!notice) return false;
  return truncated || notice.tone === "error" || (notice.actions?.length ?? 0) > 0;
}

export interface ContextBubbleLook {
  tone: "quiet" | "warn" | "danger";
  /** The percentage, shown only once the context is filling up. */
  label: string | null;
}

export function contextBubble(meter: Pick<CtxMeter, "pct" | "level">): ContextBubbleLook {
  if (meter.level === "ok") return { tone: "quiet", label: null };
  return { tone: meter.level, label: `${meter.pct}%` };
}

export interface IslandStep {
  callId: string;
  label: string;
  state: "running" | "done" | "failed";
}

/** The main agent's latest steps since `since`, newest first, for the expanded island. */
export function recentSteps(tools: Record<string, ToolCallView>, since: number, limit: number): IslandStep[] {
  return Object.values(tools)
    .filter((t) => t.agentId === MAIN_AGENT && t.startedAt >= since)
    .sort((a, b) => b.startedAt - a.startedAt)
    .slice(0, limit)
    .map((t) => ({
      callId: t.callId,
      label: activityLabel(t.tool, t.input),
      state: t.status === "running" ? "running" : t.status === "ok" ? "done" : "failed",
    }));
}
