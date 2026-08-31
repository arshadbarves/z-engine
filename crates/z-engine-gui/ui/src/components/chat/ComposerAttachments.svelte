<script lang="ts">
  import Icon, { X } from "$lib/ui/icons";

  type Props = {
    attachments: string[];
    images: string[];
    onRemoveAttachment: (path: string) => void;
    onRemoveImage: (index: number) => void;
  };

  let { attachments, images, onRemoveAttachment, onRemoveImage }: Props = $props();

  function fileName(p: string): string {
    const i = p.lastIndexOf("/");
    return i >= 0 ? p.slice(i + 1) : p;
  }

  function extLabel(p: string): string {
    const n = fileName(p);
    const d = n.lastIndexOf(".");
    return d > 0 ? n.slice(d + 1).toUpperCase() : "FILE";
  }
</script>

{#if attachments.length > 0}
  <div class="attachments">
    {#each attachments as p}
      <span class="attachment">
        <button class="att-x" title={`Remove ${p}`} onclick={() => onRemoveAttachment(p)} type="button">
          <Icon icon={X} size={9} strokeWidth={2.4} />
        </button>
        <span class="att-icon">
          <svg
            viewBox="0 0 24 24"
            width={16}
            height={16}
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
            <path d="M14 2v6h6" />
            <path d="M9 13h6M9 17h4" />
          </svg>
        </span>
        <span class="att-text">
          <span class="att-name">{fileName(p)}</span>
          <span class="att-ext">{extLabel(p)}</span>
        </span>
      </span>
    {/each}
  </div>
{/if}

{#if images.length > 0}
  <div class="attachments img-chips">
    {#each images as url, i}
      <span class="attachment img-chip">
        <button class="att-x" title="Remove image" onclick={() => onRemoveImage(i)} type="button">
          <Icon icon={X} size={9} strokeWidth={2.4} />
        </button>
        <img src={url} alt={`paste ${i + 1}`} />
      </span>
    {/each}
  </div>
{/if}
