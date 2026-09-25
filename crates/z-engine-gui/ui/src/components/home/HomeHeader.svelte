<script lang="ts">
  import { projects } from "$lib/runtime";
  import { addWorkspace } from "$lib/stores/app-actions";
  import { EmptyState } from "$lib/ui";
  import Icon, { FolderPlus, GitBranch } from "$lib/ui/icons";
  import { wsBasename } from "$lib/workspaces";

  /** Above the hero composer: which project, which branch, and the one question. */
  type Props = { root: string | null };
  let { root }: Props = $props();

  const summary = $derived(projects.summary(root));
</script>

{#if root}
  <header class="home-head">
    <p class="home-eyebrow">
      <span class="home-project">{wsBasename(root)}</span>
      {#if summary?.branch}
        <span class="home-branch"><Icon icon={GitBranch} size={11} strokeWidth={2} />{summary.branch}</span>
      {/if}
    </p>
    <h1 class="home-title">What should we work on?</h1>
  </header>
{:else}
  <EmptyState
    icon={FolderPlus}
    title="Add a project to begin"
    description="Z Engine works inside a folder on your computer: it reads, edits and runs the code there."
    class="home-empty"
  >
    <button type="button" class="btn-accent" onclick={() => void addWorkspace()}>Add a project</button>
  </EmptyState>
{/if}
