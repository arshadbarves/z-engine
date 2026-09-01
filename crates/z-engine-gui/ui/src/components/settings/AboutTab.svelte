<script lang="ts">
  import { onMount } from "svelte";
  import { getChangelog, type HarnessConfig } from "$lib/commands";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { updateStore } from "$lib/updateStore";
  import Icon, {
    ArrowRight,
    CheckCircle2,
    Download,
    ExternalLink,
    FileText,
    LoaderCircle,
    RefreshCw,
    Sparkles,
  } from "$lib/ui/icons";
  import LogoMark from "../chrome/LogoMark.svelte";
  import Markdown from "../chat/Markdown.svelte";

  type Props = { cfg: HarnessConfig };
  let { cfg }: Props = $props();

  const update = bindStore(updateStore);
  const info = $derived(update.current.info);
  const checking = $derived(update.current.checking);
  const installing = $derived(update.current.installing);
  const progress = $derived(update.current.progress);
  const pct = $derived(progress?.percentage != null ? Math.round(progress.percentage) : null);
  const isInstalling = $derived(
    installing && (progress?.phase === "installing" || progress?.phase === "ready"),
  );

  let changelog = $state<string | null>(null);
  let loadingChangelog = $state(false);
  let showChangelog = $state(false);

  const displayVersion = $derived(cfg.version || info?.current || "1.4.1");

  async function loadChangelog() {
    if (changelog) return;
    loadingChangelog = true;
    try {
      changelog = await getChangelog();
    } catch {
      changelog = "# Changelog\n\nUnable to load changelog at this time.";
    } finally {
      loadingChangelog = false;
    }
  }

  onMount(() => {
    void loadChangelog();
  });
</script>

<div class="tab-body about-tab">
  <div class="about-hero">
    <LogoMark size={44} />
    <div class="about-hero-text">
      <h3>Z Engine</h3>
      <p class="form-note">The Autonomous AI Coding Engine · v{displayVersion}</p>
    </div>
  </div>

  {#if info?.available}
    <div class="settings-update-card" role="status">
      <div class="settings-update-head">
        <div class="settings-update-badge">
          <Icon icon={Sparkles} size={15} />
        </div>
        <div class="settings-update-info">
          <div class="settings-update-title">Update Available</div>
          <div class="settings-update-versions">
            <span class="ver-current">v{info.current}</span>
            <Icon icon={ArrowRight} size={11} class="ver-arrow" />
            <span class="ver-target">v{info.latest}</span>
          </div>
        </div>
        <button
          type="button"
          class="settings-update-primary-btn"
          disabled={installing}
          onclick={() => void updateStore.install()}
        >
          {#if installing}
            <Icon icon={LoaderCircle} size={13} class="spin" />
            <span>{isInstalling && pct != null ? `Installing… ${pct}%` : "Installing…"}</span>
          {:else}
            <Icon icon={Download} size={13} />
            <span>Update & Restart</span>
          {/if}
        </button>
      </div>
      {#if info.releaseNotes}
        <div class="settings-update-notes">
          <div class="settings-notes-label">What's New in v{info.latest}</div>
          <div class="settings-notes-content">{info.releaseNotes}</div>
        </div>
      {/if}
      {#if info.url}
        <div class="settings-update-footer">
          <button type="button" class="settings-github-link" onclick={() => updateStore.openRelease()}>
            <Icon icon={ExternalLink} size={12} />
            <span>View Release on GitHub</span>
          </button>
        </div>
      {/if}
    </div>
  {:else}
    <div class="settings-uptodate-card">
      <div class="uptodate-left">
        <Icon icon={CheckCircle2} size={16} class="uptodate-icon" />
        <div class="uptodate-text">
          <strong>Z Engine is up to date</strong>
          <span>Version {displayVersion} is the latest version available.</span>
        </div>
      </div>
      <button
        type="button"
        class="update-check"
        disabled={checking}
        onclick={() => void updateStore.check(true)}
      >
        <Icon icon={RefreshCw} size={12} class={checking ? "spin" : undefined} />
        {checking ? "Checking…" : "Check Now"}
      </button>
    </div>
  {/if}

  <div class="about-section-divider"></div>

  <!-- Changelog & Release Notes Section -->
  <div class="about-changelog-card">
    <div class="about-changelog-head">
      <div class="about-changelog-title-wrap">
        <Icon icon={FileText} size={15} class="about-changelog-icon" />
        <div>
          <h4 class="about-changelog-title">Release Notes & Changelog</h4>
          <p class="form-note">View what's new and recent updates across versions</p>
        </div>
      </div>
      <button
        type="button"
        class="btn-outline-small"
        onclick={() => (showChangelog = !showChangelog)}
      >
        {showChangelog ? "Hide Changelog" : "View Changelog"}
      </button>
    </div>

    {#if showChangelog}
      <div class="about-changelog-body">
        {#if loadingChangelog}
          <div class="about-changelog-loading">
            <Icon icon={LoaderCircle} size={14} class="spin" />
            <span>Fetching latest release notes…</span>
          </div>
        {:else if changelog}
          <div class="about-changelog-content">
            <Markdown text={changelog} />
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <div class="about-section-divider"></div>
  <h4 class="about-paths-title">System Paths & Configuration</h4>
  <dl class="about-dl">
    <dt>Global Config</dt>
    <dd>
      <code>~/.config/z-engine/config.toml</code>
      <span class="form-note"> created on first launch · API key in auth.json</span>
    </dd>
    <dt>Project Config</dt>
    <dd><code>.z-engine/config.toml</code></dd>
    <dt>Session Store</dt>
    <dd><code>Application Support/z-engine/sessions</code></dd>
    <dt>Active Model</dt>
    <dd><code>{cfg.model}</code></dd>
  </dl>
</div>
