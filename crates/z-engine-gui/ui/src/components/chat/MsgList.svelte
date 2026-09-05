<script lang="ts">
  import { groupTurns } from "$lib/activity";
  import { hydrateStore } from "$lib/runtime";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import type { Msg } from "$lib/types";
  import HomeScreen from "../home/HomeScreen.svelte";
  import ActivityStrip from "./ActivityStrip.svelte";
  import ApprovalCard from "./ApprovalCard.svelte";
  import ChatTimeline from "./ChatTimeline.svelte";
  import Markdown from "./Markdown.svelte";
  import UserCard from "./UserCard.svelte";

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

    {#each turns as b (b.type === "work" ? b.items[0].id : b.msg.id)}
      {#if b.type === "user"}
        <UserCard m={b.msg} />
      {:else if b.type === "approval"}
        <ApprovalCard
          m={b.msg}
          onApprove={(d) => onApprove(b.msg, d)}
          onDeny={() => onDeny(b.msg)}
        />
      {:else if b.type === "assistant"}
        <div class="assistant-turn">
          {#if b.workItems && b.workItems.length > 0}
            <ActivityStrip items={b.workItems} />
          {/if}
          {#if b.msg.text.trim().length > 0 || b.msg.streaming}
            <div class={`msg assistant${b.msg.streaming ? " streaming" : ""}`}>
              <Markdown text={b.msg.text} />
            </div>
          {/if}
        </div>
      {:else if b.type === "work"}
        <div class="assistant-turn">
          <ActivityStrip items={b.items} />
        </div>
      {:else if b.type === "error"}
        <div class="msg error">{b.msg.text}</div>
      {/if}
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
