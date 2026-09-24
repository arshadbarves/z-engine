/** Title-bar companion chrome: whether the Now card is open, and when each chat's activity was last looked at. */
class CompanionStore {
  open = $state(false);
  #seen = $state.raw<Record<string, number>>({});

  seenAt(sessionId: string | null): number {
    return sessionId ? (this.#seen[sessionId] ?? 0) : 0;
  }

  markSeen(sessionId: string | null, at = Date.now()) {
    if (sessionId) this.#seen = { ...this.#seen, [sessionId]: at };
  }
}

export const companion = new CompanionStore();
