<script lang="ts">
  import { untrack } from "svelte";
  import { hasUnseen, recentActivity } from "$lib/domain/activity";
  import { workCounts } from "$lib/domain/agentTree";
  import { companionPose } from "$lib/domain/companion";
  import { contextMeter } from "$lib/domain/contextMeter";
  import { freshFinish, liveStatus, waitingChats } from "$lib/domain/liveStatus";
  import { sessionLabel, viewTitle } from "$lib/domain/sessionList";
  import { sessionList, sessions, toastStore } from "$lib/runtime";
  import { openChatById } from "$lib/stores/app-actions";
  import { companion } from "$lib/stores/companion.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { userSignals } from "$lib/stores/userSignals.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { Popover } from "$lib/ui";
  import { blurFade, prefersReducedMotion } from "$lib/ui/motion";
  import { ticker } from "$lib/ui/ticker.svelte";
  import Companion from "./Companion.svelte";
  import NowCard from "./NowCard.svelte";

  /** The title bar's middle: the companion, which chat this is, and what the agent is doing, in full. */
  type Props = { workspace: string | null; chat: string | null };
  let { workspace, chat }: Props = $props();

  const RECAP_MS = 8000;
  const toasts = bindStore(toastStore);
  const view = $derived(sessions.active);
  const level = $derived(settingsStore.settings?.ui.companion ?? "lively");
  const live = $derived(view !== null && (view.status !== "idle" || view.retrying !== null));
  const clock = ticker(() => live || level === "lively", 500);

  let flashTurnId = $state<string | null>(null);
  const finishKey = $derived(view?.status === "idle" ? (view.turns[view.turns.length - 1]?.turnId ?? null) : null);
  $effect(() => {
    if (!finishKey) return;
    const fresh = untrack(() => freshFinish(view, Date.now()));
    if (!fresh) return;
    flashTurnId = fresh.turnId;
    const timer = window.setTimeout(() => (flashTurnId = null), fresh.remainingMs);
    return () => {
      window.clearTimeout(timer);
      flashTurnId = null;
    };
  });

  $effect(() => {
    if (userSignals.awaySince === null) return;
    const timer = window.setTimeout(() => userSignals.dismissRecap(), RECAP_MS);
    return () => window.clearTimeout(timer);
  });

  $effect(() => {
    void sessions.activeId;
    untrack(() => (companion.open = false));
  });

  const waiting = $derived(waitingChats(sessions.activity, sessions.activeId, chatTitle));
  const notice = $derived(toasts.current[toasts.current.length - 1] ?? null);
  const status = $derived(
    liveStatus({ view, waiting, notice, flashTurnId, awaySince: userSignals.awaySince, now: clock.now }),
  );
  const pose = $derived(companionPose(status, userSignals, level, clock.now));
  const meter = $derived(
    view
      ? contextMeter({ contextTokens: view.contextTokens, contextLimit: view.contextLimit, breakdown: view.contextBreakdown })
      : null,
  );
  const activity = $derived(recentActivity(view));
  const unseen = $derived(!companion.open && hasUnseen(activity, companion.seenAt(sessions.activeId)));
  const helpers = $derived.by(() => {
    if (!view) return "";
    const c = workCounts(view.agents, view.jobs);
    return [
      c.runningAgents && `${c.runningAgents} agent${c.runningAgents === 1 ? "" : "s"}`,
      c.runningJobs && `${c.runningJobs} job${c.runningJobs === 1 ? "" : "s"}`,
      c.pendingWorktrees && `${c.pendingWorktrees} to apply`,
    ]
      .filter(Boolean)
      .join(" · ");
  });
  const progress = $derived(status.progress ? status.progress.done / status.progress.total : null);
  const label = $derived(
    [workspace, chat, status.text, status.detail, status.elapsed, status.cost].filter(Boolean).join(", ") ||
      "Z Engine",
  );

  function chatTitle(sessionId: string): string {
    const summary = sessionList.summaries.find((s) => s.sessionId === sessionId);
    return sessionLabel(viewTitle(sessions.view(sessionId) ?? undefined, summary));
  }

  function revealPending() {
    const card = document.querySelector<HTMLElement>(".transcript .msg.approval, .transcript .msg.interaction-card");
    card?.scrollIntoView({ block: "center", behavior: prefersReducedMotion() ? "auto" : "smooth" });
    card?.focus({ preventScroll: true });
  }

  function onOpenChange(open: boolean) {
    if (!open) return;
    companion.markSeen(sessions.activeId);
    userSignals.dismissRecap();
  }
</script>

<div class={`title-status tone-${status.tone}`}>
  <Popover.Root bind:open={companion.open} {onOpenChange}>
    <Popover.Trigger class="status-main" aria-label={label} disabled={!view}>
      <span
        class="status-orb"
        role="presentation"
        onpointerenter={() => (userSignals.hovering = true)}
        onpointerleave={() => (userSignals.hovering = false)}
      >
        {#if pose}
          <Companion {pose} {progress} helpers={status.helpers.running} />
        {:else}
          <span class={`status-dot tone-${status.tone}`}></span>
        {/if}
      </span>
      {#if workspace}<span class="status-ws">{workspace}</span>{/if}
      {#if workspace && chat}<span class="status-sep status-ws-sep">/</span>{/if}
      {#if chat}<span class="status-chat">{chat}</span>{/if}
      {#if status.text}
        <span class="status-sep">·</span>
        {#key status.text}
          <span class="status-step" in:blurFade={{ duration: 220, blur: 5, scale: 1 }}>{status.text}</span>
        {/key}
      {/if}
      {#if status.detail}<span class="status-detail">{status.detail}</span>{/if}
      {#if status.progress}<span class="status-metric">{status.progress.done}/{status.progress.total}</span>{/if}
      {#if status.elapsed}<span class="status-metric">{status.elapsed}</span>{/if}
      {#if status.cost}<span class="status-metric">{status.cost}</span>{/if}
      {#if unseen}<span class="status-unseen" aria-hidden="true"></span>{/if}
    </Popover.Trigger>
    {#if view && meter}
      <Popover.Portal>
        <Popover.Content class="now-card" side="bottom" align="center" sideOffset={8} collisionPadding={12}>
          <NowCard {view} {status} title={chat} {meter} {activity} onClose={() => (companion.open = false)} />
        </Popover.Content>
      </Popover.Portal>
    {/if}
  </Popover.Root>
  {#if status.kind === "attention"}
    <button type="button" class="status-action" onclick={revealPending}>Review</button>
  {/if}
  {#if helpers}
    <button type="button" class="status-pill" onclick={() => ui.openWork("agents")}>{helpers}</button>
  {/if}
  {#if status.waiting}
    {@const other = status.waiting}
    <button type="button" class="status-chip" onclick={() => void openChatById(other.sessionId)}>
      {other.title} needs you{status.moreWaiting ? ` +${status.moreWaiting}` : ""}
    </button>
  {/if}
  <span class="sr-only" aria-live="polite">
    {status.kind === "idle" ? "" : [status.text, status.detail].filter(Boolean).join(", ")}
  </span>
</div>
