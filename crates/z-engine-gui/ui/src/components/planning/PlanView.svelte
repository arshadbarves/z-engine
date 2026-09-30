<script lang="ts">
  import { untrack } from "svelte";
  import { panelPlan } from "$lib/domain/plans";
  import { MAIN_AGENT } from "$lib/domain/sessionView";
  import { todoProgress } from "$lib/domain/todos";
  import type { PermissionMode } from "$lib/protocol/PermissionMode";
  import type { PlanDecision } from "$lib/protocol/PlanDecision";
  import { resolvePlan, sessions } from "$lib/runtime";
  import { Button, EmptyState, Pill } from "$lib/ui";
  import Icon, { Eye, ListChecks, Pencil } from "$lib/ui/icons";
  import Markdown from "../chat/Markdown.svelte";
  import TodoChecklist from "./TodoChecklist.svelte";

  /**
   * The side panel's Plan tab: a plan waiting for review, with its checklist
   * and Approve / Keep planning; with nothing pending, the chat's last plan.
   * A pending plan can be edited before it is approved.
   */
  const view = $derived(sessions.active);
  const shown = $derived(panelPlan(view));
  const pending = $derived(shown?.kind === "pending" ? shown.pending : null);
  const text = $derived(shown ? (shown.kind === "pending" ? shown.pending.plan : shown.plan) : "");
  const progress = $derived(todoProgress(shown?.todos));
  const agentLabel = $derived.by(() => {
    if (!pending || pending.agentId === MAIN_AGENT) return null;
    const info = view?.agents[pending.agentId];
    return info ? `${info.agentType} · ${info.description}` : pending.agentId;
  });

  let revising = $state(false);
  let feedback = $state("");
  let sending = $state(false);
  let editing = $state(false);
  let draft = $state("");
  const edited = $derived(pending !== null && draft.trim() !== pending.plan.trim());
  const requestId = $derived(pending?.requestId ?? null);

  // Each plan under review starts unedited, without a reply half written.
  $effect(() => {
    void requestId;
    untrack(() => {
      draft = pending?.plan ?? "";
      revising = false;
      editing = false;
      feedback = "";
      sending = false;
    });
  });

  async function decide(decision: PlanDecision) {
    if (!pending || sending) return;
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

<div class="plan-view">
  {#if !shown}
    <EmptyState
      icon={ListChecks}
      title="No plan yet"
      description="In Plan mode the agent researches first and proposes a plan here. Nothing changes until you approve it."
    />
  {:else}
    <header class="plan-view-head">
      <span class={`plan-view-kicker${pending ? " is-pending" : ""}`}>{pending ? "Ready for review" : "Last plan"}</span>
      {#if agentLabel}<Pill tone="info">{agentLabel}</Pill>{/if}
      {#if edited}<Pill>Edited</Pill>{/if}
      <span class="plan-view-space"></span>
      {#if progress.total}<span class="plan-view-progress">{progress.done} of {progress.total} done</span>{/if}
      {#if pending}
        <Button size="s" active={editing} aria-pressed={editing} disabled={sending} onclick={() => (editing = !editing)}>
          <Icon icon={editing ? Eye : Pencil} size={13} />
          {editing ? "Preview" : "Edit"}
        </Button>
      {/if}
    </header>

    <div class="plan-view-body">
      {#if pending && editing}
        <textarea
          class="text-field plan-view-editor"
          bind:value={draft}
          spellcheck={false}
          aria-label="Edit the plan before approving it"
          onkeydown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              e.stopPropagation();
              editing = false;
            }
          }}
        ></textarea>
      {:else}
        <div class="plan-view-text"><Markdown text={pending ? draft : text} /></div>
      {/if}
      {#if shown.todos.length}
        <section class="plan-view-todos" aria-label="Checklist">
          <h3 class="plan-view-heading">Checklist</h3>
          <TodoChecklist todos={shown.todos} />
        </section>
      {/if}
    </div>

    {#if pending}
      <footer class="plan-view-actions">
        {#if revising}
          <!-- svelte-ignore a11y_autofocus -->
          <textarea
            class="plan-view-feedback"
            bind:value={feedback}
            rows={3}
            autofocus
            placeholder="What should change in the plan?"
            onkeydown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) keepPlanning();
              if (e.key === "Escape") {
                e.preventDefault();
                e.stopPropagation();
                revising = false;
              }
            }}
          ></textarea>
        {/if}
        <div class="plan-view-buttons">
          <Button variant="accent" disabled={sending} onclick={() => approve("acceptEdits")}>Approve & auto-accept edits</Button>
          <Button variant="secondary" disabled={sending} onclick={() => approve("default")}>Approve & ask before edits</Button>
          <Button disabled={sending} onclick={keepPlanning}>{revising ? "Send feedback" : "Keep planning"}</Button>
        </div>
      </footer>
    {/if}
  {/if}
</div>
