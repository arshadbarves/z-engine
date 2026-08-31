import { clearSnaps } from "../sessionSnaps";
import { resetShell } from "../shellStore";
import { resetTranscript } from "./mutations";
import { resetActivityCache, rt } from "./state";

/** Test hook: reset all module state. */
export function resetForTests() {
  rt.hydrateLock = false;
  rt.hydrateGen = 0;
  rt.hydrating = false;
  rt.turnStartedAt = 0;
  rt.emitPaused = false;
  clearSnaps();
  resetActivityCache();
  resetTranscript();
  rt.busy = false;
  rt.usage = { promptTokens: 0, completionTokens: 0, maxTokens: 120_000 };
  rt.model = "";
  rt.mode = "normal";
  rt.draft = "";
  rt.attachments = [];
  rt.sessionId = "";
  rt.toasts = [];
  rt.nextToastId = 1;
  rt.eventsInitialized = false;
  resetShell();
}
