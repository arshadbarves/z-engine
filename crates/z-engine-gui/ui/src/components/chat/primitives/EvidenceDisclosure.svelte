<script lang="ts">
  import { evidenceAnchorId } from "$lib/domain/evidenceAnchor";
  import type { TaskReport, TaskStatus } from "$lib/types";
  import CheckEvidenceDetails from "../CheckEvidenceDetails.svelte";
  import SupervisionDetails from "./SupervisionDetails.svelte";

  type Props = {
    report: TaskReport | undefined;
    status: TaskStatus;
    description: string;
    fallbackGoal: string;
    anchorPrefix: string;
  };
  let { report, status, description, fallbackGoal, anchorPrefix }: Props = $props();

  function evidenceAnchor(id: string) {
    return evidenceAnchorId(anchorPrefix, id);
  }
</script>

<div class="evidence-disclosure">
  <p class="task-report-description">{description}</p>
  <section aria-label="Original task">
    <h4>Original goal</h4>
    <p class="task-report-goal">{report?.goal ?? fallbackGoal}</p>
    {#if report}
      <dl class="task-report-meta">
        <dt>Workspace</dt><dd><code>{report.workspaceRoot}</code></dd>
        <dt>Task ID</dt><dd><code>{report.taskId}</code></dd>
        <dt>Report schema</dt><dd>{report.schemaVersion}</dd>
      </dl>
    {/if}
  </section>

  {#if report}
    {#if report.supervision}
      <section aria-label="Task supervision">
        <h4>Task supervision</h4>
        <SupervisionDetails supervision={report.supervision} />
      </section>
    {/if}

    <section aria-label="Original requirement coverage">
      <h4>Original requirement coverage</h4>
      {#if report.requirements.length === 0}
        <p>No original requirements were recorded.</p>
      {:else}
        <ol class="task-requirements">
          {#each report.requirements as requirement}
            {@const coverage = report.assessment?.coverage.filter(
              (entry) => entry.requirementId === requirement.id,
            ) ?? []}
            <li>
              <p><code>{requirement.id}</code> · {requirement.description}</p>
              {#if coverage.length === 0}
                <p class="task-report-muted">No evidence mapped; this requirement is unassessed.</p>
              {:else}
                {#each coverage as entry}
                  <p>{entry.explanation}</p>
                  <p class="task-evidence-refs">
                    Evidence:
                    {#each entry.evidenceIds as evidenceId}
                      {#if report.checks.some((check) => check.id === evidenceId)}
                        <a href={`#${evidenceAnchor(evidenceId)}`}><code>{evidenceId}</code></a>
                      {:else}
                        <span><code>{evidenceId}</code> (not present in this report)</span>
                      {/if}
                    {:else}
                      <span>None recorded</span>
                    {/each}
                  </p>
                {/each}
              {/if}
            </li>
          {/each}
        </ol>
      {/if}
    </section>

    <section aria-label="Recorded verification checks">
      <h4>Recorded verification checks</h4>
      <p class="task-report-muted">Only recorded check evidence is shown, not proposed commands.</p>
      {#each report.checks as check}
        <CheckEvidenceDetails {check} taskStatus={status} anchor={evidenceAnchor(check.id)} />
      {:else}
        <p>No checks were recorded. A finished response is not verification.</p>
      {/each}
    </section>

    <section aria-label="Task assessment">
      <h4>Assessment</h4>
      <p>{report.assessment?.summary ?? "No assessment recorded."}</p>
    </section>

    <section aria-label="Task blockers">
      <h4>Blockers</h4>
      <ul>
        {#each report.blockers as blocker}
          <li>{blocker}</li>
        {:else}
          <li>No blockers recorded; this alone does not establish completion.</li>
        {/each}
      </ul>
    </section>

    <section aria-label="Changed paths">
      <h4>Changed paths</h4>
      <ul>
        {#each report.changedPaths as path}
          <li><code>{path}</code></li>
        {:else}
          <li>No changed paths recorded.</li>
        {/each}
      </ul>
    </section>
  {:else}
    <p>No validated report is available. Requirements, check evidence, blockers, and changed
      paths cannot be reconstructed from assistant prose.</p>
  {/if}
</div>

<style>
  .evidence-disclosure {
    padding: 10px 12px 12px;
    color: var(--text);
    font-size: 13px;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }
  section + section { border-top: 1px solid var(--border); margin-top: 14px; padding-top: 12px; }
  h4 { margin: 0 0 8px; font-size: 13px; font-weight: 600; }
  p { margin: 6px 0; }
  code { font-family: var(--mono); font-size: 12px; }
  ul, ol { margin: 0; padding-left: 20px; }
  li + li { margin-top: 8px; }
  .task-report-description { margin-top: 0; }
  .task-report-goal { white-space: pre-wrap; }
  .task-report-meta { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 4px 12px; }
  dt, .task-report-muted { color: var(--text-2); }
  dd { margin: 0; min-width: 0; }
  .task-evidence-refs { display: flex; flex-wrap: wrap; gap: 8px; }
  a { color: var(--accent-blue); text-underline-offset: 3px; }
  a:focus-visible {
    outline: 2px solid var(--accent-blue);
    outline-offset: 3px;
    border-radius: var(--radius-s);
  }
  @media (max-width: 480px) {
    .task-report-meta { grid-template-columns: minmax(0, 1fr); }
  }
</style>
