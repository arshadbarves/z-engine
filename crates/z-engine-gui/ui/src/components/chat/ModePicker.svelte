<script lang="ts">
  import { MODES, modeMeta, type ModeMeta } from "$lib/domain/modes";
  import type { PermissionMode } from "$lib/protocol/PermissionMode";
  import { sessions, setMode } from "$lib/runtime";
  import Icon, { AlertTriangle, ChevronDown, Shield, ShieldAlert } from "$lib/ui/icons";

  const mode: PermissionMode = $derived(sessions.active?.mode ?? "default");
  const current = $derived(modeMeta(mode));
  let open = $state(false);
  let confirming = $state<ModeMeta | null>(null);

  function close() {
    open = false;
    confirming = null;
  }

  function pick(meta: ModeMeta) {
    if (meta.warning && !confirming) {
      confirming = meta;
      return;
    }
    close();
    if (meta.id !== mode) void setMode(meta.id);
  }
</script>

<div class="model-picker">
  {#if open}
    <button type="button" class="popover-backdrop" aria-label="Close permission mode menu" tabindex="-1" onclick={close}
    ></button>
  {/if}
  <button
    class={`mode model-btn${mode === "bypass" ? " mode-danger" : ""}`}
    onclick={() => (open ? close() : (open = true))}
    title="Permission mode (Shift+Tab cycles)"
  >
    <Icon icon={mode === "bypass" ? ShieldAlert : Shield} size={11} />
    <span>{current.label}</span>
    <Icon icon={ChevronDown} size={9} strokeWidth={2.4} />
  </button>
  {#if open}
    <div class="popover" role="menu">
      <div class="popover-head">Permission mode</div>
      <div class="popover-current">{current.label} · {current.description}</div>
      {#if confirming}
        <div class="mode-warning" role="alert">
          <Icon icon={AlertTriangle} size={13} />
          <span>{confirming.warning}</span>
        </div>
        <div class="mode-warning-actions">
          <button type="button" class="btn-danger" onclick={() => confirming && pick(confirming)}>
            Enable {confirming.label}
          </button>
          <button type="button" class="btn-ghost" onclick={() => (confirming = null)}>Back</button>
        </div>
      {:else}
        {#each MODES.filter((m) => m.id !== mode) as meta (meta.id)}
          <button class={`popover-item${meta.warning ? " danger" : ""}`} role="menuitem" onclick={() => pick(meta)}>
            {meta.label}
            <span class="popover-sub">{meta.description}</span>
          </button>
        {/each}
      {/if}
    </div>
  {/if}
</div>
