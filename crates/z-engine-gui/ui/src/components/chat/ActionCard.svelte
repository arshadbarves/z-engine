<script lang="ts">
  import { tailLines } from "$lib/runtime";
  import { familyTitle, groupSummary, pathPills, toolPath } from "$lib/toolGroups";
  import { fmtDur } from "$lib/toolUi";
  import type { Msg } from "$lib/types";
  import Icon, {
    ChevronRight,
    FilePenLine,
    FilePlus,
    FileText,
    Search,
    SquareTerminal,
    Workflow,
    Wrench,
    type IconSvgElement,
  } from "$lib/ui/icons";
  import ToolCard from "./ToolCard.svelte";

  const ICONS: Record<string, IconSvgElement> = {
    Read: FileText,
    Write: FilePlus,
    Edit: FilePenLine,
    Search,
    Bash: SquareTerminal,
    Task: Workflow,
  };

  type Props = { family: string; tools: Msg[] };
  let { family, tools }: Props = $props();

  let open = $state(false);
  const icon = $derived(ICONS[family] ?? Wrench);
  const pills = $derived(pathPills(tools.map(toolPath)));
  const hasBody = $derived(tools.some((t) => t.output) || pills.length > 0);
  const running = $derived(tools.some((t) => t.streaming));
  const failed = $derived(tools.some((t) => t.ok === false && !t.streaming));
  const dur = $derived(tools.reduce((n, t) => n + (t.durationMs ?? 0), 0));
</script>

<div class={`act-row${running ? " running" : ""}${failed ? " failed" : ""}`}>
  <button
    type="button"
    class="act-head"
    aria-expanded={open}
    onclick={() => hasBody && (open = !open)}
    disabled={!hasBody}
  >
    <span class="act-icon" aria-hidden="true">
      <Icon icon={icon} size={12} />
    </span>
    <span class="act-title">{familyTitle(family, tools.length)}</span>
    <span class="act-spacer"></span>
    {#if running && dur === 0}
      <span class="act-dur">…</span>
    {:else if dur > 0}
      <span class="act-dur">{fmtDur(dur)}</span>
    {/if}
    {#if hasBody}
      <span class={`act-chevron${open ? " open" : ""}`}>
        <Icon icon={ChevronRight} size={12} />
      </span>
    {/if}
  </button>
  {#if open}
    <div class="act-body">
      <div class="act-sub">{groupSummary(family, tools)}</div>
      {#if pills.length > 0}
        <div class="act-tags">
          {#each pills.slice(0, 8) as p}
            <span class="act-tag">{p.label} {p.count}</span>
          {/each}
        </div>
      {/if}
      {#each tools as m (m.id)}
        {#if m.output}
          {@const content = m.streaming ? tailLines(m.output).join("\n") : m.output}
          <div class="tool-output-wrap">
            <div class="tool-output-bar">
              <span class="tool-output-lines">{m.output.split("\n").length} lines</span>
            </div>
            <pre class={m.streaming ? "tool-tail" : "tool-full"}>{content}</pre>
          </div>
        {:else}
          <ToolCard {m} />
        {/if}
      {/each}
    </div>
  {/if}
</div>
