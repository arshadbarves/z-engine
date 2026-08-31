<script lang="ts">
  import { inspectPrompt, type PromptInspect } from "$lib/commands";
  import { promptInsights } from "$lib/promptInsights";
  import {
    inspectBody,
    inspectCopyText,
    inspectRows,
    pct,
  } from "$lib/promptInspectView";
  import { sessionStore } from "$lib/runtime";
  import Icon, {
    AlertTriangle,
    Brain,
    Check,
    ChevronLeft,
    Copy,
    Sparkles,
    Target,
    Zap,
  } from "$lib/ui/icons";
  import { fmtTokens } from "$lib/util";
  import LogoMark from "../chrome/LogoMark.svelte";
  import WindowControlsMaybe from "../chrome/WindowControlsMaybe.svelte";
  import PromptInspectChart from "./PromptInspectChart.svelte";
  import PromptInspectContent from "./PromptInspectContent.svelte";
  import PromptInspectSidebar from "./PromptInspectSidebar.svelte";
  import "../../settings.css";
  import "../promptInspect.css";

  type Props = { isClosing?: boolean; onClose: () => void };
  let { isClosing = false, onClose }: Props = $props();

  let snap = $state<PromptInspect | null>(null);
  let err = $state<string | null>(null);
  let loading = $state(true);
  let sel = $state(0);
  let copied = $state(false);

  const rows = $derived(snap ? inspectRows(snap) : []);
  const ins = $derived(snap ? promptInsights(snap) : null);
  const activeRow = $derived(rows[sel] ?? rows[0]);
  const activeLayer = $derived(ins?.layers[sel] ?? ins?.layers[0]);
  const activeLabel = $derived(
    activeRow
      ? activeRow.kind === "msg"
        ? activeRow.part.label
        : activeRow.tool.name
      : "Prompt part",
  );

  $effect(() => {
    loading = true;
    const id = sessionStore.getSnapshot() || undefined;
    inspectPrompt(id)
      .then((s) => {
        snap = s;
        err = null;
        sel = 0;
        loading = false;
      })
      .catch((e: unknown) => {
        err = String(e).replace(/^Error:\s*/, "");
        loading = false;
      });
  });

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  async function onCopyAll() {
    if (!snap) return;
    try {
      await navigator.clipboard.writeText(inspectCopyText(snap));
      copied = true;
      window.setTimeout(() => {
        copied = false;
      }, 1200);
    } catch (e) {
      console.error(e);
    }
  }
</script>

<div class={`settings-overlay${isClosing ? " is-closing" : ""}`} role="presentation">
  <div
    class={`settings-page prompt-inspect-page${isClosing ? " is-closing" : ""}`}
    role="dialog"
    tabindex="-1"
    aria-label="Prompt inspector"
  >
    <header class="app-topbar settings-topbar" data-tauri-drag-region>
      <div class="topbar-left" data-tauri-drag-region>
        <button
          type="button"
          class="icon-btn settings-back-btn"
          title="Back (Esc)"
          onclick={onClose}
          aria-label="Back"
        >
          <Icon icon={ChevronLeft} size={15} strokeWidth={1.8} />
        </button>
        <div class="settings-breadcrumb">
          <span class="settings-breadcrumb-root">Prompt Inspector</span>
          <span class="settings-breadcrumb-sep">/</span>
          <span class="settings-breadcrumb-leaf">{snap ? activeLabel : "Assembly"}</span>
        </div>
      </div>

      <div class="topbar-center" data-tauri-drag-region>
        <div class="settings-topbar-pill">
          <LogoMark size={14} />
          <span>Prompt Assembly Profiler</span>
        </div>
      </div>

      <div class="topbar-right" data-tauri-drag-region>
        {#if snap}
          <span class="prompt-topbar-stat">
            ~{fmtTokens(snap.totalTokens)} tok
          </span>
        {/if}
        <button
          type="button"
          class="settings-copy-btn"
          onclick={() => void onCopyAll()}
          disabled={!snap}
          aria-label="Copy prompt assembly"
          title="Copy full wire transcript and schemas to clipboard"
        >
          <Icon icon={copied ? Check : Copy} size={13} />
          <span>{copied ? "Assembly Copied" : "Copy Full Assembly"}</span>
        </button>
        <WindowControlsMaybe />
      </div>
    </header>

    <div class="app-body settings-body prompt-inspect-body">
      <PromptInspectSidebar
        {rows}
        layers={ins?.layers ?? []}
        selected={sel}
        onSelect={(idx) => (sel = idx)}
        totalTokens={snap?.totalTokens ?? 0}
        {loading}
        {err}
      />

      <section class="canvas-pane settings-canvas-pane prompt-canvas-pane">
        <div class="settings-content-wrap">
          <div class="prompt-main-scrollable">
            {#if err}
              <div class="prompt-inspect-err" role="alert">
                <Icon icon={AlertTriangle} size={18} />
                <div>
                  <strong>Assembly Unavailable</strong>
                  <p>{err}</p>
                </div>
              </div>
            {:else if !snap || !ins}
              <div class="settings-loading">
                <Icon icon={Brain} size={24} />
                <span>Profiling prompt context and wire token distribution…</span>
              </div>
            {:else}
              <!-- Executive Profiler Stat Cards -->
              <div class="prompt-kpi-grid">
                <div class="prompt-kpi-card">
                  <div class="kpi-icon-wrap model-icon">
                    <Icon icon={Brain} size={16} />
                  </div>
                  <div class="kpi-body">
                    <span class="kpi-label">Active Model</span>
                    <strong class="kpi-value" title={snap.model}>{snap.model}</strong>
                  </div>
                </div>

                <div class="prompt-kpi-card">
                  <div class="kpi-icon-wrap footprint-icon">
                    <Icon icon={Sparkles} size={16} />
                  </div>
                  <div class="kpi-body">
                    <span class="kpi-label">Total Prompt Footprint</span>
                    <strong class="kpi-value">~{fmtTokens(snap.totalTokens)} tok</strong>
                    <small class="kpi-sub">{snap.messages.length} msgs · {snap.tools.length} tools</small>
                  </div>
                </div>

                <div class="prompt-kpi-card">
                  <div class="kpi-icon-wrap cache-icon">
                    <Icon icon={Zap} size={16} />
                  </div>
                  <div class="kpi-body">
                    <span class="kpi-label">Cache Efficiency</span>
                    <strong class="kpi-value">{pct(ins.cacheableTokens / Math.max(1, snap.totalTokens))}</strong>
                    <small class="kpi-sub">~{fmtTokens(ins.cacheableTokens)} static prefix</small>
                  </div>
                </div>

                <div class="prompt-kpi-card">
                  <div class="kpi-icon-wrap sink-icon">
                    <Icon icon={Target} size={16} />
                  </div>
                  <div class="kpi-body">
                    <span class="kpi-label">Primary Token Sink</span>
                    <strong class="kpi-value" title={ins.largest.name}>{ins.largest.name}</strong>
                    <small class="kpi-sub">{pct(ins.largest.share)} of total context</small>
                  </div>
                </div>
              </div>

              <!-- Interactive Stack Distribution Bar & Legend -->
              <PromptInspectChart {ins} />

              <!-- Actionable Optimization Hints & Advisories -->
              {#if ins.hints.length > 0}
                <div class="prompt-advisory-box">
                  <div class="advisory-head">
                    <Icon icon={AlertTriangle} size={14} />
                    <span>Optimization Opportunities ({ins.hints.length})</span>
                  </div>
                  <ul class="prompt-advisory-list">
                    {#each ins.hints as h}
                      <li>{h}</li>
                    {/each}
                  </ul>
                </div>
              {/if}

              <!-- Deep Layer Content Inspector & Code Viewer -->
              <PromptInspectContent
                {activeRow}
                {activeLayer}
                rawContent={activeRow ? inspectBody(activeRow) : ""}
                totalTokens={snap.totalTokens}
              />
            {/if}
          </div>
        </div>
      </section>
    </div>
  </div>
</div>


