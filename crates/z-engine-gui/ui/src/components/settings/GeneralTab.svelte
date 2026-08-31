<script lang="ts">
  import {
    getConfig,
    openReleaseUrl,
    saveApiKey,
    saveGeneral,
    type HarnessConfig,
  } from "$lib/commands";
  import { configStore } from "$lib/configStore";
  import { detectProviderId, PROVIDERS } from "$lib/providers";
  import { modelStore, pushToast } from "$lib/runtime";
  import Icon, { Check, ChevronDown, ExternalLink, KeyRound, Sparkles } from "$lib/ui/icons";

  type Props = { cfg: HarnessConfig };
  let { cfg }: Props = $props();

  let selectedProviderId = $state(detectProviderId(cfg.baseUrl));
  let providerMenuOpen = $state(false);
  let model = $state(cfg.model);
  let baseUrl = $state(cfg.baseUrl ?? "");
  let maxCtx = $state(String(cfg.maxContextTokens));
  let review = $state(Boolean(cfg.reviewEnabled));
  let apiKey = $state("");
  let hasKey = $state(Boolean(cfg.hasApiKey));
  let hint = $state(cfg.apiKeyHint ?? null);
  let saved = $state(false);

  const activeProvider = $derived(
    PROVIDERS.find((p) => p.id === selectedProviderId) ?? PROVIDERS[0],
  );

  function handleProviderChange(id: string) {
    selectedProviderId = id;
    providerMenuOpen = false;
    const p = PROVIDERS.find((item) => item.id === id);
    if (!p) return;
    if (p.baseUrl) baseUrl = p.baseUrl;
    if (
      p.defaultModel &&
      (!model ||
        model === "openrouter/auto" ||
        model.startsWith("anthropic/") ||
        model.startsWith("openai/"))
    ) {
      model = p.defaultModel;
    }
  }

  async function handleConnect() {
    if (activeProvider.keyUrl) {
      pushToast(`Opening ${activeProvider.name} console…`, "info");
      await openReleaseUrl(activeProvider.keyUrl);
    }
  }

  async function refresh() {
    const next = await getConfig();
    configStore.set(next);
    hasKey = Boolean(next.hasApiKey);
    hint = next.apiKeyHint ?? null;
  }

  async function save() {
    await saveGeneral({
      model: model.trim() || null,
      baseUrl: baseUrl.trim() || null,
      maxContextTokens: Number(maxCtx) > 0 ? Number(maxCtx) : null,
      review,
    });
    if (apiKey.trim()) {
      await saveApiKey(apiKey.trim());
      apiKey = "";
    }
    if (model.trim()) modelStore.set(model.trim());
    await refresh();
    saved = true;
    setTimeout(() => {
      saved = false;
    }, 1600);
  }

  async function clearKey() {
    await saveApiKey(null);
    apiKey = "";
    await refresh();
  }
</script>

<div class="tab-body">
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>AI Provider & API Connection</h3>
      <span class="settings-group-sub">Choose your model provider and connect your API credentials</span>
    </div>
    <div class="settings-card">
      <div class="form-row custom-select-row">
        <div class="form-label-row">
          <span class="form-label-title">Provider</span>
          {#if activeProvider.keyUrl}
            <button
              type="button"
              class="provider-connect-btn"
              onclick={() => void handleConnect()}
              title={`Open ${activeProvider.name} API Keys portal in browser`}
            >
              <Icon icon={ExternalLink} size={12} />
              <span>Connect / Get API Key</span>
            </button>
          {/if}
        </div>
        <div class="custom-select-wrap">
          {#if providerMenuOpen}
            <div class="custom-select-backdrop" onclick={() => (providerMenuOpen = false)}></div>
          {/if}
          <button
            type="button"
            class={`custom-select-trigger${providerMenuOpen ? " active" : ""}`}
            onclick={() => (providerMenuOpen = !providerMenuOpen)}
            aria-haspopup="listbox"
            aria-expanded={providerMenuOpen}
          >
            <div class="custom-select-val">
              <span class="custom-select-name">{activeProvider.name}</span>
              <span class="custom-select-desc">{activeProvider.desc}</span>
            </div>
            <Icon
              icon={ChevronDown}
              size={14}
              class={`select-arrow${providerMenuOpen ? " open" : ""}`}
            />
          </button>
          {#if providerMenuOpen}
            <div class="custom-select-popover" role="listbox">
              <div class="custom-select-head">Select Model Provider</div>
              <div class="custom-select-list">
                {#each PROVIDERS as p}
                  {@const isSelected = p.id === selectedProviderId}
                  <button
                    type="button"
                    class={`custom-select-item${isSelected ? " selected" : ""}`}
                    role="option"
                    aria-selected={isSelected}
                    onclick={() => handleProviderChange(p.id)}
                  >
                    <div class="custom-select-item-text">
                      <span class="custom-select-item-name">{p.name}</span>
                      <span class="custom-select-item-desc">{p.desc}</span>
                    </div>
                    {#if isSelected}
                      <Icon icon={Check} size={14} class="custom-select-check" />
                    {/if}
                  </button>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      </div>

      <label class="form-row">
        <div class="form-label-row">
          <span class="form-label-title">API Key</span>
          {#if hasKey && hint}
            <span class="provider-status-badge ok">
              <Icon icon={KeyRound} size={11} />
              <span>Saved (••••{hint})</span>
            </span>
          {/if}
        </div>
        <span class="form-label-desc">
          {hasKey
            ? "Key is saved securely in ~/.config/z-engine/auth.json. Enter a new key below to replace it."
            : `Required for ${activeProvider.name}. Click "Connect / Get API Key" above to generate one.`}
        </span>
        <div class="form-input-with-action">
          <input
            type="password"
            bind:value={apiKey}
            spellcheck={false}
            autocomplete="off"
            placeholder={hasKey ? "••••••••••••••••" : activeProvider.keyPlaceholder}
          />
          {#if hasKey}
            <button class="ghost clear-btn" type="button" onclick={() => void clearKey()}>
              Clear key
            </button>
          {/if}
        </div>
      </label>

      <label class="form-row">
        <span class="form-label-title">Default Model</span>
        <span class="form-label-desc">Model used for new chat sessions</span>
        <input
          bind:value={model}
          spellcheck={false}
          placeholder={activeProvider.defaultModel || "e.g. anthropic/claude-sonnet-4"}
        />
      </label>
      <label class="form-row">
        <span class="form-label-title">Base URL</span>
        <span class="form-label-desc">API endpoint URL (OpenAI-compatible)</span>
        <input
          bind:value={baseUrl}
          spellcheck={false}
          placeholder={activeProvider.baseUrl || "https://api.openai.com/v1"}
        />
      </label>
    </div>
  </section>

  <section class="settings-group">
    <div class="settings-group-header">
      <h3>Agent & Context Engine</h3>
      <span class="settings-group-sub">Configure context token budgets and automated reviewer passes</span>
    </div>
    <div class="settings-card">
      <label class="form-row">
        <span class="form-label-title">Max Context Tokens</span>
        <span class="form-label-desc">Token window limit before intelligent auto-compaction</span>
        <input type="number" bind:value={maxCtx} placeholder="128000" />
      </label>
      <div class="form-row check">
        <div>
          <span class="form-label-title">Post-Edit Reviewer</span>
          <span class="form-label-desc">Automated fast reviewer pass after code file edits</span>
        </div>
        <label class="switch-toggle">
          <input type="checkbox" bind:checked={review} />
          <span class="switch-slider"></span>
        </label>
      </div>
    </div>
  </section>

  <p class="form-note">
    Model and limits are persisted to <code>.z-engine/config.toml</code>. API keys are safely encrypted in
    <code>~/.config/z-engine/auth.json</code> and loaded instantly into active sessions.
  </p>

  <div class="tab-actions">
    <button class="primary" onclick={() => void save()} type="button">
      {#if saved}
        <Icon icon={Check} size={13} />
        <span>Saved Settings</span>
      {:else}
        <Icon icon={Sparkles} size={13} />
        <span>Save Changes</span>
      {/if}
    </button>
  </div>
</div>
