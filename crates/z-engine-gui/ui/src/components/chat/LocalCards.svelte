<script lang="ts">
  import type { TimelineItem } from "$lib/domain/timeline/turns";
  import { visibleText } from "$lib/domain/timeline/blocks";
  import Icon, { ChevronDown, ChevronRight, CornerDownLeft, Layers, Terminal } from "$lib/ui/icons";
  import { fmtTokens } from "$lib/util";
  import Markdown from "./Markdown.svelte";

  /** Transcript cards that are not model output: steering, command output, errors, compaction. */
  type Props = { item: Extract<TimelineItem, { kind: "steer" | "output" | "error" | "compaction" }> };
  let { item }: Props = $props();

  let open = $state(false);
</script>

{#if item.kind === "steer"}
  <div class="steer-note">
    <Icon icon={CornerDownLeft} size={11} class="steer-icon" />
    <span class="steer-label">Steered</span>
    <span class="steer-text">{visibleText(item.message)}</span>
  </div>
{:else if item.kind === "output"}
  <div class="command-output-card">
    <div class="command-output-head">
      <Icon icon={Terminal} size={11} />
      <span>/{item.output.name}</span>
    </div>
    <div class="command-output-body"><Markdown text={item.output.markdown} /></div>
  </div>
{:else if item.kind === "error"}
  <div class="msg error" role="alert">{item.error.message}</div>
{:else}
  <div class="compaction-divider">
    <button type="button" class="compaction-head" aria-expanded={open} onclick={() => (open = !open)}>
      <Icon icon={Layers} size={11} />
      <span>
        Context compacted · {fmtTokens(item.marker.tokensBefore)} → {fmtTokens(item.marker.tokensAfter)} tokens
      </span>
      <Icon icon={open ? ChevronDown : ChevronRight} size={10} />
    </button>
    {#if open}
      <div class="compaction-summary"><Markdown text={item.marker.summary} /></div>
    {/if}
  </div>
{/if}
