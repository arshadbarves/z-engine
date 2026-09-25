<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { sessionLabel, viewTitle } from "$lib/domain/sessionList";
  import { inboxSnapshot, projects, sessions } from "$lib/runtime";
  import { createWorktreeAndStart, startNewChat } from "$lib/stores/app-actions";
  import { ui } from "$lib/stores/ui.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { presence } from "$lib/ui/presence.svelte";
  import { workspaceStore } from "$lib/workspaces";
  import WorkPanel from "../agents/WorkPanel.svelte";
  import DiffPanel from "../overlays/DiffPanel.svelte";
  import WorktreeDialog from "../overlays/WorktreeDialog.svelte";
  import AppSidebar from "../sidebar/AppSidebar.svelte";
  import MainStage from "./MainStage.svelte";
  import TopBar from "./TopBar.svelte";

  /** The window once set up: title bar, sidebar, the content stage and its docked panels. */
  type Props = { entering: boolean };
  let { entering }: Props = $props();

  const workspaces = bindStore(workspaceStore);
  const diffPresence = presence(() => ui.diffOpen, 180);
  const workPresence = presence(() => ui.workPanel !== null, 180);

  const view = $derived(sessions.active);
  const chatTitle = $derived(view ? sessionLabel(viewTitle(view)) : null);
  const inboxCount = $derived(inboxSnapshot().count);

  $effect(() => {
    void projects.refreshAll(workspaces.current.roots);
  });

  $effect(() => projects.watchFocus(() => workspaceStore.getSnapshot().roots));

  $effect(() => {
    projects.noteTurns(sessions.views, workspaces.current.roots);
  });

  $effect(() => {
    function onDblClick(e: MouseEvent) {
      const target = e.target as HTMLElement;
      if (target.closest("button, input, textarea, a, [role='button']")) return;
      if (target.closest(".app-sidebar")) void getCurrentWindow().toggleMaximize();
    }
    window.addEventListener("dblclick", onDblClick);
    return () => window.removeEventListener("dblclick", onDblClick);
  });
</script>

<main class={`app${ui.sidebarOpen ? "" : " no-sidebar"}${entering ? " app-enter" : ""}`}>
  <TopBar
    {chatTitle}
    sidebarOpen={ui.sidebarOpen}
    onToggleSidebar={() => (ui.sidebarOpen = !ui.sidebarOpen)}
    onPalette={() => ui.openPalette()}
    onNewChat={() => void startNewChat()}
    onSettings={() => ui.openSettings()}
  />

  <div class="app-body">
    <AppSidebar {inboxCount} />

    <section class="workstation-stage">
      <MainStage />

      {#if workPresence.mounted}
        <WorkPanel isClosing={workPresence.closing} onClose={() => ui.closeWork()} />
      {/if}

      {#if diffPresence.mounted}
        <DiffPanel isClosing={diffPresence.closing} onClose={() => ui.closeDiff()} />
      {/if}
    </section>
  </div>
</main>

{#if ui.worktreeOpen}
  <WorktreeDialog
    projects={workspaces.current.roots}
    project={workspaces.current.active}
    onClose={() => (ui.worktreeOpen = false)}
    onCreate={createWorktreeAndStart}
  />
{/if}
