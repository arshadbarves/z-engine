import { describe, expect, it } from "vitest";
import { evidenceAnchorId, safeAnchorSlug } from "./evidenceAnchor";

describe("safeAnchorSlug", () => {
  it("keeps alphanumeric input untouched", () => {
    expect(safeAnchorSlug("check01")).toBe("check01");
  });

  it("escapes every character that is unsafe in an id or fragment", () => {
    expect(safeAnchorSlug("a b")).toBe("a_20_b");
    expect(safeAnchorSlug("cargo test::lib")).toBe("cargo_20_test_3a__3a_lib");
    expect(safeAnchorSlug("#frag?q=1")).toBe("_23_frag_3f_q_3d_1");
    expect(safeAnchorSlug("café")).toBe("caf_e9_");
  });

  it("never collides for inputs that differ only in punctuation", () => {
    expect(safeAnchorSlug("a_b")).not.toBe(safeAnchorSlug("a-b"));
    expect(safeAnchorSlug("a.b")).not.toBe(safeAnchorSlug("a-b"));
    expect(safeAnchorSlug("a_5f_b")).not.toBe(safeAnchorSlug("a_b"));
  });

  it("is deterministic and emits only id-safe characters", () => {
    const value = "check 1 / ✅ [stdout]";
    expect(safeAnchorSlug(value)).toBe(safeAnchorSlug(value));
    expect(safeAnchorSlug(value)).toMatch(/^[A-Za-z0-9_]*$/);
  });

  it("returns an empty slug for empty input", () => {
    expect(safeAnchorSlug("")).toBe("");
  });
});

describe("evidenceAnchorId", () => {
  it("joins the prefix with the escaped evidence id", () => {
    expect(evidenceAnchorId("task-7-evidence", "check 1")).toBe("task-7-evidence-check_20_1");
  });

  it("stays stable and distinct across evidence ids", () => {
    expect(evidenceAnchorId("p", "a b")).not.toBe(evidenceAnchorId("p", "a-b"));
    expect(evidenceAnchorId("p", "a")).toBe(evidenceAnchorId("p", "a"));
  });
});
