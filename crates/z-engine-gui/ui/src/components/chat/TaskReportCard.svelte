<script lang="ts">
  import { configStore } from "$lib/configStore";
  import { displayedTaskStatus, taskStatusDescriptions, taskStatusLabels } from "$lib/domain/taskReport";
  import { projectTaskReportPresentation } from "$lib/domain/taskReportPresentation";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import type { Msg } from "$lib/types";
  import Icon, { ChevronRight } from "$lib/ui/icons";
  import EvidenceDisclosure from "./primitives/EvidenceDisclosure.svelte";
  import TaskReportSummary from "./primitives/TaskReportSummary.svelte";
  import TaskStatusChip from "./primitives/TaskStatusChip.svelte";

  type Props = { m: Msg };
  let { m }: Props = $props();
  const config = bindStore(configStore);
  const report = $derived(m.taskReport);
  const freshnessPending = $derived(Boolean(m.taskFreshnessPending && report?.status === "complete"));
  const status = $derived(displayedTaskStatus(report, freshnessPending));
  const label = $derived(freshnessPending ? "Freshness unknown" : taskStatusLabels[status]);
  const description = $derived(m.taskReportError ?? (freshnessPending
    ? "Historical completion evidence has not been revalidated against the current workspace."
    : taskStatusDescriptions[status]));
  const presentation = $derived(projectTaskReportPresentation(config.current, report));
</script>

{#if presentation.details === "inline"}
  <article class="task-report-card" aria-label="Task report">
    <header class="task-report-header">
      <TaskStatusChip {status} {label} />
      <TaskReportSummary checkCount={report?.checks.length ?? 0} summary={null} />
    </header>
    <EvidenceDisclosure
      {report}
      {status}
      {description}
      fallbackGoal={m.text}
      anchorPrefix={`task-${m.id}-evidence`}
    />
  </article>
{:else}
  <details class="task-report-disclosure">
    <summary class="task-report-trigger" aria-label={`Task report: ${label}. Show full details`}>
      <TaskStatusChip {status} {label} />
      {#if presentation.showCheckCount}
        <TaskReportSummary
          checkCount={report?.checks.length ?? 0}
          summary={presentation.summary}
        />
        <span class="task-report-hint">Details</span>
      {/if}
      <!-- Quiet mode drops the summary text, so the chevron is the only thing
           telling a reader that evidence is available. It is never optional. -->
      <span class="task-report-chevron" aria-hidden="true">
        <Icon icon={ChevronRight} size={10} strokeWidth={2.2} />
      </span>
    </summary>
    <div class="task-report-card">
      <EvidenceDisclosure
        {report}
        {status}
        {description}
        fallbackGoal={m.text}
        anchorPrefix={`task-${m.id}-evidence`}
      />
    </div>
  </details>
{/if}

<style>
  /* Quiet fill instead of a bordered card: the report already sits inside the
     canvas island, and the status chip carries the emphasis. */
  .task-report-card {
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-m);
    background: var(--surface-quiet);
  }
  .task-report-header,
  .task-report-trigger {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 8px;
  }
  .task-report-header { padding: 10px 12px 0; }
  .task-report-trigger {
    width: fit-content;
    max-width: 100%;
    list-style: none;
    cursor: pointer;
    padding: 2px 0;
  }
  .task-report-trigger::-webkit-details-marker { display: none; }
  .task-report-trigger:focus-visible {
    outline: 2px solid var(--accent-blue);
    outline-offset: 3px;
    border-radius: 999px;
  }
  .task-report-disclosure[open] .task-report-card {
    margin-top: 8px;
    animation: disclosure-in var(--dur-fast) var(--ease-out) both;
  }
  .task-report-hint {
    flex: none;
    color: var(--text-2);
    font-size: 12px;
  }
  .task-report-chevron {
    display: inline-flex;
    flex: none;
    align-items: center;
    color: var(--text-3);
    transition: transform var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
  }
  .task-report-trigger:hover .task-report-chevron { color: var(--text-2); }
  .task-report-disclosure[open] .task-report-chevron { transform: rotate(90deg); }
  @media (max-width: 480px) {
    .task-report-hint { display: none; }
  }
</style>
