import { useState, useSyncExternalStore } from "react";
import { Brain, ChevronDown } from "lucide-react";
import { setReasoningEffort } from "../lib/commands";
import { lookupModel } from "../lib/catalog";
import type { CatalogData } from "../lib/catalog";
import { modelStore } from "../lib/events";



/** Reasoning-effort chip (Codex-style). Only rendered when the active
 * model advertises reasoning support in the catalog; the chosen effort is
 * sent as the unified `reasoning.effort` parameter. */
export function EffortSelector({ catalog }: { catalog: CatalogData | null }) {
  const EFFORTS = ["low", "medium", "high", "xhigh"] as const;
  const model = useSyncExternalStore(modelStore.subscribe, () => modelStore.getSnapshot());
  const [effort, setEffort] = useState<string | null>(null);
  const [open, setOpen] = useState(false);

  if (!effort && !lookupModel(catalog, model || "")?.model.reasoning) return null;
  // Non-reasoning models never see the chip once a model without
  // reasoning is selected — but keep it while an effort is explicitly set.

  async function pick(e: string | null) {
    setOpen(false);
    setEffort(e);
    try {
      await setReasoningEffort(e);
    } catch (err) {
      console.error(err);
    }
  }

  return (
    <div className="model-picker">
      {open && <div className="popover-backdrop" onClick={() => setOpen(false)} />}
      <button
        className="mode model-btn"
        onClick={() => setOpen((o) => !o)}
        title="Reasoning effort"
      >
        <Brain size={11} />
        <span>{effort ?? "reason"}</span>
        <ChevronDown size={9} strokeWidth={2.4} />
      </button>
      {open && (
        <div className="popover" role="menu">
          <div className="popover-head">Reasoning effort</div>
          <div className="popover-current">{effort ?? "(provider default)"}</div>
          {effort && (
            <button className="popover-item" role="menuitem" onClick={() => void pick(null)}>
              clear
              <span className="popover-sub">omit the parameter</span>
            </button>
          )}
          {EFFORTS.filter((e) => e !== effort).map((e) => (
            <button key={e} className="popover-item" role="menuitem" onClick={() => void pick(e)}>
              {e}
              <span className="popover-sub">
                {e === "low"
                  ? "fast and cheap"
                  : e === "medium"
                    ? "balanced default"
                    : e === "high"
                      ? "thorough thinking"
                      : "maximum depth"}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
