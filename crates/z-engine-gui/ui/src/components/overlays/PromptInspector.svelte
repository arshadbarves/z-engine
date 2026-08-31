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
  import Icon, { Copy, Eye, Workflow, X } from "$lib/ui/icons";
  import { fmtTokens } from "$lib/util";
  import LogoMark from "../chrome/LogoMark.svelte";
  import PromptInspectChart from "./PromptInspectChart.svelte";
  import "../../settings.css";
  import "../promptInspect.css";

  type Props = { isClosing?: boolean; onClose: () => void };
  let { isClosing = false, onClose }: Props = $props();

  let snap = $state<PromptInspect | null>(null);
  let err = $state<string | null>(null);
  let sel = $state(0);
  let copied = $state(false);

  const rows = $derived(snap ? inspectRows(snap) : []);
  const ins = $derived(snap ? promptInsights(snap) : null);
  const active = $derived(rows[sel] ?? rows[0]);
  const activeLabel = $derived(
    active
      ? active.kind === "msg"
        ? active.part.label
        : active.tool.name
      : "Prompt part",
  );

  $effect(() => {
    const id = sessionStore.getSnapshot() || undefined;
    inspectPrompt(id)
      .then((s) => {
        snap = s;
        err = null;
        sel = 0;
      })
      .catch((e: unknown) => {
        err = String(e).replace(/^Error:\s*/, "");
      });
  });

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  async function onCopy() {
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
    <aside class="settings-rail">
      <div class="settings-brand">
        <LogoMark size={22} />
        <div>
          <strong>Prompt</strong>
          <span>Assembly inspector</span>
        </div>
      </div>

      <nav class="settings-nav prompt-rail-nav" aria-label="Prompt parts">
        {#if !snap && !err}
          <div class="prompt-rail-loading">Loading parts…</div>
        {:else if err}
          <div class="prompt-rail-loading">Unavailable</div>
        {:else if rows.length === 0}
          <div class="prompt-rail-loading">No parts yet</div>
        {:else}
          {#each rows as row, i}
            {@const layer = ins?.layers[i]}
            {@const label = row.kind === "msg" ? row.part.label : row.tool.name}
            {@const hint = row.kind === "msg" ? row.part.role : "tool def"}
            {@const tokens = row.kind === "msg" ? row.part.tokens : row.tool.tokens}
            <button
              type="button"
              class={`settings-nav-btn${sel === i ? " active" : ""}`}
              onclick={() => (sel = i)}
            >
              <span class="settings-nav-icon">
                <Icon icon={row.kind === "tool" ? Workflow : Eye} size={14} />
              </span>
              <span class="settings-nav-copy">
                <em>
                  <span class="prompt-ord">{layer?.order ?? i + 1}</span>
                  {label}
                </em>
                <small>{hint} · {fmtTokens(tokens)}</small>
              </span>
            </button>
          {/each}
        {/if}
      </nav>

      <div class="settings-rail-foot">
        <span>{snap ? `~${fmtTokens(snap.totalTokens)} tokens` : "Prompt study"}</span>
      </div>
    </aside>

    <section class="settings-main">
      <header class="settings-main-head">
        <div>
          <h2>{snap ? activeLabel : "Prompt assembly"}</h2>
          <p>
            {#if snap}
              {snap.sent
                ? "Wire order, budget share, and optimization hints"
                : "Preview — L0 + tools until a turn is sent"}
            {:else if err}
              Could not load prompt assembly
            {:else}
              Loading prompt study…
            {/if}
          </p>
        </div>
        <div class="prompt-head-actions">
          <button
            type="button"
            class="settings-close"
            onclick={() => void onCopy()}
            disabled={!snap}
            aria-label="Copy prompt study"
          >
            <Icon icon={Copy} size={13} />
            <span>{copied ? "Copied" : "Copy"}</span>
          </button>
          <button type="button" class="settings-close" onclick={onClose} aria-label="Close prompt inspector">
            <Icon icon={X} size={14} />
            <kbd>Esc</kbd>
          </button>
        </div>
      </header>

      <div class="settings-content-wrap">
        <div class="settings-content-body prompt-inspect-body-wrap">
          {#if err}
            <p class="prompt-inspect-err">{err}</p>
          {:else if !snap || !ins}
            <div class="settings-loading">Loading prompt assembly…</div>
          {:else}
            <div class="prompt-inspect-overview">
              <div class="prompt-overview-card">
                <em>Active Model</em>
                <strong>{snap.model}</strong>
              </div>
              <div class="prompt-overview-card">
                <em>Total Request</em>
                <strong>~{fmtTokens(snap.totalTokens)} ({snap.messages.length} msgs · {snap.tools.length} tools)</strong>
              </div>
              <div class="prompt-overview-card">
                <em>Largest Sink</em>
                <strong title={ins.largest.name}>{ins.largest.name} · {pct(ins.largest.share)}</strong>
              </div>
              <div class="prompt-overview-card">
                <em>Cache Stability</em>
                <strong>{ins.stablePrefix} stable</strong>
              </div>
            </div>

            <PromptInspectChart {ins} />

            {#if ins.hints.length > 0}
              <ul class="prompt-inspect-hints">
                {#each ins.hints as h}
                  <li>{h}</li>
                {/each}
              </ul>
            {/if}

            {#if ins.layers[sel]}
              <div class="prompt-part-stats">
                <span class="stat-badge">~{fmtTokens(ins.layers[sel].tokens)}</span>
                <span class="stat-badge">{ins.layers[sel].chars.toLocaleString()} chars</span>
                <span class="stat-badge">{ins.layers[sel].lines} lines</span>
                <span class="stat-badge">{pct(ins.layers[sel].share)} of budget</span>
                <span class={`stat-badge ${ins.layers[sel].cacheable ? "cacheable" : "volatile"}`}>
                  {ins.layers[sel].cacheable ? "Cacheable" : "Volatile"}
                </span>
                <span class="stat-badge">Wire #{ins.layers[sel].order}</span>
              </div>
            {/if}

            <pre class="prompt-inspect-content">{active ? inspectBody(active) : ""}</pre>
          {/if}
        </div>
      </div>
    </section>
  </div>
</div>
