<script lang="ts">
  import { groupTurns } from "$lib/activity";
  import { hydrateStore } from "$lib/runtime";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import type { Msg } from "$lib/types";
  import HomeScreen from "../home/HomeScreen.svelte";
  import ChatTimeline from "./ChatTimeline.svelte";
  import ConversationTurn from "./primitives/ConversationTurn.svelte";

  type Props = {
    messages: Msg[];
    busy: boolean;
    projectName: string | null;
    onApprove: (m: Msg, decision: "once" | "session" | "persist") => void;
    onDeny: (m: Msg) => void;
  };

  let { messages, busy, projectName, onApprove, onDeny }: Props = $props();

  const hydrating = bindStore(hydrateStore);
  const turns = $derived(groupTurns(messages));
  const streaming = $derived(
    messages.some(
      (m) => m.streaming && (m.kind === "assistant" || m.kind === "thinking" || m.kind === "tool"),
    ),
  );
  const showWorking = $derived(busy && !streaming && !hydrating.current);

  let secs = $state(0);
  $effect(() => {
    if (!showWorking) {
      secs = 0;
      return;
    }
    const t = setInterval(() => {
      secs += 1;
    }, 1000);
    return () => clearInterval(t);
  });
</script>

<div class="transcript-stage">
  <div class="transcript-inner">
    {#if messages.length === 0 && !hydrating.current}
      <HomeScreen {projectName} />
    {/if}

    {#each turns as turn (turn.type === "work" ? turn.items[0].id : turn.msg.id)}
      <ConversationTurn {turn} {onApprove} {onDeny} />
    {/each}

    {#if showWorking}
      <div class="working-dock" aria-live="polite">
        <div class="msg-working-pill">
          <span class="working-pulse-dot" aria-hidden="true"></span>
          <span class="working-text">Thinking…</span>
          <span class="working-sec">{secs}s</span>
          <span class="working-hint"><kbd>Esc</kbd> aborts</span>
        </div>

      </div>
    {/if}
  </div>
  <ChatTimeline {messages} />
</div>
