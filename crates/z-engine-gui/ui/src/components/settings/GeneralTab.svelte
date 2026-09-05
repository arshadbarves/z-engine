<script lang="ts">
  import {
    getConfig,
    saveGeneral,
    type HarnessConfig,
  } from "$lib/commands";
  import { configStore } from "$lib/configStore";
  import { pushToast } from "$lib/runtime";
  import Icon, { Check } from "$lib/ui/icons";

  type Props = { cfg: HarnessConfig };
  let { cfg }: Props = $props();

  let review = $state(false);
  let maxCtx = $state("128000");
  let saved = $state(false);

  $effect(() => {
    review = Boolean(cfg.reviewEnabled);
    maxCtx = String(cfg.maxContextTokens ?? 128000);
  });

  async function handleToggleReview() {
    review = !review;
    await saveGeneral({
      model: cfg.model ?? null,
      baseUrl: cfg.baseUrl ?? null,
      maxContextTokens: Number(maxCtx) > 0 ? Number(maxCtx) : null,
      review,
    });
    const next = await getConfig();
    configStore.set(next);
  }

  async function handleSaveLimits() {
    await saveGeneral({
      model: cfg.model ?? null,
      baseUrl: cfg.baseUrl ?? null,
      maxContextTokens: Number(maxCtx) > 0 ? Number(maxCtx) : null,
      review,
    });
    const next = await getConfig();
    configStore.set(next);
    saved = true;
    pushToast("Memory limits updated", "info");
    setTimeout(() => {
      saved = false;
    }, 1600);
  }
</script>

<div class="tab-body general-tab">
  <!-- Agent Behavior & Review -->
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>Agent Behavior</h3>
      <span class="settings-group-sub">Automated assistance and code inspection</span>
    </div>

    <div class="settings-card">
      <div class="form-row check">
        <div class="check-text">
          <span class="form-label-title">Automated Code Review</span>
          <span class="form-label-desc">
            Runs a swift reviewer agent pass whenever code files are edited to flag syntax or logical regressions
          </span>
        </div>
        <label class="switch-toggle">
          <input type="checkbox" checked={review} onchange={() => void handleToggleReview()} />
          <span class="switch-slider"></span>
        </label>
      </div>
    </div>
  </section>

  <!-- Memory & Context Limits -->
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>Memory & Context Limits</h3>
      <span class="settings-group-sub">Manage context token boundaries before automatic compaction</span>
    </div>

    <div class="settings-card">
      <label class="form-row">
        <span class="form-label-title">Max Context Window Tokens</span>
        <span class="form-label-desc">
          Total tokens kept in conversation memory before triggering summarization (default: 128,000)
        </span>
        <div class="advanced-input-row">
          <input type="number" bind:value={maxCtx} placeholder="128000" />
          <button type="button" class="advanced-save-btn" onclick={() => void handleSaveLimits()}>
            {#if saved}
              <Icon icon={Check} size={12} />
              <span>Saved</span>
            {:else}
              <span>Apply</span>
            {/if}
          </button>
        </div>
      </label>
    </div>
  </section>
</div>
