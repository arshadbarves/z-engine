<script lang="ts">
  import { parseActivityLedger } from "$lib/activity";
  import { activityBrief } from "$lib/toolUi";
  import type { Msg } from "$lib/types";
  import Icon, {
    Brain,
    ChevronRight,
    Copy,
    FilePenLine,
    FileText,
    Search,
    SquareTerminal,
  } from "$lib/ui/icons";

  type Props = { items: Msg[] };
  let { items }: Props = $props();

  const brief = $derived(activityBrief(items));
  const isRunning = $derived(items.some((t) => t.streaming));

  let isExpanded = $state(false);
  let activeTab = $state<"all" | "files" | "searches" | "terminal" | "reason">("all");
  let openBashIds = $state<Record<number, boolean>>({});
  let openReasonIds = $state<Record<number, boolean>>({});
  let copiedBashId = $state<number | null>(null);

  function toggleBash(id: number) {
    openBashIds = { ...openBashIds, [id]: !openBashIds[id] };
  }

  function toggleReason(id: number) {
    openReasonIds = { ...openReasonIds, [id]: !openReasonIds[id] };
  }

  function copyLog(id: number, text: string) {
    navigator.clipboard.writeText(text);
    copiedBashId = id;
    setTimeout(() => {
      if (copiedBashId === id) copiedBashId = null;
    }, 1500);
  }

  const parsed = $derived(parseActivityLedger(items));
</script>

<div class={`process-section${isRunning ? " running" : ""}${isExpanded ? " expanded" : ""}`}>
  <button
    type="button"
    class="process-trigger"
    onclick={() => (isExpanded = !isExpanded)}
    aria-expanded={isExpanded}
    title={isExpanded ? "Hide process inspector" : "Inspect what happened under the hood"}
  >
    <span class="process-indicator">
      {#if isRunning}
        <span class="process-pulse-dot" aria-hidden="true"></span>
      {:else}
        <span class="process-glyph" aria-hidden="true">✻</span>
      {/if}
      <span class="process-brief">{brief}</span>
    </span>
    <span class="process-action-hint">
      <span class="hint-text">{isExpanded ? "Hide" : "Inspect"}</span>
      <span class={`process-chevron${isExpanded ? " open" : ""}`}>
        <Icon icon={ChevronRight} size={10} strokeWidth={2.2} />
      </span>
    </span>
  </button>

  {#if isExpanded}
    <div class="process-inspector">
      <!-- Apple Segmented Control Bar -->
      <div class="inspector-segmented-bar" role="tablist">
        <button
          type="button"
          class="segment-btn"
          class:active={activeTab === "all"}
          onclick={() => (activeTab = "all")}
          role="tab"
          aria-selected={activeTab === "all"}
        >
          <span>All</span>
          <span class="segment-badge">{parsed.all.length}</span>
        </button>

        {#if parsed.files.length > 0}
          <button
            type="button"
            class="segment-btn"
            class:active={activeTab === "files"}
            onclick={() => (activeTab = "files")}
            role="tab"
            aria-selected={activeTab === "files"}
          >
            <span>Files</span>
            <span class="segment-badge">{parsed.files.length}</span>
          </button>
        {/if}

        {#if parsed.searches.length > 0}
          <button
            type="button"
            class="segment-btn"
            class:active={activeTab === "searches"}
            onclick={() => (activeTab = "searches")}
            role="tab"
            aria-selected={activeTab === "searches"}
          >
            <span>Searches</span>
            <span class="segment-badge">{parsed.searches.length}</span>
          </button>
        {/if}

        {#if parsed.terminal.length > 0}
          <button
            type="button"
            class="segment-btn"
            class:active={activeTab === "terminal"}
            onclick={() => (activeTab = "terminal")}
            role="tab"
            aria-selected={activeTab === "terminal"}
          >
            <span>Terminal</span>
            <span class="segment-badge">{parsed.terminal.length}</span>
          </button>
        {/if}

        {#if parsed.thoughts.length > 0}
          <button
            type="button"
            class="segment-btn"
            class:active={activeTab === "reason"}
            onclick={() => (activeTab = "reason")}
            role="tab"
            aria-selected={activeTab === "reason"}
          >
            <span>Reasoning</span>
          </button>
        {/if}
      </div>

      <!-- Inspector Content Deck -->
      <div class="inspector-deck">
        {#if activeTab === "all"}
          <div class="ledger-list">
            {#each parsed.all as e (e.id)}
              <div class="ledger-row">
                <span class={`badge-tag ${e.category}`}>{e.category}</span>
                <div class="ledger-content">
                  <span class="ledger-title" title={e.title}>{e.title}</span>
                  {#if e.sub}<span class="ledger-sub" title={e.sub}>{e.sub}</span>{/if}
                </div>
                <div class="ledger-trailing">
                  {#if e.metric}<span class="ledger-metric">{e.metric}</span>{/if}
                  {#if e.category === "bash" && e.output}
                    <button
                      type="button"
                      class="ledger-toggle"
                      onclick={() => toggleBash(e.id)}
                    >
                      {openBashIds[e.id] ? "Hide" : "Output"}
                    </button>
                  {:else if e.category === "thought" && e.body}
                    <button
                      type="button"
                      class="ledger-toggle"
                      onclick={() => toggleReason(e.id)}
                    >
                      {openReasonIds[e.id] ? "Hide" : "Inspect"}
                    </button>
                  {/if}
                </div>
              </div>
              {#if openBashIds[e.id] && e.output}
                <div class="ledger-expand-block">
                  <pre>{e.output}</pre>
                </div>
              {/if}
              {#if openReasonIds[e.id] && e.body}
                <div class="ledger-expand-thought">
                  {e.body}
                </div>
              {/if}
            {/each}
          </div>

        {:else if activeTab === "files"}
          <div class="file-grid">
            {#each parsed.files as f (f.id)}
              <div class="file-tile" title={f.sub || f.title}>
                <span class={`file-tile-icon ${f.category}`}>
                  <Icon icon={f.category === "edit" ? FilePenLine : FileText} size={13} />
                </span>
                <div class="file-tile-info">
                  <span class="file-tile-name">{f.title}</span>
                  {#if f.sub}<span class="file-tile-path">{f.sub}</span>{/if}
                </div>
                {#if f.metric}<span class="file-tile-metric">{f.metric}</span>{/if}
              </div>
            {/each}
          </div>

        {:else if activeTab === "searches"}
          <div class="ledger-list">
            {#each parsed.searches as s (s.id)}
              <div class="ledger-row">
                <span class="search-tile-icon"><Icon icon={Search} size={13} /></span>
                <div class="ledger-content">
                  <code class="search-query">{s.title}</code>
                  {#if s.sub}<span class="ledger-sub">{s.sub}</span>{/if}
                </div>
                {#if s.metric}<span class="ledger-metric">{s.metric}</span>{/if}
              </div>
            {/each}
          </div>

        {:else if activeTab === "terminal"}
          <div class="terminal-deck">
            {#each parsed.terminal as b (b.id)}
              <div class="terminal-item">
                <div class="terminal-header">
                  <span class="terminal-icon"><Icon icon={SquareTerminal} size={13} /></span>
                  <code class="terminal-cmd">{b.title}</code>
                  <div class="ledger-trailing">
                    {#if b.metric}<span class="ledger-metric">{b.metric}</span>{/if}
                    {#if b.output}
                      <button
                        type="button"
                        class="terminal-copy-btn"
                        onclick={() => copyLog(b.id, b.output!)}
                        title="Copy command output"
                      >
                        <Icon icon={Copy} size={11} />
                        <span>{copiedBashId === b.id ? "Copied" : "Copy"}</span>
                      </button>
                    {/if}
                  </div>
                </div>
                {#if b.output}
                  <pre class="terminal-box">{b.output}</pre>
                {/if}
              </div>
            {/each}
          </div>

        {:else if activeTab === "reason"}
          <div class="reason-deck">
            {#each parsed.thoughts as t (t.id)}
              <div class="reason-item">
                <div class="reason-header">
                  <span class="reason-icon"><Icon icon={Brain} size={13} /></span>
                  <span class="reason-title">{t.title}</span>
                </div>
                {#if t.body}
                  <div class="reason-body">{t.body}</div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

