import { listen } from "@tauri-apps/api/event";

export type MsgKind =
  | "user"
  | "assistant"
  | "thinking"
  | "tool"
  | "approval"
  | "notice"
  | "command"
  | "error";

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

function emitChange() {
  for (const l of listeners) l();
}

function emitToasts() {
  for (const l of toastListeners) l();
}

export function pushToast(text: string, tone: Toast["tone"] = "info") {
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
  for (const l of approvalGateSubs) l();
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

/** Push a quiet inline row into the transcript (TUI `Block::Notice`
 * parity). Used by slash commands and startup banner. */
export function pushNotice(text: string) {
  push("notice", text);
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
  const rule = m.suggestedRule ?? m.bashCommand ?? "";
  const text =
    decision === "deny"
      ? `✗ denied · ${m.toolName ?? "call"}`
      : decision === "once"
        ? `✓ approved (once) · ${m.bashCommand ?? m.toolName ?? ""}`.trimEnd()
        : `${decision === "persist" ? "✓ persisted rule" : "✓ approved · session rule"} '${rule}'`;
  update(m.id, { kind: "notice", text, streaming: false });
  if (pendingApprovals > 0) {
    pendingApprovals--;
    emitApprovalGate();
  }
}

let assistantBuf = "";
let assistantMsgId = -1;
let thinkingOpen = false;
let thinkingMsgId = -1;
/** Counts user submissions since the current agent loop started — the
 * checkpoint-stack index the backend will assign to each new turn. */
let runTurnCounter = 0;

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

/** Route a core status note: shell echoes stay in the transcript,
 * everything else becomes a transient toast so chat stays clean. */
function routeStatusNote(text: string) {
  if (text.startsWith("$ ")) {
    // collapse consecutive shell output into one message
    const last = messages[messages.length - 1];
    if (last && last.kind === "notice" && last.text.startsWith("$ ")) {
      update(last.id, { text: `${last.text}\n${text}` });
    } else {
      push("notice", text);
    }
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
  if (text.startsWith("rewind")) {
    pushToast(text, text.includes("nothing") ? "info" : "ok");
    return;
  }
  // Context-pressure events stay in the transcript as durable notices:
  // a toast covers the header and vanishes, hiding the compaction audit.
  if (
    text.startsWith("context at ") ||
    text.startsWith("context compacted")
  ) {
    push("notice", text);
    return;
  }
  // TUI parity: everything else narrates inline instead of vanishing
  // into a toast (reviewer findings, compaction notes, auto-accepts…).
  push("notice", text);
}

type EventPayload = { type: string } & Record<string, unknown>;

export function handleEvent(ev: EventPayload) {
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
        push(ok ? "notice" : "error", `${ok ? "✓" : "✗"} ${ev.name} ─ ${ev.summary}`);
      }
      break;
    }
    case "approvalRequired": {
      closeThinking();
      endAssistant();
      busy = false;
      pendingApprovals++;
      emitApprovalGate();
      push(
        "approval",
        `⚠ approval required — ${ev.tool}\ninput: ${ev.inputPreview}`,
        {
          approvalId: Number(ev.id),
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
      // A finished turn leaves no decision pending; stale gates would
      // block the queue forever.
      if (pendingApprovals !== 0) {
        pendingApprovals = 0;
        emitApprovalGate();
      }
      if (ev.type === "turnCompleted") {
        usage = {
          ...usage,
          promptTokens: Number(ev.promptTokens ?? usage.promptTokens),
          completionTokens: Number(
            ev.completionTokens ?? usage.completionTokens,
          ),
        };
        push("notice", "✓ done");
      } else {
        push("notice", "■ aborted");
      }
      emitChange();
      break;
    case "sessionChanged":
      sessionId = String(ev.ulid ?? "");
      for (const l of sessionSubs) l();
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

export function commandLocal(cmd: string) {
  push("command", `! ${cmd}`);
}

export function setBusy(v: boolean) {
  busy = v;
  emitChange();
}

let eventsInitialized = false;
export async function initEvents() {
  // Idempotent: React StrictMode double-invokes effects in dev, and a
  // second registration would handle every event twice (duplicated
  // messages/toasts).
  if (eventsInitialized) return;
  await listen<EventPayload>("appEvent", (e) => handleEvent(e.payload));
  // The backend announces new/resumed session ids on its own channel.
  await listen<{ ulid: string }>("sessionChanged", (e) => {
    sessionId = String(e.payload.ulid ?? "");
    for (const l of sessionSubs) l();
  });
  eventsInitialized = true;
}

/** Test hook: reset all module state. */
export function resetForTests() {
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
 * harness_core::session::SessionEvent (`user_msg`, `assistant_msg`,
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
        push("user", ev.text ?? "");
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
        push("notice", ev.text ?? "");
        break;
      case "meta":
      case "tool_result":
        break;
    }
  }
}
