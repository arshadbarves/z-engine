import { startSession } from "./commands";
import {
  estimateCompletionTokens,
  estimatePromptTokens,
} from "./contextBreakdown";
import {
  activateSession,
  beginHydrate,
  busyStore,
  endHydrate,
  parkCurrentAndReset,
  pushToast,
  replaySession,
  resetUsage,
  sessionStore,
  setUsageTokens,
  transcriptStore,
  type ReplayEvent,
} from "./events";

function applyUsageFromTranscript() {
  const messages = transcriptStore.getSnapshot();
  setUsageTokens(estimatePromptTokens(messages), estimateCompletionTokens(messages));
}

function ulidFromPath(path: string): string {
  const base = path.split(/[/\\]/).pop() ?? path;
  return base.replace(/\.[^.]+$/, "");
}

/** Open a chat. If it is already running in the background, just show it —
 * the agent loop keeps going. Cold opens hydrate from disk. */
export async function hydrateOpenSession(
  path: string,
  root?: string | null,
): Promise<void> {
  const id = ulidFromPath(path);
  activateSession(id);
  const live =
    transcriptStore.getSnapshot().length > 0 || busyStore.getSnapshot();
  const gen = live ? null : beginHydrate();
  try {
    const result = await startSession(path, root ?? null);
    // After restart (and any sessionChanged race) the parked snap is
    // empty — rebuild from JSONL whenever the transcript is still blank.
    if (transcriptStore.getSnapshot().length === 0 && !busyStore.getSnapshot()) {
      replaySession((result?.events ?? []) as ReplayEvent[]);
      applyUsageFromTranscript();
    }
  } catch (e) {
    console.error("session replay failed:", e);
    pushToast(live ? "Could not switch to this chat" : "Could not restore this chat", "warn");
  } finally {
    if (gen != null) window.setTimeout(() => endHydrate(gen), 32);
  }
}

export async function hydrateNewSession(
  root: string | null,
): Promise<{ ulid: string; path: string } | null> {
  const gen = beginHydrate();
  parkCurrentAndReset();
  resetUsage();
  try {
    const result = await startSession(null, root);
    const ulid = result?.ulid;
    if (ulid && ulid !== sessionStore.getSnapshot()) activateSession(ulid);
    const path = result?.path ?? "";
    if (ulid && path) return { ulid, path };
    return ulid ? { ulid, path } : null;
  } catch (e) {
    console.error(e);
    pushToast("Could not start a new chat", "warn");
    return null;
  } finally {
    window.setTimeout(() => endHydrate(gen), 32);
  }
}
