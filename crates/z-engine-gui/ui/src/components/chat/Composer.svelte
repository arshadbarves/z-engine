<script lang="ts">
  import { editQueue, cancelTurn, sessions } from "$lib/runtime";
  import { shellStore, showShell } from "$lib/shellStore";
  import { composer } from "$lib/stores/composer.svelte";
  import { currentStage } from "$lib/stores/stage.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { workspaceStore } from "$lib/workspaces";
  import ShellOverlay from "../overlays/ShellOverlay.svelte";
  import ComposerAttachments from "./ComposerAttachments.svelte";
  import ComposerBar from "./ComposerBar.svelte";
  import ComposerDropZone from "./ComposerDropZone.svelte";
  import ComposerInput from "./ComposerInput.svelte";
  import ComposerQueue from "./ComposerQueue.svelte";

  /**
   * The floating composer: queued steering, attachments, the draft and its
   * bar. Larger on the project home; images can be pasted, picked or dropped.
   */
  const shell = bindStore(shellStore);
  const workspaces = bindStore(workspaceStore);

  let input: ComposerInput | undefined = $state();
  let imageInput: HTMLInputElement | undefined = $state();
  let dragging = $state(0);

  const view = $derived(sessions.active);
  const busy = $derived((view?.status ?? "idle") !== "idle");
  const root = $derived(view?.info?.projectRoot ?? workspaces.current.active ?? workspaces.current.roots[0] ?? null);
  const text = $derived(composer.draft);
  const shellMode = $derived(text.startsWith("!"));
  const hero = $derived(currentStage() === "home");
  const canSend = $derived(shellMode ? Boolean(text.slice(1).trim()) : Boolean(text.trim() || composer.attachments.length > 0));

  function hasImages(list: FileList | null | undefined): boolean {
    return Array.from(list ?? []).some((f) => f.type.startsWith("image/"));
  }

  function takeFiles(e: Event, list: FileList | null | undefined) {
    if (!hasImages(list)) return;
    e.preventDefault();
    void composer.addImageFiles(Array.from(list ?? []));
  }

  function onDragEnter(e: DragEvent) {
    if (!e.dataTransfer?.types.includes("Files")) return;
    e.preventDefault();
    dragging += 1;
  }

  function onDrop(e: DragEvent) {
    dragging = 0;
    takeFiles(e, e.dataTransfer?.files);
  }
</script>

<div class="composer-wrap" class:is-hero={hero}>
  <ShellOverlay />
  <div
    class={`composer${shellMode ? " shell" : ""}${dragging ? " is-dropping" : ""}`}
    role="group"
    aria-label="Message composer"
    ondragenter={onDragEnter}
    ondragleave={() => (dragging = Math.max(0, dragging - 1))}
    ondragover={(e) => e.preventDefault()}
    ondrop={onDrop}
  >
    <ComposerDropZone active={dragging > 0} />
    <ComposerQueue items={view?.queue ?? []} onChange={(queued) => void editQueue(queued)} />
    <ComposerAttachments attachments={composer.attachments} onRemove={(i) => composer.removeAttachment(i)} />
    <ComposerInput
      bind:this={input}
      {root}
      {busy}
      {hero}
      shellVisible={shell.current.visible}
      onPasteFiles={(e) => takeFiles(e, e.clipboardData?.files)}
    />
    <input
      type="file"
      accept="image/*"
      multiple
      hidden
      bind:this={imageInput}
      onchange={(e) => {
        void composer.addImageFiles(Array.from(e.currentTarget.files ?? []));
        e.currentTarget.value = "";
      }}
    />
    <ComposerBar
      {shellMode}
      {busy}
      {canSend}
      hasText={Boolean(text.trim())}
      showTerminal={!shell.current.visible && shell.current.entries.length > 0}
      onAttach={() => imageInput?.click()}
      onInsert={(prefix) => input?.insert(prefix)}
      onShowShell={showShell}
      onSend={() => void input?.submit(false)}
      onInterrupt={() => void input?.submit(true)}
      onCancel={() => void cancelTurn()}
    />
  </div>
</div>
