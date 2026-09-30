<script lang="ts">
  import { isAgentDone, type AgentNode } from "$lib/domain/agentTree";
  import { elapsed } from "$lib/domain/format";
  import { totalTokens } from "$lib/domain/usage";
  import AgentSummary from "./AgentSummary.svelte";

  /** One subagent in the panel's tree; while it runs, what it is doing now shows under it. */
  type Props = { node: AgentNode; now: number; activity: string | null; onOpen: (agentId: string) => void };
  let { node, now, activity, onOpen }: Props = $props();

  const agent = $derived(node.info);
  const running = $derived(!isAgentDone(agent.status));
</script>

<AgentSummary
  density="row"
  depth={node.depth}
  type={agent.agentType}
  description={agent.description}
  status={agent.status}
  time={elapsed(agent.startedAt, running ? null : agent.finishedAt, now)}
  background={agent.background}
  model={agent.model}
  tokens={totalTokens(agent.usage)}
  costUsd={agent.costUsd}
  toolCalls={agent.toolCalls}
  worktree={agent.worktree}
  note={agent.error ?? agent.resultPreview}
  noteIsError={Boolean(agent.error)}
  activity={running ? activity : null}
  onOpen={() => onOpen(agent.agentId)}
/>
