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
    onSelect: (value: T) => void;
  };

  let { label, options, value, busy = false, onSelect }: Props = $props();
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

<div class="segmented-choice" role="radiogroup" aria-label={label} aria-busy={busy}>
  {#each options as option, index (option.value)}
    <button
      bind:this={buttons[index]}
      type="button"
      role="radio"
      aria-checked={value === option.value}
      class:active={value === option.value}
      tabindex={value === option.value ? 0 : -1}
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
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 10px;
  }

  button {
    min-height: 34px;
    padding: 6px 12px;
    border: 1px solid transparent;
    border-radius: 7px;
    background: transparent;
    color: var(--text-3);
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
    color: var(--text);
    background: var(--hover-quiet);
  }

  button.active {
    color: var(--text);
    background: var(--surface-3);
    border-color: var(--border-strong);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.28);
  }

  button:focus-visible {
    outline: 2px solid var(--accent-ring);
    outline-offset: 1px;
  }

  .segmented-choice[aria-busy="true"] button {
    cursor: progress;
  }
</style>
