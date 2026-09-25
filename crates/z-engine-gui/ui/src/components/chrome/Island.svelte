<script lang="ts">
  import { tick, untrack } from "svelte";
  import { Spring } from "svelte/motion";
  import type { ActivityItem } from "$lib/domain/activity";
  import type { CompanionPose } from "$lib/domain/companion";
  import { noticeOpensIsland, type IslandLine } from "$lib/domain/island";
  import type { LiveStatus } from "$lib/domain/liveStatus";
  import type { SessionView } from "$lib/domain/sessionView";
  import type { Toast } from "$lib/runtime/toasts";
  import { companion } from "$lib/stores/companion.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { userSignals } from "$lib/stores/userSignals.svelte";
  import { Popover } from "$lib/ui";
  import { SPRING, prefersReducedMotion } from "$lib/ui/motion";
  import Companion from "./Companion.svelte";
  import IslandSheet from "./IslandSheet.svelte";

  /**
   * The compact island: the companion, one line and at most one number. Its
   * width springs to fit and stops at the room available; whatever does not
   * fit opens the island downward instead of pushing the title bar around.
   */
  type Props = {
    status: LiveStatus;
    line: IslandLine;
    pose: CompanionPose | null;
    progress: number | null;
    notice: Toast | null;
    view: SessionView | null;
    activity: ActivityItem[];
    unseen: boolean;
    label: string;
    title: string | null;
  };
  let { status, line, pose, progress, notice, view, activity, unseen, label, title }: Props = $props();

  let pill: HTMLButtonElement | undefined = $state();
  let measure: HTMLSpanElement | undefined = $state();
  let textEl: HTMLSpanElement | undefined = $state();
  const width = new Spring(0, SPRING.smooth);
  let autoOpened = $state(false);
  let hovering = $state(false);
  let lastNotice: number | null = null;

  const attention = $derived(status.kind === "attention");

  $effect(() => {
    if (!measure) return;
    const el = measure;
    const observer = new ResizeObserver(() => {
      const next = Math.ceil(el.getBoundingClientRect().width);
      void width.set(next, { instant: prefersReducedMotion() || width.current === 0 });
    });
    observer.observe(el);
    return () => observer.disconnect();
  });

  $effect(() => {
    const current = notice;
    if (!current || current.id === lastNotice) return;
    lastNotice = current.id;
    requestAnimationFrame(() => {
      const truncated = !!textEl && textEl.scrollWidth > textEl.clientWidth + 1;
      if (companion.open || !noticeOpensIsland(current, truncated)) return;
      autoOpened = true;
      companion.open = true;
    });
  });

  $effect(() => {
    if (companion.open && autoOpened && notice === null && !hovering) close();
  });

  $effect(() => {
    if (!companion.open) return;
    const sessionId = view?.sessionId ?? null;
    untrack(() => {
      companion.markSeen(sessionId);
      userSignals.dismissRecap();
    });
  });

  function close() {
    companion.open = false;
  }

  async function revealPending() {
    if (ui.view !== "chat") {
      ui.view = "chat";
      await tick();
    }
    const card = document.querySelector<HTMLElement>("[data-pending-card]");
    card?.scrollIntoView({ block: "center", behavior: prefersReducedMotion() ? "auto" : "smooth" });
    card?.focus({ preventScroll: true });
  }

  function onPillClick() {
    if (attention) {
      void revealPending();
      return;
    }
    if (companion.open && autoOpened) {
      autoOpened = false;
      return;
    }
    autoOpened = false;
    companion.open = !companion.open;
  }
</script>

<Popover.Root bind:open={companion.open}>
  <button
    bind:this={pill}
    type="button"
    class={`island tone-${status.tone}${line.text ? " has-text" : ""}${companion.open ? " is-open" : ""}`}
    style:width={width.current ? `${width.current}px` : undefined}
    aria-label={attention ? `${label}. Show what needs you` : label}
    aria-haspopup="dialog"
    aria-expanded={companion.open}
    onclick={onPillClick}
  >
    <span
      class="island-orb"
      role="presentation"
      data-splash-target
      onpointerenter={() => (userSignals.hovering = true)}
      onpointerleave={() => (userSignals.hovering = false)}
    >
      {#if pose}
        <Companion {pose} {progress} helpers={status.helpers.running} />
      {:else}
        <span class={`island-dot tone-${status.tone}`}></span>
      {/if}
    </span>
    {#if line.text}<span class="island-text" bind:this={textEl}>{line.text}</span>{/if}
    {#if line.metric}<span class="island-metric">{line.metric}</span>{/if}
    {#if unseen}<span class="island-unseen" aria-hidden="true"></span>{/if}
  </button>
  <span class={`island island-measure${line.text ? " has-text" : ""}`} aria-hidden="true" bind:this={measure}>
    <span class="island-orb-space"></span>
    {#if line.text}<span class="island-text">{line.text}</span>{/if}
    {#if line.metric}<span class="island-metric">{line.metric}</span>{/if}
    {#if unseen}<span class="island-unseen"></span>{/if}
  </span>
  <Popover.Portal>
    <Popover.Content
      class="island-sheet"
      customAnchor={pill}
      side="bottom"
      align="center"
      sideOffset={6}
      collisionPadding={12}
      trapFocus={!autoOpened}
      onOpenAutoFocus={(e) => {
        if (autoOpened) e.preventDefault();
      }}
      onCloseAutoFocus={(e) => {
        if (autoOpened) e.preventDefault();
      }}
      onInteractOutside={(e) => {
        if (pill?.contains(e.target as Node)) e.preventDefault();
      }}
      onpointerenter={() => (hovering = true)}
      onpointerleave={() => (hovering = false)}
    >
      <IslandSheet {view} {status} {notice} {activity} {title} onClose={close} />
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>
