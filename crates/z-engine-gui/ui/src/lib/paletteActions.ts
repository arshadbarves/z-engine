import { compact, notes, setMode, setModel, submit } from "./commands";
import { draftStore, modeStore, modelStore, submitLocal } from "./events";
import { HERO_EXAMPLES } from "./constants";
import type { PaletteItem } from "../components/CommandPalette";
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
    { label: "New chat", hint: "session", keywords: "new task session chat", run: opts.newTask },
    {
      label: "Add workspace…",
      hint: "project",
      keywords: "add open folder workspace project",
      run: opts.addWorkspace,
    },
    {
      label: "New task in git worktree…",
      hint: "isolated branch",
      keywords: "worktree branch isolate parallel task new",
      run: opts.openWorktree,
    },
    {
      label: "Review changes",
      hint: "diff panel",
      keywords: "diff review changes files git",
      run: opts.openDiff,
    },
    { label: "/compact — force context compaction", hint: "context", keywords: "compact context", run: () => void compact() },
    { label: "/notes — dump durable context notes", hint: "notes", keywords: "notes context", run: () => void notes() },
    {
      label: "Open settings…",
      hint: "app",
      keywords: "settings preferences config permissions mcp cost",
      run: opts.openSettings,
    },
    {
      label: `Set permission mode · ${nextMode}`,
      hint: "mode",
      keywords: "mode permission auto accept plan normal",
      run: () => {
        modeStore.set(nextMode);
        void setMode(nextMode);
      },
    },
    ...MODEL_PRESETS.map((p) => ({
      label: `Switch model · ${p}`,
      hint: "model",
      keywords: `model switch ${p}`,
      run: () => {
        void setModel(p).then(() => modelStore.set(p));
      },
    })),
    {
      label: "Toggle sidebar",
      hint: `${modLabel()}B`,
      keywords: "toggle sidebar view",
      run: opts.toggleSidebar,
    },
    ...HERO_EXAMPLES.map((ex) => ({
      label: ex,
      hint: "start a task",
      keywords: "task example prompt",
      run: () => {
        draftStore.set("");
        submitLocal(ex);
        void submit(ex).catch(console.error);
      },
    })),
  ];
}
