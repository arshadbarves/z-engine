<script lang="ts">
  import { lookupModel, type CatalogData } from "$lib/catalog";
  import { EFFORTS } from "$lib/domain/modes";
  import type { Effort } from "$lib/protocol/Effort";
  import { sessions, setEffort } from "$lib/runtime";
  import Icon, { Brain, ChevronDown } from "$lib/ui/icons";

  type Props = { catalog: CatalogData | null };
  let { catalog }: Props = $props();

  const effort: Effort | null = $derived(sessions.active?.effort ?? null);
  const model = $derived(sessions.active?.model ?? "");
  const show = $derived(Boolean(effort) || Boolean(lookupModel(catalog, model)?.model.reasoning));
  let open = $state(false);

  function pick(next: Effort | null) {
    open = false;
    if (next !== effort) void setEffort(next);
  }
</script>

{#if show}
  <div class="model-picker">
    {#if open}
      <button
        type="button"
        class="popover-backdrop"
        aria-label="Close reasoning effort menu"
        tabindex="-1"
        onclick={() => (open = false)}
      ></button>
    {/if}
    <button class="mode model-btn" onclick={() => (open = !open)} title="Reasoning effort">
      <Icon icon={Brain} size={11} />
      <span>{effort ?? "effort"}</span>
      <Icon icon={ChevronDown} size={9} strokeWidth={2.4} />
    </button>
    {#if open}
      <div class="popover" role="menu">
        <div class="popover-head">Reasoning effort</div>
        <div class="popover-current">{effort ?? "(model default)"}</div>
        {#if effort}
          <button class="popover-item" role="menuitem" onclick={() => pick(null)}>
            default
            <span class="popover-sub">Let the model decide</span>
          </button>
        {/if}
        {#each EFFORTS.filter((e) => e.id !== effort) as option (option.id)}
          <button class="popover-item" role="menuitem" onclick={() => pick(option.id)}>
            {option.id}
            <span class="popover-sub">{option.description}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
{/if}
