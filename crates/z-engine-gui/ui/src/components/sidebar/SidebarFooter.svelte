<script lang="ts">
  import { modLabel } from "$lib/platform";
  import { detectProviderId, presetById, requiresApiKey } from "$lib/providers";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { Tooltip } from "$lib/ui";
  import Icon, { Settings } from "$lib/ui/icons";
  import { updateStore } from "$lib/updateStore";
  import { shortModel } from "$lib/util";

  /** Which model answers, whether it is ready, a waiting update, and Settings. */
  const mod = modLabel();
  const update = bindStore(updateStore);
  const provider = $derived(settingsStore.activeProvider);
  const preset = $derived(presetById(detectProviderId(provider?.baseUrl)));
  const model = $derived(shortModel(settingsStore.settings?.model.main ?? ""));
  const ready = $derived(!preset || !provider || provider.hasKey || !requiresApiKey(preset));
  const chipLabel = $derived(
    ready
      ? `${model || "No model"} via ${preset?.name ?? "custom provider"}. Change in Settings`
      : `${preset?.name ?? "The provider"} needs an API key. Connect in Settings`,
  );
</script>

<div class="sidebar-footer">
  <button
    type="button"
    class={`model-chip${ready ? "" : " needs-setup"}`}
    aria-label={chipLabel}
    title={chipLabel}
    onclick={() => ui.openSettings("providers")}
  >
    <span class="model-chip-dot" aria-hidden="true"></span>
    <span class="model-chip-text">
      <span class="model-chip-name">{ready ? model || "Choose a model" : "Connect a model"}</span>
      <span class="model-chip-provider">{preset?.name ?? "Custom provider"}</span>
    </span>
  </button>
  {#if update.current.info?.available}
    <button type="button" class="update-pill" onclick={() => ui.openSettings("about")}>Update</button>
  {/if}
  <Tooltip text="Settings" shortcut={`${mod},`} side="top">
    <button type="button" class="icon-btn" aria-label="Settings" onclick={() => ui.openSettings()}>
      <Icon icon={Settings} size={15} strokeWidth={1.8} />
    </button>
  </Tooltip>
</div>
