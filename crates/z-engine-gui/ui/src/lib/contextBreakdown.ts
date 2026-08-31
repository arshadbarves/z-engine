import type { Msg } from "./events";

export interface CtxSlice {
  id: string;
  label: string;
  tokens: number;
  color: string;
}

export interface CtxBreakdown {
  slices: CtxSlice[];
  used: number;
  remaining: number;
  max: number;
}

function toks(text: string): number {
  return Math.ceil(text.length / 4);
}

/** Cursor-style context buckets. Provider `promptTokens` is the source of
 * truth when present; otherwise we estimate from the transcript so a
 * restored session still has a chart. */
export function contextBreakdown(
  messages: Msg[],
  promptTokens: number,
  maxTokens: number,
): CtxBreakdown {
  const max = Math.max(1, maxTokens);
  let conversation = 0;
  let files = 0;
  const toolNames = new Set<string>();
  for (const m of messages) {
    if (m.kind === "user" || m.kind === "assistant") conversation += toks(m.text);
    if (m.kind === "thinking") conversation += toks(m.thinkingBody ?? "");
    if (m.kind === "tool") {
      toolNames.add(m.toolName ?? "tool");
      files += toks(m.preview ?? "");
      files += toks(m.output ?? "");
    }
  }
  const system = 1800;
  const tools = Math.max(900, toolNames.size * 280);
  const rules = 420;
  const raw = [
    { id: "system", label: "System", tokens: system, color: "#8b8b96" },
    { id: "tools", label: "Tools", tokens: tools, color: "#a78bfa" },
    { id: "rules", label: "Rules", tokens: rules, color: "#7c85e0" },
    { id: "chat", label: "Chat", tokens: conversation, color: "#f0a090" },
    { id: "files", label: "Files", tokens: files, color: "#7dd3fc" },
  ];
  const estimated = raw.reduce((n, s) => n + s.tokens, 0);
  const target = promptTokens > 0 ? promptTokens : estimated;
  const scale = estimated > 0 ? target / estimated : 1;
  const slices = raw.map((s) => ({ ...s, tokens: Math.max(0, Math.round(s.tokens * scale)) }));
  const used = slices.reduce((n, s) => n + s.tokens, 0);
  return { slices, used, remaining: Math.max(0, max - used), max };
}

export function estimatePromptTokens(messages: Msg[]): number {
  return contextBreakdown(messages, 0, 1).used;
}

export function estimateCompletionTokens(messages: Msg[]): number {
  return messages
    .filter((m) => m.kind === "assistant")
    .reduce((n, m) => n + toks(m.text), 0);
}
