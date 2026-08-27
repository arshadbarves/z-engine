import type { Msg } from "./events";

export type TranscriptBlock =
  | { type: "msg"; msg: Msg }
  | { type: "work"; items: Msg[] };

function skip(msg: Msg): boolean {
  if (msg.kind === "command") return true;
  // Status and post-approval audit lines stay out of the chat.
  if (msg.kind === "notice") return true;
  return false;
}

function isWork(msg: Msg): boolean {
  return msg.kind === "tool" || msg.kind === "thinking";
}

/** Merge consecutive thinking + tool rows into one activity strip. */
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
