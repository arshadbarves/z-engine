import { compact, notes, setMode, setModel, submit } from "./commands";
import { draftStore, modeStore, modelStore, submitLocal } from "./events";
import { HERO_EXAMPLES } from "./constants";
import type { PaletteItem } from "./paletteTypes";
import {
  Brain,
  FileText,
  Folder,
  GitBranch,
  GitCompare,
  PanelLeft,
  Plus,
  Settings,
  Shield,
  Sparkles,
  Workflow,
} from "./ui/icons";
import { modLabel } from "./platform";

const MODEL_PRESETS = [
  "anthropic/claude-sonnet-4",
  "anthropic/claude-opus-4",
  "openai/gpt-4.1",
  "openai/o4-mini",
  "google/gemini-2.5-pro",
];

export function paletteActions(opts: {
  newTask: () => void;
  addWorkspace: () => void;
  openWorktree: () => void;
  openDiff: () => void;
  openSettings: () => void;
  toggleSidebar: () => void;
}): PaletteItem[] {
  const modeOrder = ["normal", "accept-edits", "plan"];
  const nextMode =
    modeOrder[(modeOrder.indexOf(modeStore.getSnapshot()) + 1) % modeOrder.length];
  return [
    {
      label: "New chat",
      hint: "Create session",
      keywords: "new task session chat create",
      group: "Actions",
      icon: Plus,
      shortcut: `${modLabel()}N`,
      run: opts.newTask,
    },
    {
      label: "Add workspace…",
      hint: "Open folder",
      keywords: "add open folder workspace project",
      group: "Actions",
      icon: Folder,
      run: opts.addWorkspace,
    },
    {
      label: "New task in git worktree…",
      hint: "Isolated branch",
      keywords: "worktree branch isolate parallel task new",
      group: "Actions",
      icon: GitBranch,
      run: opts.openWorktree,
    },
    {
      label: "Review uncommitted changes",
      hint: "Diff workbench",
      keywords: "diff review changes files git",
      group: "Actions",
      icon: GitCompare,
      run: opts.openDiff,
    },
    {
      label: "Open settings…",
      hint: "Preferences",
      keywords: "settings preferences config permissions mcp cost",
      group: "Actions",
      icon: Settings,
      shortcut: `${modLabel()},`,
      run: opts.openSettings,
    },
    {
      label: "Toggle sidebar",
      hint: "Toggle drawer",
      keywords: "toggle sidebar view drawer",
      group: "Actions",
      icon: PanelLeft,
      shortcut: `${modLabel()}B`,
      run: opts.toggleSidebar,
    },
    {
      label: `Set permission mode · ${nextMode}`,
      hint: "Permissions",
      keywords: "mode permission auto accept plan normal",
      group: "Controls",
      icon: Shield,
      run: () => {
        modeStore.set(nextMode);
        void setMode(nextMode);
      },
    },
    {
      label: "/compact — compact session context",
      hint: "Free tokens",
      keywords: "compact context tokens memory",
      group: "Controls",
      icon: Brain,
      run: () => void compact(),
    },
    {
      label: "/notes — dump durable context notes",
      hint: "Persisted notes",
      keywords: "notes context session memory",
      group: "Controls",
      icon: FileText,
      run: () => void notes(),
    },
    ...MODEL_PRESETS.map((p) => ({
      label: `Switch model · ${p}`,
      hint: "AI Model",
      keywords: `model switch ${p}`,
      group: "Models",
      icon: Sparkles,
      run: () => {
        void setModel(p).then(() => modelStore.set(p));
      },
    })),
    ...HERO_EXAMPLES.map((ex) => ({
      label: ex,
      hint: "Starter task",
      keywords: "task example prompt starter",
      group: "Starters",
      icon: Workflow,
      run: () => {
        draftStore.set("");
        submitLocal(ex);
        void submit(ex).catch(console.error);
      },
    })),
  ];
}
