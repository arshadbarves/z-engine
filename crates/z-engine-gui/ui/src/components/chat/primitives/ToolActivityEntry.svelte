<script lang="ts">
  import type { LedgerEntry } from "$lib/activity";
  import { pushToast } from "$lib/runtime";
  import { copyFeedback } from "$lib/ui";
  import Icon, { Check, Copy } from "$lib/ui/icons";
  import ReasoningDisclosure from "./ReasoningDisclosure.svelte";

  type Props = { entry: LedgerEntry };
  let { entry }: Props = $props();

  let expanded = $state(false);
  const feedback = copyFeedback();
  const copied = $derived(feedback.copied);
  const hasOutput = $derived(Boolean(entry.output?.length));

  async function copyOutput() {
    if (!entry.output) return;
    if (!(await feedback.copy(entry.output))) pushToast("Copy failed", "warn");
  }
</script>

{#if entry.category === "thought"}
  <ReasoningDisclosure {entry} />
{:else}
  <div class={`activity-entry${entry.ok === false ? " failed" : ""}`}>
    <div class="ledger-row">
      <span class={`badge-tag ${entry.category}`}>{entry.category}</span>
      <div class="ledger-content">
        <span class="ledger-title" title={entry.title}>{entry.title}</span>
        {#if entry.sub}<span class="ledger-sub" title={entry.sub}>{entry.sub}</span>{/if}
      </div>
      <div class="ledger-trailing">
        {#if entry.streaming}<span class="process-pulse-dot" aria-label="Running"></span>{/if}
        {#if entry.metric}<span class="ledger-metric">{entry.metric}</span>{/if}
        {#if hasOutput}
          <button
            type="button"
            class="ledger-toggle"
            aria-expanded={expanded}
            onclick={() => (expanded = !expanded)}
          >
            {expanded ? "Hide" : "Output"}
          </button>
        {/if}
      </div>
    </div>

    {#if expanded && entry.output}
      <div class="activity-output">
        <div class="activity-output-bar">
          <span>{entry.output.split("\n").length} lines</span>
          <button type="button" class="terminal-copy-btn" onclick={() => void copyOutput()}>
            <Icon icon={copied ? Check : Copy} size={11} />
            <span>{copied ? "Copied" : "Copy"}</span>
          </button>
        </div>
        <pre>{entry.output}</pre>
      </div>
    {/if}
  </div>
{/if}

<style>
  .activity-entry.failed .ledger-title {
    color: var(--err);
  }

  .activity-output {
    margin: 2px 0 5px 29px;
    overflow: hidden;
    background: rgba(0, 0, 0, 0.38);
    border: 1px solid var(--border);
    border-radius: 6px;
  }

  .activity-output-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 8px;
    color: var(--text-3);
    font-family: var(--mono);
    font-size: 10px;
    border-bottom: 1px solid var(--border);
  }

  pre {
    max-height: 180px;
    margin: 0;
    padding: 8px 10px;
    overflow: auto;
    color: var(--text-2);
    font: 11px/1.45 var(--mono);
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
