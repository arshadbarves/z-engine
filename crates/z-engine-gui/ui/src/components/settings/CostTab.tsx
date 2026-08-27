import { useState } from "react";
import {
  getConfig,
  removeCostOverride,
  setCostOverride,
  type HarnessConfig,
} from "../../lib/commands";
import { configStore } from "../../lib/configStore";

export function CostTab({ cfg }: { cfg: HarnessConfig }) {
  const overrides = cfg.costOverrides ?? {};
  const [model, setModel] = useState("");
  const [usdIn, setUsdIn] = useState("");
  const [usdOut, setUsdOut] = useState("");

  async function refresh() {
    configStore.set(await getConfig());
  }

  async function add() {
    const m = model.trim();
    const i = Number(usdIn);
    const o = Number(usdOut);
    if (!m || !(i >= 0) || !(o >= 0)) return;
    await setCostOverride(m, i, o);
    setModel("");
    setUsdIn("");
    setUsdOut("");
    await refresh();
  }

  async function remove(m: string) {
    await removeCostOverride(m);
    await refresh();
  }

  return (
    <div className="tab-body">
      <p className="form-note">
        Per-model USD per million tokens. Exact-id overrides beat the built-in family table; unknown
        models show tokens only.
      </p>
      <ul className="rule-list">
        {Object.entries(overrides).map(([m, p]) => (
          <li key={m}>
            <code>
              {m} · ${p.usdPerMtokInput}/{p.usdPerMtokOutput}
            </code>
            <button className="mini" title={`Remove override for ${m}`} onClick={() => void remove(m)}>
              ✕
            </button>
          </li>
        ))}
        {Object.keys(overrides).length === 0 && (
          <li className="none">No overrides — using the built-in table.</li>
        )}
      </ul>
      <form
        className="inline-form cost-form"
        onSubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <input
          value={model}
          onChange={(e) => setModel(e.currentTarget.value)}
          placeholder="model id"
          spellCheck={false}
        />
        <input
          value={usdIn}
          onChange={(e) => setUsdIn(e.currentTarget.value)}
          placeholder="$ in / MTok"
          type="number"
          step="any"
        />
        <input
          value={usdOut}
          onChange={(e) => setUsdOut(e.currentTarget.value)}
          placeholder="$ out / MTok"
          type="number"
          step="any"
        />
        <button type="submit" disabled={!model.trim()}>
          Add override
        </button>
      </form>
    </div>
  );
}
