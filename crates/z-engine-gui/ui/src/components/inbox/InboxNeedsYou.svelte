<script lang="ts">
  import type { NeedsYouItem } from "$lib/domain/inbox";
  import { resolveApproval } from "$lib/runtime";
  import { openChatById } from "$lib/stores/app-actions";
  import Icon, { HelpCircle, ListChecks, Lock, Shield } from "$lib/ui/icons";

  /** Chats blocked on you. Approvals can be answered right here; the rest open their chat. */
  type Props = { items: NeedsYouItem[] };
  let { items }: Props = $props();

  const ICONS = { approval: Shield, question: HelpCircle, plan: ListChecks, trust: Lock } as const;
  const OPEN = { approval: "Open chat", question: "Answer", plan: "Review plan", trust: "Decide" } as const;
  let busy = $state<Record<string, boolean>>({});

  async function decide(item: NeedsYouItem, allow: boolean) {
    if (!item.requestId) return;
    busy[item.key] = true;
    const decision = allow ? ({ type: "allowOnce" } as const) : ({ type: "deny", feedback: null } as const);
    await resolveApproval(item.requestId, decision, item.sessionId);
    busy[item.key] = false;
  }
</script>

<section class="inbox-section" aria-label="Needs you">
  <h2 class="inbox-heading">Needs you <span class="inbox-count tone-attention">{items.length}</span></h2>
  <ul class="inbox-list">
    {#each items as item (item.key)}
      <li class="inbox-item is-attention">
        <span class="inbox-icon" aria-hidden="true"><Icon icon={ICONS[item.kind]} size={15} /></span>
        <div class="inbox-body">
          <p class="inbox-item-title">{item.title}</p>
          <p class="inbox-item-meta">
            <span>{item.chatTitle}</span>
            {#if item.detail}<span class="inbox-item-detail">{item.detail}</span>{/if}
          </p>
        </div>
        <div class="inbox-actions">
          {#if item.kind === "approval"}
            <button type="button" class="btn-accent" disabled={busy[item.key]} onclick={() => void decide(item, true)}>Allow once</button>
            <button type="button" class="btn-secondary" disabled={busy[item.key]} onclick={() => void decide(item, false)}>Deny</button>
          {/if}
          <button type="button" class="btn-ghost" onclick={() => void openChatById(item.sessionId)}>{OPEN[item.kind]}</button>
        </div>
      </li>
    {/each}
  </ul>
</section>
