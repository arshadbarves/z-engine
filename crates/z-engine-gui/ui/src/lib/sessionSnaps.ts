import type { Msg } from "./types";

export interface SessionSnap {
  messages: Msg[];
  nextId: number;
  assistantBuf: string;
  assistantMsgId: number;
  thinkingOpen: boolean;
  thinkingMsgId: number;
  runTurnCounter: number;
  busy: boolean;
  turnStartedAt: number;
  pendingApprovals: number;
  queue: { text: string; images: string[] }[];
  usage: {
    promptTokens: number;
    completionTokens: number;
    maxTokens: number;
  };
}

const parked = new Map<string, SessionSnap>();

export function emptySnap(maxTokens: number): SessionSnap {
  return {
    messages: [],
    nextId: 1,
    assistantBuf: "",
    assistantMsgId: -1,
    thinkingOpen: false,
    thinkingMsgId: -1,
    runTurnCounter: 0,
    busy: false,
    turnStartedAt: 0,
    pendingApprovals: 0,
    queue: [],
    usage: { promptTokens: 0, completionTokens: 0, maxTokens },
  };
}

export function parkSnap(id: string, snap: SessionSnap) {
  if (id) parked.set(id, snap);
}

export function takeSnap(id: string): SessionSnap | undefined {
  const s = parked.get(id);
  if (s) parked.delete(id);
  return s;
}

export function getSnap(id: string): SessionSnap | undefined {
  return parked.get(id);
}

export function hasSnap(id: string): boolean {
  return parked.has(id);
}

export function parkedEntries(): [string, SessionSnap][] {
  return [...parked.entries()];
}

export function clearSnaps() {
  parked.clear();
}
