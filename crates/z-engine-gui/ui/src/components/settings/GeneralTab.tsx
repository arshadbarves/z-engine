import { useState } from "react";
import { Check } from "lucide-react";
import { getConfig, saveGeneral, type HarnessConfig } from "../../lib/commands";
import { configStore } from "../../lib/configStore";
import { modelStore } from "../../lib/events";

export function GeneralTab({ cfg }: { cfg: HarnessConfig }) {
  const [model, setModel] = useState(cfg.model);
  const [baseUrl, setBaseUrl] = useState(cfg.baseUrl ?? "");
  const [maxCtx, setMaxCtx] = useState(String(cfg.maxContextTokens));
  const [review, setReview] = useState(Boolean(cfg.reviewEnabled));
  const [saved, setSaved] = useState(false);

  async function save() {
    await saveGeneral({
      model: model.trim() || null,
      baseUrl: baseUrl.trim() || null,
      maxContextTokens: Number(maxCtx) > 0 ? Number(maxCtx) : null,
      review,
    });
    if (model.trim()) modelStore.set(model.trim());
    configStore.set(await getConfig());
    setSaved(true);
    setTimeout(() => setSaved(false), 1600);
  }

  return (
    <div className="tab-body">
      <label className="form-row">
        <span className="form-label-title">Default Model ID</span>
        <span className="form-label-desc">Primary AI model for new turns and sessions</span>
        <input value={model} onChange={(e) => setModel(e.currentTarget.value)} spellCheck={false} placeholder="e.g. claude-3-7-sonnet" />
      </label>
      <label className="form-row">
        <span className="form-label-title">Custom Base URL</span>
        <span className="form-label-desc">Optional LLM endpoint or proxy (e.g. OpenRouter, Ollama)</span>
        <input value={baseUrl} onChange={(e) => setBaseUrl(e.currentTarget.value)} spellCheck={false} placeholder="https://api.openai.com/v1" />
      </label>
      <label className="form-row">
        <span className="form-label-title">Max Context Tokens</span>
        <span className="form-label-desc">Context window capacity before auto-compaction triggers</span>
        <input type="number" value={maxCtx} onChange={(e) => setMaxCtx(e.currentTarget.value)} placeholder="128000" />
      </label>
      <div className="form-row check">
        <div>
          <span className="form-label-title">Post-Edit Reviewer Pass</span>
          <span className="form-label-desc">Inspect and verify diffs with a fast reviewer sub-agent</span>
        </div>
        <label className="switch-toggle">
          <input type="checkbox" checked={review} onChange={(e) => setReview(e.currentTarget.checked)} />
          <span className="switch-slider" />
        </label>
      </div>
      <p className="form-note">
        Saved to <code>.z-engine/config.toml</code>. Changes apply immediately to new turns.
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
