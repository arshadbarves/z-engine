<script lang="ts">
  import { untrack } from "svelte";
  import type { PendingPlan } from "$lib/protocol/PendingPlan";
  import type { PermissionMode } from "$lib/protocol/PermissionMode";
  import type { PlanDecision } from "$lib/protocol/PlanDecision";
  import { resolvePlan } from "$lib/runtime";
  import Icon, { Eye, ListChecks, Pencil } from "$lib/ui/icons";
  import Markdown from "../chat/Markdown.svelte";

  type Props = { pending: PendingPlan; agentLabel: string | null };
  let { pending, agentLabel }: Props = $props();

  let editing = $state(false);
  let draft = $state(untrack(() => pending.plan));
  let revising = $state(false);
  let feedback = $state("");
  let sending = $state(false);
  const edited = $derived(draft.trim() !== pending.plan.trim());

  async function decide(decision: PlanDecision) {
    if (sending) return;
    sending = true;
    if (!(await resolvePlan(pending.requestId, decision))) sending = false;
  }

  function approve(mode: PermissionMode) {
    void decide({ type: "approve", mode, editedPlan: edited ? draft : null });
  }

  function keepPlanning() {
    if (!revising) {
      revising = true;
      return;
    }
    void decide({ type: "revise", feedback: feedback.trim() });
  }
</script>

<div class="msg interaction-card plan-card" data-pending-card>
  <div class="interaction-kicker">
    <Icon icon={ListChecks} size={13} />
    <span>Plan ready for review</span>
    {#if agentLabel}<span class="interaction-agent">{agentLabel}</span>{/if}
    <span class="interaction-kicker-spacer"></span>
    {#if edited}<span class="plan-edited-tag">edited</span>{/if}
    <button
      type="button"
      class={`plan-edit-toggle${editing ? " active" : ""}`}
      aria-pressed={editing}
      onclick={() => (editing = !editing)}
    >
      <Icon icon={editing ? Eye : Pencil} size={11} />
      <span>{editing ? "Preview" : "Edit"}</span>
    </button>
  </div>

  {#if editing}
    <textarea class="plan-editor" bind:value={draft} rows={14} spellcheck={false}></textarea>
  {:else}
    <div class="plan-body"><Markdown text={draft} /></div>
  {/if}

  {#if revising}
    <!-- svelte-ignore a11y_autofocus -->
    <textarea
      class="interaction-feedback"
      bind:value={feedback}
      rows={2}
      autofocus
      placeholder="What should change in the plan?"
      onkeydown={(e) => {
        if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) keepPlanning();
        if (e.key === "Escape") revising = false;
      }}
    ></textarea>
  {/if}

  <div class="interaction-actions">
    <button type="button" class="btn-accent" disabled={sending} onclick={() => approve("acceptEdits")}>
      Approve & auto-accept edits
    </button>
    <button type="button" class="btn-primary" disabled={sending} onclick={() => approve("default")}>
      Approve & ask before edits
    </button>
    <button type="button" class="btn-ghost" disabled={sending} onclick={keepPlanning}>
      {revising ? "Send feedback" : "Keep planning"}
    </button>
  </div>
</div>
