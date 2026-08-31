import type { SessionSnap } from "../sessionSnaps";
import { emptySnap } from "../sessionSnaps";
import type {
  Listener,
  Msg,
  QueuedMessage,
  SessionActivity,
  Toast,
  Usage,
} from "../types";

export const rt = {
  messages: [] as Msg[],
  busy: false,
  usage: { promptTokens: 0, completionTokens: 0, maxTokens: 120_000 } as Usage,
  model: "",
  mode: "normal",
  draft: "",
  attachments: [] as string[],
  sessionId: "",
  toasts: [] as Toast[],
  queue: [] as QueuedMessage[],
  pendingApprovals: 0,
  sessionsTick: 0,
  hydrating: false,
  hydrateLock: false,
  hydrateGen: 0,
  emitPaused: false,
  nextId: 1,
  nextToastId: 1,
  assistantBuf: "",
  assistantMsgId: -1,
  thinkingOpen: false,
  thinkingMsgId: -1,
  runTurnCounter: 0,
  turnStartedAt: 0,
  eventsInitialized: false,
};

let activityCache: Record<string, SessionActivity> = {};

const listeners = new Set<Listener>();
const modelSubs = new Set<Listener>();
const toastListeners = new Set<Listener>();
const workingSubs = new Set<Listener>();
const approvalGateSubs = new Set<Listener>();
const queueListeners = new Set<Listener>();
const sessionsTickSubs = new Set<Listener>();
const modeSubs = new Set<Listener>();
const draftSubs = new Set<Listener>();
const attachmentSubs = new Set<Listener>();
const sessionSubs = new Set<Listener>();
const hydrateSubs = new Set<Listener>();

function sameActivity(
  a: Record<string, SessionActivity>,
  b: Record<string, SessionActivity>,
): boolean {
  const aKeys = Object.keys(a);
  return (
    aKeys.length === Object.keys(b).length && aKeys.every((k) => a[k] === b[k])
  );
}

export function collectActivity(
  parked: Iterable<[string, SessionSnap]>,
): Record<string, SessionActivity> {
  const map: Record<string, SessionActivity> = {};
  if (rt.sessionId) {
    if (rt.pendingApprovals > 0) map[rt.sessionId] = "approval";
    else if (rt.busy) map[rt.sessionId] = "working";
  }
  for (const [id, s] of parked) {
    if (s.pendingApprovals > 0) map[id] = "approval";
    else if (s.busy) map[id] = "working";
  }
  return map;
}

export function syncWorking(parked: Iterable<[string, SessionSnap]>) {
  const next = collectActivity(parked);
  if (sameActivity(activityCache, next)) return;
  activityCache = next;
  for (const l of workingSubs) l();
}

export function emitChange(parked: Iterable<[string, SessionSnap]>) {
  if (rt.emitPaused) return;
  for (const l of listeners) l();
  syncWorking(parked);
}

export function emitToasts() {
  for (const l of toastListeners) l();
}

export function emitApprovalGate(parked: Iterable<[string, SessionSnap]>) {
  if (rt.emitPaused) return;
  for (const l of approvalGateSubs) l();
  syncWorking(parked);
}

export function emitQueue() {
  if (rt.emitPaused) return;
  for (const l of queueListeners) l();
}

export function emitHydrate() {
  for (const l of hydrateSubs) l();
}

export function emitModel() {
  for (const l of modelSubs) l();
}

export function emitMode() {
  for (const l of modeSubs) l();
}

export function emitDraft() {
  for (const l of draftSubs) l();
}

export function emitAttachments() {
  for (const l of attachmentSubs) l();
}

export function emitSession() {
  for (const l of sessionSubs) l();
}

export function bumpSessionsTick() {
  rt.sessionsTick += 1;
  for (const l of sessionsTickSubs) l();
}

export function snapshot(): SessionSnap {
  return {
    messages: rt.messages,
    nextId: rt.nextId,
    assistantBuf: rt.assistantBuf,
    assistantMsgId: rt.assistantMsgId,
    thinkingOpen: rt.thinkingOpen,
    thinkingMsgId: rt.thinkingMsgId,
    runTurnCounter: rt.runTurnCounter,
    busy: rt.busy,
    turnStartedAt: rt.turnStartedAt,
    pendingApprovals: rt.pendingApprovals,
    queue: rt.queue,
    usage: rt.usage,
  };
}

export function loadSnap(s: SessionSnap) {
  rt.messages = s.messages;
  rt.nextId = s.nextId;
  rt.assistantBuf = s.assistantBuf;
  rt.assistantMsgId = s.assistantMsgId;
  rt.thinkingOpen = s.thinkingOpen;
  rt.thinkingMsgId = s.thinkingMsgId;
  rt.runTurnCounter = s.runTurnCounter;
  rt.busy = s.busy;
  rt.turnStartedAt = s.turnStartedAt;
  rt.pendingApprovals = s.pendingApprovals;
  rt.queue = s.queue;
  rt.usage = s.usage;
}

export function resetActivityCache() {
  activityCache = {};
}

export const transcriptStore = {
  subscribe(l: Listener) {
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  },
  getSnapshot(): Msg[] {
    return rt.messages;
  },
};

export const busyStore = {
  subscribe(l: Listener) {
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  },
  getSnapshot(): boolean {
    return rt.busy;
  },
};

export const approvalGateStore = {
  subscribe(l: Listener) {
    approvalGateSubs.add(l);
    return () => {
      approvalGateSubs.delete(l);
    };
  },
  getSnapshot(): number {
    return rt.pendingApprovals;
  },
};

export const queueStore = {
  subscribe(l: Listener) {
    queueListeners.add(l);
    return () => {
      queueListeners.delete(l);
    };
  },
  getSnapshot(): QueuedMessage[] {
    return rt.queue;
  },
  push(text: string, images: string[] = []) {
    rt.queue = [...rt.queue, { text, images }];
    emitQueue();
  },
  removeAt(i: number) {
    rt.queue = rt.queue.filter((_, j) => j !== i);
    emitQueue();
  },
  shift(): QueuedMessage | undefined {
    const [first, ...rest] = rt.queue;
    if (first === undefined) return undefined;
    rt.queue = rest;
    emitQueue();
    return first;
  },
};

export const usageStore = {
  subscribe(l: Listener) {
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  },
  getSnapshot(): Usage {
    return rt.usage;
  },
};

export const modelStore = {
  subscribe(l: Listener) {
    modelSubs.add(l);
    return () => {
      modelSubs.delete(l);
    };
  },
  getSnapshot(): string {
    return rt.model;
  },
  set(m: string) {
    if (m && m !== rt.model) {
      rt.model = m;
      emitModel();
    }
  },
};

export const toastStore = {
  subscribe(l: Listener) {
    toastListeners.add(l);
    return () => {
      toastListeners.delete(l);
    };
  },
  getSnapshot(): Toast[] {
    return rt.toasts;
  },
};

export const sessionsTickStore = {
  subscribe(l: Listener) {
    sessionsTickSubs.add(l);
    return () => {
      sessionsTickSubs.delete(l);
    };
  },
  getSnapshot(): number {
    return rt.sessionsTick;
  },
};

export const modeStore = {
  subscribe(l: Listener) {
    modeSubs.add(l);
    return () => {
      modeSubs.delete(l);
    };
  },
  getSnapshot(): string {
    return rt.mode;
  },
  set(m: string) {
    if (m && m !== rt.mode) {
      rt.mode = m;
      emitMode();
    }
  },
};

export const draftStore = {
  subscribe(l: Listener) {
    draftSubs.add(l);
    return () => {
      draftSubs.delete(l);
    };
  },
  getSnapshot(): string {
    return rt.draft;
  },
  set(text: string) {
    if (text !== rt.draft) {
      rt.draft = text;
      emitDraft();
    }
  },
};

export const attachmentStore = {
  subscribe(l: Listener) {
    attachmentSubs.add(l);
    return () => {
      attachmentSubs.delete(l);
    };
  },
  getSnapshot(): string[] {
    return rt.attachments;
  },
  add(path: string) {
    if (!path || rt.attachments.includes(path)) return;
    rt.attachments = [...rt.attachments, path];
    emitAttachments();
  },
  remove(path: string) {
    rt.attachments = rt.attachments.filter((p) => p !== path);
    emitAttachments();
  },
  clear() {
    if (rt.attachments.length === 0) return;
    rt.attachments = [];
    emitAttachments();
  },
};

export const sessionStore = {
  subscribe(l: Listener) {
    sessionSubs.add(l);
    return () => {
      sessionSubs.delete(l);
    };
  },
  getSnapshot(): string {
    return rt.sessionId;
  },
};

export const sessionActivityStore = {
  subscribe(l: Listener) {
    workingSubs.add(l);
    return () => {
      workingSubs.delete(l);
    };
  },
  getSnapshot(): Record<string, SessionActivity> {
    return activityCache;
  },
};

export const hydrateStore = {
  subscribe(l: Listener) {
    hydrateSubs.add(l);
    return () => {
      hydrateSubs.delete(l);
    };
  },
  getSnapshot(): boolean {
    return rt.hydrating;
  },
};

export function clearRuntime(maxTokens = 120_000) {
  loadSnap(emptySnap(maxTokens));
  rt.model = "";
  rt.mode = "normal";
  rt.draft = "";
  rt.attachments = [];
  rt.sessionId = "";
  rt.toasts = [];
  rt.nextToastId = 1;
  rt.hydrating = false;
  rt.hydrateLock = false;
  rt.hydrateGen = 0;
  rt.emitPaused = false;
  rt.eventsInitialized = false;
  resetActivityCache();
}
