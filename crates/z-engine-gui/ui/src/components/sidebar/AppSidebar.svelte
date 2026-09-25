<script lang="ts">
  import { sidebarModel } from "$lib/domain/sidebarModel";
  import { projects, sessionList, sessions } from "$lib/runtime";
  import {
    addWorkspace,
    openChat,
    removeChat,
    removeWorkspace,
    startNewChatIn,
  } from "$lib/stores/app-actions";
  import { ui } from "$lib/stores/ui.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { EmptyState } from "$lib/ui";
  import Icon, { ChevronDown, ChevronRight, FolderPlus, Plus } from "$lib/ui/icons";
  import { ticker } from "$lib/ui/ticker.svelte";
  import { sameWorkspacePath, workspaceStore } from "$lib/workspaces";
  import ChatRow from "./ChatRow.svelte";
  import ProjectGroup from "./ProjectGroup.svelte";
  import SidebarFooter from "./SidebarFooter.svelte";
  import SidebarNav from "./SidebarNav.svelte";

  /** Projects with their chats between the fixed top and the footer. */
  type Props = { inboxCount: number };
  let { inboxCount }: Props = $props();

  const workspaces = bindStore(workspaceStore);
  const clock = ticker(() => true, 30_000);
  let expanded = $state<Record<string, boolean>>({});
  let folded = $state<Record<string, boolean>>({});
  let othersOpen = $state(true);

  const model = $derived(
    sidebarModel({
      sessions: sessionList.items,
      roots: workspaces.current.roots,
      activeRoot: workspaces.current.active,
      activeSessionId: sessions.activeId,
      activity: sessions.activity,
      unread: sessions.unread,
      expanded,
      now: clock.now,
    }),
  );

  function isOpen(root: string, active: boolean): boolean {
    return folded[root] === undefined ? active : !folded[root];
  }

  function toggle(root: string, active: boolean) {
    folded[root] = isOpen(root, active);
    if (!sameWorkspacePath(workspaces.current.active, root)) workspaceStore.setActive(root);
  }

  function newWorktree(root: string) {
    workspaceStore.setActive(root);
    ui.openWorktree();
  }
</script>

<div class="sidebar-slot">
  <aside class="app-sidebar" aria-label="Projects and chats">
    <SidebarNav {inboxCount} />

    <div class="sidebar-scroll">
      <section class="sidebar-section" aria-label="Projects">
        <div class="sidebar-section-head">
          <span>Projects</span>
          <button type="button" class="sidebar-icon-btn" aria-label="Add a project folder" title="Add a project…" onclick={() => void addWorkspace()}>
            <Icon icon={Plus} size={12} strokeWidth={2} />
          </button>
        </div>
        {#each model.projects as project (project.root)}
          <ProjectGroup
            {project}
            open={isOpen(project.root, project.active)}
            summary={projects.summary(project.root)}
            onToggle={() => toggle(project.root, project.active)}
            onShowMore={() => (expanded[project.root] = true)}
            onOpenChat={(id, root) => void openChat(id, root)}
            onDeleteChat={(id, title) => void removeChat(id, title)}
            onNewChat={() => void startNewChatIn(project.root)}
            onNewWorktree={() => newWorktree(project.root)}
            onRemove={() => void removeWorkspace(project.root)}
          />
        {:else}
          <EmptyState icon={FolderPlus} title="No projects yet" description="Add a folder and Z Engine works inside it." class="sidebar-empty">
            <button type="button" class="btn-secondary" onclick={() => void addWorkspace()}>Add a project</button>
          </EmptyState>
        {/each}
      </section>

      {#if model.others.length > 0}
        <section class="sidebar-section" aria-label="Other chats">
          <button
            type="button"
            class="sidebar-section-head is-toggle"
            aria-expanded={othersOpen}
            onclick={() => (othersOpen = !othersOpen)}
          >
            <span>Other chats</span>
            <Icon icon={othersOpen ? ChevronDown : ChevronRight} size={11} strokeWidth={2} />
          </button>
          {#if othersOpen}
            <div class="project-chats">
              {#each model.others as row (row.item.sessionId)}
                <ChatRow
                  {row}
                  onOpen={() => void openChat(row.item.sessionId, row.item.projectRoot)}
                  onDelete={() => void removeChat(row.item.sessionId, row.item.title)}
                />
              {/each}
              {#if model.othersHidden}
                <button type="button" class="sidebar-more" onclick={() => (expanded.others = true)}>
                  Show {model.othersHidden} more
                </button>
              {/if}
            </div>
          {/if}
        </section>
      {/if}
    </div>

    <SidebarFooter />
  </aside>
</div>
