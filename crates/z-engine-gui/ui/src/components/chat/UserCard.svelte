<script lang="ts">
  import { abort, revertToTurn } from "$lib/commands";
  import { busyStore, draftStore, pushToast, trimTranscript } from "$lib/runtime";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import type { Msg } from "$lib/types";
  import Icon, { Check, Copy, Undo2 } from "$lib/ui/icons";

  const COLLAPSE_CHARS = 380;
  const COLLAPSE_LINES = 6;

  type Props = { m: Msg };
  let { m }: Props = $props();

  const busy = bindStore(busyStore);
  let copied = $state(false);
  let pending = $state(false);
  const lines = $derived(m.text.split("\n").length);
  const isLong = $derived(m.text.length > COLLAPSE_CHARS || lines > COLLAPSE_LINES);
  let expanded = $state(true);

  async function copy() {
    try {
      await navigator.clipboard.writeText(m.text);
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 1200);
    } catch {
      pushToast("Copy failed", "warn");
    }
  }

  const canRevert = $derived(typeof m.runTurn === "number");

  async function revert() {
    if (!canRevert || pending) return;
    pending = true;
    try {
      if (busy.current) await abort();
      draftStore.set(m.text);
      trimTranscript(m.runTurn as number);
      await revertToTurn(m.runTurn as number);
    } catch (e) {
      console.error(e);
      pushToast("Could not restore that prompt", "warn");
    } finally {
      pending = false;
    }
  }
</script>

<div class="user-message-row" id={`msg-${m.id}`} data-msg-id={m.id}>
  <div class="user-message-wrapper">
    <div class="user-message-bubble">
      <div class={`user-prompt-text${isLong && !expanded ? " collapsed" : ""}`}>
        {m.text}
      </div>
      {#if m.images && m.images.length > 0}
        <div class="user-attached-images">
          {#each m.images as url, i}
            <img src={url} alt={`attached ${i + 1}`} class="user-img-thumb" />
          {/each}
        </div>
      {/if}
      {#if isLong}
        <button type="button" class="user-expand-btn" onclick={() => (expanded = !expanded)}>
          {expanded ? "Show less" : "Show more"}
        </button>
      {/if}
    </div>
    <div class="user-bubble-actions">
      <button
        type="button"
        class={`bubble-action-icon-btn${copied ? " ok" : ""}`}
        title={copied ? "Copied" : "Copy prompt"}
        onclick={() => void copy()}
        aria-label="Copy prompt"
      >
        {#if copied}
          <Icon icon={Check} size={12} strokeWidth={2} class="copy-ok" />
        {:else}
          <Icon icon={Copy} size={12} strokeWidth={1.8} />
        {/if}
      </button>
      {#if canRevert}
        <button
          type="button"
          class="bubble-action-icon-btn"
          disabled={pending}
          title={busy.current ? "Stop & edit prompt" : "Revert & edit prompt"}
          onclick={() => void revert()}
          aria-label="Revert & edit prompt"
        >
          <Icon icon={Undo2} size={12} strokeWidth={1.8} />
        </button>
      {/if}
    </div>
  </div>
</div>
