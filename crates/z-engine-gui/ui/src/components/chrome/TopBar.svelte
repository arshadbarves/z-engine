<script lang="ts">
  import Icon, { FolderGit2, GitCompare, PanelLeft, Search } from "$lib/ui/icons";
  import { isMacPlatform, modLabel } from "$lib/platform";
  import ContextMeter from "./ContextMeter.svelte";
  import LogoMark from "./LogoMark.svelte";
  import UpdateButton from "./UpdateButton.svelte";
  import WindowControlsMaybe from "./WindowControlsMaybe.svelte";

  type Props = {
    title: string;
    titleHint?: string;
    diffOpen: boolean;
    sidebarOpen: boolean;
    onToggleSidebar: () => void;
    onPalette: () => void;
    onToggleDiff: () => void;
    onInspectPrompt?: () => void;
  };

  let {
    title,
    titleHint,
    diffOpen,
    sidebarOpen,
    onToggleSidebar,
    onPalette,
    onToggleDiff,
    onInspectPrompt,
  }: Props = $props();

  const isMac = isMacPlatform();
</script>

<header class="app-topbar" data-tauri-drag-region>
  <div class="topbar-left" data-tauri-drag-region>
    {#if !isMac}
      <div class="topbar-brand" title="Z Engine">
        <LogoMark size={15} />
        <span>Z Engine</span>
      </div>
    {/if}
    <button
      type="button"
      class="icon-btn"
      title={sidebarOpen ? `Hide sidebar (${modLabel()}B)` : `Show sidebar (${modLabel()}B)`}
      onclick={onToggleSidebar}
    >
      <Icon icon={PanelLeft} size={14} strokeWidth={1.8} />
    </button>
    <button
      type="button"
      class="topbar-workspace-pill"
      title={titleHint || "Open quick switcher"}
      onclick={onPalette}
    >
      <Icon icon={FolderGit2} size={13} class="topbar-ws-icon" strokeWidth={1.8} />
      <span class="topbar-ws-name">{title}</span>
    </button>
  </div>

  <div class="topbar-center" data-tauri-drag-region>
    <button
      type="button"
      class="topbar-search-bar"
      title={`Search & commands (${modLabel()}K)`}
      onclick={onPalette}
    >
      <Icon icon={Search} size={13} class="topbar-search-icon" strokeWidth={1.8} />
      <span class="topbar-search-text">Search chats, workspaces, commands…</span>
      <kbd class="topbar-search-kbd">{modLabel()}K</kbd>
    </button>
  </div>

  <div class="topbar-right" data-tauri-drag-region>
    <UpdateButton />
    <ContextMeter onInspect={onInspectPrompt} />
    <button
      type="button"
      class={`icon-btn${diffOpen ? " active" : ""}`}
      title="Review uncommitted git changes vs HEAD"
      onclick={onToggleDiff}
    >
      <Icon icon={GitCompare} size={14} strokeWidth={1.8} />
    </button>
    <WindowControlsMaybe />
  </div>
</header>
