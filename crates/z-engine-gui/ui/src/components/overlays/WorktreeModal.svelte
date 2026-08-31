<script lang="ts">
  import { Icon, GitBranch, X } from "$lib/ui/icons";

  type Props = {
    isClosing?: boolean;
    onClose: () => void;
    onCreate: (name: string) => void;
  };

  let { isClosing = false, onClose, onCreate }: Props = $props();

  let name = $state("");
  const slug = $derived(name.toLowerCase().replace(/[^a-z0-9-]/g, ""));
</script>

<!-- Name prompt for a new git worktree: creates `.z-engine/worktrees/<name>`
     on branch `zengine/<name>`, registers it as a workspace and starts a task
     there (handled by the caller). Existing modal-overlay markup keeps CSS. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-overlay{isClosing ? ' is-closing' : ''}" onmousedown={onClose}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal{isClosing ? ' is-closing' : ''}" onmousedown={(e) => e.stopPropagation()}>
    <div class="modal-head">
      <Icon icon={GitBranch} size={13} />
      <span>New task in a git worktree</span>
      <button type="button" class="icon-btn" onclick={onClose}>
        <Icon icon={X} size={12} />
      </button>
    </div>
    <p class="modal-sub">
      Creates an isolated checkout at
      <code>.z-engine/worktrees/{slug || "<name>"}</code> on branch
      <code>zengine/{slug || "<name>"}</code>, then starts a session there. The main working tree
      stays untouched.
    </p>
    <form
      onsubmit={(e) => {
        e.preventDefault();
        if (slug) onCreate(slug);
      }}
    >
      <!-- svelte-ignore a11y_autofocus -->
      <input
        autofocus
        bind:value={name}
        placeholder="worktree name (e.g. fix-login)"
        spellcheck={false}
        onkeydown={(e) => e.key === "Escape" && onClose()}
      />
      <div class="modal-actions">
        <button type="button" class="btn-ghost" onclick={onClose}>Cancel</button>
        <button type="submit" disabled={!slug} class="btn-primary">Create & start</button>
      </div>
    </form>
  </div>
</div>
