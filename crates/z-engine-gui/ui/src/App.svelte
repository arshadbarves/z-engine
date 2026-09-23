<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import WorkPanel from "./components/agents/WorkPanel.svelte";
  import Composer from "./components/chat/Composer.svelte";
  import Transcript from "./components/chat/Transcript.svelte";
  import JumpLatest from "./components/chrome/JumpLatest.svelte";
  import SplashScreen from "./components/chrome/SplashScreen.svelte";
  import ToastHost from "./components/chrome/ToastHost.svelte";
  import TopBar from "./components/chrome/TopBar.svelte";
  import CommandPalette from "./components/overlays/CommandPalette.svelte";
  import DiffPanel from "./components/overlays/DiffPanel.svelte";
  import PromptInspector from "./components/overlays/PromptInspector.svelte";
  import WorktreePanel from "./components/overlays/WorktreePanel.svelte";
  import SettingsPage from "./components/settings/SettingsPage.svelte";
  import AppSidebar from "./components/sidebar/AppSidebar.svelte";
  import { getConfig } from "./lib/commands";
  import { configStore } from "./lib/configStore";
  import { workCounts } from "./lib/domain/agentTree";
  import { sessionLabel, viewTitle } from "./lib/domain/sessionList";
  import { lastPromptId } from "./lib/domain/timeline/blocks";
  import { paletteActions } from "./lib/paletteActions";
  import { initEvents, sessionList, sessions } from "./lib/runtime";
  import {
    addWorkspace,
    createWorktreeAndStart,
    openChat,
    removeChat,
    removeWorkspace,
    startNewChat,
  } from "./lib/stores/app-actions";
  import { ui } from "./lib/stores/ui.svelte";
  import { bindStore } from "./lib/svelte/bind.svelte";
  import { presence } from "./lib/ui/presence.svelte";
  import { createScrollController } from "./lib/ui/scrollController.svelte";
  import { updateStore } from "./lib/updateStore";
  import { workspaceStore, wsBasename } from "./lib/workspaces";

  const config = bindStore(configStore);
  const workspaces = bindStore(workspaceStore);
  const scroller = createScrollController({ bottomThreshold: 24 });

  let splash = $state(true);
  let transcriptEl: HTMLDivElement | undefined = $state();

  const palettePresence = presence(() => ui.paletteOpen, 180);
  const settingsPresence = presence(() => ui.settingsOpen, 180);
  const inspectPresence = presence(() => ui.inspectOpen, 180);
  const worktreePresence = presence(() => ui.worktreeOpen, 180);
  const diffPresence = presence(() => ui.diffOpen, 180);
  const workPresence = presence(() => ui.workPanel !== null, 180);

  const view = $derived(sessions.active);
  const activity = $derived(sessions.activeId ? (sessions.activity[sessions.activeId] ?? null) : null);
  const counts = $derived(view ? workCounts(view.agents, view.jobs) : null);
  const workBadge = $derived(counts ? counts.runningAgents + counts.runningJobs + counts.pendingWorktrees : 0);
  const chatTitle = $derived(view ? sessionLabel(viewTitle(view)) : "New Chat");
  const projectRoot = $derived(view?.info?.projectRoot ?? workspaces.current.active);
  const workspaceName = $derived(projectRoot ? wsBasename(projectRoot) : config.current?.projectName || null);
  const lastPrompt = $derived(lastPromptId(view?.messages));

  $effect(() => {
    void (async () => {
      await initEvents();
      await workspaceStore.load();
      await sessionList.refresh();
      try {
        configStore.set(await getConfig());
      } catch (e) {
        console.warn("get_config unavailable", e);
      }
      void updateStore.check();
    })();
  });

  $effect(() => scroller.bindContainer(transcriptEl));

  $effect(() => {
    void view;
    scroller.onContentUpdated(sessions.activeId, lastPrompt);
  });

  $effect(() => {
    function onDblClick(e: MouseEvent) {
      const target = e.target as HTMLElement;
      if (target.closest("button, input, textarea, a, .session, .ws-head")) return;
      if (target.closest(".sidebar, .chat-head")) void getCurrentWindow().toggleMaximize();
    }
    window.addEventListener("dblclick", onDblClick);
    return () => window.removeEventListener("dblclick", onDblClick);
  });

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (!(e.metaKey || e.ctrlKey)) return;
      const k = e.key.toLowerCase();
      if (k === "k") ui.togglePalette();
      else if (k === "n") void startNewChat();
      else if (k === "b") ui.sidebarOpen = !ui.sidebarOpen;
      else if (k === "d") ui.diffOpen = !ui.diffOpen;
      else if (e.key === ",") ui.settingsOpen = !ui.settingsOpen;
      else return;
      e.preventDefault();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

{#if splash}
  <SplashScreen onDone={() => (splash = false)} />
{/if}

<ToastHost />

<main class={`app${ui.sidebarOpen ? "" : " no-sidebar"}${splash ? "" : " app-enter"}`}>
  <TopBar
    {workspaceName}
    {chatTitle}
    titleHint={projectRoot
      ? `workspace ${projectRoot}${sessions.activeId ? ` · session ${sessions.activeId}` : ""}`
      : sessions.activeId
        ? `session ${sessions.activeId}`
        : undefined}
    diffOpen={ui.diffOpen}
    workOpen={ui.workPanel !== null}
    {workBadge}
    sidebarOpen={ui.sidebarOpen}
    isWorking={activity === "working"}
    isApproval={activity === "approval"}
    onToggleSidebar={() => (ui.sidebarOpen = !ui.sidebarOpen)}
    onPalette={() => ui.openPalette()}
    onToggleDiff={() => (ui.diffOpen = !ui.diffOpen)}
    onToggleWork={() => ui.toggleWork()}
    onInspectPrompt={() => (ui.inspectOpen = true)}
    onNewChat={() => void startNewChat()}
    onSettings={() => ui.openSettings()}
  />

  <div class="app-body">
    <AppSidebar
      sessions={sessionList.items}
      workspaces={workspaces.current.roots}
      activeWorkspace={workspaces.current.active}
      activeSessionId={sessions.activeId}
      activity={sessions.activity}
      unread={sessions.unread}
      version={config.current?.version}
      onOpen={(id, root) => void openChat(id, root)}
      onDelete={(id) => void removeChat(id)}
      onAddWorkspace={() => void addWorkspace()}
      onRemoveWorkspace={(root) => void removeWorkspace(root)}
      onActivateWorkspace={(root) => workspaceStore.setActive(root)}
      onNewChat={() => void startNewChat()}
    />

    <section class="workstation-stage">
      <div class="canvas-pane">
        <div class="transcript-wrap">
          {#if sessions.hydrating}
            <div class="hydrate-shimmer" aria-label="Restoring chat"></div>
          {/if}
          <div class="transcript" bind:this={transcriptEl}>
            <Transcript projectName={workspaces.current.active ? wsBasename(workspaces.current.active) : null} />
          </div>
          {#if scroller.showJump}
            <JumpLatest onJump={() => scroller.jumpToLatest()} busy={view?.status === "busy"} />
          {/if}
        </div>

        <Composer />
      </div>

      {#if worktreePresence.mounted}
        <WorktreePanel
          isClosing={worktreePresence.closing}
          onClose={() => (ui.worktreeOpen = false)}
          onCreate={(name) => void createWorktreeAndStart(name)}
          workspaces={workspaces.current.roots}
          activeWorkspace={workspaces.current.active}
          onActivateWorkspace={(root) => workspaceStore.setActive(root)}
        />
      {/if}

      {#if workPresence.mounted}
        <WorkPanel isClosing={workPresence.closing} onClose={() => ui.closeWork()} />
      {/if}

      {#if diffPresence.mounted}
        <DiffPanel isClosing={diffPresence.closing} onClose={() => (ui.diffOpen = false)} />
      {/if}
    </section>
  </div>

  {#if palettePresence.mounted}
    <CommandPalette
      isClosing={palettePresence.closing}
      onClose={() => (ui.paletteOpen = false)}
      sessions={sessionList.items}
      sessionsOnly={ui.paletteSessionsOnly}
      workspaces={workspaces.current.roots}
      activeWorkspace={workspaces.current.active}
      actions={paletteActions({
        newTask: () => void startNewChat(),
        addWorkspace: () => void addWorkspace(),
        openWorktree: () => ui.openWorktree(),
        openDiff: () => (ui.diffOpen = true),
        openSettings: () => ui.openSettings(),
        openInspector: () => (ui.inspectOpen = true),
        toggleSidebar: () => (ui.sidebarOpen = !ui.sidebarOpen),
      })}
      onOpenSession={(id, root) => void openChat(id, root)}
      onActivateWorkspace={(root) => workspaceStore.setActive(root)}
    />
  {/if}
  {#if settingsPresence.mounted}
    <SettingsPage
      isClosing={settingsPresence.closing}
      initialTab={ui.settingsTab}
      onClose={() => (ui.settingsOpen = false)}
    />
  {/if}
  {#if inspectPresence.mounted}
    <PromptInspector isClosing={inspectPresence.closing} onClose={() => (ui.inspectOpen = false)} />
  {/if}
</main>
