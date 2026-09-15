<script lang="ts">
  import { parseActivityLedger, type LedgerEntry } from "$lib/activity";
  import {
    nextActivityTabIndex,
    resolveActivityTab,
    shouldRestoreProcessTrigger,
  } from "$lib/domain/activityTabs";
  import {
    transitionProcessRun,
    toggleProcessDisclosure,
    type ProcessDisclosure,
  } from "$lib/domain/processDisclosure";
  import { activityBrief } from "$lib/toolUi";
  import type { Msg } from "$lib/types";
  import Icon, { ChevronRight } from "$lib/ui/icons";
  import { tick } from "svelte";
  import ToolActivityEntry from "./ToolActivityEntry.svelte";

  type ActivityTab = "all" | "files" | "searches" | "terminal" | "reason";
  type ActivityTabOption = { id: ActivityTab; label: string; entries: LedgerEntry[] };
  type Props = { items: Msg[]; expanded?: boolean };

  let { items, expanded = $bindable(false) }: Props = $props();
  let activeTab = $state<ActivityTab>("all");
  let wasRunning = $state(false);
  let autoOpened = $state(false);
  let preRunExpanded = $state(false);
  let tabButtons = $state<Partial<Record<ActivityTab, HTMLButtonElement>>>({});
  let tablistElement = $state<HTMLDivElement>();
  let inspectorElement = $state<HTMLDivElement>();
  let processTrigger = $state<HTMLButtonElement>();
  let focusWasInTablist = false;

  const parsed = $derived(parseActivityLedger(items));
  const brief = $derived(activityBrief(items));
  const isRunning = $derived(items.some((item) => item.streaming));
  const tabs = $derived.by(() =>
    [
      { id: "all", label: "All", entries: parsed.all },
      { id: "files", label: "Files", entries: parsed.files },
      { id: "searches", label: "Searches", entries: parsed.searches },
      { id: "terminal", label: "Terminal", entries: parsed.terminal },
      { id: "reason", label: "Reasoning", entries: parsed.thoughts },
    ].filter((tab) => tab.id === "all" || tab.entries.length > 0) as ActivityTabOption[],
  );
  const visibleEntries = $derived(tabs.find((tab) => tab.id === activeTab)?.entries ?? parsed.all);
  const panelId = $derived(`process-panel-${items[0]?.id ?? "empty"}`);

  function applyDisclosure(next: ProcessDisclosure) {
    expanded = next.expanded;
    autoOpened = next.autoOpened;
    preRunExpanded = next.preRunExpanded;
  }

  function toggleProcess() {
    applyDisclosure(toggleProcessDisclosure({ expanded, autoOpened, preRunExpanded }));
  }

  $effect(() => {
    const current: ProcessDisclosure = { expanded, autoOpened, preRunExpanded };
    const next = transitionProcessRun(current, wasRunning, isRunning);
    if (isRunning) {
      if (!wasRunning) applyDisclosure(next);
      wasRunning = true;
      return;
    }
    if (!wasRunning) return;
    const focusInsideInspector =
      typeof document !== "undefined" &&
      (inspectorElement?.contains(document.activeElement) ?? false);
    // Focus only needs rescuing when the panel it lives in disappears.
    const restoreFocus =
      current.expanded &&
      !next.expanded &&
      shouldRestoreProcessTrigger(wasRunning, isRunning, focusInsideInspector);
    applyDisclosure(next);
    wasRunning = false;
    if (restoreFocus) void focusProcessTrigger();
  });

  $effect.pre(() => {
    if (
      !tabs.some((tab) => tab.id === activeTab) &&
      typeof document !== "undefined"
    ) {
      focusWasInTablist = tablistElement?.contains(document.activeElement) ?? false;
    }
  });

  $effect(() => {
    const resolution = resolveActivityTab(
      activeTab,
      tabs.map((tab) => tab.id),
      focusWasInTablist,
    );
    focusWasInTablist = false;
    if (resolution.active === activeTab) return;
    activeTab = resolution.active;
    if (resolution.restoreFocus) void focusTab(resolution.active);
  });

  async function focusTab(tab: ActivityTab) {
    await tick();
    tabButtons[tab]?.focus();
  }

  async function focusProcessTrigger() {
    await tick();
    processTrigger?.focus();
  }

  async function handleTabKeydown(event: KeyboardEvent, index: number) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const nextIndex = nextActivityTabIndex(index, tabs.length, event.key);
    activeTab = tabs[nextIndex].id;
    await focusTab(activeTab);
  }
</script>

<section class={`process-section${isRunning ? " running" : ""}${expanded ? " expanded" : ""}`}>
  <!-- The trigger stays operable while streaming: reporting it as disabled
       while its panel is open and updating would be a lie to a screen reader. -->
  <button
    type="button"
    class="process-trigger"
    aria-expanded={expanded}
    bind:this={processTrigger}
    onclick={toggleProcess}
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
      <span class="hint-text">{expanded ? "Hide" : isRunning ? "Live" : "Inspect"}</span>
      <span class={`process-chevron${expanded ? " open" : ""}`}>
        <Icon icon={ChevronRight} size={10} strokeWidth={2.2} />
      </span>
    </span>
  </button>

  {#if expanded}
    <div class="process-inspector" bind:this={inspectorElement}>
      <div
        class="inspector-segmented-bar"
        role="tablist"
        aria-label="Process details"
        bind:this={tablistElement}
      >
        {#each tabs as tab, index (tab.id)}
          <button
            type="button"
            class="segment-btn"
            class:active={activeTab === tab.id}
            role="tab"
            id={`${panelId}-${tab.id}`}
            aria-controls={panelId}
            aria-selected={activeTab === tab.id}
            tabindex={activeTab === tab.id ? 0 : -1}
            bind:this={tabButtons[tab.id]}
            onclick={() => (activeTab = tab.id)}
            onkeydown={(event) => void handleTabKeydown(event, index)}
          >
            {tab.label} <span class="segment-badge">{tab.entries.length}</span>
          </button>
        {/each}
      </div>

      <div
        class="inspector-deck"
        role="tabpanel"
        id={panelId}
        aria-labelledby={`${panelId}-${activeTab}`}
      >
        {#each visibleEntries as entry (entry.id)}
          <ToolActivityEntry {entry} />
        {/each}
      </div>
    </div>
  {/if}
</section>
