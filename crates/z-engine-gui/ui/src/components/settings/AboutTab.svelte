<script lang="ts">
  import type { HarnessConfig } from "$lib/commands";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { updateStore } from "$lib/updateStore";
  import Icon, {
    ArrowRight,
    CheckCircle2,
    Download,
    ExternalLink,
    LoaderCircle,
    RefreshCw,
    Sparkles,
  } from "$lib/ui/icons";
  import LogoMark from "../chrome/LogoMark.svelte";

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
</script>

<div class="tab-body about-tab">
  <div class="about-hero">
    <LogoMark size={44} />
    <div class="about-hero-text">
      <h3>Z Engine</h3>
      <p class="form-note">The Autonomous AI Coding Engine · v{cfg.version ?? "1.3.0"}</p>
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
            <span>Installing…</span>
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
          <span>Version {cfg.version ?? "1.3.0"} is the latest version available.</span>
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
