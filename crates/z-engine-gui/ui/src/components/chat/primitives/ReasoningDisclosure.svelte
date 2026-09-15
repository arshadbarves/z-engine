<script lang="ts">
  import type { LedgerEntry } from "$lib/activity";
  import {
    transitionProcessRun,
    toggleProcessDisclosure,
    type ProcessDisclosure,
  } from "$lib/domain/processDisclosure";
  import Icon, { Brain, ChevronRight } from "$lib/ui/icons";

  type Props = { entry: LedgerEntry };
  let { entry }: Props = $props();

  let expanded = $state(false);
  let wasStreaming = $state(false);
  let autoOpened = $state(false);
  let preRunExpanded = $state(false);
  const hasBody = $derived(Boolean(entry.body?.trim()));

  function applyDisclosure(next: ProcessDisclosure) {
    expanded = next.expanded;
    autoOpened = next.autoOpened;
    preRunExpanded = next.preRunExpanded;
  }

  function toggleReasoning() {
    applyDisclosure(toggleProcessDisclosure({ expanded, autoOpened, preRunExpanded }));
  }

  $effect(() => {
    const current: ProcessDisclosure = { expanded, autoOpened, preRunExpanded };
    const isStreaming = Boolean(entry.streaming);
    const next = transitionProcessRun(current, wasStreaming, isStreaming);
    if (next !== current) applyDisclosure(next);
    wasStreaming = isStreaming;
  });
</script>

<div class={`reason-item${entry.streaming ? " running" : ""}`}>
  <button
    type="button"
    class="reason-disclosure-trigger"
    disabled={!hasBody}
    aria-expanded={hasBody ? expanded : undefined}
    onclick={toggleReasoning}
  >
    {#if hasBody}
      <span class={`reason-chevron${expanded ? " open" : ""}`}>
        <Icon icon={ChevronRight} size={10} />
      </span>
    {/if}
    {#if entry.streaming}
      <span class="process-pulse-dot" aria-hidden="true"></span>
    {:else}
      <span class="reason-icon"><Icon icon={Brain} size={13} /></span>
    {/if}
    <span class="reason-title">{entry.title}</span>
  </button>

  {#if expanded && entry.body}
    <div class="reason-body">{entry.body}</div>
  {/if}
</div>

<style>
  .reason-disclosure-trigger {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 0;
    color: inherit;
    text-align: left;
    background: transparent;
    border: 0;
  }

  .reason-disclosure-trigger:not(:disabled) {
    cursor: pointer;
  }

  .reason-disclosure-trigger:focus-visible {
    outline: 1px solid var(--border-strong);
    outline-offset: 3px;
    border-radius: 3px;
  }

  .reason-title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
    font-size: 11.5px;
    font-weight: 500;
  }
</style>
