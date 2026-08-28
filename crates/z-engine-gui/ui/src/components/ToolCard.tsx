import { useEffect, useState } from "react";
import { tailLines, type Msg } from "../lib/events";
import { cleanSummary, fmtDur, toolLabel } from "../lib/toolUi";

function Elapsed() {
  const [secs, setSecs] = useState(0);
  useEffect(() => {
    const t = setInterval(() => setSecs((s) => s + 0.1), 100);
    return () => clearInterval(t);
  }, []);
  return <span className="tool-elapsed">{secs.toFixed(1)}s</span>;
}

function Chevron({ open }: { open: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={11}
      height={11}
      fill="none"
      stroke="currentColor"
      strokeWidth={2.2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      className={`tool-chevron${open ? " open" : ""}`}
    >
      <path d="M9 18l6-6-6-6" />
    </svg>
  );
}

function OutputBox({ output, streaming }: { output: string; streaming?: boolean }) {
  const [copied, setCopied] = useState(false);
  const content = streaming ? tailLines(output).join("\n") : output;
  async function copy() {
    try {
      await navigator.clipboard.writeText(output);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {
      console.error("Failed to copy output");
    }
  }
  return (
    <div className="tool-output-wrap">
      <div className="tool-output-bar">
        <span className="tool-output-lines">{output.split("\n").length} lines</span>
        <button type="button" className="tool-copy-btn" onClick={() => void copy()} title="Copy output">
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      <pre className={streaming ? "tool-tail" : "tool-full"}>{content}</pre>
    </div>
  );
}

/** Quiet one-line tool row. Output is hidden until the user expands it. */
export function ToolCard({ m }: { m: Msg }) {
  const [expanded, setExpanded] = useState(false);
  const label = toolLabel(m.toolName ?? "");
  const hasOutput = Boolean(m.output && m.output.length > 0);
  const canExpand = hasOutput || Boolean(m.streaming && m.output);
  const summary = cleanSummary(m.toolName, m.summary || m.preview || "");

  return (
    <div
      className={`msg tool-card ${m.streaming ? "running" : m.ok === false ? "bad" : "ok"}${
        canExpand ? " expandable" : ""
      }`}
    >
      <button
        className="tool-row"
        onClick={() => canExpand && setExpanded((e) => !e)}
        disabled={!canExpand}
        aria-expanded={canExpand ? expanded : undefined}
      >
        <span className="tool-dot" aria-hidden>
          <span className="tool-dot-inner" />
        </span>
        <span className="tool-label">{label}</span>
        <span className="tool-arg">{summary}</span>
        {canExpand && <Chevron open={expanded} />}
        {m.streaming ? <Elapsed /> : <span className="tool-elapsed">{m.durationMs ? fmtDur(m.durationMs) : ""}</span>}
      </button>
      {expanded && hasOutput && (
        <OutputBox output={m.output ?? ""} streaming={m.streaming} />
      )}
    </div>
  );
}
