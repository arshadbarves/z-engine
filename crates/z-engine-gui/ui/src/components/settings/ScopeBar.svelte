<script lang="ts">
  import { SCOPE_OPTIONS, type SettingsScope } from "$lib/domain/settings/scopes";
  import { layerError } from "$lib/domain/settings/provenance";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { SegmentedChoice } from "$lib/ui";

  const options = $derived(SCOPE_OPTIONS.filter((option) => settingsStore.scopes.includes(option.value)));
  const file = $derived(settingsStore.files[settingsStore.scope]);
  const skipped = $derived.by(() => {
    const provenance = settingsStore.provenance;
    return provenance ? layerError(provenance, settingsStore.scope) : null;
  });
  const selected = $derived(SCOPE_OPTIONS.find((option) => option.value === settingsStore.scope));
</script>

<div class="scope-bar">
  <div class="scope-bar-copy">
    <span class="scope-bar-title">Save changes to</span>
    <span class="scope-bar-desc">{selected?.description}</span>
    {#if file}
      <code class="scope-bar-path" title={file.path}>
        {file.path}{file.exists ? "" : " — created on the first change"}
      </code>
    {/if}
    {#if !settingsStore.root}
      <span class="scope-bar-desc">Open a workspace to edit its project and personal settings.</span>
    {/if}
    {#if skipped}
      <span class="scope-bar-error" role="alert">This file is not applied until it is fixed: {skipped}</span>
    {/if}
  </div>
  {#if options.length > 1}
    <SegmentedChoice
      label="Settings file to edit"
      {options}
      value={settingsStore.scope}
      onSelect={(scope: SettingsScope) => (settingsStore.scope = scope)}
    />
  {/if}
</div>
