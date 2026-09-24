<script lang="ts">
  import { plural } from "$lib/domain/format";
  import { verificationBadge } from "$lib/domain/verification";
  import type { CheckRecord } from "$lib/protocol/CheckRecord";
  import type { VerificationOutcome } from "$lib/protocol/VerificationOutcome";
  import Icon, { AlertOctagon, AlertTriangle, CheckCircle2, Info, LoaderCircle } from "$lib/ui/icons";
  import CheckEvidence from "./CheckEvidence.svelte";

  type Props = { outcome: VerificationOutcome; evidence: CheckRecord[]; live?: boolean };
  let { outcome, evidence, live = false }: Props = $props();

  const ICONS = { ok: CheckCircle2, warn: AlertTriangle, err: AlertOctagon, neutral: Info } as const;
  const badge = $derived(verificationBadge(outcome));
  const hasDetail = $derived(evidence.length > 0 || Boolean(badge.reason));
  let open = $state(false);
</script>

<div class="verify-wrap">
  <button
    type="button"
    class={`verify-badge tone-${badge.tone}${live ? " live" : ""}`}
    disabled={!hasDetail}
    aria-expanded={hasDetail ? open : undefined}
    onclick={() => (open = !open)}
  >
    <Icon icon={live ? LoaderCircle : ICONS[badge.tone]} size={11} class={live ? "spin" : ""} />
    <span>{live ? `Verifying · ${badge.label}` : badge.label}</span>
    {#if evidence.length > 0}<span class="verify-count">{plural(evidence.length, "check")}</span>{/if}
  </button>
  {#if open && hasDetail}
    <div class="verify-detail">
      {#if badge.reason}<p class="verify-reason">{badge.reason}</p>{/if}
      <CheckEvidence records={evidence} />
    </div>
  {/if}
</div>
