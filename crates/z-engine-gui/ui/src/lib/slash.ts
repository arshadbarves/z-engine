import { listSlashCommands, type SlashCommandInfo } from "./commands";

export interface SlashCommand {
  name: string;
  desc: string;
  /** Set for user-defined commands (`.z-engine/commands/*.md`). */
  custom?: boolean;
}

export const SLASH_COMMANDS: SlashCommand[] = [
  { name: "compact", desc: "Force context compaction now" },
  { name: "notes", desc: "Dump durable context notes" },
  { name: "help", desc: "Show commands and keys" },
  { name: "cost", desc: "Session token totals and cost" },
  { name: "status", desc: "Model · mode · session · tokens" },
];

/** Custom commands discovered from disk; refreshed per session open. */
let customCommands: SlashCommandInfo[] = [];

export async function refreshCustomCommands() {
  try {
    customCommands = await listSlashCommands();
  } catch {
    customCommands = [];
  }
}

export function getCustomCommands(): SlashCommandInfo[] {
  return customCommands;
}

function allCommands(): SlashCommand[] {
  return [
    ...SLASH_COMMANDS,
    ...customCommands.map((c) => ({
      name: c.name,
      desc: `${c.description}${c.source === "global" ? " · global" : ""}`,
      custom: true,
    })),
  ];
}

/** If the input is a slash query (starts with `/`), return the filtered
 * command list; otherwise null. Empty query after `/` returns all.
 * A trailing space means the command word is complete — stop filtering. */
export function filterSlash(input: string): SlashCommand[] | null {
  if (!input.startsWith("/")) return null;
  const rest = input.slice(1);
  if (rest.endsWith(" ") || rest.includes(" ")) return [];
  const q = rest.trim().toLowerCase();
  return allCommands().filter((c) => c.name.startsWith(q));
}
