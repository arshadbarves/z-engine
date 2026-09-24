<script lang="ts">
  import type { ActivityItem } from "$lib/domain/activity";
  import { workCounts } from "$lib/domain/agentTree";
  import type { CtxMeter } from "$lib/domain/contextMeter";
  import type { LiveStatus } from "$lib/domain/liveStatus";
  import { MAIN_AGENT, type SessionView } from "$lib/domain/sessionView";
  import { compact, pushToast } from "$lib/runtime";
  import { ui } from "$lib/stores/ui.svelte";
  import { fmtCost, fmtTokens, relTime } from "$lib/util";
  import TodoChecklist from "../planning/TodoChecklist.svelte";

  /** Everything behind the status line, in full: the step, plan, context, cost, agents and recent notices. */
  type Props = {
    view: SessionView;
    status: LiveStatus;
    title: string | null;
    meter: CtxMeter;
    activity: ActivityItem[];
    onClose: () => void;
  };
  let { view, status, title, meter, activity, onClose }: Props = $props();

  const todos = $derived(view.todos[MAIN_AGENT] ?? []);
  const counts = $derived(workCounts(view.agents, view.jobs));
  const agentsText = $derived(
    [
      counts.runningAgents && `${counts.runningAgents} agent${counts.runningAgents === 1 ? "" : "s"} running`,
      counts.runningJobs && `${counts.runningJobs} job${counts.runningJobs === 1 ? "" : "s"} running`,
      counts.pendingWorktrees && `${counts.pendingWorktrees} waiting to apply`,
    ]
      .filter(Boolean)
      .join(" · "),
  );
  const turnCost = $derived(view.activeTurn ? view.costUsd - view.activeTurn.costAtStart : null);
  let compacting = $state(false);

  async function onCompact() {
    compacting = true;
    if (await compact()) {
      pushToast("Compacting the conversation…", "info");
      onClose();
    }
    compacting = false;
  }

  function onInspect() {
    onClose();
    ui.inspectOpen = true;
  }

  function onAgents() {
    onClose();
    ui.openWork("agents");
  }
</script>

<div class="now-card-body">
  <header class="now-head">
    {#if title}<p class="now-title">{title}</p>{/if}
    <p class={`now-step tone-${status.tone}`}>{status.text ?? "Ready when you are"}</p>
    {#if status.detail}<p class="now-detail">{status.detail}</p>{/if}
  </header>

  {#if todos.length}
    <section class="now-section">
      <h3 class="now-heading">Plan</h3>
      <TodoChecklist {todos} />
    </section>
  {/if}

  <section class="now-section">
    <div class="now-row">
      <span class="now-heading">Context</span>
      <span class="now-value">{meter.pct}% · {fmtTokens(meter.used)} of {fmtTokens(meter.max)}</span>
    </div>
    <div class={`now-meter level-${meter.level}`} aria-hidden="true">
      <span style:width={`${meter.pct}%`}></span>
    </div>
    <div class="now-actions">
      <button type="button" class="now-btn" disabled={compacting || view.status !== "idle"} onclick={onCompact}>
        Compact
      </button>
      <button type="button" class="now-btn" onclick={onInspect}>Inspect prompt</button>
    </div>
  </section>

  <section class="now-section now-row">
    <span class="now-heading">Cost</span>
    <span class="now-value">
      {#if turnCost !== null}This turn {fmtCost(turnCost)} · {/if}Session {fmtCost(view.costUsd)}
    </span>
  </section>

  {#if agentsText}
    <section class="now-section now-row">
      <span class="now-value">{agentsText}</span>
      <button type="button" class="now-btn" onclick={onAgents}>Open</button>
    </section>
  {/if}

  {#if activity.length}
    <section class="now-section">
      <h3 class="now-heading">Recent</h3>
      <ul class="now-activity">
        {#each activity.slice(0, 6) as item (item.key)}
          <li class={`tone-${item.tone}`}>
            <span class="now-activity-text">{item.text}</span>
            <time class="now-activity-time">{relTime(item.at)}</time>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>
