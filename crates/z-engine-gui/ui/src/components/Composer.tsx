import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import {
  busyStore,
  setBusy,
  submitLocal,
  commandLocal,
  draftStore,
  attachmentStore,
  pushToast,
} from "../lib/events";
import { abort, listProjectFiles, shellPassthrough, submit } from "../lib/commands";
import { activeAtToken, stripAtToken } from "../lib/atFile";
import { queueStore } from "../lib/events";
import { Terminal, Paperclip, ArrowUp, CornerDownLeft, Square } from "lucide-react";
import { filterSlash } from "../lib/slash";
import { dispatchSlashCommand } from "../lib/composerCommands";
import { fileToDataUrl } from "../lib/imageUtil";
import { ModelPicker } from "./ModelPicker";
import { ModePicker } from "./ModePicker";
import { EffortSelector } from "./EffortSelector";
import { ShellOverlay } from "./ShellOverlay";
import { catalogStore } from "../lib/catalog";
import { hideShell, shellStore, showShell } from "../lib/shellStore";
import { ComposerPopovers } from "./ComposerPopovers";
import { ComposerAttachments } from "./ComposerAttachments";

import { useComposerHistory } from "../lib/composerHistory";

export function Composer() {
  const input = useSyncExternalStore(draftStore.subscribe, () => draftStore.getSnapshot());
  useEffect(() => {
    void catalogStore.ensure();
  }, []);
  const attachments = useSyncExternalStore(
    attachmentStore.subscribe,
    () => attachmentStore.getSnapshot(),
  );
  const [images, setImages] = useState<string[]>([]);
  const [caret, setCaret] = useState(0);
  const [slashSel, setSlashSel] = useState(0);
  const [fileSel, setFileSel] = useState(0);
  const [dismissed, setDismissed] = useState(false);
  const [fileResult, setFileResult] = useState<{ q: string; list: string[] | null }>({
    q: "",
    list: null,
  });
  const taRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const { pushHistory, historyPrev, historyNext } = useComposerHistory();

  const busyNow = useSyncExternalStore(busyStore.subscribe, () => busyStore.getSnapshot());
  const shell = useSyncExternalStore(shellStore.subscribe, () => shellStore.getSnapshot());
  const shellMode = input.startsWith("!");

  const slashMatches = !dismissed ? filterSlash(input) : null;
  const atQuery =
    !dismissed && slashMatches === null ? activeAtToken(input, caret) : null;
  const showSlash = Boolean(slashMatches && slashMatches.length > 0);
  const showFiles = atQuery !== null;
  const files = fileResult.q === atQuery ? fileResult.list : null;

  useEffect(() => {
    if (atQuery === null) return;
    let alive = true;
    const t = setTimeout(() => {
      listProjectFiles(atQuery)
        .then((r) => alive && setFileResult({ q: atQuery, list: r }))
        .catch(() => alive && setFileResult({ q: atQuery, list: [] }));
    }, 140);
    return () => {
      alive = false;
      clearTimeout(t);
    };
  }, [atQuery]);

  function syncCaret() {
    const ta = taRef.current;
    if (ta) setCaret(ta.selectionStart);
  }

  function onInputChanged(text: string, caretPos: number) {
    draftStore.set(text);
    setCaret(caretPos);
    setSlashSel(0);
    setFileSel(0);
    setDismissed(false);
  }

  function runCommand(name: string) {
    const currentInput = input;
    draftStore.set("");
    setCaret(0);
    setDismissed(false);
    dispatchSlashCommand(name, currentInput);
  }

  function insertFile(path: string) {
    attachmentStore.add(path);
    const r = stripAtToken(input, caret);
    draftStore.set(r.text);
    setDismissed(false);
    requestAnimationFrame(() => {
      const ta = taRef.current;
      if (ta) {
        ta.focus();
        ta.setSelectionRange(r.caret, r.caret);
        setCaret(r.caret);
      }
    });
  }

  async function onPaste(e: React.ClipboardEvent<HTMLTextAreaElement>) {
    const pasted = Array.from(e.clipboardData.files ?? []);
    if (pasted.length === 0) return;
    e.preventDefault();
    for (const f of pasted.slice(0, 4)) {
      const url = await fileToDataUrl(f);
      if (url) setImages((imgs) => [...imgs, url].slice(0, 6));
    }
  }

  async function onFileInputChanged(e: React.ChangeEvent<HTMLInputElement>) {
    const list = Array.from(e.target.files ?? []);
    for (const f of list.slice(0, 4)) {
      if (f.type.startsWith("image/")) {
        const url = await fileToDataUrl(f);
        if (url) setImages((imgs) => [...imgs, url].slice(0, 6));
      } else {
        attachmentStore.add(f.name);
      }
    }
    if (fileInputRef.current) fileInputRef.current.value = "";
  }

  async function send() {
    const text = input.trim();
    if (!text && images.length === 0) return;
    const atts = attachmentStore.getSnapshot();
    draftStore.set("");
    attachmentStore.clear();
    setCaret(0);
    setDismissed(false);
    const myImages = images;
    setImages([]);
    const composed =
      atts.length > 0
        ? `${text}\n\n${atts.map((p) => `@${p}`).join(" ")}`
        : text;

    if (busyNow) {
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
      } catch (e) {
        console.error(e);
      }
      return;
    }
    submitLocal(composed, myImages);
    setBusy(true);
    try {
      await submit(composed, myImages);
    } catch (e) {
      console.error(e);
      setBusy(false);
      pushToast(String(e).replace("Error: ", ""), "warn");
    }
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    syncCaret();
    if (showSlash && slashMatches && slashMatches.length > 0) {
      const n = slashMatches.length;
      if (e.key === "ArrowDown") { e.preventDefault(); setSlashSel((s) => (s + 1) % n); return; }
      if (e.key === "ArrowUp") { e.preventDefault(); setSlashSel((s) => (s - 1 + n) % n); return; }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        runCommand(slashMatches[Math.min(slashSel, n - 1)].name);
        return;
      }
      if (e.key === "Escape") { e.preventDefault(); setDismissed(true); return; }
    }
    if (showFiles && files && files.length > 0) {
      const fn = files.length;
      if (e.key === "ArrowDown") { e.preventDefault(); setFileSel((s) => (s + 1) % fn); return; }
      if (e.key === "ArrowUp") { e.preventDefault(); setFileSel((s) => (s - 1 + fn) % fn); return; }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        insertFile(files[Math.min(fileSel, fn - 1)]);
        return;
      }
      if (e.key === "Escape") { e.preventDefault(); setDismissed(true); return; }
    }
    if ((e.key === "ArrowUp" || e.key === "ArrowDown") && !input.includes("\n")) {
      e.preventDefault();
      if (e.key === "ArrowUp") historyPrev(setCaret);
      else historyNext(setCaret);
      return;
    }
    if (e.key === "Escape") {
      if (busyNow) { e.preventDefault(); void abort(); }
      else if (shell.visible) { e.preventDefault(); hideShell(); }
      else if (input) { e.preventDefault(); draftStore.set(""); setCaret(0); }
      return;
    }
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing && e.keyCode !== 229) {
      e.preventDefault();
      void send();
    }
  }

  return (
    <div className="composer-wrap">
      <ShellOverlay />
      <div className={`composer${shellMode ? " shell" : ""}`}>
        <ComposerPopovers
          showSlash={showSlash}
          slashMatches={slashMatches}
          slashSel={slashSel}
          onSelectSlash={runCommand}
          onHoverSlash={setSlashSel}
          showFiles={showFiles}
          files={files}
          fileSel={fileSel}
          onSelectFile={insertFile}
          onHoverFile={setFileSel}
        />
        <ComposerAttachments
          attachments={attachments}
          images={images}
          onRemoveAttachment={(p) => attachmentStore.remove(p)}
          onRemoveImage={(i) => setImages((imgs) => imgs.filter((_, j) => j !== i))}
        />
        <div className={`composer-input-area${shellMode ? " shell-active" : ""}`}>
          {shellMode && (
            <div className="shell-prefix-glyph" aria-hidden="true">
              <Terminal size={13} className="shell-glyph-icon" />
              <span className="shell-glyph-arrow">❯</span>
            </div>
          )}
          <textarea
            ref={taRef}
            rows={2}
            className={shellMode ? "shell-textarea" : ""}
            placeholder={
              busyNow
                ? "Working… press Stop or Esc to abort."
                : shellMode
                  ? "Enter shell command… (e.g. !git status, !cargo test)"
                  : "Ask a question, describe a task, @ files, / commands…"
            }
            value={input}
            onChange={(e) =>
              onInputChanged(e.currentTarget.value, e.currentTarget.selectionStart)
            }
            onSelect={syncCaret}
            onClick={syncCaret}
            onKeyUp={syncCaret}
            onKeyDown={onKeyDown}
            onPaste={(e) => void onPaste(e)}
          />
        </div>
        <div className="composer-bar">
          <input
            type="file"
            ref={fileInputRef}
            multiple
            style={{ display: "none" }}
            onChange={(e) => void onFileInputChanged(e)}
          />
          {shellMode ? (
            <div className="shell-bar-left">
              <span className="shell-mode-pill">
                <Terminal size={11} />
                <span>Shell Pass-Through</span>
              </span>
              <span className="shell-hint-inline">
                <kbd>Esc</kbd> to exit
              </span>
            </div>
          ) : (
            <div className="composer-controls-left">
              <ModePicker />
              <ModelPicker />
              <EffortSelector catalog={catalogStore.getSnapshot()} />
              <button
                type="button"
                className="composer-icon-btn"
                title="Attach file or image"
                onClick={() => fileInputRef.current?.click()}
              >
                <Paperclip size={13} />
              </button>
              {!shell.visible && shell.entries.length > 0 && (
                <button
                  type="button"
                  className="composer-icon-btn"
                  title="Show terminal drawer"
                  onClick={showShell}
                >
                  <Terminal size={13} />
                </button>
              )}
            </div>
          )}

          <div className="composer-actions-right">
            {!shellMode && (
              <div className="composer-hints-deck">
                <span className="c-hint">
                  <kbd>@</kbd> files
                </span>
                <span className="c-hint">
                  <kbd>/</kbd> cmds
                </span>
                <span className="c-hint">
                  <kbd>!</kbd> bash
                </span>
              </div>
            )}
            {busyNow ? (
              <button
                className="stop"
                title="Stop (Esc)"
                onClick={() => void abort()}
                type="button"
              >
                <Square size={11} fill="currentColor" />
              </button>
            ) : shellMode ? (
              <button
                className="send shell-send"
                title="Run shell command (Enter)"
                onClick={() => void send()}
                disabled={!input.slice(1).trim()}
                type="button"
              >
                <CornerDownLeft size={12} />
                <span>Run</span>
              </button>
            ) : (
              <button
                className="send"
                title="Send (Enter)"
                onClick={() => void send()}
                disabled={!input.trim() && attachments.length === 0}
                type="button"
              >
                <ArrowUp size={15} strokeWidth={2.4} />
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
