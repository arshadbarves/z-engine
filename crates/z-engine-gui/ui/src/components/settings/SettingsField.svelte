<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    title: string;
    description: string;
    controlId?: string;
    children: Snippet;
  };

  let { title, description, controlId, children }: Props = $props();
</script>

<div class="settings-field">
  <div class="settings-field-copy">
    {#if controlId}
      <label class="form-label-title" for={controlId}>{title}</label>
    {:else}
      <span class="form-label-title">{title}</span>
    {/if}
    <span class="form-label-desc">{description}</span>
  </div>
  {@render children()}
</div>

<style>
  .settings-field {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px;
  }

  .settings-field-copy {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .form-label-title {
    color: var(--text);
    font-size: 12.5px;
    font-weight: 600;
  }

  .form-label-desc {
    color: var(--text-3);
    font-size: 11.5px;
    line-height: 1.45;
  }

  :global(.settings-field input:not([type="checkbox"])) {
    width: 100%;
    padding: 9px 12px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-s);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    outline: none;
    transition: border-color var(--dur-fast) var(--ease-out);
  }

  :global(.settings-field input:not([type="checkbox"]):focus) {
    border-color: var(--border-strong);
  }
</style>
