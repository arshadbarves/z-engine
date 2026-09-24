import type { RuntimeEffect } from "../domain/sessions";
import { sessionLabel, viewTitle } from "../domain/sessionList";
import { appendShellOutput } from "../shellStore";
import { openSession } from "./actions";
import { sessionList } from "./sessionList.svelte";
import { sessions } from "./sessions.svelte";
import { pushToast } from "./toasts";

function openFromToast(sessionId: string) {
  const root =
    sessions.view(sessionId)?.info?.projectRoot ??
    sessionList.summaries.find((s) => s.sessionId === sessionId)?.projectRoot ??
    "";
  void openSession(sessionId, root);
}

export function runEffects(effects: RuntimeEffect[]) {
  for (const effect of effects) {
    switch (effect.kind) {
      case "toast":
        pushToast(effect.text, effect.tone);
        break;
      case "shellOutput":
        appendShellOutput(effect.text);
        break;
      case "attention":
        pushToast({
          title: sessionLabel(viewTitle(sessions.view(effect.sessionId) ?? undefined)),
          text: effect.text,
          tone: "warn",
          actions: [{ label: "Open", variant: "primary", onclick: () => openFromToast(effect.sessionId) }],
        });
        break;
      case "refreshSessions":
        sessionList.refreshSoon();
        break;
    }
  }
}
