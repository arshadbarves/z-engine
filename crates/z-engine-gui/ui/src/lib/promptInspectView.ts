import type { PromptInspect, PromptPart, PromptTool } from "./commands";
import { promptInsights } from "./promptInsights";

export type InspectRow =
  | { key: string; kind: "msg"; part: PromptPart }
  | { key: string; kind: "tool"; tool: PromptTool };

export type ContextCategory = "instructions" | "project" | "conversation" | "capabilities";

export function categorizeRow(row: InspectRow): ContextCategory {
  if (row.kind === "tool") return "capabilities";
  const label = row.part.label.toLowerCase();
  const role = row.part.role.toLowerCase();
  if (
    label.includes("repo") ||
    label.includes("agents") ||
    label.includes("map") ||
    label.includes("context") ||
    label.includes("note")
  ) {
    return "project";
  }
  if (role === "system") return "instructions";
  return "conversation";
}

export function categoryMeta(cat: ContextCategory): { label: string; desc: string; color: string } {
  switch (cat) {
    case "instructions":
      return {
        label: "Instructions",
        desc: "Core operating rules and behavioral guidelines given to the assistant.",
        color: "#38bdf8",
      };
    case "project":
      return {
        label: "Project Knowledge",
        desc: "Workspace structure, repository map, and custom AGENTS.md rules.",
        color: "#00d68f",
      };
    case "conversation":
      return {
        label: "Conversation",
        desc: "Recent messages and tool results exchanged in this session.",
        color: "#a78bfa",
      };
    case "capabilities":
      return {
        label: "Capabilities",
        desc: "Tools the assistant is authorized to execute (files, terminal, search).",
        color: "#f5a623",
      };
  }
}

export function inspectRows(snap: PromptInspect): InspectRow[] {
  return [
    ...snap.messages.map((part, i) => ({ key: `m-${i}`, kind: "msg" as const, part })),
    ...snap.tools.map((tool, i) => ({ key: `t-${i}`, kind: "tool" as const, tool })),
  ];
}

export function inspectBody(row: InspectRow): string {
  if (row.kind === "msg") return row.part.content;
  return `${row.tool.description}\n\n${row.tool.schema}`;
}

export function inspectCopyText(snap: PromptInspect): string {
  const ins = promptInsights(snap);
  const chunks = [
    `model: ${snap.model}`,
    snap.sent ? "sent: yes" : "sent: preview / reconstructed",
    `tokens ≈ ${snap.totalTokens}`,
    `largest: ${ins.largest.name} (${Math.round(ins.largest.share * 100)}%)`,
    `stable prefix: ${ins.stablePrefix}`,
    `volatile tail: ${ins.volatileTail}`,
    "",
    "## Order (wire)",
    ...ins.layers.map(
      (l) => `${l.order}. ${l.label} [${l.role}] ~${l.tokens} tok (${Math.round(l.share * 100)}%)`,
    ),
    "",
    "## Hints",
    ...ins.hints.map((h) => `- ${h}`),
    "",
  ];
  for (const m of snap.messages) {
    chunks.push(`## ${m.label} (${m.role}, ~${m.tokens} tok)`, m.content, "");
  }
  if (snap.tools.length) {
    chunks.push("## Tools");
    for (const t of snap.tools) {
      chunks.push(`### ${t.name} (~${t.tokens} tok)`, t.description, t.schema, "");
    }
  }
  return chunks.join("\n");
}

export function pct(n: number): string {
  return `${Math.round(n * 100)}%`;
}
