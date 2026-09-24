<script lang="ts">
  import type { SessionListItem } from "$lib/domain/sessionList";
  import type { SessionActivity, UnreadMark } from "$lib/domain/sessions";
  import { modLabel } from "$lib/platform";
  import { Tooltip } from "$lib/ui";
  import Icon, { Plus, Search } from "$lib/ui/icons";
  import Sidebar from "./Sidebar.svelte";

  type Props = {
    sessions: SessionListItem[];
    workspaces: string[];
    activeWorkspace: string | null;
    activeSessionId: string | null;
    activity: Record<string, SessionActivity>;
    unread: Record<string, UnreadMark>;
    onOpen: (sessionId: string, projectRoot: string) => void;
    onDelete: (sessionId: string) => void;
    onAddWorkspace: () => void;
    onRemoveWorkspace: (root: string) => void;
    onActivateWorkspace: (root: string | null) => void;
    onNewChat: () => void;
    onSearch: () => void;
  };

  let { onNewChat, onSearch, ...list }: Props = $props();
  const mod = modLabel();
</script>

<div class="sidebar-slot">
  <aside class="app-sidebar" aria-label="Chats">
    <div class="sidebar-top">
      <Tooltip text="New chat" shortcut={`${mod}N`} side="right">
        <button type="button" class="sidebar-action" onclick={onNewChat}>
          <Icon icon={Plus} size={14} strokeWidth={2} />
          <span>New chat</span>
        </button>
      </Tooltip>
      <Tooltip text="Search chats" shortcut={`${mod}K`} side="right">
        <button type="button" class="sidebar-action" onclick={onSearch}>
          <Icon icon={Search} size={14} strokeWidth={1.9} />
          <span>Search</span>
        </button>
      </Tooltip>
    </div>
    <Sidebar {...list} />
  </aside>
</div>
