import { recentActivity } from "./activity";
import { unreadSessionOutcome } from "./sessionOutcome";
import type { SessionActivity, UnreadMark } from "./sessions";
import type { SessionView } from "./sessionView/types";

/** Something a chat is blocked on until the user answers. */
export interface NeedsYouItem {
  key: string;
  sessionId: string;
  chatTitle: string;
  kind: "approval" | "question" | "plan" | "trust";
  title: string;
  detail: string | null;
  /** Set for approvals, which the inbox can answer in place. */
  requestId: string | null;
}

export interface FinishedItem {
  sessionId: string;
  chatTitle: string;
  label: string;
  tone: "ok" | "neutral" | "danger";
  at: number;
}

/** A passing notice (toast) as the inbox keeps it: in full. */
export interface RecordedNotice {
  key: string;
  tone: "info" | "ok" | "warn" | "error";
  title: string;
  text: string;
  sessionId: string | null;
  at: number;
}

export interface InboxNotice extends RecordedNotice {
  chatTitle: string | null;
}

type TitleOf = (sessionId: string) => string;

export function needsYou(views: Record<string, SessionView>, titleOf: TitleOf): NeedsYouItem[] {
  const items: NeedsYouItem[] = [];
  for (const view of Object.values(views)) {
    const id = view.sessionId;
    const base = { sessionId: id, chatTitle: titleOf(id) };
    for (const a of Object.values(view.approvals)) {
      items.push({ ...base, key: `${id}:${a.requestId}`, kind: "approval", title: a.title, detail: a.reason || null, requestId: a.requestId });
    }
    for (const q of Object.values(view.questions)) {
      const first = q.questions[0];
      items.push({ ...base, key: `${id}:${q.requestId}`, kind: "question", title: first?.question ?? "A question for you", detail: null, requestId: null });
    }
    for (const p of Object.values(view.plans)) {
      items.push({ ...base, key: `${id}:${p.requestId}`, kind: "plan", title: "Plan ready for review", detail: null, requestId: null });
    }
    if (view.trustRequest) {
      items.push({
        ...base,
        key: `${id}:trust`,
        kind: "trust",
        title: "Trust this project?",
        detail: view.trustRequest.defines.length ? `Held back: ${view.trustRequest.defines.join(", ")}` : null,
        requestId: null,
      });
    }
  }
  return items;
}

const FINISHED_TONE = { verified: "ok", warn: "danger", neutral: "neutral" } as const;

/** Chats whose turn ended in the background and were not opened since, newest first. */
export function finishedChats(
  unread: Record<string, UnreadMark>,
  activity: Record<string, SessionActivity>,
  titleOf: TitleOf,
): FinishedItem[] {
  const items: FinishedItem[] = [];
  for (const [sessionId, mark] of Object.entries(unread)) {
    const outcome = unreadSessionOutcome(mark, false, activity[sessionId] ?? null);
    if (!outcome) continue;
    items.push({ sessionId, chatTitle: titleOf(sessionId), label: outcome.label, tone: FINISHED_TONE[outcome.tone], at: mark.at });
  }
  return items.sort((a, b) => b.at - a.at);
}

/** Every notice worth keeping: passing ones plus each chat's warnings, blocked hooks and errors, newest first. */
export function inboxNotices(recorded: RecordedNotice[], views: Record<string, SessionView>, titleOf: TitleOf): InboxNotice[] {
  const fromChats: InboxNotice[] = Object.values(views).flatMap((view) =>
    recentActivity(view, 50).map((item) => ({
      key: `${view.sessionId}:${item.key}`,
      tone: item.tone,
      title: item.text,
      text: item.text,
      sessionId: view.sessionId,
      chatTitle: titleOf(view.sessionId),
      at: item.at,
    })),
  );
  const passing = recorded.map((n) => ({ ...n, chatTitle: n.sessionId ? titleOf(n.sessionId) : null }));
  return [...fromChats, ...passing].sort((a, b) => b.at - a.at);
}

/** The sidebar's Inbox badge: what needs you, results not yet seen, and problems since the inbox was last opened. */
export function inboxCount(input: { needsYou: number; finished: number; notices: InboxNotice[]; readAt: number }): number {
  const problems = input.notices.filter((n) => (n.tone === "warn" || n.tone === "error") && n.at > input.readAt).length;
  return input.needsYou + input.finished + problems;
}
