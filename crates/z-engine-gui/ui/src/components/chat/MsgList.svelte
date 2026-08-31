<script lang="ts">
  import { groupTranscript } from "$lib/activity";
  import { HERO_STARTERS } from "$lib/constants";
  import { draftStore, hydrateStore } from "$lib/runtime";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import type { Msg } from "$lib/types";
  import Icon, { FolderGit2, Search, Sparkles, Workflow, Wrench } from "$lib/ui/icons";
  import LogoMark from "../chrome/LogoMark.svelte";
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
  const blocks = $derived(groupTranscript(messages));
  const streaming = $derived(
    messages.some(
      (m) => m.streaming && (m.kind === "assistant" || m.kind === "thinking" || m.kind === "tool"),
    ),
  );
  const showWorking = $derived(busy && !streaming && !hydrating.current);

  const starterIcon = {
    Search,
    Sparkles,
    Wrench,
    Workflow,
  } as const;

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
  <ChatTimeline {messages} />
  <div class="transcript-inner">
    {#if messages.length === 0 && !hydrating.current}
      <div class="start-hub">
        <div class="start-hub-brand">
          <div class="start-hub-icon-halo">
            <LogoMark size={28} />
          </div>
          <h1 class="start-hub-title">What should we build today?</h1>
          {#if projectName}
            <div class="start-hub-ws-pill">
              <Icon icon={FolderGit2} size={12} strokeWidth={1.8} />
              <span>{projectName}</span>
            </div>
          {:else}
            <p class="start-hub-desc">
              Autonomous coding agent with full codebase awareness, tool execution, and live verification.
            </p>
          {/if}
        </div>
        <div class="start-hub-grid">
          {#each HERO_STARTERS as card, index}
            <button
              type="button"
              class="start-hub-card"
              style={`--card-index: ${index}`}
              onclick={() => draftStore.set(card.prompt)}
            >
              <div class="card-icon-box">
                <Icon
                  icon={starterIcon[card.iconName as keyof typeof starterIcon] ?? Sparkles}
                  size={14}
                  strokeWidth={1.8}
                />
              </div>
              <div class="card-text-col">
                <span class="card-title">{card.title}</span>
                <span class="card-desc">{card.desc}</span>
              </div>
            </button>
          {/each}
        </div>
        <div class="start-hub-hints">
          <span class="hint-pill"><kbd>@</kbd> Reference files</span>
          <span class="hint-pill"><kbd>/</kbd> Slash commands</span>
          <span class="hint-pill"><kbd>!</kbd> Bash mode</span>
        </div>
      </div>
    {/if}

    {#each blocks as b (b.type === "work" ? b.items[0].id : b.msg.id)}
      {#if b.type === "work"}
        <ActivityStrip items={b.items} />
      {:else if b.msg.kind === "approval"}
        <ApprovalCard
          m={b.msg}
          onApprove={(d) => onApprove(b.msg, d)}
          onDeny={() => onDeny(b.msg)}
        />
      {:else if b.msg.kind === "user"}
        <UserCard m={b.msg} />
      {:else if b.msg.kind === "assistant"}
        <div class={`msg assistant${b.msg.streaming ? " streaming" : ""}`}>
          <Markdown text={b.msg.text} />
        </div>
      {:else if b.msg.kind === "error"}
        <div class="msg error">{b.msg.text}</div>
      {:else if b.msg.kind === "status"}
        <div class={`msg working${b.msg.ok === false ? " aborted" : " done"}`}>{b.msg.text}</div>
      {/if}
    {/each}

    {#if showWorking}
      <div class="msg working" aria-live="polite">
        <span class="working-glyph">✻</span> working…
        <span class="working-sec">{secs}s</span>
        <span class="working-hint">Esc aborts</span>
      </div>
    {/if}
  </div>
</div>
