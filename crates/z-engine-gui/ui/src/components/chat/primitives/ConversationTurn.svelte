<script lang="ts">
  import type { TurnBlock } from "$lib/activity";
  import type { Msg } from "$lib/types";
  import ApprovalCard from "../ApprovalCard.svelte";
  import TaskReportCard from "../TaskReportCard.svelte";
  import UserCard from "../UserCard.svelte";
  import AssistantMessage from "./AssistantMessage.svelte";
  import ToolActivityGroup from "./ToolActivityGroup.svelte";

  type Props = {
    turn: TurnBlock;
    onApprove: (message: Msg, decision: "once" | "session" | "persist") => void;
    onDeny: (message: Msg) => void;
  };

  let { turn, onApprove, onDeny }: Props = $props();
</script>

{#if turn.type === "user"}
  <UserCard m={turn.msg} />
{:else if turn.type === "approval"}
  <ApprovalCard
    m={turn.msg}
    onApprove={(decision) => onApprove(turn.msg, decision)}
    onDeny={() => onDeny(turn.msg)}
  />
{:else if turn.type === "assistant"}
  <AssistantMessage message={turn.msg} workItems={turn.workItems} />
{:else if turn.type === "work"}
  <div class="assistant-turn">
    <ToolActivityGroup items={turn.items} />
  </div>
{:else if turn.type === "error"}
  <div class="msg error">{turn.msg.text}</div>
{:else if turn.type === "task"}
  <TaskReportCard m={turn.msg} />
{:else if turn.type === "status"}
  <p class="response-status" role="status">{turn.msg.text}</p>
{/if}

<style>
  .response-status {
    margin: 0;
    color: var(--text-2);
    font-size: 12px;
  }
</style>
