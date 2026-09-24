<script lang="ts">
  import { contextMeter } from "$lib/domain/contextMeter";
  import { usageLine } from "$lib/domain/usage";
  import { compact, pushToast, requestContextReport, sessions } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import Icon, {
    AlertOctagon,
    AlertTriangle,
    CheckCircle2,
    Eye,
    Minimize2,
    RefreshCw,
    X,
  } from "$lib/ui/icons";
  import { fmtCost, fmtTokens } from "$lib/util";

  type Props = { onInspect?: () => void };
  let { onInspect }: Props = $props();

  const RING_R = 7;
  const RING_C = 2 * Math.PI * RING_R;

  let compacting = $state(false);
  let root: HTMLDivElement | undefined = $state();

  const view = $derived(sessions.active);
  const meter = $derived(
    contextMeter({
      contextTokens: view?.contextTokens ?? 0,
      contextLimit: view?.contextLimit ?? 0,
      breakdown: view?.contextBreakdown ?? null,
    }),
  );
  const cost = $derived(view?.costUsd ?? 0);
  const usage = $derived(view ? usageLine(view.usage) : "");
  const compactAt = $derived(settingsStore.settings?.context.compact_at_percent ?? 92);
  const statusText = $derived(
    meter.level === "ok"
      ? "Memory Healthy · Plenty of space"
      : meter.level === "warn"
        ? "Memory Active · Moderately full"
        : "Memory High · Compaction near",
  );
  const statusIcon = $derived(
    meter.level === "ok" ? CheckCircle2 : meter.level === "warn" ? AlertTriangle : AlertOctagon,
  );

  $effect(() => {
    if (!ui.contextOpen) return;
    function onDoc(e: MouseEvent) {
      if (!root?.contains(e.target as Node)) ui.contextOpen = false;
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") ui.contextOpen = false;
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  });

  async function handleCompact() {
    if (!view) return;
    compacting = true;
    if (await compact()) {
      pushToast("Compacting session context…", "ok");
      ui.contextOpen = false;
    }
    compacting = false;
  }
</script>

<div class={`ctx-meter ${meter.level}${ui.contextOpen ? " is-open" : ""}`} bind:this={root}>
  <button
    type="button"
    class="ctx-ring-btn"
    aria-expanded={ui.contextOpen}
    aria-label={`${meter.pct}% context used`}
    onclick={() => (ui.contextOpen = !ui.contextOpen)}
  >
    <svg class="ctx-ring" viewBox="0 0 20 20" width={18} height={18} aria-hidden="true">
      <circle class="ctx-track" cx="10" cy="10" r={RING_R} />
      <circle
        class="ctx-fill"
        cx="10"
        cy="10"
        r={RING_R}
        stroke-dasharray={RING_C}
        stroke-dashoffset={RING_C * (1 - meter.pct / 100)}
      />
    </svg>
  </button>

  {#if !ui.contextOpen}
    <div class="ctx-tip" role="tooltip">
      <strong>{meter.pct}% context used</strong>
      <span>{fmtTokens(meter.used)} of {fmtTokens(meter.max)}{cost > 0 ? ` · ${fmtCost(cost)}` : ""}</span>
    </div>
  {/if}

  {#if ui.contextOpen}
    <div class="ctx-popover" role="dialog" aria-label="Context usage">
      <div class="ctx-pop-header">
        <div class={`ctx-status-pill ${meter.level}`}>
          <Icon icon={statusIcon} size={13} />
          <span>{statusText}</span>
        </div>
        <button type="button" class="icon-btn" onclick={() => (ui.contextOpen = false)} aria-label="Close">
          <Icon icon={X} size={13} />
        </button>
      </div>
      <div class="ctx-metrics-grid">
        <div class="ctx-metric-card">
          <span class="ctx-metric-label">Used</span>
          <strong class="ctx-metric-val">{fmtTokens(meter.used)}</strong>
          <span class="ctx-metric-sub">{meter.pct}% of window</span>
        </div>
        <div class="ctx-metric-card">
          <span class="ctx-metric-label">Headroom</span>
          <strong class="ctx-metric-val">{fmtTokens(meter.remaining)}</strong>
          <span class="ctx-metric-sub">{100 - meter.pct}% available</span>
        </div>
        <div class="ctx-metric-card">
          <span class="ctx-metric-label">Session</span>
          <strong class="ctx-metric-val">{fmtCost(cost)}</strong>
          <span class="ctx-metric-sub">{usage || "no usage yet"}</span>
        </div>
      </div>
      <div class="ctx-progress-section">
        <div class="ctx-multi-bar" aria-hidden="true">
          {#each meter.slices as s (s.id)}
            <div
              class="ctx-slice-bar"
              style={`width: ${Math.max(1, (s.tokens / meter.max) * 100)}%; background-color: ${s.color}`}
              title={`${s.label}: ${fmtTokens(s.tokens)}`}
            ></div>
          {/each}
        </div>
        <div class="ctx-compact-marker" style={`left: ${compactAt}%`} title={`Auto-compacts at ${compactAt}%`}>
          <span>Compact {compactAt}%</span>
        </div>
      </div>
      <div class="ctx-breakdown-list">
        {#each meter.slices as s (s.id)}
          <div class="ctx-breakdown-item">
            <div class="ctx-item-left">
              <span class="ctx-dot" style={`background-color: ${s.color}`}></span>
              <span class="ctx-item-name">{s.label}</span>
            </div>
            <div class="ctx-item-right">
              <span class="ctx-item-tokens">{fmtTokens(s.tokens)}</span>
              <span class="ctx-item-pct">{Math.round((s.tokens / meter.max) * 100)}%</span>
            </div>
          </div>
        {/each}
        {#if meter.totalOnly && view}
          <button type="button" class="ctx-refresh-btn" onclick={() => void requestContextReport()}>
            <Icon icon={RefreshCw} size={11} />
            <span>Break down by prompt layer</span>
          </button>
        {/if}
      </div>
      <div class="ctx-pop-footer">
        <button
          type="button"
          class="ctx-btn-compact"
          disabled={compacting || !view}
          onclick={() => void handleCompact()}
        >
          <Icon icon={Minimize2} size={12} />
          <span>{compacting ? "Compacting…" : "Compact Now"}</span>
        </button>
        <button
          type="button"
          class="ctx-btn-inspect"
          onclick={() => {
            ui.contextOpen = false;
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
