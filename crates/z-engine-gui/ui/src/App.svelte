<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import MsgList from "./components/chat/MsgList.svelte";
  import Composer from "./components/chat/Composer.svelte";
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
  import { sessionLabel } from "./lib/sessionList";
  import { configStore } from "./lib/configStore";
  import { paletteActions } from "./lib/paletteActions";
  import {
    approvalGateStore,
    busyStore,
    hydrateStore,
    initEvents,
    modelStore,
    queueStore,
    sessionActivityStore,
    sessionStore,
    sessionsTickStore,
    setMaxTokens,
    transcriptStore,
  } from "./lib/runtime";
  import {
    addWorkspace,
    applyUserTitle,
    createWorktreeAndStart,
    delSession,
    flushReadyQueues,
    handleApprove,
    handleDeny,
    newTask,
    openSession,
    refreshSessions,
    removeWorkspace,
    type PendingNew,
  } from "./lib/stores/app-actions";
  import { bindStore } from "./lib/svelte/bind.svelte";
  import { presence } from "./lib/ui/presence.svelte";
  import { createScrollController } from "./lib/ui/scrollController.svelte";
  import { updateStore } from "./lib/updateStore";
  import type { SessionEntry } from "./lib/util";
  import { workspaceStore, wsBasename } from "./lib/workspaces";

  const messages = bindStore(transcriptStore);
  const busy = bindStore(busyStore);
  const sessionId = bindStore(sessionStore);
  const sessionsTick = bindStore(sessionsTickStore);
  const config = bindStore(configStore);
  const workspaces = bindStore(workspaceStore);
  const queued = bindStore(queueStore);
  const awaitingApproval = bindStore(approvalGateStore);
  const sessionActivity = bindStore(sessionActivityStore);
  const hydrating = bindStore(hydrateStore);

  const scroller = createScrollController({ bottomThreshold: 24 });

  let sessionsList = $state<SessionEntry[]>([]);
  let settingsOpen = $state(false);
  let inspectOpen = $state(false);
  let splash = $state(true);
  let paletteOpen = $state(false);
  let sidebarOpen = $state(true);
  let diffOpen = $state(false);
  let worktreeOpen = $state(false);
  let pendingNew = $state<PendingNew>(null);
  let transcriptEl: HTMLDivElement | undefined = $state();

  const palettePresence = presence(() => paletteOpen, 180);
  const settingsPresence = presence(() => settingsOpen, 180);
  const inspectPresence = presence(() => inspectOpen, 180);
  const worktreePresence = presence(() => worktreeOpen, 180);
  const diffPresence = presence(() => diffOpen, 180);

  function setList(fn: (prev: SessionEntry[]) => SessionEntry[]) {
    sessionsList = fn(sessionsList);
  }

  async function refresh() {
    await refreshSessions(setList);
  }

  async function startNew() {
    pendingNew = await newTask(refresh);
  }

  $effect(() => {
    void busy.current;
    void awaitingApproval.current;
    void sessionActivity.current;
    flushReadyQueues();
  });

  $effect(() => {
    void (async () => {
      await initEvents();
      try {
        const cfg = await getConfig();
        configStore.set(cfg);
        if (cfg.model) modelStore.set(cfg.model);
        if (cfg.maxContextTokens) setMaxTokens(Number(cfg.maxContextTokens));
      } catch (e) {
        console.error(e);
      }
      await refresh();
      await workspaceStore.load();
      void updateStore.check();
    })();
  });

  $effect(() => {
    if (sessionsTick.current === 0) return;
    void refresh();
  });

  $effect(() => {
    return scroller.bindContainer(transcriptEl);
  });

  $effect(() => {
    scroller.onMessagesUpdated(messages.current, sessionId.current, () => {
      applyUserTitle(messages.current, sessionId.current, pendingNew, setList);
    });
  });

  $effect(() => {
    function onDblClick(e: MouseEvent) {
      const target = e.target as HTMLElement;
      if (target.closest("button, input, textarea, a, .session, .ws-head")) return;
      if (target.closest(".sidebar, .chat-head")) {
        void getCurrentWindow().toggleMaximize();
      }
    }
    window.addEventListener("dblclick", onDblClick);
    return () => window.removeEventListener("dblclick", onDblClick);
  });

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (!(e.metaKey || e.ctrlKey)) return;
      const k = e.key.toLowerCase();
      if (k === "k") { e.preventDefault(); paletteOpen = !paletteOpen; }
      else if (k === "n") { e.preventDefault(); void startNew(); }
      else if (k === "b") { e.preventDefault(); sidebarOpen = !sidebarOpen; }
      else if (k === "d") { e.preventDefault(); diffOpen = !diffOpen; }
      else if (e.key === ",") { e.preventDefault(); settingsOpen = !settingsOpen; }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
  const activeWorkspaceName = $derived(
    workspaces.current.active
      ? wsBasename(workspaces.current.active)
      : config.current?.projectName || null,
  );

  const activeChatTitle = $derived.by(() => {
    const current = sessionsList.find((s) => s.ulid === sessionId.current);
    if (current?.firstUserMsg) return sessionLabel(current.firstUserMsg);
    const firstUser = messages.current.find((m) => m.kind === "user");
    if (firstUser?.text) return sessionLabel(firstUser.text);
    return "New Chat";
  });
</script>

{#if splash}
  <SplashScreen onDone={() => (splash = false)} />
{/if}

<ToastHost />

<main class={`app${sidebarOpen ? "" : " no-sidebar"}${splash ? "" : " app-enter"}`}>
  <TopBar
    workspaceName={activeWorkspaceName}
    chatTitle={activeChatTitle}
    titleHint={workspaces.current.active
      ? `workspace ${workspaces.current.active}${sessionId.current ? ` · session ${sessionId.current}` : ""}`
      : sessionId.current ? `session ${sessionId.current}` : undefined}
    {diffOpen}
    {sidebarOpen}
    isWorking={busy.current}
    isApproval={Boolean(awaitingApproval.current)}
    onToggleSidebar={() => (sidebarOpen = !sidebarOpen)}
    onPalette={() => (paletteOpen = true)}
    onToggleDiff={() => (diffOpen = !diffOpen)}
    onInspectPrompt={() => (inspectOpen = true)}
    onNewChat={() => void startNew()}
    onSettings={() => (settingsOpen = true)}
  />

  <div class="app-body">
    <AppSidebar
      sessions={sessionsList}
      workspaces={workspaces.current.roots}
      activeWorkspace={workspaces.current.active}
      activeUlid={sessionId.current}
      activity={sessionActivity.current}
      version={config.current?.version}
      onOpen={(p, root) => void openSession(p, root, refresh)}
      onDelete={(p) => void delSession(p, setList, startNew, refresh)}
      onAddWorkspace={() => void addWorkspace()}
      onRemoveWorkspace={(root) => void removeWorkspace(root, sessionsList, setList, startNew, refresh)}
      onActivateWorkspace={(root) => workspaceStore.setActive(root)}
      onNewChat={() => void startNew()}
    />

    <section class="workstation-stage">
      <div class="canvas-pane">
        <div class="transcript-wrap">
          {#if hydrating.current}
            <div class="hydrate-shimmer" aria-label="Restoring chat"></div>
          {/if}
          <div class="transcript" bind:this={transcriptEl}>
            <MsgList
              messages={messages.current}
              busy={busy.current}
              projectName={workspaces.current.active ? wsBasename(workspaces.current.active) : null}
              onApprove={(m, d) => void handleApprove(m, d)}
              onDeny={(m) => handleDeny(m)}
            />
          </div>
          {#if scroller.showJump}
            <JumpLatest onJump={() => scroller.jumpToLatest()} busy={busy.current} />
          {/if}
        </div>

        {#if queued.current.length > 0}
          <div class="queue-strip">
            <span class="queue-label">queued</span>
            {#each queued.current as q, i}
              <span class="queue-pill" title={q.text}>
                {q.text.slice(0, 48) || `(${q.images.length} image(s))`}
                <button title="Remove from queue" onclick={() => queueStore.removeAt(i)}>×</button>
              </span>
            {/each}
          </div>
        {/if}

        <Composer />
      </div>

      {#if worktreePresence.mounted}
        <WorktreePanel
          isClosing={worktreePresence.closing}
          onClose={() => (worktreeOpen = false)}
          onCreate={(n) => void createWorktreeAndStart(n, startNew)}
          workspaces={workspaces.current.roots}
          activeWorkspace={workspaces.current.active}
          onActivateWorkspace={(root) => workspaceStore.setActive(root)}
        />
      {/if}

      {#if diffPresence.mounted}
        <DiffPanel isClosing={diffPresence.closing} onClose={() => (diffOpen = false)} />
      {/if}
    </section>
  </div>

  {#if palettePresence.mounted}
    <CommandPalette
      isClosing={palettePresence.closing}
      onClose={() => (paletteOpen = false)}
      sessions={sessionsList}
      workspaces={workspaces.current.roots}
      activeWorkspace={workspaces.current.active}
      actions={paletteActions({
        newTask: () => void startNew(),
        addWorkspace: () => void addWorkspace(),
        openWorktree: () => (worktreeOpen = true),
        openDiff: () => (diffOpen = true),
        openSettings: () => (settingsOpen = true),
        toggleSidebar: () => (sidebarOpen = !sidebarOpen),
      })}
      onOpenSession={(p, root) => void openSession(p, root, refresh)}
      onActivateWorkspace={(root) => workspaceStore.setActive(root)}
    />
  {/if}
  {#if settingsPresence.mounted}
    <SettingsPage isClosing={settingsPresence.closing} onClose={() => (settingsOpen = false)} />
  {/if}
  {#if inspectPresence.mounted}
    <PromptInspector isClosing={inspectPresence.closing} onClose={() => (inspectOpen = false)} />
  {/if}
</main>
