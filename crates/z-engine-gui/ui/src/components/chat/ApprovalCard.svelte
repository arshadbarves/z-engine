<script lang="ts">
  import { approvalCommand, approvalToolName } from "$lib/approvalPreview";
  import { looksLikeDiff } from "$lib/diffParse";
  import type { Msg } from "$lib/types";
  import Icon, { ShieldAlert } from "$lib/ui/icons";
  import DiffView from "../overlays/DiffView.svelte";

  type Props = {
    m: Msg;
    onApprove: (decision: "once" | "session" | "persist") => void;
    onDeny: () => void;
  };
  let { m, onApprove, onDeny }: Props = $props();

  const tool = $derived(approvalToolName(m));
  const command = $derived(approvalCommand(m));
  const preview = $derived(m.detailPreview ?? null);
  const isDiff = $derived(preview != null && looksLikeDiff(preview));
</script>

<div class="msg approval">
  <div class="approval-kicker">
    <Icon icon={ShieldAlert} size={13} class="approval-kicker-icon" />
    <span>Needs approval</span>
  </div>
  <div class="approval-title">{tool}</div>
  {#if isDiff && preview}
    <DiffView text={preview} />
  {:else if command}
    <pre class="approval-cmd"><code>{command}</code></pre>
  {/if}
  <div class="approval-actions">
    <button class="primary" onclick={() => onApprove("once")} type="button">Allow once</button>
    <button onclick={() => onApprove("session")} type="button">Always · session</button>
    {#if m.canPersist}
      <button onclick={() => onApprove("persist")} type="button">Always · persist</button>
    {/if}
    <button class="deny" onclick={onDeny} type="button">Deny</button>
    <span class="hint">
      <kbd>y</kbd> once <kbd>s</kbd> session
      {#if m.canPersist}
        <kbd>p</kbd> persist
      {/if}
      <kbd>n</kbd> deny
    </span>
  </div>
</div>
