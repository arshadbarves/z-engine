import { appendShellLine } from "../shellStore";
import type { EventPayload, Toast } from "../types";
import {
  closeThinking,
  endAssistant,
  latestOpenTool,
  patch,
  push,
  pushToast,
  trimTranscript,
  update,
} from "./mutations";
import { activateSession, applyToParked } from "./session";
import {
  bumpSessionsTick,
  emitApprovalGate,
  emitChange,
  modelStore,
  modeStore,
  rt,
} from "./state";
import { parkedEntries } from "../sessionSnaps";

function parked() {
  return parkedEntries();
}

export function handleEvent(ev: EventPayload) {
  const sid = String(ev.sessionId ?? "");
  if (sid && rt.sessionId && sid !== rt.sessionId) {
    if (sid === "boot") return;
    applyToParked(sid, dispatchEvent, ev);
    return;
  }
  if (rt.hydrateLock && ev.type !== "sessionChanged") return;
  dispatchEvent(ev);
}

export function dispatchEvent(ev: EventPayload) {
  if (rt.hydrateLock && ev.type !== "sessionChanged") return;
  switch (ev.type) {
    case "tokenDelta": {
      closeThinking();
      rt.assistantBuf += String(ev.text ?? "");
      if (rt.assistantMsgId < 0) {
        rt.assistantMsgId = push("assistant", rt.assistantBuf, { streaming: true });
      } else {
        update(rt.assistantMsgId, { text: rt.assistantBuf, streaming: true });
      }
      break;
    }
    case "reasoningDelta": {
      if (!rt.thinkingOpen) {
        rt.thinkingMsgId = push("thinking", "✻ thinking…", {
          streaming: true,
          thinkingBody: "",
          collapsed: false,
        });
        rt.thinkingOpen = true;
      }
      const body =
        (rt.messages.find((m) => m.id === rt.thinkingMsgId)?.thinkingBody ?? "") +
        String(ev.text ?? "");
      update(rt.thinkingMsgId, {
        text: `✻ thinking… (${body.length} chars)`,
        thinkingBody: body,
        streaming: true,
      });
      break;
    }
    case "toggleThinking": {
      const m = rt.messages.find((x) => x.id === Number(ev.id));
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
      const output =
        merged.length > MAX_TOOL_OUTPUT
          ? merged.slice(0, MAX_TOOL_OUTPUT / 2) +
            "\n[…output truncated…]\n" +
            merged.slice(merged.length - MAX_TOOL_OUTPUT / 2)
          : merged;
      patch(open.id, { output });
      emitChange(parked());
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
      if (rt.messages.some((m) => m.kind === "approval" && m.approvalId === approvalId)) {
        break;
      }
      rt.busy = false;
      rt.pendingApprovals++;
      emitApprovalGate(parked());
      push("approval", `⚠ approval required — ${ev.tool}\ninput: ${ev.inputPreview}`, {
        approvalId,
        toolName: String(ev.tool),
        canPersist: Boolean(ev.canPersist),
        suggestedRule: (ev.suggestedRule as string | null) ?? null,
        bashCommand: (ev.bashCommand as string | null) ?? null,
        detailPreview: (ev.detailPreview as string | null) ?? null,
        text: `⚠ approval required — ${ev.tool}\ninput: ${ev.inputPreview}`,
      });
      emitChange(parked());
      break;
    }
    case "usageUpdated":
      rt.usage = {
        ...rt.usage,
        promptTokens: Number(ev.promptTokens ?? 0),
        completionTokens: Number(ev.completionTokens ?? 0),
      };
      emitChange(parked());
      break;
    case "statusNote":
      routeStatusNote(String(ev.text ?? ""));
      break;
    case "turnCompleted":
    case "turnAborted":
      closeThinking();
      endAssistant();
      rt.busy = false;
      if (rt.pendingApprovals !== 0) {
        rt.pendingApprovals = 0;
        emitApprovalGate(parked());
      }
      if (rt.messages.some((m) => m.kind === "approval")) {
        rt.messages = rt.messages.map((m) =>
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
        rt.usage = {
          ...rt.usage,
          promptTokens: Number(ev.promptTokens ?? rt.usage.promptTokens),
          completionTokens: Number(ev.completionTokens ?? rt.usage.completionTokens),
        };
        const ms = rt.turnStartedAt ? Date.now() - rt.turnStartedAt : 0;
        push("status", ms > 0 ? `✓ done · ${(ms / 1000).toFixed(1)}s` : "✓ done", {
          ok: true,
        });
      } else {
        push("status", "■ aborted", { ok: false });
      }
      rt.turnStartedAt = 0;
      bumpSessionsTick();
      emitChange(parked());
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
      rt.busy = false;
      push("error", `ERROR: ${ev.message}`);
      emitChange(parked());
      break;
  }
}

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
