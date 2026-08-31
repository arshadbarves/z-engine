<script lang="ts">
  import Icon, { FolderGit2, GitCompare, MessageSquare, PanelLeft, Search } from "$lib/ui/icons";
  import { isMacPlatform, modLabel } from "$lib/platform";
  import ContextMeter from "./ContextMeter.svelte";
  import LogoMark from "./LogoMark.svelte";
  import UpdateButton from "./UpdateButton.svelte";
  import WindowControlsMaybe from "./WindowControlsMaybe.svelte";

  type Props = {
    workspaceName?: string | null;
    chatTitle?: string | null;
    titleHint?: string;
    diffOpen: boolean;
    sidebarOpen: boolean;
    onToggleSidebar: () => void;
    onPalette: () => void;
    onToggleDiff: () => void;
    onInspectPrompt?: () => void;
  };

  let {
    workspaceName,
    chatTitle,
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
      aria-label="Toggle sidebar"
    >
      <Icon icon={PanelLeft} size={14} strokeWidth={1.8} />
    </button>
    <button
      type="button"
      class="icon-btn"
      title={`Search & commands (${modLabel()}K)`}
      onclick={onPalette}
      aria-label="Search and commands"
    >
      <Icon icon={Search} size={14} strokeWidth={1.8} />
    </button>
  </div>

  <div class="topbar-center" data-tauri-drag-region>
    <div
      class="topbar-session-info"
      data-tauri-drag-region
      title={titleHint || `${workspaceName || ""}${workspaceName && chatTitle ? " / " : ""}${chatTitle || ""}`}
    >
      {#if workspaceName}
        <span class="topbar-ws-tag" data-tauri-drag-region>
          <Icon icon={FolderGit2} size={12} strokeWidth={1.8} class="topbar-ws-icon" />
          <span class="topbar-ws-text" data-tauri-drag-region>{workspaceName}</span>
        </span>
      {/if}
      {#if workspaceName && chatTitle}
        <span class="topbar-crumb-sep" data-tauri-drag-region>/</span>
      {/if}
      {#if chatTitle}
        <span class="topbar-chat-title" data-tauri-drag-region>
          <Icon icon={MessageSquare} size={11} strokeWidth={1.8} class="topbar-chat-icon" />
          <span data-tauri-drag-region>{chatTitle}</span>
        </span>
      {/if}
    </div>
  </div>

  <div class="topbar-right" data-tauri-drag-region>
    <UpdateButton />
    <ContextMeter onInspect={onInspectPrompt} />
    <button
      type="button"
      class={`icon-btn${diffOpen ? " active" : ""}`}
      title="Review this chat’s file changes"
      onclick={onToggleDiff}
    >
      <Icon icon={GitCompare} size={14} strokeWidth={1.8} />
    </button>
    <WindowControlsMaybe />
  </div>
</header>
