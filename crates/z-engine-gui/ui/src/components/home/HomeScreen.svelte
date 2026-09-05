<script lang="ts">
  import { HERO_STARTERS } from "$lib/constants";
  import { modLabel } from "$lib/platform";
  import { draftStore } from "$lib/runtime";
  import Icon, {
    FolderGit2,
    Search,
    Sparkles,
    Workflow,
    Wrench,
  } from "$lib/ui/icons";
  import LogoMark from "../chrome/LogoMark.svelte";

  type Props = {
    projectName: string | null;
  };

  let { projectName }: Props = $props();

  const starterIcon = {
    Search,
    Sparkles,
    Wrench,
    Workflow,
  } as const;

  function pickStarter(prompt: string) {
    draftStore.set(prompt);
  }

  function appendPrefix(prefix: string) {
    draftStore.set(prefix);
  }
</script>

<div class="home-screen-wrap">
  <div class="home-hero">
    <div class="home-icon-halo" aria-hidden="true">
      <div class="home-halo-glow"></div>
      <LogoMark size={36} />
    </div>
    <div class="home-brand-badge">
      <LogoMark size={13} />
      <span class="home-brand-name">Z Engine</span>
      <span class="brand-beta-pill">BETA</span>
    </div>
    <h1 class="home-title">What should we build today?</h1>
    {#if projectName}
      <div class="home-ws-capsule" title={`Active workspace: ${projectName}`}>
        <Icon icon={FolderGit2} size={12} strokeWidth={1.8} />
        <span class="home-ws-name">{projectName}</span>
      </div>
    {:else}
      <p class="home-subtitle">
        Autonomous coding assistant with deep codebase awareness, verified tool execution, and live diffs.
      </p>
    {/if}
  </div>

  <div class="home-bento-grid">
    {#each HERO_STARTERS as card, index}
      <button
        type="button"
        class="home-bento-card"
        style={`--card-index: ${index}`}
        onclick={() => pickStarter(card.prompt)}
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

  <div class="home-shortcuts-deck">
    <button
      type="button"
      class="home-shortcut-tag"
      onclick={() => appendPrefix("@")}
      title="Reference project files (@)"
    >
      <kbd>@</kbd>
      <span>Files</span>
    </button>
    <button
      type="button"
      class="home-shortcut-tag"
      onclick={() => appendPrefix("/")}
      title="Run slash commands (/)"
    >
      <kbd>/</kbd>
      <span>Commands</span>
    </button>
    <button
      type="button"
      class="home-shortcut-tag"
      onclick={() => appendPrefix("!")}
      title="Execute bash command (!)"
    >
      <kbd>!</kbd>
      <span>Bash</span>
    </button>
    <div class="home-shortcut-divider" aria-hidden="true"></div>
    <span class="home-shortcut-hint">
      <kbd>{modLabel()}K</kbd> Search
    </span>
    <span class="home-shortcut-hint">
      <kbd>{modLabel()}N</kbd> New chat
    </span>
    <span class="home-shortcut-hint">
      <kbd>{modLabel()}B</kbd> Sidebar
    </span>
  </div>
</div>
