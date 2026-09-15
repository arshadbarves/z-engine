<script lang="ts">
  import { evidenceFreshness } from "$lib/domain/taskReport";
  import type { CheckEvidence, TaskStatus } from "$lib/types";

  type Props = { check: CheckEvidence; taskStatus: TaskStatus; anchor: string };
  let { check, taskStatus, anchor }: Props = $props();
  const outcomeLabels = {
    passed: "Passed",
    failed: "Failed",
    blocked: "Blocked",
    cancelled: "Cancelled",
    stale: "Stale",
  };
</script>

<article class="check-evidence" id={anchor} aria-label={`Check ${check.id}`}>
  <header>
    <strong><code>{check.id}</code> · {check.spec.kind === "cargo_test" ? "Cargo test" : "Cargo build"}</strong>
    <span class="check-outcome" data-outcome={check.outcome}>{outcomeLabels[check.outcome]}</span>
  </header>
  <p>{check.summary}</p>
  <dl>
    <dt>Recorded argv</dt><dd><code>{JSON.stringify(check.command)}</code></dd>
    <dt>Working directory</dt><dd><code>{check.cwd}</code></dd>
    <dt>Package</dt><dd><code>{check.spec.package ?? "Workspace default"}</code></dd>
    <dt>Filter</dt><dd><code>{check.spec.filter ?? "None"}</code></dd>
    <dt>Exit code</dt><dd>{check.exitCode ?? "Not recorded (no successful exit established)"}</dd>
    <dt>Tests run</dt><dd>{check.testsRun ?? "Not recorded"}</dd>
    <dt>Started at</dt><dd>{check.startedAtMs} ms since Unix epoch</dd>
    <dt>Duration</dt><dd>{check.durationMs} ms</dd>
    <dt>Toolchain</dt><dd><code>{check.toolchain || "Not recorded"}</code></dd>
    <dt>Freshness</dt><dd>{evidenceFreshness(check, taskStatus)}</dd>
    <dt>Input fingerprint</dt><dd><code>{check.inputFingerprint ?? "Not recorded"}</code></dd>
    {#each [{ label: "stdout", artifact: check.stdout }, { label: "stderr", artifact: check.stderr }] as item}
      <dt>{item.label} artifact</dt>
      <dd>
        {#if item.artifact}
          <code class="artifact-path">{item.artifact.path}</code>
          <span class="artifact-digest">Digest: <code>{item.artifact.digest}</code></span>
        {:else}
          No artifact recorded
        {/if}
      </dd>
    {/each}
  </dl>
  <p class="artifact-note">Artifact paths are selectable text. Availability is not checked by this view.</p>
</article>

<style>
  .check-evidence {
    margin-top: 10px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-s);
    background: var(--surface);
    scroll-margin-top: 16px;
    overflow-wrap: anywhere;
    min-width: 0;
  }
  .check-evidence:target { border-color: var(--accent-blue); }
  header { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px; }
  .check-outcome { font-weight: 600; }
  .check-outcome[data-outcome="failed"] { color: var(--err); }
  .check-outcome[data-outcome="blocked"], .check-outcome[data-outcome="stale"] { color: var(--tone-attention); }
  p { margin: 8px 0; }
  dl { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 6px 12px; margin-bottom: 0; }
  dt, .artifact-note { color: var(--text-2); }
  dd { margin: 0; min-width: 0; }
  code { font-family: var(--mono); font-size: 12px; white-space: pre-wrap; user-select: text; }
  .artifact-path, .artifact-digest { display: block; }
  .artifact-note { font-size: 12px; }
  @media (max-width: 480px) {
    dl { grid-template-columns: minmax(0, 1fr); }
    dt { margin-top: 6px; }
  }
</style>
