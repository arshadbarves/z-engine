<script lang="ts">
  import { transcriptStore } from "$lib/runtime/state";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import type { TaskReport } from "$lib/domain/taskReport";

  const messages = bindStore(transcriptStore);

  const latestTask = $derived.by(() => {
    const taskMsg = messages.current.findLast((m) => m.kind === "task");
    return (taskMsg?.taskReport as TaskReport | undefined) ?? null;
  });
</script>

<div class="deck-content active" id="deck-plan">
  <div class="plan-list">
    {#if latestTask}
      <div style="padding: 4px 6px; margin-bottom: 4px;">
        <div style="font-weight:600; font-size:12px; color:var(--text-primary);">{latestTask.goal}</div>
        <div style="font-size:10px; color:var(--text-dim);">Status: {latestTask.status}</div>
      </div>

      {#each latestTask.requirements as req}
        <div class="plan-item done">
          <div class="plan-check">✓</div>
          <div>
            <div style="font-weight:600;">{req.description}</div>
            <div style="font-size:10px; color:var(--text-dim);">Requirement {req.id}</div>
          </div>
        </div>
      {/each}

      {#each latestTask.checks as check}
        {@const passed = check.outcome === "passed"}
        <div class={`plan-item${passed ? " done" : " in-progress"}`}>
          <div class="plan-check">{passed ? "✓" : "●"}</div>
          <div>
            <div style="font-weight:600; color:{passed ? 'inherit' : 'var(--apple-blue)'};">
              {check.spec.kind}: {check.spec.package || "root"}
            </div>
            <div style="font-size:10px; color:var(--text-dim);">{check.summary || check.command.join(" ")}</div>
          </div>
        </div>
      {/each}

      {#each latestTask.blockers as blocker}
        <div class="plan-item">
          <div class="plan-check"></div>
          <div>
            <div style="font-weight:600; color:var(--apple-red);">{blocker}</div>
            <div style="font-size:10px; color:var(--text-dim);">Blocker pending resolution</div>
          </div>
        </div>
      {/each}
    {:else}
      <div class="plan-item done">
        <div class="plan-check">✓</div>
        <div>
          <div style="font-weight:600;">Scan Dependencies & AST</div>
          <div style="font-size:10px; color:var(--text-dim);">Resolved workspace dependencies and type signatures</div>
        </div>
      </div>

      <div class="plan-item done">
        <div class="plan-check">✓</div>
        <div>
          <div style="font-weight:600;">Patch Token Verification Method</div>
          <div style="font-size:10px; color:var(--text-dim);">Added fallback secret rotation handler</div>
        </div>
      </div>

      <div class="plan-item in-progress">
        <div class="plan-check">●</div>
        <div>
          <div style="font-weight:600; color:var(--apple-blue);">Execute Full Regression Suite</div>
          <div style="font-size:10px; color:var(--text-dim);">Running 48 integration tests in workspace</div>
        </div>
      </div>

      <div class="plan-item">
        <div class="plan-check"></div>
        <div>
          <div style="font-weight:600; color:var(--text-dim);">Commit Git Changes</div>
          <div style="font-size:10px; color:var(--text-dim);">Signed with agent verification evidence</div>
        </div>
      </div>
    {/if}
  </div>
</div>
