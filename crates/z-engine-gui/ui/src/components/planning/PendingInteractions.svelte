<script lang="ts">
  import type { SessionView } from "$lib/domain/sessionView";
  import { MAIN_AGENT } from "$lib/domain/sessionView";
  import ApprovalCard from "../chat/ApprovalCard.svelte";
  import PlanCard from "./PlanCard.svelte";
  import QuestionCard from "./QuestionCard.svelte";

  type Props = { view: SessionView };
  let { view }: Props = $props();

  const approvals = $derived(Object.values(view.approvals));
  const questions = $derived(Object.values(view.questions));
  const plans = $derived(Object.values(view.plans));

  function agentLabel(agentId: string): string | null {
    if (agentId === MAIN_AGENT) return null;
    const info = view.agents[agentId];
    return info ? `${info.agentType} · ${info.description}` : agentId;
  }
</script>

{#each approvals as request, i (request.requestId)}
  <ApprovalCard {request} agentLabel={agentLabel(request.agentId)} autoFocus={i === 0} />
{/each}
{#each questions as pending (pending.requestId)}
  <QuestionCard {pending} agentLabel={agentLabel(pending.agentId)} />
{/each}
{#each plans as pending (pending.requestId)}
  <PlanCard {pending} agentLabel={agentLabel(pending.agentId)} />
{/each}
