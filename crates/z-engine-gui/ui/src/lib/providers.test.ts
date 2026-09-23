import { describe, expect, it } from "vitest";
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

describe("OpenCode Zen keyless connect", () => {
  it("detects the Zen gateway", () => {
    expect(detectProviderId("https://opencode.ai/zen/v1")).toBe("opencode");
  });

  it("does not require an API key to connect", () => {
    expect(requiresApiKey(preset("opencode"))).toBe(false);
    expect(requiresApiKey(preset("ollama"))).toBe(false);
    expect(requiresApiKey(preset("openrouter"))).toBe(true);
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

describe("connectFormDefaults", () => {
  const active = { model: "openrouter/free", baseUrl: "https://openrouter.ai/api/v1" };

  it("switches endpoint and model to the preset when connecting a different provider", () => {
    expect(connectFormDefaults(preset("opencode"), active)).toEqual({
      model: "deepseek-v4-flash-free",
      baseUrl: "https://opencode.ai/zen/v1",
    });
  });

  it("keeps the live values when reconfiguring the active provider", () => {
    expect(connectFormDefaults(preset("openrouter"), active)).toEqual(active);
  });

  it("keeps a custom endpoint the presets do not know", () => {
    const custom = { model: "local/mix", baseUrl: "https://proxy.internal/v1" };
    expect(connectFormDefaults(preset("custom"), custom)).toEqual(custom);
  });
});
