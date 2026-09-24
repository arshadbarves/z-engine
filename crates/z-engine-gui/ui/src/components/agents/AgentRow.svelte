<script lang="ts">
  import { isAgentDone, type AgentNode } from "$lib/domain/agentTree";
  import { elapsed, plural } from "$lib/domain/format";
  import { totalTokens } from "$lib/domain/usage";
  import { applyAgentChanges, discardAgentChanges } from "$lib/runtime";
  import Icon, { ChevronRight, GitBranch, GitMerge } from "$lib/ui/icons";
  import { fmtCost, fmtTokens, shortModel } from "$lib/util";
  import StatusChip from "./StatusChip.svelte";

  type Props = { node: AgentNode; now: number; onOpen: (agentId: string) => void };
  let { node, now, onOpen }: Props = $props();

  const agent = $derived(node.info);
  const worktree = $derived(agent.worktree);
  const tokens = $derived(totalTokens(agent.usage));
  let deciding = $state(false);

  async function decide(apply: boolean) {
    deciding = true;
    await (apply ? applyAgentChanges(agent.agentId) : discardAgentChanges(agent.agentId));
    deciding = false;
  }
</script>

<div class={`agent-row status-${agent.status}`} style={`--depth: ${node.depth}`}>
  <button type="button" class="agent-row-main" onclick={() => onOpen(agent.agentId)} title="Open transcript">
    <StatusChip status={agent.status} />
    <span class="agent-row-type">{agent.agentType}</span>
    <span class="agent-row-desc">{agent.description}</span>
    <Icon icon={ChevronRight} size={11} class="agent-row-chevron" />
  </button>
  <div class="agent-row-meta">
    <span>{shortModel(agent.model)}</span>
    {#if tokens > 0}<span>{fmtTokens(tokens)} tokens</span>{/if}
    {#if agent.costUsd > 0}<span>{fmtCost(agent.costUsd)}</span>{/if}
    {#if agent.toolCalls > 0}<span>{plural(agent.toolCalls, "tool call")}</span>{/if}
    <span>{elapsed(agent.startedAt, isAgentDone(agent.status) ? agent.finishedAt : null, now)}</span>
    {#if agent.background}<span class="agent-pill">background</span>{/if}
    {#if agent.isolation === "worktree"}
      <span class={`agent-pill worktree-${worktree?.state ?? "pending"}`}>
        worktree{worktree ? ` · ${worktree.state}` : ""}
      </span>
    {/if}
  </div>

  {#if worktree && (worktree.state === "pending" || worktree.state === "conflicted")}
    <div class="agent-worktree">
      <span class="agent-worktree-info" title={worktree.path}>
        <Icon icon={GitBranch} size={11} />
        {worktree.branch} · {plural(worktree.filesChanged, "file")}{worktree.diffstat ? ` · ${worktree.diffstat}` : ""}
      </span>
      {#if worktree.state === "conflicted"}
        <span class="agent-worktree-note">Applying conflicted; the tree was left unchanged.</span>
      {/if}
      <div class="agent-worktree-actions">
        <button type="button" class="btn-primary" disabled={deciding} onclick={() => void decide(true)}>
          <Icon icon={GitMerge} size={11} />
          Apply
        </button>
        <button type="button" class="btn-ghost" disabled={deciding} onclick={() => void decide(false)}>
          Discard
        </button>
      </div>
    </div>
  {/if}

  {#if agent.error}
    <p class="agent-row-note is-error">{agent.error}</p>
  {:else if agent.resultPreview}
    <p class="agent-row-note">{agent.resultPreview}</p>
  {/if}
</div>
