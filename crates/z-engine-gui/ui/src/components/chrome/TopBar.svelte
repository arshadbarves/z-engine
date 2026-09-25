<script lang="ts">
  import { modLabel } from "$lib/platform";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { PanelLeft, Plus, Search, Settings } from "$lib/ui/icons";
  import { updateStore } from "$lib/updateStore";
  import ChangesButton from "./ChangesButton.svelte";
  import TitleStatus from "./TitleStatus.svelte";
  import TitlebarButton from "./TitlebarButton.svelte";
  import WindowControlsMaybe from "./WindowControlsMaybe.svelte";

  /**
   * Title zone: the sidebar toggle at the left, the split island in the
   * middle, Changes at the right. What lives in the sidebar (New chat,
   * Search, Settings) joins the bar only while the sidebar is hidden.
   */
  type Props = {
    chatTitle: string | null;
    sidebarOpen: boolean;
    onToggleSidebar: () => void;
    onPalette: () => void;
    onNewChat: () => void;
    onSettings: () => void;
  };

  let { chatTitle, sidebarOpen, onToggleSidebar, onPalette, onNewChat, onSettings }: Props = $props();

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
    <TitleStatus chat={chatTitle} />
  </div>

  <div class="titlebar-side end" data-tauri-drag-region>
    <ChangesButton />
    {#if !sidebarOpen}
      <TitlebarButton
        label={update.current.info?.available ? "Settings · update available" : "Settings"}
        shortcut={`${mod},`}
        icon={Settings}
        dot={Boolean(update.current.info?.available)}
        onclick={onSettings}
      />
    {/if}
    <WindowControlsMaybe />
  </div>
</header>
