<script lang="ts">
  import Markdown from "svelte-exmarkdown";
  import { gfmPlugin } from "svelte-exmarkdown/gfm";
  import type { Snippet } from "svelte";
  import { highlightRoot } from "$lib/highlight";
  import Icon, { Check, Copy } from "$lib/ui/icons";

  type Props = { text: string };
  let { text }: Props = $props();

  const plugins = [gfmPlugin()];
  let copiedEl: HTMLElement | null = $state(null);
  let root: HTMLDivElement | undefined = $state();

  async function copyFrom(btn: HTMLButtonElement) {
    const pre = btn.closest(".code-block")?.querySelector("pre");
    const code = pre?.textContent ?? "";
    try {
      await navigator.clipboard.writeText(code);
      copiedEl = btn;
      setTimeout(() => {
        if (copiedEl === btn) copiedEl = null;
      }, 1400);
    } catch (e) {
      console.error("Failed to copy code", e);
    }
  }

  $effect(() => {
    void text;
    if (!root) return;
    queueMicrotask(() => {
      root?.querySelectorAll(".code-block").forEach((block) => {
        const langEl = block.querySelector("[data-lang-slot]");
        const cls = block.querySelector("code")?.className ?? "";
        const lang = cls.match(/language-([\w+-]+)/)?.[1];
        if (langEl && lang) langEl.textContent = lang;
      });
      highlightRoot(root);
    });
  });
</script>

<div class="md" bind:this={root}>
  <Markdown md={text} {plugins}>
    {#snippet pre({ children }: { children?: Snippet })}
      <div class="code-block">
        <div class="code-block-head">
          <div class="code-meta-left">
            <span class="code-lang" data-lang-slot>code</span>
          </div>
          <button
            type="button"
            class="code-copy-btn"
            onclick={(e) => void copyFrom(e.currentTarget)}
            title="Copy code to clipboard"
          >
            {#if copiedEl}
              <Icon icon={Check} size={11} class="copy-ok" />
              <span>Copied</span>
            {:else}
              <Icon icon={Copy} size={11} />
              <span>Copy</span>
            {/if}
          </button>
        </div>
        <pre>{#if children}{@render children()}{/if}</pre>
      </div>
    {/snippet}
    {#snippet code({ children, class: className }: { children?: Snippet; class?: string })}
      <code class={className}>{#if children}{@render children()}{/if}</code>
    {/snippet}
  </Markdown>
</div>
