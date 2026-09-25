<script lang="ts" generics="T extends string">
  import { nextSegmentIndex } from "$lib/domain/segmentedChoice";

  type SegmentOption<T extends string> = {
    value: T;
    label: string;
    description: string;
  };

  type Props = {
    label: string;
    options: readonly SegmentOption<T>[];
    value: T;
    /** Persisting a choice must not disable the controls: a disabled button
     *  loses focus, which drops a keyboard reader out of the group. */
    busy?: boolean;
    /** Toolbar size: for panel headers and filters rather than forms. */
    compact?: boolean;
    onSelect: (value: T) => void;
  };

  let { label, options, value, busy = false, compact = false, onSelect }: Props = $props();
  let buttons: HTMLButtonElement[] = [];

  function activate(index: number) {
    const option = options[index];
    if (!option) return;
    // WebKit does not focus a button on click, so the group would otherwise
    // keep arrow-key focus on the previously checked option.
    buttons[index]?.focus();
    if (option.value === value) return;
    onSelect(option.value);
  }

  function moveSelection(event: KeyboardEvent, index: number) {
    const nextIndex = nextSegmentIndex(event.key, index, options.length);
    if (nextIndex === null) return;
    event.preventDefault();
    activate(nextIndex);
  }
</script>

<div class="segmented-choice" class:is-compact={compact} role="radiogroup" aria-label={label} aria-busy={busy}>
  {#each options as option, index (option.value)}
    <button
      bind:this={buttons[index]}
      type="button"
      role="radio"
      aria-checked={value === option.value}
      class:active={value === option.value}
      tabindex={value === option.value ? 0 : -1}
      title={option.description || undefined}
      onclick={() => activate(index)}
      onkeydown={(event) => moveSelection(event, index)}
    >
      <span>{option.label}</span>
    </button>
  {/each}
</div>

<style>
  /* Columns follow the option count so the primitive stays reusable. */
  .segmented-choice {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, 1fr);
    gap: 4px;
    padding: 4px;
    background: var(--raised-2);
    border: 1px solid var(--separator);
    border-radius: 10px;
  }

  button {
    min-height: 34px;
    padding: 6px 12px;
    border: 1px solid transparent;
    border-radius: 7px;
    background: transparent;
    color: var(--label-3);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition:
      color var(--dur-fast) var(--ease-out),
      background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }

  button:hover {
    color: var(--label);
    background: var(--fill-hover);
  }

  button.active {
    color: var(--label);
    background: var(--raised-3);
    border-color: var(--separator-strong);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.28);
  }

  button:focus-visible {
    outline: 2px solid var(--focus-ring);
    outline-offset: 1px;
  }

  .is-compact {
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
  }

  .is-compact button {
    min-height: 24px;
    padding: 2px 10px;
    font-size: 11.5px;
    font-weight: 550;
    border-radius: 6px;
  }

  .segmented-choice[aria-busy="true"] button {
    cursor: progress;
  }
</style>
