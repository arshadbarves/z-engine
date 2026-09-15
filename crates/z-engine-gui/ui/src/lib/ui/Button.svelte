<script lang="ts">
  import type { HTMLButtonAttributes } from "svelte/elements";

  type Props = HTMLButtonAttributes & {
    variant?: "ghost" | "primary" | "accent" | "icon" | "secondary" | "outline" | "danger";
  };

  let { variant = "ghost", class: className = "", children, ...rest }: Props = $props();

  const variantClass = $derived(
    variant === "primary"
      ? "btn-primary"
      : variant === "accent"
        ? "btn-accent"
        : variant === "icon"
          ? "icon-btn"
          : variant === "secondary" || variant === "outline"
            ? "btn-secondary"
            : variant === "danger"
              ? "btn-danger"
              : "btn-ghost",
  );
</script>

<button type="button" class="{variantClass} {className}" {...rest}>{@render children?.()}</button>
