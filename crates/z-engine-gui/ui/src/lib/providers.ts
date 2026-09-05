export interface ProviderPreset {
  id: string;
  name: string;
  baseUrl: string;
  defaultModel: string;
  keyUrl?: string;
  keyPlaceholder: string;
  desc: string;
  tag: "API key" | "Local" | "Custom";
  color: string;
}

export const PROVIDERS: ProviderPreset[] = [
  {
    id: "openrouter",
    name: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    defaultModel: "openrouter/auto",
    keyUrl: "https://openrouter.ai/keys",
    keyPlaceholder: "sk-or-v1-...",
    desc: "Universal gateway with access to 200+ top LLMs",
    tag: "API key",
    color: "#6366f1",
  },
  {
    id: "anthropic",
    name: "Anthropic",
    baseUrl: "https://api.anthropic.com/v1",
    defaultModel: "anthropic/claude-sonnet-4",
    keyUrl: "https://console.anthropic.com/settings/keys",
    keyPlaceholder: "sk-ant-...",
    desc: "Claude 3.7 Sonnet, Claude 3.5 Haiku, and Claude Opus",
    tag: "API key",
    color: "#d97706",
  },
  {
    id: "openai",
    name: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    defaultModel: "openai/gpt-4o",
    keyUrl: "https://platform.openai.com/api-keys",
    keyPlaceholder: "sk-proj-...",
    desc: "Direct access to GPT-4o, o3-mini, and OpenAI models",
    tag: "API key",
    color: "#10a37f",
  },
  {
    id: "google",
    name: "Google AI (Gemini)",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai/",
    defaultModel: "google/gemini-2.5-pro",
    keyUrl: "https://aistudio.google.com/app/apikey",
    keyPlaceholder: "AIzaSy...",
    desc: "Google AI Studio Gemini 2.5 Pro and Flash models",
    tag: "API key",
    color: "#4285f4",
  },
  {
    id: "deepseek",
    name: "DeepSeek",
    baseUrl: "https://api.deepseek.com/v1",
    defaultModel: "deepseek/deepseek-chat",
    keyUrl: "https://platform.deepseek.com/api_keys",
    keyPlaceholder: "sk-...",
    desc: "DeepSeek-V3 and DeepSeek-R1 reasoning models",
    tag: "API key",
    color: "#0284c7",
  },
  {
    id: "groq",
    name: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    defaultModel: "groq/llama-3.3-70b-versatile",
    keyUrl: "https://console.groq.com/keys",
    keyPlaceholder: "gsk_...",
    desc: "Ultra-low latency inference for Llama 3.3 and DeepSeek",
    tag: "API key",
    color: "#f97316",
  },
  {
    id: "mistral",
    name: "Mistral AI",
    baseUrl: "https://api.mistral.ai/v1",
    defaultModel: "mistral/mistral-large-latest",
    keyUrl: "https://console.mistral.ai/api-keys",
    keyPlaceholder: "...",
    desc: "Mistral Large, Mistral Small, and Codestral",
    tag: "API key",
    color: "#ea580c",
  },
  {
    id: "ollama",
    name: "Ollama (Local)",
    baseUrl: "http://localhost:11434/v1",
    defaultModel: "ollama/llama3.3",
    keyPlaceholder: "ollama (no key required)",
    desc: "Run open-source LLMs locally offline on your own machine",
    tag: "Local",
    color: "#94a3b8",
  },
  {
    id: "custom",
    name: "Custom (OpenAI-Compatible)",
    baseUrl: "",
    defaultModel: "",
    keyPlaceholder: "API key or bearer token",
    desc: "Connect to any custom OpenAI-compatible server or proxy",
    tag: "Custom",
    color: "#a855f7",
  },
];

export function detectProviderId(baseUrl: string | null | undefined): string {
  const url = (baseUrl ?? "").trim().toLowerCase();
  if (!url || url.includes("openrouter.ai")) return "openrouter";
  if (url.includes("openai.com")) return "openai";
  if (url.includes("anthropic.com")) return "anthropic";
  if (url.includes("googleapis.com")) return "google";
  if (url.includes("deepseek.com")) return "deepseek";
  if (url.includes("groq.com")) return "groq";
  if (url.includes("mistral.ai")) return "mistral";
  if (url.includes("11434") || url.includes("ollama")) return "ollama";
  return "custom";
}
