import { describe, expect, it } from "vitest";
import { groupTranscript } from "./activity";
import { contextBreakdown } from "./contextBreakdown";
import { familyTitle, pathPills, splitWork } from "./toolGroups";
import { activityBrief } from "./toolUi";
import type { Msg } from "./events";

function msg(partial: Partial<Msg> & Pick<Msg, "id" | "kind" | "text">): Msg {
  return partial;
}

describe("groupTranscript", () => {
  it("merges thinking with tools and drops audit notices", () => {
    const blocks = groupTranscript([
      msg({ id: 1, kind: "user", text: "hi" }),
      msg({ id: 2, kind: "command", text: "! ls" }),
      msg({ id: 3, kind: "thinking", text: "plan" }),
      msg({ id: 4, kind: "tool", text: "read", toolName: "read_file" }),
      msg({ id: 5, kind: "tool", text: "edit", toolName: "edit_file" }),
      msg({ id: 6, kind: "notice", text: "✓ approved (once)" }),
      msg({ id: 7, kind: "assistant", text: "ok" }),
      msg({ id: 8, kind: "status", text: "✓ done · 1.2s" }),
    ]);
    expect(blocks.map((b) => b.type)).toEqual(["msg", "work", "msg", "msg"]);
    expect(blocks[1].type === "work" && blocks[1].items).toHaveLength(3);
    expect(blocks[3].type === "msg" && blocks[3].msg.kind).toBe("status");
  });
});

describe("activityBrief", () => {
  it("summarizes finished tools by family", () => {
    const tools: Msg[] = [
      msg({ id: 1, kind: "tool", text: "", toolName: "read_file", durationMs: 400, ok: true }),
      msg({ id: 2, kind: "tool", text: "", toolName: "read_file", durationMs: 200, ok: true }),
      msg({ id: 3, kind: "tool", text: "", toolName: "edit_file", durationMs: 400, ok: true }),
    ];
    expect(activityBrief(tools)).toMatch(/Read 2/);
    expect(activityBrief(tools)).toMatch(/Edit/);
  });

  it("includes thought in the finished brief", () => {
    const items: Msg[] = [
      msg({ id: 0, kind: "thinking", text: "plan" }),
      msg({ id: 1, kind: "tool", text: "", toolName: "read_file", durationMs: 400, ok: true }),
    ];
    expect(activityBrief(items)).toMatch(/^Thought/);
    expect(activityBrief(items)).toMatch(/Read/);
  });

  it("shows the live step while a tool is running", () => {
    const tools: Msg[] = [
      msg({ id: 1, kind: "tool", text: "", toolName: "read_file", ok: true }),
      msg({
        id: 2,
        kind: "tool",
        text: "",
        toolName: "grep",
        preview: "foo",
        streaming: true,
      }),
    ];
    expect(activityBrief(tools)).toBe("2/2 · Grep foo");
  });
});

describe("splitWork", () => {
  it("keeps reasoning lines and groups consecutive reads", () => {
    const parts = splitWork([
      msg({ id: 1, kind: "thinking", text: "plan", thinkingBody: "Start with the docs." }),
      msg({ id: 2, kind: "tool", text: "", toolName: "read_file", preview: "docs/a.md", ok: true }),
      msg({ id: 3, kind: "tool", text: "", toolName: "read_file", preview: "README.md", ok: true }),
      msg({ id: 4, kind: "tool", text: "", toolName: "edit_file", preview: "src/lib.rs", ok: true }),
    ]);
    expect(parts.map((p) => p.type)).toEqual(["reason", "group", "group"]);
    expect(parts[1].type === "group" && parts[1].family).toBe("Read");
    expect(parts[1].type === "group" && parts[1].tools).toHaveLength(2);
    expect(parts[2].type === "group" && parts[2].family).toBe("Edit");
  });

  it("titles a read group like the Cursor card", () => {
    expect(familyTitle("Read", 4)).toBe("Read 4 files");
    expect(familyTitle("Read", 1)).toBe("Read 1 file");
  });

  it("rolls file paths into directory pills", () => {
    expect(pathPills(["docs/a.md", "docs/b.md", ".gitignore", "README.md"])).toEqual([
      { label: "docs/", count: 2 },
      { label: ".gitignore", count: 1 },
      { label: "README.md", count: 1 },
    ]);
  });
});

describe("contextBreakdown", () => {
  it("fills two-column categories instead of only input/output", () => {
    const br = contextBreakdown(
      [
        msg({ id: 1, kind: "user", text: "hello world ".repeat(20) }),
        msg({ id: 2, kind: "assistant", text: "ok ".repeat(10) }),
        msg({ id: 3, kind: "tool", text: "", toolName: "read_file", preview: "a.rs", output: "fn " }),
      ],
      4000,
      100_000,
    );
    expect(br.slices.map((s) => s.id)).toEqual(["system", "tools", "rules", "chat", "files"]);
    expect(br.used).toBeGreaterThan(0);
    expect(br.remaining).toBe(br.max - br.used);
  });
});
