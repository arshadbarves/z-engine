import { describe, expect, it } from "vitest";
import { approvalCommand, approvalToolName } from "./approvalPreview";

describe("approvalPreview", () => {
  it("prefers bashCommand over the JSON input dump", () => {
    expect(
      approvalCommand({
        text: '⚠ approval required — bash\ninput: {"command":"ls -l"}',
        bashCommand: "printf 'dummy\\n' > ~/Desktop/z-engine-dummy.txt",
      }),
    ).toBe("printf 'dummy\\n' > ~/Desktop/z-engine-dummy.txt");
  });

  it("parses command from a JSON input preview", () => {
    expect(
      approvalCommand({
        text: '⚠ approval required — bash\ninput: {"command":"cargo test"}',
      }),
    ).toBe("cargo test");
  });

  it("falls back to a path for file tools", () => {
    expect(
      approvalCommand({
        text: '⚠ approval required — edit_file\ninput: {"path":"src/main.rs"}',
      }),
    ).toBe("src/main.rs");
  });

  it("reads the tool name from metadata or the title line", () => {
    expect(approvalToolName({ text: "⚠ approval required — bash\ninput: {}", toolName: "bash" })).toBe(
      "bash",
    );
    expect(approvalToolName({ text: "⚠ approval required — edit_file\ninput: {}" })).toBe("edit_file");
  });
});
