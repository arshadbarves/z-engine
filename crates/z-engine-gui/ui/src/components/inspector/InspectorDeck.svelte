<script lang="ts">
  import Icon, { X } from "$lib/ui/icons";
  import DeckDiff from "./DeckDiff.svelte";
  import DeckPlan from "./DeckPlan.svelte";
  import DeckTerminal from "./DeckTerminal.svelte";

  type Tab = "diff" | "terminal" | "plan";

  type Props = {
    collapsed?: boolean;
    onClose?: () => void;
  };

  let { collapsed = false, onClose }: Props = $props();

  let currentTab = $state<Tab>("diff");
</script>

<aside id="inspector" class={collapsed ? "collapsed" : ""} aria-label="Inspector Deck">
  <div class="inspector-tabs">
    <div class="segmented" style="flex:1;">
      <button
        type="button"
        class={`seg-btn${currentTab === "diff" ? " active" : ""}`}
        style="flex:1;"
        onclick={() => (currentTab = "diff")}
      >
        Unified Diff
      </button>
      <button
        type="button"
        class={`seg-btn${currentTab === "terminal" ? " active" : ""}`}
        style="flex:1;"
        onclick={() => (currentTab = "terminal")}
      >
        Terminal
      </button>
      <button
        type="button"
        class={`seg-btn${currentTab === "plan" ? " active" : ""}`}
        style="flex:1;"
        onclick={() => (currentTab = "plan")}
      >
        Agent Plan
      </button>
    </div>

    {#if onClose}
      <button
        type="button"
        class="icon-btn-mini"
        title="Close Inspector"
        onclick={onClose}
        style="margin-left:4px;"
      >
        <Icon icon={X} size={13} />
      </button>
    {/if}
  </div>

  {#if !collapsed}
    {#if currentTab === "diff"}
      <DeckDiff />
    {:else if currentTab === "terminal"}
      <DeckTerminal />
    {:else if currentTab === "plan"}
      <DeckPlan />
    {/if}
  {/if}
</aside>
