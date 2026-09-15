<script lang="ts">
  import {
    getConfig,
    saveGeneral,
    type HarnessConfig,
    type TaskReportView,
  } from "$lib/commands";
  import {
    APPEARANCE_OPTIONS,
    beginTaskReportViewUpdate,
    mergeTaskReportView,
    resolveTaskReportViewRecovery,
    type CompensationOutcome,
  } from "$lib/domain/appearanceSettings";
  import { configStore } from "$lib/configStore";
  import { pushToast } from "$lib/runtime";
  import { SegmentedChoice } from "$lib/ui";
  import AppearancePreview from "./AppearancePreview.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsField from "./SettingsField.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";

  type Props = { cfg: HarnessConfig };
  let { cfg }: Props = $props();

  let current = $state<TaskReportView>("quiet");
  let saving = $state(false);
  const selected = $derived(
    APPEARANCE_OPTIONS.find((option) => option.value === current) ?? APPEARANCE_OPTIONS[0],
  );

  $effect(() => {
    current = cfg.taskReportView;
  });

  function applyTaskReportView(taskReportView: TaskReportView) {
    current = taskReportView;
    configStore.set(
      mergeTaskReportView(configStore.getSnapshot(), cfg, taskReportView),
    );
  }

  function viewLabel(view: TaskReportView) {
    return APPEARANCE_OPTIONS.find((option) => option.value === view)?.label ?? view;
  }

  async function recoverFailedUpdate(
    update: ReturnType<typeof beginTaskReportViewUpdate>,
    persisted: boolean,
    updateError: unknown,
  ) {
    let compensation: CompensationOutcome = "not-needed";
    let compensationError: unknown;
    let reconciliationError: unknown;
    let reconciled: HarnessConfig | null = null;

    if (persisted) {
      try {
        await saveGeneral({ taskReportView: update.previous.taskReportView });
        compensation = "succeeded";
      } catch (error) {
        compensation = "failed";
        compensationError = error;
      }
    }

    try {
      reconciled = await getConfig();
    } catch (error) {
      reconciliationError = error;
    }

    const recovery = resolveTaskReportViewRecovery(update, {
      initialSavePersisted: persisted,
      compensation,
      reconciled,
    });
    applyTaskReportView(recovery.config.taskReportView);

    if (compensation === "failed") {
      const confirmation = recovery.durabilityConfirmed
        ? `Reloaded saved setting: ${viewLabel(recovery.config.taskReportView)}.`
        : `Durable rollback could not be confirmed; showing ${viewLabel(recovery.config.taskReportView)} as the best known saved setting.`;
      pushToast(
        `Report detail update failed. Rollback also failed: ${String(compensationError)}. ${confirmation}`,
        "warn",
      );
      return;
    }
    if (!recovery.durabilityConfirmed) {
      pushToast(
        `Report detail update failed. Durable rollback could not be confirmed (${String(reconciliationError)}); showing ${viewLabel(recovery.config.taskReportView)} as the best known saved setting.`,
        "warn",
      );
      return;
    }
    if (!recovery.restoredPrevious) {
      pushToast(
        `Report detail update reported an error; reloaded saved setting: ${viewLabel(recovery.config.taskReportView)}.`,
        "warn",
      );
      return;
    }
    pushToast(`Could not update report detail: ${String(updateError)}`, "warn");
  }

  async function selectView(next: TaskReportView) {
    if (saving || next === current) return;
    const update = beginTaskReportViewUpdate(
      configStore.getSnapshot() ?? cfg,
      next,
    );
    current = next;
    configStore.set(update.optimistic);
    saving = true;
    let persisted = false;

    try {
      await saveGeneral({ taskReportView: next });
      persisted = true;
      const refreshed = await getConfig();
      applyTaskReportView(refreshed.taskReportView);
    } catch (error) {
      await recoverFailedUpdate(update, persisted, error);
    } finally {
      saving = false;
    }
  }
</script>

<div class="tab-body appearance-tab">
  <SettingsGroup
    title="Task report detail"
    description="Choose how much verification information completed tasks show"
  >
    <SettingsCard>
      <SettingsField
        title="Information density"
        description="This changes report presentation everywhere. It does not change how the agent works or what it verifies."
      >
        <SegmentedChoice
          label="Task report information density"
          options={APPEARANCE_OPTIONS}
          value={current}
          busy={saving}
          onSelect={(value) => void selectView(value)}
        />
        <p class="selection-description" aria-live="polite">{selected.description}</p>
      </SettingsField>
    </SettingsCard>
  </SettingsGroup>

  <SettingsGroup
    title="Preview"
    description="An illustration of the layout only — no task or session data is used"
  >
    <AppearancePreview view={current} label={selected.label} />
  </SettingsGroup>
</div>

<style>
  .selection-description {
    min-height: 36px;
    margin: 0;
    color: var(--text-2);
    font-size: 12px;
    line-height: 1.5;
  }
</style>
