<script lang="ts">
  import { scopeLabel } from "$lib/domain/settings/scopes";
  import type { KeyStatus } from "$lib/protocol/config/KeyStatus";
  import type { ProviderSettings } from "$lib/protocol/config/ProviderSettings";
  import type { ConnectFormValues, ProviderPreset } from "$lib/providers";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { Dialog, DialogPanel } from "$lib/ui";
  import Icon, { Sparkles } from "$lib/ui/icons";
  import ProviderForm from "./ProviderForm.svelte";

  type Props = {
    provider: ProviderPreset;
    model: string;
    live: ProviderSettings;
    credentials: Record<string, KeyStatus>;
    onClose: () => void;
    /** Resolves to the error to show, or null once connected. */
    onSave: (values: ConnectFormValues, apiKey: string) => Promise<string | null>;
  };

  let { provider, model, live, credentials, onClose, onSave }: Props = $props();

  async function save(values: ConnectFormValues, apiKey: string): Promise<string | null> {
    const error = await onSave(values, apiKey);
    if (!error) onClose();
    return error;
  }
</script>

<Dialog.Root
  open={true}
  onOpenChange={(open) => {
    if (!open) onClose();
  }}
>
  <DialogPanel contentClass="provider-dialog">
    <header class="provider-dialog-head">
      <span class="provider-badge-icon" style={`color: ${provider.color}`}><Icon icon={Sparkles} size={16} /></span>
      <div class="provider-dialog-titles">
        <p class="provider-dialog-title">Connect {provider.name}</p>
        <p class="provider-dialog-desc">{provider.desc}</p>
      </div>
    </header>
    <ProviderForm
      {provider}
      {model}
      {live}
      {credentials}
      submitLabel="Connect and use"
      scopeHint={`Saves to ${scopeLabel(settingsStore.scope)} settings`}
      onCancel={onClose}
      onSave={save}
    />
  </DialogPanel>
</Dialog.Root>
