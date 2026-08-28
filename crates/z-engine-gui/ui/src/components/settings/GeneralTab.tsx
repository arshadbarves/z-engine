import { useState } from "react";
import { Check, ChevronDown, ExternalLink, KeyRound, Sparkles } from "lucide-react";
import {
  getConfig,
  openReleaseUrl,
  saveApiKey,
  saveGeneral,
  type HarnessConfig,
} from "../../lib/commands";
import { configStore } from "../../lib/configStore";
import { modelStore, pushToast } from "../../lib/events";

interface ProviderPreset {
  id: string;
  name: string;
  baseUrl: string;
  defaultModel: string;
  keyUrl?: string;
  keyPlaceholder: string;
  desc: string;
}

const PROVIDERS: ProviderPreset[] = [
  {
    id: "openrouter",
    name: "OpenRouter (Recommended)",
    baseUrl: "https://openrouter.ai/api/v1",
    defaultModel: "openrouter/auto",
    keyUrl: "https://openrouter.ai/keys",
    keyPlaceholder: "sk-or-v1-...",
    desc: "Universal gateway with access to 200+ top LLMs",
  },
  {
    id: "openai",
    name: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    defaultModel: "openai/gpt-4o",
    keyUrl: "https://platform.openai.com/api-keys",
    keyPlaceholder: "sk-proj-...",
    desc: "Direct access to GPT-4o, o3-mini, and OpenAI models",
  },
  {
    id: "anthropic",
    name: "Anthropic",
    baseUrl: "https://api.anthropic.com/v1",
    defaultModel: "anthropic/claude-sonnet-4",
    keyUrl: "https://console.anthropic.com/settings/keys",
    keyPlaceholder: "sk-ant-...",
    desc: "Claude 3.7 Sonnet, Claude 3.5 Haiku, and Claude Opus",
  },
  {
    id: "google",
    name: "Google AI (Gemini)",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta/openai/",
    defaultModel: "google/gemini-2.5-pro",
    keyUrl: "https://aistudio.google.com/app/apikey",
    keyPlaceholder: "AIzaSy...",
    desc: "Google AI Studio Gemini 2.5 Pro and Flash models",
  },
  {
    id: "deepseek",
    name: "DeepSeek",
    baseUrl: "https://api.deepseek.com/v1",
    defaultModel: "deepseek/deepseek-chat",
    keyUrl: "https://platform.deepseek.com/api_keys",
    keyPlaceholder: "sk-...",
    desc: "DeepSeek-V3 and DeepSeek-R1 reasoning models",
  },
  {
    id: "groq",
    name: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    defaultModel: "groq/llama-3.3-70b-versatile",
    keyUrl: "https://console.groq.com/keys",
    keyPlaceholder: "gsk_...",
    desc: "Ultra-low latency inference for Llama 3.3 and DeepSeek",
  },
  {
    id: "mistral",
    name: "Mistral AI",
    baseUrl: "https://api.mistral.ai/v1",
    defaultModel: "mistral/mistral-large-latest",
    keyUrl: "https://console.mistral.ai/api-keys",
    keyPlaceholder: "...",
    desc: "Mistral Large, Mistral Small, and Codestral",
  },
  {
    id: "ollama",
    name: "Ollama (Local)",
    baseUrl: "http://localhost:11434/v1",
    defaultModel: "ollama/llama3.3",
    keyUrl: "https://ollama.com/download",
    keyPlaceholder: "ollama (no key required)",
    desc: "Run open-source LLMs locally offline on your own machine",
  },
  {
    id: "custom",
    name: "Custom (OpenAI-Compatible)",
    baseUrl: "",
    defaultModel: "",
    keyPlaceholder: "API key or bearer token",
    desc: "Connect to any custom OpenAI-compatible server or proxy",
  },
];

function detectProviderId(baseUrl: string | null | undefined): string {
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

export function GeneralTab({ cfg }: { cfg: HarnessConfig }) {
  const [selectedProviderId, setSelectedProviderId] = useState(() => detectProviderId(cfg.baseUrl));
  const [providerMenuOpen, setProviderMenuOpen] = useState(false);
  const [model, setModel] = useState(cfg.model);
  const [baseUrl, setBaseUrl] = useState(cfg.baseUrl ?? "");
  const [maxCtx, setMaxCtx] = useState(String(cfg.maxContextTokens));
  const [review, setReview] = useState(Boolean(cfg.reviewEnabled));
  const [apiKey, setApiKey] = useState("");
  const [hasKey, setHasKey] = useState(Boolean(cfg.hasApiKey));
  const [hint, setHint] = useState(cfg.apiKeyHint ?? null);
  const [saved, setSaved] = useState(false);

  const activeProvider = PROVIDERS.find((p) => p.id === selectedProviderId) ?? PROVIDERS[0];

  function handleProviderChange(id: string) {
    setSelectedProviderId(id);
    setProviderMenuOpen(false);
    const p = PROVIDERS.find((item) => item.id === id);
    if (!p) return;
    if (p.baseUrl) setBaseUrl(p.baseUrl);
    if (p.defaultModel && (!model || model === "openrouter/auto" || model.startsWith("anthropic/") || model.startsWith("openai/"))) {
      setModel(p.defaultModel);
    }
  }

  async function handleConnect() {
    if (activeProvider.keyUrl) {
      pushToast(`Opening ${activeProvider.name} console…`, "info");
      await openReleaseUrl(activeProvider.keyUrl);
    }
  }

  async function refresh() {
    const next = await getConfig();
    configStore.set(next);
    setHasKey(Boolean(next.hasApiKey));
    setHint(next.apiKeyHint ?? null);
  }

  async function save() {
    await saveGeneral({
      model: model.trim() || null,
      baseUrl: baseUrl.trim() || null,
      maxContextTokens: Number(maxCtx) > 0 ? Number(maxCtx) : null,
      review,
    });
    if (apiKey.trim()) {
      await saveApiKey(apiKey.trim());
      setApiKey("");
    }
    if (model.trim()) modelStore.set(model.trim());
    await refresh();
    setSaved(true);
    setTimeout(() => setSaved(false), 1600);
  }

  async function clearKey() {
    await saveApiKey(null);
    setApiKey("");
    await refresh();
  }

  return (
    <div className="tab-body">
      <section className="settings-group">
        <div className="settings-group-header">
          <h3>AI Provider & API Connection</h3>
          <span className="settings-group-sub">Choose your model provider and connect your API credentials</span>
        </div>
        <div className="settings-card">
          <div className="form-row custom-select-row">
            <div className="form-label-row">
              <span className="form-label-title">Provider</span>
              {activeProvider.keyUrl && (
                <button
                  type="button"
                  className="provider-connect-btn"
                  onClick={() => void handleConnect()}
                  title={`Open ${activeProvider.name} API Keys portal in browser`}
                >
                  <ExternalLink size={12} />
                  <span>Connect / Get API Key</span>
                </button>
              )}
            </div>

            <div className="custom-select-wrap">
              {providerMenuOpen && (
                <div
                  className="custom-select-backdrop"
                  onClick={() => setProviderMenuOpen(false)}
                />
              )}
              <button
                type="button"
                className={`custom-select-trigger${providerMenuOpen ? " active" : ""}`}
                onClick={() => setProviderMenuOpen((o) => !o)}
                aria-haspopup="listbox"
                aria-expanded={providerMenuOpen}
              >
                <div className="custom-select-val">
                  <span className="custom-select-name">{activeProvider.name}</span>
                  <span className="custom-select-desc">{activeProvider.desc}</span>
                </div>
                <ChevronDown size={14} className={`select-arrow${providerMenuOpen ? " open" : ""}`} />
              </button>

              {providerMenuOpen && (
                <div className="custom-select-popover" role="listbox">
                  <div className="custom-select-head">Select Model Provider</div>
                  <div className="custom-select-list">
                    {PROVIDERS.map((p) => {
                      const isSelected = p.id === selectedProviderId;
                      return (
                        <button
                          key={p.id}
                          type="button"
                          className={`custom-select-item${isSelected ? " selected" : ""}`}
                          role="option"
                          aria-selected={isSelected}
                          onClick={() => handleProviderChange(p.id)}
                        >
                          <div className="custom-select-item-text">
                            <span className="custom-select-item-name">{p.name}</span>
                            <span className="custom-select-item-desc">{p.desc}</span>
                          </div>
                          {isSelected && <Check size={14} className="custom-select-check" />}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>
          </div>

          <label className="form-row">
            <div className="form-label-row">
              <span className="form-label-title">API Key</span>
              {hasKey && hint && (
                <span className="provider-status-badge ok">
                  <KeyRound size={11} />
                  <span>Saved (••••{hint})</span>
                </span>
              )}
            </div>
            <span className="form-label-desc">
              {hasKey
                ? "Key is saved securely in ~/.config/z-engine/auth.json. Enter a new key below to replace it."
                : `Required for ${activeProvider.name}. Click "Connect / Get API Key" above to generate one.`}
            </span>
            <div className="form-input-with-action">
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.currentTarget.value)}
                spellCheck={false}
                autoComplete="off"
                placeholder={hasKey ? "••••••••••••••••" : activeProvider.keyPlaceholder}
              />
              {hasKey && (
                <button className="ghost clear-btn" type="button" onClick={() => void clearKey()}>
                  Clear key
                </button>
              )}
            </div>
          </label>

          <label className="form-row">
            <span className="form-label-title">Default Model</span>
            <span className="form-label-desc">Model used for new chat sessions</span>
            <input
              value={model}
              onChange={(e) => setModel(e.currentTarget.value)}
              spellCheck={false}
              placeholder={activeProvider.defaultModel || "e.g. anthropic/claude-sonnet-4"}
            />
          </label>

          <label className="form-row">
            <span className="form-label-title">Base URL</span>
            <span className="form-label-desc">API endpoint URL (OpenAI-compatible)</span>
            <input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.currentTarget.value)}
              spellCheck={false}
              placeholder={activeProvider.baseUrl || "https://api.openai.com/v1"}
            />
          </label>
        </div>
      </section>

      <section className="settings-group">
        <div className="settings-group-header">
          <h3>Agent & Context Engine</h3>
          <span className="settings-group-sub">Configure context token budgets and automated reviewer passes</span>
        </div>
        <div className="settings-card">
          <label className="form-row">
            <span className="form-label-title">Max Context Tokens</span>
            <span className="form-label-desc">Token window limit before intelligent auto-compaction</span>
            <input
              type="number"
              value={maxCtx}
              onChange={(e) => setMaxCtx(e.currentTarget.value)}
              placeholder="128000"
            />
          </label>
          <div className="form-row check">
            <div>
              <span className="form-label-title">Post-Edit Reviewer</span>
              <span className="form-label-desc">Automated fast reviewer pass after code file edits</span>
            </div>
            <label className="switch-toggle">
              <input
                type="checkbox"
                checked={review}
                onChange={(e) => setReview(e.currentTarget.checked)}
              />
              <span className="switch-slider" />
            </label>
          </div>
        </div>
      </section>

      <p className="form-note">
        Model and limits are persisted to <code>.z-engine/config.toml</code>. API keys are safely encrypted in{" "}
        <code>~/.config/z-engine/auth.json</code> and loaded instantly into active sessions.
      </p>

      <div className="tab-actions">
        <button className="primary" onClick={() => void save()} type="button">
          {saved ? (
            <>
              <Check size={13} />
              <span>Saved Settings</span>
            </>
          ) : (
            <>
              <Sparkles size={13} />
              <span>Save Changes</span>
            </>
          )}
        </button>
      </div>
    </div>
  );
}
