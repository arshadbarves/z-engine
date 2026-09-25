<script lang="ts">
  import { turnFiles } from "$lib/domain/receipt";
  import type { StreamingMessage, ToolCallView } from "$lib/domain/sessionView";
  import type { ToolResultInfo } from "$lib/domain/timeline/blocks";
  import { groupTurnItems } from "$lib/domain/timeline/groups";
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
  import ToolGroup from "./tools/ToolGroup.svelte";

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

  const blocks = $derived(groupTurnItems(turn.items));
  const files = $derived(
    turn.record
      ? turnFiles(
          turn.items.flatMap((item) =>
            item.kind === "tool"
              ? [{ name: item.use.name, input: item.use.input, ok: results[item.use.callId]?.isError !== true }]
              : [],
          ),
          projectRoot,
        )
      : [],
  );
</script>

<section class="turn" data-turn={turn.key}>
  {#if turn.user}
    <UserCard message={turn.user} canRestoreCode={canRestoreCode?.(turn.user.id) ?? false} {onRewind} />
  {/if}

  {#if turn.items.length > 0 || live.length > 0}
    <div class="assistant-turn">
      {#each blocks as block (block.key)}
        {#if block.kind === "run"}
          <ToolGroup
            uses={block.uses}
            {results}
            {tools}
            busy={turn.active}
            {projectRoot}
            {agents}
            {agentLinks}
            {onOpenAgent}
          />
        {:else if block.item.kind === "text"}
          <AssistantText text={block.item.text} />
        {:else if block.item.kind === "thinking"}
          <ThinkingDisclosure text={block.item.text} redacted={block.item.redacted} />
        {:else if block.item.kind === "tool"}
          {@const use = block.item.use}
          {@const agentId = agentLinks[use.callId]}
          <ToolCall
            toolUse={use}
            result={results[use.callId]}
            live={tools[use.callId]}
            busy={turn.active}
            {projectRoot}
            agent={agentId ? (agents[agentId] ?? null) : null}
            {onOpenAgent}
          />
        {:else}
          <LocalCards item={block.item} />
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
    <TurnFooter record={turn.record} {checks} {files} />
  {/if}
</section>
