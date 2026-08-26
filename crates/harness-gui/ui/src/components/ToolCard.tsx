import { useEffect, useState } from "react";
import { tailLines, type Msg } from "../lib/events";

/** Live elapsed timer for running cards (interval-driven state only —
 * no Date.now() during render). */
function Elapsed() {
  const [secs, setSecs] = useState(0);
  useEffect(() => {
    const t = setInterval(() => setSecs((s) => s + 0.1), 100);
    return () => clearInterval(t);
  }, []);
  return <span className="tool-elapsed">{secs.toFixed(1)}s</span>;
}

function fmtDur(ms: number): string {
  return ms >= 60_000 ? `${(ms / 60_000).toFixed(1)}m` : `${(ms / 1000).toFixed(1)}s`;
}

/** Human label per tool (Claude Code style). */
function toolLabel(name: string): string {
  const map: Record<string, string> = {
    read_file: "Read",
    write_file: "Write",
    edit_file: "Edit",
    bash: "Bash",
    grep: "Grep",
    glob: "Glob",
    task: "Task",
    update_context_notes: "Notes",
    go_to_definition: "Definition",
    find_references: "References",
    lsp_diagnostics: "Diagnostics",
  };
  if (map[name]) return map[name];
  const words = name.replace(/[-_]/g, " ").split(" ");
  return words.map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(" ");
}

/** Core summaries echo the tool name (`bash (0): cmd…`, `read_file: …`);
 * the card already shows the tool label, so strip the prefix. */
function cleanSummary(toolName: string | undefined, summary: string): string {
  let s = summary;
  if (toolName) {
    s = s.replace(new RegExp(`^${toolName}\\s+\\(\\d+\\):\\s*`), "");
    s = s.replace(new RegExp(`^${toolName}\\s*:\\s*`), "");
  }
  s = s.replace(/^\(timed out\):\s*/, "");
  return s || summary;
}

function Chevron({ open }: { open: boolean }) {
  return (
    <svg viewBox="0 0 24 24" width={11} height={11} fill="none" stroke="currentColor"
      strokeWidth={2.2} strokeLinecap="round" strokeLinejoin="round"
      aria-hidden className={`tool-chevron${open ? " open" : ""}`}>
      <path d="M9 18l6-6-6-6" />
    </svg>
  );
}

/** Tool call card, Claude Code style: one quiet row — status dot, tool
 * label, truncated argument preview, duration — click expands the full
 * output. Running cards show elapsed + live stdout tail. */
export function ToolCard({ m }: { m: Msg }) {
  const [expanded, setExpanded] = useState(false);
  const label = toolLabel(m.toolName ?? "");

  if (m.streaming) {
    const tail = tailLines(m.output ?? "").join("\n");
    return (
      <div className="msg tool-card running">
        <div className="tool-row">
          <span className="tool-dot running" aria-hidden>
            <span className="tool-dot-inner" />
          </span>
          <span className="tool-label">{label}</span>
          <span className="tool-arg">{m.preview}</span>
          <Elapsed />
        </div>
        {m.output && m.output.length > 0 && <pre className="tool-tail">{tail || "…"}</pre>}
      </div>
    );
  }

  const hasOutput = Boolean(m.output && m.output.length > 0);
  return (
    <div
      className={`msg tool-card ${m.ok ? "ok" : "bad"}${hasOutput ? " expandable" : ""}`}
    >
      <button
        className="tool-row"
        onClick={() => hasOutput && setExpanded((e) => !e)}
        disabled={!hasOutput}
        aria-expanded={hasOutput ? expanded : undefined}
      >
        <span className="tool-dot" aria-hidden>
          <span className="tool-dot-inner" />
        </span>
        <span className="tool-label">{label}</span>
        <span className="tool-arg">{cleanSummary(m.toolName, m.summary || m.preview || "")}</span>
        {hasOutput && <Chevron open={expanded} />}
        <span className="tool-elapsed">{m.durationMs ? fmtDur(m.durationMs) : ""}</span>
      </button>
      {expanded && hasOutput && <pre className="tool-full">{m.output}</pre>}
    </div>
  );
}
