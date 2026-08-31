import {
  emptySnap,
  getSnap,
  hasSnap,
  parkSnap,
  parkedEntries,
  takeSnap,
} from "../sessionSnaps";
import {
  emitApprovalGate,
  emitChange,
  emitQueue,
  emitSession,
  loadSnap,
  queueStore,
  rt,
  snapshot,
  syncWorking,
} from "./state";
import { resetTranscript, setBusy, submitLocal } from "./mutations";

function parked() {
  return parkedEntries();
}

export function hasSessionRuntime(id: string): boolean {
  return Boolean(id) && (id === rt.sessionId || hasSnap(id));
}

/** Switch the visible chat. The previous session stays in memory. */
export function activateSession(ulid: string) {
  if (!ulid || ulid === rt.sessionId) return;
  if (rt.sessionId) parkSnap(rt.sessionId, snapshot());
  const parkedSnap = takeSnap(ulid);
  rt.sessionId = ulid;
  loadSnap(parkedSnap ?? emptySnap(rt.usage.maxTokens));
  emitSession();
  emitChange(parked());
  emitQueue();
  emitApprovalGate(parked());
}

export function parkCurrentAndReset() {
  if (rt.sessionId) parkSnap(rt.sessionId, snapshot());
  rt.sessionId = "";
  loadSnap(emptySnap(rt.usage.maxTokens));
  emitSession();
  emitChange(parked());
  emitQueue();
  emitApprovalGate(parked());
}

/** Queued prompts whose session is idle (active or background). */
export function drainReadyQueues(): { sessionId: string; text: string; images: string[] }[] {
  const out: { sessionId: string; text: string; images: string[] }[] = [];
  if (!rt.busy && rt.pendingApprovals === 0 && rt.queue.length > 0) {
    const next = queueStore.shift();
    if (next && rt.sessionId) {
      out.push({ sessionId: rt.sessionId, text: next.text, images: next.images });
    }
  }
  for (const [id, snap] of parkedEntries()) {
    if (snap.busy || snap.pendingApprovals > 0 || snap.queue.length === 0) continue;
    const next = snap.queue[0];
    snap.queue = snap.queue.slice(1);
    out.push({ sessionId: id, text: next.text, images: next.images });
  }
  if (out.length) emitQueue();
  return out;
}

export function applyToParked(
  sid: string,
  dispatch: (ev: { type: string } & Record<string, unknown>) => void,
  ev: { type: string } & Record<string, unknown>,
) {
  rt.emitPaused = true;
  const saved = snapshot();
  const savedId = rt.sessionId;
  loadSnap(getSnap(sid) ?? emptySnap(rt.usage.maxTokens));
  rt.sessionId = sid;
  dispatch(ev);
  parkSnap(sid, snapshot());
  rt.sessionId = savedId;
  loadSnap(saved);
  rt.emitPaused = false;
  syncWorking(parked());
}

export function submitOnSession(id: string, text: string, images: string[] = []) {
  if (!id || id === rt.sessionId) {
    submitLocal(text, images);
    setBusy(true);
    return;
  }
  rt.emitPaused = true;
  const saved = snapshot();
  const savedId = rt.sessionId;
  loadSnap(getSnap(id) ?? emptySnap(rt.usage.maxTokens));
  rt.sessionId = id;
  submitLocal(text, images);
  setBusy(true);
  parkSnap(id, snapshot());
  rt.sessionId = savedId;
  loadSnap(saved);
  rt.emitPaused = false;
  syncWorking(parked());
}

export function resetVisibleSession() {
  resetTranscript();
}
