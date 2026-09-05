<script lang="ts">
  import Icon, {
    FolderGit2,
    GitCompare,
    MessageSquare,
    PanelLeft,
    Plus,
    Search,
    Settings,
  } from "$lib/ui/icons";
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
    isWorking?: boolean;
    isApproval?: boolean;
    onToggleSidebar: () => void;
    onPalette: () => void;
    onToggleDiff: () => void;
    onInspectPrompt?: () => void;
    onNewChat?: () => void;
    onSettings?: () => void;
  };

  let {
    workspaceName,
    chatTitle,
    titleHint,
    diffOpen,
    sidebarOpen,
    isWorking = false,
    isApproval = false,
    onToggleSidebar,
    onPalette,
    onToggleDiff,
    onInspectPrompt,
    onNewChat,
    onSettings,
  }: Props = $props();

  const isMac = isMacPlatform();
</script>

<header class="app-topbar" data-tauri-drag-region>
  <div class="topbar-left" data-tauri-drag-region>
    {#if !isMac}
      <div class="topbar-brand" title="Z Engine Beta">
        <LogoMark size={14} />
        <span>Z Engine</span>
        <span class="brand-beta-pill">BETA</span>
      </div>
    {/if}

    <div class="topbar-item-wrap">
      <button
        type="button"
        class="icon-btn topbar-toggle-btn"
        onclick={onToggleSidebar}
        aria-label="Toggle sidebar"
      >
        <Icon icon={PanelLeft} size={14} strokeWidth={1.8} />
      </button>
      <div class="topbar-micro-tip" role="tooltip">
        <span>{sidebarOpen ? "Hide sidebar" : "Show sidebar"}</span>
        <kbd>{modLabel()}B</kbd>
      </div>
    </div>

    {#if onNewChat && !sidebarOpen}
      <div class="topbar-item-wrap">
        <button
          type="button"
          class="icon-btn"
          onclick={onNewChat}
          aria-label="New chat"
        >
          <Icon icon={Plus} size={14} strokeWidth={2} />
        </button>
        <div class="topbar-micro-tip" role="tooltip">
          <span>New chat</span>
          <kbd>{modLabel()}N</kbd>
        </div>
      </div>
    {/if}
  </div>

  <div class="topbar-center" data-tauri-drag-region>
    <div
      class={`topbar-session-info${isWorking ? " is-working" : ""}${isApproval ? " is-approval" : ""}`}
      data-tauri-drag-region
      title={titleHint || `${workspaceName || ""}${workspaceName && chatTitle ? " / " : ""}${chatTitle || ""}`}
    >
      {#if isWorking}
        <span class="topbar-live-dot working" aria-hidden="true"></span>
        <span class="topbar-live-label working">Working</span>
        <span class="topbar-crumb-sep" data-tauri-drag-region>·</span>
      {:else if isApproval}
        <span class="topbar-live-dot approval" aria-hidden="true"></span>
        <span class="topbar-live-label approval">Review</span>
        <span class="topbar-crumb-sep" data-tauri-drag-region>·</span>
      {/if}

      {#if workspaceName}
        <span class="topbar-ws-tag" data-tauri-drag-region>
          <Icon icon={FolderGit2} size={12} strokeWidth={1.8} class="topbar-ws-icon" />
          <span class="topbar-ws-text" data-tauri-drag-region>{workspaceName}</span>
        </span>
      {/if}
      {#if workspaceName && chatTitle}
        <span class="topbar-crumb-sep" data-tauri-drag-region>›</span>
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
    <div class="topbar-item-wrap">
      <button
        type="button"
        class="icon-btn"
        onclick={onPalette}
        aria-label="Search and commands"
      >
        <Icon icon={Search} size={14} strokeWidth={1.8} />
      </button>
      <div class="topbar-micro-tip" role="tooltip">
        <span>Search & commands</span>
        <kbd>{modLabel()}K</kbd>
      </div>
    </div>

    <UpdateButton />
    <ContextMeter onInspect={onInspectPrompt} />

    <div class="topbar-item-wrap">
      <button
        type="button"
        class={`icon-btn${diffOpen ? " active" : ""}`}
        onclick={onToggleDiff}
        aria-label="Review file changes"
      >
        <Icon icon={GitCompare} size={14} strokeWidth={1.8} />
      </button>
      <div class="topbar-micro-tip" role="tooltip">
        <span>Review changes</span>
        <kbd>{modLabel()}D</kbd>
      </div>
    </div>

    {#if onSettings}
      <div class="topbar-item-wrap">
        <button
          type="button"
          class="icon-btn"
          onclick={onSettings}
          aria-label="Open settings"
        >
          <Icon icon={Settings} size={14} strokeWidth={1.8} />
        </button>
        <div class="topbar-micro-tip" role="tooltip">
          <span>Settings</span>
          <kbd>{modLabel()},</kbd>
        </div>
      </div>
    {/if}

    <WindowControlsMaybe />
  </div>
</header>
