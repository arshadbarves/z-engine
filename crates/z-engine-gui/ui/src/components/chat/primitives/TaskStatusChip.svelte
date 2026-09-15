<script lang="ts">
  import type { TaskStatus } from "$lib/types";

  type Props = { status: TaskStatus; label: string };
  let { status, label }: Props = $props();
</script>

<span class="task-status-chip" data-task-status={status} role="status" aria-atomic="true">
  <span class="sr-only">Task status: </span>{label}
</span>

<style>
  .task-status-chip {
    display: inline-flex;
    align-items: center;
    min-height: 24px;
    padding: 1px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    color: var(--text-2);
    background: var(--surface-quiet);
    font-size: 12px;
    font-weight: 600;
    line-height: 1.4;
  }
  [data-task-status="complete"] { color: var(--tone-working); }
  [data-task-status="blocked"],
  [data-task-status="stale"],
  [data-task-status="needs_verification"] { color: var(--tone-attention); }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
