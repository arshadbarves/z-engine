import type { JsonValue } from "../../protocol/serde_json/JsonValue";
import type { ToolCallStatus } from "../sessionView/types";
import { activityLabel } from "../tools/activityLabel";
import { str } from "../tools/toolInput";
import { toolMeta, type ToolFamily } from "../tools/toolMeta";
import type { ToolUseRef } from "./blocks";
import type { TimelineItem } from "./turns";

/** A transcript block: one item, or a run of consecutive tool calls shown as one line. */
export type TurnBlock =
  | { kind: "item"; key: string; item: TimelineItem }
  | { kind: "run"; key: string; uses: ToolUseRef[] };

/** Calls that matter on their own: a subagent, a question, a plan. */
const SOLO: ReadonlySet<ToolFamily> = new Set(["agent", "question", "plan"]);

function foldable(item: TimelineItem): item is Extract<TimelineItem, { kind: "tool" }> {
  return item.kind === "tool" && !SOLO.has(toolMeta(item.use.name).family);
}

/** Folds two or more consecutive tool calls into a run; a lone call keeps its own card. */
export function groupTurnItems(items: TimelineItem[]): TurnBlock[] {
  const blocks: TurnBlock[] = [];
  let pending: Extract<TimelineItem, { kind: "tool" }>[] = [];
  const flush = () => {
    if (pending.length >= 2) {
      blocks.push({ kind: "run", key: `run:${pending[0].use.callId}`, uses: pending.map((p) => p.use) });
    } else {
      for (const item of pending) blocks.push({ kind: "item", key: item.key, item });
    }
    pending = [];
  };
  for (const item of items) {
    if (foldable(item)) {
      pending.push(item);
      continue;
    }
    flush();
    blocks.push({ kind: "item", key: item.key, item });
  }
  flush();
  return blocks;
}

export interface RunCall {
  name: string;
  input: JsonValue;
  status: ToolCallStatus;
}

export interface RunSummary {
  /** Plain phrases in the order the work happened: "read 2 files", "ran 1 command". */
  parts: string[];
  state: "running" | "ok" | "failed";
  /** The step in progress, in words, while the run is live. */
  current: string | null;
  failed: number;
  denied: number;
}

function n(count: number, one: string, many = `${one}s`): string {
  return `${count} ${count === 1 ? one : many}`;
}

function pathOf(input: JsonValue): string {
  return str(input, "file_path", "notebook_path", "path");
}

/** What a run of tool calls did, counted by kind, with its live step and failures. */
export function summarizeRun(calls: RunCall[]): RunSummary {
  const order: string[] = [];
  const files = new Map<string, Set<string>>();
  const counts = new Map<string, number>();
  const bump = (key: string, path?: string) => {
    if (!counts.has(key)) order.push(key);
    counts.set(key, (counts.get(key) ?? 0) + 1);
    if (path !== undefined) {
      const set = files.get(key) ?? new Set<string>();
      set.add(path);
      files.set(key, set);
    }
  };
  for (const c of calls) {
    const meta = toolMeta(c.name);
    if (meta.family === "read" || meta.family === "edit" || meta.family === "write") bump(meta.family, pathOf(c.input));
    else if (meta.family === "mcp") bump(`mcp:${meta.server ?? ""}`);
    else if (meta.family === "web") bump(c.name === "WebSearch" ? "websearch" : "webfetch");
    else if (["search", "bash", "todo", "job"].includes(meta.family)) bump(meta.family);
    else bump("other");
  }
  const parts = order.map((key) => {
    const count = counts.get(key) ?? 0;
    const distinct = files.get(key)?.size ?? count;
    if (key === "read") return `read ${n(distinct, "file")}`;
    if (key === "edit") return `edited ${n(distinct, "file")}`;
    if (key === "write") return `wrote ${n(distinct, "file")}`;
    if (key === "search") return `searched ${count}×`;
    if (key === "bash") return `ran ${n(count, "command")}`;
    if (key === "webfetch") return `read ${n(count, "web page")}`;
    if (key === "websearch") return count === 1 ? "searched the web" : `searched the web ${count}×`;
    if (key === "todo") return "updated the plan";
    if (key === "job") return `checked ${n(count, "background job")}`;
    if (key.startsWith("mcp:")) return `used ${key.slice(4)} ${count}×`;
    return `used ${n(count, "other tool")}`;
  });
  const live = [...calls].reverse().find((c) => c.status === "running");
  const failed = calls.filter((c) => c.status === "error").length;
  const denied = calls.filter((c) => c.status === "denied").length;
  return {
    parts,
    state: live ? "running" : failed || denied ? "failed" : "ok",
    current: live ? activityLabel(live.name, live.input) : null,
    failed,
    denied,
  };
}

/** The run's one line, capitalized: "Read 2 files · ran 1 command". */
export function runLine(summary: RunSummary): string {
  const line = summary.parts.join(" · ");
  return line.charAt(0).toUpperCase() + line.slice(1);
}
