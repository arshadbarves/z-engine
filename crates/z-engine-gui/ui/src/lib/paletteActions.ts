import { MODES } from "./domain/modes";
import { SETTING_ENTRIES } from "./domain/settings/searchIndex";
import type { PaletteItem } from "./paletteTypes";
import { modLabel } from "./platform";
import { compact, exportTranscript, sessions, setMode } from "./runtime";
import { goHome, showInbox } from "./stores/app-actions";
import { ui } from "./stores/ui.svelte";
import { runUiCommand } from "./stores/uiCommands";
import {
  Bot,
  Brain,
  Copy,
  Eye,
  FolderPlus,
  GitBranch,
  GitCompare,
  HelpCircle,
  Home,
  Inbox,
  PanelLeft,
  Plus,
  Settings,
  Shield,
  SquareTerminal,
} from "./ui/icons";

/** Everything the palette can do, in plain words; settings appear once you type. */
export function paletteActions(opts: {
  newTask: () => void;
  addWorkspace: () => void;
  openWorktree: () => void;
  openDiff: () => void;
  openSettings: () => void;
  openInspector: () => void;
  toggleSidebar: () => void;
}): PaletteItem[] {
  const mode = sessions.active?.mode ?? "default";
  const mod = modLabel();
  const go = (item: Omit<PaletteItem, "group">): PaletteItem => ({ ...item, group: "Go to" });
  const act = (item: Omit<PaletteItem, "group">): PaletteItem => ({ ...item, group: "Actions" });
  const chat = (item: Omit<PaletteItem, "group">): PaletteItem => ({ ...item, group: "This chat" });
  return [
    act({ label: "New chat", keywords: "new task session create start", icon: Plus, shortcut: `${mod}N`, run: opts.newTask }),
    act({ label: "New chat in a worktree…", hint: "On its own branch", keywords: "worktree branch isolate parallel", icon: GitBranch, run: opts.openWorktree }),
    act({ label: "Add a project…", hint: "Choose a folder", keywords: "add open folder workspace project", icon: FolderPlus, run: opts.addWorkspace }),
    go({ label: "Home", keywords: "home start projects", icon: Home, run: () => goHome() }),
    go({ label: "Inbox", hint: "Approvals, finished chats, notices", keywords: "inbox activity notifications approvals", icon: Inbox, run: () => showInbox() }),
    go({ label: "Settings", keywords: "settings preferences config", icon: Settings, shortcut: `${mod},`, run: opts.openSettings }),
    go({ label: "Show or hide the sidebar", keywords: "toggle sidebar", icon: PanelLeft, shortcut: `${mod}B`, run: opts.toggleSidebar }),
    chat({ label: "Review changes", hint: "What this chat changed", keywords: "diff review changes files git", icon: GitCompare, shortcut: `${mod}D`, run: opts.openDiff }),
    chat({ label: "Agents", hint: "Helpers and work to apply", keywords: "agents subagents helpers worktree usage cost", icon: Bot, run: () => ui.openWork("agents") }),
    chat({ label: "Background jobs", keywords: "jobs background shell stop output", icon: SquareTerminal, run: () => ui.openWork("jobs") }),
    chat({ label: "Inspect the prompt", hint: "The last request, part by part", keywords: "inspect prompt request context tokens system tools", icon: Eye, run: opts.openInspector }),
    chat({ label: "Context usage", keywords: "context tokens window breakdown /context", icon: Brain, run: () => void runUiCommand("context", "") }),
    chat({ label: "Compact the conversation", hint: "Summarize older messages", keywords: "compact summarize free tokens /compact", icon: Brain, run: () => void compact() }),
    chat({ label: "Copy the chat as Markdown", keywords: "export transcript markdown share", icon: Copy, run: () => void exportTranscript("markdown") }),
    chat({ label: "Copy the chat as JSON", keywords: "export transcript json", icon: Copy, run: () => void exportTranscript("json") }),
    ...MODES.filter((m) => m.id !== mode && !m.warning).map((m) => ({
      label: `Switch to ${m.label}`,
      hint: m.description,
      keywords: `mode permission ${m.id}`,
      group: "Permission mode",
      icon: Shield,
      run: () => void setMode(m.id),
    })),
    {
      label: "Commands and shortcuts",
      keywords: "help commands keys shortcuts /help",
      group: "Help",
      icon: HelpCircle,
      run: () => void runUiCommand("help", ""),
    },
  ];
}

/** Single settings, found by name or by the words people use; they open at their row. */
export function paletteSettings(): PaletteItem[] {
  return SETTING_ENTRIES.map((entry) => ({
    label: entry.title,
    keywords: `setting ${entry.words}`,
    group: "Settings",
    icon: Settings,
    run: () => ui.openSettings(entry.tab, entry.key),
  }));
}
