<script lang="ts">
  import { isAgentDone, type AgentNode } from "$lib/domain/agentTree";
  import { elapsed, plural } from "$lib/domain/format";
  import { totalTokens } from "$lib/domain/usage";
  import { Disclosure } from "$lib/ui";
  import Icon, { ArrowRight } from "$lib/ui/icons";
  import { fmtCost, fmtTokens, shortModel } from "$lib/util";
  import StatusChip from "./StatusChip.svelte";

  /**
   * One subagent: status, what it was asked, how long; while it runs, what
   * it is doing now. Model, usage and its result fold away.
   */
  type Props = { node: AgentNode; now: number; activity: string | null; onOpen: (agentId: string) => void };
  let { node, now, activity, onOpen }: Props = $props();

  const agent = $derived(node.info);
  const running = $derived(!isAgentDone(agent.status));
  const tokens = $derived(totalTokens(agent.usage));
  let open = $state(false);
</script>

<div class={`agent-row status-${agent.status}`} style={`--depth: ${node.depth}`}>
  <Disclosure bind:open summaryClass="agent-row-summary">
    {#snippet summary()}
      <StatusChip status={agent.status} />
      <span class="agent-row-type">{agent.agentType}</span>
      <span class="agent-row-desc" title={agent.description}>{agent.description}</span>
      <span class="agent-row-time">{elapsed(agent.startedAt, running ? null : agent.finishedAt, now)}</span>
    {/snippet}
    {#snippet actions()}
      <button type="button" class="agent-row-open" aria-label={`Open the ${agent.agentType} transcript`} onclick={() => onOpen(agent.agentId)}>
        <Icon icon={ArrowRight} size={12} />
      </button>
    {/snippet}
    <div class="agent-row-detail">
      <p class="agent-row-meta">
        <span>{shortModel(agent.model)}</span>
        {#if tokens > 0}<span>{fmtTokens(tokens)} tokens</span>{/if}
        {#if agent.costUsd > 0}<span>{fmtCost(agent.costUsd)}</span>{/if}
        {#if agent.toolCalls > 0}<span>{plural(agent.toolCalls, "tool call")}</span>{/if}
        {#if agent.background}<span class="agent-pill">background</span>{/if}
        {#if agent.worktree}<span class={`agent-pill worktree-${agent.worktree.state}`}>worktree · {agent.worktree.state}</span>{/if}
      </p>
      {#if agent.error}
        <p class="agent-row-note is-error">{agent.error}</p>
      {:else if agent.resultPreview}
        <p class="agent-row-note">{agent.resultPreview}</p>
      {/if}
    </div>
  </Disclosure>
  {#if running && activity}<p class="agent-row-now">{activity}</p>{/if}
</div>
