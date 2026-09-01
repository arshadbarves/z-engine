<script lang="ts">
  import { openReleaseUrl, type HarnessConfig } from "$lib/commands";
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

  const CHANGELOG_URL = "https://github.com/arshadbarves/z-engine/blob/release/CHANGELOG.md";

  type Props = { cfg: HarnessConfig };
  let { cfg }: Props = $props();

  const update = bindStore(updateStore);
  const info = $derived(update.current.info);
  const checking = $derived(update.current.checking);
  const installing = $derived(update.current.installing);
  const progress = $derived(update.current.progress);
  const pct = $derived(progress?.percentage != null ? Math.round(progress.percentage) : null);
  const isDownloading = $derived(installing && progress?.phase === "downloading");
  const isInstalling = $derived(
    installing && (progress?.phase === "installing" || progress?.phase === "ready"),
  );
  const displayVersion = $derived(cfg.version || info?.current || "1.4.1");
</script>

<div class="tab-body about-tab">
  <!-- Hero -->
  <div class="about-hero">
    <LogoMark size={44} />
    <div class="about-hero-text">
      <h3>Z Engine</h3>
      <p class="form-note">The Autonomous AI Coding Engine · v{displayVersion}</p>
    </div>
  </div>

  <!-- Update Section -->
  <div class="about-update-section">
    {#if info?.available}
      <div class="about-update-card has-update" role="status">
        <div class="about-update-row">
          <div class="about-update-icon-wrap pulse">
            <Icon icon={Sparkles} size={16} />
          </div>
          <div class="about-update-detail">
            <div class="about-update-headline">
              <span class="about-update-label">Update Available</span>
              <span class="about-update-ver-badge">
                <span class="ver-from">v{info.current}</span>
                <Icon icon={ArrowRight} size={9} class="ver-arrow-sm" />
                <span class="ver-to">v{info.latest}</span>
              </span>
            </div>
            {#if info.releaseNotes}
              <p class="about-update-summary">{info.releaseNotes}</p>
            {/if}
          </div>
        </div>

        <!-- Progress bar when downloading/installing -->
        {#if installing && pct != null}
          <div class="about-update-progress">
            <div class="about-progress-track">
              <div
                class="about-progress-fill"
                style="width: {pct}%"
                aria-valuenow={pct}
                aria-valuemin={0}
                aria-valuemax={100}
                role="progressbar"
              ></div>
            </div>
            <span class="about-progress-pct">{pct}%</span>
          </div>
        {/if}

        <div class="about-update-actions">
          <button
            type="button"
            class="about-btn-primary"
            disabled={installing}
            onclick={() => void updateStore.install()}
          >
            {#if isInstalling}
              <Icon icon={LoaderCircle} size={13} class="spin" />
              <span>Installing…</span>
            {:else if isDownloading}
              <Icon icon={LoaderCircle} size={13} class="spin" />
              <span>Downloading…</span>
            {:else}
              <Icon icon={Download} size={13} />
              <span>Update & Restart</span>
            {/if}
          </button>
          <button
            type="button"
            class="about-btn-ghost"
            title="Recheck latest release on GitHub"
            disabled={checking || installing}
            onclick={() => void updateStore.check(true)}
          >
            <Icon icon={RefreshCw} size={12} class={checking ? "spin" : undefined} />
          </button>
          {#if info.url}
            <button
              type="button"
              class="about-btn-ghost"
              title="View release on GitHub"
              onclick={() => updateStore.openRelease()}
            >
              <Icon icon={ExternalLink} size={12} />
            </button>
          {/if}
        </div>
      </div>
    {:else}
      <div class="about-update-card is-current" role="status">
        <div class="about-update-row">
          <div class="about-update-icon-wrap ok">
            <Icon icon={CheckCircle2} size={16} />
          </div>
          <div class="about-update-detail">
            <span class="about-update-label">Up to Date</span>
            <span class="about-uptodate-sub">v{displayVersion} is the latest version</span>
          </div>
        </div>
        <button
          type="button"
          class="about-btn-outline"
          disabled={checking}
          onclick={() => void updateStore.check(true)}
        >
          <Icon icon={RefreshCw} size={12} class={checking ? "spin" : undefined} />
          <span>{checking ? "Checking…" : "Check for Updates"}</span>
        </button>
      </div>
    {/if}
  </div>

  <div class="about-section-divider"></div>

  <!-- Links -->
  <div class="about-links-row">
    <button
      type="button"
      class="about-link-btn"
      onclick={() => void openReleaseUrl(CHANGELOG_URL)}
    >
      <Icon icon={FileText} size={13} />
      <span>View Changelog</span>
      <Icon icon={ExternalLink} size={10} class="about-link-ext" />
    </button>
    {#if info?.url}
      <button
        type="button"
        class="about-link-btn"
        onclick={() => updateStore.openRelease()}
      >
        <Icon icon={Sparkles} size={13} />
        <span>Latest Release</span>
        <Icon icon={ExternalLink} size={10} class="about-link-ext" />
      </button>
    {/if}
  </div>

  <div class="about-section-divider"></div>

  <!-- System Paths -->
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
