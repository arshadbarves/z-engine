<script lang="ts">
  import { fmtDuration } from "$lib/domain/format";
  import { usageLine } from "$lib/domain/usage";
  import { outcomeNote, turnEvidence } from "$lib/domain/verification";
  import type { CheckRecord } from "$lib/protocol/CheckRecord";
  import type { TurnRecord } from "$lib/protocol/TurnRecord";
  import { fmtCost } from "$lib/util";
  import VerificationBadge from "./VerificationBadge.svelte";

  type Props = { record: TurnRecord; checks: CheckRecord[] };
  let { record, checks }: Props = $props();

  const note = $derived(outcomeNote(record.outcome));
  const evidence = $derived(turnEvidence(record, checks));
  const stats = $derived(
    [
      fmtDuration(record.finishedAt - record.startedAt),
      usageLine(record.usage),
      record.costUsd > 0 ? fmtCost(record.costUsd) : "",
    ].filter(Boolean),
  );
</script>

<footer class="turn-footer">
  <VerificationBadge outcome={record.verification} {evidence} />
  {#if note}<span class={`turn-outcome tone-${note.tone}`}>{note.label}</span>{/if}
  {#if stats.length > 0}<span class="turn-stats">{stats.join(" · ")}</span>{/if}
</footer>
