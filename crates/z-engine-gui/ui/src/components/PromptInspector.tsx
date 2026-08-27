import { useEffect, useMemo, useState } from "react";
import { ChevronLeft, Copy } from "lucide-react";
import { inspectPrompt, type PromptInspect, type PromptPart, type PromptTool } from "../lib/commands";
import { sessionStore } from "../lib/events";
import { fmtTokens } from "../lib/util";
import "./promptInspect.css";

type Row =
  | { key: string; kind: "msg"; part: PromptPart }
  | { key: string; kind: "tool"; tool: PromptTool };

function rowsOf(snap: PromptInspect): Row[] {
  const msgs: Row[] = snap.messages.map((part, i) => ({
    key: `m-${i}`,
    kind: "msg",
    part,
  }));
  const tools: Row[] = snap.tools.map((tool, i) => ({
    key: `t-${i}`,
    kind: "tool",
    tool,
  }));
  return [...msgs, ...tools];
}

function bodyOf(row: Row): string {
  if (row.kind === "msg") return row.part.content;
  return `${row.tool.description}\n\n${row.tool.schema}`;
}

function copyText(snap: PromptInspect): string {
  const chunks = [
    `model: ${snap.model}`,
    snap.sent ? "sent: yes" : "sent: preview",
    `tokens ≈ ${snap.totalTokens}`,
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

/** Debug overlay: the exact chat-completion payload last assembled for the LLM. */
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
          <strong>Prompt</strong>
          <span>
            {snap
              ? snap.sent
                ? "Last request sent to the model"
                : "Preview — not sent yet"
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
          {copied ? "Copied" : "Copy all"}
        </button>
      </header>
      {err && <p className="prompt-inspect-err">{err}</p>}
      {snap && (
        <>
          <div className="prompt-inspect-sum">
            <span>{snap.model}</span>
            <span>
              {snap.messages.length} messages · {snap.tools.length} tools · ~
              {fmtTokens(snap.totalTokens)}
            </span>
          </div>
          <div className="prompt-inspect-body">
            <nav className="prompt-inspect-nav" aria-label="Prompt parts">
              {rows.map((row, i) => {
                const label = row.kind === "msg" ? row.part.label : row.tool.name;
                const hint = row.kind === "msg" ? row.part.role : "tool";
                const tokens = row.kind === "msg" ? row.part.tokens : row.tool.tokens;
                return (
                  <button
                    key={row.key}
                    type="button"
                    className={i === sel ? "active" : ""}
                    onClick={() => setSel(i)}
                  >
                    <em>{label}</em>
                    <span>{hint}</span>
                    <strong>{fmtTokens(tokens)}</strong>
                  </button>
                );
              })}
            </nav>
            <pre className="prompt-inspect-content">{active ? bodyOf(active) : ""}</pre>
          </div>
        </>
      )}
    </div>
  );
}
