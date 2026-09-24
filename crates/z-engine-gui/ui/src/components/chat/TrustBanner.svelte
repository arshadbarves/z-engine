<script lang="ts">
  import type { TrustRequestView } from "$lib/domain/sessionView/types";
  import { answerTrust } from "$lib/runtime";
  import Icon, { ShieldAlert } from "$lib/ui/icons";

  type Props = { request: TrustRequestView };
  let { request }: Props = $props();

  let sending = $state(false);

  async function answer(trusted: boolean) {
    if (sending) return;
    sending = true;
    if (!(await answerTrust(trusted))) sending = false;
  }
</script>

<div class="msg interaction-card trust-banner" role="region" aria-label="Workspace trust">
  <div class="interaction-kicker">
    <Icon icon={ShieldAlert} size={13} />
    <span>Trust this workspace?</span>
  </div>
  <p class="trust-banner-text">
    <code>{request.projectRoot}</code> defines {request.defines.join(", ")}. They stay off until you trust it,
    because they run commands on your machine.
  </p>
  <div class="interaction-actions">
    <button type="button" class="btn-accent" disabled={sending} onclick={() => void answer(true)}>
      Trust this workspace
    </button>
    <button type="button" class="btn-ghost" disabled={sending} onclick={() => void answer(false)}>Not now</button>
  </div>
</div>

<style>
  .trust-banner-text {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-2);
    overflow-wrap: anywhere;
  }
</style>
