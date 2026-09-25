<script lang="ts">
  import { inspectRequest } from "$lib/commands";
  import { outlineGroups, stepSelection } from "$lib/domain/inspectOutline";
  import { parseInspectRequest, type PromptInspect } from "$lib/domain/requestInspect";
  import { categorizeRow, inspectCopyText, inspectRows, type ContextCategory } from "$lib/promptInspectView";
  import { errorText, sessions } from "$lib/runtime";
  import { EmptyState } from "$lib/ui";
  import { copyFeedback } from "$lib/ui/copyFeedback.svelte";
  import Icon, { Brain, Check, ChevronLeft, Copy } from "$lib/ui/icons";
  import WindowControlsMaybe from "../chrome/WindowControlsMaybe.svelte";
  import InspectorInsights from "./InspectorInsights.svelte";
  import InspectorMap from "./InspectorMap.svelte";
  import InspectorOutline from "./InspectorOutline.svelte";
  import InspectorReader from "./InspectorReader.svelte";

  /**
   * The last request sent to the model, part by part: a map of what fills
   * the window, an outline to move through, and each part to read.
   */
  type Props = { isClosing?: boolean; onClose: () => void };
  let { isClosing = false, onClose }: Props = $props();

  let snap = $state<PromptInspect | null>(null);
  let err = $state<string | null>(null);
  let selected = $state(0);
  let query = $state("");
  let only = $state<ContextCategory | null>(null);
  const copied = copyFeedback();

  const rows = $derived(snap ? inspectRows(snap) : []);
  const groups = $derived(outlineGroups(rows, query, only));
  const visible = $derived(groups.flatMap((g) => g.items.map((i) => i.index)));
  const limit = $derived(sessions.active?.contextLimit || 200_000);
  const totals = $derived.by(() => {
    const t: Record<ContextCategory, number> = { instructions: 0, project: 0, conversation: 0, capabilities: 0 };
    for (const row of rows) t[categorizeRow(row)] += row.kind === "msg" ? row.part.tokens : row.tool.tokens;
    return t;
  });

  $effect(() => {
    const id = sessions.activeId;
    snap = null;
    err = null;
    if (!id) {
      err = "Open a chat and send a message; its request shows up here.";
      return;
    }
    inspectRequest(id)
      .then((raw) => {
        snap = parseInspectRequest(raw);
        err = snap ? null : "No request yet. Send a message first.";
        selected = 0;
      })
      .catch((e: unknown) => (err = errorText(e)));
  });

  // What is selected stays visible when the outline is filtered.
  $effect(() => {
    if (visible.length && !visible.includes(selected)) selected = visible[0] ?? 0;
  });

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      e.preventDefault();
      onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function step(dir: -1 | 1) {
    const next = stepSelection(visible, selected, dir);
    if (next !== null) selected = next;
  }
</script>

<div class="inspector-overlay" class:is-closing={isClosing} role="presentation">
  <div class="inspector-page" class:is-closing={isClosing} role="dialog" tabindex="-1" aria-label="Prompt inspector">
    <header class="app-titlebar inspector-head" data-tauri-drag-region>
      <div class="titlebar-side" data-tauri-drag-region>
        <button type="button" class="icon-btn" aria-label="Back" title="Back (Esc)" onclick={onClose}>
          <Icon icon={ChevronLeft} size={15} />
        </button>
        <h1 class="inspector-title">Prompt</h1>
        {#if snap}<span class="inspector-model" title={snap.model}>{snap.model}</span>{/if}
      </div>
      <div class="titlebar-side" data-tauri-drag-region>
        <button type="button" class="btn-ghost inspector-copy" disabled={!snap} onclick={() => snap && void copied.copy(inspectCopyText(snap))}>
          <Icon icon={copied.copied ? Check : Copy} size={13} />
          {copied.copied ? "Copied" : "Copy all"}
        </button>
        <WindowControlsMaybe />
      </div>
    </header>

    {#if err}
      <div class="inspector-state"><EmptyState icon={Brain} title="Nothing to inspect yet" description={err} /></div>
    {:else if !snap}
      <p class="inspector-state inspector-loading">Reading the last request…</p>
    {:else}
      <div class="inspector-body">
        <aside class="inspector-side">
          <InspectorMap {totals} used={snap.totalTokens} {limit} active={only} onPick={(c) => (only = c)} />
          <InspectorOutline {groups} {selected} {query} total={rows.length} onQuery={(q) => (query = q)} onSelect={(i) => (selected = i)} onStep={step} />
          <InspectorInsights {snap} />
        </aside>
        <section class="inspector-main">
          <InspectorReader row={rows[selected]} totalTokens={snap.totalTokens} />
        </section>
      </div>
    {/if}
  </div>
</div>
