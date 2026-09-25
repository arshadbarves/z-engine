<script lang="ts">
  import { CATEGORY_ORDER } from "$lib/domain/inspectOutline";
  import { categoryMeta, type ContextCategory } from "$lib/promptInspectView";
  import { fmtTokens } from "$lib/util";

  /**
   * What the request is made of, as one bar split by kind of part; the line
   * above says how much of the context window it fills. A kind filters the
   * outline; picking it again clears the filter.
   */
  type Props = {
    totals: Record<ContextCategory, number>;
    used: number;
    limit: number;
    active: ContextCategory | null;
    onPick: (category: ContextCategory | null) => void;
  };
  let { totals, used, limit, active, onPick }: Props = $props();

  const pct = $derived(limit > 0 ? Math.min(100, Math.round((used / limit) * 100)) : 0);
  const level = $derived(pct >= 85 ? "danger" : pct >= 65 ? "warn" : "ok");
  const parts = $derived(CATEGORY_ORDER.filter((c) => totals[c] > 0));
  const sum = $derived(parts.reduce((n, c) => n + totals[c], 0));

  function width(tokens: number): string {
    return `${sum > 0 ? (tokens / sum) * 100 : 0}%`;
  }
</script>

<section class="inspector-map" aria-label="What fills the context window">
  <p class="inspector-map-line">
    <span class={`inspector-map-pct level-${level}`}>{pct}%</span>
    of the context window · {fmtTokens(used)} of {fmtTokens(limit)} tokens
  </p>
  <div class="inspector-map-bar" aria-hidden="true">
    {#each parts as c (c)}
      <span class:is-dim={active !== null && active !== c} style:width={width(totals[c])} style:background={categoryMeta(c).color}></span>
    {/each}
  </div>
  <div class="inspector-map-keys" role="group" aria-label="Show one kind of part">
    {#each parts as c (c)}
      <button type="button" class="inspector-map-key" aria-pressed={active === c} onclick={() => onPick(active === c ? null : c)}>
        <span class="inspector-swatch" style:background={categoryMeta(c).color}></span>
        {categoryMeta(c).label}
        <span class="inspector-map-tokens">{fmtTokens(totals[c])}</span>
      </button>
    {/each}
  </div>
</section>
