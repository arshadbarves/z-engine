<script lang="ts">
  import LogoMark from "../chrome/LogoMark.svelte";
  import Sidebar from "./Sidebar.svelte";
  import { modLabel } from "$lib/platform";
  import type { SessionActivity } from "$lib/types";
  import type { SessionEntry } from "$lib/util";
  import { Icon, Plus, Settings } from "$lib/ui/icons";

  type Props = {
    sessions: SessionEntry[];
    workspaces: string[];
    activeWorkspace: string | null;
    activeUlid: string;
    activity: Record<string, SessionActivity>;
    version?: string;
    onNewChat: () => void;
    onOpen: (path: string, projectRoot?: string | null) => void;
    onDelete: (path: string) => void;
    onAddWorkspace: () => void;
    onRemoveWorkspace: (root: string) => void;
    onActivateWorkspace: (root: string | null) => void;
    onSettings: () => void;
  };

  let {
    sessions,
    workspaces,
    activeWorkspace,
    activeUlid,
    activity,
    version,
    onNewChat,
    onOpen,
    onDelete,
    onAddWorkspace,
    onRemoveWorkspace,
    onActivateWorkspace,
    onSettings,
  }: Props = $props();
</script>

<div class="sidebar-slot">
  <aside class="sidebar">
    <div class="sidebar-top-bar">
      <div class="sidebar-brand-pill">
        <LogoMark size={16} />
        <span class="sidebar-brand-text">Z Engine</span>
      </div>
    </div>

    <button class="sidebar-new-chat-btn" onclick={onNewChat} type="button">
      <span class="btn-left">
        <Icon icon={Plus} size={13} strokeWidth={2} />
        <span>New chat</span>
      </span>
      <kbd class="sidebar-kbd">{modLabel()}N</kbd>
    </button>

    <Sidebar
      {sessions}
      {workspaces}
      {activeWorkspace}
      {activeUlid}
      {activity}
      {onOpen}
      {onDelete}
      {onAddWorkspace}
      {onRemoveWorkspace}
      {onActivateWorkspace}
    />

    <div class="sidebar-footer">
      <button class="sidebar-footer-btn" title="Open Settings" onclick={onSettings} type="button">
        <Icon icon={Settings} size={13} strokeWidth={1.8} />
        <span class="footer-label">Settings</span>
      </button>
      <span class="footer-version-tag">{version ? `v${version}` : ""}</span>
    </div>
  </aside>
</div>
