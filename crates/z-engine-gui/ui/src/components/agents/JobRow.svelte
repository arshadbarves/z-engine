<script lang="ts">
  import { elapsed } from "$lib/domain/format";
  import type { JobInfo } from "$lib/protocol/JobInfo";
  import { killJob } from "$lib/runtime";
  import Icon, { Bot, SquareTerminal } from "$lib/ui/icons";
  import StatusChip from "./StatusChip.svelte";

  type Props = { job: JobInfo; now: number; onOpenAgent: (agentId: string) => void };
  let { job, now, onOpenAgent }: Props = $props();

  let killing = $state(false);

  async function kill() {
    killing = true;
    if (!(await killJob(job.jobId))) killing = false;
  }
</script>

<div class={`job-row status-${job.status}`}>
  <div class="job-row-head">
    <Icon icon={job.kind === "agent" ? Bot : SquareTerminal} size={12} class="job-kind-icon" />
    <span class="job-label" title={job.label}>{job.label}</span>
    <StatusChip status={job.status} />
  </div>
  <div class="job-row-meta">
    <span>{job.kind === "agent" ? "background agent" : "shell"}</span>
    <span>{elapsed(job.startedAt, job.finishedAt, now)}</span>
    {#if job.exitCode !== null}<span class={job.exitCode === 0 ? "" : "job-exit-bad"}>exit {job.exitCode}</span>{/if}
    <span class="job-row-spacer"></span>
    {#if job.agentId}
      <button type="button" class="job-link" onclick={() => job.agentId && onOpenAgent(job.agentId)}>Transcript</button>
    {/if}
    {#if job.status === "running"}
      <button type="button" class="btn-danger job-kill" disabled={killing} onclick={() => void kill()}>
        {killing ? "Killing…" : "Kill"}
      </button>
    {/if}
  </div>
  {#if job.outputTail}
    <details class="job-output" open={job.status === "running"}>
      <summary>Output</summary>
      <pre>{job.outputTail}</pre>
    </details>
  {/if}
</div>
