import { HERO_EXAMPLES } from "./constants";
import { MODES } from "./domain/modes";
import type { PaletteItem } from "./paletteTypes";
import { modLabel } from "./platform";
import { compact, exportTranscript, sessions, setMode, submitPrompt } from "./runtime";
import { composer } from "./stores/composer.svelte";
import { ui } from "./stores/ui.svelte";
import { runUiCommand } from "./stores/uiCommands";
import {
  Bot,
  Brain,
  Copy,
  Eye,
  Folder,
  GitBranch,
  GitCompare,
  HelpCircle,
  PanelLeft,
  Plus,
  Settings,
  Shield,
  SquareTerminal,
  Workflow,
} from "./ui/icons";

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
  const action = (item: Omit<PaletteItem, "group">): PaletteItem => ({ ...item, group: "Actions" });
  return [
    action({ label: "New chat", hint: "Create session", keywords: "new task session chat create clear", icon: Plus, shortcut: `${modLabel()}N`, run: opts.newTask }),
    action({ label: "Add workspace…", hint: "Open folder", keywords: "add open folder workspace project", icon: Folder, run: opts.addWorkspace }),
    action({ label: "New task in git worktree…", hint: "Isolated branch", keywords: "worktree branch isolate parallel task new", icon: GitBranch, run: opts.openWorktree }),
    action({ label: "Review session changes", hint: "This chat’s edits", keywords: "diff review changes files git session chat", icon: GitCompare, shortcut: `${modLabel()}D`, run: opts.openDiff }),
    action({ label: "Agents", hint: "Subagents, worktrees, usage", keywords: "agents subagents tree worktree usage cost", icon: Bot, run: () => ui.openWork("agents") }),
    action({ label: "Background jobs", hint: "Shells and agents", keywords: "jobs background shell kill output", icon: SquareTerminal, run: () => ui.openWork("jobs") }),
    action({ label: "Inspect last model request", hint: "Prompt inspector", keywords: "inspect prompt request context tokens system tools", icon: Eye, run: opts.openInspector }),
    action({ label: "Context usage", hint: "/context", keywords: "context tokens memory breakdown window", icon: Brain, run: () => void runUiCommand("context", "") }),
    action({ label: "Export transcript as Markdown", hint: "Copies to clipboard", keywords: "export transcript markdown copy share", icon: Copy, run: () => void exportTranscript("markdown") }),
    action({ label: "Export transcript as JSON", hint: "Copies to clipboard", keywords: "export transcript json copy", icon: Copy, run: () => void exportTranscript("json") }),
    action({ label: "Open settings…", hint: "Preferences", keywords: "settings preferences config permissions mcp", icon: Settings, shortcut: `${modLabel()},`, run: opts.openSettings }),
    action({ label: "Toggle sidebar", hint: "Toggle drawer", keywords: "toggle sidebar view drawer", icon: PanelLeft, shortcut: `${modLabel()}B`, run: opts.toggleSidebar }),
    ...MODES.filter((m) => m.id !== mode && !m.warning).map((m) => ({
      label: `Permission mode · ${m.label}`,
      hint: m.description,
      keywords: `mode permission ${m.id} ${m.label}`,
      group: "Controls",
      icon: Shield,
      run: () => void setMode(m.id),
    })),
    {
      label: "/compact — summarize older history",
      hint: "Free tokens",
      keywords: "compact context tokens memory summarize",
      group: "Controls",
      icon: Brain,
      run: () => void compact(),
    },
    {
      label: "/help — commands and shortcuts",
      hint: "Reference",
      keywords: "help commands keys shortcuts",
      group: "Controls",
      icon: HelpCircle,
      run: () => void runUiCommand("help", ""),
    },
    ...HERO_EXAMPLES.map((example) => ({
      label: example,
      hint: "Starter task",
      keywords: "task example prompt starter",
      group: "Starters",
      icon: Workflow,
      run: () => {
        composer.clear();
        void submitPrompt(example, []);
      },
    })),
  ];
}
