import { useState } from "react";
import {
  ChevronRight,
  FilePenLine,
  FilePlus,
  FileText,
  Search,
  SquareTerminal,
  Workflow,
  Wrench,
} from "lucide-react";
import { familyTitle, groupSummary, pathPills, toolPath } from "../lib/toolGroups";
import { fmtDur } from "../lib/toolUi";
import { tailLines, type Msg } from "../lib/events";
import { ToolCard } from "./ToolCard";

const ICONS: Record<string, typeof FileText> = {
  Read: FileText,
  Write: FilePlus,
  Edit: FilePenLine,
  Search: Search,
  Bash: SquareTerminal,
  Task: Workflow,
};

function Elapsed({ tools }: { tools: Msg[] }) {
  const running = tools.some((t) => t.streaming);
  const dur = tools.reduce((n, t) => n + (t.durationMs ?? 0), 0);
  if (running && dur === 0) return <span className="act-dur">…</span>;
  if (dur <= 0) return null;
  return <span className="act-dur">{fmtDur(dur)}</span>;
}

/** One quiet line per tool group; expand for tags and output. */
export function ActionCard({ family, tools }: { family: string; tools: Msg[] }) {
  const [open, setOpen] = useState(false);
  const Icon = ICONS[family] ?? Wrench;
  const pills = pathPills(tools.map(toolPath));
  const hasBody = tools.some((t) => t.output) || pills.length > 0;
  const running = tools.some((t) => t.streaming);
  const failed = tools.some((t) => t.ok === false && !t.streaming);

  return (
    <div className={`act-row${running ? " running" : ""}${failed ? " failed" : ""}`}>
      <button
        type="button"
        className="act-head"
        aria-expanded={open}
        onClick={() => hasBody && setOpen((v) => !v)}
        disabled={!hasBody}
      >
        <span className="act-icon" aria-hidden>
          <Icon size={12} />
        </span>
        <span className="act-title">{familyTitle(family, tools.length)}</span>
        <span className="act-spacer" />
        <Elapsed tools={tools} />
        {hasBody && (
          <span className={`act-chevron${open ? " open" : ""}`}>
            <ChevronRight size={12} />
          </span>
        )}
      </button>
      {open && (
        <div className="act-body">
          <div className="act-sub">{groupSummary(family, tools)}</div>
          {pills.length > 0 && (
            <div className="act-tags">
              {pills.slice(0, 8).map((p) => (
                <span key={p.label} className="act-tag">
                  {p.label} {p.count}
                </span>
              ))}
            </div>
          )}
          {tools.map((m) =>
            m.output ? (
              <pre key={m.id} className={m.streaming ? "tool-tail" : "tool-full"}>
                {m.streaming ? tailLines(m.output).join("\n") : m.output}
              </pre>
            ) : (
              <ToolCard key={m.id} m={m} />
            ),
          )}
        </div>
      )}
    </div>
  );
}
