<script lang="ts">
  import { setMode } from "$lib/commands";
  import { modeStore } from "$lib/runtime";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import Icon, { ChevronDown, Shield } from "$lib/ui/icons";

  const MODES = [
    { id: "normal", label: "normal", desc: "Ask before every file edit and command" },
    { id: "accept-edits", label: "auto-accept edits", desc: "Apply edits without asking; commands still gated" },
    { id: "plan", label: "plan", desc: "Read-only — explore and propose, change nothing" },
  ] as const;

  const mode = bindStore(modeStore);
  let open = $state(false);
  const current = $derived(MODES.find((m) => m.id === mode.current) ?? MODES[0]);

  async function pick(id: string) {
    open = false;
    if (id === mode.current) return;
    modeStore.set(id);
    try {
      await setMode(id);
    } catch (e) {
      console.error(e);
    }
  }
</script>

<div class="model-picker">
  {#if open}
    <div class="popover-backdrop" onclick={() => (open = false)}></div>
  {/if}
  <button class="mode model-btn" onclick={() => (open = !open)} title="Permission mode">
    <Icon icon={Shield} size={11} />
    <span>{current.label}</span>
    <Icon icon={ChevronDown} size={9} strokeWidth={2.4} />
  </button>
  {#if open}
    <div class="popover" role="menu">
      <div class="popover-head">Permission mode</div>
      <div class="popover-current">{current.label}</div>
      {#each MODES.filter((m) => m.id !== mode.current) as m}
        <button class="popover-item" role="menuitem" onclick={() => void pick(m.id)}>
          {m.label}
          <span class="popover-sub">{m.desc}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>
