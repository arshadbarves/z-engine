<script lang="ts">
  import type { Catalog } from "$lib/catalog";
  import Icon, { ArrowUp, CornerDownLeft, Paperclip, Square, Terminal } from "$lib/ui/icons";
  import EffortSelector from "./EffortSelector.svelte";
  import ModePicker from "./ModePicker.svelte";
  import ModelPicker from "./ModelPicker.svelte";

  type Props = {
    shellMode: boolean;
    busy: boolean;
    canSend: boolean;
    canSendShell: boolean;
    catalog: Catalog | null;
    showTerminalBtn: boolean;
    onAttachClick: () => void;
    onShowShell: () => void;
    onSend: () => void;
    onAbort: () => void;
  };

  let {
    shellMode,
    busy,
    canSend,
    canSendShell,
    catalog,
    showTerminalBtn,
    onAttachClick,
    onShowShell,
    onSend,
    onAbort,
  }: Props = $props();
</script>

<div class="composer-bar">
  {#if shellMode}
    <div class="shell-bar-left">
      <span class="shell-mode-pill">
        <Icon icon={Terminal} size={11} />
        <span>Bash Mode</span>
      </span>
      <span class="shell-hint-inline"><kbd>Esc</kbd> to return</span>
    </div>
  {:else}
    <div class="composer-controls-left">
      <ModePicker />
      <ModelPicker />
      <EffortSelector {catalog} />
      <button
        type="button"
        class="composer-icon-btn"
        title="Attach file or image"
        onclick={onAttachClick}
      >
        <Icon icon={Paperclip} size={13} />
      </button>
      {#if showTerminalBtn}
        <button
          type="button"
          class="composer-icon-btn"
          title="Show terminal drawer"
          onclick={onShowShell}
        >
          <Icon icon={Terminal} size={13} />
        </button>
      {/if}
    </div>
  {/if}

  <div class="composer-actions-right">
    {#if busy}
      <button class="stop" title="Stop (Esc)" onclick={onAbort} type="button">
        <Icon icon={Square} size={11} />
      </button>
    {:else if shellMode}
      <button
        class="send shell-send"
        title="Run shell command (Enter)"
        onclick={onSend}
        disabled={!canSendShell}
        type="button"
      >
        <Icon icon={CornerDownLeft} size={12} />
        <span>Run</span>
      </button>
    {:else}
      <button
        class="send"
        title="Send (Enter)"
        onclick={onSend}
        disabled={!canSend}
        type="button"
      >
        <Icon icon={ArrowUp} size={15} strokeWidth={2.4} />
      </button>
    {/if}
  </div>
</div>
