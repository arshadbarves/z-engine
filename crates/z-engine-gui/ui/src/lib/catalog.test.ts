import { describe, expect, it } from "vitest";
import { catalogForPicker, groupModels, lookupModel, type CatalogData } from "./catalog";
import type { ModelInfo } from "./protocol/ModelInfo";

function model(provider: string, id: string, name: string, reasoning = false): ModelInfo {
  return { id, name, provider, contextWindow: 0, maxOutput: 0, pricing: null, tools: true, vision: false, reasoning };
}

const sample: CatalogData = [
  model("openrouter", "anthropic/claude-sonnet-4", "Claude Sonnet 4", true),
  model("anthropic", "claude-sonnet-4-5", "Claude Sonnet 4.5", true),
  model("openai", "gpt-4o", "GPT-4o"),
  model("opencode", "deepseek-v4-flash-free", "DeepSeek V4 Flash Free", true),
];

describe("catalogForPicker", () => {
  it("shows OpenRouter while it is active and connected", () => {
    const out = catalogForPicker(sample, "https://openrouter.ai/api/v1", true);
    expect(out.map((m) => m.id)).toEqual(["anthropic/claude-sonnet-4"]);
  });

  it("shows OpenCode while it is active without an API key", () => {
    const out = catalogForPicker(sample, "https://opencode.ai/zen/v1", false);
    expect(out.map((m) => m.name)).toEqual(["DeepSeek V4 Flash Free"]);
  });

  it("shows native Anthropic models for the Anthropic endpoint", () => {
    expect(catalogForPicker(sample, "https://api.anthropic.com", true).map((m) => m.id)).toEqual(["claude-sonnet-4-5"]);
  });

  it("hides OpenRouter models after its API key is disconnected", () => {
    expect(catalogForPicker(sample, "https://openrouter.ai/api/v1", false)).toEqual([]);
  });

  it("returns empty when the catalog is missing or lacks the active provider", () => {
    expect(catalogForPicker(null, "https://opencode.ai/zen/v1", false)).toEqual([]);
    expect(catalogForPicker([sample[2]], "https://opencode.ai/zen/v1", false)).toEqual([]);
  });
});

describe("groupModels", () => {
  it("groups by provider name and filters by id, name or provider", () => {
    expect(groupModels(sample, "").map((g) => [g.provider, g.items.length])).toEqual([
      ["Anthropic", 1],
      ["OpenAI", 1],
      ["OpenCode Zen", 1],
      ["OpenRouter", 1],
    ]);
    expect(groupModels(sample, "sonnet").flatMap((g) => g.items.map((m) => m.id))).toEqual([
      "claude-sonnet-4-5",
      "anthropic/claude-sonnet-4",
    ]);
    expect(groupModels(sample, "zen").map((g) => g.provider)).toEqual(["OpenCode Zen"]);
    expect(groupModels(sample, "", 0).every((g) => g.items.length === 0)).toBe(true);
  });
});

describe("lookupModel", () => {
  it("resolves exact, vendor-prefixed and case-insensitive ids like the engine", () => {
    expect(lookupModel(sample, "gpt-4o")?.providerId).toBe("openai");
    expect(lookupModel(sample, "openai/gpt-4o")?.id).toBe("gpt-4o");
    expect(lookupModel(sample, "claude-sonnet-4")?.id).toBe("anthropic/claude-sonnet-4");
    expect(lookupModel(sample, "GPT-4O")?.model.name).toBe("GPT-4o");
    expect(lookupModel(sample, "no-such-model")).toBeNull();
    expect(lookupModel(null, "gpt-4o")).toBeNull();
  });
});
