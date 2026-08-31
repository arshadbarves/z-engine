<script lang="ts">
  import type { SlashCommand } from "$lib/slash";

  type Props = {
    showSlash: boolean;
    slashMatches: SlashCommand[] | null;
    slashSel: number;
    onSelectSlash: (name: string) => void;
    onHoverSlash: (index: number) => void;
    showFiles: boolean;
    files: string[] | null;
    fileSel: number;
    onSelectFile: (path: string) => void;
    onHoverFile: (index: number) => void;
  };

  let {
    showSlash,
    slashMatches,
    slashSel,
    onSelectSlash,
    onHoverSlash,
    showFiles,
    files,
    fileSel,
    onSelectFile,
    onHoverFile,
  }: Props = $props();
</script>

{#if showSlash || showFiles}
  {#if showSlash && slashMatches}
    <div class="composer-pop" role="listbox" aria-label="Slash commands">
      {#each slashMatches as c, i}
        <button
          role="option"
          aria-selected={i === slashSel}
          class={`pop-item${i === slashSel ? " sel" : ""}`}
          onmouseenter={() => onHoverSlash(i)}
          onclick={() => onSelectSlash(c.name)}
        >
          <span class="pop-name">/{c.name}</span>
          <span class="pop-desc">{c.desc}</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if showFiles}
    <div class="composer-pop" role="listbox" aria-label="Matching project files">
      {#if files === null}
        <div class="pop-note">searching…</div>
      {:else if files.length === 0}
        <div class="pop-note">no matching files</div>
      {/if}
      {#each files ?? [] as f, i}
        <button
          role="option"
          aria-selected={i === fileSel}
          class={`pop-item mono${i === fileSel ? " sel" : ""}`}
          onmouseenter={() => onHoverFile(i)}
          onclick={() => onSelectFile(f)}
        >
          <span class="pop-name">{f}</span>
        </button>
      {/each}
    </div>
  {/if}
{/if}
