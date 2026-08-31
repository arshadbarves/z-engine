<script lang="ts">
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { hideShell, shellStore } from "$lib/shellStore";
  import { Icon, X } from "$lib/ui/icons";

  const shell = bindStore(shellStore);
  let scroller: HTMLPreElement | undefined = $state();

  const visible = $derived(shell.current.visible);
  const entries = $derived(shell.current.entries);

  const last = $derived(entries[entries.length - 1]);
  const body = $derived(
    entries
      .slice(-6)
      .map((e) => (e.cmd ? `$ ${e.cmd}\n${e.lines.join("\n")}` : e.lines.join("\n")))
      .join("\n\n"),
  );

  $effect(() => {
    void entries;
    const el = scroller;
    if (el) el.scrollTop = el.scrollHeight;
  });
</script>

{#if visible && entries.length > 0 && last}
  <div class="term-panel" role="log" aria-label="Shell output">
    <div class="term-head">
      <span class="term-prompt">$</span>
      <span class="term-cmd" title={last.cmd}>{last.cmd || "shell"}</span>
      <button type="button" class="mini" title="Hide (Esc)" onclick={hideShell}>
        <Icon icon={X} size={12} />
      </button>
    </div>
    <pre bind:this={scroller} class="term-body">{body || "running…"}</pre>
  </div>
{/if}
