<script lang="ts">
  import type { AgentStatus } from "$lib/protocol/AgentStatus";
  import type { JobStatus } from "$lib/protocol/JobStatus";

  type Props = { status: AgentStatus | JobStatus };
  let { status }: Props = $props();

  const LABELS: Record<AgentStatus | JobStatus, string> = {
    running: "Running",
    waiting: "Waiting",
    completed: "Done",
    failed: "Failed",
    cancelled: "Cancelled",
    killed: "Killed",
  };
</script>

<span class={`status-chip status-${status}`}>
  <span class="status-chip-dot" aria-hidden="true"></span>
  {LABELS[status]}
</span>
