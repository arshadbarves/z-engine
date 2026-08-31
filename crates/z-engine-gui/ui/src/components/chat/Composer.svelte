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
  import Icon, { ArrowUp, CornerDownLeft, Paperclip, Square, Terminal } from "$lib/ui/icons";
  import ShellOverlay from "../overlays/ShellOverlay.svelte";
  import ComposerAttachments from "./ComposerAttachments.svelte";
  import ComposerPopovers from "./ComposerPopovers.svelte";
  import EffortSelector from "./EffortSelector.svelte";
  import ModePicker from "./ModePicker.svelte";
  import ModelPicker from "./ModelPicker.svelte";

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

  function onKeyDown(e: KeyboardEvent) {
    syncCaret();
    if (showSlash && slashMatches && slashMatches.length > 0) {
      const n = slashMatches.length;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        slashSel = (slashSel + 1) % n;
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        slashSel = (slashSel - 1 + n) % n;
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        runCommand(slashMatches[Math.min(slashSel, n - 1)].name);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        dismissed = true;
        return;
      }
    }
    if (showFiles && files && files.length > 0) {
      const fn = files.length;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        fileSel = (fileSel + 1) % fn;
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        fileSel = (fileSel - 1 + fn) % fn;
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        insertFile(files[Math.min(fileSel, fn - 1)]);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        dismissed = true;
        return;
      }
    }
    if ((e.key === "ArrowUp" || e.key === "ArrowDown") && !input.current.includes("\n")) {
      e.preventDefault();
      if (e.key === "ArrowUp") historyPrev((n) => (caret = n));
      else historyNext((n) => (caret = n));
      return;
    }
    if (e.key === "Escape") {
      if (busyNow.current) {
        e.preventDefault();
        void abort();
      } else if (shell.current.visible) {
        e.preventDefault();
        hideShell();
      } else if (input.current) {
        e.preventDefault();
        draftStore.set("");
        caret = 0;
      }
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
          ? "Working… press Stop or Esc to abort."
          : shellMode
            ? "Enter shell command… (e.g. !git status, !cargo test)"
            : "Ask a question, describe a task, @ files, / commands…"}
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
    <div class="composer-bar">
      <input
        type="file"
        bind:this={fileInput}
        multiple
        style="display: none"
        onchange={(e) => void onFileInputChanged(e)}
      />
      {#if shellMode}
        <div class="shell-bar-left">
          <span class="shell-mode-pill">
            <Icon icon={Terminal} size={11} />
            <span>Bash Mode</span>
          </span>
          <span class="shell-hint-inline"><kbd>Esc</kbd> to return</span>
        </div>
      {:else}
        <div class="composer-controls-left">
          <ModePicker />
          <ModelPicker />
          <EffortSelector catalog={catalog.current} />
          <button
            type="button"
            class="composer-icon-btn"
            title="Attach file or image"
            onclick={() => fileInput?.click()}
          >
            <Icon icon={Paperclip} size={13} />
          </button>
          {#if !shell.current.visible && shell.current.entries.length > 0}
            <button
              type="button"
              class="composer-icon-btn"
              title="Show terminal drawer"
              onclick={showShell}
            >
              <Icon icon={Terminal} size={13} />
            </button>
          {/if}
        </div>
      {/if}

      <div class="composer-actions-right">
        {#if !shellMode}
          <div class="composer-hints-deck">
            <span class="c-hint"><kbd>@</kbd> files</span>
            <span class="c-hint"><kbd>/</kbd> cmds</span>
            <span class="c-hint"><kbd>!</kbd> bash</span>
          </div>
        {/if}
        {#if busyNow.current}
          <button class="stop" title="Stop (Esc)" onclick={() => void abort()} type="button">
            <Icon icon={Square} size={11} />
          </button>
        {:else if shellMode}
          <button
            class="send shell-send"
            title="Run shell command (Enter)"
            onclick={() => void send()}
            disabled={!input.current.slice(1).trim()}
            type="button"
          >
            <Icon icon={CornerDownLeft} size={12} />
            <span>Run</span>
          </button>
        {:else}
          <button
            class="send"
            title="Send (Enter)"
            onclick={() => void send()}
            disabled={!input.current.trim() && attachments.current.length === 0}
            type="button"
          >
            <Icon icon={ArrowUp} size={15} strokeWidth={2.4} />
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>
