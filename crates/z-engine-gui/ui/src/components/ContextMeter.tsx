import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { configStore } from "../lib/configStore";
import { transcriptStore, usageStore } from "../lib/events";
import { contextBreakdown } from "../lib/contextBreakdown";
import { estimateCost, fmtCost, fmtTokens } from "../lib/util";

const RING_R = 7;
const RING_C = 2 * Math.PI * RING_R;

/** Header context ring. Click opens a two-column category chart. */
export function ContextMeter() {
  const usage = useSyncExternalStore(usageStore.subscribe, () => usageStore.getSnapshot());
  const messages = useSyncExternalStore(transcriptStore.subscribe, () => transcriptStore.getSnapshot());
  const cfg = useSyncExternalStore(configStore.subscribe, () => configStore.getSnapshot());
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function onDoc(e: MouseEvent) {
      if (!rootRef.current?.contains(e.target as Node)) setOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const max = Math.max(1, usage.maxTokens);
  const br = contextBreakdown(messages, usage.promptTokens, max);
  const pct = Math.min(100, Math.round((br.used / br.max) * 100));
  const level = pct >= 92 ? "danger" : pct >= 80 ? "warn" : "ok";
  const cost = estimateCost(cfg?.pricing ?? null, usage.promptTokens, usage.completionTokens);
  const compactAt = cfg?.compactAtPercent ?? 92;
  const mid = Math.ceil(br.slices.length / 2);
  const left = br.slices.slice(0, mid);
  const right = [...br.slices.slice(mid), { id: "rest", label: "Free", tokens: br.remaining, color: "rgba(255,255,255,0.18)" }];

  return (
    <div className={`ctx-meter ${level}`} ref={rootRef}>
      <button
        type="button"
        className="ctx-ring-btn"
        aria-expanded={open}
        aria-label={`${pct}% context used`}
        title={`${pct}% context used`}
        onClick={() => setOpen((v) => !v)}
      >
        <svg className="ctx-ring" viewBox="0 0 20 20" width={18} height={18} aria-hidden>
          <circle className="ctx-track" cx="10" cy="10" r={RING_R} />
          <circle
            className="ctx-fill"
            cx="10"
            cy="10"
            r={RING_R}
            strokeDasharray={RING_C}
            strokeDashoffset={RING_C * (1 - pct / 100)}
          />
        </svg>
      </button>
      <div className="ctx-tip" role="tooltip">
        <strong>{pct}% context used</strong>
        <span>
          {fmtTokens(br.used)} / {fmtTokens(br.max)}
        </span>
      </div>
      {open && (
        <div className="ctx-pop" role="dialog" aria-label="Context usage">
          <div className="ctx-pop-head">
            <span>Context</span>
            <button type="button" className="ctx-pop-x" onClick={() => setOpen(false)} aria-label="Close">
              ×
            </button>
          </div>
          <div className="ctx-pop-sum">
            <span>{pct}% full</span>
            <span>
              {fmtTokens(br.used)} / {fmtTokens(br.max)}
            </span>
          </div>
          <div className="ctx-bar" aria-hidden>
            {br.slices.map((s) => (
              <i key={s.id} style={{ width: `${(s.tokens / br.max) * 100}%`, background: s.color }} />
            ))}
          </div>
          <div className="ctx-cols">
            <ul className="ctx-legend">
              {left.map((s) => (
                <li key={s.id}>
                  <i className="swatch" style={{ background: s.color }} />
                  {s.label}
                  <span>{fmtTokens(s.tokens)}</span>
                </li>
              ))}
            </ul>
            <ul className="ctx-legend">
              {right.map((s) => (
                <li key={s.id}>
                  <i className="swatch" style={{ background: s.color }} />
                  {s.label}
                  <span>{fmtTokens(s.tokens)}</span>
                </li>
              ))}
            </ul>
          </div>
          <p className="ctx-note">
            Auto-compacts at {compactAt}%
            {cost != null ? ` · est. ${fmtCost(cost)}` : ""}
          </p>
        </div>
      )}
    </div>
  );
}
