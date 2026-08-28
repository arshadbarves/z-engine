import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { CheckCircle2, AlertTriangle, AlertOctagon, Minimize2, Eye, X } from "lucide-react";
import { configStore } from "../lib/configStore";
import { transcriptStore, usageStore, pushToast } from "../lib/events";
import { contextBreakdown } from "../lib/contextBreakdown";
import { compact } from "../lib/commands";
import { estimateCost, fmtCost, fmtTokens } from "../lib/util";
import { PromptInspector } from "./PromptInspector";

const RING_R = 7;
const RING_C = 2 * Math.PI * RING_R;

/** Human-psychology centered context window monitor. */
export function ContextMeter() {
  const usage = useSyncExternalStore(usageStore.subscribe, () => usageStore.getSnapshot());
  const messages = useSyncExternalStore(transcriptStore.subscribe, () => transcriptStore.getSnapshot());
  const cfg = useSyncExternalStore(configStore.subscribe, () => configStore.getSnapshot());
  const [open, setOpen] = useState(false);
  const [inspect, setInspect] = useState(false);
  const [compacting, setCompacting] = useState(false);
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
  const level = pct >= 85 ? "danger" : pct >= 65 ? "warn" : "ok";
  const cost = estimateCost(cfg?.pricing ?? null, usage.promptTokens, usage.completionTokens);
  const compactAt = cfg?.compactAtPercent ?? 92;

  async function handleCompact() {
    setCompacting(true);
    try {
      pushToast("Compacting session context…", "ok");
      await compact();
      setOpen(false);
    } catch (e) {
      pushToast(String(e).replace("Error: ", ""), "warn");
    } finally {
      setCompacting(false);
    }
  }

  const statusText =
    level === "ok"
      ? "Memory Healthy · Plenty of space"
      : level === "warn"
        ? "Memory Active · Moderately full"
        : "Memory High · Compaction near";

  const StatusIcon = level === "ok" ? CheckCircle2 : level === "warn" ? AlertTriangle : AlertOctagon;

  return (
    <div className={`ctx-meter ${level}`} ref={rootRef}>
      <button
        type="button"
        className="ctx-ring-btn"
        aria-expanded={open}
        aria-label={`${pct}% context used`}
        title={`${pct}% context used (${fmtTokens(br.used)} / ${fmtTokens(br.max)})`}
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
          {fmtTokens(br.used)} of {fmtTokens(br.max)}
        </span>
      </div>

      {open && (
        <div className="ctx-popover" role="dialog" aria-label="Context usage">
          <div className="ctx-pop-header">
            <div className={`ctx-status-pill ${level}`}>
              <StatusIcon size={13} />
              <span>{statusText}</span>
            </div>
            <button type="button" className="icon-btn" onClick={() => setOpen(false)} aria-label="Close">
              <X size={13} />
            </button>
          </div>

          <div className="ctx-metrics-grid">
            <div className="ctx-metric-card">
              <span className="ctx-metric-label">Used</span>
              <strong className="ctx-metric-val">{fmtTokens(br.used)}</strong>
              <span className="ctx-metric-sub">{pct}% of window</span>
            </div>
            <div className="ctx-metric-card">
              <span className="ctx-metric-label">Headroom</span>
              <strong className="ctx-metric-val">{fmtTokens(br.remaining)}</strong>
              <span className="ctx-metric-sub">{100 - pct}% available</span>
            </div>
            <div className="ctx-metric-card">
              <span className="ctx-metric-label">Capacity</span>
              <strong className="ctx-metric-val">{fmtTokens(br.max)}</strong>
              <span className="ctx-metric-sub">{cost != null ? fmtCost(cost) : "tokens"}</span>
            </div>
          </div>

          <div className="ctx-progress-section">
            <div className="ctx-multi-bar" aria-hidden>
              {br.slices.map((s) => (
                <div
                  key={s.id}
                  className="ctx-slice-bar"
                  style={{
                    width: `${Math.max(1, (s.tokens / br.max) * 100)}%`,
                    backgroundColor: s.color,
                  }}
                  title={`${s.label}: ${fmtTokens(s.tokens)} (${Math.round((s.tokens / br.max) * 100)}%)`}
                />
              ))}
            </div>
            <div className="ctx-compact-marker" style={{ left: `${compactAt}%` }} title={`Auto-compacts at ${compactAt}%`}>
              <span>Compact {compactAt}%</span>
            </div>
          </div>

          <div className="ctx-breakdown-list">
            {br.slices.map((s) => {
              const p = Math.round((s.tokens / br.max) * 100);
              return (
                <div key={s.id} className="ctx-breakdown-item">
                  <div className="ctx-item-left">
                    <span className="ctx-dot" style={{ backgroundColor: s.color }} />
                    <span className="ctx-item-name">{s.label}</span>
                  </div>
                  <div className="ctx-item-right">
                    <span className="ctx-item-tokens">{fmtTokens(s.tokens)}</span>
                    <span className="ctx-item-pct">{p}%</span>
                  </div>
                </div>
              );
            })}
          </div>

          <div className="ctx-pop-footer">
            <button
              type="button"
              className="ctx-btn-compact"
              disabled={compacting}
              onClick={() => void handleCompact()}
            >
              <Minimize2 size={12} />
              <span>{compacting ? "Compacting…" : "Compact Now"}</span>
            </button>
            <button
              type="button"
              className="ctx-btn-inspect"
              onClick={() => {
                setOpen(false);
                setInspect(true);
              }}
            >
              <Eye size={12} />
              <span>Inspect Prompt</span>
            </button>
          </div>
        </div>
      )}

      {inspect && <PromptInspector onClose={() => setInspect(false)} />}
    </div>
  );
}
