import { describe, expect, it } from "vitest";
import {
  estimateCost,
  filterSessions,
  fmtCost,
  fmtTokens,
  groupSessions,
  relTime,
  type SessionEntry,
} from "./util";
import { SLASH_COMMANDS, filterSlash } from "./slash";
import { activeAtToken, stripAtToken } from "./atFile";

const NOW = new Date("2026-08-25T15:00:00").getTime();

function sess(
  path: string,
  modifiedMs: number,
  firstUserMsg: string | null = "hello",
): SessionEntry {
  return { path, ulid: path, firstUserMsg, modifiedMs };
}

describe("sessions", () => {
  it("buckets today / yesterday / date", () => {
    const t = (h: number, dayOffset = 0) =>
      sess(`s${dayOffset}-${h}`, NOW - dayOffset * 86_400_000 - h * 3_600_000);
    const groups = groupSessions([t(1), t(2), t(1, 1), t(5, 9)], NOW);
    expect(groups.map((g) => g.label)).toEqual([
      "Today",
      "Yesterday",
      "Aug 16, 2026",
    ]);
  });

  it("filters by preview and ulid", () => {
    const list = [
      sess("01ABC", NOW, "Fix the failing test"),
      sess("02DEF", NOW, "Add input validation"),
    ];
    expect(filterSessions(list, "failing")).toHaveLength(1);
    expect(filterSessions(list, "02de")).toHaveLength(1);
    expect(filterSessions(list, "")).toHaveLength(2);
  });

  it("relTime buckets", () => {
    expect(relTime(NOW - 30_000, NOW)).toBe("now");
    expect(relTime(NOW - 120_000, NOW)).toBe("2m");
    expect(relTime(NOW - 7_200_000, NOW)).toBe("2h");
    expect(relTime(NOW - 172_800_000, NOW)).toBe("2d");
  });
});

describe("formatting", () => {
  it("fmtTokens", () => {
    expect(fmtTokens(940)).toBe("940");
    expect(fmtTokens(12_345)).toBe("12k");
    expect(fmtTokens(1_234)).toBe("1.2k");
    expect(fmtTokens(1_234_567)).toBe("1.23M");
  });

  it("estimateCost + fmtCost", () => {
    const p = { usdPerMtokInput: 3, usdPerMtokOutput: 15 };
    expect(estimateCost(p, 1_000_000, 100_000)).toBeCloseTo(4.5);
    expect(estimateCost(null, 1000, 100)).toBeNull();
    expect(fmtCost(null)).toBe("–");
    expect(fmtCost(0.0012)).toBe("$0.0012");
    expect(fmtCost(1.234)).toBe("$1.23");
  });
});

describe("slash commands", () => {
  it("returns null for non-slash input", () => {
    expect(filterSlash("fix the test")).toBeNull();
  });
  it("returns all on bare slash", () => {
    expect(filterSlash("/")).toEqual(SLASH_COMMANDS);
  });
  it("filters by prefix", () => {
    expect(filterSlash("/com")).toEqual([{ name: "compact", desc: expect.any(String) }]);
    expect(filterSlash("/zzz")).toEqual([]);
  });
});

describe("@file token", () => {
  it("detects active token at caret", () => {
    expect(activeAtToken("see @src/li", 11)).toBe("src/li");
    expect(activeAtToken("@", 1)).toBe("");
    expect(activeAtToken("plain text", 10)).toBeNull();
    // @ mid-word is not a file query
    expect(activeAtToken("email@x", 7)).toBeNull();
  });

  it("strips the token when the pick becomes a chip", () => {
    const r = stripAtToken("use @src", 8);
    expect(r.text).toBe("use ");
    expect(r.caret).toBe(4);
    // preserves trailing text after the caret
    const r2 = stripAtToken("use @src rest", 8);
    expect(r2.text).toBe("use  rest");
    expect(r2.caret).toBe(4);
    // no active token → unchanged
    const r3 = stripAtToken("plain", 5);
    expect(r3.text).toBe("plain");
    expect(r3.caret).toBe(5);
  });
});
