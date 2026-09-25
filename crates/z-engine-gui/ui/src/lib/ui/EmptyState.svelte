<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon, { type IconSvgElement } from "./icons";

  /** What an empty place is for, and the one thing to do next. */
  type Props = {
    icon?: IconSvgElement;
    title: string;
    description?: string;
    class?: string;
    children?: Snippet;
  };

  let { icon, title, description, class: className = "", children }: Props = $props();
</script>

<div class={`empty-state ${className}`}>
  {#if icon}
    <span class="empty-state-icon" aria-hidden="true"><Icon {icon} size={18} strokeWidth={1.6} /></span>
  {/if}
  <p class="empty-state-title">{title}</p>
  {#if description}<p class="empty-state-desc">{description}</p>{/if}
  {#if children}<div class="empty-state-actions">{@render children()}</div>{/if}
</div>
