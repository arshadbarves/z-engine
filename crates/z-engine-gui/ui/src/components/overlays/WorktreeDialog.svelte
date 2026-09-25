<script lang="ts">
  import { Dialog, DialogPanel } from "$lib/ui";
  import Icon, { GitBranch } from "$lib/ui/icons";
  import { wsBasename } from "$lib/workspaces";

  /**
   * A new chat on its own branch: a second checkout of the project, so its
   * changes stay apart from your working tree until you merge them.
   */
  type Props = {
    projects: string[];
    project: string | null;
    onClose: () => void;
    onCreate: (name: string, from: string | null) => Promise<void>;
  };
  let { projects, project, onClose, onCreate }: Props = $props();

  let name = $state("");
  let from = $state<string | null>(null);
  let creating = $state(false);
  const source = $derived(from ?? project ?? projects[0] ?? null);
  const slug = $derived(
    name
      .trim()
      .toLowerCase()
      .replace(/\s+/g, "-")
      .replace(/[^a-z0-9-]/g, "")
      .replace(/-+/g, "-")
      .replace(/^-|-$/g, ""),
  );

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!slug || creating) return;
    creating = true;
    await onCreate(slug, source);
    creating = false;
    onClose();
  }
</script>

<Dialog.Root
  open={true}
  onOpenChange={(open) => {
    if (!open) onClose();
  }}
>
  <DialogPanel contentClass="worktree-dialog">
    <header class="worktree-dialog-head">
      <span class="worktree-dialog-icon"><Icon icon={GitBranch} size={16} /></span>
      <div>
        <p class="worktree-dialog-title">New chat in a worktree</p>
        <p class="worktree-dialog-desc">
          A second copy of the project on its own branch. The chat's changes stay apart from your working tree until you merge them.
        </p>
      </div>
    </header>

    <form class="worktree-dialog-form" onsubmit={submit}>
      {#if projects.length > 1}
        <label class="worktree-field">
          <span>Project</span>
          <select class="setting-input" value={source} onchange={(e) => (from = e.currentTarget.value)}>
            {#each projects as root (root)}<option value={root}>{wsBasename(root)}</option>{/each}
          </select>
        </label>
      {/if}
      <label class="worktree-field">
        <span>What is it for?</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="setting-input" bind:value={name} placeholder="fix auth redirect" spellcheck={false} autofocus />
      </label>
      <p class="worktree-preview" aria-live="polite">
        {#if slug}
          Branch <code>zengine/{slug}</code> in <code>.z-engine/worktrees/{slug}</code>
        {:else}
          A short name becomes the branch and folder name.
        {/if}
      </p>
      <div class="worktree-actions">
        <button type="button" class="btn-ghost" onclick={onClose}>Cancel</button>
        <button type="submit" class="btn-primary" disabled={!slug || !source || creating}>
          {creating ? "Creating…" : "Create and start"}
        </button>
      </div>
    </form>
  </DialogPanel>
</Dialog.Root>
