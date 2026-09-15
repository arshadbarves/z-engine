import { beforeEach, describe, expect, it } from "vitest";
import { handleEvent, resetForTests, resolveApproval, transcriptStore } from "./events";

const msgs = () => transcriptStore.getSnapshot();
beforeEach(() => resetForTests());

describe("approval", () => {
  it("captures id, scopes and diff detailPreview", () => {
    handleEvent({
      type: "approvalRequired",
      id: 7,
      tool: "edit_file",
      inputPreview: "src/lib.rs",
      suggestedRule: null,
      detailPreview: "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new",
      canPersist: true,
      bashCommand: null,
    });
    const m = msgs()[0];
    expect(m.kind).toBe("approval");
    expect(m.approvalId).toBe(7);
    expect(m.canPersist).toBe(true);
    expect(m.detailPreview).toContain("@@ -1");
  });

  it("does not duplicate an approval card with the same id", () => {
    const ev = {
      type: "approvalRequired",
      id: 7,
      tool: "bash",
      inputPreview: "ls",
      suggestedRule: null,
      detailPreview: null,
      canPersist: false,
      bashCommand: "ls",
    };
    handleEvent(ev);
    handleEvent(ev);
    expect(msgs().filter((m) => m.kind === "approval")).toHaveLength(1);
  });
});

describe("approval resolution (A3)", () => {
  it("collapses the card to an approved notice in place", () => {
    handleEvent({
      type: "approvalRequired",
      id: 3,
      tool: "bash",
      inputPreview: "cargo test",
      suggestedRule: "cargo test*",
      detailPreview: null,
      canPersist: true,
      bashCommand: "cargo test",
    });
    resolveApproval(3, "once");
    const m = msgs()[0];
    expect(m.kind).toBe("notice");
    expect(m.text).toContain("✓ approved");
    expect(m.text).toContain("cargo test");
    expect(m.approvalId).toBe(3);
  });

  it("records session/persist scope and denials", () => {
    handleEvent({
      type: "approvalRequired",
      id: 4,
      tool: "bash",
      inputPreview: "rm x",
      suggestedRule: null,
      detailPreview: null,
      canPersist: false,
      bashCommand: "rm x",
    });
    resolveApproval(4, "session");
    expect(msgs()[0].text).toContain("session rule");
    handleEvent({
      type: "approvalRequired",
      id: 5,
      tool: "edit_file",
      inputPreview: "a.rs",
      suggestedRule: null,
      detailPreview: null,
      canPersist: false,
      bashCommand: null,
    });
    resolveApproval(5, "deny");
    expect(msgs()[1].text).toContain("✗ denied");
  });

  it("ignores unknown approval ids", () => {
    resolveApproval(999, "once");
    expect(msgs()).toHaveLength(0);
  });
});
