<script lang="ts">
  import { activeAtToken, stripAtToken } from "$lib/atFile";
  import { catalogStore } from "$lib/catalog";
  import { abort, listProjectFiles, shellPassthrough, submit } from "$lib/commands";
  import { dispatchSlashCommand } from "$lib/composerCommands";
  import { createComposerHistory } from "$lib/composerHistory";
  import { fileToDataUrl } from "$lib/imageUtil";
  import {
    attachmentStore,
    busyStore,
    commandLocal,
    draftStore,
    pushToast,
    queueStore,
    setBusy,
    submitLocal,
  } from "$lib/runtime";
  import { hideShell, shellStore, showShell } from "$lib/shellStore";
  import { filterSlash } from "$lib/slash";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import ShellOverlay from "../overlays/ShellOverlay.svelte";
  import ComposerAttachments from "./ComposerAttachments.svelte";
  import ComposerBar from "./ComposerBar.svelte";
  import ComposerPopovers from "./ComposerPopovers.svelte";

  const input = bindStore(draftStore);
  const attachments = bindStore(attachmentStore);
  const busyNow = bindStore(busyStore);
  const shell = bindStore(shellStore);
  const catalog = bindStore(catalogStore);
  const { pushHistory, historyPrev, historyNext } = createComposerHistory();

  let images = $state<string[]>([]);
  let caret = $state(0);
  let slashSel = $state(0);
  let fileSel = $state(0);
  let dismissed = $state(false);
  let fileResult = $state<{ q: string; list: string[] | null }>({ q: "", list: null });
  let ta: HTMLTextAreaElement | undefined = $state();
  let fileInput: HTMLInputElement | undefined = $state();

  const shellMode = $derived(input.current.startsWith("!"));
  const slashMatches = $derived(!dismissed ? filterSlash(input.current) : null);
  const atQuery = $derived(
    !dismissed && slashMatches === null ? activeAtToken(input.current, caret) : null,
  );
  const showSlash = $derived(Boolean(slashMatches && slashMatches.length > 0));
  const showFiles = $derived(atQuery !== null);
  const files = $derived(fileResult.q === atQuery ? fileResult.list : null);

  $effect(() => {
    void catalogStore.ensure();
  });

  $effect(() => {
    const q = atQuery;
    if (q === null) return;
    let alive = true;
    const t = setTimeout(() => {
      listProjectFiles(q)
        .then((r) => {
          if (alive) fileResult = { q, list: r };
        })
        .catch(() => {
          if (alive) fileResult = { q, list: [] };
        });
    }, 140);
    return () => {
      alive = false;
      clearTimeout(t);
    };
  });

  function syncCaret() {
    if (ta) caret = ta.selectionStart;
  }

  function onInputChanged(text: string, caretPos: number) {
    draftStore.set(text);
    caret = caretPos;
    slashSel = 0;
    fileSel = 0;
    dismissed = false;
  }

  function runCommand(name: string) {
    const currentInput = input.current;
    draftStore.set("");
    caret = 0;
    dismissed = false;
    dispatchSlashCommand(name, currentInput);
  }

  function insertFile(path: string) {
    attachmentStore.add(path);
    const r = stripAtToken(input.current, caret);
    draftStore.set(r.text);
    dismissed = false;
    requestAnimationFrame(() => {
      if (ta) {
        ta.focus();
        ta.setSelectionRange(r.caret, r.caret);
        caret = r.caret;
      }
    });
  }

  async function onPaste(e: ClipboardEvent) {
    const pasted = Array.from(e.clipboardData?.files ?? []);
    if (pasted.length === 0) return;
    e.preventDefault();
    for (const f of pasted.slice(0, 4)) {
      const url = await fileToDataUrl(f);
      if (url) images = [...images, url].slice(0, 6);
    }
  }

  async function onFileInputChanged(e: Event) {
    const list = Array.from((e.currentTarget as HTMLInputElement).files ?? []);
    for (const f of list.slice(0, 4)) {
      if (f.type.startsWith("image/")) {
        const url = await fileToDataUrl(f);
        if (url) images = [...images, url].slice(0, 6);
      } else {
        attachmentStore.add(f.name);
      }
    }
    if (fileInput) fileInput.value = "";
  }

  async function send() {
    const text = input.current.trim();
    if (!text && images.length === 0) return;
    const atts = attachmentStore.getSnapshot();
    draftStore.set("");
    attachmentStore.clear();
    caret = 0;
    dismissed = false;
    const myImages = images;
    images = [];
    const composed = atts.length > 0 ? `${text}\n\n${atts.map((p) => `@${p}`).join(" ")}` : text;

    if (busyNow.current) {
      if (composed || myImages.length > 0) {
        queueStore.push(composed, myImages);
        pushToast("Queued — sends when the turn finishes", "info");
      }
      return;
    }

    pushHistory(composed);
    if (text.startsWith("!")) {
      const cmd = text.slice(1).trim();
      if (!cmd) return;
      commandLocal(cmd);
      try {
        await shellPassthrough(cmd);
      } catch (err) {
        console.error(err);
      }
      return;
    }
    submitLocal(composed, myImages);
    setBusy(true);
    try {
      await submit(composed, myImages);
    } catch (err) {
      console.error(err);
      setBusy(false);
      pushToast(String(err).replace("Error: ", ""), "warn");
    }
  }

  function handleNav(
    e: KeyboardEvent,
    len: number,
    getSel: () => number,
    setSel: (n: number) => void,
    onPick: (idx: number) => void,
  ): boolean {
    if (len === 0) return false;
    if (e.key === "ArrowDown") { e.preventDefault(); setSel((getSel() + 1) % len); return true; }
    if (e.key === "ArrowUp") { e.preventDefault(); setSel((getSel() - 1 + len) % len); return true; }
    if (e.key === "Enter" || e.key === "Tab") { e.preventDefault(); onPick(Math.min(getSel(), len - 1)); return true; }
    if (e.key === "Escape") { e.preventDefault(); dismissed = true; return true; }
    return false;
  }

  function onKeyDown(e: KeyboardEvent) {
    syncCaret();
    if (showSlash && slashMatches && slashMatches.length > 0) {
      if (handleNav(e, slashMatches.length, () => slashSel, (n) => (slashSel = n), (i) => runCommand(slashMatches[i].name)))
        return;
    }
    if (showFiles && files && files.length > 0) {
      if (handleNav(e, files.length, () => fileSel, (n) => (fileSel = n), (i) => insertFile(files[i])))
        return;
    }
    if ((e.key === "ArrowUp" || e.key === "ArrowDown") && !input.current.includes("\n")) {
      e.preventDefault();
      if (e.key === "ArrowUp") historyPrev((n) => (caret = n));
      else historyNext((n) => (caret = n));
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      if (busyNow.current) void abort();
      else if (shell.current.visible) hideShell();
      else if (input.current) { draftStore.set(""); caret = 0; }
      return;
    }
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing && e.keyCode !== 229) {
      e.preventDefault();
      void send();
    }
  }
</script>

<div class="composer-wrap">
  <ShellOverlay />
  <div class={`composer${shellMode ? " shell" : ""}`}>
    <ComposerPopovers
      {showSlash}
      {slashMatches}
      {slashSel}
      onSelectSlash={runCommand}
      onHoverSlash={(i) => (slashSel = i)}
      {showFiles}
      {files}
      {fileSel}
      onSelectFile={insertFile}
      onHoverFile={(i) => (fileSel = i)}
    />
    <ComposerAttachments
      attachments={attachments.current}
      {images}
      onRemoveAttachment={(p) => attachmentStore.remove(p)}
      onRemoveImage={(i) => (images = images.filter((_, j) => j !== i))}
    />
    <div class={`composer-input-area${shellMode ? " shell-active" : ""}`}>
      <textarea
        bind:this={ta}
        rows={2}
        class={shellMode ? "shell-textarea" : ""}
        placeholder={busyNow.current
          ? "Agent is working… press Esc to abort"
          : shellMode
            ? "Enter shell command… (e.g. !git status)"
            : "Ask anything, @ for files, / for commands, ! for bash…"}
        value={input.current}
        oninput={(e) =>
          onInputChanged(e.currentTarget.value, e.currentTarget.selectionStart)}
        onselect={syncCaret}
        onclick={syncCaret}
        onkeyup={syncCaret}
        onkeydown={onKeyDown}
        onpaste={(e) => void onPaste(e)}
      ></textarea>
    </div>
    <input
      type="file"
      bind:this={fileInput}
      multiple
      style="display: none"
      onchange={(e) => void onFileInputChanged(e)}
    />
    <ComposerBar
      {shellMode}
      busy={busyNow.current}
      canSend={Boolean(input.current.trim() || attachments.current.length > 0 || images.length > 0)}
      canSendShell={Boolean(input.current.slice(1).trim())}
      catalog={catalog.current}
      showTerminalBtn={!shell.current.visible && shell.current.entries.length > 0}
      onAttachClick={() => fileInput?.click()}
      onShowShell={showShell}
      onSend={() => void send()}
      onAbort={() => void abort()}
    />
  </div>
</div>
