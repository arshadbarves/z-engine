<script lang="ts">
  import type { ActivityItem } from "$lib/domain/activity";
  import { workCounts } from "$lib/domain/agentTree";
  import { recentSteps } from "$lib/domain/island";
  import type { LiveStatus } from "$lib/domain/liveStatus";
  import { MAIN_AGENT, type SessionView } from "$lib/domain/sessionView";
  import { todoProgress } from "$lib/domain/todos";
  import { dismissToast, type Toast } from "$lib/runtime/toasts";
  import { showInbox } from "$lib/stores/app-actions";
  import { ui } from "$lib/stores/ui.svelte";
  import { Disclosure } from "$lib/ui";
  import { fmtCost, relTime } from "$lib/util";
  import TodoChecklist from "../planning/TodoChecklist.svelte";

  /** The island opened: the whole message, the live steps, plan, helpers and cost. */
  type Props = {
    view: SessionView | null;
    status: LiveStatus;
    notice: Toast | null;
    activity: ActivityItem[];
    title: string | null;
    onClose: () => void;
  };
  let { view, status, notice, activity, title, onClose }: Props = $props();

  const shownNotice = $derived(status.kind === "notice" ? notice : null);
  const todos = $derived(view?.todos[MAIN_AGENT] ?? []);
  const plan = $derived(todoProgress(todos));
  const steps = $derived(view?.activeTurn ? recentSteps(view.tools, view.activeTurn.startedAt, 4) : []);
  const helpers = $derived.by(() => {
    if (!view) return "";
    const c = workCounts(view.agents, view.jobs);
    return [
      c.runningAgents && `${c.runningAgents} agent${c.runningAgents === 1 ? "" : "s"} working`,
      c.runningJobs && `${c.runningJobs} job${c.runningJobs === 1 ? "" : "s"} running`,
      c.pendingWorktrees && `${c.pendingWorktrees} ready to apply`,
    ]
      .filter(Boolean)
      .join(" · ");
  });
  const turnCost = $derived(view?.activeTurn ? view.costUsd - view.activeTurn.costAtStart : null);
  const warnings = $derived(activity.filter((a) => a.tone !== "info").slice(0, 3));

  function go(run: () => void) {
    onClose();
    run();
  }
</script>

<div class="sheet-body">
  <header class="sheet-head">
    {#if title}<p class="sheet-title">{title}</p>{/if}
    {#if shownNotice}
      <p class={`sheet-step tone-${status.tone}`}>{shownNotice.title ?? shownNotice.text}</p>
      {#if shownNotice.title && shownNotice.title !== shownNotice.text}
        <p class="sheet-detail">{shownNotice.text}</p>
      {/if}
      <div class="sheet-actions">
        {#each shownNotice.actions ?? [] as action (action.label)}
          <button
            type="button"
            class={action.variant === "primary" ? "btn-accent" : "btn-secondary"}
            onclick={() =>
              go(() => {
                action.onclick?.();
                dismissToast(shownNotice.id);
              })}
          >
            {action.label}
          </button>
        {/each}
        <button type="button" class="btn-ghost" onclick={() => go(showInbox)}>Open in Inbox</button>
      </div>
    {:else}
      <p class={`sheet-step tone-${status.tone}`}>{status.text ?? "Ready when you are"}</p>
      {#if status.detail}<p class="sheet-detail">{status.detail}</p>{/if}
    {/if}
  </header>

  {#if steps.length}
    <section class="sheet-section">
      <h3 class="sheet-heading">Now</h3>
      <ul class="sheet-steps">
        {#each steps as step (step.callId)}
          <li class={`is-${step.state}`}>{step.label}</li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if todos.length}
    <section class="sheet-section">
      <Disclosure open={view?.status !== "idle"} summaryClass="sheet-summary">
        {#snippet summary()}
          <span class="sheet-heading">Plan</span>
          <span class="sheet-value">{plan.done} of {plan.total} done</span>
        {/snippet}
        <div class="sheet-plan"><TodoChecklist {todos} /></div>
      </Disclosure>
    </section>
  {/if}

  {#if helpers}
    <section class="sheet-section sheet-row">
      <span class="sheet-value">{helpers}</span>
      <button type="button" class="btn-ghost" onclick={() => go(() => ui.openWork("agents"))}>Open</button>
    </section>
  {/if}

  {#if view && view.costUsd > 0}
    <section class="sheet-section sheet-row">
      <span class="sheet-heading">Cost</span>
      <span class="sheet-value">
        {#if turnCost !== null}This turn {fmtCost(turnCost)} · {/if}Chat {fmtCost(view.costUsd)}
      </span>
    </section>
  {/if}

  {#if warnings.length}
    <section class="sheet-section">
      <div class="sheet-row">
        <h3 class="sheet-heading">Recent</h3>
        <button type="button" class="sheet-link" onclick={() => go(showInbox)}>Open in Inbox</button>
      </div>
      <ul class="sheet-activity">
        {#each warnings as item (item.key)}
          <li class={`tone-${item.tone}`}>
            <span class="sheet-activity-text">{item.text}</span>
            <time class="sheet-activity-time">{relTime(item.at)}</time>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>
