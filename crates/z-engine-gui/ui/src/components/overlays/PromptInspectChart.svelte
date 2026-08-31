<script lang="ts">
  import type { PromptInsights } from "$lib/promptInsights";
  import { fmtTokens } from "$lib/util";

  type Props = { ins: PromptInsights };
  let { ins }: Props = $props();

  const total = $derived(Math.max(1, ins.cacheableTokens + ins.volatileTokens));
  function pct(n: number): string {
    return `${Math.round(n * 100)}%`;
  }
</script>

<div class="prompt-chart">
  <div class="prompt-stack" aria-label="Token share by role">
    {#each ins.stack as s}
      <i
        title={`${s.label} · ${fmtTokens(s.tokens)} (${pct(s.share)})`}
        style={`width: ${Math.max(0.4, s.share * 100)}%; background: ${s.color}`}
      ></i>
    {/each}
  </div>
  <ul class="prompt-stack-legend">
    {#each ins.stack as s}
      <li>
        <i class="swatch" style={`background: ${s.color}`}></i>
        {s.label}
        <span>{fmtTokens(s.tokens)} · {pct(s.share)}</span>
      </li>
    {/each}
  </ul>
  <div class="prompt-cache">
    <span>Cacheable prefix ~{fmtTokens(ins.cacheableTokens)} ({pct(ins.cacheableTokens / total)})</span>
    <span>Volatile tail ~{fmtTokens(ins.volatileTokens)} ({pct(ins.volatileTokens / total)})</span>
  </div>
</div>
