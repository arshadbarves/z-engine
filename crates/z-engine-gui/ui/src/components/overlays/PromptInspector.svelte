<script lang="ts">
  import { inspectPrompt, type PromptInspect } from "$lib/commands";
  import {
    categorizeRow,
    inspectBody,
    inspectCopyText,
    inspectRows,
    type ContextCategory,
  } from "$lib/promptInspectView";
  import { sessionStore } from "$lib/runtime";
  import Icon, {
    AlertTriangle,
    Brain,
    Check,
    ChevronLeft,
    Copy,
  } from "$lib/ui/icons";
  import { fmtTokens } from "$lib/util";
  import LogoMark from "../chrome/LogoMark.svelte";
  import WindowControlsMaybe from "../chrome/WindowControlsMaybe.svelte";
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
  let activeCategory = $state<ContextCategory | "all">("all");

  const rows = $derived(snap ? inspectRows(snap) : []);
  const activeRow = $derived(rows[sel] ?? rows[0]);

  // Context category token distribution
  const categoryStats = $derived.by(() => {
    const stats = {
      instructions: 0,
      project: 0,
      conversation: 0,
      capabilities: 0,
      total: snap?.totalTokens ?? 0,
    };
    if (!snap) return stats;
    for (const r of rows) {
      const cat = categorizeRow(r);
      const tok = r.kind === "msg" ? r.part.tokens : r.tool.tokens;
      stats[cat] += tok;
    }
    return stats;
  });

  // Estimated max context window for the active model (200,000 standard)
  const maxContext = 200_000;
  const memoryPct = $derived(
    categoryStats.total > 0
      ? Math.min(100, Math.round((categoryStats.total / maxContext) * 100))
      : 0,
  );

  const memoryStatus = $derived.by(() => {
    if (memoryPct < 40) return { text: "Plenty of room", color: "#00d68f" };
    if (memoryPct < 75) return { text: "Normal usage", color: "#38bdf8" };
    if (memoryPct < 90) return { text: "Getting full", color: "#f5a623" };
    return { text: "Near limit", color: "#ff453a" };
  });

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
      setTimeout(() => (copied = false), 1400);
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
    aria-label="Context and memory inspector"
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
          <span class="settings-breadcrumb-root">Context & Memory</span>
          {#if snap?.model}
            <span class="settings-breadcrumb-sep">/</span>
            <span class="prompt-model-badge" title={snap.model}>{snap.model}</span>
          {/if}
        </div>
      </div>

      <div class="topbar-center" data-tauri-drag-region>
        <div class="settings-topbar-pill">
          <LogoMark size={14} />
          <span>Active Context Inspector</span>
        </div>
      </div>

      <div class="topbar-right" data-tauri-drag-region>
        <button
          type="button"
          class="settings-copy-btn"
          onclick={() => void onCopyAll()}
          disabled={!snap}
          aria-label="Copy all context"
          title="Copy entire context and instructions to clipboard"
        >
          <Icon icon={copied ? Check : Copy} size={13} />
          <span>{copied ? "Context Copied" : "Copy All Context"}</span>
        </button>
        <WindowControlsMaybe />
      </div>
    </header>

    <div class="app-body settings-body prompt-inspect-body">
      <PromptInspectSidebar
        {rows}
        selected={sel}
        onSelect={(idx) => (sel = idx)}
        {activeCategory}
        onSelectCategory={(cat) => (activeCategory = cat)}
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
                  <strong>Context Unavailable</strong>
                  <p>{err}</p>
                </div>
              </div>
            {:else if !snap}
              <div class="settings-loading">
                <Icon icon={Brain} size={24} />
                <span>Reading assistant context and active memory…</span>
              </div>
            {:else}
              <!-- Unified Apple Memory Health Meter (Zero Duplicates) -->
              <div class="prompt-memory-card">
                <div class="memory-card-header">
                  <div class="memory-title-wrap">
                    <span class="memory-card-title">Memory Capacity</span>
                    <span class="memory-card-sub">
                      ~{fmtTokens(categoryStats.total)} tokens used ({memoryPct}% capacity) · 
                      <span style={`color: ${memoryStatus.color}; font-weight: 550;`}>{memoryStatus.text}</span>
                    </span>
                  </div>
                </div>

                <!-- Multi-Segmented Proportional Memory Track -->
                <div class="memory-track" aria-label="Context memory breakdown">
                  {#if categoryStats.instructions > 0}
                    <div
                      class="memory-segment seg-instructions"
                      style={`width: ${(categoryStats.instructions / Math.max(1, categoryStats.total)) * 100}%;`}
                      title={`Instructions: ~${fmtTokens(categoryStats.instructions)} tokens`}
                    ></div>
                  {/if}
                  {#if categoryStats.project > 0}
                    <div
                      class="memory-segment seg-project"
                      style={`width: ${(categoryStats.project / Math.max(1, categoryStats.total)) * 100}%;`}
                      title={`Project Knowledge: ~${fmtTokens(categoryStats.project)} tokens`}
                    ></div>
                  {/if}
                  {#if categoryStats.conversation > 0}
                    <div
                      class="memory-segment seg-conversation"
                      style={`width: ${(categoryStats.conversation / Math.max(1, categoryStats.total)) * 100}%;`}
                      title={`Conversation: ~${fmtTokens(categoryStats.conversation)} tokens`}
                    ></div>
                  {/if}
                  {#if categoryStats.capabilities > 0}
                    <div
                      class="memory-segment seg-capabilities"
                      style={`width: ${(categoryStats.capabilities / Math.max(1, categoryStats.total)) * 100}%;`}
                      title={`Capabilities: ~${fmtTokens(categoryStats.capabilities)} tokens`}
                    ></div>
                  {/if}
                </div>

                <!-- Interactive Category Pills -->
                <div class="memory-pills-row">
                  <button
                    type="button"
                    class={`memory-pill pill-instructions${activeCategory === "instructions" ? " active" : ""}`}
                    onclick={() => (activeCategory = activeCategory === "instructions" ? "all" : "instructions")}
                  >
                    <span class="pill-dot seg-instructions"></span>
                    <span class="pill-label">Instructions</span>
                    <span class="pill-stat">~{fmtTokens(categoryStats.instructions)}</span>
                  </button>
                  <button
                    type="button"
                    class={`memory-pill pill-project${activeCategory === "project" ? " active" : ""}`}
                    onclick={() => (activeCategory = activeCategory === "project" ? "all" : "project")}
                  >
                    <span class="pill-dot seg-project"></span>
                    <span class="pill-label">Project</span>
                    <span class="pill-stat">~{fmtTokens(categoryStats.project)}</span>
                  </button>
                  <button
                    type="button"
                    class={`memory-pill pill-conversation${activeCategory === "conversation" ? " active" : ""}`}
                    onclick={() => (activeCategory = activeCategory === "conversation" ? "all" : "conversation")}
                  >
                    <span class="pill-dot seg-conversation"></span>
                    <span class="pill-label">Chat</span>
                    <span class="pill-stat">~{fmtTokens(categoryStats.conversation)}</span>
                  </button>
                  <button
                    type="button"
                    class={`memory-pill pill-capabilities${activeCategory === "capabilities" ? " active" : ""}`}
                    onclick={() => (activeCategory = activeCategory === "capabilities" ? "all" : "capabilities")}
                  >
                    <span class="pill-dot seg-capabilities"></span>
                    <span class="pill-label">Tools</span>
                    <span class="pill-stat">~{fmtTokens(categoryStats.capabilities)}</span>
                  </button>
                </div>
              </div>

              <!-- Apple Reader Section Viewer -->
              <PromptInspectContent
                {activeRow}
                rawContent={activeRow ? inspectBody(activeRow) : ""}
              />
            {/if}
          </div>
        </div>
      </section>
    </div>
  </div>
</div>



