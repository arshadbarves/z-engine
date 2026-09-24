<script lang="ts">
  import type { StreamingMessage, ToolCallView } from "$lib/domain/sessionView";
  import type { ToolResultInfo } from "$lib/domain/timeline/blocks";
  import type { TimelineTurn } from "$lib/domain/timeline/turns";
  import type { AgentInfo } from "$lib/protocol/AgentInfo";
  import type { CheckRecord } from "$lib/protocol/CheckRecord";
  import type { Message } from "$lib/protocol/Message";
  import type { RewindScope } from "$lib/protocol/RewindScope";
  import AssistantText from "./AssistantText.svelte";
  import LocalCards from "./LocalCards.svelte";
  import ThinkingDisclosure from "./ThinkingDisclosure.svelte";
  import TurnFooter from "./TurnFooter.svelte";
  import UserCard from "./UserCard.svelte";
  import ToolCall from "./tools/ToolCall.svelte";

  type Props = {
    turn: TimelineTurn;
    results: Record<string, ToolResultInfo>;
    tools: Record<string, ToolCallView>;
    agents: Record<string, AgentInfo>;
    agentLinks: Record<string, string>;
    checks: CheckRecord[];
    projectRoot: string | null;
    live?: StreamingMessage[];
    canRestoreCode?: (messageId: string) => boolean;
    onRewind?: (message: Message, scope: RewindScope) => void;
    onOpenAgent: (agentId: string) => void;
  };

  let {
    turn,
    results,
    tools,
    agents,
    agentLinks,
    checks,
    projectRoot,
    live = [],
    canRestoreCode,
    onRewind,
    onOpenAgent,
  }: Props = $props();
</script>

<section class="turn" data-turn={turn.key}>
  {#if turn.user}
    <UserCard message={turn.user} canRestoreCode={canRestoreCode?.(turn.user.id) ?? false} {onRewind} />
  {/if}

  {#if turn.items.length > 0 || live.length > 0}
    <div class="assistant-turn">
      {#each turn.items as item (item.key)}
        {#if item.kind === "text"}
          <AssistantText text={item.text} />
        {:else if item.kind === "thinking"}
          <ThinkingDisclosure text={item.text} redacted={item.redacted} />
        {:else if item.kind === "tool"}
          {@const agentId = agentLinks[item.use.callId]}
          <ToolCall
            toolUse={item.use}
            result={results[item.use.callId]}
            live={tools[item.use.callId]}
            busy={turn.active}
            {projectRoot}
            agent={agentId ? (agents[agentId] ?? null) : null}
            {onOpenAgent}
          />
        {:else}
          <LocalCards {item} />
        {/if}
      {/each}
      {#each live as stream (stream.messageId)}
        {#if stream.thinking}
          <ThinkingDisclosure text={stream.thinking} streaming={!stream.text} />
        {/if}
        {#if stream.text}
          <AssistantText text={stream.text} streaming />
        {/if}
      {/each}
    </div>
  {/if}

  {#if turn.record}
    <TurnFooter record={turn.record} {checks} />
  {/if}
</section>
