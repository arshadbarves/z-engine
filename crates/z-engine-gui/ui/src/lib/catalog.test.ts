import { describe, expect, it } from "vitest";
import { catalogForPicker, type CatalogData } from "./catalog";

const sample: CatalogData = {
  openai: { name: "OpenAI", models: { "gpt-4o": { name: "GPT-4o", reasoning: false, attachment: true } } },
  opencode: {
    name: "OpenCode Zen",
    models: {
      "deepseek-v4-flash-free": {
        name: "DeepSeek V4 Flash Free",
        reasoning: true,
        attachment: false,
      },
    },
  },
  openrouter: {
    name: "OpenRouter",
    models: {
      "anthropic/claude-sonnet-4": { name: "Claude Sonnet 4", reasoning: true, attachment: true },
    },
  },
};

describe("catalogForPicker", () => {
  it("shows OpenRouter while it is active and connected", () => {
    const out = catalogForPicker(sample, "https://openrouter.ai/api/v1", true);
    expect(Object.keys(out)).toEqual(["openrouter"]);
    expect(out.openrouter.models["anthropic/claude-sonnet-4"]?.name).toBe("Claude Sonnet 4");
  });

  it("shows OpenCode while it is active without an API key", () => {
    const out = catalogForPicker(sample, "https://opencode.ai/zen/v1", false);
    expect(Object.keys(out)).toEqual(["opencode"]);
    expect(out.opencode.models["deepseek-v4-flash-free"]?.name).toBe(
      "DeepSeek V4 Flash Free",
    );
  });

  it("hides OpenRouter models after its API key is disconnected", () => {
    expect(catalogForPicker(sample, "https://openrouter.ai/api/v1", false)).toEqual({});
  });

  it("returns empty when the catalog is missing or lacks the active provider", () => {
    expect(catalogForPicker(null, "https://opencode.ai/zen/v1", false)).toEqual({});
    expect(
      catalogForPicker({ openai: sample.openai }, "https://opencode.ai/zen/v1", false),
    ).toEqual({});
  });
});
