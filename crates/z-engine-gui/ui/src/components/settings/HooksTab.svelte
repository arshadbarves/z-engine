<script lang="ts">
  import { HOOK_EVENTS } from "$lib/domain/settings/hooks";
  import HookEventCard from "./HookEventCard.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import TrustNote from "./TrustNote.svelte";
</script>

<div class="tab-body hooks-tab">
  <SettingsGroup
    title="How hooks run"
    description="Shell commands the engine runs on agent events. Hooks from every settings file run, user first."
  >
    <SettingsCard class="hook-help">
      <p>
        The command gets JSON on stdin: <code>session_id</code>, <code>transcript_path</code>, <code>cwd</code>,
        <code>hook_event_name</code>, plus event fields such as <code>tool_name</code>, <code>tool_input</code>,
        <code>tool_response</code>, <code>prompt</code> and <code>stop_hook_active</code>.
      </p>
      <ul>
        <li><strong>Exit 0</strong> continues. For SessionStart and UserPromptSubmit, stdout becomes context.</li>
        <li><strong>Exit 2</strong> blocks, with stderr as the reason given to the model.</li>
        <li><strong>Other codes</strong> show a warning and continue.</li>
        <li>
          JSON on stdout may set <code>decision</code> (<code>block</code>/<code>approve</code>), <code>reason</code>,
          <code>continue: false</code> with <code>stopReason</code>, and <code>hookSpecificOutput</code>
          (<code>permissionDecision</code>, <code>updatedInput</code>, <code>additionalContext</code>).
        </li>
      </ul>
      <p>The matcher of PreToolUse and PostToolUse is a regular expression over tool names, such as <code>Bash|Edit</code>.</p>
    </SettingsCard>
  </SettingsGroup>

  <TrustNote what="hooks" />

  <SettingsGroup title="Events" description="Add hooks to the selected settings file; the order within a file is the run order.">
    <div class="hook-events">
      {#each HOOK_EVENTS as event (event.name)}
        <HookEventCard {event} />
      {/each}
    </div>
  </SettingsGroup>
</div>
