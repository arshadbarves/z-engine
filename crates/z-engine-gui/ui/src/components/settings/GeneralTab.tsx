import { useState } from "react";
import { Check } from "lucide-react";
import {
  getConfig,
  saveApiKey,
  saveGeneral,
  type HarnessConfig,
} from "../../lib/commands";
import { configStore } from "../../lib/configStore";
import { modelStore } from "../../lib/events";

export function GeneralTab({ cfg }: { cfg: HarnessConfig }) {
  const [model, setModel] = useState(cfg.model);
  const [baseUrl, setBaseUrl] = useState(cfg.baseUrl ?? "");
  const [maxCtx, setMaxCtx] = useState(String(cfg.maxContextTokens));
  const [review, setReview] = useState(Boolean(cfg.reviewEnabled));
  const [apiKey, setApiKey] = useState("");
  const [hasKey, setHasKey] = useState(Boolean(cfg.hasApiKey));
  const [hint, setHint] = useState(cfg.apiKeyHint ?? null);
  const [saved, setSaved] = useState(false);

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
        <h3>Provider</h3>
        <div className="settings-card">
          <label className="form-row">
            <span className="form-label-title">OpenRouter API key</span>
            <span className="form-label-desc">
              {hasKey
                ? `Saved (••••${hint ?? ""}). Paste a new key to replace it.`
                : "Required for chat. Stored in ~/.config/z-engine/auth.json."}
            </span>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.currentTarget.value)}
              spellCheck={false}
              autoComplete="off"
              placeholder={hasKey ? "••••••••" : "sk-or-…"}
            />
            {hasKey && (
              <button className="ghost" type="button" onClick={() => void clearKey()}>
                Clear key
              </button>
            )}
          </label>
          <label className="form-row">
            <span className="form-label-title">Default model</span>
            <span className="form-label-desc">OpenRouter model for new chats</span>
            <input
              value={model}
              onChange={(e) => setModel(e.currentTarget.value)}
              spellCheck={false}
              placeholder="e.g. anthropic/claude-sonnet-4"
            />
          </label>
          <label className="form-row">
            <span className="form-label-title">Base URL</span>
            <span className="form-label-desc">Leave default unless you use a proxy</span>
            <input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.currentTarget.value)}
              spellCheck={false}
              placeholder="https://openrouter.ai/api/v1"
            />
          </label>
        </div>
      </section>
      <section className="settings-group">
        <h3>Agent</h3>
        <div className="settings-card">
          <label className="form-row">
            <span className="form-label-title">Max context tokens</span>
            <span className="form-label-desc">Window size before auto-compaction</span>
            <input
              type="number"
              value={maxCtx}
              onChange={(e) => setMaxCtx(e.currentTarget.value)}
              placeholder="128000"
            />
          </label>
          <div className="form-row check">
            <div>
              <span className="form-label-title">Post-edit reviewer</span>
              <span className="form-label-desc">Verify diffs with a fast reviewer pass</span>
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
        Model and limits save to <code>.z-engine/config.toml</code>. The API key saves to{" "}
        <code>~/.config/z-engine/auth.json</code> and applies to the current session immediately.
      </p>
      <div className="tab-actions">
        <button className="primary" onClick={() => void save()} type="button">
          {saved ? (
            <>
              <Check size={13} />
              <span>Saved</span>
            </>
          ) : (
            "Save changes"
          )}
        </button>
      </div>
    </div>
  );
}
