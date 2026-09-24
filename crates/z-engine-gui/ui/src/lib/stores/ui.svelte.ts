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

/** App chrome state (overlays, panels, sidebar). UI-only; no engine I/O. */
class UiStore {
  sidebarOpen = $state(true);
  paletteOpen = $state(false);
  /** Palette opened by `/resume`: chats only. */
  paletteSessionsOnly = $state(false);
  settingsOpen = $state(false);
  settingsTab = $state<SettingsTab>("providers");
  inspectOpen = $state(false);
  diffOpen = $state(false);
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

  openSettings(tab: SettingsTab = "providers") {
    this.settingsTab = tab;
    this.settingsOpen = true;
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
    this.closeWork();
    this.worktreeOpen = true;
  }
}

export const ui = new UiStore();
