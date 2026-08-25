import { listen } from "@tauri-apps/api/event";
import { writable, get } from "svelte/store";

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
  /** live-updated while streaming */
  streaming?: boolean;
  ok?: boolean;
}

export const messages = writable<Msg[]>([]);
export const busy = writable(false);

let nextId = 1;
let assistantBuf = "";
let thinkingOpen = false;
let thinkingChars = 0;

function push(kind: MsgKind, text: string, extra?: Partial<Msg>): number {
  const id = nextId++;
  messages.update((m) => [...m, { id, kind, text, ...extra }]);
  return id;
}
function update(id: number, text: string, extra?: Partial<Msg>) {
  messages.update((m) => m.map((x) => (x.id === id ? { ...x, text, ...extra } : x)));
}

function closeThinking() {
  if (thinkingOpen) {
    update(thinkingMsgId, `✻ thinking collapsed (${thinkingChars} chars)`, {
      kind: "notice",
    });
    thinkingOpen = false;
    thinkingChars = 0;
  }
}

let thinkingMsgId = -1;

type AppEvent = { type: string; [k: string]: unknown };

function handle(ev: AppEvent) {
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
      push("tool", `⚙ ${ev.name} ─ ${ev.preview}`);
      break;
    }
    case "toolCallFinished": {
      const ok = Boolean(ev.ok);
      push(ok ? "notice" : "error", `${ok ? "✓" : "✗"} ${ev.name} ─ ${ev.summary}`);
      break;
    }
    case "approvalRequired": {
      closeThinking();
      endAssistant();
      push(
        "approval",
        `⚠ approval required: ${ev.tool}\n${ev.inputPreview}` +
          (ev.bashCommand ? `\ncommand: ${ev.bashCommand}` : ""),
      );
      break;
    }
    case "usageUpdated":
      break; // meter handled via separate store later
    case "statusNote":
      push("notice", String(ev.text ?? ""));
      break;
    case "turnCompleted":
      closeThinking();
      endAssistant();
      busy.set(false);
      break;
    case "turnAborted":
      closeThinking();
      endAssistant();
      busy.set(false);
      break;
    case "error":
      closeThinking();
      endAssistant();
      busy.set(false);
      push("error", `ERROR: ${ev.message}`);
      break;
  }
}

let assistantMsgId = -1;
function endAssistant() {
  if (assistantMsgId >= 0) {
    update(assistantMsgId, assistantBuf, { streaming: false });
    assistantBuf = "";
    assistantMsgId = -1;
  }
}

export function submitLocal(text: string) {
  closeThinking();
  endAssistant();
  push("user", text);
}

export async function initEvents() {
  await listen<AppEvent>("appEvent", (e) => handle(e.payload));
}

// silence unused warnings for helpers used across phases
void get;
