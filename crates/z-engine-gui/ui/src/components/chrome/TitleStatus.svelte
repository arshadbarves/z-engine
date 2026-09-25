<script lang="ts">
  import { untrack } from "svelte";
  import { hasUnseen, recentActivity } from "$lib/domain/activity";
  import { companionPose } from "$lib/domain/companion";
  import { contextMeter } from "$lib/domain/contextMeter";
  import { islandLine } from "$lib/domain/island";
  import { freshFinish, liveStatus, waitingChats } from "$lib/domain/liveStatus";
  import { chatTitle, sessions, toastStore } from "$lib/runtime";
  import { companion } from "$lib/stores/companion.svelte";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { userSignals } from "$lib/stores/userSignals.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { ticker } from "$lib/ui/ticker.svelte";
  import ContextBubble from "./ContextBubble.svelte";
  import Island from "./Island.svelte";
  import WaitingBubble from "./WaitingBubble.svelte";

  /**
   * The title bar's middle, a split island: other chats that need you on the
   * left, the companion with one line of status in the middle, and how full
   * this chat's context is on the right.
   */
  type Props = { chat: string | null };
  let { chat }: Props = $props();

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
  const line = $derived(islandLine(status, chat));
  const pose = $derived(companionPose(status, userSignals, level, clock.now));
  const meter = $derived(
    view?.info
      ? contextMeter({ contextTokens: view.contextTokens, contextLimit: view.contextLimit, breakdown: view.contextBreakdown })
      : null,
  );
  const activity = $derived(recentActivity(view));
  const unseen = $derived(!companion.open && hasUnseen(activity, companion.seenAt(sessions.activeId)));
  const progress = $derived(status.progress ? status.progress.done / status.progress.total : null);
  const label = $derived([line.text, status.detail, line.metric].filter(Boolean).join(", ") || "Z Engine");
</script>

<div class={`title-status tone-${status.tone}`}>
  {#if status.waiting}
    <WaitingBubble waiting={status.waiting} more={status.moreWaiting} />
  {/if}
  <Island {status} {line} {pose} {progress} {notice} {view} {activity} {unseen} {label} title={chat} />
  {#if view && meter}
    <ContextBubble sessionId={view.sessionId} {meter} busy={view.status !== "idle"} />
  {/if}
  <span class="sr-only" aria-live="polite">
    {status.kind === "idle" ? "" : [status.text, status.detail].filter(Boolean).join(", ")}
  </span>
</div>
