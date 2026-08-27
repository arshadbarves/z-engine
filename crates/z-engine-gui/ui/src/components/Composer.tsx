import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import {
  busyStore,
  setBusy,
  submitLocal,
  usageStore,
  commandLocal,
  draftStore,
  modeStore,
  modelStore,
  attachmentStore,
  sessionStore,
  pushNotice,
  pushToast,
} from "../lib/events";
import { configStore } from "../lib/configStore";
import {
  abort,
  compact,
  notes,
  listProjectFiles,
  shellPassthrough,
  submit,
} from "../lib/commands";
import { activeAtToken, stripAtToken } from "../lib/atFile";
import { queueStore } from "../lib/events";
import { Terminal, X } from "lucide-react";
import { filterSlash, getCustomCommands } from "../lib/slash";
import { readSlashCommand } from "../lib/commands";
import { estimateCost, fmtCost } from "../lib/util";
import { ModelPicker } from "./ModelPicker";
import { ModePicker } from "./ModePicker";
import { EffortSelector } from "./EffortSelector";
import { ShellOverlay } from "./ShellOverlay";
import { catalogStore } from "../lib/catalog";
import { hideShell, shellStore, showShell } from "../lib/shellStore";

function fileName(p: string): string {
  const i = p.lastIndexOf("/");
  return i >= 0 ? p.slice(i + 1) : p;
}

function extLabel(p: string): string {
  const n = fileName(p);
  const d = n.lastIndexOf(".");
  return d > 0 ? n.slice(d + 1).toUpperCase() : "FILE";
}

function FileIcon() {
  return (
    <svg viewBox="0 0 24 24" width={16} height={16} fill="none" stroke="currentColor"
      strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <path d="M14 2v6h6" />
      <path d="M9 13h6M9 17h4" />
    </svg>
  );
}

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
  // fetched file list is keyed by its query so a new query shows "searching…"
  // without synchronous setState inside the effect
  const [fileResult, setFileResult] = useState<{ q: string; list: string[] | null }>({
    q: "",
    list: null,
  });
  const taRef = useRef<HTMLTextAreaElement>(null);
  const historyRef = useRef<string[]>([]);
  const histPosRef = useRef<number | null>(null);

  const busyNow = useSyncExternalStore(busyStore.subscribe, () => busyStore.getSnapshot());
  const shell = useSyncExternalStore(shellStore.subscribe, () => shellStore.getSnapshot());
  const shellMode = input.startsWith("!");

  const slashMatches = !dismissed ? filterSlash(input) : null;
  const atQuery =
    !dismissed && slashMatches === null ? activeAtToken(input, caret) : null;
  const showSlash = Boolean(slashMatches && slashMatches.length > 0);
  const showFiles = atQuery !== null;
  const files = fileResult.q === atQuery ? fileResult.list : null;

  // debounced project-file lookup for the @ picker
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
    if (!ta) return;
    setCaret(ta.selectionStart);
  }

  function onInputChanged(text: string, caretPos: number) {
    draftStore.set(text);
    setCaret(caretPos);
    setSlashSel(0);
    setFileSel(0);
    setDismissed(false);
  }

  function runCommand(name: string) {
    draftStore.set("");
    setCaret(0);
    setDismissed(false);

    // User-defined command: expand template ($ARGUMENTS ← typed args)
    // and submit as a normal task message.
    const custom = getCustomCommands().find((c) => c.name === name);
    if (custom) {
      const args = input.replace(/^\/\S*\s*/, "").trim();
      void (async () => {
        try {
          const template = await readSlashCommand(name);
          const prompt = template.replaceAll("$ARGUMENTS", args).replace(/\s+$/, "");
          submitLocal(prompt);
          setBusy(true);
          await submit(prompt);
        } catch (e) {
          console.error(e);
          setBusy(false);
          pushNotice(`/${name}: ${String(e)}`);
        }
      })();
      return;
    }

    switch (name) {
      case "compact":
        void compact();
        break;
      case "notes":
        void notes();
        break;
      case "help":
        pushNotice(
          "commands: /help /compact /notes /cost /status\n" +
            "keys: Enter send · Esc abort · ⇧⏎ newline · ! shell · @ files · ⌘K palette",
        );
        break;
      case "cost": {
        const u = usageStore.getSnapshot();
        const cfg = configStore.getSnapshot();
        const cost = estimateCost(
          cfg?.pricing ?? null,
          u.promptTokens,
          u.completionTokens,
        );
        const total = u.promptTokens + u.completionTokens;
        pushNotice(
          `tokens this session: prompt=${u.promptTokens} completion=${u.completionTokens} total=${total}` +
            (cost != null ? ` · est. ${fmtCost(cost)}` : " · $–"),
        );
        break;
      }
      case "status": {
        const u = usageStore.getSnapshot();
        const cfg = configStore.getSnapshot();
        pushNotice(
          `model=${cfg?.model || modelStore.getSnapshot()} · mode=${modeStore.getSnapshot()} · session=${
            sessionStore.getSnapshot() || "(new)"
          } · tokens ${u.promptTokens + u.completionTokens}/${u.maxTokens}`,
        );
        break;
      }
    }
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

  /** Downscale pasted images to <=1568px long side and JPEG-encode to
   * keep payloads sane for vision APIs. */
  async function fileToDataUrl(file: File): Promise<string | null> {
    if (!file.type.startsWith("image/")) return null;
    const bitmap = await createImageBitmap(file).catch(() => null);
    if (!bitmap) return null;
    const maxSide = 1568;
    const scale = Math.min(1, maxSide / Math.max(bitmap.width, bitmap.height));
    const w = Math.max(1, Math.round(bitmap.width * scale));
    const h = Math.max(1, Math.round(bitmap.height * scale));
    const canvas = document.createElement("canvas");
    canvas.width = w;
    canvas.height = h;
    canvas.getContext("2d")!.drawImage(bitmap, 0, 0, w, h);
    return canvas.toDataURL("image/jpeg", 0.85);
  }

  async function onPaste(e: React.ClipboardEvent<HTMLTextAreaElement>) {
    const files = Array.from(e.clipboardData.files ?? []);
    if (files.length === 0) return;
    e.preventDefault();
    for (const f of files.slice(0, 4)) {
      const url = await fileToDataUrl(f);
      if (url) setImages((imgs) => [...imgs, url].slice(0, 6));
    }
  }

  async function send() {
    const text = input.trim();
    if (!text && images.length === 0) return;
    const atts = attachmentStore.getSnapshot();
    draftStore.set("");
    attachmentStore.clear();
    setCaret(0);
    setDismissed(false);
    histPosRef.current = null;
    const myImages = images;
    setImages([]);
    const composed =
      atts.length > 0
        ? `${text}\n\n${atts.map((p) => `@${p}`).join(" ")}`
        : text;

    // Busy → Codex-style follow-up queue instead of a dead input.
    if (busyNow) {
      if (composed || myImages.length > 0) {
        queueStore.push(composed, myImages);
        pushToast("Queued — sends when the turn finishes", "info");
      }
      return;
    }

    historyRef.current = [...historyRef.current, composed].slice(-100);
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
      // The turn will never complete, so clear busy ourselves or the
      // composer soft-locks until restart.
      console.error(e);
      setBusy(false);
      pushToast(String(e).replace("Error: ", ""), "warn");
    }
  }

  function historyPrev() {
    const h = historyRef.current;
    if (h.length === 0) return;
    const pos =
      histPosRef.current === null ? h.length - 1 : Math.max(0, histPosRef.current - 1);
    histPosRef.current = pos;
    draftStore.set(h[pos]);
    setCaret(h[pos].length);
  }

  function historyNext() {
    const h = historyRef.current;
    const pos = histPosRef.current;
    if (pos === null) return;
    if (pos + 1 >= h.length) {
      histPosRef.current = null;
      draftStore.set("");
      setCaret(0);
    } else {
      histPosRef.current = pos + 1;
      draftStore.set(h[pos + 1]);
      setCaret(h[pos + 1].length);
    }
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    syncCaret();
    if (showSlash) {
      const n = slashMatches!.length;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSlashSel((s) => (s + 1) % n);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSlashSel((s) => (s - 1 + n) % n);
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        runCommand(slashMatches![Math.min(slashSel, n - 1)].name);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setDismissed(true);
        return;
      }
    }
    if (showFiles && files && files.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setFileSel((s) => (s + 1) % files.length);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setFileSel((s) => (s - 1 + files.length) % files.length);
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        insertFile(files[Math.min(fileSel, files.length - 1)]);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setDismissed(true);
        return;
      }
    }
    // TUI parity: ↑/↓ recall submission history on single-line drafts
    if ((e.key === "ArrowUp" || e.key === "ArrowDown") && !input.includes("\n")) {
      e.preventDefault();
      if (e.key === "ArrowUp") historyPrev();
      else historyNext();
      return;
    }
    // TUI parity: Esc aborts a running turn, else hides the terminal, else clears the draft
    if (e.key === "Escape") {
      if (busyNow) {
        e.preventDefault();
        void abort();
      } else if (shell.visible) {
        e.preventDefault();
        hideShell();
      } else if (input) {
        e.preventDefault();
        draftStore.set("");
        setCaret(0);
      }
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void send();
    }
  }

  return (
    <div className="composer-wrap">
      <ShellOverlay />
      <div className={`composer${shellMode ? " shell" : ""}`}>
        {showSlash && (
          <div className="composer-pop" role="listbox">
            {slashMatches!.map((c, i) => (
              <button
                key={c.name}
                role="option"
                aria-selected={i === slashSel}
                className={`pop-item${i === slashSel ? " sel" : ""}`}
                onMouseEnter={() => setSlashSel(i)}
                onClick={() => runCommand(c.name)}
              >
                <span className="pop-name">/{c.name}</span>
                <span className="pop-desc">{c.desc}</span>
              </button>
            ))}
          </div>
        )}
        {showFiles && (
          <div className="composer-pop" role="listbox">
            {files === null && <div className="pop-note">searching…</div>}
            {files !== null && files.length === 0 && (
              <div className="pop-note">no matching files</div>
            )}
            {files?.map((f, i) => (
              <button
                key={f}
                role="option"
                aria-selected={i === fileSel}
                className={`pop-item mono${i === fileSel ? " sel" : ""}`}
                onMouseEnter={() => setFileSel(i)}
                onClick={() => insertFile(f)}
              >
                <span className="pop-name">{f}</span>
              </button>
            ))}
          </div>
        )}
        {attachments.length > 0 && (
          <div className="attachments">
            {attachments.map((p) => (
              <span key={p} className="attachment">
                <button
                  className="att-x"
                  title={`Remove ${p}`}
                  onClick={() => attachmentStore.remove(p)}
                >
                  <svg viewBox="0 0 24 24" width={9} height={9} fill="none" stroke="currentColor"
                    strokeWidth={2.4} strokeLinecap="round" aria-hidden>
                    <path d="M18 6L6 18M6 6l12 12" />
                  </svg>
                </button>
                <span className="att-icon">
                  <FileIcon />
                </span>
                <span className="att-text">
                  <span className="att-name">{fileName(p)}</span>
                  <span className="att-ext">{extLabel(p)}</span>
                </span>
              </span>
            ))}
          </div>
        )}
        {images.length > 0 && (
          <div className="attachments img-chips">
            {images.map((url, i) => (
              <span key={i} className="attachment img-chip">
                <button
                  className="att-x"
                  title="Remove image"
                  onClick={() => setImages((imgs) => imgs.filter((_, j) => j !== i))}
                >
                  <X size={9} strokeWidth={2.4} />
                </button>
                <img src={url} alt={`paste ${i + 1}`} />
              </span>
            ))}
          </div>
        )}
        <textarea
          ref={taRef}
          rows={2}
          placeholder={
            busyNow
              ? "Working… press Stop or Esc to abort."
              : shellMode
                ? "shell command…  (not sent to the model)"
                : "Describe a task…  (! for shell · / commands · @ files)"
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
        ></textarea>
        <div className="composer-bar">
          {shellMode && (
            <span className="shell-prompt" title="Shell command — not sent to the model">
              $
            </span>
          )}
          <ModePicker />
          <ModelPicker />
          <EffortSelector catalog={catalogStore.getSnapshot()} />
          {!shell.visible && shell.entries.length > 0 && (
            <button
              type="button"
              className="icon-btn"
              title="Show terminal"
              onClick={showShell}
            >
              <Terminal size={13} />
            </button>
          )}
          {!shellMode && (
            <span className="composer-hint">
              <kbd>!</kbd> shell · <kbd>/</kbd> cmds · <kbd>@</kbd> files
            </span>
          )}
          {busyNow ? (
            <button className="stop" title="Stop" onClick={() => void abort()}>
              <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden>
                <rect x="5" y="5" width="14" height="14" rx="2.5" />
              </svg>
            </button>
          ) : (
            <button
              className="send"
              title="Send"
              onClick={() => void send()}
              disabled={!input.trim() && attachments.length === 0}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2.4}
                strokeLinecap="round" strokeLinejoin="round" aria-hidden>
                <path d="M12 19V5M5 12l7-7 7 7" />
              </svg>
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
