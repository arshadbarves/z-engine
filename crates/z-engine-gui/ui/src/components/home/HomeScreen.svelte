<script lang="ts">
  import { HERO_STARTERS } from "$lib/constants";
  import { composer } from "$lib/stores/composer.svelte";
  import Icon, { Search, Sparkles, Workflow, Wrench } from "$lib/ui/icons";

  const starterIcon = { Search, Sparkles, Wrench, Workflow } as const;
</script>

<div class="home-screen-wrap">
  <div class="home-hero">
    <h1 class="home-title">What should we build today?</h1>
  </div>

  <div class="home-bento-grid">
    {#each HERO_STARTERS as card, index}
      <button
        type="button"
        class="home-bento-card"
        style={`--card-index: ${index}`}
        onclick={() => composer.setDraft(card.prompt)}
      >
        <div class="home-bento-icon-box">
          <Icon
            icon={starterIcon[card.iconName as keyof typeof starterIcon] ?? Sparkles}
            size={15}
            strokeWidth={1.8}
          />
        </div>
        <div class="home-bento-content">
          <span class="home-bento-title">{card.title}</span>
          <span class="home-bento-desc">{card.desc}</span>
        </div>
      </button>
    {/each}
  </div>
</div>
