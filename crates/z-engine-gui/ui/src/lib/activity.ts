import type { Msg } from "./events";
import { fmtDur, toolLabel } from "./toolUi";

export type LedgerEntry = {
  id: number;
  category: "read" | "edit" | "search" | "bash" | "thought" | "tool";
  title: string;
  sub?: string;
  metric?: string;
  dur: number;
  ok?: boolean;
  output?: string;
  body?: string;
  streaming?: boolean;
};

export type ActivityLedger = {
  all: LedgerEntry[];
  files: LedgerEntry[];
  searches: LedgerEntry[];
  terminal: LedgerEntry[];
  thoughts: LedgerEntry[];
};

export type TranscriptBlock =
  | { type: "msg"; msg: Msg }
  | { type: "work"; items: Msg[] };

export type TurnBlock =
  | { type: "user"; msg: Msg }
  | { type: "approval"; msg: Msg }
  | { type: "assistant"; msg: Msg; workItems: Msg[] }
  | { type: "work"; items: Msg[] }
  | { type: "error"; msg: Msg };

function skip(msg: Msg): boolean {
  if (msg.kind === "command") return true;
  // Status and post-approval audit lines stay out of the chat.
  if (msg.kind === "notice") return true;
  return false;
}

function isWork(msg: Msg): boolean {
  return msg.kind === "tool" || msg.kind === "thinking";
}

/** Merge consecutive thinking + tool rows into one activity strip (legacy). */
export function groupTranscript(messages: Msg[]): TranscriptBlock[] {
  const blocks: TranscriptBlock[] = [];
  for (const msg of messages) {
    if (skip(msg)) continue;
    if (isWork(msg)) {
      const last = blocks[blocks.length - 1];
      if (last && last.type === "work") last.items.push(msg);
      else blocks.push({ type: "work", items: [msg] });
      continue;
    }
    blocks.push({ type: "msg", msg });
  }
  return blocks;
}

/**
 * Group messages into cohesive conversational Turns.
 * All tools, reasoning, and steps for an assistant response belong to that turn,
 * preventing fractured multiple tool rows and vertical gap clutter.
 */
export function groupTurns(messages: Msg[]): TurnBlock[] {
  const blocks: TurnBlock[] = [];
  let pendingWork: Msg[] = [];

  for (const m of messages) {
    if (m.kind === "command" || m.kind === "notice") continue;

    if (m.kind === "thinking" || m.kind === "tool") {
      pendingWork.push(m);
      continue;
    }

    if (m.kind === "status") {
      if (m.ok === false) {
        if (pendingWork.length > 0) {
          blocks.push({ type: "work", items: pendingWork });
          pendingWork = [];
        }
        blocks.push({ type: "error", msg: m });
      }
      continue;
    }

    if (m.kind === "user") {
      if (pendingWork.length > 0) {
        blocks.push({ type: "work", items: pendingWork });
        pendingWork = [];
      }
      blocks.push({ type: "user", msg: m });
      continue;
    }

    if (m.kind === "approval") {
      if (pendingWork.length > 0) {
        blocks.push({ type: "work", items: pendingWork });
        pendingWork = [];
      }
      blocks.push({ type: "approval", msg: m });
      continue;
    }

    if (m.kind === "error") {
      if (pendingWork.length > 0) {
        blocks.push({ type: "work", items: pendingWork });
        pendingWork = [];
      }
      blocks.push({ type: "error", msg: m });
      continue;
    }

    if (m.kind === "assistant") {
      // Discard empty non-streaming assistant messages that occur before tools
      if (!m.text.trim() && !m.streaming) {
        continue;
      }
      const last = blocks[blocks.length - 1];
      if (last && last.type === "assistant" && pendingWork.length === 0) {
        last.msg = { ...last.msg, text: last.msg.text + "\n\n" + m.text };
      } else {
        blocks.push({
          type: "assistant",
          msg: m,
          workItems: pendingWork,
        });
        pendingWork = [];
      }
      continue;
    }
  }

  if (pendingWork.length > 0) {
    blocks.push({ type: "work", items: pendingWork });
  }

  return blocks;
}

export function parseActivityLedger(items: Msg[]): ActivityLedger {
  const all: LedgerEntry[] = [];
  const files: LedgerEntry[] = [];
  const searches: LedgerEntry[] = [];
  const terminal: LedgerEntry[] = [];
  const thoughts: LedgerEntry[] = [];

  for (const m of items) {
    if (m.kind === "thinking") {
      const entry: LedgerEntry = {
        id: m.id,
        category: "thought",
        title: m.text || (m.streaming ? "Thinking…" : "Reasoning"),
        body: m.thinkingBody,
        dur: m.durationMs ?? 0,
        streaming: m.streaming,
      };
      all.push(entry);
      thoughts.push(entry);
      continue;
    }

    if (m.kind === "tool") {
      const tool = m.toolName ?? "";
      const path = (m.preview ?? "").trim();
      const name = path ? path.split("/").pop() || path : tool;
      const dur = m.durationMs ?? 0;

      if (tool === "read_file") {
        const entry: LedgerEntry = {
          id: m.id,
          category: "read",
          title: name,
          sub: path !== name ? path : undefined,
          metric: dur > 0 ? fmtDur(dur) : undefined,
          dur,
          ok: m.ok,
        };
        all.push(entry);
        files.push(entry);
      } else if (tool === "edit_file" || tool === "write_file") {
        const entry: LedgerEntry = {
          id: m.id,
          category: "edit",
          title: name,
          sub: m.summary || (path !== name ? path : undefined),
          metric: dur > 0 ? fmtDur(dur) : undefined,
          dur,
          ok: m.ok,
        };
        all.push(entry);
        files.push(entry);
      } else if (tool === "grep" || tool === "glob" || tool.includes("search")) {
        const entry: LedgerEntry = {
          id: m.id,
          category: "search",
          title: m.preview || m.summary || "codebase",
          sub: m.summary,
          metric: dur > 0 ? fmtDur(dur) : undefined,
          dur,
          ok: m.ok,
        };
        all.push(entry);
        searches.push(entry);
      } else if (tool === "bash") {
        const entry: LedgerEntry = {
          id: m.id,
          category: "bash",
          title: m.bashCommand || m.preview || "bash",
          metric: dur > 0 ? fmtDur(dur) : undefined,
          output: m.output,
          dur,
          ok: m.ok,
          streaming: m.streaming,
        };
        all.push(entry);
        terminal.push(entry);
      } else {
        const entry: LedgerEntry = {
          id: m.id,
          category: "tool",
          title: toolLabel(tool),
          sub: m.preview || m.summary,
          metric: dur > 0 ? fmtDur(dur) : undefined,
          dur,
          ok: m.ok,
        };
        all.push(entry);
      }
    }
  }

  return { all, files, searches, terminal, thoughts };
}
