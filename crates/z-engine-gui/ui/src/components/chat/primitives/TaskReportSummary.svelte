<script lang="ts">
  type Props = { checkCount: number; summary: string | null };
  let { checkCount, summary }: Props = $props();
</script>

<span class="task-report-summary">
  <span>{checkCount} {checkCount === 1 ? "check" : "checks"}</span>
  {#if summary}
    <span class="task-report-summary-text">{summary}</span>
  {/if}
</span>

<style>
  .task-report-summary {
    display: inline-flex;
    min-width: 0;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
    font-size: 12px;
  }
  .task-report-summary-text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .task-report-summary-text::before {
    content: "·";
    margin-right: 8px;
  }
</style>
