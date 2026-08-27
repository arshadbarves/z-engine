import { describe, expect, it } from "vitest";
import type { SessionEntry } from "./util";
import {
  fallbackTitle,
  mergeSessionLists,
  patchSessionTitle,
  sessionLabel,
  titledSessions,
  unreadFromEvents,
  upsertSession,
  applyFirstUserTitle,
} from "./sessionList";

function sess(
  ulid: string,
  extra: Partial<SessionEntry> = {},
): SessionEntry {
  return {
    path: `/s/${ulid}.jsonl`,
    ulid,
    firstUserMsg: extra.firstUserMsg ?? null,
    modifiedMs: extra.modifiedMs ?? 1,
    projectRoot: extra.projectRoot ?? "/proj",
  };
}

describe("sessionLabel", () => {
  it("uses the stored title", () => {
    expect(sessionLabel("Fix auth")).toBe("Fix auth");
    expect(sessionLabel(null)).toBe("(empty)");
  });
});

describe("titledSessions", () => {
  it("hides chats that do not have a title yet", () => {
    const list = [
      sess("blank"),
      sess("named", { firstUserMsg: "Fix auth" }),
    ];
    expect(titledSessions(list).map((s) => s.ulid)).toEqual(["named"]);
  });
});

describe("unreadOutcome", () => {
  it("stays until an ack, then clears", () => {
    expect(unreadFromEvents(["completed"])).toBe("completed");
    expect(unreadFromEvents(["completed", "ack"])).toBeNull();
    expect(unreadFromEvents(["completed", "ack", "aborted"])).toBe("aborted");
  });
});

describe("fallbackTitle", () => {
  it("uses the first non-empty line, clipped to 48 chars", () => {
    expect(fallbackTitle("  \nFix the flaky test\nmore")).toBe("Fix the flaky test");
    const long = "a".repeat(80);
    const t = fallbackTitle(`${long}\nmore`);
    expect(t.startsWith("a")).toBe(true);
    expect(t.endsWith("…")).toBe(true);
    expect([...t].length).toBe(49);
  });
});

describe("upsertSession", () => {
  it("inserts a brand-new chat at the top of its workspace list", () => {
    const list = [sess("old", { firstUserMsg: "older", modifiedMs: 10 })];
    const next = upsertSession(
      list,
      sess("new", { firstUserMsg: null, modifiedMs: 20, projectRoot: "/proj" }),
    );
    expect(next.map((s) => s.ulid)).toEqual(["new", "old"]);
    expect(next[0].firstUserMsg).toBeNull();
  });

  it("updates an existing chat in place", () => {
    const list = [sess("a", { firstUserMsg: null, modifiedMs: 1 })];
    const next = upsertSession(list, sess("a", { firstUserMsg: "Hello", modifiedMs: 5 }));
    expect(next).toHaveLength(1);
    expect(next[0].firstUserMsg).toBe("Hello");
  });
});

describe("patchSessionTitle", () => {
  it("fills the title for a new chat once the first message lands", () => {
    const list = [sess("a"), sess("b", { firstUserMsg: "Other" })];
    const next = patchSessionTitle(list, "a", "Fix auth tests");
    expect(next.find((s) => s.ulid === "a")?.firstUserMsg).toBe("Fix auth tests");
    expect(next.find((s) => s.ulid === "b")?.firstUserMsg).toBe("Other");
  });
});

describe("mergeSessionLists", () => {
  it("keeps an optimistic new chat that disk has not listed yet", () => {
    const disk = [sess("old", { firstUserMsg: "older", modifiedMs: 10 })];
    const current = [
      sess("new", { firstUserMsg: null, modifiedMs: 20 }),
      sess("old", { firstUserMsg: "older", modifiedMs: 10 }),
    ];
    const merged = mergeSessionLists(disk, current);
    expect(merged.map((s) => s.ulid)).toEqual(["new", "old"]);
  });

  it("keeps an in-memory title when disk has not recorded it yet", () => {
    const disk = [sess("new", { firstUserMsg: null, modifiedMs: 20 })];
    const current = [sess("new", { firstUserMsg: "Fix auth", modifiedMs: 21 })];
    const merged = mergeSessionLists(disk, current);
    expect(merged[0].firstUserMsg).toBe("Fix auth");
  });

  it("prefers a generated disk title over the fallback", () => {
    const disk = [sess("new", { firstUserMsg: "Fix auth tests", modifiedMs: 30 })];
    const current = [sess("new", { firstUserMsg: "please fix the flaky auth", modifiedMs: 20 })];
    const merged = mergeSessionLists(disk, current);
    expect(merged[0].firstUserMsg).toBe("Fix auth tests");
  });
});

describe("applyFirstUserTitle", () => {
  it("does not overwrite a title that is already set", () => {
    const list = [sess("a", { firstUserMsg: "Existing" })];
    const next = applyFirstUserTitle(list, "a", [{ kind: "user", text: "later prompt" }]);
    expect(next[0].firstUserMsg).toBe("Existing");
  });

  it("fills the title from the first user message", () => {
    const list = [sess("a")];
    const next = applyFirstUserTitle(list, "a", [
      { kind: "assistant", text: "hi" },
      { kind: "user", text: "Fix the flaky auth test" },
    ]);
    expect(next[0].firstUserMsg).toBe("Fix the flaky auth test");
  });

  it("inserts the chat only after the first message, using pending path", () => {
    const next = applyFirstUserTitle(
      [sess("old", { firstUserMsg: "older" })],
      "a",
      [{ kind: "user", text: "Fix the flaky auth test" }],
      { ulid: "a", path: "/s/a.jsonl", projectRoot: "/proj" },
    );
    expect(next.map((s) => s.ulid)).toEqual(["a", "old"]);
    expect(next[0].firstUserMsg).toBe("Fix the flaky auth test");
    expect(next[0].path).toBe("/s/a.jsonl");
  });
});
