<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    getConfig,
    saveGeneral,
    type HarnessConfig,
  } from "$lib/commands";
  import { configStore } from "$lib/configStore";
  import {
    DEFAULT_TASK_CONTINUATIONS, MAX_TASK_CONTINUATIONS, taskContinuationLimitError,
  } from "$lib/domain/configSupervision";
  import { pushToast } from "$lib/runtime";
  import Button from "$lib/ui/Button.svelte";
  import Icon, { Check } from "$lib/ui/icons";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsField from "./SettingsField.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";

  type Props = { cfg: HarnessConfig };
  let { cfg }: Props = $props();

  let review = $state(false);
  let maxCtx = $state("128000");
  let saved = $state(false);
  let maxContinuations = $state<number | undefined>(DEFAULT_TASK_CONTINUATIONS);
  let supervisionSaving = $state(false);
  let supervisionError = $state<string | null>(null);
  let savedResetTimer: ReturnType<typeof setTimeout> | undefined;
  let alive = true;

  onDestroy(() => {
    alive = false;
    if (savedResetTimer !== undefined) clearTimeout(savedResetTimer);
  });

  $effect(() => {
    review = Boolean(cfg.reviewEnabled);
    maxCtx = String(cfg.maxContextTokens ?? 128000);
    maxContinuations = cfg.maxTaskContinuations ?? DEFAULT_TASK_CONTINUATIONS;
  });

  async function handleSaveSupervision() {
    supervisionError = taskContinuationLimitError(maxContinuations);
    if (supervisionError || maxContinuations === undefined) return;
    supervisionSaving = true;
    let persisted = false;
    try {
      await saveGeneral({ maxTaskContinuations: maxContinuations });
      persisted = true;
      const next = await getConfig();
      if (!alive) return;
      configStore.set(next);
      pushToast("Task supervision updated. Applies to new sessions.", "info");
    } catch (error) {
      if (!alive) return;
      supervisionError = persisted
        ? `Settings were saved, but could not be reloaded: ${String(error)}`
        : `Could not save task supervision: ${String(error)}`;
      pushToast(supervisionError, "warn");
    } finally {
      if (alive) supervisionSaving = false;
    }
  }

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
    if (!alive) return;
    const next = await getConfig();
    if (!alive) return;
    configStore.set(next);
    saved = true;
    pushToast("Memory limits updated", "info");
    if (savedResetTimer !== undefined) clearTimeout(savedResetTimer);
    savedResetTimer = setTimeout(() => {
      saved = false;
      savedResetTimer = undefined;
    }, 1600);
  }
</script>

<div class="tab-body general-tab">
  <SettingsGroup title="Agent Behavior" description="Automated assistance and code inspection">
    <SettingsCard>
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
    </SettingsCard>
    <SettingsCard>
      <SettingsField
        title="Task Continuation Limit"
        description="Bounded continuation for supported code-changing tasks, using the same permissions."
        controlId="max-task-continuations"
      >
        <form
          class="supervision-form"
          onsubmit={(event) => { event.preventDefault(); void handleSaveSupervision(); }}
          aria-busy={supervisionSaving}
        >
          <p id="task-supervision-help" class="supervision-help">
            Default: 3. Set 0 to disable; maximum: 10. Changes apply to new sessions only.
            Existing sessions keep their current limit.
          </p>
          <div class="advanced-input-row">
            <input
              id="max-task-continuations"
              type="number"
              min="0"
              max={MAX_TASK_CONTINUATIONS}
              step="1"
              required
              bind:value={maxContinuations}
              disabled={supervisionSaving}
              aria-invalid={supervisionError !== null}
              aria-describedby={supervisionError
                ? "task-supervision-help task-supervision-error"
                : "task-supervision-help"}
              onblur={() => { supervisionError = taskContinuationLimitError(maxContinuations); }}
              oninput={() => { supervisionError = null; }}
            />
            <Button type="submit" variant="secondary" disabled={supervisionSaving}>
              {supervisionSaving ? "Saving..." : "Apply"}
            </Button>
          </div>
          {#if supervisionError}
            <p id="task-supervision-error" class="supervision-error" role="alert">{supervisionError}</p>
          {/if}
        </form>
      </SettingsField>
    </SettingsCard>
  </SettingsGroup>

  <SettingsGroup
    title="Memory & Context Limits"
    description="Manage context token boundaries before automatic compaction"
  >
    <SettingsCard>
      <SettingsField
        title="Max Context Window Tokens"
        description="Total tokens kept in conversation memory before triggering summarization (default: 128,000)"
        controlId="max-context-tokens"
      >
        <div class="advanced-input-row">
          <input id="max-context-tokens" type="number" bind:value={maxCtx} placeholder="128000" />
          <button type="button" class="advanced-save-btn" onclick={() => void handleSaveLimits()}>
            {#if saved}
              <Icon icon={Check} size={12} />
              <span>Saved</span>
            {:else}
              <span>Apply</span>
            {/if}
          </button>
        </div>

      </SettingsField>
    </SettingsCard>
  </SettingsGroup>
</div>

<style>
  .supervision-form { display: grid; gap: 10px; }
  .supervision-help, .supervision-error { margin: 0; font-size: 12px; line-height: 1.5; }
  .supervision-help { color: var(--text-2); }
  .supervision-error { color: var(--err); }
  #max-task-continuations:focus-visible {
    outline: 2px solid var(--border-strong);
    outline-offset: 2px;
  }
</style>
