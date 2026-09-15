<script lang="ts">
  import { shellStore } from "$lib/shellStore";
  import { bindStore } from "$lib/svelte/bind.svelte";

  const shell = bindStore(shellStore);

  const hasEntries = $derived(shell.current.entries.length > 0);
  const latestEntry = $derived(
    hasEntries ? shell.current.entries[shell.current.entries.length - 1] : null,
  );
</script>

<div class="deck-content active" id="deck-terminal">
  <div class="term-container" id="terminal-stdout">
    {#if hasEntries}
      {#each shell.current.entries as entry (entry.id)}
        <div class="term-cmd">
          <span class="term-prompt">agent@macbook:~$</span>
          <span>{entry.cmd}</span>
        </div>
        {#if entry.lines.length > 0}
          <div class="term-out">
            {#each entry.lines as line}
              <div>{line}</div>
            {/each}
          </div>
        {/if}
      {/each}
      <div class="term-cmd">
        <span class="term-prompt">agent@macbook:~$</span>
        <span class="pulse-dot green" style="display:inline-block;" aria-hidden="true"></span>
      </div>
    {:else}
      <div class="term-cmd">
        <span class="term-prompt">agent@macbook:~$</span>
        <span>cargo test --workspace</span>
      </div>
      <div class="term-out">
        Running unittests src/lib.rs<br />
        test result: ok. 24 passed; 0 failed; 0 ignored<br /><br />
        Running unittests src/agent/turn.rs<br />
        test result: ok. 18 passed; 0 failed; 0 ignored<br /><br />
        All test suites passed.
      </div>
      <div class="term-cmd">
        <span class="term-prompt">agent@macbook:~$</span>
        <span class="pulse-dot green" style="display:inline-block;" aria-hidden="true"></span>
      </div>
    {/if}
  </div>
</div>
