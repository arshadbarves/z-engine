import type { PromptInsights, PromptStackSlice } from "../lib/promptInsights";
import { fmtTokens } from "../lib/util";

function pct(n: number): string {
  return `${Math.round(n * 100)}%`;
}

/** Stacked budget bar + cacheable/volatile split for the prompt inspector. */
export function PromptInspectChart({ ins }: { ins: PromptInsights }) {
  const total = Math.max(1, ins.cacheableTokens + ins.volatileTokens);
  return (
    <div className="prompt-chart">
      <div className="prompt-stack" aria-label="Token share by role">
        {ins.stack.map((s: PromptStackSlice) => (
          <i
            key={s.id}
            title={`${s.label} · ${fmtTokens(s.tokens)} (${pct(s.share)})`}
            style={{ width: `${Math.max(0.4, s.share * 100)}%`, background: s.color }}
          />
        ))}
      </div>
      <ul className="prompt-stack-legend">
        {ins.stack.map((s) => (
          <li key={s.id}>
            <i className="swatch" style={{ background: s.color }} />
            {s.label}
            <span>
              {fmtTokens(s.tokens)} · {pct(s.share)}
            </span>
          </li>
        ))}
      </ul>
      <div className="prompt-cache">
        <span>
          Cacheable prefix ~{fmtTokens(ins.cacheableTokens)} ({pct(ins.cacheableTokens / total)})
        </span>
        <span>
          Volatile tail ~{fmtTokens(ins.volatileTokens)} ({pct(ins.volatileTokens / total)})
        </span>
      </div>
    </div>
  );
}
