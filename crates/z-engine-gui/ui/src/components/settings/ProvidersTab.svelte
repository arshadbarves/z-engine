<script lang="ts">
  import { saveApiKey } from "$lib/commands";
  import { credentialKey, keyHintText, keyStatus } from "$lib/domain/settings/credentials";
  import { PROVIDER_KIND_OPTIONS } from "$lib/domain/settings/options";
  import { connectWrites, normalizeBaseUrl } from "$lib/domain/settings/providerWrites";
  import type { Settings } from "$lib/protocol/config/Settings";
  import { detectProviderId, isProviderConnected, PROVIDERS, requiresApiKey, type ConnectFormValues, type ProviderPreset } from "$lib/providers";
  import { errorText, pushToast, sessions, setModel } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import Icon, { Check, KeyRound, Sparkles } from "$lib/ui/icons";
  import ProviderConnectModal from "./ProviderConnectModal.svelte";
  import SourceBadge from "./SourceBadge.svelte";

  type Props = { settings: Settings };
  let { settings }: Props = $props();

  let modalProvider = $state<ProviderPreset | null>(null);
  const activeId = $derived(detectProviderId(settings.provider.base_url));
  const activePreset = $derived(PROVIDERS.find((p) => p.id === activeId));

  /** The live endpoint for the active preset, else the preset's own; custom has none until active. */
  function endpointOf(p: ProviderPreset): string | null {
    return p.id === activeId ? settings.provider.base_url : p.baseUrl || null;
  }

  function bucketOf(p: ProviderPreset): string | null {
    const endpoint = endpointOf(p);
    return endpoint ? credentialKey(endpoint) : null;
  }

  const activeKey = $derived(keyStatus(settingsStore.credentials, credentialKey(settings.provider.base_url)));
  const kindLabel = $derived(PROVIDER_KIND_OPTIONS.find((o) => o.value === settings.provider.kind)?.label ?? settings.provider.kind);

  async function removeKey(p: ProviderPreset) {
    const baseUrl = endpointOf(p);
    if (!baseUrl) return;
    try {
      await saveApiKey(baseUrl, null);
      pushToast(requiresApiKey(p) ? `${p.name} key removed` : `${p.name} key removed — free models still work`, "info");
    } catch (e) {
      pushToast(`Could not remove the key · ${errorText(e)}`, "warn");
    }
    await settingsStore.loadCredentials();
  }

  async function connect(p: ProviderPreset, form: ConnectFormValues, apiKey: string): Promise<string | null> {
    if (apiKey) {
      try {
        await saveApiKey(normalizeBaseUrl(form.baseUrl), apiKey);
      } catch (e) {
        return `Could not save the key: ${errorText(e)}`;
      }
    }
    const error = await settingsStore.apply(connectWrites(form, settings.provider));
    await settingsStore.loadCredentials();
    if (error) return error;
    if (sessions.activeId && form.model.trim()) void setModel(form.model.trim());
    pushToast(`Connected to ${p.name}`, "ok");
    return null;
  }
</script>

<div class="tab-body providers-tab">
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>Providers</h3>
      <span class="settings-group-sub">Keys are stored once per API host in auth.json, never in settings files</span>
    </div>

    <div class="settings-card providers-table-card">
      {#each PROVIDERS as p (p.id)}
        {@const isCurrent = p.id === activeId}
        {@const key = keyStatus(settingsStore.credentials, bucketOf(p))}
        {@const isConnected = isProviderConnected(p, isCurrent, key.hasKey)}
        <div class={`provider-table-row${isConnected ? " is-connected" : ""}`}>
          <div class="provider-row-left">
            <span class="provider-row-icon" style={`color: ${p.color}`}>
              <Icon icon={Sparkles} size={15} />
            </span>
            <span class="provider-row-name">{p.name}</span>
            <span class="provider-tag-badge">{p.tag}</span>
            {#if isConnected}
              <span class="provider-active-pill"><Icon icon={Check} size={11} /><span>Active</span></span>
            {:else if isCurrent}
              <span class="provider-tag-badge provider-warn-badge">Active · needs a key</span>
            {/if}
            {#if key.hasKey}
              <span class="provider-key-badge" title="A key is stored for this API host">
                <Icon icon={KeyRound} size={10} /><span>{keyHintText(key)}</span>
              </span>
            {/if}
          </div>
          <div class="provider-row-right">
            <button
              type="button"
              class={`provider-action-btn ${isCurrent ? "configure" : "connect"}`}
              onclick={() => (modalProvider = p)}
            >
              {isCurrent ? "Configure" : "Connect"}
            </button>
            {#if key.hasKey}
              <button type="button" class="provider-action-btn disconnect" onclick={() => void removeKey(p)}>
                Remove key
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </section>

  <section class="settings-group">
    <div class="settings-card active-model-summary">
      <div class="active-model-info">
        <span class="active-model-label">{activePreset?.name ?? "Custom endpoint"}</span>
        <code class="active-model-val">{settings.model.main}</code>
        <SourceBadge source={settingsStore.sourceOf(["provider", "base_url"])} />
      </div>
      <span class="active-model-hint">
        {settings.provider.base_url} · {kindLabel} ·
        {activeKey.hasKey
          ? keyHintText(activeKey)
          : activePreset && !requiresApiKey(activePreset)
            ? "no key needed"
            : "no key stored"}
      </span>
    </div>
  </section>

  {#if modalProvider}
    {@const p = modalProvider}
    <ProviderConnectModal
      provider={p}
      model={settings.model.main}
      live={settings.provider}
      credentials={settingsStore.credentials}
      onClose={() => (modalProvider = null)}
      onSave={(values, apiKey) => connect(p, values, apiKey)}
    />
  {/if}
</div>
