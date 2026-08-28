import { describe, expect, it } from "vitest";
import { catalogForPicker, type CatalogData } from "./catalog";

const sample: CatalogData = {
  openai: { name: "OpenAI", models: { "gpt-4o": { name: "GPT-4o", reasoning: false, attachment: true } } },
  openrouter: {
    name: "OpenRouter",
    models: {
      "anthropic/claude-sonnet-4": { name: "Claude Sonnet 4", reasoning: true, attachment: true },
    },
  },
};

describe("catalogForPicker", () => {
  it("keeps only OpenRouter", () => {
    const out = catalogForPicker(sample);
    expect(Object.keys(out)).toEqual(["openrouter"]);
    expect(out.openrouter.models["anthropic/claude-sonnet-4"]?.name).toBe("Claude Sonnet 4");
  });

  it("returns empty when catalog is missing or has no OpenRouter", () => {
    expect(catalogForPicker(null)).toEqual({});
    expect(catalogForPicker({ openai: sample.openai })).toEqual({});
  });
});
