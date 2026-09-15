import { parkedEntries } from "../sessionSnaps";
import { resetShell, startShell } from "../shellStore";
import type { Msg, MsgKind, Toast, ToastAction } from "../types";
import {
  attachmentStore,
  draftStore,
  emitApprovalGate,
  emitChange,
  emitHydrate,
  emitQueue,
  emitToasts,
  rt,
} from "./state";

function parked() {
  return parkedEntries();
}

function notify() {
  emitChange(parked());
}

/** Last `n` lines of accumulated tool output for the live tail. */
export function tailLines(output: string, n = 10): string[] {
  const lines = output.replace(/\n$/, "").split("\n");
  return lines.slice(Math.max(0, lines.length - n));
}

export function dismissToast(id: number) {
  const hit = rt.toasts.find((x) => x.id === id);
  if (hit?.onDismiss) {
    hit.onDismiss();
  }
  rt.toasts = rt.toasts.filter((x) => x.id !== id);
  emitToasts();
}

export type ToastInput = string | (Partial<Toast> & { text: string });

export function pushToast(input: ToastInput, toneArg: Toast["tone"] = "info") {
  if (rt.emitPaused) return;

  let text = "";
  let title: string | undefined;
  let tag: string | undefined;
  let tone: Toast["tone"] = toneArg;
  let actions: ToastAction[] | undefined;
  let onDismiss: (() => void) | undefined;

  if (typeof input === "string") {
    text = input;
    tone = toneArg;

    if (text.includes(" · ")) {
      const parts = text.split(" · ");
      title = parts[0]?.trim();
      tag = parts[1]?.trim();
    } else if (text.includes(" ─ ")) {
      const parts = text.split(" ─ ");
      title = parts[0]?.trim();
      tag = parts[1]?.trim();
    }
  } else {
    text = input.text;
    title = input.title;
    tag = input.tag;
    tone = input.tone ?? toneArg;
    actions = input.actions;
    onDismiss = input.onDismiss;
  }

  const id = rt.nextToastId++;
  const t: Toast = { id, text, title, tag, tone, actions, onDismiss };
  rt.toasts = [...rt.toasts.slice(-3), t];
  emitToasts();

  const life = tone === "warn" || tone === "error" ? 6000 : 4500;
  setTimeout(() => {
    rt.toasts = rt.toasts.filter((x) => x.id !== t.id);
    emitToasts();
  }, life);
  return id;
}

export function pushNotice(text: string) {
  pushToast(text, "info");
}

export function setMaxTokens(max: number) {
  if (max > 0) rt.usage = { ...rt.usage, maxTokens: max };
  notify();
}

export function resetUsage() {
  rt.usage = { promptTokens: 0, completionTokens: 0, maxTokens: rt.usage.maxTokens };
  notify();
}

export function setUsageTokens(prompt: number, completion: number) {
  rt.usage = { ...rt.usage, promptTokens: prompt, completionTokens: completion };
  notify();
}

export function push(kind: MsgKind, text: string, extra?: Partial<Msg>): number {
  const id = rt.nextId++;
  rt.messages = [...rt.messages, { id, kind, text, ...extra }];
  notify();
  return id;
}

export function update(id: number, extra: Partial<Msg>) {
  rt.messages = rt.messages.map((m: Msg) => (m.id === id ? { ...m, ...extra } : m));
  notify();
}

export function patch(id: number, extra: Partial<Msg>): void {
  rt.messages = rt.messages.map((m) => (m.id === id ? { ...m, ...extra } : m));
}

export function latestOpenTool(name: string): Msg | null {
  for (let i = rt.messages.length - 1; i >= 0; i--) {
    const m = rt.messages[i];
    if (m.kind === "tool" && m.streaming && m.toolName === name) return m;
  }
  return null;
}

export function endAssistant() {
  if (rt.assistantMsgId >= 0) {
    update(rt.assistantMsgId, { text: rt.assistantBuf, streaming: false });
    rt.assistantBuf = "";
    rt.assistantMsgId = -1;
  }
}

export function closeThinking() {
  if (rt.thinkingOpen) {
    const body =
      rt.messages.find((m) => m.id === rt.thinkingMsgId)?.thinkingBody?.length ?? 0;
    update(rt.thinkingMsgId, {
      text: `✻ thought (${body} chars)`,
      streaming: false,
      collapsed: true,
    });
    rt.thinkingOpen = false;
  }
}

export function submitLocal(text: string, images: string[] = []) {
  closeThinking();
  endAssistant();
  push("user", text, { runTurn: rt.runTurnCounter++, images });
}

export function trimTranscript(keepTurn: number) {
  closeThinking();
  endAssistant();
  if (keepTurn <= 0) {
    rt.messages = [];
    rt.runTurnCounter = 0;
    notify();
    return;
  }
  let cut = -1;
  for (let i = 0; i < rt.messages.length; i++) {
    const m = rt.messages[i];
    if (m.kind === "user" && m.runTurn === keepTurn) {
      cut = i;
      break;
    }
  }
  if (cut >= 0) {
    rt.messages = rt.messages.slice(0, cut);
  } else {
    rt.messages = rt.messages.filter(
      (m) => typeof m.runTurn !== "number" || m.runTurn < keepTurn,
    );
  }
  const validTurns = new Set(
    rt.messages.filter((m) => m.kind === "user").map((m) => m.runTurn),
  );
  rt.messages = rt.messages.filter((m) => {
    if (m.kind === "task") {
      return typeof m.runTurn === "number" && validTurns.has(m.runTurn);
    }
    return true;
  });
  rt.runTurnCounter = keepTurn;
  notify();
}

export function commandLocal(cmd: string) {
  startShell(cmd);
}

export function setBusy(v: boolean) {
  if (v && !rt.busy) rt.turnStartedAt = Date.now();
  rt.busy = v;
  notify();
}

export function resolveApproval(
  approvalId: number,
  decision: "once" | "session" | "persist" | "deny",
) {
  const m = rt.messages.find(
    (x) => x.kind === "approval" && x.approvalId === approvalId,
  );
  if (!m) return;
  if (rt.pendingApprovals > 0) rt.pendingApprovals--;
  if (rt.pendingApprovals === 0) rt.busy = true;
  const rule = m.suggestedRule ?? m.bashCommand ?? "";
  const text =
    decision === "deny"
      ? `✗ denied · ${m.toolName ?? "call"}`
      : decision === "once"
        ? `✓ approved (once) · ${m.bashCommand ?? m.toolName ?? ""}`.trimEnd()
        : `${decision === "persist" ? "✓ persisted rule" : "✓ approved · session rule"} '${rule}'`;
  update(m.id, { kind: "notice", text, streaming: false });
  emitApprovalGate(parked());
}

export function resetTranscript() {
  rt.messages = [];
  rt.nextId = 1;
  rt.assistantBuf = "";
  rt.assistantMsgId = -1;
  rt.thinkingOpen = false;
  rt.thinkingMsgId = -1;
  rt.runTurnCounter = 0;
  rt.busy = false;
  rt.turnStartedAt = 0;
  if (rt.pendingApprovals !== 0) {
    rt.pendingApprovals = 0;
    emitApprovalGate(parked());
  }
  if (rt.queue.length > 0) {
    rt.queue = [];
    emitQueue();
  }
  if (rt.draft) draftStore.set("");
  attachmentStore.clear();
  resetShell();
  notify();
}

export function beginHydrate(): number {
  rt.hydrateLock = true;
  rt.hydrating = true;
  rt.hydrateGen += 1;
  emitHydrate();
  return rt.hydrateGen;
}

export function endHydrate(gen?: number) {
  if (gen != null && gen !== rt.hydrateGen) return;
  rt.hydrateLock = false;
  rt.hydrating = false;
  emitHydrate();
}
