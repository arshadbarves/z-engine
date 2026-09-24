<script lang="ts">
  import type { SessionListItem } from "$lib/domain/sessionList";
  import { sidebarMark, type SidebarMark } from "$lib/domain/sessionOutcome";
  import type { SessionActivity, UnreadMark } from "$lib/domain/sessions";
  import { ChevronDown, ChevronRight, Icon, Plus, Trash2, X } from "$lib/ui/icons";
  import { sameWorkspacePath, wsBasename } from "$lib/workspaces";

  type Props = {
    sessions: SessionListItem[];
    workspaces: string[];
    activeWorkspace: string | null;
    activeSessionId: string | null;
    activity: Record<string, SessionActivity>;
    unread: Record<string, UnreadMark>;
    onOpen: (sessionId: string, projectRoot: string) => void;
    onDelete: (sessionId: string) => void;
    onAddWorkspace: () => void;
    onRemoveWorkspace: (root: string) => void;
    onActivateWorkspace: (root: string | null) => void;
  };

  let {
    sessions,
    workspaces,
    activeWorkspace,
    activeSessionId,
    activity,
    unread,
    onOpen,
    onDelete,
    onAddWorkspace,
    onRemoveWorkspace,
    onActivateWorkspace,
  }: Props = $props();

  let othersOpen = $state(true);
  let wsOpen = $state<Record<string, boolean>>({});
  let lastActive: string | null | undefined = undefined;

  const { byWorkspace, otherSessions } = $derived.by(() => {
    const byWorkspace = new Map<string, SessionListItem[]>();
    for (const root of workspaces) byWorkspace.set(root, []);
    const otherSessions: SessionListItem[] = [];
    for (const s of sessions) {
      const hit = s.projectRoot ? workspaces.find((root) => sameWorkspacePath(s.projectRoot, root)) : undefined;
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

  function markFor(session: SessionListItem): SidebarMark | null {
    return sidebarMark({
      active: session.sessionId === activeSessionId,
      activity: activity[session.sessionId] ?? null,
      unread: unread[session.sessionId],
      lastOutcome: session.lastOutcome,
    });
  }

  /** A collapsed workspace hides its chats, so it carries their most urgent live mark. */
  function groupMark(items: SessionListItem[]): SidebarMark | null {
    const live = items.map((s) => activity[s.sessionId]).filter(Boolean);
    if (live.includes("approval")) return { tone: "attention", label: "A chat here needs you" };
    return live.length ? { tone: "working", label: "Working" } : null;
  }

  function activate(e: KeyboardEvent, run: () => void) {
    if (e.key !== "Enter" && e.key !== " ") return;
    e.preventDefault();
    run();
  }
</script>

{#snippet chatRow(session: SessionListItem)}
  {@const mark = markFor(session)}
  {@const open = () => onOpen(session.sessionId, session.projectRoot)}
  <div
    class={`chat-row${session.sessionId === activeSessionId ? " is-active" : ""}`}
    role="button"
    tabindex={0}
    title={[session.title, mark?.label, session.legacy ? "imported from v1" : null].filter(Boolean).join(" · ")}
    onclick={open}
    onkeydown={(e) => activate(e, open)}
  >
    <span class="chat-row-title">{session.title}</span>
    {#if mark}<span class={`chat-mark tone-${mark.tone}`} role="img" aria-label={mark.label}></span>{/if}
    <button
      type="button"
      class="chat-row-delete"
      aria-label="Delete chat"
      title="Delete chat"
      onclick={(e) => {
        e.stopPropagation();
        onDelete(session.sessionId);
      }}
    >
      <Icon icon={Trash2} size={12} strokeWidth={1.8} />
    </button>
  </div>
{/snippet}

{#snippet workspaceGroup(root: string)}
  {@const active = sameWorkspacePath(activeWorkspace, root)}
  {@const items = (byWorkspace.get(root) ?? []).slice(0, 40)}
  {@const open = isWsOpen(root, active)}
  {@const mark = open ? null : groupMark(items)}
  {@const toggle = () => {
    onActivateWorkspace(root);
    wsOpen[root] = !open;
  }}
  <div class={`ws-group${active ? " is-active" : ""}`}>
    <div
      class="ws-head"
      role="button"
      tabindex={0}
      title={root}
      aria-expanded={open}
      onclick={toggle}
      onkeydown={(e) => activate(e, toggle)}
    >
      <Icon icon={open ? ChevronDown : ChevronRight} size={11} strokeWidth={2} class="ws-chevron" />
      <span class="ws-name">{wsBasename(root)}</span>
      {#if mark}<span class={`chat-mark tone-${mark.tone}`} role="img" aria-label={mark.label}></span>{/if}
      <button
        type="button"
        class="sidebar-icon-btn"
        aria-label="Remove workspace"
        title="Remove workspace"
        onclick={(e) => {
          e.stopPropagation();
          onRemoveWorkspace(root);
        }}
      >
        <Icon icon={X} size={11} strokeWidth={2} />
      </button>
    </div>
    {#if open}
      <div class="ws-chats">
        {#each items as s (s.sessionId)}
          {@render chatRow(s)}
        {:else}
          <p class="ws-empty">No chats yet</p>
        {/each}
      </div>
    {/if}
  </div>
{/snippet}

<div class="sidebar-scroll">
  <div class="sidebar-section">
    <div class="sidebar-section-head">
      <span>Workspaces</span>
      <button
        type="button"
        class="sidebar-icon-btn"
        aria-label="Add workspace folder"
        title="Add workspace folder…"
        onclick={onAddWorkspace}
      >
        <Icon icon={Plus} size={12} strokeWidth={2} />
      </button>
    </div>
    {#each workspaces as root (root)}
      {@render workspaceGroup(root)}
    {:else}
      <div class="sidebar-empty">
        <span>Add a project folder to start.</span>
        <button type="button" class="sidebar-empty-btn" onclick={onAddWorkspace}>Add folder</button>
      </div>
    {/each}
  </div>

  {#if otherSessions.length > 0}
    <div class="sidebar-section">
      <div
        class="sidebar-section-head is-toggle"
        role="button"
        tabindex={0}
        aria-expanded={othersOpen}
        onclick={() => (othersOpen = !othersOpen)}
        onkeydown={(e) => activate(e, () => (othersOpen = !othersOpen))}
      >
        <span>Other chats</span>
        <Icon icon={othersOpen ? ChevronDown : ChevronRight} size={11} strokeWidth={2} />
      </div>
      {#if othersOpen}
        <div class="ws-chats">
          {#each otherSessions.slice(0, 24) as s (s.sessionId)}
            {@render chatRow(s)}
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>
