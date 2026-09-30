<script lang="ts">
  import { isAgentDone } from "$lib/domain/agentTree";
  import { elapsed, fmtDuration } from "$lib/domain/format";
  import type { ToolCallState } from "$lib/domain/timeline/toolState";
  import { bool, firstLine, str } from "$lib/domain/tools/toolInput";
  import { totalTokens } from "$lib/domain/usage";
  import type { AgentInfo } from "$lib/protocol/AgentInfo";
  import type { AgentStatus } from "$lib/protocol/AgentStatus";
  import { ticker } from "$lib/ui/ticker.svelte";
  import AgentSummary from "../../agents/AgentSummary.svelte";

  /** A subagent call in the transcript, as the same one-line summary the agents panel uses. */
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
  const preview = $derived(
    agent?.error ?? agent?.resultPreview ?? (running ? firstLine(str(call.input, "prompt")) : firstLine(call.output)),
  );
</script>

<AgentSummary
  density="line"
  type={agent?.agentType || str(call.input, "subagent_type") || "general"}
  description={agent?.description || str(call.input, "description")}
  {status}
  time={agent ? elapsed(agent.startedAt, agent.finishedAt, clock.now) : fmtDuration(call.durationMs) || null}
  background={agent?.background ?? bool(call.input, "run_in_background")}
  model={agent?.model ?? null}
  tokens={agent ? totalTokens(agent.usage) : 0}
  costUsd={agent?.costUsd ?? 0}
  toolCalls={agent?.toolCalls ?? 0}
  worktree={agent?.worktree ?? null}
  note={preview || null}
  noteIsError={Boolean(agent?.error)}
  onOpen={agent && onOpen ? () => onOpen(agent.agentId) : undefined}
/>
