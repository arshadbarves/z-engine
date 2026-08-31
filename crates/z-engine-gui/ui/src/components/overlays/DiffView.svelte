<script lang="ts">
  import { parseGitDiff } from "$lib/diffParse";

  type Props = { text: string };
  let { text }: Props = $props();

  function fileName(path: string): string {
    const i = path.lastIndexOf("/");
    return i >= 0 ? path.slice(i + 1) : path;
  }

  function dirName(path: string): string {
    const i = path.lastIndexOf("/");
    return i > 0 ? path.slice(0, i) : "";
  }

  function sign(kind: "add" | "del" | "ctx"): string {
    if (kind === "add") return "+";
    if (kind === "del") return "−";
    return "";
  }

  const d = $derived(parseGitDiff(text));
  const name = $derived(d.path ? fileName(d.path) : "diff");
  const dir = $derived(d.path ? dirName(d.path) : "");
</script>

<div class="diff-view">
  <div class="diff-filebar">
    <span class="diff-file-name">{name}</span>
    {#if dir}
      <span class="diff-file-dir">{dir}</span>
    {/if}
    <span class="diff-stat">
      {#if d.added > 0}<span class="add">+{d.added}</span>{/if}
      {#if d.deleted > 0}<span class="del">−{d.deleted}</span>{/if}
    </span>
  </div>
  <div class="diff-rows">
    {#each d.rows as r}
      {#if r.kind === "hunk"}
        <div class="diff-row hunk">
          {r.newNo != null ? `line ${r.newNo}` : "···"}
        </div>
      {:else}
        <div class="diff-row {r.kind}">
          <span class="diff-no">{r.oldNo ?? ""}</span>
          <span class="diff-no">{r.newNo ?? ""}</span>
          <span class="diff-sign">{sign(r.kind)}</span>
          <span class="diff-code">{r.text || "\u00a0"}</span>
        </div>
      {/if}
    {/each}
  </div>
</div>
