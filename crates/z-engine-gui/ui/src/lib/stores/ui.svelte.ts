import type { StageView } from "../domain/stage";

export type SettingsTab =
  | "models"
  | "providers"
  | "permissions"
  | "hooks"
  | "extensions"
  | "mcp"
  | "verification"
  | "memory"
  | "advanced"
  | "appearance"
  | "about";
export type WorkTab = "agents" | "jobs";
export type DiffLayout = "unified" | "split";

const DIFF_LAYOUT_KEY = "z-engine.diffLayout";

function storedDiffLayout(): DiffLayout {
  try {
    return globalThis.localStorage?.getItem(DIFF_LAYOUT_KEY) === "split" ? "split" : "unified";
  } catch {
    return "unified";
  }
}

/** App chrome state (overlays, panels, sidebar, stage page). UI-only; no engine I/O. */
class UiStore {
  sidebarOpen = $state(true);
  /** The page the main stage is asked for; `stageFor` resolves what it shows. */
  view = $state<StageView>("home");
  paletteOpen = $state(false);
  /** Palette opened by `/resume`: chats only. */
  paletteSessionsOnly = $state(false);
  settingsOpen = $state(false);
  settingsTab = $state<SettingsTab>("providers");
  /** A setting to scroll to and highlight when Settings opens (deep search). */
  settingsFocus = $state<string | null>(null);
  inspectOpen = $state(false);
  diffOpen = $state(false);
  /** The diff review takes the whole stage instead of docking beside the chat. */
  diffExpanded = $state(false);
  /** A file the diff should select when it opens, and the scope to show it in. */
  diffFocus = $state<string | null>(null);
  diffScope = $state<"session" | "git" | null>(null);
  /** Unified or side-by-side; remembered on this machine. */
  diffLayout = $state<DiffLayout>(storedDiffLayout());
  worktreeOpen = $state(false);
  workPanel = $state<WorkTab | null>(null);
  /** Agent whose transcript the work panel shows. */
  agentTranscript = $state<string | null>(null);

  togglePalette() {
    this.paletteSessionsOnly = false;
    this.paletteOpen = !this.paletteOpen;
  }

  openPalette(sessionsOnly = false) {
    this.paletteSessionsOnly = sessionsOnly;
    this.paletteOpen = true;
  }

  /** Without a page, Settings opens where it was last left. */
  openSettings(tab: SettingsTab | null = null, focus: string | null = null) {
    if (tab) this.settingsTab = tab;
    this.settingsFocus = focus;
    this.settingsOpen = true;
  }

  openDiff(file: string | null = null, scope: "session" | "git" | null = null) {
    this.diffFocus = file;
    this.diffScope = scope;
    this.diffOpen = true;
  }

  toggleDiff() {
    this.diffOpen = !this.diffOpen;
    if (!this.diffOpen) this.diffExpanded = false;
  }

  closeDiff() {
    this.diffOpen = false;
    this.diffExpanded = false;
  }

  setDiffLayout(layout: DiffLayout) {
    this.diffLayout = layout;
    try {
      globalThis.localStorage?.setItem(DIFF_LAYOUT_KEY, layout);
    } catch {
      /* private mode: remember for this run only */
    }
  }

  openWork(tab: WorkTab, agentId: string | null = null) {
    this.worktreeOpen = false;
    this.workPanel = tab;
    this.agentTranscript = agentId;
  }

  toggleWork(tab: WorkTab = "agents") {
    if (this.workPanel) this.closeWork();
    else this.openWork(tab);
  }

  closeWork() {
    this.workPanel = null;
    this.agentTranscript = null;
  }

  openWorktree() {
    this.worktreeOpen = true;
  }
}

export const ui = new UiStore();
