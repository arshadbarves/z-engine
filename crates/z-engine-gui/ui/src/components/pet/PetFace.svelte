<script lang="ts">
  import type { PetFaceParts } from "$lib/domain/pet/emotions";

  /**
   * The pet's face in its 32×32 space: cheeks, brows, eyes, mouth, and a
   * sweat drop or tears. Ellipse eyes morph between shapes on a CSS
   * transition; drawn eyes (happy, closed, star, heart, spiral) pop in.
   * The eyes group moves with the gaze (--gx/--gy on the svg).
   */
  type Props = { face: PetFaceParts; blinking?: boolean };
  let { face, blinking = false }: Props = $props();

  const drawn = $derived(["happy", "closed", "star", "heart", "spiral"].includes(face.eyes));
  const HEART = "M0 1.8s-2-1.3-2-2.7c0-.8.6-1.3 1.2-1.3.4 0 .7.2.8.5.1-.3.4-.5.8-.5.6 0 1.2.5 1.2 1.3 0 1.4-2 2.7-2 2.7z";
  const STAR = "M0-2.3l.7 1.6 1.6.7-1.6.7-.7 1.6-.7-1.6-1.6-.7 1.6-.7z";
  const SPIRAL = "M0 0a.4.4 0 1 1 .8 0a.8.8 0 1 1-1.6 0a1.2 1.2 0 1 1 2.4 0a1.6 1.6 0 1 1-3.2 0";
</script>

<g class={`pet-face eyes-${face.eyes} brows-${face.brows} blush-${face.blush}`} class:is-blinking={blinking}>
  <ellipse class="pet-cheek" cx="9.3" cy="21.7" rx="1.9" ry="1.1" />
  <ellipse class="pet-cheek" cx="22.7" cy="21.7" rx="1.9" ry="1.1" />
  <path class="pet-brow is-left" d="M10.9 14.6h3" />
  <path class="pet-brow is-right" d="M18.1 14.6h3" />

  <g class="pet-eyes">
    {#if !drawn}
      <ellipse class="pet-eye is-left" cx="12.4" cy="18.4" rx="1.35" ry="2" />
      <ellipse class="pet-eye is-right" cx="19.6" cy="18.4" rx="1.35" ry="2" />
      {#if face.eyes === "puppy"}
        <circle class="pet-eye-glint" cx="12.9" cy="17.4" r="0.55" />
        <circle class="pet-eye-glint" cx="20.1" cy="17.4" r="0.55" />
      {/if}
    {:else if face.eyes === "happy"}
      <path class="pet-eye-line is-drawn" d="M11.1 19.1q1.3-1.9 2.6 0M18.3 19.1q1.3-1.9 2.6 0" />
    {:else if face.eyes === "closed"}
      <path class="pet-eye-line is-drawn" d="M11.1 18.3q1.3 1.3 2.6 0M18.3 18.3q1.3 1.3 2.6 0" />
    {:else if face.eyes === "star"}
      <g transform="translate(12.4 18.4)"><path class="pet-eye-star is-drawn" d={STAR} /></g>
      <g transform="translate(19.6 18.4)"><path class="pet-eye-star is-drawn is-late" d={STAR} /></g>
    {:else if face.eyes === "heart"}
      <g transform="translate(12.4 18.4)"><path class="pet-eye-heart is-drawn" d={HEART} /></g>
      <g transform="translate(19.6 18.4)"><path class="pet-eye-heart is-drawn is-late" d={HEART} /></g>
    {:else}
      <g transform="translate(12.4 18.4)"><path class="pet-eye-spiral is-drawn" d={SPIRAL} /></g>
      <g transform="translate(19.6 18.4)"><path class="pet-eye-spiral is-drawn is-late" d={SPIRAL} /></g>
    {/if}
  </g>

  {#key face.mouth}
    <g class="pet-mouth">
      {#if face.mouth === "smile"}
        <path class="mouth-line" d="M14.6 22.5q1.4 1.3 2.8 0" />
      {:else if face.mouth === "grin"}
        <path class="mouth-fill" d="M14.3 22.2h3.4q-.3 2.2-1.7 2.2t-1.7-2.2z" />
        <ellipse class="mouth-tongue" cx="16" cy="23.7" rx="0.8" ry="0.4" />
      {:else if face.mouth === "laugh"}
        <path class="mouth-fill" d="M13.9 21.9h4.2q-.3 3-2.1 3t-2.1-3z" />
        <ellipse class="mouth-tongue" cx="16" cy="24.1" rx="1" ry="0.5" />
      {:else if face.mouth === "o"}
        <ellipse class="mouth-fill" cx="16" cy="23" rx="0.85" ry="1" />
      {:else if face.mouth === "flat"}
        <path class="mouth-line" d="M15 23h2" />
      {:else if face.mouth === "frown"}
        <path class="mouth-line" d="M14.7 23.8q1.3-1.3 2.6 0" />
      {:else if face.mouth === "wobble"}
        <path class="mouth-line is-thin" d="M14.2 23.2q.45-.55.9 0t.9 0t.9 0t.9 0" />
      {:else if face.mouth === "yawn"}
        <ellipse class="mouth-fill mouth-yawn" cx="16" cy="23.3" rx="1.3" ry="1.8" />
      {:else if face.mouth === "tongue"}
        <path class="mouth-line" d="M14.6 22.4q1.4 1.2 2.8 0" />
        <path class="mouth-tongue" d="M16.8 22.9q.2 1.3.9 1.1t.2-1.2z" />
      {:else if face.mouth === "cat"}
        <path class="mouth-line" d="M14.1 22.4q.95 1.1 1.9 0q.95 1.1 1.9 0" />
      {:else if face.mouth === "smug"}
        <path class="mouth-line" d="M14.8 23q1.6.5 2.9-.9" />
      {:else if face.mouth === "talk"}
        <ellipse class="mouth-fill mouth-talk" cx="16" cy="23" rx="0.95" ry="0.9" />
      {/if}
    </g>
  {/key}

  {#if face.sweat}
    <path class="pet-sweat" d="M24.3 11.4c.8 1.1 1.2 1.8 1.2 2.4a1.2 1.2 0 0 1-2.4 0c0-.6.4-1.3 1.2-2.4z" />
  {/if}
  {#if face.tears}
    <path class="pet-tear" d="M11.3 20.4c.5.7.8 1.2.8 1.6a.8.8 0 0 1-1.6 0c0-.4.3-.9.8-1.6z" />
    <path class="pet-tear is-late" d="M20.7 20.4c.5.7.8 1.2.8 1.6a.8.8 0 0 1-1.6 0c0-.4.3-.9.8-1.6z" />
  {/if}
</g>
