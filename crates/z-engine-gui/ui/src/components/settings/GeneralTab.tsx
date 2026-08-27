import { useState } from "react";
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
        <span>Model id</span>
        <input value={model} onChange={(e) => setModel(e.currentTarget.value)} spellCheck={false} />
      </label>
      <label className="form-row">
        <span>Base URL</span>
        <input value={baseUrl} onChange={(e) => setBaseUrl(e.currentTarget.value)} spellCheck={false} />
      </label>
      <label className="form-row">
        <span>Max context tokens</span>
        <input type="number" value={maxCtx} onChange={(e) => setMaxCtx(e.currentTarget.value)} />
      </label>
      <label className="form-row check">
        <input type="checkbox" checked={review} onChange={(e) => setReview(e.currentTarget.checked)} />
        <span>Post-edit reviewer pass</span>
      </label>
      <p className="form-note">
        Written to <code>.z-engine/config.toml</code> (still reads{" "}
        <code>.harness/config.toml</code> if the new file is missing). The model applies to new
        turns immediately.
      </p>
      <div className="tab-actions">
        <button className="primary" onClick={() => void save()}>
          {saved ? "Saved" : "Save"}
        </button>
      </div>
    </div>
  );
}
