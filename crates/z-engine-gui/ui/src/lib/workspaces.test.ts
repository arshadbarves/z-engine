import { describe, expect, it } from "vitest";
import { sameWorkspacePath, wsBasename } from "./workspaces";

describe("sameWorkspacePath", () => {
  it("treats trailing slashes as the same folder", () => {
    expect(sameWorkspacePath("/Users/me/proj", "/Users/me/proj/")).toBe(true);
    expect(sameWorkspacePath("/Users/me/proj", "/Users/me/other")).toBe(false);
    expect(sameWorkspacePath(null, "/x")).toBe(false);
    expect(sameWorkspacePath("C:\\Users\\me\\proj", "C:/Users/me/proj")).toBe(true);
  });
});

describe("wsBasename", () => {
  it("uses the last path segment", () => {
    expect(wsBasename("/Users/me/z-engine")).toBe("z-engine");
    expect(wsBasename("/Users/me/z-engine/")).toBe("z-engine");
  });
});
