<script lang="ts">
  import { parseGitDiff } from "$lib/diffParse";
  import Icon, { Check, Copy, FileCode } from "$lib/ui/icons";

  type Props = {
    text: string;
    filePath?: string;
  };

  let { text, filePath = "" }: Props = $props();

  let copied = $state(false);
  let showAll = $state(false);
  const MAX_INITIAL_ROWS = 250;

  function fileName(path: string): string {
    const clean = path || filePath;
    const i = clean.lastIndexOf("/");
    return i >= 0 ? clean.slice(i + 1) : clean;
  }

  function dirName(path: string): string {
    const clean = path || filePath;
    const i = clean.lastIndexOf("/");
    return i > 0 ? clean.slice(0, i) : "";
  }

  function sign(kind: "add" | "del" | "ctx"): string {
    if (kind === "add") return "+";
    if (kind === "del") return "−";
    return " ";
  }

  const d = $derived(parseGitDiff(text));
  const name = $derived(d.path ? fileName(d.path) : fileName(filePath) || "diff");
  const dir = $derived(d.path ? dirName(d.path) : dirName(filePath));
  const visibleRows = $derived(showAll ? d.rows : d.rows.slice(0, MAX_INITIAL_ROWS));
  const remainingRowsCount = $derived(Math.max(0, d.rows.length - MAX_INITIAL_ROWS));

  async function copyDiff() {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      setTimeout(() => (copied = false), 1800);
    } catch {
      // ignore
    }
  }
</script>

<div class="diff-view-pro">
  <div class="diff-filebar-pro">
    <div class="diff-filebar-left">
      <Icon icon={FileCode} size={13} class="diff-view-icon" />
      {#if dir}
        <span class="diff-file-dir">{dir}/</span>
      {/if}
      <span class="diff-file-name">{name}</span>
    </div>
    <div class="diff-filebar-right">
      <span class="diff-stat-pill">
        {#if d.added > 0}<span class="add">+{d.added}</span>{/if}
        {#if d.deleted > 0}<span class="del">−{d.deleted}</span>{/if}
      </span>
      <button
        type="button"
        class={`diff-copy-btn${copied ? " copied" : ""}`}
        title="Copy diff text"
        onclick={copyDiff}
      >
        <Icon icon={copied ? Check : Copy} size={11} strokeWidth={1.8} />
        <span>{copied ? "Copied" : "Copy"}</span>
      </button>
    </div>
  </div>

  <div class="diff-rows-table">
    {#each visibleRows as r}
      {#if r.kind === "hunk"}
        <div class="diff-row-hunk">
          <span class="hunk-dots">···</span>
          <span class="hunk-badge">@@ {r.newNo != null ? `line ${r.newNo}` : "hunk"} @@</span>
        </div>
      {:else}
        <div class={`diff-row-line ${r.kind}`}>
          <span class="diff-line-no old">{r.oldNo ?? ""}</span>
          <span class="diff-line-no new">{r.newNo ?? ""}</span>
          <span class="diff-gutter-sign">{sign(r.kind)}</span>
          <span class="diff-code-text">{r.text || "\u00a0"}</span>
        </div>
      {/if}
    {/each}

    {#if remainingRowsCount > 0 && !showAll}
      <div class="diff-expand-more">
        <button type="button" class="btn-expand-diff" onclick={() => (showAll = true)}>
          Show remaining {remainingRowsCount} changes…
        </button>
      </div>
    {/if}
  </div>
</div>
