<script lang="ts">
  import {
    categorizeRow,
    categoryMeta,
    type InspectRow,
  } from "$lib/promptInspectView";
  import Icon, {
    Brain,
    Check,
    Copy,
    FolderGit2,
    MessageSquare,
    Wrench,
  } from "$lib/ui/icons";
  import { fmtTokens } from "$lib/util";

  type Props = {
    activeRow: InspectRow | undefined;
    rawContent: string;
  };

  let { activeRow, rawContent }: Props = $props();

  let copied = $state(false);
  let viewMode = $state<"formatted" | "raw">("formatted");
  let wrap = $state(true);

  const lines = $derived(rawContent ? rawContent.split("\n") : []);
  const cat = $derived(activeRow ? categorizeRow(activeRow) : "instructions");
  const meta = $derived(categoryMeta(cat));
  const title = $derived(
    activeRow
      ? activeRow.kind === "msg"
        ? activeRow.part.label
        : activeRow.tool.name
      : "No section selected",
  );
  const tokens = $derived(
    activeRow
      ? activeRow.kind === "msg"
        ? activeRow.part.tokens
        : activeRow.tool.tokens
      : 0,
  );

  function getCatIcon(c: typeof cat) {
    if (c === "instructions") return Brain;
    if (c === "project") return FolderGit2;
    if (c === "capabilities") return Wrench;
    return MessageSquare;
  }

  async function onCopy() {
    if (!rawContent) return;
    try {
      await navigator.clipboard.writeText(rawContent);
      copied = true;
      setTimeout(() => (copied = false), 1400);
    } catch (e) {
      console.error(e);
    }
  }
</script>

<div class="prompt-content-viewer">
  <div class="prompt-viewer-header">
    <div class="prompt-viewer-title-group">
      <div class={`prompt-viewer-role-badge cat-${cat}`} style={`color: ${meta.color}`}>
        <Icon icon={getCatIcon(cat)} size={13} />
        <span>{meta.label}</span>
      </div>
      <h3 class="prompt-viewer-title">{title}</h3>
      <span class="prompt-token-pill">~{fmtTokens(tokens)} tokens</span>
    </div>

    <div class="prompt-viewer-actions">
      <!-- Formatted vs Raw Mode Toggle -->
      <div class="prompt-mode-toggle" role="tablist">
        <button
          type="button"
          class={`prompt-mode-btn${viewMode === "formatted" ? " active" : ""}`}
          onclick={() => (viewMode = "formatted")}
          title="Human-readable formatted document"
        >
          Reader
        </button>
        <button
          type="button"
          class={`prompt-mode-btn${viewMode === "raw" ? " active" : ""}`}
          onclick={() => (viewMode = "raw")}
          title="Exact wire text with line numbers"
        >
          Raw
        </button>
      </div>

      {#if viewMode === "raw"}
        <button
          type="button"
          class={`prompt-wrap-toggle${wrap ? " active" : ""}`}
          onclick={() => (wrap = !wrap)}
          title="Toggle word wrap"
        >
          <span>Wrap</span>
        </button>
      {/if}

      <button
        type="button"
        class="prompt-copy-layer-btn"
        onclick={() => void onCopy()}
        disabled={!rawContent}
        title="Copy this section's text"
      >
        <Icon icon={copied ? Check : Copy} size={13} />
        <span>{copied ? "Copied" : "Copy"}</span>
      </button>
    </div>
  </div>

  <!-- Non-technical friendly context explanation banner -->
  <div class="prompt-viewer-explain-banner">
    <p>{meta.desc}</p>
  </div>

  <div class="prompt-doc-viewport">
    {#if !rawContent}
      <div class="prompt-code-empty">No content in this section</div>
    {:else if viewMode === "formatted"}
      <article class="prompt-reader-doc">
        <pre class="prompt-reader-pre">{rawContent}</pre>
      </article>
    {:else}
      <div class={`prompt-code-container${wrap ? " is-wrapped" : ""}`}>
        <div class="prompt-code-lines">
          {#each lines as line, i}
            <div class="prompt-code-line">
              <span class="prompt-line-num">{i + 1}</span>
              <span class="prompt-line-text">{line || "\n"}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>

