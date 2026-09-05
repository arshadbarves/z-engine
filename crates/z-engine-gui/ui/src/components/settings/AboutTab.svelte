<script lang="ts">
  import { openReleaseUrl, type HarnessConfig } from "$lib/commands";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { updateStore } from "$lib/updateStore";
  import Icon, {
    ArrowRight,
    Check,
    CheckCircle2,
    Copy,
    Download,
    ExternalLink,
    FileText,
    Folder,
    KeyRound,
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

  let copiedPath = $state<string | null>(null);

  async function copyToClipboard(text: string, id: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedPath = id;
      setTimeout(() => {
        if (copiedPath === id) copiedPath = null;
      }, 1600);
    } catch {
      // ignore
    }
  }

  const PATHS = [
    {
      id: "global",
      label: "Global Configuration",
      path: "~/.config/z-engine/config.toml",
      desc: "Default settings and model preferences",
      icon: FileText,
    },
    {
      id: "auth",
      label: "Secure API Credentials",
      path: "~/.config/z-engine/auth.json",
      desc: "Encrypted API keys and provider tokens",
      icon: KeyRound,
    },
    {
      id: "project",
      label: "Workspace Configuration",
      path: ".z-engine/config.toml",
      desc: "Per-repository project overrides",
      icon: FileText,
    },
    {
      id: "sessions",
      label: "Local Session Storage",
      path: "~/Library/Application Support/z-engine/sessions",
      desc: "Chat history, checkpoints and diff snapshots",
      icon: Folder,
    },
  ];
</script>

<div class="tab-body about-tab">
  <!-- Hero -->
  <div class="about-hero">
    <div class="about-hero-logo">
      <LogoMark size={48} />
    </div>
    <div class="about-hero-text">
      <div class="about-hero-title-row">
        <h3>Z Engine</h3>
        <span class="about-hero-badge">v{displayVersion}</span>
      </div>
      <p class="about-hero-sub">Autonomous AI Coding Engine for macOS</p>
    </div>
  </div>

  <!-- Update Section -->
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>Software Updates</h3>
      <span class="settings-group-sub">Z Engine checks for new releases on launch</span>
    </div>

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
            title="Recheck latest release"
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
            <span class="about-uptodate-sub">Version {displayVersion} is the latest release</span>
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
  </section>

  <!-- System Paths -->
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>System Paths & Storage</h3>
      <span class="settings-group-sub">Local configuration and session history on your Mac</span>
    </div>

    <div class="settings-card paths-card">
      {#each PATHS as p}
        <div class="path-item-row">
          <div class="path-item-icon">
            <Icon icon={p.icon} size={14} />
          </div>
          <div class="path-item-copy">
            <div class="path-item-title-row">
              <span class="path-item-name">{p.label}</span>
              <span class="path-item-desc">{p.desc}</span>
            </div>
            <code class="path-item-code">{p.path}</code>
          </div>
          <button
            type="button"
            class={`path-copy-btn${copiedPath === p.id ? " is-copied" : ""}`}
            title={`Copy ${p.path}`}
            onclick={() => void copyToClipboard(p.path, p.id)}
          >
            {#if copiedPath === p.id}
              <Icon icon={Check} size={12} />
              <span>Copied</span>
            {:else}
              <Icon icon={Copy} size={12} />
              <span>Copy</span>
            {/if}
          </button>
        </div>
      {/each}
    </div>
  </section>

  <!-- External Links -->
  <div class="about-links-row">
    <button
      type="button"
      class="about-link-btn"
      onclick={() => void openReleaseUrl(CHANGELOG_URL)}
    >
      <Icon icon={FileText} size={13} />
      <span>View Release Notes</span>
      <Icon icon={ExternalLink} size={10} class="about-link-ext" />
    </button>
    {#if info?.url}
      <button
        type="button"
        class="about-link-btn"
        onclick={() => updateStore.openRelease()}
      >
        <Icon icon={Sparkles} size={13} />
        <span>GitHub Release</span>
        <Icon icon={ExternalLink} size={10} class="about-link-ext" />
      </button>
    {/if}
  </div>
</div>
