<script lang="ts">
  import Sidebar from "./Sidebar.svelte";
  import LogoMark from "../chrome/LogoMark.svelte";
  import Icon, { Plus } from "$lib/ui/icons";
  import { modLabel } from "$lib/platform";
  import type { SessionActivity } from "$lib/types";
  import type { SessionEntry } from "$lib/util";

  type Props = {
    sessions: SessionEntry[];
    workspaces: string[];
    activeWorkspace: string | null;
    activeUlid: string;
    activity: Record<string, SessionActivity>;
    version?: string;
    onOpen: (path: string, projectRoot?: string | null) => void;
    onDelete: (path: string) => void;
    onAddWorkspace: () => void;
    onRemoveWorkspace: (root: string) => void;
    onActivateWorkspace: (root: string | null) => void;
    onNewChat?: () => void;
  };

  let {
    sessions,
    workspaces,
    activeWorkspace,
    activeUlid,
    activity,
    version,
    onOpen,
    onDelete,
    onAddWorkspace,
    onRemoveWorkspace,
    onActivateWorkspace,
    onNewChat,
  }: Props = $props();
</script>

<div class="sidebar-slot">
  <aside class="sidebar">
    <div class="sidebar-header-deck">
      <div class="sidebar-brand-row">
        <div class="sidebar-brand">
          <div class="sidebar-brand-icon">
            <LogoMark size={14} />
          </div>
          <span class="sidebar-brand-name">Z Engine</span>
          <span class="brand-beta-pill">BETA</span>
        </div>
      </div>
      {#if onNewChat}
        <button
          type="button"
          class="sidebar-new-chat-btn"
          onclick={onNewChat}
          aria-label="New chat"
        >
          <div class="btn-left">
            <Icon icon={Plus} size={13} strokeWidth={2.4} />
            <span class="sidebar-new-chat-text">New chat</span>
          </div>
          <kbd class="sidebar-new-chat-kbd">{modLabel()}N</kbd>
        </button>
      {/if}
    </div>

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

    {#if version}
      <div class="sidebar-footer">
        <span class="footer-version-tag">v{version}</span>
      </div>
    {/if}
  </aside>
</div>
