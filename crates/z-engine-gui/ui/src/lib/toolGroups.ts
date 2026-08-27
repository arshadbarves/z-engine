import type { Msg } from "./events";
import { toolLabel } from "./toolUi";

const FAMILY: Record<string, string> = {
  read_file: "Read",
  write_file: "Write",
  edit_file: "Edit",
  grep: "Search",
  glob: "Search",
  bash: "Bash",
  task: "Task",
};

const NOUN: Record<string, [string, string]> = {
  Read: ["file", "files"],
  Write: ["file", "files"],
  Edit: ["file", "files"],
  Search: ["search", "searches"],
  Bash: ["command", "commands"],
  Task: ["task", "tasks"],
};

export type WorkPart =
  | { type: "reason"; text: string; msg: Msg }
  | { type: "group"; family: string; tools: Msg[] };

export interface PathPill {
  label: string;
  count: number;
}

function firstSentence(raw: string): string {
  const t = raw.replace(/^✻\s*(thinking…|thought)[^\n]*\n?/i, "").trim();
  if (!t) return "";
  const line = t.split(/\n/)[0]?.trim() ?? "";
  const cut = line.match(/^(.{1,140}?[.!?])(\s|$)/);
  return (cut?.[1] ?? line).slice(0, 160);
}

export function splitWork(items: Msg[]): WorkPart[] {
  const parts: WorkPart[] = [];
  for (const m of items) {
    if (m.kind === "thinking") {
      parts.push({ type: "reason", text: firstSentence(m.thinkingBody ?? m.text), msg: m });
      continue;
    }
    if (m.kind !== "tool") continue;
    const family = FAMILY[m.toolName ?? ""] ?? toolLabel(m.toolName ?? "Step");
    const last = parts[parts.length - 1];
    if (last && last.type === "group" && last.family === family) last.tools.push(m);
    else parts.push({ type: "group", family, tools: [m] });
  }
  return parts;
}

export function familyTitle(family: string, n: number): string {
  const [one, many] = NOUN[family] ?? ["step", "steps"];
  const noun = n === 1 ? one : many;
  return `${family} ${n} ${noun}`;
}

export function toolPath(m: Msg): string {
  const p = (m.preview ?? "").trim();
  if (!p) return "";
  return p.split(/\s+/)[0] ?? p;
}

export function pathPills(paths: string[]): PathPill[] {
  const counts = new Map<string, number>();
  for (const raw of paths) {
    const p = raw.replace(/\\/g, "/").replace(/^\.\//, "");
    if (!p) continue;
    const parts = p.split("/").filter(Boolean);
    const key = parts.length > 1 ? `${parts[0]}/` : (parts[0] ?? p);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return [...counts.entries()].map(([label, count]) => ({ label, count }));
}

export function groupSummary(family: string, tools: Msg[]): string {
  const resolved = tools.filter((t) => t.ok !== false && !t.streaming).length;
  const names = pathPills(tools.map(toolPath))
    .slice(0, 3)
    .map((p) => p.label.replace(/\/$/, ""));
  const tail = names.length ? ` · ${names.join(", ")}` : "";
  if (family === "Bash") {
    return `${resolved} ran${tail}`;
  }
  return `${resolved} resolved${tail}`;
}
