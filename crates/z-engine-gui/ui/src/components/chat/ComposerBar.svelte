<script lang="ts">
  import { modLabel } from "$lib/platform";
  import Icon, { ArrowUp, CornerDownLeft, Square, Terminal } from "$lib/ui/icons";
  import ComposerPlusMenu from "./ComposerPlusMenu.svelte";
  import ModePicker from "./ModePicker.svelte";
  import ModelPicker from "./ModelPicker.svelte";

  /**
   * Under the draft: the + menu and the mode on the left; the model and Send
   * on the right. While a turn runs, Send becomes Stop and a typed message is
   * queued (Enter) or interrupts (⌘Enter).
   */
  type Props = {
    shellMode: boolean;
    busy: boolean;
    canSend: boolean;
    hasText: boolean;
    showTerminal: boolean;
    onAttach: () => void;
    onInsert: (prefix: string) => void;
    onShowShell: () => void;
    onSend: () => void;
    onInterrupt: () => void;
    onCancel: () => void;
  };

  let { shellMode, busy, canSend, hasText, showTerminal, onAttach, onInsert, onShowShell, onSend, onInterrupt, onCancel }: Props =
    $props();

  const mod = modLabel();
</script>

<div class="composer-bar">
  {#if shellMode}
    <div class="composer-controls-left">
      <span class="shell-mode-pill">
        <Icon icon={Terminal} size={11} />
        <span>Shell</span>
      </span>
      <span class="composer-hint">Runs in the project, without the agent · <kbd class="kbd">Esc</kbd> to leave</span>
    </div>
  {:else}
    <div class="composer-controls-left">
      <ComposerPlusMenu {showTerminal} {onAttach} {onInsert} onShowTerminal={onShowShell} />
      <ModePicker />
    </div>
  {/if}

  <div class="composer-actions-right">
    {#if busy && hasText && !shellMode}
      <span class="composer-hint">
        Enter queues ·
        <button type="button" class="composer-hint-btn" onclick={onInterrupt}>{mod}Enter interrupts</button>
      </span>
    {/if}
    {#if !shellMode}<ModelPicker />{/if}
    {#if busy}
      <button type="button" class="composer-send is-stop" title="Stop the turn (Esc)" aria-label="Stop the turn" onclick={onCancel}>
        <Icon icon={Square} size={11} />
      </button>
    {/if}
    {#if shellMode}
      <button type="button" class="composer-send is-run" title="Run (Enter)" onclick={onSend} disabled={!canSend}>
        <Icon icon={CornerDownLeft} size={12} />
        <span>Run</span>
      </button>
    {:else if !busy || hasText}
      <button
        type="button"
        class="composer-send"
        title={busy ? "Queue for the agent (Enter)" : "Send (Enter)"}
        aria-label={busy ? "Queue message" : "Send"}
        onclick={onSend}
        disabled={!canSend}
      >
        <Icon icon={ArrowUp} size={15} strokeWidth={2.4} />
      </button>
    {/if}
  </div>
</div>
