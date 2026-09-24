import { describe, expect, it } from "vitest";
import type { ProviderSettings } from "./protocol/config/ProviderSettings";
import {
  canSubmitProviderConnect,
  connectFormDefaults,
  detectProviderId,
  isProviderConnected,
  PROVIDERS,
  requiresApiKey,
} from "./providers";

function preset(id: string) {
  const found = PROVIDERS.find((p) => p.id === id);
  if (!found) throw new Error(`missing provider ${id}`);
  return found;
}

const live = (base_url: string, overrides: Partial<ProviderSettings> = {}): ProviderSettings => ({
  kind: "auto",
  base_url,
  headers: {},
  cache_control: null,
  ...overrides,
});

describe("OpenCode Zen keyless connect", () => {
  it("detects the Zen gateway", () => {
    expect(detectProviderId("https://opencode.ai/zen/v1")).toBe("opencode");
  });

  it("does not require an API key to connect", () => {
    expect(requiresApiKey(preset("opencode"))).toBe(false);
    expect(requiresApiKey(preset("ollama"))).toBe(false);
    expect(requiresApiKey(preset("lmstudio"))).toBe(false);
    expect(requiresApiKey(preset("openrouter"))).toBe(true);
    expect(requiresApiKey(preset("anthropic"))).toBe(true);
  });

  it("treats the selected OpenCode provider as connected without a key", () => {
    expect(isProviderConnected(preset("opencode"), true, false)).toBe(true);
    expect(isProviderConnected(preset("opencode"), false, false)).toBe(false);
    expect(isProviderConnected(preset("openrouter"), true, false)).toBe(false);
    expect(isProviderConnected(preset("openrouter"), true, true)).toBe(true);
  });

  it("allows Connect & Set Active with an empty key", () => {
    expect(canSubmitProviderConnect(preset("opencode"), "", false)).toBe(true);
    expect(canSubmitProviderConnect(preset("openrouter"), "", false)).toBe(false);
    expect(canSubmitProviderConnect(preset("openrouter"), "sk-or-v1-x", false)).toBe(true);
  });
});

describe("presets", () => {
  it("reaches Anthropic natively and local servers as OpenAI-compatible endpoints", () => {
    expect(preset("anthropic")).toMatchObject({ kind: "anthropic", baseUrl: "https://api.anthropic.com" });
    expect(preset("lmstudio")).toMatchObject({ kind: "openai_chat", baseUrl: "http://localhost:1234/v1", tag: "Local" });
    expect(preset("custom").kind).toBe("auto");
  });

  it("detects every preset from its own endpoint", () => {
    for (const p of PROVIDERS.filter((p) => p.baseUrl)) expect(detectProviderId(p.baseUrl)).toBe(p.id);
    expect(detectProviderId("")).toBe("openrouter");
    expect(detectProviderId("https://proxy.internal/v1")).toBe("custom");
  });
});

describe("connectFormDefaults", () => {
  const active = { model: "openrouter/free", provider: live("https://openrouter.ai/api/v1") };

  it("switches endpoint, kind and model to the preset when connecting a different provider", () => {
    expect(connectFormDefaults(preset("opencode"), active)).toEqual({
      model: "deepseek-v4-flash-free",
      baseUrl: "https://opencode.ai/zen/v1",
      kind: "openai_chat",
      headers: {},
      cacheControl: null,
    });
  });

  it("keeps the live values when reconfiguring the active provider", () => {
    const configured = { model: "openrouter/free", provider: live("https://openrouter.ai/api/v1", { headers: { "X-Title": "z" }, cache_control: false }) };
    expect(connectFormDefaults(preset("openrouter"), configured)).toEqual({
      model: "openrouter/free",
      baseUrl: "https://openrouter.ai/api/v1",
      kind: "auto",
      headers: { "X-Title": "z" },
      cacheControl: false,
    });
  });

  it("keeps a custom endpoint the presets do not know", () => {
    const custom = { model: "local/mix", provider: live("https://proxy.internal/v1", { kind: "anthropic" }) };
    expect(connectFormDefaults(preset("custom"), custom)).toMatchObject({
      model: "local/mix",
      baseUrl: "https://proxy.internal/v1",
      kind: "anthropic",
    });
  });
});
