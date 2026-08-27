import type { Msg } from "./events";

/** Human label per tool (Claude Code style). */
export function toolLabel(name: string): string {
  const map: Record<string, string> = {
    read_file: "Read",
    write_file: "Write",
    edit_file: "Edit",
    bash: "Bash",
    grep: "Grep",
    glob: "Glob",
    task: "Task",
    update_context_notes: "Notes",
    go_to_definition: "Definition",
    find_references: "References",
    lsp_diagnostics: "Diagnostics",
  };
  if (map[name]) return map[name];
  const words = name.replace(/[-_]/g, " ").split(" ");
  return words.map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(" ");
}

/** Core summaries echo the tool name (`bash (0): cmd…`); the card already
 * shows the label, so strip the prefix. */
export function cleanSummary(toolName: string | undefined, summary: string): string {
  let s = summary;
  if (toolName) {
    s = s.replace(new RegExp(`^${toolName}\\s+\\(\\d+\\):\\s*`), "");
    s = s.replace(new RegExp(`^${toolName}\\s*:\\s*`), "");
  }
  s = s.replace(/^\(timed out\):\s*/, "");
  return s || summary;
}

export function fmtDur(ms: number): string {
  return ms >= 60_000 ? `${(ms / 60_000).toFixed(1)}m` : `${(ms / 1000).toFixed(1)}s`;
}

const FAMILY: Record<string, string> = {
  read_file: "Read",
  write_file: "Write",
  edit_file: "Edit",
  grep: "Search",
  glob: "Search",
  bash: "Bash",
  task: "Task",
};

/** One-line brief for a collapsed work group: `Thought · Read 4 · 12s`. */
export function activityBrief(items: Msg[]): string {
  const current = [...items].reverse().find((t) => t.streaming);
  if (current) {
    if (current.kind === "thinking") return "Thinking…";
    const done = items.filter((t) => !t.streaming).length;
    const label = toolLabel(current.toolName ?? "");
    const arg = (current.preview ?? "").trim();
    return `${done + 1}/${items.length} · ${label}${arg ? ` ${arg}` : ""}`;
  }
  const thinks = items.filter((i) => i.kind === "thinking").length;
  const counts = new Map<string, number>();
  let failed = 0;
  let dur = 0;
  for (const t of items) {
    if (t.kind !== "tool") continue;
    const fam = FAMILY[t.toolName ?? ""] ?? "Step";
    counts.set(fam, (counts.get(fam) ?? 0) + 1);
    if (t.ok === false) failed += 1;
    dur += t.durationMs ?? 0;
  }
  const parts = [...counts.entries()].map(([k, n]) => (n === 1 ? k : `${k} ${n}`));
  if (thinks > 0) parts.unshift(thinks === 1 ? "Thought" : `Thought ${thinks}`);
  const time = dur > 0 ? ` · ${fmtDur(dur)}` : "";
  const fail = failed ? ` · ${failed} failed` : "";
  return `${parts.join(" · ") || "Work"}${time}${fail}`;
}
