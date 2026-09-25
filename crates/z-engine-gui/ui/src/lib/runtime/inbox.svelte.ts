import {
  finishedChats,
  inboxCount,
  inboxNotices,
  needsYou,
  type FinishedItem,
  type InboxNotice,
  type NeedsYouItem,
  type RecordedNotice,
} from "../domain/inbox";
import { sessions } from "./sessions.svelte";
import { chatTitle } from "./titles";

/** Notices kept for this app session; older ones fall off. */
const KEEP = 200;

/**
 * The Activity inbox's own memory: every passing notice in full (engine
 * notices live in the session views), and when the inbox was last read.
 */
class InboxStore {
  #recorded = $state.raw<RecordedNotice[]>([]);
  readAt = $state(0);

  get recorded(): RecordedNotice[] {
    return this.#recorded;
  }

  record(notice: RecordedNotice) {
    this.#recorded = [notice, ...this.#recorded].slice(0, KEEP);
  }

  markRead(at = Date.now()) {
    this.readAt = at;
  }
}

export const inbox = new InboxStore();

export interface InboxSnapshot {
  needsYou: NeedsYouItem[];
  finished: FinishedItem[];
  notices: InboxNotice[];
  count: number;
}

/** Everything the inbox shows and the badge count; reactive inside `$derived`. */
export function inboxSnapshot(): InboxSnapshot {
  const waiting = needsYou(sessions.views, chatTitle);
  const finished = finishedChats(sessions.unread, sessions.activity, chatTitle);
  const notices = inboxNotices(inbox.recorded, sessions.views, chatTitle);
  const count = inboxCount({ needsYou: waiting.length, finished: finished.length, notices, readAt: inbox.readAt });
  return { needsYou: waiting, finished, notices, count };
}
