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

/** Natural human-friendly title display for sessions in sidebar. */
export function humanSessionTitle(title: string | null | undefined): string {
  const t = title?.trim();
  if (!t || t === "(empty)") return "New Conversation";
  if (/^\d+$/.test(t)) return `Chat #${t}`;
  return t;
}

/** Formats session timestamp to human friendly relative or date string. */
export function formatSessionTime(ms: number): string {
  if (!ms || isNaN(ms)) return "";
  const d = new Date(ms);
  const now = Date.now();
  const diffMs = now - ms;
  if (diffMs < 60_000) return "Just now";
  if (diffMs < 3_600_000) return `${Math.floor(diffMs / 60_000)}m ago`;

  const today = new Date();
  today.setHours(0, 0, 0, 0);
  if (ms >= today.getTime()) {
    return d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  }

  const yesterday = new Date(today.getTime() - 86_400_000);
  if (ms >= yesterday.getTime()) {
    return "Yesterday";
  }

  if (diffMs < 7 * 86_400_000) {
    return d.toLocaleDateString([], { weekday: "short" });
  }

  return d.toLocaleDateString([], { month: "short", day: "numeric" });
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
