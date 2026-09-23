import type { SlashCommandInfo } from "../commands/engine";

const ui = (name: string, description: string, argumentHint: string | null = null): SlashCommandInfo => ({
  name,
  description,
  argumentHint,
  source: "builtin",
  kind: "ui",
});

/** GUI-handled commands; used when the engine list is missing them. */
export const LOCAL_UI_COMMANDS: SlashCommandInfo[] = [
  ui("help", "Show commands and keyboard shortcuts"),
  ui("agents", "Open the agents panel"),
  ui("jobs", "Open background jobs"),
  ui("context", "Show context usage by prompt layer"),
  ui("export", "Copy the transcript to the clipboard", "[markdown|json]"),
  ui("clear", "Start a new chat"),
  ui("resume", "Switch to another chat"),
  ui("config", "Open settings"),
  ui("mcp", "Manage MCP servers"),
  ui("permissions", "Manage permission rules"),
  ui("hooks", "Show recent hook runs"),
  ui("memory", "Remember something in AGENTS.md"),
];

/** Engine list first; local UI commands fill the gaps. */
export function mergeCommands(remote: SlashCommandInfo[] | null | undefined): SlashCommandInfo[] {
  const list = remote ?? [];
  const names = new Set(list.map((c) => c.name));
  return [...list, ...LOCAL_UI_COMMANDS.filter((c) => !names.has(c.name))];
}

/** The command word being typed (`/rev` -> `rev`), or null once past the name. */
export function slashQuery(text: string): string | null {
  if (!text.startsWith("/")) return null;
  const rest = text.slice(1);
  if (/\s/.test(rest)) return null;
  return rest.toLowerCase();
}

/** Name prefix matches first, then name substrings; descriptions count from three characters. */
export function filterCommands(commands: SlashCommandInfo[], query: string): SlashCommandInfo[] {
  const q = query.trim().toLowerCase();
  if (!q) return commands;
  const prefix = commands.filter((c) => c.name.toLowerCase().startsWith(q));
  const rest = commands.filter(
    (c) =>
      !prefix.includes(c) &&
      (c.name.toLowerCase().includes(q) || (q.length >= 3 && c.description.toLowerCase().includes(q))),
  );
  return [...prefix, ...rest];
}

export function parseSlash(text: string): { name: string; args: string } | null {
  const match = /^\/([^\s/]+)(?:\s+([\s\S]*))?$/.exec(text.trim());
  if (!match) return null;
  return { name: match[1], args: (match[2] ?? "").trim() };
}

export function findCommand(commands: SlashCommandInfo[], name: string): SlashCommandInfo | null {
  return commands.find((c) => c.name === name) ?? null;
}

export function sourceTag(command: SlashCommandInfo): string {
  if (command.source === "builtin") return command.kind === "ui" ? "APP" : "BUILT-IN";
  return command.source.toUpperCase();
}
