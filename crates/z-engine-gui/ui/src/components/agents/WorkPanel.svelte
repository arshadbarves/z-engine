<script lang="ts">
  import { agentTree, agentUsageRows, isAgentDone, workCounts } from "$lib/domain/agentTree";
  import { sessions } from "$lib/runtime";
  import { ui, type WorkTab } from "$lib/stores/ui.svelte";
  import Icon, { Bot, ChevronLeft, X } from "$lib/ui/icons";
  import { ticker } from "$lib/ui/ticker.svelte";
  import AgentRow from "./AgentRow.svelte";
  import AgentTranscript from "./AgentTranscript.svelte";
  import AgentUsage from "./AgentUsage.svelte";
  import JobRow from "./JobRow.svelte";

  type Props = { isClosing?: boolean; onClose: () => void };
  let { isClosing = false, onClose }: Props = $props();

  const view = $derived(sessions.active);
  const agents = $derived(view?.agents ?? {});
  const jobs = $derived(view?.jobs ?? {});
  const tree = $derived(agentTree(agents));
  const jobList = $derived(Object.values(jobs).sort((a, b) => b.startedAt - a.startedAt));
  const counts = $derived(workCounts(agents, jobs));
  const usageRows = $derived(view ? agentUsageRows(agents, view.agentUsage, view.costUsd) : []);
  const focused = $derived(ui.agentTranscript ? (agents[ui.agentTranscript] ?? null) : null);
  const tab: WorkTab = $derived(ui.workPanel ?? "agents");
  const clock = ticker(() => counts.runningAgents + counts.runningJobs > 0);

  function openAgent(agentId: string) {
    ui.openWork("agents", agentId);
  }
</script>

<aside class={`work-panel${isClosing ? " is-closing" : ""}`} aria-label="Agents and jobs">
  <div class="work-head">
    {#if focused}
      <button type="button" class="icon-btn" title="Back to agents" onclick={() => (ui.agentTranscript = null)}>
        <Icon icon={ChevronLeft} size={14} />
      </button>
      <Icon icon={Bot} size={13} class="work-head-icon" />
      <span class="work-title" title={focused.description}>{focused.agentType} · {focused.description}</span>
    {:else}
      <div class="work-tabs" role="tablist" aria-label="Background work">
        <button
          type="button"
          role="tab"
          class={`work-tab${tab === "agents" ? " active" : ""}`}
          aria-selected={tab === "agents"}
          onclick={() => ui.openWork("agents")}
        >
          Agents <span class="work-count">{tree.length}</span>
          {#if counts.runningAgents > 0}<span class="work-live-dot" aria-label="running"></span>{/if}
        </button>
        <button
          type="button"
          role="tab"
          class={`work-tab${tab === "jobs" ? " active" : ""}`}
          aria-selected={tab === "jobs"}
          onclick={() => ui.openWork("jobs")}
        >
          Jobs <span class="work-count">{jobList.length}</span>
          {#if counts.runningJobs > 0}<span class="work-live-dot" aria-label="running"></span>{/if}
        </button>
      </div>
    {/if}
    <span class="work-head-spacer"></span>
    <button type="button" class="icon-btn" title="Close panel" aria-label="Close panel" onclick={onClose}>
      <Icon icon={X} size={13} />
    </button>
  </div>

  <div class="work-body">
    {#if !view}
      <p class="work-empty">Open a chat to see its agents and jobs.</p>
    {:else if focused}
      {#key focused.agentId}
        <AgentTranscript {view} agent={focused} onOpenAgent={openAgent} />
      {/key}
    {:else if tab === "agents"}
      {#if tree.length === 0}
        <p class="work-empty">No subagents yet. The agent starts them with the Agent tool.</p>
      {:else}
        <div class="agent-list">
          {#each tree as node (node.info.agentId)}
            <AgentRow {node} now={clock.now} onOpen={openAgent} />
          {/each}
        </div>
      {/if}
      {#if usageRows.length > 1 || tree.some((n) => !isAgentDone(n.info.status))}
        <AgentUsage rows={usageRows} />
      {/if}
    {:else if jobList.length === 0}
      <p class="work-empty">No background jobs. Shells started with run_in_background and background agents show here.</p>
    {:else}
      <div class="job-list">
        {#each jobList as job (job.jobId)}
          <JobRow {job} now={clock.now} onOpenAgent={openAgent} />
        {/each}
      </div>
    {/if}
  </div>
</aside>
