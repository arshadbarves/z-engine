<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";

  type Props = HTMLButtonAttributes & {
    variant?: "ghost" | "primary" | "accent" | "icon";
  };

  let { variant = "ghost", class: className = "", children, ...rest }: Props = $props();

  const variantClass =
    variant === "primary"
      ? "btn-primary"
      : variant === "accent"
        ? "btn-accent"
        : variant === "icon"
          ? "icon-btn"
          : "btn-ghost";
</script>

<button type="button" class="{variantClass} {className}" {...rest}>{@render children?.()}</button>
