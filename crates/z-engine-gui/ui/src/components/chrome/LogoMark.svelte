<script lang="ts">
  type Props = { size?: number; class?: string };
  let { size = 18, class: className }: Props = $props();
</script>

<svg
  class={className}
  width={size}
  height={size}
  viewBox="0 0 512 512"
  fill="none"
  aria-hidden="true"
>
  <defs>
    <linearGradient id="lm-bg" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#26262A" />
      <stop offset="50%" stop-color="#1B1B1E" />
      <stop offset="100%" stop-color="#101012" />
    </linearGradient>
    <linearGradient id="lm-rim" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#FFFFFF" stop-opacity="0.25" />
      <stop offset="15%" stop-color="#FFFFFF" stop-opacity="0.08" />
      <stop offset="85%" stop-color="#FFFFFF" stop-opacity="0.02" />
      <stop offset="100%" stop-color="#FFFFFF" stop-opacity="0.07" />
    </linearGradient>
    <linearGradient id="lm-titanium" x1="15%" y1="10%" x2="85%" y2="90%">
      <stop offset="0%" stop-color="#FFFFFF" />
      <stop offset="22%" stop-color="#ECECF2" />
      <stop offset="55%" stop-color="#C5C5D0" />
      <stop offset="85%" stop-color="#A5A5B2" />
      <stop offset="100%" stop-color="#8E8E9B" />
    </linearGradient>
    <linearGradient id="lm-edge" x1="20%" y1="0%" x2="80%" y2="100%">
      <stop offset="0%" stop-color="#FFFFFF" stop-opacity="0.8" />
      <stop offset="35%" stop-color="#FFFFFF" stop-opacity="0.2" />
      <stop offset="70%" stop-color="#FFFFFF" stop-opacity="0.05" />
      <stop offset="100%" stop-color="#FFFFFF" stop-opacity="0.35" />
    </linearGradient>
    <radialGradient id="lm-solar" cx="34%" cy="30%" r="70%">
      <stop offset="0%" stop-color="#FFB366" />
      <stop offset="30%" stop-color="#FF7322" />
      <stop offset="70%" stop-color="#EA4400" />
      <stop offset="95%" stop-color="#BE2800" />
      <stop offset="100%" stop-color="#9C1E00" />
    </radialGradient>
  </defs>

  <!-- Apple Standard Continuous Squircle Base -->
  <rect width="512" height="512" rx="116" fill="url(#lm-bg)" />
  <rect x="1" y="1" width="510" height="510" rx="115" fill="none" stroke="url(#lm-rim)" stroke-width="2" />

  <!-- Sculptural Liquid Titanium Z Mark -->
  <path
    d="M 144 110
      C 185 110, 225 110, 248 110
      C 266 110, 278 120, 280 136
      C 282 152, 275 168, 260 192
      C 236 228, 206 278, 184 318
      C 176 330, 184 338, 200 338
      L 364 338
      C 386 338, 404 354, 404 372
      C 404 391, 386 406, 364 406
      L 144 406
      C 122 406, 104 391, 104 372
      C 104 354, 114 340, 128 322
      C 152 290, 182 240, 204 200
      C 212 188, 204 180, 188 180
      L 144 180
      C 122 180, 104 165, 104 145
      C 104 125, 122 110, 144 110 Z"
    fill="url(#lm-titanium)"
  />
  <path
    d="M 144 110
      C 185 110, 225 110, 248 110
      C 266 110, 278 120, 280 136
      C 282 152, 275 168, 260 192
      C 236 228, 206 278, 184 318
      C 176 330, 184 338, 200 338
      L 364 338
      C 386 338, 404 354, 404 372
      C 404 391, 386 406, 364 406
      L 144 406
      C 122 406, 104 391, 104 372
      C 104 354, 114 340, 128 322
      C 152 290, 182 240, 204 200
      C 212 188, 204 180, 188 180
      L 144 180
      C 122 180, 104 165, 104 145
      C 104 125, 122 110, 144 110 Z"
    fill="none"
    stroke="url(#lm-edge)"
    stroke-width="1.5"
  />

  <!-- Radiant Solar Combustion Core -->
  <circle cx="356" cy="148" r="60" fill="url(#lm-solar)" />
  <circle cx="356" cy="148" r="59.5" fill="none" stroke="#FFB366" stroke-opacity="0.45" stroke-width="1" />
  <ellipse cx="340" cy="128" rx="19" ry="10" transform="rotate(-26 340 128)" fill="#FFFFFF" fill-opacity="0.38" />
</svg>
