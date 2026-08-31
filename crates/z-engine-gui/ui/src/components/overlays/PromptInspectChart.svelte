<script lang="ts">
  import type { PromptInsights } from "$lib/promptInsights";
  import Icon, { RefreshCw, Zap } from "$lib/ui/icons";
  import { fmtTokens } from "$lib/util";

  type Props = { ins: PromptInsights };
  let { ins }: Props = $props();

  const total = $derived(Math.max(1, ins.cacheableTokens + ins.volatileTokens));
  function pct(n: number): string {
    return `${Math.round(n * 100)}%`;
  }
</script>

<div class="prompt-chart-card">
  <div class="prompt-chart-header">
    <div class="chart-title-wrap">
      <span class="chart-title">Token Distribution by Role</span>
      <span class="chart-sub">Proportional context footprint across prompt assembly layers</span>
    </div>
    <div class="chart-cache-ratios">
      <span class="cache-pill cacheable" title="Static byte prefix reused by LLM provider">
        <Icon icon={Zap} size={12} /> Cacheable {pct(ins.cacheableTokens / total)} (~{fmtTokens(ins.cacheableTokens)})
      </span>
      <span class="cache-pill volatile" title="Dynamic turn content generated per request">
        <Icon icon={RefreshCw} size={11} /> Dynamic {pct(ins.volatileTokens / total)} (~{fmtTokens(ins.volatileTokens)})
      </span>
    </div>
  </div>

  <div class="prompt-stack-track" aria-label="Token share by role">
    {#each ins.stack as s}
      {@const widthPct = Math.max(0.6, s.share * 100)}
      <div
        class="prompt-stack-segment"
        style={`width: ${widthPct}%; background: ${s.color};`}
        title={`${s.label}: ~${fmtTokens(s.tokens)} tokens (${pct(s.share)})`}
      ></div>
    {/each}
  </div>

  <ul class="prompt-stack-legend">
    {#each ins.stack as s}
      <li class="legend-item" title={`${s.label}: ~${fmtTokens(s.tokens)} tokens (${pct(s.share)})`}>
        <i class="swatch" style={`background: ${s.color}`}></i>
        <span class="legend-label">{s.label}</span>
        <span class="legend-stats">{fmtTokens(s.tokens)} <small>({pct(s.share)})</small></span>
      </li>
    {/each}
  </ul>
</div>

