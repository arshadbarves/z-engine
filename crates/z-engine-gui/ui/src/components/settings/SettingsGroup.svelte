<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    title: string;
    description: string;
    children: Snippet;
  };

  let { title, description, children }: Props = $props();
</script>

<section class="settings-group">
  <div class="settings-group-header">
    <h3>{title}</h3>
    <span class="settings-group-sub">{description}</span>
  </div>
  {@render children()}
</section>

<style>
  .settings-group-header {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  h3 {
    margin: 0;
    color: var(--text);
    font-size: 13px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .settings-group-sub {
    color: var(--text-3);
    font-size: 11.5px;
    line-height: 1.4;
  }
</style>
