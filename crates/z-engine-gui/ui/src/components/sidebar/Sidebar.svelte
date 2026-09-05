<script lang="ts">
  import { sessionLabel } from "$lib/sessionList";
  import type { SessionActivity } from "$lib/types";
  import type { SessionEntry } from "$lib/util";
  import { sameWorkspacePath, wsBasename } from "$lib/workspaces";
  import {
    ChevronDown,
    ChevronRight,
    FolderGit2,
    Icon,
    LoaderCircle,
    MessageSquare,
    Plus,
    ShieldAlert,
    Trash2,
  } from "$lib/ui/icons";

  type Props = {
    sessions: SessionEntry[];
    workspaces: string[];
    activeWorkspace: string | null;
    activeUlid: string;
    activity: Record<string, SessionActivity>;
    onOpen: (path: string, projectRoot?: string | null) => void;
    onDelete: (path: string) => void;
    onAddWorkspace: () => void;
    onRemoveWorkspace: (root: string) => void;
    onActivateWorkspace: (root: string | null) => void;
  };

  let {
    sessions,
    workspaces,
    activeWorkspace,
    activeUlid,
    activity,
    onOpen,
    onDelete,
    onAddWorkspace,
    onRemoveWorkspace,
    onActivateWorkspace,
  }: Props = $props();

  let recentsOpen = $state(true);
  let wsOpen = $state<Record<string, boolean>>({});
  let lastActive: string | null | undefined = undefined;

  const { byWorkspace, otherSessions } = $derived.by(() => {
    const byWorkspace = new Map<string, SessionEntry[]>();
    for (const root of workspaces) byWorkspace.set(root, []);
    const otherSessions: SessionEntry[] = [];
    for (const s of sessions) {
      const hit = s.projectRoot
        ? workspaces.find((root) => sameWorkspacePath(s.projectRoot, root))
        : undefined;
      if (hit) byWorkspace.get(hit)!.push(s);
      else otherSessions.push(s);
    }
    return { byWorkspace, otherSessions };
  });

  $effect(() => {
    const current = activeWorkspace;
    if (lastActive !== undefined && current && !sameWorkspacePath(lastActive, current)) {
      const match = workspaces.find((r) => sameWorkspacePath(current, r));
      if (match) wsOpen[match] = true;
    }
    lastActive = current;
  });

  function isWsOpen(root: string, isActive: boolean): boolean {
    return wsOpen[root] ?? isActive;
  }

  function toggleWs(root: string, isActive: boolean) {
    wsOpen[root] = !isWsOpen(root, isActive);
  }

  function sessionTitle(session: SessionEntry): string {
    return sessionLabel(session.firstUserMsg);
  }

  function unreadOutcome(session: SessionEntry, active: boolean, activityState: SessionActivity | null) {
    return !active && !activityState &&
      (session.unreadOutcome === "completed" || session.unreadOutcome === "aborted")
      ? session.unreadOutcome
      : null;
  }

  function workspaceActivity(items: SessionEntry[]): SessionActivity | null {
    let working = false;
    for (const s of items) {
      const a = activity[s.ulid];
      if (a === "approval") return "approval";
      if (a === "working") working = true;
    }
    return working ? "working" : null;
  }
</script>

{#snippet sessionTreeItem(session: SessionEntry)}
  {@const active = session.ulid === activeUlid}
  {@const activityState = activity[session.ulid] ?? null}
  {@const title = sessionTitle(session)}
  {@const isWorking = activityState === "working"}
  {@const isApproval = activityState === "approval"}
  {@const unread = unreadOutcome(session, active, activityState)}
  <div
    class="sidebar-session-item{active ? ' active' : ''}{isWorking ? ' working' : ''}{isApproval
      ? ' approval'
      : ''}{unread ? ` unread unread-${unread}` : ''}"
    role="button"
    tabindex={0}
    title={isApproval
      ? `Action Required · ${title}`
      : isWorking
        ? `Agent Working · ${title}`
        : title}
    onclick={(e) => {
      e.stopPropagation();
      onOpen(session.path, session.projectRoot);
    }}
    onkeydown={(e) => e.key === "Enter" && onOpen(session.path, session.projectRoot)}
  >
    <div class="session-item-icon-wrap">
      {#if isWorking}
        <span class="session-activity-ring working" aria-hidden="true"></span>
        <Icon icon={LoaderCircle} size={13} strokeWidth={2} class="spin session-spin-icon" />
      {:else if isApproval}
        <span class="session-activity-ring approval" aria-hidden="true"></span>
        <Icon icon={ShieldAlert} size={13} strokeWidth={2} class="session-alert-icon" />
      {:else}
        <Icon icon={MessageSquare} size={13} strokeWidth={1.8} class="session-msg-icon" />
      {/if}
    </div>
    <span class="session-item-title"><span class="title-text">{title}</span></span>
    <div class="session-item-tail">
      {#if isWorking}
        <span class="session-live-pill working" title="Agent working">Live</span>
      {:else if isApproval}
        <span class="session-live-pill approval" title="Needs approval">Review</span>
      {:else if unread}
        <span
          class="session-status-dot dot-{unread}"
          title={unread === "completed" ? "Completed" : "Stopped"}
          aria-label={unread}
        ></span>
      {/if}
      <button
        type="button"
        class="session-delete-btn"
        title="Delete chat"
        onclick={(e) => {
          e.stopPropagation();
          onDelete(session.path);
        }}
      >
        <Icon icon={Trash2} size={11} strokeWidth={1.8} />
      </button>
    </div>
  </div>
{/snippet}

{#snippet workspaceTreeItem(root: string)}
  {@const active = sameWorkspacePath(activeWorkspace, root)}
  {@const items = (byWorkspace.get(root) ?? []).slice(0, 40)}
  {@const name = wsBasename(root)}
  {@const open = isWsOpen(root, active)}
  {@const wsActivity = workspaceActivity(items)}
  <div class="workspace-item{active ? ' active-ws' : ''}">
    <div
      class="workspace-header{wsActivity ? ` ws-${wsActivity}` : ''}"
      role="button"
      tabindex={0}
      title="{root}{active ? ' (Active Workspace)' : ''}"
      onclick={() => {
        onActivateWorkspace(root);
        toggleWs(root, active);
      }}
      onkeydown={(e) => e.key === "Enter" && onActivateWorkspace(root)}
    >
      <span class="workspace-chevron" aria-hidden="true">
        {#if open}
          <Icon icon={ChevronDown} size={11} strokeWidth={2} />
        {:else}
          <Icon icon={ChevronRight} size={11} strokeWidth={2} />
        {/if}
      </span>
      <Icon icon={FolderGit2} size={13} strokeWidth={1.8} class="workspace-folder-icon" />
      <span class="workspace-title"><span class="title-text">{name}</span></span>
      <div class="workspace-actions">
        {#if wsActivity === "working"}
          <span class="ws-activity-dot working" title="Agent working in this workspace"></span>
        {:else if wsActivity === "approval"}
          <span class="ws-activity-dot approval" title="Approval needed in this workspace"></span>
        {/if}
        {#if items.length > 0}
          <span class="workspace-badge">{items.length}</span>
        {/if}
        <button
          type="button"
          class="workspace-del-btn"
          title="Remove workspace"
          onclick={(e) => {
            e.stopPropagation();
            onRemoveWorkspace(root);
          }}
        >
          <Icon icon={Trash2} size={11} strokeWidth={1.8} />
        </button>
      </div>
    </div>
    {#if open}
      <div class="workspace-session-list">
        {#if items.length === 0}
          <div class="workspace-empty-hint">No chats in this workspace</div>
        {:else}
          {#each items as s (s.path)}
            {@render sessionTreeItem(s)}
          {/each}
        {/if}
      </div>
    {/if}
  </div>
{/snippet}

<div class="sidebar-content-deck">
  <div class="sidebar-scrollable-area">
    <div class="sidebar-group-header">
      <span class="group-title">Workspaces</span>
      <button
        type="button"
        class="group-action-btn"
        title="Add workspace folder…"
        onclick={onAddWorkspace}
      >
        <Icon icon={Plus} size={12} strokeWidth={2} />
      </button>
    </div>

    {#if workspaces.length === 0}
      <div class="sidebar-empty-state">
        <span>No workspaces linked.</span>
        <button type="button" class="empty-add-btn" onclick={onAddWorkspace}>Add folder</button>
      </div>
    {/if}

    {#each workspaces as root (root)}
      {@render workspaceTreeItem(root)}
    {/each}

    {#if otherSessions.length > 0}
      <div class="sidebar-group-section">
        <div
          class="sidebar-group-header clickable"
          role="button"
          tabindex={0}
          onclick={() => (recentsOpen = !recentsOpen)}
          onkeydown={(e) => e.key === "Enter" && (recentsOpen = !recentsOpen)}
        >
          <span class="group-title">Other Chats</span>
          <span class="workspace-badge">{otherSessions.length}</span>
        </div>
        {#if recentsOpen}
          <div class="loose-sessions-list">
            {#each otherSessions.slice(0, 24) as s (s.path)}
              {@render sessionTreeItem(s)}
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>
