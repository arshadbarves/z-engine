<script lang="ts">
  import { todoProgress } from "$lib/domain/todos";
  import type { TodoItem } from "$lib/protocol/TodoItem";
  import Icon, { ChevronDown, ChevronRight, ListChecks } from "$lib/ui/icons";
  import TodoChecklist from "./TodoChecklist.svelte";

  type Props = { todos: TodoItem[] };
  let { todos }: Props = $props();

  let open = $state(false);
  const progress = $derived(todoProgress(todos));
  const pct = $derived(progress.total ? Math.round((progress.done / progress.total) * 100) : 0);
</script>

{#if progress.total > 0}
  <div class={`todo-strip${open ? " open" : ""}${progress.allDone ? " done" : ""}`}>
    <button
      type="button"
      class="todo-strip-head"
      aria-expanded={open}
      onclick={() => (open = !open)}
    >
      <Icon icon={ListChecks} size={12} class="todo-strip-icon" />
      <span class="todo-strip-count">{progress.done}/{progress.total}</span>
      <span class="todo-strip-bar" aria-hidden="true">
        <span class="todo-strip-fill" style={`width: ${pct}%`}></span>
      </span>
      <span class="todo-strip-current">
        {progress.allDone ? "All tasks done" : progress.currentLabel}
      </span>
      <Icon icon={open ? ChevronDown : ChevronRight} size={11} />
    </button>
    {#if open}
      <div class="todo-strip-body">
        <TodoChecklist {todos} />
      </div>
    {/if}
  </div>
{/if}
