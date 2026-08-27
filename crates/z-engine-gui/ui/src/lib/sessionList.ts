import type { SessionEntry } from "./util";

export interface PendingSession {
  ulid: string;
  path: string;
  projectRoot: string | null;
}

/** Sidebar label for a session that already has a title. */
export function sessionLabel(title: string | null | undefined): string {
  const t = title?.trim();
  return t ? t : "(empty)";
}

/** Drop chats that have not been titled yet — they stay out of the list
 * until the first user message. */
export function titledSessions(list: SessionEntry[]): SessionEntry[] {
  return list.filter((s) => Boolean(s.firstUserMsg?.trim()));
}

/** Last turn-end that has not been opened/acked. */
export function unreadFromEvents(seq: Array<"completed" | "aborted" | "ack">): string | null {
  let last: string | null = null;
  let acked = true;
  for (const ev of seq) {
    if (ev === "ack") acked = true;
    else {
      last = ev;
      acked = false;
    }
  }
  return acked ? null : last;
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

/** First user message becomes the sidebar title until the generated one lands.
 * If the chat is not in the list yet, `pending` supplies path + workspace. */
export function applyFirstUserTitle(
  list: SessionEntry[],
  ulid: string,
  messages: Array<{ kind: string; text: string }>,
  pending?: PendingSession | null,
): SessionEntry[] {
  if (!ulid) return list;
  const user = messages.find((m) => m.kind === "user");
  if (!user?.text.trim()) return list;
  const title = fallbackTitle(user.text);
  const current = list.find((s) => s.ulid === ulid);
  if (current?.firstUserMsg) return list;
  if (current) return patchSessionTitle(list, ulid, title);
  if (!pending || pending.ulid !== ulid || !pending.path) return list;
  return upsertSession(list, {
    ulid,
    path: pending.path,
    projectRoot: pending.projectRoot,
    firstUserMsg: title,
    modifiedMs: Date.now(),
  });
}
