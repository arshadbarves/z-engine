<script lang="ts">
  import { compact } from "$lib/commands";
  import { configStore } from "$lib/configStore";
  import { contextBreakdown } from "$lib/contextBreakdown";
  import { pushToast, transcriptStore, usageStore } from "$lib/runtime";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import Icon, {
    AlertOctagon,
    AlertTriangle,
    CheckCircle2,
    Eye,
    Minimize2,
    X,
  } from "$lib/ui/icons";
  import { estimateCost, fmtCost, fmtTokens } from "$lib/util";

  type Props = { onInspect?: () => void };
  let { onInspect }: Props = $props();

  const RING_R = 7;
  const RING_C = 2 * Math.PI * RING_R;

  const usage = bindStore(usageStore);
  const messages = bindStore(transcriptStore);
  const cfg = bindStore(configStore);
  let open = $state(false);
  let compacting = $state(false);
  let root: HTMLDivElement | undefined = $state();

  $effect(() => {
    if (!open) return;
    function onDoc(e: MouseEvent) {
      if (!root?.contains(e.target as Node)) open = false;
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") open = false;
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  });

  const max = $derived(Math.max(1, usage.current.maxTokens));
  const br = $derived(contextBreakdown(messages.current, usage.current.promptTokens, max));
  const pct = $derived(Math.min(100, Math.round((br.used / br.max) * 100)));
  const level = $derived(pct >= 85 ? "danger" : pct >= 65 ? "warn" : "ok");
  const cost = $derived(
    estimateCost(cfg.current?.pricing ?? null, usage.current.promptTokens, usage.current.completionTokens),
  );
  const compactAt = $derived(cfg.current?.compactAtPercent ?? 92);
  const statusText = $derived(
    level === "ok"
      ? "Memory Healthy · Plenty of space"
      : level === "warn"
        ? "Memory Active · Moderately full"
        : "Memory High · Compaction near",
  );
  const statusIcon = $derived(
    level === "ok" ? CheckCircle2 : level === "warn" ? AlertTriangle : AlertOctagon,
  );

  async function handleCompact() {
    compacting = true;
    try {
      pushToast("Compacting session context…", "ok");
      await compact();
      open = false;
    } catch (e) {
      pushToast(String(e).replace("Error: ", ""), "warn");
    } finally {
      compacting = false;
    }
  }
</script>

<div class={`ctx-meter ${level}${open ? " is-open" : ""}`} bind:this={root}>
  <button
    type="button"
    class="ctx-ring-btn"
    aria-expanded={open}
    aria-label={`${pct}% context used`}
    title={open ? undefined : `${pct}% context used (${fmtTokens(br.used)} / ${fmtTokens(br.max)})`}
    onclick={() => (open = !open)}
  >
    <svg class="ctx-ring" viewBox="0 0 20 20" width={18} height={18} aria-hidden="true">
      <circle class="ctx-track" cx="10" cy="10" r={RING_R} />
      <circle
        class="ctx-fill"
        cx="10"
        cy="10"
        r={RING_R}
        stroke-dasharray={RING_C}
        stroke-dashoffset={RING_C * (1 - pct / 100)}
      />
    </svg>
  </button>

  {#if !open}
    <div class="ctx-tip" role="tooltip">
      <strong>{pct}% context used</strong>
      <span>{fmtTokens(br.used)} of {fmtTokens(br.max)}</span>
    </div>
  {/if}

  {#if open}
    <div class="ctx-popover" role="dialog" aria-label="Context usage">
      <div class="ctx-pop-header">
        <div class={`ctx-status-pill ${level}`}>
          <Icon icon={statusIcon} size={13} />
          <span>{statusText}</span>
        </div>
        <button type="button" class="icon-btn" onclick={() => (open = false)} aria-label="Close">
          <Icon icon={X} size={13} />
        </button>
      </div>
      <div class="ctx-metrics-grid">
        <div class="ctx-metric-card">
          <span class="ctx-metric-label">Used</span>
          <strong class="ctx-metric-val">{fmtTokens(br.used)}</strong>
          <span class="ctx-metric-sub">{pct}% of window</span>
        </div>
        <div class="ctx-metric-card">
          <span class="ctx-metric-label">Headroom</span>
          <strong class="ctx-metric-val">{fmtTokens(br.remaining)}</strong>
          <span class="ctx-metric-sub">{100 - pct}% available</span>
        </div>
        <div class="ctx-metric-card">
          <span class="ctx-metric-label">Capacity</span>
          <strong class="ctx-metric-val">{fmtTokens(br.max)}</strong>
          <span class="ctx-metric-sub">{cost != null ? fmtCost(cost) : "tokens"}</span>
        </div>
      </div>
      <div class="ctx-progress-section">
        <div class="ctx-multi-bar" aria-hidden="true">
          {#each br.slices as s}
            <div
              class="ctx-slice-bar"
              style={`width: ${Math.max(1, (s.tokens / br.max) * 100)}%; background-color: ${s.color}`}
              title={`${s.label}: ${fmtTokens(s.tokens)} (${Math.round((s.tokens / br.max) * 100)}%)`}
            ></div>
          {/each}
        </div>
        <div class="ctx-compact-marker" style={`left: ${compactAt}%`} title={`Auto-compacts at ${compactAt}%`}>
          <span>Compact {compactAt}%</span>
        </div>
      </div>
      <div class="ctx-breakdown-list">
        {#each br.slices as s}
          {@const p = Math.round((s.tokens / br.max) * 100)}
          <div class="ctx-breakdown-item">
            <div class="ctx-item-left">
              <span class="ctx-dot" style={`background-color: ${s.color}`}></span>
              <span class="ctx-item-name">{s.label}</span>
            </div>
            <div class="ctx-item-right">
              <span class="ctx-item-tokens">{fmtTokens(s.tokens)}</span>
              <span class="ctx-item-pct">{p}%</span>
            </div>
          </div>
        {/each}
      </div>
      <div class="ctx-pop-footer">
        <button type="button" class="ctx-btn-compact" disabled={compacting} onclick={() => void handleCompact()}>
          <Icon icon={Minimize2} size={12} />
          <span>{compacting ? "Compacting…" : "Compact Now"}</span>
        </button>
        <button
          type="button"
          class="ctx-btn-inspect"
          onclick={() => {
            open = false;
            onInspect?.();
          }}
        >
          <Icon icon={Eye} size={12} />
          <span>Inspect Prompt</span>
        </button>
      </div>
    </div>
  {/if}
</div>
