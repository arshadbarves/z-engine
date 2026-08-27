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
  if (live) {
    try {
      await startSession(path, root ?? null);
    } catch (e) {
      console.error("session switch failed:", e);
      pushToast("Could not switch to this chat", "warn");
    }
    return;
  }
  const gen = beginHydrate();
  try {
    const result = await startSession(path, root ?? null);
    replaySession(((result?.events ?? []) as ReplayEvent[]));
    applyUsageFromTranscript();
  } catch (e) {
    console.error("session replay failed:", e);
    pushToast("Could not restore this chat", "warn");
  } finally {
    window.setTimeout(() => endHydrate(gen), 32);
  }
}

export async function hydrateNewSession(root: string | null): Promise<void> {
  const gen = beginHydrate();
  parkCurrentAndReset();
  resetUsage();
  try {
    const result = await startSession(null, root);
    const ulid = result?.ulid;
    if (ulid && ulid !== sessionStore.getSnapshot()) activateSession(ulid);
  } catch (e) {
    console.error(e);
    pushToast("Could not start a new chat", "warn");
  } finally {
    window.setTimeout(() => endHydrate(gen), 32);
  }
}
