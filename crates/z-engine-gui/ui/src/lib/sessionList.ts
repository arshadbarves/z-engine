import type { SessionEntry } from "./util";

/** Sidebar label: untitled chats show as "New chat". */
export function sessionLabel(title: string | null | undefined): string {
  const t = title?.trim();
  return t ? t : "New chat";
}

/** First non-empty line, clipped to 48 characters — matches core fallback_title. */
export function fallbackTitle(prompt: string): string {
  const line =
    prompt
      .split("\n")
      .map((l) => l.trim())
      .find((l) => l.length > 0) ?? prompt.trim();
  const chars = [...line];
  if (chars.length <= 48) return chars.join("");
  return `${chars.slice(0, 48).join("")}…`;
}

/** Insert or replace a session, newest first. */
export function upsertSession(list: SessionEntry[], entry: SessionEntry): SessionEntry[] {
  const rest = list.filter((s) => s.ulid !== entry.ulid);
  return [entry, ...rest].sort((a, b) => Number(b.modifiedMs) - Number(a.modifiedMs));
}

/** Set the display title for one session without dropping the rest. */
export function patchSessionTitle(
  list: SessionEntry[],
  ulid: string,
  title: string,
): SessionEntry[] {
  const now = Date.now();
  return list.map((s) =>
    s.ulid === ulid ? { ...s, firstUserMsg: title, modifiedMs: now } : s,
  );
}

/** Combine a disk listing with in-memory chats so a brand-new session
 * stays visible if `list_sessions` has not caught up yet. Disk wins on
 * conflict so generated titles replace the optimistic placeholder. */
export function mergeSessionLists(
  disk: SessionEntry[],
  current: SessionEntry[],
): SessionEntry[] {
  const byUlid = new Map<string, SessionEntry>();
  for (const s of current) byUlid.set(s.ulid, s);
  for (const s of disk) {
    const mem = byUlid.get(s.ulid);
    const title = s.firstUserMsg ?? mem?.firstUserMsg ?? null;
    const modifiedMs = Math.max(Number(s.modifiedMs), Number(mem?.modifiedMs ?? 0));
    byUlid.set(s.ulid, { ...s, firstUserMsg: title, modifiedMs });
  }
  return [...byUlid.values()].sort((a, b) => Number(b.modifiedMs) - Number(a.modifiedMs));
}

/** First user message becomes the sidebar title until the generated one lands. */
export function applyFirstUserTitle(
  list: SessionEntry[],
  ulid: string,
  messages: Array<{ kind: string; text: string }>,
): SessionEntry[] {
  if (!ulid) return list;
  const current = list.find((s) => s.ulid === ulid);
  if (!current || current.firstUserMsg) return list;
  const user = messages.find((m) => m.kind === "user");
  if (!user?.text.trim()) return list;
  return patchSessionTitle(list, ulid, fallbackTitle(user.text));
}

export function newSessionEntry(
  ulid: string,
  path: string,
  projectRoot: string | null,
): SessionEntry {
  return {
    path,
    ulid,
    firstUserMsg: null,
    modifiedMs: Date.now(),
    projectRoot,
  };
}
