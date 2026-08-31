import type { ReplayEvent } from "../types";
import { push, resetTranscript } from "./mutations";
import { rt } from "./state";

function shortArgs(name: string, raw: string): string {
  try {
    const v = JSON.parse(raw) as Record<string, unknown>;
    if (typeof v.path === "string") return v.path;
    if (typeof v.command === "string") return v.command;
    if (typeof v.pattern === "string") return v.pattern;
    if (typeof v.query === "string") return v.query;
    return Object.values(v)[0]?.toString() ?? name;
  } catch {
    return raw;
  }
}

function resultSummary(content: string): string {
  for (const line of content.split("\n")) {
    const t = line.trim();
    if (!t || t.startsWith("--- ") || t.startsWith("[killed") || t.startsWith("exit code:")) {
      continue;
    }
    return t.length > 120 ? `${t.slice(0, 117)}…` : t;
  }
  return "ok";
}

/** Rebuild transcript cards from a session JSONL event list. */
export function replaySession(events: ReplayEvent[]) {
  resetTranscript();
  const results = new Map<string, string>();
  for (const ev of events) {
    if (ev.type === "tool_result" && ev.tool_call_id) {
      results.set(ev.tool_call_id, ev.content ?? "");
    }
  }
  for (const ev of events) {
    switch (ev.type) {
      case "user_msg":
        push("user", ev.text ?? "", {
          images: ev.images && ev.images.length > 0 ? ev.images : undefined,
          runTurn: rt.runTurnCounter++,
        });
        break;
      case "assistant_msg": {
        if (ev.content) push("assistant", ev.content);
        for (const tc of ev.tool_calls ?? []) {
          const out = results.get(tc.id) ?? "";
          push("tool", `✓ ${tc.name} ─ ${shortArgs(tc.name, tc.arguments)}`, {
            toolName: tc.name,
            preview: shortArgs(tc.name, tc.arguments),
            summary: resultSummary(out),
            output: out,
            ok: true,
            streaming: false,
            durationMs: 0,
          });
        }
        break;
      }
      case "note":
      case "meta":
      case "tool_result":
      case "title":
        break;
    }
  }
}
