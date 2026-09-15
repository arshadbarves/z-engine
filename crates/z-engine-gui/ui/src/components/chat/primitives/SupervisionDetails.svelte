<script lang="ts">
  import { supervisionActionLabels, type SupervisionReport } from "$lib/domain/taskReport";

  type Props = { supervision: SupervisionReport };
  let { supervision }: Props = $props();
</script>

<dl class="supervision-meta">
  <dt>Continuations used</dt>
  <dd>{supervision.continuations} / {supervision.maxContinuations}</dd>
  <dt>Last decision</dt>
  <dd>{supervisionActionLabels[supervision.lastAction]}</dd>
  <dt>Reason</dt>
  <dd class="supervision-reason">{supervision.reason}</dd>
</dl>

<style>
  .supervision-meta {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 4px 12px;
    margin: 0;
  }
  dt { color: var(--text-2); }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  .supervision-reason { white-space: pre-wrap; }
  @media (max-width: 480px) {
    .supervision-meta { grid-template-columns: minmax(0, 1fr); }
  }
</style>
