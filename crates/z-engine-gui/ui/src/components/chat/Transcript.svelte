<script lang="ts">
  import type { SessionView } from "$lib/domain/sessionView";
  import { pairResults, toolUses, visibleText } from "$lib/domain/timeline/blocks";
  import { linkAgentCalls, liveStreams } from "$lib/domain/timeline/toolState";
  import { buildTimeline, type TimelineTurn } from "$lib/domain/timeline/turns";
  import type { Message } from "$lib/protocol/Message";
  import type { RewindScope } from "$lib/protocol/RewindScope";
  import { cancelTurn, rewind, sessions } from "$lib/runtime";
  import { composer } from "$lib/stores/composer.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import HomeScreen from "../home/HomeScreen.svelte";
  import PendingInteractions from "../planning/PendingInteractions.svelte";
  import ChatTimeline from "./ChatTimeline.svelte";
  import TrustBanner from "./TrustBanner.svelte";
  import TurnView from "./TurnView.svelte";


  const NO_LIST: never[] = [];
  const NO_RECORD: Record<string, never> = {};

  const view: SessionView | null = $derived(sessions.active);
  const messages = $derived(view?.messages ?? NO_LIST);
  const turns = $derived(view?.turns ?? NO_LIST);
  const turnStarts = $derived(view?.turnStarts ?? NO_RECORD);
  const steeringIds = $derived(view?.steeringIds ?? NO_RECORD);
  const rawOutputs = $derived(view?.outputs ?? NO_LIST);
  const outputs = $derived(rawOutputs.filter((o) => o.name !== "shell"));
  const errors = $derived(view?.errors ?? NO_LIST);
  const compactions = $derived(view?.compactions ?? NO_LIST);
  const activeMessageId = $derived(view?.activeTurn?.messageId ?? null);
  const status = $derived(view?.status ?? "idle");
  const busy = $derived(status !== "idle");
  const streaming = $derived(view?.streaming ?? NO_RECORD);
  const tools = $derived(view?.tools ?? NO_RECORD);
  const agents = $derived(view?.agents ?? NO_RECORD);
  const checks = $derived(view?.checks ?? NO_LIST);
  const checkpoints = $derived(view?.checkpoints ?? NO_LIST);
  const projectRoot = $derived(view?.info?.projectRoot ?? null);

  const timeline: TimelineTurn[] = $derived(
    buildTimeline({ messages, turns, turnStarts, steeringIds, activeMessageId, busy, outputs, errors, compactions }),
  );
  const results = $derived(pairResults(messages));
  const agentLinks = $derived(linkAgentCalls(messages.flatMap(toolUses), Object.values(agents)));
  const live = $derived(liveStreams(streaming));
  const checkpointIds = $derived(new Set(checkpoints.map((c) => c.messageId)));
  const prompts = $derived(
    timeline.flatMap((t) => (t.user ? [{ id: t.user.id, text: visibleText(t.user) }] : [])),
  );
  const empty = $derived(timeline.length === 0 && live.length === 0);
  const liveTurn: TimelineTurn = { key: "live", user: null, items: [], record: null, active: true };

  async function onRewind(message: Message, scope: RewindScope) {
    if (busy) await cancelTurn();
    if ((await rewind(message.id, scope)) && scope !== "code") composer.setDraft(visibleText(message));
  }

  function openAgent(agentId: string) {
    ui.openWork("agents", agentId);
  }
</script>

<div class="transcript-stage">
  <div class="transcript-inner">
    {#if empty && !sessions.hydrating}
      <HomeScreen />
    {/if}

    {#each timeline as turn, i (turn.key)}
      <TurnView
        {turn}
        {results}
        {tools}
        {agents}
        {agentLinks}
        {checks}
        {projectRoot}
        live={i === timeline.length - 1 ? live : undefined}
        canRestoreCode={(id) => checkpointIds.has(id)}
        {onRewind}
        onOpenAgent={openAgent}
      />
    {/each}
    {#if timeline.length === 0 && live.length > 0}
      <TurnView
        turn={liveTurn}
        {results}
        {tools}
        {agents}
        {agentLinks}
        {checks}
        {projectRoot}
        {live}
        onOpenAgent={openAgent}
      />
    {/if}

    {#if view}
      {#if view.trustRequest}<TrustBanner request={view.trustRequest} />{/if}
      <PendingInteractions {view} />
    {/if}
  </div>
  <ChatTimeline {prompts} />
</div>
