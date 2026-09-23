<script lang="ts">
  import { parseGitDiff } from "$lib/diffParse";
  import type { ToolCallState } from "$lib/domain/timeline/toolState";
  import { editDiff } from "$lib/domain/tools/editDiff";
  import { relPath, str } from "$lib/domain/tools/toolInput";
  import { FilePenLine, FilePlus } from "$lib/ui/icons";
  import DiffView from "../../overlays/DiffView.svelte";
  import ToolFrame from "./ToolFrame.svelte";
  import ToolOutput from "./ToolOutput.svelte";

  type Props = { call: ToolCallState; projectRoot: string | null };
  let { call, projectRoot }: Props = $props();

  const AUTO_OPEN_ROWS = 40;
  const path = $derived(relPath(str(call.input, "file_path", "notebook_path", "path"), projectRoot));
  const diff = $derived(editDiff(call.name, call.input, path));
  const stats = $derived(diff ? parseGitDiff(diff) : null);
  const failed = $derived(call.status === "error" || call.status === "denied");
  let toggled = $state<boolean | null>(null);
  const open = $derived(toggled ?? (failed || (stats !== null && stats.rows.length <= AUTO_OPEN_ROWS)));
  const extra = $derived(
    stats ? [stats.added ? `+${stats.added}` : "", stats.deleted ? `−${stats.deleted}` : ""].filter(Boolean).join(" ") : "",
  );
</script>

<ToolFrame
  {call}
  icon={call.name === "Write" ? FilePlus : FilePenLine}
  label={call.name}
  subject={path}
  {extra}
  expandable={Boolean(diff) || Boolean(call.output)}
  bind:open={() => open, (value) => (toggled = value)}
>
  {#if failed && call.output}
    <ToolOutput text={call.output} label="Result" />
  {/if}
  {#if diff}
    <DiffView text={diff} filePath={path} />
  {/if}
</ToolFrame>
