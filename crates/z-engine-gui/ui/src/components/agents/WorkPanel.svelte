<script lang="ts">
  import { agentActivity, agentSections, agentTree, agentUsageRows, workCounts } from "$lib/domain/agentTree";
  import { sessions } from "$lib/runtime";
  import { ui, type WorkTab } from "$lib/stores/ui.svelte";
  import { Badge, Disclosure, EmptyState } from "$lib/ui";
  import Icon, { Bot, ChevronLeft, SquareTerminal, X } from "$lib/ui/icons";
  import { ticker } from "$lib/ui/ticker.svelte";
  import AgentRow from "./AgentRow.svelte";
  import AgentTranscript from "./AgentTranscript.svelte";
  import AgentUsage from "./AgentUsage.svelte";
  import ApplyCard from "./ApplyCard.svelte";
  import JobRow from "./JobRow.svelte";

  /**
   * The agents panel: work waiting for Apply first, then running helpers
   * with what each does now; finished ones and usage fold away. Jobs have
   * their own tab.
   */
  type Props = { isClosing?: boolean; onClose: () => void };
  let { isClosing = false, onClose }: Props = $props();

  const view = $derived(sessions.active);
  const agents = $derived(view?.agents ?? {});
  const jobs = $derived(view?.jobs ?? {});
  const tree = $derived(agentTree(agents));
  const sections = $derived(agentSections(tree));
  const jobList = $derived(Object.values(jobs).sort((a, b) => b.startedAt - a.startedAt));
  const counts = $derived(workCounts(agents, jobs));
  const usageRows = $derived(view ? agentUsageRows(agents, view.agentUsage, view.costUsd) : []);
  const focused = $derived(ui.agentTranscript ? (agents[ui.agentTranscript] ?? null) : null);
  const tab: WorkTab = $derived(ui.workPanel ?? "agents");
  const clock = ticker(() => counts.runningAgents + counts.runningJobs > 0);
  let finishedOpen = $state(false);
  let usageOpen = $state(false);

  function openAgent(agentId: string) {
    ui.openWork("agents", agentId);
  }

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape" || e.defaultPrevented || ui.settingsOpen || ui.inspectOpen || ui.paletteOpen) return;
      if ((e.target as HTMLElement | null)?.closest("textarea, input, [role='dialog'], [role='menu']")) return;
      if (focused) ui.agentTranscript = null;
      else onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<aside class={`work-panel${isClosing ? " is-closing" : ""}`} aria-label="Agents and jobs">
  <header class="work-head">
    {#if focused}
      <button type="button" class="icon-btn" aria-label="Back to agents" onclick={() => (ui.agentTranscript = null)}>
        <Icon icon={ChevronLeft} size={15} />
      </button>
      <span class="work-title" title={focused.description}>{focused.agentType} · {focused.description}</span>
    {:else}
      <div class="work-tabs" role="tablist" aria-label="Background work">
        <button type="button" role="tab" class="work-tab" class:is-on={tab === "agents"} aria-selected={tab === "agents"} onclick={() => ui.openWork("agents")}>
          <Icon icon={Bot} size={13} />
          Agents
          {#if counts.runningAgents}<span class="work-live" aria-label="running"></span>{/if}
          <Badge count={tree.length} />
        </button>
        <button type="button" role="tab" class="work-tab" class:is-on={tab === "jobs"} aria-selected={tab === "jobs"} onclick={() => ui.openWork("jobs")}>
          <Icon icon={SquareTerminal} size={13} />
          Jobs
          {#if counts.runningJobs}<span class="work-live" aria-label="running"></span>{/if}
          <Badge count={jobList.length} />
        </button>
      </div>
    {/if}
    <button type="button" class="icon-btn work-close" aria-label="Close panel" title="Close (Esc)" onclick={onClose}>
      <Icon icon={X} size={14} />
    </button>
  </header>

  <div class="work-body">
    {#if !view}
      <EmptyState icon={Bot} title="No chat open" description="Open a chat to see its helpers and background jobs." />
    {:else if focused}
      {#key focused.agentId}
        <AgentTranscript {view} agent={focused} onOpenAgent={openAgent} />
      {/key}
    {:else if tab === "agents"}
      {#if tree.length === 0}
        <EmptyState icon={Bot} title="No helpers yet" description="For broad work the agent sends helpers (subagents) off in parallel. They appear here." />
      {:else}
        {#if sections.ready.length}
          <section class="work-section" aria-label="Ready to apply">
            <h3 class="work-section-title">Ready to apply</h3>
            {#each sections.ready as node (node.info.agentId)}<ApplyCard agent={node.info} onOpen={openAgent} />{/each}
          </section>
        {/if}
        {#if sections.working.length}
          <section class="work-section" aria-label="Working">
            <h3 class="work-section-title">Working</h3>
            {#each sections.working as node (node.info.agentId)}
              <AgentRow {node} now={clock.now} activity={agentActivity(view.tools, node.info.agentId)} onOpen={openAgent} />
            {/each}
          </section>
        {/if}
        {#if sections.finished.length}
          <Disclosure bind:open={finishedOpen} class="work-fold" summaryClass="work-fold-summary">
            {#snippet summary()}<span>Finished</span><Badge count={sections.finished.length} />{/snippet}
            <div class="work-section">
              {#each sections.finished as node (node.info.agentId)}
                <AgentRow {node} now={clock.now} activity={null} onOpen={openAgent} />
              {/each}
            </div>
          </Disclosure>
        {/if}
        {#if usageRows.length > 1}
          <Disclosure bind:open={usageOpen} class="work-fold" summaryClass="work-fold-summary">
            {#snippet summary()}<span>Usage by agent</span>{/snippet}
            <AgentUsage rows={usageRows} />
          </Disclosure>
        {/if}
      {/if}
    {:else if jobList.length === 0}
      <EmptyState icon={SquareTerminal} title="No background jobs" description="Commands the agent leaves running, and background helpers, appear here." />
    {:else}
      <div class="work-section">
        {#each jobList as job (job.jobId)}<JobRow {job} now={clock.now} onOpenAgent={openAgent} />{/each}
      </div>
    {/if}
  </div>
</aside>
