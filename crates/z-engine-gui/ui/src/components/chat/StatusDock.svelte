<script lang="ts">
  import type { RetryingView } from "$lib/domain/sessionView";
  import type { SessionStatus } from "$lib/protocol/SessionStatus";
  import { modLabel } from "$lib/platform";
  import Icon, { Clock } from "$lib/ui/icons";
  import { ticker } from "$lib/ui/ticker.svelte";

  /** Working / waiting pill and the provider back-off banner under the transcript. */
  type Props = {
    status: SessionStatus;
    startedAt: number | null;
    streaming: boolean;
    retrying: RetryingView | null;
  };
  let { status, startedAt, streaming, retrying }: Props = $props();

  const clock = ticker(() => status !== "idle" || retrying !== null, 500);
  const secs = $derived(startedAt ? Math.max(0, Math.floor((clock.now - startedAt) / 1000)) : null);
  const retryIn = $derived(retrying ? Math.max(0, Math.ceil((retrying.at + retrying.delayMs - clock.now) / 1000)) : 0);
</script>

{#if retrying}
  <div class="retry-banner" role="status">
    <Icon icon={Clock} size={12} />
    <span class="retry-title">Provider busy · retry {retrying.attempt} {retryIn > 0 ? `in ${retryIn}s` : "now"}</span>
    <span class="retry-reason">{retrying.reason}</span>
  </div>
{/if}

{#if status === "waiting"}
  <div class="working-dock" aria-live="polite">
    <div class="msg-working-pill waiting">
      <span class="working-pulse-dot waiting" aria-hidden="true"></span>
      <span class="working-text">Waiting for you</span>
      <span class="working-hint">answer the card above to continue</span>
    </div>
  </div>
{:else if status === "busy" && !streaming && !retrying}
  <div class="working-dock" aria-live="polite">
    <div class="msg-working-pill">
      <span class="working-pulse-dot" aria-hidden="true"></span>
      <span class="working-text">Working…</span>
      {#if secs !== null}<span class="working-sec">{secs}s</span>{/if}
      <span class="working-hint"><kbd>Esc</kbd> cancel · <kbd>{modLabel()}↵</kbd> interrupt</span>
    </div>
  </div>
{/if}
