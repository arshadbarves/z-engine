<script lang="ts">
  import { untrack } from "svelte";
  import { openReleaseUrl } from "$lib/commands";
  import { credentialKey, keyHintText, keyStatus } from "$lib/domain/settings/credentials";
  import { formatKeyValues, parseKeyValues } from "$lib/domain/settings/formText";
  import { CACHE_OPTIONS, cacheChoice, cacheValue, PROVIDER_KIND_OPTIONS, type CacheChoice } from "$lib/domain/settings/options";
  import { connectFormError } from "$lib/domain/settings/providerWrites";
  import type { KeyStatus } from "$lib/protocol/config/KeyStatus";
  import type { ProviderKind } from "$lib/protocol/config/ProviderKind";
  import type { ProviderSettings } from "$lib/protocol/config/ProviderSettings";
  import { connectFormDefaults, requiresApiKey, type ConnectFormValues, type ProviderPreset } from "$lib/providers";
  import { scopeLabel } from "$lib/domain/settings/scopes";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { SegmentedChoice } from "$lib/ui";
  import Icon, { ChevronDown, ChevronRight, ExternalLink, Eye, KeyRound, LoaderCircle, Sparkles, X } from "$lib/ui/icons";
  import { cancelOnEscape } from "./escape";

  type Props = {
    provider: ProviderPreset;
    model: string;
    live: ProviderSettings;
    credentials: Record<string, KeyStatus>;
    onClose: () => void;
    /** Resolves to the error to show, or null once connected. */
    onSave: (values: ConnectFormValues, apiKey: string) => Promise<string | null>;
  };

  let { provider, model: liveModel, live, credentials, onClose, onSave }: Props = $props();

  const defaults = untrack(() => connectFormDefaults(provider, { model: liveModel, provider: live }));
  let apiKey = $state("");
  let model = $state(defaults.model);
  let baseUrl = $state(defaults.baseUrl);
  let kind = $state<ProviderKind>(defaults.kind);
  let headers = $state(formatKeyValues(defaults.headers, ":"));
  let cache = $state<CacheChoice>(cacheChoice(defaults.cacheControl));
  let showKey = $state(false);
  let showAdvanced = $state(untrack(() => provider.id === "custom"));
  let saving = $state(false);
  let error = $state<string | null>(null);

  const saved = $derived(keyStatus(credentials, credentialKey(baseUrl || provider.baseUrl)));
  const parsedHeaders = $derived(parseKeyValues(headers, ":"));
  const values = $derived<ConnectFormValues>({
    model: model.trim() || provider.defaultModel,
    baseUrl,
    kind,
    headers: parsedHeaders.values,
    cacheControl: cacheValue(cache),
  });
  const problem = $derived(parsedHeaders.error ?? connectFormError(provider, values, apiKey, saved.hasKey));
  const local = $derived(provider.tag === "Local");

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    if (problem) return;
    saving = true;
    error = await onSave(values, apiKey.trim());
    saving = false;
    if (!error) onClose();
  }

</script>

<div class="provider-modal-backdrop" role="presentation" {@attach cancelOnEscape(onClose)}>
  <button type="button" class="provider-modal-scrim" onclick={onClose} aria-label="Close dialog" tabindex="-1"></button>
  <div class="provider-modal" role="dialog" aria-modal="true" aria-labelledby="provider-modal-title">
    <div class="provider-modal-head">
      <div class="provider-modal-head-left">
        <span class="provider-badge-icon" style={`color: ${provider.color}`}><Icon icon={Sparkles} size={16} /></span>
        <div class="provider-modal-titles">
          <div class="provider-modal-title-row">
            <h3 id="provider-modal-title">Connect {provider.name}</h3>
            <span class="provider-tag-badge">{provider.tag}</span>
          </div>
          <p class="provider-modal-desc">{provider.desc}</p>
        </div>
      </div>
      <button type="button" class="provider-modal-close" onclick={onClose} aria-label="Close"><Icon icon={X} size={14} /></button>
    </div>

    <form class="provider-modal-form" onsubmit={handleSubmit}>
      {#if provider.keyUrl}
        <div class="provider-portal-banner">
          <div class="provider-portal-text">
            <span>{requiresApiKey(provider) ? `Need an API key for ${provider.name}?` : `Paid ${provider.name} models need a key`}</span>
            <small>
              {requiresApiKey(provider)
                ? "Create or copy one from your developer dashboard"
                : "Skip this for free models — connect directly with no key"}
            </small>
          </div>
          <button type="button" class="provider-portal-btn" onclick={() => provider.keyUrl && void openReleaseUrl(provider.keyUrl)}>
            <Icon icon={ExternalLink} size={12} />
            <span>Get API Key</span>
          </button>
        </div>
      {/if}

      {#if !local}
        {#if !requiresApiKey(provider)}
          <p class="provider-field-hint">Free models work with no key. Add a Zen key only if you want paid models.</p>
        {/if}
        <label class="provider-field-group">
          <div class="provider-field-label-row">
            <span class="provider-field-label">API Key</span>
            {#if saved.hasKey}<span class="provider-saved-hint">{keyHintText(saved)}</span>{/if}
          </div>
          <div class="provider-key-input-wrap">
            <input
              type={showKey ? "text" : "password"}
              bind:value={apiKey}
              placeholder={saved.hasKey ? "Leave blank to keep the saved key" : provider.keyPlaceholder}
              spellcheck={false}
              autocomplete="off"
            />
            <button type="button" class="provider-eye-btn" title={showKey ? "Hide key" : "Show key"} onclick={() => (showKey = !showKey)}>
              <Icon icon={showKey ? KeyRound : Eye} size={13} />
            </button>
          </div>
        </label>
      {/if}

      <label class="provider-field-group">
        <div class="provider-field-label-row">
          <span class="provider-field-label">Main model</span>
          {#if provider.defaultModel}
            <button type="button" class="provider-model-reset" onclick={() => (model = provider.defaultModel)}>
              Reset ({provider.defaultModel})
            </button>
          {/if}
        </div>
        <input bind:value={model} placeholder={provider.defaultModel || "The model id your server serves"} spellcheck={false} />
      </label>

      <div class="provider-advanced-section">
        <button type="button" class="provider-advanced-toggle" onclick={() => (showAdvanced = !showAdvanced)} aria-expanded={showAdvanced}>
          <Icon icon={showAdvanced ? ChevronDown : ChevronRight} size={12} />
          <span>Endpoint {provider.baseUrl ? `(${provider.baseUrl})` : "(custom)"}</span>
        </button>
        {#if showAdvanced}
          <label class="provider-field-group provider-advanced-field">
            <span class="provider-field-label">API base URL</span>
            <input bind:value={baseUrl} placeholder={provider.baseUrl || "https://..."} spellcheck={false} />
          </label>
          <div class="provider-field-group">
            <span class="provider-field-label">Wire format</span>
            <SegmentedChoice label="Wire format" options={PROVIDER_KIND_OPTIONS} value={kind} onSelect={(next) => (kind = next)} />
          </div>
          <label class="provider-field-group">
            <span class="provider-field-label">Extra headers</span>
            <textarea class="setting-textarea mono" rows="3" bind:value={headers} placeholder="Name: value" spellcheck={false}></textarea>
            <small class="provider-field-hint">One per line. Headers merge across settings files.</small>
          </label>
          <div class="provider-field-group">
            <span class="provider-field-label">Prompt caching</span>
            <SegmentedChoice label="Prompt caching" options={CACHE_OPTIONS} value={cache} onSelect={(next) => (cache = next)} />
          </div>
        {/if}
      </div>

      {#if error}
        <p class="setting-error" role="alert">{error}</p>
      {:else if problem}
        <p class="setting-note">{problem}</p>
      {/if}

      <div class="provider-modal-foot">
        <span class="provider-scope-hint">Saves to {scopeLabel(settingsStore.scope)} settings</span>
        <button type="button" class="provider-btn-secondary" onclick={onClose}>Cancel</button>
        <button type="submit" class="provider-btn-primary" disabled={saving || problem !== null}>
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
