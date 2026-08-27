import type { PromptInspect, PromptPart, PromptTool } from "./commands";

export interface PromptLayer {
  order: number;
  kind: "message" | "tool";
  label: string;
  role: string;
  tokens: number;
  share: number;
}

export interface PromptInsights {
  layers: PromptLayer[];
  largest: { name: string; tokens: number; share: number };
  stablePrefix: string;
  volatileTail: string;
  hints: string[];
}

function layer(
  order: number,
  kind: PromptLayer["kind"],
  label: string,
  role: string,
  tokens: number,
  total: number,
): PromptLayer {
  return {
    order,
    kind,
    label,
    role,
    tokens,
    share: total > 0 ? tokens / total : 0,
  };
}

/** Case-study view of a prompt: send order, budget share, optimization hints. */
export function promptInsights(snap: PromptInspect): PromptInsights {
  const total = Math.max(1, snap.totalTokens);
  const layers: PromptLayer[] = [
    ...snap.messages.map((m: PromptPart, i) =>
      layer(i + 1, "message", m.label, m.role, m.tokens, total),
    ),
    ...snap.tools.map((t: PromptTool, i) =>
      layer(snap.messages.length + i + 1, "tool", t.name, "tool def", t.tokens, total),
    ),
  ];
  const ranked = [...layers].sort((a, b) => b.tokens - a.tokens);
  const biggest = ranked[0] ?? layer(1, "message", "empty", "none", 0, 1);
  const msgLabels = snap.messages.map((m) => m.label);
  const hints: string[] = [];
  const toolTok = snap.tools.reduce((n, t) => n + t.tokens, 0);
  if (!snap.sent) {
    hints.push("This snapshot has not been sent this process — restore or send a turn to capture the live request.");
  }
  if (toolTok / total > 0.35) {
    hints.push(
      `Tool schemas are ${Math.round((toolTok / total) * 100)}% of the request. Drop unused tools or shorten descriptions to cut this.`,
    );
  }
  const repo = snap.messages.find((m) => m.label === "Repo map");
  if (repo && repo.tokens / total > 0.15) {
    hints.push("Repo map is a large volatile block. Narrow tracked files or lower the map budget.");
  }
  const toolResults = snap.messages.filter((m) => m.role === "tool");
  if (toolResults.length >= 4) {
    hints.push(
      `${toolResults.length} tool results are still in the working set. Mark droppable output or compact to shrink the tail.`,
    );
  }
  const sys = snap.messages.find((m) => m.label === "System");
  if (sys) {
    hints.push(
      "Keep the system prefix byte-stable (no clocks or counters) so the provider cache can reuse it across turns.",
    );
  }
  if (ranked[0] && ranked[0].share > 0.4) {
    hints.push(`Largest sink: ${ranked[0].label} (~${Math.round(ranked[0].share * 100)}%). Start optimization there.`);
  }
  return {
    layers,
    largest: { name: biggest.label, tokens: biggest.tokens, share: biggest.share },
    stablePrefix: msgLabels[0] ?? "System",
    volatileTail: msgLabels.filter((l) => l !== "System").slice(-2).join(" → ") || "working set",
    hints,
  };
}
