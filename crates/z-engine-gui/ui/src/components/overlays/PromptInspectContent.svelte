<script lang="ts">
  import type { PromptLayer } from "$lib/promptInsights";
  import type { InspectRow } from "$lib/promptInspectView";
  import Icon, {
    Brain,
    Check,
    Copy,
    Eye,
    RefreshCw,
    Sparkles,
    Terminal,
    User,
    Workflow,
    Zap,
  } from "$lib/ui/icons";
  import { fmtTokens } from "$lib/util";

  type Props = {
    activeRow: InspectRow | undefined;
    activeLayer: PromptLayer | undefined;
    rawContent: string;
    totalTokens: number;
  };

  let { activeRow, activeLayer, rawContent, totalTokens }: Props = $props();

  let copied = $state(false);
  let wrap = $state(true);

  const lines = $derived(rawContent ? rawContent.split("\n") : []);
  const sharePct = $derived(
    activeLayer && totalTokens > 0 ? Math.round(activeLayer.share * 100) : 0,
  );

  function getRoleIcon(role: string | undefined, kind: "msg" | "tool" | undefined) {
    if (kind === "tool") return Workflow;
    if (role === "system") return Brain;
    if (role === "user") return User;
    if (role === "assistant") return Sparkles;
    if (role === "tool") return Terminal;
    return Eye;
  }

  async function onCopyContent() {
    if (!rawContent) return;
    try {
      await navigator.clipboard.writeText(rawContent);
      copied = true;
      window.setTimeout(() => {
        copied = false;
      }, 1200);
    } catch (e) {
      console.error("Copy failed", e);
    }
  }
</script>

<div class="prompt-content-viewer">
  <div class="prompt-viewer-header">
    <div class="prompt-viewer-title-group">
      <div class={`prompt-viewer-role-badge role-${(activeLayer?.role ?? "system").replace(/\s+/g, "-")}`}>
        <Icon icon={getRoleIcon(activeLayer?.role, activeRow?.kind)} size={14} />
        <span>{activeLayer?.role ?? "part"}</span>
      </div>
      <h3 class="prompt-viewer-title">
        {activeRow ? (activeRow.kind === "msg" ? activeRow.part.label : activeRow.tool.name) : "No part selected"}
      </h3>
      {#if activeLayer}
        <span class="prompt-wire-tag">Wire #{activeLayer.order}</span>
      {/if}
    </div>

    <div class="prompt-viewer-actions">
      <button
        type="button"
        class={`prompt-wrap-toggle${wrap ? " active" : ""}`}
        onclick={() => (wrap = !wrap)}
        title="Toggle word wrap"
      >
        <span>Wrap</span>
      </button>
      <button
        type="button"
        class="prompt-copy-layer-btn"
        onclick={() => void onCopyContent()}
        disabled={!rawContent}
        title="Copy part content"
      >
        <Icon icon={copied ? Check : Copy} size={13} />
        <span>{copied ? "Copied" : "Copy"}</span>
      </button>
    </div>
  </div>

  {#if activeLayer}
    <div class="prompt-viewer-meta-strip">
      <div class="prompt-meta-pill">
        <span class="meta-lbl">Tokens</span>
        <strong class="meta-val">~{fmtTokens(activeLayer.tokens)}</strong>
      </div>
      <div class="prompt-meta-pill">
        <span class="meta-lbl">Share</span>
        <strong class="meta-val">{sharePct}%</strong>
      </div>
      <div class="prompt-meta-pill">
        <span class="meta-lbl">Length</span>
        <strong class="meta-val">{activeLayer.chars.toLocaleString()} chars</strong>
      </div>
      <div class="prompt-meta-pill">
        <span class="meta-lbl">Lines</span>
        <strong class="meta-val">{lines.length}</strong>
      </div>
      <div class={`prompt-meta-pill cache-tag ${activeLayer.cacheable ? "cacheable" : "volatile"}`}>
        <span class="meta-lbl">Cache Status</span>
        <strong class="meta-val">
          <Icon icon={activeLayer.cacheable ? Zap : RefreshCw} size={11} />
          <span>{activeLayer.cacheable ? "Prefix Cacheable" : "Dynamic Turn"}</span>
        </strong>
      </div>
    </div>
  {/if}

  <div class={`prompt-code-container${wrap ? " is-wrapped" : ""}`}>
    {#if lines.length === 0}
      <div class="prompt-code-empty">No content in this part</div>
    {:else}
      <div class="prompt-code-lines">
        {#each lines as line, i}
          <div class="prompt-code-line">
            <span class="prompt-line-num">{i + 1}</span>
            <span class="prompt-line-text">{line || "\n"}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>
