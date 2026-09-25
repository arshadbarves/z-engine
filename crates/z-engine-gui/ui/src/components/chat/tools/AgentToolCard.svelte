<script lang="ts">
  import { isAgentDone } from "$lib/domain/agentTree";
  import { elapsed, fmtDuration } from "$lib/domain/format";
  import type { ToolCallState } from "$lib/domain/timeline/toolState";
  import { bool, firstLine, str } from "$lib/domain/tools/toolInput";
  import { totalTokens } from "$lib/domain/usage";
  import type { AgentInfo } from "$lib/protocol/AgentInfo";
  import type { AgentStatus } from "$lib/protocol/AgentStatus";
  import { Disclosure } from "$lib/ui";
  import Icon, { ArrowRight, Bot } from "$lib/ui/icons";
  import { ticker } from "$lib/ui/ticker.svelte";
  import { fmtCost, fmtTokens, shortModel } from "$lib/util";
  import StatusChip from "../../agents/StatusChip.svelte";

  /** A subagent as one line (type, task, status, time, Open); model, usage and result fold away. */
  type Props = { call: ToolCallState; agent: AgentInfo | null; onOpen?: (agentId: string) => void };
  let { call, agent, onOpen }: Props = $props();

  const FROM_TOOL: Record<ToolCallState["status"], AgentStatus> = {
    running: "running",
    ok: "completed",
    error: "failed",
    denied: "cancelled",
    cancelled: "cancelled",
  };

  const status = $derived(agent?.status ?? FROM_TOOL[call.status]);
  const running = $derived(!isAgentDone(status));
  const clock = ticker(() => running);
  const type = $derived(agent?.agentType || str(call.input, "subagent_type") || "general");
  const description = $derived(agent?.description || str(call.input, "description"));
  const background = $derived(agent?.background ?? bool(call.input, "run_in_background"));
  const duration = $derived(
    agent ? elapsed(agent.startedAt, agent.finishedAt, clock.now) : fmtDuration(call.durationMs),
  );
  const tokens = $derived(agent ? totalTokens(agent.usage) : 0);
  const preview = $derived(
    agent?.error ?? agent?.resultPreview ?? (running ? firstLine(str(call.input, "prompt")) : firstLine(call.output)),
  );
  const hasDetail = $derived(Boolean(preview || agent?.model || tokens > 0 || agent?.worktree));
  let open = $state(false);
</script>

<div class={`agent-line status-${status}`}>
  <Disclosure bind:open disabled={!hasDetail} chevron={hasDetail} summaryClass="agent-line-summary">
    {#snippet summary()}
      <span class="agent-line-icon" aria-hidden="true"><Icon icon={Bot} size={13} /></span>
      <span class="agent-line-type">{type}</span>
      <span class="agent-line-desc" title={description}>{description}</span>
      {#if background}<span class="agent-pill">background</span>{/if}
      <StatusChip {status} />
      {#if duration}<span class="agent-line-time">{duration}</span>{/if}
    {/snippet}
    {#snippet actions()}
      {#if agent}
        <button type="button" class="agent-line-open" onclick={() => onOpen?.(agent.agentId)}>
          <span>Open</span>
          <Icon icon={ArrowRight} size={11} />
        </button>
      {/if}
    {/snippet}
    <div class="agent-line-detail">
      <p class="agent-line-meta">
        {#if agent?.model}<span>{shortModel(agent.model)}</span>{/if}
        {#if tokens > 0}<span>{fmtTokens(tokens)} tokens</span>{/if}
        {#if agent && agent.costUsd > 0}<span>{fmtCost(agent.costUsd)}</span>{/if}
        {#if agent && agent.toolCalls > 0}<span>{agent.toolCalls} tool calls</span>{/if}
        {#if agent?.worktree}<span class={`agent-pill worktree-${agent.worktree.state}`}>worktree · {agent.worktree.state}</span>{/if}
      </p>
      {#if preview}<p class={`agent-line-preview${agent?.error ? " is-error" : ""}`}>{preview}</p>{/if}
    </div>
  </Disclosure>
</div>
