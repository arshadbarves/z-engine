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
import { ulidFromPath } from "./sessionList";
import { applyToParked } from "./runtime/session";
import { markTaskReportsPending, mergeRefreshedTaskReports } from "./runtime/taskReportRefresh";

function applyUsageFromTranscript() {
  const messages = transcriptStore.getSnapshot();
  setUsageTokens(estimatePromptTokens(messages), estimateCompletionTokens(messages));
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
  const reports = markTaskReportsPending();
  const gen = live ? null : beginHydrate();
  try {
    const result = await startSession(path, root ?? null);
    if (id !== sessionStore.getSnapshot()) {
      applyToParked(id, () => mergeRefreshedTaskReports(result.events, reports), {
        type: "refreshTaskReports",
      });
      return;
    }
    // After restart (and any sessionChanged race) the parked snap is
    // empty — rebuild from JSONL whenever the transcript is still blank.
    if (transcriptStore.getSnapshot().length === 0 && !busyStore.getSnapshot()) {
      replaySession((result?.events ?? []) as ReplayEvent[]);
      applyUsageFromTranscript();
    } else {
      mergeRefreshedTaskReports(result.events, reports);
    }
  } catch (e) {
    console.error("session replay failed:", e);
    pushToast(live ? "Could not switch to this chat" : "Could not restore this chat", "warn");
  } finally {
    if (gen != null) globalThis.setTimeout(() => endHydrate(gen), 32);
  }
}

export async function hydrateNewSession(
  root: string | null,
): Promise<{ ulid: string; path: string } | null> {
  const gen = beginHydrate();
  let startedId = "";
  parkCurrentAndReset();
  resetUsage();
  try {
    const result = await startSession(null, root);
    const ulid = result?.ulid;
    startedId = ulid;
    if (!ulid || ulid === "boot" || !result.path) {
      throw new Error("The backend did not return a recorded session path");
    }
    if (ulid !== sessionStore.getSnapshot()) activateSession(ulid);
    return { ulid, path: result.path };
  } catch (e) {
    console.error(e);
    if (startedId && sessionStore.getSnapshot() === startedId) parkCurrentAndReset();
    pushToast("Could not start a new chat", "warn");
    return null;
  } finally {
    endHydrate(gen);
  }
}
