import { describe, expect, it } from "vitest";
import { queuePreview, queueTitle } from "./queuePreview";

describe("queuePreview", () => {
  it("uses the trimmed prompt text", () => {
    expect(queuePreview({ text: "  run the tests  ", images: [] })).toBe("run the tests");
  });

  it("collapses newlines so the chip stays one line", () => {
    expect(queuePreview({ text: "first\n\nsecond\nthird", images: [] })).toBe(
      "first second third",
    );
  });

  it("truncates long prompts with an ellipsis", () => {
    const label = queuePreview({ text: "x".repeat(80), images: [] });
    expect(label).toBe(`${"x".repeat(47)}…`);
    expect(label.length).toBe(48);
  });

  it("keeps text that exactly fills the budget verbatim", () => {
    const text = "y".repeat(48);
    expect(queuePreview({ text, images: [] })).toBe(text);
  });

  it("describes image-only follow-ups", () => {
    expect(queuePreview({ text: "", images: ["a"] })).toBe("1 image");
    expect(queuePreview({ text: "   ", images: ["a", "b"] })).toBe("2 images");
  });

  it("falls back to a neutral label when nothing is queued in the item", () => {
    expect(queuePreview({ text: "", images: [] })).toBe("Empty follow-up");
  });
});

describe("queueTitle", () => {
  it("shows the full untruncated prompt so the tooltip adds information", () => {
    const text = "z".repeat(120);
    expect(queueTitle({ text, images: [] })).toBe(text);
  });

  it("trims surrounding whitespace rather than rendering a blank tooltip", () => {
    expect(queueTitle({ text: "  deploy staging\n", images: [] })).toBe("deploy staging");
  });

  it("keeps interior newlines, which tooltips render as separate lines", () => {
    expect(queueTitle({ text: "first\nsecond", images: [] })).toBe("first\nsecond");
  });

  it("falls back to the preview when the text is only whitespace", () => {
    expect(queueTitle({ text: "   \n  ", images: ["a", "b"] })).toBe("2 images");
    expect(queueTitle({ text: "\t", images: [] })).toBe("Empty follow-up");
  });
});
