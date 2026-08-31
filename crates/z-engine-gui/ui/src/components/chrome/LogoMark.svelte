<script lang="ts">
  type Props = { size?: number; class?: string };
  let { size = 18, class: className }: Props = $props();
</script>

<svg
  class={className}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  aria-hidden="true"
>
  <rect x="1.5" y="1.5" width="21" height="21" rx="6" stroke="currentColor" stroke-width="1.4" />
  <g fill="currentColor">
    <rect class="logo-bar logo-bar-top" x="5.53" y="5.53" width="12.94" height="3" rx="0.47" />
    <path class="logo-bar logo-bar-fold" d="M15.384 9.281h3.085L8.625 14.719H5.531z" />
    <rect class="logo-bar logo-bar-bot" x="5.53" y="15.47" width="12.94" height="3" rx="0.47" />
  </g>
</svg>
