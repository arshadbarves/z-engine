import { useEffect, useMemo, useState } from "react";
import { ChevronLeft, Copy } from "lucide-react";
import { inspectPrompt, type PromptInspect, type PromptPart, type PromptTool } from "../lib/commands";
import { sessionStore } from "../lib/events";
import { promptInsights } from "../lib/promptInsights";
import { fmtTokens } from "../lib/util";
import { PromptInspectChart } from "./PromptInspectChart";
import "./promptInspect.css";

type Row =
  | { key: string; kind: "msg"; part: PromptPart }
  | { key: string; kind: "tool"; tool: PromptTool };

function rowsOf(snap: PromptInspect): Row[] {
  return [
    ...snap.messages.map((part, i) => ({ key: `m-${i}`, kind: "msg" as const, part })),
    ...snap.tools.map((tool, i) => ({ key: `t-${i}`, kind: "tool" as const, tool })),
  ];
}

function bodyOf(row: Row): string {
  if (row.kind === "msg") return row.part.content;
  return `${row.tool.description}\n\n${row.tool.schema}`;
}

function copyText(snap: PromptInspect): string {
  const ins = promptInsights(snap);
  const chunks = [
    `model: ${snap.model}`,
    snap.sent ? "sent: yes" : "sent: preview / reconstructed",
    `tokens ≈ ${snap.totalTokens}`,
    `largest: ${ins.largest.name} (${Math.round(ins.largest.share * 100)}%)`,
    `stable prefix: ${ins.stablePrefix}`,
    `volatile tail: ${ins.volatileTail}`,
    "",
    "## Order (wire)",
    ...ins.layers.map(
      (l) => `${l.order}. ${l.label} [${l.role}] ~${l.tokens} tok (${Math.round(l.share * 100)}%)`,
    ),
    "",
    "## Hints",
    ...ins.hints.map((h) => `- ${h}`),
    "",
  ];
  for (const m of snap.messages) {
    chunks.push(`## ${m.label} (${m.role}, ~${m.tokens} tok)`, m.content, "");
  }
  if (snap.tools.length) {
    chunks.push("## Tools");
    for (const t of snap.tools) {
      chunks.push(`### ${t.name} (~${t.tokens} tok)`, t.description, t.schema, "");
    }
  }
  return chunks.join("\n");
}

function pct(n: number): string {
  return `${Math.round(n * 100)}%`;
}

/** Case-study view of the last (or restored) chat-completion request. */
export function PromptInspector({ onClose }: { onClose: () => void }) {
  const [snap, setSnap] = useState<PromptInspect | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [sel, setSel] = useState(0);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    const id = sessionStore.getSnapshot() || undefined;
    inspectPrompt(id)
      .then((s) => {
        setSnap(s);
        setErr(null);
      })
      .catch((e: unknown) => {
        setErr(String(e).replace(/^Error:\s*/, ""));
      });
  }, []);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const rows = useMemo(() => (snap ? rowsOf(snap) : []), [snap]);
  const ins = useMemo(() => (snap ? promptInsights(snap) : null), [snap]);
  const active = rows[sel] ?? rows[0];

  async function onCopy() {
    if (!snap) return;
    try {
      await navigator.clipboard.writeText(copyText(snap));
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    } catch (e) {
      console.error(e);
    }
  }

  return (
    <div className="prompt-inspect" role="dialog" aria-label="Prompt inspector">
      <header className="prompt-inspect-head">
        <button type="button" className="prompt-inspect-back" onClick={onClose}>
          <ChevronLeft size={15} />
          Back
        </button>
        <div className="prompt-inspect-title">
          <strong>Prompt assembly</strong>
          <span>
            {snap
              ? snap.sent
                ? "Wire order, budget share, and optimization hints"
                : "Preview — L0 + tools until a turn is sent"
              : "Loading…"}
          </span>
        </div>
        <button
          type="button"
          className="prompt-inspect-copy"
          onClick={() => void onCopy()}
          disabled={!snap}
        >
          <Copy size={12} />
          {copied ? "Copied" : "Copy study"}
        </button>
      </header>
      {err && <p className="prompt-inspect-err">{err}</p>}
      {snap && ins && (
        <>
          <div className="prompt-inspect-overview">
            <div className="prompt-overview-card">
              <em>Active Model</em>
              <strong>{snap.model}</strong>
            </div>
            <div className="prompt-overview-card">
              <em>Total Request</em>
              <strong>
                ~{fmtTokens(snap.totalTokens)} ({snap.messages.length} msgs · {snap.tools.length} tools)
              </strong>
            </div>
            <div className="prompt-overview-card">
              <em>Largest Sink</em>
              <strong title={ins.largest.name}>
                {ins.largest.name} · {pct(ins.largest.share)}
              </strong>
            </div>
            <div className="prompt-overview-card">
              <em>Cache Stability</em>
              <strong>{ins.stablePrefix} stable</strong>
            </div>
          </div>
          <PromptInspectChart ins={ins} />
          {ins.hints.length > 0 && (
            <ul className="prompt-inspect-hints">
              {ins.hints.map((h) => (
                <li key={h}>{h}</li>
              ))}
            </ul>
          )}
          <div className="prompt-inspect-body">
            <nav className="prompt-inspect-nav" aria-label="Prompt parts">
              {rows.map((row, i) => {
                const layer = ins.layers[i];
                const label = row.kind === "msg" ? row.part.label : row.tool.name;
                const hint = row.kind === "msg" ? row.part.role : "tool def";
                const tokens = row.kind === "msg" ? row.part.tokens : row.tool.tokens;
                return (
                  <button
                    key={row.key}
                    type="button"
                    className={i === sel ? "active" : ""}
                    onClick={() => setSel(i)}
                  >
                    <em>
                      <span className="prompt-ord">{layer?.order ?? i + 1}</span>
                      {label}
                    </em>
                    <span>
                      {hint}
                      <i
                        className="prompt-share"
                        style={{ width: pct(layer?.share ?? 0) }}
                      />
                    </span>
                    <strong>{fmtTokens(tokens)}</strong>
                  </button>
                );
              })}
            </nav>
            <div className="prompt-inspect-pane">
              {ins.layers[sel] && (
                <div className="prompt-part-stats">
                  <span className="stat-badge">~{fmtTokens(ins.layers[sel].tokens)}</span>
                  <span className="stat-badge">{ins.layers[sel].chars.toLocaleString()} chars</span>
                  <span className="stat-badge">{ins.layers[sel].lines} lines</span>
                  <span className="stat-badge">{pct(ins.layers[sel].share)} of budget</span>
                  <span className={`stat-badge ${ins.layers[sel].cacheable ? "cacheable" : "volatile"}`}>
                    {ins.layers[sel].cacheable ? "Cacheable" : "Volatile"}
                  </span>
                  <span className="stat-badge">Wire #{ins.layers[sel].order}</span>
                </div>
              )}
              <pre className="prompt-inspect-content">{active ? bodyOf(active) : ""}</pre>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
