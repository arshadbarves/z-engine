import { listen } from "@tauri-apps/api/event";
import { appendShellLine, resetShell, startShell } from "./shellStore";
import {
  clearSnaps,
  emptySnap,
  getSnap,
  hasSnap,
  parkSnap,
  parkedEntries,
  takeSnap,
  type SessionSnap,
} from "./sessionSnaps";

export type MsgKind =
  | "user"
  | "assistant"
  | "thinking"
  | "tool"
  | "approval"
  | "notice"
  | "command"
  | "error"
  | "status";

export interface Msg {
  id: number;
  kind: MsgKind;
  text: string;
  streaming?: boolean;
  ok?: boolean;
  approvalId?: number;
  canPersist?: boolean;
  suggestedRule?: string | null;
  bashCommand?: string | null;
  /** Unified-diff rich preview for approval cards. */
  detailPreview?: string | null;
  /** Tool card fields. */
  toolName?: string;
  preview?: string;
  summary?: string;
  /** Accumulated stdout/stderr while a bash call runs. */
  output?: string;
  startedAt?: number;
  durationMs?: number;
  /** Thinking block: full retained body + collapsed flag for toggling. */
  thinkingBody?: string;
  collapsed?: boolean;
  /** 0-based index of this user message among turns submitted during the
   * current app run. Null for replayed messages — their file checkpoints
   * are gone, so per-message revert is unavailable. */
  runTurn?: number | null;
  /** Pasted screenshots (data URLs) attached to this user message. */
  images?: string[];
}

/** Last `n` lines of accumulated tool output for the live tail. */
export function tailLines(output: string, n = 10): string[] {
  const lines = output.replace(/\n$/, "").split("\n");
  return lines.slice(Math.max(0, lines.length - n));
}

export interface Toast {
  id: number;
  text: string;
  tone: "info" | "ok" | "warn";
}

interface Usage {
  promptTokens: number;
  completionTokens: number;
  maxTokens: number;
}

type Listener = () => void;
const modelSubs = new Set<Listener>();

let messages: Msg[] = [];
let busy = false;
let usage: Usage = { promptTokens: 0, completionTokens: 0, maxTokens: 120_000 };
let model = "";
const listeners = new Set<Listener>();
let toastListeners = new Set<Listener>();
let toasts: Toast[] = [];
let nextId = 1;
let nextToastId = 1;

let emitPaused = false;
const workingSubs = new Set<Listener>();

/** What a session's sidebar indicator should say: `working` while the
 * turn runs, `approval` while it sits blocked on a permission decision.
 * Approval wins — a gated turn is still occupied, just waiting. */
export type SessionActivity = "working" | "approval";

let activityCache: Record<string, SessionActivity> = {};

function collectActivity(): Record<string, SessionActivity> {
  const map: Record<string, SessionActivity> = {};
  if (sessionId) {
    if (pendingApprovals > 0) map[sessionId] = "approval";
    else if (busy) map[sessionId] = "working";
  }
  for (const [id, s] of parkedEntries()) {
    if (s.pendingApprovals > 0) map[id] = "approval";
    else if (s.busy) map[id] = "working";
  }
  return map;
}

function sameActivity(
  a: Record<string, SessionActivity>,
  b: Record<string, SessionActivity>,
): boolean {
  const aKeys = Object.keys(a);
  return (
    aKeys.length === Object.keys(b).length && aKeys.every((k) => b[k] === a[k])
  );
}

function syncWorking() {
  const next = collectActivity();
  if (sameActivity(activityCache, next)) return;
  activityCache = next;
  for (const l of workingSubs) l();
}

function emitChange() {
  if (emitPaused) return;
  for (const l of listeners) l();
  syncWorking();
}

function emitToasts() {
  for (const l of toastListeners) l();
}

export function pushToast(text: string, tone: Toast["tone"] = "info") {
  if (emitPaused) return;
  const t: Toast = { id: nextToastId++, text, tone };
  toasts = [...toasts.slice(-3), t];
  emitToasts();
  const life = tone === "warn" ? 4200 : 2600;
  setTimeout(() => {
    toasts = toasts.filter((x) => x.id !== t.id);
    emitToasts();
  }, life);
}

export const transcriptStore = {
  subscribe(l: Listener) {
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  },
  getSnapshot(): Msg[] {
    return messages;
  },
};

export const busyStore = {
  subscribe(l: Listener) {
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  },
  getSnapshot(): boolean {
    return busy;
  },
};

/** Number of approval cards awaiting a decision. Kept separate from
 * `busy` (which only drives the WorkingRow spinner): an agent blocked on
 * approval is NOT free to accept queued follow-ups, so queue flushing
 * must consult this gate too. */
let pendingApprovals = 0;
const approvalGateSubs = new Set<Listener>();
function emitApprovalGate() {
  if (emitPaused) return;
  for (const l of approvalGateSubs) l();
  syncWorking();
}
export const approvalGateStore = {
  subscribe(l: Listener) {
    approvalGateSubs.add(l);
    return () => {
      approvalGateSubs.delete(l);
    };
  },
  getSnapshot(): number {
    return pendingApprovals;
  },
};

/** Follow-up messages typed while a turn runs (Codex-style queue). */
export interface QueuedMessage {
  text: string;
  images: string[];
}
let queue: QueuedMessage[] = [];
const queueListeners = new Set<Listener>();

function emitQueue() {
  if (emitPaused) return;
  for (const l of queueListeners) l();
}

export const queueStore = {
  subscribe(l: Listener) {
    queueListeners.add(l);
    return () => {
      queueListeners.delete(l);
    };
  },
  getSnapshot(): QueuedMessage[] {
    return queue;
  },
  push(text: string, images: string[] = []) {
    queue = [...queue, { text, images }];
    emitQueue();
  },
  removeAt(i: number) {
    queue = queue.filter((_, j) => j !== i);
    emitQueue();
  },
  shift(): QueuedMessage | undefined {
    const [first, ...rest] = queue;
    if (first === undefined) return undefined;
    queue = rest;
    emitQueue();
    return first;
  },
};

export const usageStore = {
  subscribe(l: Listener) {
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  },
  getSnapshot(): Usage {
    return usage;
  },
};

export const modelStore = {
  subscribe(l: Listener) {
    modelSubs.add(l);
    return () => {
      modelSubs.delete(l);
    };
  },
  getSnapshot(): string {
    return model;
  },
  set(m: string) {
    if (m && m !== model) {
      model = m;
      for (const l of modelSubs) l();
    }
  },
};

export const toastStore = {
  subscribe(l: Listener) {
    toastListeners.add(l);
    return () => {
      toastListeners.delete(l);
    };
  },
  getSnapshot(): Toast[] {
    return toasts;
  },
};

/** Bumped when the sidebar should reload (first send, generated title). */
let sessionsTick = 0;
const sessionsTickSubs = new Set<Listener>();
export const sessionsTickStore = {
  subscribe(l: Listener) {
    sessionsTickSubs.add(l);
    return () => {
      sessionsTickSubs.delete(l);
    };
  },
  getSnapshot(): number {
    return sessionsTick;
  },
};

function bumpSessionsTick() {
  sessionsTick += 1;
  for (const l of sessionsTickSubs) l();
}

export function setMaxTokens(max: number) {
  if (max > 0) usage = { ...usage, maxTokens: max };
  emitChange();
}

/** Zero the token counters (fresh session): the meter must not carry the
 * previous session's numbers until the next provider report arrives. */
export function resetUsage() {
  usage = { promptTokens: 0, completionTokens: 0, maxTokens: usage.maxTokens };
  emitChange();
}

export function setUsageTokens(prompt: number, completion: number) {
  usage = { ...usage, promptTokens: prompt, completionTokens: completion };
  emitChange();
}

/** Permission-mode singleton shared between the composer select,
 * the palette, and status notes. */
let mode = "normal";
const modeSubs = new Set<Listener>();
export const modeStore = {
  subscribe(l: Listener) {
    modeSubs.add(l);
    return () => {
      modeSubs.delete(l);
    };
  },
  getSnapshot(): string {
    return mode;
  },
  set(m: string) {
    if (m && m !== mode) {
      mode = m;
      for (const l of modeSubs) l();
    }
  },
};

/** Composer draft text shared between the textarea, hero chips, and
 * slash/file popups so any surface can seed or clear the input. */
let draft = "";
const draftSubs = new Set<Listener>();
export const draftStore = {
  subscribe(l: Listener) {
    draftSubs.add(l);
    return () => {
      draftSubs.delete(l);
    };
  },
  getSnapshot(): string {
    return draft;
  },
  set(text: string) {
    if (text !== draft) {
      draft = text;
      for (const l of draftSubs) l();
    }
  },
};

/** `@`-mention attachment chips pending on the composer. */
let attachments: string[] = [];
const attachmentSubs = new Set<Listener>();
export const attachmentStore = {
  subscribe(l: Listener) {
    attachmentSubs.add(l);
    return () => {
      attachmentSubs.delete(l);
    };
  },
  getSnapshot(): string[] {
    return attachments;
  },
  add(path: string) {
    if (!path || attachments.includes(path)) return;
    attachments = [...attachments, path];
    for (const l of attachmentSubs) l();
  },
  remove(path: string) {
    attachments = attachments.filter((p) => p !== path);
    for (const l of attachmentSubs) l();
  },
  clear() {
    if (attachments.length === 0) return;
    attachments = [];
    for (const l of attachmentSubs) l();
  },
};

/** ULID of the session the agent loop is currently writing to. */
let sessionId = "";
const sessionSubs = new Set<Listener>();
export const sessionStore = {
  subscribe(l: Listener) {
    sessionSubs.add(l);
    return () => {
      sessionSubs.delete(l);
    };
  },
  getSnapshot(): string {
    return sessionId;
  },
};

function snapshot(): SessionSnap {
  return {
    messages,
    nextId,
    assistantBuf,
    assistantMsgId,
    thinkingOpen,
    thinkingMsgId,
    runTurnCounter,
    busy,
    turnStartedAt,
    pendingApprovals,
    queue,
    usage,
  };
}

function loadSnap(s: SessionSnap) {
  messages = s.messages;
  nextId = s.nextId;
  assistantBuf = s.assistantBuf;
  assistantMsgId = s.assistantMsgId;
  thinkingOpen = s.thinkingOpen;
  thinkingMsgId = s.thinkingMsgId;
  runTurnCounter = s.runTurnCounter;
  busy = s.busy;
  turnStartedAt = s.turnStartedAt;
  pendingApprovals = s.pendingApprovals;
  queue = s.queue;
  usage = s.usage;
}

export function hasSessionRuntime(id: string): boolean {
  return Boolean(id) && (id === sessionId || hasSnap(id));
}

/** Switch the visible chat. The previous session stays in memory (and its
 * agent loop keeps running); coming back restores the live transcript. */
export function activateSession(ulid: string) {
  if (!ulid || ulid === sessionId) return;
  if (sessionId) parkSnap(sessionId, snapshot());
  const parked = takeSnap(ulid);
  sessionId = ulid;
  loadSnap(parked ?? emptySnap(usage.maxTokens));
  for (const l of sessionSubs) l();
  emitChange();
  emitQueue();
  emitApprovalGate();
}

export const sessionActivityStore = {
  subscribe(l: Listener) {
    workingSubs.add(l);
    return () => {
      workingSubs.delete(l);
    };
  },
  getSnapshot(): Record<string, SessionActivity> {
    return activityCache;
  },
};

function applyToParked(sid: string, ev: EventPayload) {
  emitPaused = true;
  const saved = snapshot();
  const savedId = sessionId;
  loadSnap(getSnap(sid) ?? emptySnap(usage.maxTokens));
  sessionId = sid;
  dispatchEvent(ev);
  parkSnap(sid, snapshot());
  sessionId = savedId;
  loadSnap(saved);
  emitPaused = false;
  syncWorking();
}

/** Submit a follow-up onto a session that may not be on screen. */
export function submitOnSession(id: string, text: string, images: string[] = []) {
  if (!id || id === sessionId) {
    submitLocal(text, images);
    setBusy(true);
    return;
  }
  emitPaused = true;
  const saved = snapshot();
  const savedId = sessionId;
  loadSnap(getSnap(id) ?? emptySnap(usage.maxTokens));
  sessionId = id;
  submitLocal(text, images);
  setBusy(true);
  parkSnap(id, snapshot());
  sessionId = savedId;
  loadSnap(saved);
  emitPaused = false;
  syncWorking();
}

/** Queued prompts whose session is idle (active or background). */
export function drainReadyQueues(): { sessionId: string; text: string; images: string[] }[] {
  const out: { sessionId: string; text: string; images: string[] }[] = [];
  if (!busy && pendingApprovals === 0 && queue.length > 0) {
    const next = queueStore.shift();
    if (next && sessionId) out.push({ sessionId, text: next.text, images: next.images });
  }
  for (const [id, snap] of parkedEntries()) {
    if (snap.busy || snap.pendingApprovals > 0 || snap.queue.length === 0) continue;
    const next = snap.queue[0];
    snap.queue = snap.queue.slice(1);
    out.push({ sessionId: id, text: next.text, images: next.images });
  }
  if (out.length) emitQueue();
  return out;
}

/** Transient status (slash commands, model switch, startup). Chat stays
 * a conversation — these never become transcript rows. */
export function pushNotice(text: string) {
  pushToast(text, "info");
}

/** Collapse a decided approval card in place to a one-line notice so the
 * history keeps an audit trail without clutter (TUI parity). */
export function resolveApproval(
  approvalId: number,
  decision: "once" | "session" | "persist" | "deny",
) {
  const m = messages.find(
    (x) => x.kind === "approval" && x.approvalId === approvalId,
  );
  if (!m) return;
  if (pendingApprovals > 0) pendingApprovals--;
  // The loop resumes the moment its last gate is decided (a granted tool
  // runs, a denial goes back to the model). Mark the turn busy again or
  // the indicator stays dark while the approved command is still running.
  if (pendingApprovals === 0) busy = true;
  const rule = m.suggestedRule ?? m.bashCommand ?? "";
  const text =
    decision === "deny"
      ? `✗ denied · ${m.toolName ?? "call"}`
      : decision === "once"
        ? `✓ approved (once) · ${m.bashCommand ?? m.toolName ?? ""}`.trimEnd()
        : `${decision === "persist" ? "✓ persisted rule" : "✓ approved · session rule"} '${rule}'`;
  update(m.id, { kind: "notice", text, streaming: false });
  emitApprovalGate();
}

let assistantBuf = "";
let assistantMsgId = -1;
let thinkingOpen = false;
let thinkingMsgId = -1;
/** Counts user submissions since the current agent loop started — the
 * checkpoint-stack index the backend will assign to each new turn. */
let runTurnCounter = 0;
let turnStartedAt = 0;

/** Most recent open (running) tool card with the given name, or null.
 * Core finish events carry only the tool name — matching the newest
 * unfinished card of that name is what the TUI does, and what keeps
 * overlapping same-name calls (parallel reads) from cross-completing. */
function latestOpenTool(name: string): Msg | null {
  for (let i = messages.length - 1; i >= 0; i--) {
    const m = messages[i];
    if (m.kind === "tool" && m.streaming && m.toolName === name) return m;
  }
  return null;
}

function push(kind: MsgKind, text: string, extra?: Partial<Msg>): number {
  const id = nextId++;
  messages = [...messages, { id, kind, text, ...extra }];
  emitChange();
  return id;
}

function update(id: number, extra: Partial<Msg>) {
  messages = messages.map((m: Msg) => (m.id === id ? { ...m, ...extra } : m));
  emitChange();
}

function patch(id: number, extra: Partial<Msg>): void {
  messages = messages.map((m) => (m.id === id ? { ...m, ...extra } : m));
}

function endAssistant() {
  if (assistantMsgId >= 0) {
    update(assistantMsgId, { text: assistantBuf, streaming: false });
    assistantBuf = "";
    assistantMsgId = -1;
  }
}

function closeThinking() {
  if (thinkingOpen) {
    const body = messages.find((m) => m.id === thinkingMsgId)?.thinkingBody?.length ?? 0;
    update(thinkingMsgId, {
      text: `✻ thought (${body} chars)`,
      streaming: false,
      collapsed: true,
    });
    thinkingOpen = false;
  }
}

/** Route a core status note: `!` shell echoes go to the terminal overlay,
 * everything else is a toast so the chat stays a conversation. */
function routeStatusNote(text: string) {
  if (text.startsWith("$ ")) {
    appendShellLine(text);
    return;
  }
  if (text.startsWith("mode: ")) {
    const m = text.slice(6).trim();
    modeStore.set(
      m === "auto-accept edits" ? "accept-edits" : m === "plan" ? "plan" : "normal",
    );
    pushToast(`Mode · ${m}`);
    return;
  }
  if (text.startsWith("model set to ")) {
    const id = text.slice("model set to ".length);
    modelStore.set(id);
    pushToast(`Model · ${id}`, "ok");
    return;
  }
  if (text === "shell unavailable") {
    appendShellLine("shell unavailable");
    pushToast("Shell unavailable", "warn");
    return;
  }
  const tone: Toast["tone"] =
    text.includes("failed") || text.includes("error") ? "warn" : "info";
  pushToast(text, tone);
}

type EventPayload = { type: string } & Record<string, unknown>;

/** While a session is being swapped, drop live loop events (shutdown
 * `turnAborted` would otherwise toast and race the restored transcript). */
let hydrateLock = false;
let hydrateGen = 0;
let hydrating = false;
const hydrateSubs = new Set<Listener>();
function emitHydrate() {
  for (const l of hydrateSubs) l();
}

export function beginHydrate(): number {
  hydrateLock = true;
  hydrating = true;
  hydrateGen += 1;
  emitHydrate();
  return hydrateGen;
}

export function endHydrate(gen?: number) {
  if (gen != null && gen !== hydrateGen) return;
  hydrateLock = false;
  hydrating = false;
  emitHydrate();
}

export const hydrateStore = {
  subscribe(l: Listener) {
    hydrateSubs.add(l);
    return () => {
      hydrateSubs.delete(l);
    };
  },
  getSnapshot(): boolean {
    return hydrating;
  },
};

export function handleEvent(ev: EventPayload) {
  const sid = String(ev.sessionId ?? "");
  if (sid && sessionId && sid !== sessionId) {
    if (sid === "boot") return;
    applyToParked(sid, ev);
    return;
  }
  if (hydrateLock && ev.type !== "sessionChanged") return;
  dispatchEvent(ev);
}

function dispatchEvent(ev: EventPayload) {
  if (hydrateLock && ev.type !== "sessionChanged") return;
  switch (ev.type) {
    case "tokenDelta": {
      closeThinking();
      assistantBuf += String(ev.text ?? "");
      if (assistantMsgId < 0) {
        assistantMsgId = push("assistant", assistantBuf, { streaming: true });
      } else {
        update(assistantMsgId, { text: assistantBuf, streaming: true });
      }
      break;
    }
    case "reasoningDelta": {
      if (!thinkingOpen) {
        thinkingMsgId = push("thinking", "✻ thinking…", {
          streaming: true,
          thinkingBody: "",
          collapsed: false,
        });
        thinkingOpen = true;
      }
      const body =
        (messages.find((m) => m.id === thinkingMsgId)?.thinkingBody ?? "") +
        String(ev.text ?? "");
      update(thinkingMsgId, {
        text: `✻ thinking… (${body.length} chars)`,
        thinkingBody: body,
        streaming: true,
      });
      break;
    }
    case "toggleThinking": {
      const m = messages.find((x) => x.id === Number(ev.id));
      if (m && m.kind === "thinking") {
        update(m.id, { collapsed: !m.collapsed });
      }
      break;
    }
    case "turnStarted":
      bumpSessionsTick();
      break;
    case "toolCallStarted": {
      closeThinking();
      endAssistant();
      push("tool", `⚙ ${ev.name} ─ ${ev.preview}`, {
        toolName: String(ev.name),
        preview: String(ev.preview),
        output: "",
        startedAt: Date.now(),
        streaming: true,
      });
      break;
    }
    case "toolOutputDelta": {
      const open = latestOpenTool(String(ev.toolName ?? ""));
      if (!open) break;
      const MAX_TOOL_OUTPUT = 64 * 1024;
      const merged = (open.output ?? "") + String(ev.text ?? "");
      // Head+tail cap: `yes`-style commands must not grow the session's
      // memory (and re-render cost) without bound.
      const output =
        merged.length > MAX_TOOL_OUTPUT
          ? merged.slice(0, MAX_TOOL_OUTPUT / 2) +
            "\n[…output truncated…]\n" +
            merged.slice(merged.length - MAX_TOOL_OUTPUT / 2)
          : merged;
      patch(open.id, { output });
      emitChange();
      break;
    }
    case "toolCallFinished": {
      const ok = Boolean(ev.ok);
      const open = latestOpenTool(String(ev.name ?? ""));
      if (open) {
        update(open.id, {
          text: `${ok ? "✓" : "✗"} ${ev.name} ─ ${ev.summary}`,
          streaming: false,
          ok,
          summary: String(ev.summary),
          durationMs: Number(ev.durationMs ?? 0),
        });
      } else {
        pushToast(`${ok ? "✓" : "✗"} ${ev.name} ─ ${ev.summary}`, ok ? "ok" : "warn");
      }
      break;
    }
    case "approvalRequired": {
      closeThinking();
      endAssistant();
      const approvalId = Number(ev.id);
      if (messages.some((m) => m.kind === "approval" && m.approvalId === approvalId)) {
        break;
      }
      busy = false;
      pendingApprovals++;
      emitApprovalGate();
      push(
        "approval",
        `⚠ approval required — ${ev.tool}\ninput: ${ev.inputPreview}`,
        {
          approvalId,
          toolName: String(ev.tool),
          canPersist: Boolean(ev.canPersist),
          suggestedRule: (ev.suggestedRule as string | null) ?? null,
          bashCommand: (ev.bashCommand as string | null) ?? null,
          detailPreview: (ev.detailPreview as string | null) ?? null,
          text: `⚠ approval required — ${ev.tool}\ninput: ${ev.inputPreview}`,
        },
      );
      emitChange();
      break;
    }
    case "usageUpdated":
      usage = {
        ...usage,
        promptTokens: Number(ev.promptTokens ?? 0),
        completionTokens: Number(ev.completionTokens ?? 0),
      };
      emitChange();
      break;
    case "statusNote":
      routeStatusNote(String(ev.text ?? ""));
      break;
    case "turnCompleted":
    case "turnAborted":
      closeThinking();
      endAssistant();
      busy = false;
      if (pendingApprovals !== 0) {
        pendingApprovals = 0;
        emitApprovalGate();
      }
      // An aborted turn can leave its approval card behind; expire it so a
      // late click can't resume a loop that no longer exists.
      if (messages.some((m) => m.kind === "approval")) {
        messages = messages.map((m) =>
          m.kind === "approval"
            ? {
                ...m,
                kind: "notice",
                text: `■ approval expired · ${m.toolName ?? "call"}`,
                streaming: false,
              }
            : m,
        );
      }
      if (ev.type === "turnCompleted") {
        usage = {
          ...usage,
          promptTokens: Number(ev.promptTokens ?? usage.promptTokens),
          completionTokens: Number(
            ev.completionTokens ?? usage.completionTokens,
          ),
        };
        const ms = turnStartedAt ? Date.now() - turnStartedAt : 0;
        push("status", ms > 0 ? `✓ done · ${(ms / 1000).toFixed(1)}s` : "✓ done", {
          ok: true,
        });
      } else {
        push("status", "■ aborted", { ok: false });
      }
      turnStartedAt = 0;
      bumpSessionsTick();
      emitChange();
      break;
    case "sessionChanged":
      activateSession(String(ev.ulid ?? ""));
      break;
    case "transcriptTrimmed":
      trimTranscript(Number(ev.keepTurn ?? 0));
      break;
    case "sessionTitle":
      bumpSessionsTick();
      break;
    case "error":
      closeThinking();
      endAssistant();
      busy = false;
      push("error", `ERROR: ${ev.message}`);
      emitChange();
      break;
  }
}

export function submitLocal(text: string, images: string[] = []) {
  closeThinking();
  endAssistant();
  push("user", text, { runTurn: runTurnCounter++, images });
}

/** Drop the user message at `keepTurn` and every later card. Restores the
 * run-turn counter so a follow-up send reuses that checkpoint index. */
export function trimTranscript(keepTurn: number) {
  closeThinking();
  endAssistant();
  let cut = -1;
  for (let i = 0; i < messages.length; i++) {
    const m = messages[i];
    if (m.kind === "user" && m.runTurn === keepTurn) {
      cut = i;
      break;
    }
  }
  if (cut < 0) return;
  messages = messages.slice(0, cut);
  runTurnCounter = keepTurn;
  emitChange();
}

export function commandLocal(cmd: string) {
  startShell(cmd);
}

export function setBusy(v: boolean) {
  if (v && !busy) turnStartedAt = Date.now();
  busy = v;
  emitChange();
}

let eventsInitialized = false;
export async function initEvents() {
  // Set the flag before awaiting — StrictMode remounts would otherwise
  // register two Tauri listeners and duplicate every approval/tool row.
  if (eventsInitialized) return;
  eventsInitialized = true;
  await listen<EventPayload>("appEvent", (e) => handleEvent(e.payload));
  await listen<{ ulid: string }>("sessionChanged", (e) => {
    activateSession(String(e.payload.ulid ?? ""));
  });
}

/** Test hook: reset all module state. */
export function resetForTests() {
  hydrateLock = false;
  hydrateGen = 0;
  hydrating = false;
  turnStartedAt = 0;
  emitPaused = false;
  clearSnaps();
  activityCache = {};
  resetTranscript();
  busy = false;
  usage = { promptTokens: 0, completionTokens: 0, maxTokens: 120_000 };
  model = "";
  mode = "normal";
  draft = "";
  attachments = [];
  sessionId = "";
  toasts = [];
  nextToastId = 1;
  resetShell();
}

export function parkCurrentAndReset() {
  if (sessionId) parkSnap(sessionId, snapshot());
  sessionId = "";
  loadSnap(emptySnap(usage.maxTokens));
  for (const l of sessionSubs) l();
  emitChange();
  emitQueue();
  emitApprovalGate();
}

/** Clear the transcript (session switch / new task). */
export function resetTranscript() {
  messages = [];
  nextId = 1;
  assistantBuf = "";
  assistantMsgId = -1;
  thinkingOpen = false;
  thinkingMsgId = -1;
  // A fresh loop starts with an empty checkpoint stack.
  runTurnCounter = 0;
  busy = false;
  turnStartedAt = 0;
  if (pendingApprovals !== 0) {
    pendingApprovals = 0;
    emitApprovalGate();
  }
  if (queue.length > 0) {
    queue = [];
    emitQueue();
  }
  if (draft) draftStore.set("");
  attachmentStore.clear();
  resetShell();
  emitChange();
}

export interface ReplayToolCall {
  id: string;
  name: string;
  arguments: string;
}

export interface ReplayEvent {
  type: string;
  text?: string;
  content?: string | null;
  tool_calls?: ReplayToolCall[];
  tool_call_id?: string;
  model?: string;
  project_root?: string;
  images?: string[];
}

function shortArgs(name: string, raw: string): string {
  try {
    const v = JSON.parse(raw) as Record<string, unknown>;
    if (typeof v.path === "string") return v.path;
    if (typeof v.command === "string") return v.command;
    if (typeof v.pattern === "string") return v.pattern;
    if (typeof v.query === "string") return v.query;
    return Object.values(v)[0]?.toString() ?? name;
  } catch {
    return raw;
  }
}

/** First meaningful line of a persisted tool result, for the collapsed
 * summary (mirrors what the live `toolCallFinished` summary shows). */
function resultSummary(content: string): string {
  for (const line of content.split("\n")) {
    const t = line.trim();
    if (!t || t.startsWith("--- ") || t.startsWith("[killed") || t.startsWith("exit code:")) {
      continue;
    }
    return t.length > 120 ? `${t.slice(0, 117)}…` : t;
  }
  return "ok";
}

/** Rebuild transcript cards from a session JSONL event list
 * (`read_session`). Event `type` tags are serde snake_case variants of
 * z_engine_core::session::SessionEvent (`user_msg`, `assistant_msg`,
 * `tool_result`, `note`, `meta`) — live-looking but inert. */
export function replaySession(events: ReplayEvent[]) {
  resetTranscript();
  const results = new Map<string, string>();
  for (const ev of events) {
    if (ev.type === "tool_result" && ev.tool_call_id) {
      results.set(ev.tool_call_id, ev.content ?? "");
    }
  }
  for (const ev of events) {
    switch (ev.type) {
      case "user_msg":
        push("user", ev.text ?? "", {
          images: ev.images && ev.images.length > 0 ? ev.images : undefined,
          runTurn: runTurnCounter++,
        });
        break;
      case "assistant_msg": {
        if (ev.content) push("assistant", ev.content);
        for (const tc of ev.tool_calls ?? []) {
          const out = results.get(tc.id) ?? "";
          push("tool", `✓ ${tc.name} ─ ${shortArgs(tc.name, tc.arguments)}`, {
            toolName: tc.name,
            preview: shortArgs(tc.name, tc.arguments),
            summary: resultSummary(out),
            output: out,
            ok: true,
            streaming: false,
            durationMs: 0,
          });
        }
        break;
      }
      case "note":
        // Historical status notes stay out of the chat; live ones toast.
        break;
      case "meta":
      case "tool_result":
      case "title":
        break;
    }
  }
}
