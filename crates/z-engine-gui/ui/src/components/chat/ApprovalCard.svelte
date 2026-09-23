<script lang="ts">
  import { tick } from "svelte";
  import { compactJson } from "$lib/domain/tools/toolInput";
  import { toolSubject } from "$lib/domain/tools/toolMeta";
  import type { ApprovalDecision } from "$lib/protocol/ApprovalDecision";
  import type { ApprovalRequest } from "$lib/protocol/ApprovalRequest";
  import { resolveApproval } from "$lib/runtime";
  import Icon, { ShieldAlert } from "$lib/ui/icons";
  import DiffView from "../overlays/DiffView.svelte";

  type Props = { request: ApprovalRequest; agentLabel: string | null; autoFocus?: boolean };
  let { request, agentLabel, autoFocus = false }: Props = $props();

  let root: HTMLDivElement | undefined = $state();
  let feedbackEl: HTMLInputElement | undefined = $state();
  let denying = $state(false);
  let feedback = $state("");
  let sending = $state(false);

  const preview = $derived(request.preview);
  const rule = $derived(request.suggestedRule);
  const canProject = $derived(request.canPersist && Boolean(rule));
  const fallback = $derived(toolSubject(request.tool, request.input) || compactJson(request.input, 400));

  $effect(() => {
    if (!autoFocus || !root) return;
    const active = document.activeElement;
    const idle =
      !active ||
      active === document.body ||
      (active instanceof HTMLTextAreaElement && active.value.trim() === "");
    if (idle) root.focus({ preventScroll: true });
  });

  async function decide(decision: ApprovalDecision) {
    if (sending) return;
    sending = true;
    if (!(await resolveApproval(request.requestId, decision))) sending = false;
  }

  async function startDeny() {
    denying = true;
    await tick();
    feedbackEl?.focus();
  }

  function deny() {
    const text = feedback.trim();
    void decide({ type: "deny", feedback: text || null });
  }

  function onKey(e: KeyboardEvent) {
    if (e.target !== root || e.metaKey || e.ctrlKey || e.altKey) return;
    const key = e.key.toLowerCase();
    if (key === "y") void decide({ type: "allowOnce" });
    else if (key === "s" && rule) void decide({ type: "allowSession", rule });
    else if (key === "p" && canProject && rule) void decide({ type: "allowProject", rule });
    else if (key === "n") void startDeny();
    else return;
    e.preventDefault();
  }
</script>

<!-- The card takes focus so y/s/p/n answer it without a pointer. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="msg approval"
  role="group"
  aria-label={`Approval needed: ${request.title}`}
  tabindex="0"
  bind:this={root}
  onkeydown={onKey}
>
  <div class="approval-kicker">
    <Icon icon={ShieldAlert} size={13} class="approval-kicker-icon" />
    <span>Needs approval</span>
    <span class="approval-tool-tag">{request.tool}</span>
    {#if agentLabel}<span class="interaction-agent">{agentLabel}</span>{/if}
  </div>
  <div class="approval-title">{request.title}</div>
  {#if request.reason}<p class="approval-reason">{request.reason}</p>{/if}

  {#if preview?.type === "diff"}
    <DiffView text={preview.diff} filePath={preview.path} />
  {:else if preview?.type === "command"}
    <pre class="approval-cmd"><code>{preview.command}</code></pre>
    {#if preview.description}<p class="approval-reason">{preview.description}</p>{/if}
  {:else if preview?.type === "text"}
    <pre class="approval-body">{preview.text}</pre>
  {:else if fallback}
    <pre class="approval-cmd"><code>{fallback}</code></pre>
  {/if}

  {#if denying}
    <div class="approval-feedback">
      <input
        bind:this={feedbackEl}
        bind:value={feedback}
        placeholder="Tell the agent what to do instead (optional)"
        onkeydown={(e) => {
          if (e.key === "Enter") deny();
          if (e.key === "Escape") {
            denying = false;
            root?.focus();
          }
        }}
      />
      <button type="button" class="deny" disabled={sending} onclick={deny}>Deny</button>
    </div>
  {/if}

  <div class="approval-actions">
    <button class="primary" type="button" disabled={sending} onclick={() => void decide({ type: "allowOnce" })}>
      Allow once
    </button>
    {#if rule}
      <button type="button" disabled={sending} title={rule} onclick={() => void decide({ type: "allowSession", rule })}>
        Allow for session
      </button>
    {/if}
    {#if canProject && rule}
      <button type="button" disabled={sending} title={rule} onclick={() => void decide({ type: "allowProject", rule })}>
        Always for project
      </button>
    {/if}
    {#if !denying}
      <button class="deny" type="button" disabled={sending} onclick={() => void startDeny()}>Deny…</button>
    {/if}
    <span class="hint">
      <kbd>y</kbd> once
      {#if rule}<kbd>s</kbd> session{/if}
      {#if canProject}<kbd>p</kbd> project{/if}
      <kbd>n</kbd> deny
    </span>
  </div>
  {#if rule}<p class="approval-rule">Rule: <code>{rule}</code></p>{/if}
</div>
