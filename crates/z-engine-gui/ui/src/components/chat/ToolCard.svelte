<script lang="ts">
  import { tailLines } from "$lib/runtime";
  import { cleanSummary, fmtDur, toolLabel } from "$lib/toolUi";
  import type { Msg } from "$lib/types";

  type Props = { m: Msg };
  let { m }: Props = $props();

  let expanded = $state(false);
  let copied = $state(false);
  let secs = $state(0);

  const label = $derived(toolLabel(m.toolName ?? ""));
  const hasOutput = $derived(Boolean(m.output && m.output.length > 0));
  const canExpand = $derived(hasOutput || Boolean(m.streaming && m.output));
  const summary = $derived(cleanSummary(m.toolName, m.summary || m.preview || ""));
  const outputContent = $derived(
    m.streaming ? tailLines(m.output ?? "").join("\n") : (m.output ?? ""),
  );

  $effect(() => {
    if (!m.streaming) return;
    const t = setInterval(() => {
      secs += 0.1;
    }, 100);
    return () => clearInterval(t);
  });

  async function copy() {
    try {
      await navigator.clipboard.writeText(m.output ?? "");
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 1200);
    } catch {
      console.error("Failed to copy output");
    }
  }
</script>

<div
  class={`msg tool-card ${m.streaming ? "running" : m.ok === false ? "bad" : "ok"}${
    canExpand ? " expandable" : ""
  }`}
>
  <button
    class="tool-row"
    onclick={() => canExpand && (expanded = !expanded)}
    disabled={!canExpand}
    aria-expanded={canExpand ? expanded : undefined}
  >
    <span class="tool-dot" aria-hidden="true">
      <span class="tool-dot-inner"></span>
    </span>
    <span class="tool-label">{label}</span>
    <span class="tool-arg">{summary}</span>
    {#if canExpand}
      <svg
        viewBox="0 0 24 24"
        width={11}
        height={11}
        fill="none"
        stroke="currentColor"
        stroke-width="2.2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
        class={`tool-chevron${expanded ? " open" : ""}`}
      >
        <path d="M9 18l6-6-6-6" />
      </svg>
    {/if}
    {#if m.streaming}
      <span class="tool-elapsed">{secs.toFixed(1)}s</span>
    {:else}
      <span class="tool-elapsed">{m.durationMs ? fmtDur(m.durationMs) : ""}</span>
    {/if}
  </button>
  {#if expanded && hasOutput}
    <div class="tool-output-wrap">
      <div class="tool-output-bar">
        <span class="tool-output-lines">{(m.output ?? "").split("\n").length} lines</span>
        <button type="button" class="tool-copy-btn" onclick={() => void copy()} title="Copy output">
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <pre class={m.streaming ? "tool-tail" : "tool-full"}>{outputContent}</pre>
    </div>
  {/if}
</div>
