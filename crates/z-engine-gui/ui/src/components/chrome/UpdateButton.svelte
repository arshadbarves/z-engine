<script lang="ts">
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { updateStore } from "$lib/updateStore";
  import Icon, { Check, Download, ExternalLink, LoaderCircle } from "$lib/ui/icons";

  const update = bindStore(updateStore);
  const info = $derived(update.current.info);
  const installing = $derived(update.current.installing);
  const progress = $derived(update.current.progress);
  const pct = $derived(progress?.percentage != null ? Math.round(progress.percentage) : null);
  const isDownloading = $derived(installing && progress?.phase === "downloading");
  const isReady = $derived(installing && progress?.phase === "ready");
  const isInstalling = $derived(installing && (progress?.phase === "installing" || isReady));
</script>

{#if info?.available}
  <div class="update-btn-wrap">
    <button
      type="button"
      class={`update-chip${installing ? " is-active" : ""}${isReady ? " is-ready" : ""}`}
      title={isReady
        ? "Update ready. Restart application to complete update."
        : installing
          ? `Downloading update${pct != null ? ` (${pct}%)` : "…"}`
          : `Directly update to v${info.latest}`}
      disabled={installing && !isReady}
      onclick={() => void updateStore.install()}
    >
      <span class="update-icon-wrap">
        {#if isReady}
          <Icon icon={Check} size={12} strokeWidth={2.4} class="update-check-icon" />
        {:else if installing}
          <Icon icon={LoaderCircle} size={12} strokeWidth={2.2} class="spin update-spin-icon" />
        {:else}
          <Icon icon={Download} size={12} strokeWidth={2} class="update-download-icon" />
        {/if}
      </span>
      <span class="update-label">
        {#if isReady}
          Restart to apply
        {:else if isInstalling}
          Installing…
        {:else if isDownloading}
          {pct != null ? `Downloading ${pct}%` : "Downloading…"}
        {:else}
          v{info.latest}
        {/if}
      </span>
      {#if !installing}
        <span class="update-beacon-dot" aria-hidden="true"></span>
      {/if}
      {#if installing && pct != null}
        <span class="update-progress-fill" style={`width: ${pct}%`} aria-hidden="true"></span>
      {/if}
    </button>
    {#if info.url && !installing}
      <button
        type="button"
        class="update-ext-btn"
        title="Open GitHub release details"
        onclick={() => updateStore.openRelease()}
        aria-label="Open GitHub release details"
      >
        <Icon icon={ExternalLink} size={11} strokeWidth={1.8} />
      </button>
    {/if}
  </div>
{/if}
