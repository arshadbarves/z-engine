<script lang="ts">
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { clearShell, hideShell, shellStore } from "$lib/shellStore";
  import Icon, { Check, Copy, Minimize2, Terminal, Trash2, X } from "$lib/ui/icons";

  const shell = bindStore(shellStore);
  let scroller: HTMLDivElement | undefined = $state();
  let copied = $state(false);
  let expanded = $state(false);

  const visible = $derived(shell.current.visible);
  const entries = $derived(shell.current.entries);
  const last = $derived(entries[entries.length - 1]);

  const fullText = $derived(
    entries
      .map((e) => (e.cmd ? `$ ${e.cmd}\n${e.lines.join("\n")}` : e.lines.join("\n")))
      .join("\n\n"),
  );

  const totalLines = $derived(
    entries.reduce((acc, e) => acc + (e.cmd ? 1 : 0) + e.lines.length, 0),
  );

  async function copyOutput() {
    if (!fullText) return;
    try {
      await navigator.clipboard.writeText(fullText);
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 1400);
    } catch (err) {
      console.error("Failed to copy terminal text", err);
    }
  }

  $effect(() => {
    void entries;
    const el = scroller;
    if (el) el.scrollTop = el.scrollHeight;
  });
</script>

{#if visible && entries.length > 0 && last}
  <div class={`term-panel${expanded ? " expanded" : ""}`} role="region" aria-label="Terminal output">
    <div class="term-head">
      <div class="term-head-left">
        <div class="term-status-badge">
          <span class="term-dot"></span>
          <Icon icon={Terminal} size={13} class="term-icon" />
          <span class="term-title">Terminal</span>
        </div>
        {#if last.cmd}
          <div class="term-cmd-chip" title={last.cmd}>
            <span class="term-prompt-glyph">❯</span>
            <span class="term-cmd-text">{last.cmd}</span>
          </div>
        {/if}
      </div>

      <div class="term-actions">
        <button
          type="button"
          class={`term-action-btn${copied ? " ok" : ""}`}
          title={copied ? "Copied to clipboard" : "Copy output"}
          onclick={() => void copyOutput()}
          aria-label="Copy terminal output"
        >
          {#if copied}
            <Icon icon={Check} size={12} strokeWidth={2} />
          {:else}
            <Icon icon={Copy} size={12} strokeWidth={1.8} />
          {/if}
        </button>

        <button
          type="button"
          class="term-action-btn"
          title={expanded ? "Compact view" : "Expand view"}
          onclick={() => (expanded = !expanded)}
          aria-label="Toggle terminal height"
        >
          <Icon icon={Minimize2} size={12} strokeWidth={1.8} />
        </button>

        <button
          type="button"
          class="term-action-btn"
          title="Clear terminal"
          onclick={clearShell}
          aria-label="Clear terminal output"
        >
          <Icon icon={Trash2} size={12} strokeWidth={1.8} />
        </button>

        <div class="term-action-divider"></div>

        <button
          type="button"
          class="term-action-btn term-close-btn"
          title="Hide terminal (Esc)"
          onclick={hideShell}
          aria-label="Hide terminal"
        >
          <Icon icon={X} size={13} strokeWidth={2} />
        </button>
      </div>
    </div>

    <div bind:this={scroller} class="term-body">
      {#each entries as entry (entry.id)}
        <div class="term-entry">
          {#if entry.cmd}
            <div class="term-entry-cmd">
              <span class="term-prompt-arrow">❯</span>
              <span class="term-entry-cmd-text">{entry.cmd}</span>
            </div>
          {/if}
          {#if entry.lines.length > 0}
            <pre class="term-entry-lines">{entry.lines.join("\n")}</pre>
          {/if}
        </div>
      {/each}
    </div>

    <div class="term-footer">
      <div class="term-footer-left">
        <span class="term-footer-stat">{entries.length} command{entries.length === 1 ? "" : "s"}</span>
        <span class="term-footer-sep">·</span>
        <span class="term-footer-stat">{totalLines} line{totalLines === 1 ? "" : "s"}</span>
      </div>
      <div class="term-footer-right">
        <span class="term-footer-hint"><kbd>Esc</kbd> hide</span>
      </div>
    </div>
  </div>
{/if}
