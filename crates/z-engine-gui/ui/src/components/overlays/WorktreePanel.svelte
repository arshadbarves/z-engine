<script lang="ts">
  import Icon, { Check, FolderGit2, GitBranch, Plus, X } from "$lib/ui/icons";
  import { wsBasename } from "$lib/workspaces";

  type Props = {
    isClosing?: boolean;
    onClose: () => void;
    onCreate: (name: string) => void;
    workspaces?: string[];
    activeWorkspace?: string | null;
    onActivateWorkspace?: (root: string) => void;
  };

  let {
    isClosing = false,
    onClose,
    onCreate,
    workspaces = [],
    activeWorkspace = null,
    onActivateWorkspace,
  }: Props = $props();

  let name = $state("");
  const slug = $derived(name.toLowerCase().replace(/[^a-z0-9-]/g, ""));

  function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (slug) {
      onCreate(slug);
      name = "";
    }
  }
</script>

<aside class={`worktree-panel${isClosing ? " is-closing" : ""}`}>
  <div class="worktree-head">
    <div class="worktree-head-left">
      <Icon icon={GitBranch} size={14} class="worktree-head-icon" />
      <span class="worktree-title">Git Worktrees</span>
      {#if workspaces.length > 0}
        <span class="worktree-badge">{workspaces.length} active</span>
      {/if}
    </div>
    <div class="worktree-head-actions">
      <button
        type="button"
        class="icon-btn"
        title="Close worktree panel (Esc)"
        onclick={onClose}
        aria-label="Close worktree panel"
      >
        <Icon icon={X} size={13} />
      </button>
    </div>
  </div>

  <div class="worktree-body">
    <div class="worktree-card create-card">
      <div class="create-card-head">
        <span class="create-card-title">New Isolated Worktree</span>
      </div>
      <p class="create-card-desc">
        Creates a clean branch and isolated directory checkout. The main working tree stays untouched.
      </p>

      <form onsubmit={handleSubmit} class="worktree-form">
        <div class="worktree-input-wrap">
          <input
            bind:value={name}
            placeholder="task name (e.g. fix-auth, refactor-api)"
            spellcheck={false}
            class="worktree-input"
            onkeydown={(e) => e.key === "Escape" && onClose()}
          />
        </div>

        {#if slug}
          <div class="worktree-preview-box">
            <div class="preview-row">
              <span class="preview-label">Branch:</span>
              <span class="preview-val">zengine/{slug}</span>
            </div>
            <div class="preview-row">
              <span class="preview-label">Path:</span>
              <span class="preview-val">.z-engine/worktrees/{slug}</span>
            </div>
          </div>
        {/if}

        <button
          type="submit"
          disabled={!slug}
          class="btn-primary worktree-create-btn"
        >
          <Icon icon={Plus} size={13} />
          <span>Create & Start Session</span>
        </button>
      </form>
    </div>

    {#if workspaces.length > 0}
      <div class="worktree-section">
        <div class="worktree-section-head">
          <span>Active Workspaces</span>
        </div>
        <div class="worktree-list">
          {#each workspaces as ws}
            {@const isActive = ws === activeWorkspace}
            <button
              type="button"
              class={`worktree-item${isActive ? " active" : ""}`}
              onclick={() => onActivateWorkspace?.(ws)}
            >
              <Icon icon={FolderGit2} size={13} class="ws-item-icon" />
              <div class="ws-item-info">
                <span class="ws-item-name">{wsBasename(ws)}</span>
                <span class="ws-item-path" title={ws}>{ws}</span>
              </div>
              {#if isActive}
                <span class="ws-active-tag">
                  <Icon icon={Check} size={11} strokeWidth={2.2} />
                  <span>active</span>
                </span>
              {/if}
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <div class="worktree-card explainer-card">
      <span class="explainer-title">How Worktrees Work</span>
      <p class="explainer-desc">
        Git worktrees let multiple branches be checked out simultaneously in separate folders. Changes are tested and verified independently without switching branches in your main IDE.
      </p>
    </div>
  </div>
</aside>
