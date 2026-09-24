<script lang="ts">
  import { Tooltip } from "$lib/ui";
  import Icon, { type IconSvgElement } from "$lib/ui/icons";

  type Props = {
    label: string;
    icon: IconSvgElement;
    shortcut?: string;
    /** Toggle buttons report their state; plain actions leave it undefined. */
    pressed?: boolean;
    dot?: boolean;
    onclick: () => void;
  };
  let { label, icon, shortcut, pressed, dot = false, onclick }: Props = $props();
</script>

<Tooltip text={label} {shortcut}>
  <button
    type="button"
    class={`titlebar-btn${pressed ? " is-active" : ""}`}
    aria-label={label}
    aria-pressed={pressed}
    {onclick}
  >
    <Icon {icon} size={15} strokeWidth={1.8} />
    {#if dot}<span class="titlebar-dot" aria-hidden="true"></span>{/if}
  </button>
</Tooltip>
