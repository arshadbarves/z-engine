import { submit } from "./commands";
import { hydrateNewSession } from "./sessionOpen";
import { getSnap } from "./sessionSnaps";
import {
  approvalGateStore, busyStore, handleEvent, pushToast, queueStore,
  sessionStore, submitOnSession,
} from "./runtime";
import { workspaceStore } from "./workspaces";
import { markTaskReportsPending } from "./runtime/taskReportRefresh";

let creatingSession: Promise<string> | null = null;

async function recordedSession(): Promise<string> {
  const current = sessionStore.getSnapshot();
  if (current && current !== "boot") return current;
  if (!creatingSession) {
    creatingSession = (async () => {
      const created = await hydrateNewSession(workspaceStore.getSnapshot().active);
      if (!created?.ulid || !created.path) throw new Error("A recorded session could not be created");
      return created.ulid;
    })().finally(() => { creatingSession = null; });
  }
  return creatingSession;
}

/** Every model-facing entry point must use an explicit recorder-backed session. */
export async function submitTask(text: string, images: string[] = [], sessionId?: string): Promise<boolean> {
  let id = sessionId ?? "";
  let submitted = false;
  try {
    if (!id || id === "boot") id = await recordedSession();
    const visible = id === sessionStore.getSnapshot();
    const parked = visible ? undefined : getSnap(id);
    const waiting = visible
      ? busyStore.getSnapshot() || approvalGateStore.getSnapshot() > 0
      : parked?.busy || (parked?.pendingApprovals ?? 0) > 0;
    if (waiting) {
      if (visible) queueStore.push(text, images);
      else if (parked) parked.queue = [...parked.queue, { text, images }];
      return true;
    }
    if (visible) markTaskReportsPending();
    submitOnSession(id, text, images);
    submitted = true;
    await submit(text, images, id);
    return true;
  } catch (error) {
    console.error(error);
    const message = String(error).replace("Error: ", "");
    if (submitted) handleEvent({ type: "error", sessionId: id, message });
    else pushToast(message, "warn");
    return false;
  }
}
