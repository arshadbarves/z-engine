import { listen } from "@tauri-apps/api/event";

export type MsgKind =
  | "user"
  | "assistant"
  | "thinking"
  | "tool"
  | "approval"
  | "notice"
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
}

type Listener = () => void;

let messages: Msg[] = [];
let busy = false;
const listeners = new Set<Listener>();

function emitChange() {
  for (const l of listeners) l();
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

let nextId = 1;
let assistantBuf = "";
let assistantMsgId = -1;
let thinkingOpen = false;
let thinkingMsgId = -1;
let thinkingChars = 0;
let openToolId = -1;

function push(kind: MsgKind, text: string, extra?: Partial<Msg>): number {
  const id = nextId++;
  messages = [...messages, { id, kind, text, ...extra }];
  emitChange();
  return id;
}

function update(id: number, text: string, extra?: Partial<Msg>) {
  messages = messages.map((m: Msg) =>
    m.id === id ? { ...m, text, ...extra } : m,
  );
  emitChange();
}

function endAssistant() {
  if (assistantMsgId >= 0) {
    update(assistantMsgId, assistantBuf, { streaming: false });
    assistantBuf = "";
    assistantMsgId = -1;
  }
}

function closeThinking() {
  if (thinkingOpen) {
    update(
      thinkingMsgId,
      `✻ thinking collapsed (${thinkingChars} chars)`,
      { kind: "notice", streaming: false },
    );
    thinkingOpen = false;
    thinkingChars = 0;
  }
}

function handle(ev: { type: string;[k: string]: unknown }) {
  switch (ev.type) {
    case "tokenDelta": {
      closeThinking();
      assistantBuf += String(ev.text ?? "");
      if (assistantMsgId < 0) {
        assistantMsgId = push("assistant", assistantBuf, { streaming: true });
      } else {
        update(assistantMsgId, assistantBuf, { streaming: true });
      }
      break;
    }
    case "reasoningDelta": {
      if (!thinkingOpen) {
        thinkingMsgId = push("thinking", "✻ thinking…", { streaming: true });
        thinkingOpen = true;
        thinkingChars = 0;
      }
      thinkingChars += String(ev.text ?? "").length;
      update(thinkingMsgId, `✻ thinking… (${thinkingChars} chars)`, { streaming: true });
      break;
    }
    case "toolCallStarted": {
      closeThinking();
      endAssistant();
      openToolId = push("tool", `⚙ ${ev.name} ─ ${ev.preview}`, { streaming: true });
      break;
    }
    case "toolCallFinished": {
      const ok = Boolean(ev.ok);
      if (openToolId >= 0) {
        update(openToolId, `${ok ? "✓" : "✗"} ${ev.name} ─ ${ev.summary}`, {
          streaming: false,
          ok,
        });
        openToolId = -1;
      } else {
        push(ok ? "notice" : "error", `${ok ? "✓" : "✗"} ${ev.name} ─ ${ev.summary}`);
      }
      break;
    }
    case "approvalRequired": {
      closeThinking();
      endAssistant();
      busy = false;
      push("approval", `⚠ approval required — ${ev.tool}`, {
        approvalId: Number(ev.id),
        canPersist: Boolean(ev.canPersist),
        suggestedRule: (ev.suggestedRule as string | null) ?? null,
        bashCommand: (ev.bashCommand as string | null) ?? null,
        text:
          `⚠ approval required — ${ev.tool}\n` +
          `input: ${ev.inputPreview}` +
          (ev.detailPreview ? `\n${ev.detailPreview}` : ""),
      });
      break;
    }
    case "usageUpdated":
      break;
    case "statusNote":
      push("notice", String(ev.text ?? ""));
      break;
    case "turnCompleted":
    case "turnAborted":
      closeThinking();
      endAssistant();
      busy = false;
      break;
    case "error":
      closeThinking();
      endAssistant();
      busy = false;
      push("error", `ERROR: ${ev.message}`);
      break;
  }
}

export function submitLocal(text: string) {
  closeThinking();
  endAssistant();
  push("user", text);
}

export function setBusy(v: boolean) {
  busy = v;
  emitChange();
}

export async function initEvents() {
  await listen<{ type: string;[k: string]: unknown }>("appEvent", (e) =>
    handle(e.payload),
  );
}
