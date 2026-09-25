<script lang="ts">
  import type { CompanionPose } from "$lib/domain/companion";
  import Icon, { BadgeCheck, Folder, Shield } from "$lib/ui/icons";
  import Companion from "../chrome/Companion.svelte";

  /** First impression: who this is, three promises, one button. */
  type Props = { onNext: () => void; onSkip: () => void };
  let { onNext, onSkip }: Props = $props();

  const GREETING: CompanionPose = { mood: "greeting", gaze: "center", particles: "sparkles", tone: "quiet" };
  const PROMISES = [
    { icon: Folder, title: "Works inside your project", text: "It reads, edits and runs your code where it lives." },
    { icon: Shield, title: "Asks before it changes things", text: "You approve edits and commands, or let it go on its own." },
    { icon: BadgeCheck, title: "Checks its own work", text: "It runs your tests and tells you whether they pass." },
  ];
</script>

<div class="welcome">
  <div class="welcome-orb" data-splash-target>
    <Companion pose={GREETING} progress={null} helpers={0} size={84} />
  </div>
  <h1 class="welcome-title">Meet Z Engine</h1>
  <p class="welcome-lead">An AI engineer that works inside your projects.</p>
  <ul class="welcome-promises">
    {#each PROMISES as item (item.title)}
      <li>
        <span class="welcome-icon" aria-hidden="true"><Icon icon={item.icon} size={16} strokeWidth={1.7} /></span>
        <span class="welcome-promise">
          <span class="welcome-promise-title">{item.title}</span>
          <span class="welcome-promise-text">{item.text}</span>
        </span>
      </li>
    {/each}
  </ul>
  <div class="welcome-actions">
    <button type="button" class="btn-accent onboarding-primary" onclick={onNext}>Get started</button>
    <button type="button" class="btn-ghost" onclick={onSkip}>Skip setup</button>
  </div>
</div>
