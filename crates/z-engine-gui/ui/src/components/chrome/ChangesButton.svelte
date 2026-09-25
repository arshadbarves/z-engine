<script lang="ts">
  import { modLabel } from "$lib/platform";
  import { chatChanges, sessions } from "$lib/runtime";
  import { ui } from "$lib/stores/ui.svelte";
  import { Tooltip } from "$lib/ui";
  import Icon, { GitCompare } from "$lib/ui/icons";

  /** Opens the diff review; the count is how many files this chat has changed. */
  const mod = modLabel();
  const view = $derived(sessions.active);
  const turnsDone = $derived(view?.turns.length ?? 0);
  const count = $derived(chatChanges.count(sessions.activeId));
  const label = $derived(count ? `Review changes · ${count} file${count === 1 ? "" : "s"} changed in this chat` : "Review changes");

  $effect(() => {
    const id = sessions.activeId;
    void turnsDone;
    if (id && view?.info) void chatChanges.refresh(id);
  });
</script>

<Tooltip text={label} shortcut={`${mod}D`}>
  <button
    type="button"
    class={`titlebar-btn changes-btn${ui.diffOpen ? " is-active" : ""}${count ? " has-count" : ""}`}
    aria-label={label}
    aria-pressed={ui.diffOpen}
    onclick={() => ui.toggleDiff()}
  >
    <Icon icon={GitCompare} size={15} strokeWidth={1.8} />
    {#if count}<span class="changes-count">{count}</span>{/if}
  </button>
</Tooltip>
