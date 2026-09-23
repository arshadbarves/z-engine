<script lang="ts">
  import { isAgentDone } from "$lib/domain/agentTree";
  import { elapsed, fmtDuration } from "$lib/domain/format";
  import type { ToolCallState } from "$lib/domain/timeline/toolState";
  import { bool, firstLine, str } from "$lib/domain/tools/toolInput";
  import { totalTokens } from "$lib/domain/usage";
  import type { AgentInfo } from "$lib/protocol/AgentInfo";
  import type { AgentStatus } from "$lib/protocol/AgentStatus";
  import Icon, { ArrowRight, Bot } from "$lib/ui/icons";
  import { ticker } from "$lib/ui/ticker.svelte";
  import { fmtCost, fmtTokens, shortModel } from "$lib/util";
  import StatusChip from "../../agents/StatusChip.svelte";

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
</script>

<div class={`agent-call-card status-${status}`}>
  <div class="agent-call-head">
    <span class="agent-call-icon" aria-hidden="true"><Icon icon={Bot} size={13} /></span>
    <span class="agent-call-type">{type}</span>
    <span class="agent-call-desc" title={description}>{description}</span>
    <StatusChip {status} />
  </div>
  <div class="agent-call-meta">
    {#if agent?.model}<span>{shortModel(agent.model)}</span>{/if}
    {#if tokens > 0}<span>{fmtTokens(tokens)} tokens</span>{/if}
    {#if agent && agent.costUsd > 0}<span>{fmtCost(agent.costUsd)}</span>{/if}
    {#if agent && agent.toolCalls > 0}<span>{agent.toolCalls} tool calls</span>{/if}
    {#if duration}<span>{duration}</span>{/if}
    {#if background}<span class="agent-pill">background</span>{/if}
    {#if agent?.worktree}
      <span class={`agent-pill worktree-${agent.worktree.state}`}>worktree · {agent.worktree.state}</span>
    {/if}
  </div>
  {#if preview}
    <p class={`agent-call-preview${agent?.error ? " is-error" : ""}`}>{preview}</p>
  {/if}
  {#if agent}
    <button type="button" class="agent-call-open" onclick={() => onOpen?.(agent.agentId)}>
      <span>Open transcript</span>
      <Icon icon={ArrowRight} size={11} />
    </button>
  {/if}
</div>
