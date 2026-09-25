<script lang="ts">
  import type { CompanionPose } from "$lib/domain/companion";
  import { settingWrite } from "$lib/domain/settings/tomlValue";
  import type { CompanionLevel } from "$lib/protocol/config/CompanionLevel";
  import type { PermissionMode } from "$lib/protocol/PermissionMode";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { SegmentedChoice } from "$lib/ui";
  import Icon, { Eye, LoaderCircle, Pencil, Shield } from "$lib/ui/icons";
  import Companion from "../chrome/Companion.svelte";
  import StepLayout from "./StepLayout.svelte";

  /** How much the agent may do on its own, and how lively the companion is. */
  type Props = { onNext: () => void; onBack: () => void };
  let { onNext, onBack }: Props = $props();

  const MODES: Array<{ id: PermissionMode; title: string; desc: string; icon: typeof Shield }> = [
    { id: "default", title: "Ask before changes", desc: "It asks before editing files or running commands. The best place to start.", icon: Shield },
    { id: "acceptEdits", title: "Edit on its own", desc: "It edits files without asking, and still asks before running commands.", icon: Pencil },
    { id: "plan", title: "Plan first", desc: "It researches and proposes a plan. Nothing changes until you approve it.", icon: Eye },
  ];
  const LEVELS = [
    { value: "lively" as const, label: "Lively", description: "Reacts to the agent and to you" },
    { value: "calm" as const, label: "Calm", description: "Reacts only to the agent" },
    { value: "off" as const, label: "Off", description: "A small dot instead" },
  ];
  const PREVIEW: Record<Exclude<CompanionLevel, "off">, CompanionPose> = {
    lively: { mood: "happy", gaze: "center", particles: "sparkles", tone: "ok" },
    calm: { mood: "idle", gaze: "center", particles: "none", tone: "quiet" },
  };

  let mode = $state<PermissionMode>(settingsStore.settings?.permissions.mode ?? "default");
  let level = $state<CompanionLevel>(settingsStore.settings?.ui.companion ?? "lively");
  let saving = $state(false);
  let error = $state<string | null>(null);

  async function save() {
    saving = true;
    settingsStore.scope = "user";
    error = await settingsStore.apply([settingWrite(["permissions", "mode"], mode), settingWrite(["ui", "companion"], level)]);
    saving = false;
    if (!error) onNext();
  }
</script>

<StepLayout title="Decide how it works with you" lead="Both can change any time: the mode from the composer, the rest in Settings.">
  <div class="choice-list" role="radiogroup" aria-label="Permission mode">
    {#each MODES as item (item.id)}
      <button type="button" role="radio" aria-checked={mode === item.id} class="choice-card" class:is-selected={mode === item.id} onclick={() => (mode = item.id)}>
        <span class="choice-icon"><Icon icon={item.icon} size={16} /></span>
        <span class="choice-text">
          <span class="choice-title">{item.title}</span>
          <span class="choice-desc">{item.desc}</span>
        </span>
      </button>
    {/each}
  </div>

  <div class="companion-pick">
    <div class="companion-preview" aria-hidden="true">
      {#if level === "off"}
        <span class="companion-preview-dot"></span>
      {:else}
        <Companion pose={PREVIEW[level]} progress={null} helpers={0} size={44} />
      {/if}
    </div>
    <SegmentedChoice label="Companion" options={LEVELS} value={level} onSelect={(next) => (level = next)} />
  </div>

  {#snippet footer()}
    <button type="button" class="btn-ghost" onclick={onBack}>Back</button>
    {#if error}<p class="setting-error step-error" role="alert">{error}</p>{/if}
    <button type="button" class="btn-accent onboarding-primary" disabled={saving} onclick={() => void save()}>
      {#if saving}<Icon icon={LoaderCircle} size={13} class="spin" />{/if}
      <span>Continue</span>
    </button>
  {/snippet}
</StepLayout>
