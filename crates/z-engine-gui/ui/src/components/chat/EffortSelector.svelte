<script lang="ts">
  import { lookupModel, type CatalogData } from "$lib/catalog";
  import { setReasoningEffort } from "$lib/commands";
  import { modelStore } from "$lib/runtime";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import Icon, { Brain, ChevronDown } from "$lib/ui/icons";

  type Props = { catalog: CatalogData | null };
  let { catalog }: Props = $props();

  const EFFORTS = ["low", "medium", "high", "xhigh"] as const;
  const model = bindStore(modelStore);
  let effort = $state<string | null>(null);
  let open = $state(false);

  const show = $derived(Boolean(effort || lookupModel(catalog, model.current || "")?.model.reasoning));

  async function pick(e: string | null) {
    open = false;
    effort = e;
    try {
      await setReasoningEffort(e);
    } catch (err) {
      console.error(err);
    }
  }
</script>

{#if show}
  <div class="model-picker">
    {#if open}
      <div class="popover-backdrop" onclick={() => (open = false)}></div>
    {/if}
    <button class="mode model-btn" onclick={() => (open = !open)} title="Reasoning effort">
      <Icon icon={Brain} size={11} />
      <span>{effort ?? "reason"}</span>
      <Icon icon={ChevronDown} size={9} strokeWidth={2.4} />
    </button>
    {#if open}
      <div class="popover" role="menu">
        <div class="popover-head">Reasoning effort</div>
        <div class="popover-current">{effort ?? "(provider default)"}</div>
        {#if effort}
          <button class="popover-item" role="menuitem" onclick={() => void pick(null)}>
            clear
            <span class="popover-sub">omit the parameter</span>
          </button>
        {/if}
        {#each EFFORTS.filter((e) => e !== effort) as e}
          <button class="popover-item" role="menuitem" onclick={() => void pick(e)}>
            {e}
            <span class="popover-sub">
              {e === "low"
                ? "fast and cheap"
                : e === "medium"
                  ? "balanced default"
                  : e === "high"
                    ? "thorough thinking"
                    : "maximum depth"}
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
{/if}
