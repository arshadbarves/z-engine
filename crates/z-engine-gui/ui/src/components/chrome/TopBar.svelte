<script lang="ts">
  import { modLabel } from "$lib/platform";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { GitCompare, PanelLeft, Plus, Search, Settings } from "$lib/ui/icons";
  import { updateStore } from "$lib/updateStore";
  import TitleStatus from "./TitleStatus.svelte";
  import TitlebarButton from "./TitlebarButton.svelte";
  import WindowControlsMaybe from "./WindowControlsMaybe.svelte";

  /** Title zone: window controls and toggles at the sides, the companion and status line in the middle. */
  type Props = {
    workspaceName: string | null;
    chatTitle: string | null;
    diffOpen: boolean;
    sidebarOpen: boolean;
    onToggleSidebar: () => void;
    onPalette: () => void;
    onToggleDiff: () => void;
    onNewChat: () => void;
    onSettings: () => void;
  };

  let {
    workspaceName,
    chatTitle,
    diffOpen,
    sidebarOpen,
    onToggleSidebar,
    onPalette,
    onToggleDiff,
    onNewChat,
    onSettings,
  }: Props = $props();

  const update = bindStore(updateStore);
  const mod = modLabel();
</script>

<header class="app-titlebar" data-tauri-drag-region>
  <div class="titlebar-side" data-tauri-drag-region>
    <TitlebarButton
      label={sidebarOpen ? "Hide sidebar" : "Show sidebar"}
      shortcut={`${mod}B`}
      icon={PanelLeft}
      pressed={sidebarOpen}
      onclick={onToggleSidebar}
    />
    {#if !sidebarOpen}
      <TitlebarButton label="New chat" shortcut={`${mod}N`} icon={Plus} onclick={onNewChat} />
      <TitlebarButton label="Search and commands" shortcut={`${mod}K`} icon={Search} onclick={onPalette} />
    {/if}
  </div>

  <div class="titlebar-center">
    <TitleStatus workspace={workspaceName} chat={chatTitle} />
  </div>

  <div class="titlebar-side end" data-tauri-drag-region>
    <TitlebarButton
      label="Review changes"
      shortcut={`${mod}D`}
      icon={GitCompare}
      pressed={diffOpen}
      onclick={onToggleDiff}
    />
    <TitlebarButton
      label={update.current.info?.available ? "Settings · update available" : "Settings"}
      shortcut={`${mod},`}
      icon={Settings}
      dot={Boolean(update.current.info?.available)}
      onclick={onSettings}
    />
    <WindowControlsMaybe />
  </div>
</header>
