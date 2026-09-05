<script lang="ts">
  import { openReleaseUrl } from "$lib/commands";
  import type { ProviderPreset } from "$lib/providers";
  import { pushToast } from "$lib/runtime";
  import Icon, {
    ChevronDown,
    ChevronRight,
    ExternalLink,
    Eye,
    KeyRound,
    LoaderCircle,
    Sparkles,
    X,
  } from "$lib/ui/icons";

  type Props = {
    provider: ProviderPreset;
    currentModel?: string;
    currentBaseUrl?: string;
    hasKey: boolean;
    keyHint?: string | null;
    onClose: () => void;
    onSave: (params: { apiKey: string; model: string; baseUrl: string }) => Promise<void>;
  };

  let {
    provider,
    currentModel = "",
    currentBaseUrl = "",
    hasKey,
    keyHint = null,
    onClose,
    onSave,
  }: Props = $props();

  let apiKey = $state("");
  let model = $state("");
  let baseUrl = $state("");
  let showKey = $state(false);
  let showAdvanced = $state(false);
  let saving = $state(false);

  $effect(() => {
    model = currentModel || provider.defaultModel;
    baseUrl = currentBaseUrl || provider.baseUrl;
  });

  $effect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  async function handleOpenKeyPortal() {
    if (provider.keyUrl) {
      pushToast(`Opening ${provider.name} dashboard…`, "info");
      await openReleaseUrl(provider.keyUrl);
    }
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    try {
      await onSave({
        apiKey: apiKey.trim(),
        model: model.trim() || provider.defaultModel,
        baseUrl: baseUrl.trim() || provider.baseUrl,
      });
      onClose();
    } catch (err) {
      pushToast(`Failed to connect: ${String(err)}`, "error");
    } finally {
      saving = false;
    }
  }
</script>

<div class="provider-modal-backdrop" role="presentation">
  <button
    type="button"
    class="provider-modal-scrim"
    onclick={onClose}
    aria-label="Close dialog"
  ></button>

  <div
    class="provider-modal"
    role="dialog"
    aria-modal="true"
    aria-labelledby="provider-modal-title"
  >
    <!-- Modal Header -->
    <div class="provider-modal-head">
      <div class="provider-modal-head-left">
        <span class="provider-badge-icon" style={`color: ${provider.color}`}>
          <Icon icon={Sparkles} size={16} />
        </span>
        <div class="provider-modal-titles">
          <div class="provider-modal-title-row">
            <h3 id="provider-modal-title">Connect {provider.name}</h3>
            <span class="provider-tag-badge">{provider.tag}</span>
          </div>
          <p class="provider-modal-desc">{provider.desc}</p>
        </div>
      </div>
      <button
        type="button"
        class="provider-modal-close"
        onclick={onClose}
        aria-label="Close"
      >
        <Icon icon={X} size={14} />
      </button>
    </div>

    <!-- Modal Form -->
    <form class="provider-modal-form" onsubmit={handleSubmit}>
      {#if provider.keyUrl}
        <div class="provider-portal-banner">
          <div class="provider-portal-text">
            <span>Need an API key for {provider.name}?</span>
            <small>Create or copy one from your developer dashboard</small>
          </div>
          <button
            type="button"
            class="provider-portal-btn"
            onclick={() => void handleOpenKeyPortal()}
          >
            <Icon icon={ExternalLink} size={12} />
            <span>Get API Key</span>
          </button>
        </div>
      {/if}

      {#if provider.tag !== "Local"}
        <label class="provider-field-group">
          <div class="provider-field-label-row">
            <span class="provider-field-label">API Key</span>
            {#if hasKey && keyHint}
              <span class="provider-saved-hint">Saved (••••{keyHint})</span>
            {/if}
          </div>
          <div class="provider-key-input-wrap">
            <input
              type={showKey ? "text" : "password"}
              bind:value={apiKey}
              placeholder={hasKey ? "••••••••••••••••" : provider.keyPlaceholder}
              spellcheck={false}
              autocomplete="off"
            />
            <button
              type="button"
              class="provider-eye-btn"
              title={showKey ? "Hide key" : "Show key"}
              onclick={() => (showKey = !showKey)}
            >
              <Icon icon={showKey ? KeyRound : Eye} size={13} />
            </button>
          </div>
        </label>
      {/if}

      <label class="provider-field-group">
        <div class="provider-field-label-row">
          <span class="provider-field-label">Default Model</span>
          {#if provider.defaultModel}
            <button
              type="button"
              class="provider-model-reset"
              onclick={() => (model = provider.defaultModel)}
            >
              Reset ({provider.defaultModel})
            </button>
          {/if}
        </div>
        <input
          bind:value={model}
          placeholder={provider.defaultModel || "e.g. anthropic/claude-sonnet-4"}
          spellcheck={false}
        />
      </label>

      <!-- Advanced / Custom Endpoint -->
      <div class="provider-advanced-section">
        <button
          type="button"
          class="provider-advanced-toggle"
          onclick={() => (showAdvanced = !showAdvanced)}
          aria-expanded={showAdvanced}
        >
          <Icon icon={showAdvanced ? ChevronDown : ChevronRight} size={12} />
          <span>Endpoint URL {provider.baseUrl ? `(${provider.baseUrl})` : "(Custom)"}</span>
        </button>

        {#if showAdvanced}
          <label class="provider-field-group provider-advanced-field">
            <span class="provider-field-label">API Base URL</span>
            <input
              bind:value={baseUrl}
              placeholder={provider.baseUrl || "https://..."}
              spellcheck={false}
            />
          </label>
        {/if}
      </div>

      <!-- Modal Footer -->
      <div class="provider-modal-foot">
        <button type="button" class="provider-btn-secondary" onclick={onClose}>
          Cancel
        </button>
        <button
          type="submit"
          class="provider-btn-primary"
          disabled={saving || (provider.tag !== "Local" && !apiKey.trim() && !hasKey)}
        >
          {#if saving}
            <Icon icon={LoaderCircle} size={13} class="spin" />
            <span>Connecting…</span>
          {:else}
            <span>Connect & Set Active</span>
          {/if}
        </button>
      </div>
    </form>
  </div>
</div>
