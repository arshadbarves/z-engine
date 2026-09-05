<script lang="ts">
  import {
    getConfig,
    saveApiKey,
    saveGeneral,
    type HarnessConfig,
  } from "$lib/commands";
  import { configStore } from "$lib/configStore";
  import { detectProviderId, PROVIDERS, type ProviderPreset } from "$lib/providers";
  import { modelStore, pushToast } from "$lib/runtime";
  import Icon, { Check, Sparkles } from "$lib/ui/icons";
  import ProviderConnectModal from "./ProviderConnectModal.svelte";

  type Props = { cfg: HarnessConfig };
  let { cfg }: Props = $props();

  let selectedProviderId = $state("openrouter");
  let model = $state("");
  let baseUrl = $state("");
  let hasKey = $state(false);
  let hint = $state<string | null>(null);
  let modalProvider = $state<ProviderPreset | null>(null);

  $effect(() => {
    selectedProviderId = detectProviderId(cfg.baseUrl);
    model = cfg.model;
    baseUrl = cfg.baseUrl ?? "";
    hasKey = Boolean(cfg.hasApiKey);
    hint = cfg.apiKeyHint ?? null;
  });

  async function refresh() {
    const next = await getConfig();
    configStore.set(next);
    hasKey = Boolean(next.hasApiKey);
    hint = next.apiKeyHint ?? null;
    model = next.model;
    baseUrl = next.baseUrl ?? "";
    selectedProviderId = detectProviderId(next.baseUrl);
  }

  async function handleDisconnect(p: ProviderPreset) {
    if (p.id === selectedProviderId && hasKey) {
      await saveApiKey(null);
      pushToast(`${p.name} disconnected`, "info");
      await refresh();
    }
  }

  async function handleConnectModalSave(params: {
    apiKey: string;
    model: string;
    baseUrl: string;
  }) {
    if (params.apiKey) {
      await saveApiKey(params.apiKey);
    }
    await saveGeneral({
      model: params.model.trim() || null,
      baseUrl: params.baseUrl.trim() || null,
      maxContextTokens: cfg.maxContextTokens ?? null,
      review: cfg.reviewEnabled ?? null,
    });
    if (params.model.trim()) {
      modelStore.set(params.model.trim());
    }
    pushToast(`Connected to ${modalProvider?.name ?? "provider"}`, "info");
    await refresh();
  }
</script>

<div class="tab-body providers-tab">
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>Providers</h3>
      <span class="settings-group-sub">Connected providers</span>
    </div>

    <!-- Grouped Table Card Matching Reference Design -->
    <div class="settings-card providers-table-card">
      {#each PROVIDERS as p}
        {@const isCurrent = p.id === selectedProviderId}
        {@const isConnected = isCurrent && (hasKey || p.tag === "Local")}
        <div class={`provider-table-row${isConnected ? " is-connected" : ""}`}>
          <div class="provider-row-left">
            <span class="provider-row-icon" style={`color: ${p.color}`}>
              <Icon icon={Sparkles} size={15} />
            </span>
            <span class="provider-row-name">{p.name}</span>
            <span class="provider-tag-badge">{p.tag}</span>
            {#if isConnected}
              <span class="provider-active-pill">
                <Icon icon={Check} size={11} />
                <span>Active</span>
              </span>
            {/if}
          </div>

          <div class="provider-row-right">
            {#if isConnected}
              <button
                type="button"
                class="provider-action-btn configure"
                onclick={() => (modalProvider = p)}
              >
                Configure
              </button>
              {#if p.tag !== "Local"}
                <button
                  type="button"
                  class="provider-action-btn disconnect"
                  onclick={() => void handleDisconnect(p)}
                >
                  Disconnect
                </button>
              {/if}
            {:else}
              <button
                type="button"
                class="provider-action-btn connect"
                onclick={() => (modalProvider = p)}
              >
                Connect
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  </section>

  <!-- Active Model Status Card -->
  <section class="settings-group">
    <div class="settings-card active-model-summary">
      <div class="active-model-info">
        <span class="active-model-label">Current Active Model</span>
        <code class="active-model-val">{model || "openrouter/auto"}</code>
      </div>
      <span class="active-model-hint">
        {hasKey ? "API Key connected and ready" : "Connect an API key above to start chat sessions"}
      </span>
    </div>
  </section>

  <!-- Connect / Configure Modal -->
  {#if modalProvider}
    <ProviderConnectModal
      provider={modalProvider}
      currentModel={model}
      currentBaseUrl={baseUrl}
      {hasKey}
      keyHint={hint}
      onClose={() => (modalProvider = null)}
      onSave={handleConnectModalSave}
    />
  {/if}
</div>
