<script lang="ts" module>
  import { AlertDialog } from "bits-ui";
</script>

<script lang="ts">
  /** A question that must be answered before something irreversible happens. */
  type Props = {
    open: boolean;
    title: string;
    description?: string;
    confirmLabel?: string;
    cancelLabel?: string;
    tone?: "danger" | "default";
    onConfirm: () => void;
    onCancel: () => void;
  };

  let {
    open,
    title,
    description,
    confirmLabel = "Confirm",
    cancelLabel = "Cancel",
    tone = "default",
    onConfirm,
    onCancel,
  }: Props = $props();
</script>

<AlertDialog.Root
  {open}
  onOpenChange={(next) => {
    if (!next) onCancel();
  }}
>
  <AlertDialog.Portal>
    <AlertDialog.Overlay class="modal-overlay" />
    <AlertDialog.Content class="modal confirm-dialog">
      <AlertDialog.Title class="confirm-title">{title}</AlertDialog.Title>
      {#if description}
        <AlertDialog.Description class="confirm-desc">{description}</AlertDialog.Description>
      {/if}
      <div class="confirm-actions">
        <AlertDialog.Cancel class="btn-secondary">{cancelLabel}</AlertDialog.Cancel>
        <AlertDialog.Action class={tone === "danger" ? "btn-danger is-solid" : "btn-accent"} onclick={onConfirm}>
          {confirmLabel}
        </AlertDialog.Action>
      </div>
    </AlertDialog.Content>
  </AlertDialog.Portal>
</AlertDialog.Root>
