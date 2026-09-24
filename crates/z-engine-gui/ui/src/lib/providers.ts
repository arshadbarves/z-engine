import type { ProviderKind } from "./protocol/config/ProviderKind";
import type { ProviderSettings } from "./protocol/config/ProviderSettings";

export interface ProviderPreset {
  id: string;
  name: string;
  /** Wire format written to `provider.kind`. */
  kind: ProviderKind;
  baseUrl: string;
  defaultModel: string;
  /** models.dev provider id whose models the picker lists. */
  catalogProvider: string;
  keyUrl?: string;
  keyPlaceholder: string;
  desc: string;
  tag: "API key" | "Optional key" | "Local" | "Custom";
  color: string;
}

const preset = (p: Omit<ProviderPreset, "catalogProvider">): ProviderPreset => ({ ...p, catalogProvider: p.id });

export const PROVIDERS: ProviderPreset[] = [
  preset({
    id: "opencode",
    name: "OpenCode Zen",
    kind: "openai_chat",
    baseUrl: "https://opencode.ai/zen/v1",
    defaultModel: "deepseek-v4-flash-free",
    keyUrl: "https://opencode.ai/zen",
    keyPlaceholder: "Optional — paid models only",
    desc: "OpenCode’s curated gateway. Connect directly with no key for free chat models; a Zen key is only needed for paid models.",
    tag: "Optional key",
    color: "#0ea5e9",
  }),
  preset({
    id: "openrouter",
    name: "OpenRouter",
    kind: "openai_chat",
    baseUrl: "https://openrouter.ai/api/v1",
    defaultModel: "openrouter/auto",
    keyUrl: "https://openrouter.ai/keys",
    keyPlaceholder: "sk-or-v1-...",
    desc: "Universal gateway with access to hundreds of models",
    tag: "API key",
    color: "#6366f1",
  }),
  preset({
    id: "anthropic",
    name: "Anthropic",
    kind: "anthropic",
    baseUrl: "https://api.anthropic.com",
    defaultModel: "claude-sonnet-4-5",
    keyUrl: "https://console.anthropic.com/settings/keys",
    keyPlaceholder: "sk-ant-...",
    desc: "Claude models through the native Messages API",
    tag: "API key",
    color: "#d97706",
  }),
  preset({
    id: "openai",
    name: "OpenAI",
    kind: "openai_chat",
    baseUrl: "https://api.openai.com/v1",
    defaultModel: "gpt-4o",
    keyUrl: "https://platform.openai.com/api-keys",
    keyPlaceholder: "sk-proj-...",
    desc: "Direct access to GPT and o-series models",
    tag: "API key",
    color: "#10a37f",
  }),
  preset({
    id: "google",
    name: "Google AI (Gemini)",
    kind: "openai_chat",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai",
    defaultModel: "gemini-2.5-pro",
    keyUrl: "https://aistudio.google.com/app/apikey",
    keyPlaceholder: "AIzaSy...",
    desc: "Gemini models through Google’s OpenAI-compatible endpoint",
    tag: "API key",
    color: "#4285f4",
  }),
  preset({
    id: "deepseek",
    name: "DeepSeek",
    kind: "openai_chat",
    baseUrl: "https://api.deepseek.com/v1",
    defaultModel: "deepseek-chat",
    keyUrl: "https://platform.deepseek.com/api_keys",
    keyPlaceholder: "sk-...",
    desc: "DeepSeek chat and reasoning models",
    tag: "API key",
    color: "#0284c7",
  }),
  preset({
    id: "groq",
    name: "Groq",
    kind: "openai_chat",
    baseUrl: "https://api.groq.com/openai/v1",
    defaultModel: "llama-3.3-70b-versatile",
    keyUrl: "https://console.groq.com/keys",
    keyPlaceholder: "gsk_...",
    desc: "Low-latency inference for open models",
    tag: "API key",
    color: "#f97316",
  }),
  preset({
    id: "mistral",
    name: "Mistral AI",
    kind: "openai_chat",
    baseUrl: "https://api.mistral.ai/v1",
    defaultModel: "mistral-large-latest",
    keyUrl: "https://console.mistral.ai/api-keys",
    keyPlaceholder: "...",
    desc: "Mistral Large, Mistral Small, and Codestral",
    tag: "API key",
    color: "#ea580c",
  }),
  preset({
    id: "ollama",
    name: "Ollama (Local)",
    kind: "openai_chat",
    baseUrl: "http://localhost:11434/v1",
    defaultModel: "llama3.3",
    keyPlaceholder: "ollama (no key required)",
    desc: "Run open models locally on your own machine",
    tag: "Local",
    color: "#94a3b8",
  }),
  preset({
    id: "lmstudio",
    name: "LM Studio (Local)",
    kind: "openai_chat",
    baseUrl: "http://localhost:1234/v1",
    defaultModel: "",
    keyPlaceholder: "LM Studio (no key required)",
    desc: "Serve a downloaded model from LM Studio’s local server",
    tag: "Local",
    color: "#8b5cf6",
  }),
  preset({
    id: "custom",
    name: "Custom endpoint",
    kind: "auto",
    baseUrl: "",
    defaultModel: "",
    keyPlaceholder: "API key or bearer token",
    desc: "Any OpenAI-compatible or Anthropic-compatible server or proxy",
    tag: "Custom",
    color: "#a855f7",
  }),
];

/** Paid gateways need a stored or typed key. Zen free models and local servers do not. */
export function requiresApiKey(provider: ProviderPreset): boolean {
  return provider.tag === "API key" || provider.tag === "Custom";
}

export function isProviderConnected(provider: ProviderPreset, isCurrent: boolean, hasKey: boolean): boolean {
  return isCurrent && (hasKey || !requiresApiKey(provider));
}

export function canSubmitProviderConnect(provider: ProviderPreset, apiKey: string, hasSavedKey: boolean): boolean {
  if (!requiresApiKey(provider)) return true;
  return Boolean(apiKey.trim() || hasSavedKey);
}

export function detectProviderId(baseUrl: string | null | undefined): string {
  const url = (baseUrl ?? "").trim().toLowerCase();
  if (url.includes("opencode.ai")) return "opencode";
  if (!url || url.includes("openrouter.ai")) return "openrouter";
  if (url.includes("openai.com")) return "openai";
  if (url.includes("anthropic.com")) return "anthropic";
  if (url.includes("googleapis.com")) return "google";
  if (url.includes("deepseek.com")) return "deepseek";
  if (url.includes("groq.com")) return "groq";
  if (url.includes("mistral.ai")) return "mistral";
  if (url.includes("11434") || url.includes("ollama")) return "ollama";
  if (url.includes(":1234") || url.includes("lmstudio")) return "lmstudio";
  return "custom";
}

export function presetById(id: string): ProviderPreset | undefined {
  return PROVIDERS.find((p) => p.id === id);
}

export interface ConnectFormValues {
  model: string;
  baseUrl: string;
  kind: ProviderKind;
  headers: Record<string, string>;
  cacheControl: boolean | null;
}

/** Prefill for the connect dialog. Live values belong to the active provider
 * only — reusing them for another provider would save the wrong endpoint. */
export function connectFormDefaults(
  provider: ProviderPreset,
  active: { model?: string; provider?: ProviderSettings | null },
): ConnectFormValues {
  const live = active.provider;
  if (!live || detectProviderId(live.base_url) !== provider.id) {
    return { model: provider.defaultModel, baseUrl: provider.baseUrl, kind: provider.kind, headers: {}, cacheControl: null };
  }
  return {
    model: active.model || provider.defaultModel,
    baseUrl: live.base_url || provider.baseUrl,
    kind: live.kind,
    headers: { ...live.headers },
    cacheControl: live.cache_control,
  };
}
