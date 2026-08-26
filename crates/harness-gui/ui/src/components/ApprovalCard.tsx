import { useState } from "react";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { oneDark } from "react-syntax-highlighter/dist/esm/styles/prism";
import type { Msg } from "../lib/events";

function looksLikeDiff(text: string): boolean {
  return /(^|\n)(@@ |--- |\+\+\+ |diff --git )/.test(text);
}

/** Approval card with four scope buttons; rich diff preview is
 * syntax-highlighted and collapsible when long. */
export function ApprovalCard({
  m,
  onApprove,
  onDeny,
}: {
  m: Msg;
  onApprove: (decision: "once" | "session" | "persist") => void;
  onDeny: () => void;
}) {
  const [showDiff, setShowDiff] = useState(false);

  const body = m.text.replace(/^⚠ approval required — /, "");
  const nl = body.indexOf("\n");
  const tool = nl >= 0 ? body.slice(0, nl) : body;
  const detail = nl >= 0 ? body.slice(nl + 1) : "";
  const diff = m.detailPreview ?? null;
  const diffLines = diff ? diff.split("\n").length : 0;
  const longDiff = diffLines > 15;

  return (
    <div className="msg approval">
      <div className="approval-title">{tool}</div>
      {detail && <div className="approval-body">{detail}</div>}
      {diff && looksLikeDiff(diff) && (
        <div className="approval-diff">
          <button className="diff-toggle" onClick={() => setShowDiff((s) => !s)}>
            {showDiff ? "Hide" : "Show"} diff · {diffLines} lines
          </button>
          {(showDiff || !longDiff) && (
            <SyntaxHighlighter
              language="diff"
              style={oneDark}
              wrapLongLines={false}
              customStyle={{
                margin: "8px 0 0",
                borderRadius: 8,
                border: "1px solid var(--border)",
                fontSize: 11.5,
                maxHeight: 320,
                overflow: "auto",
                background: "#0d0d10",
              }}
            >
              {diff}
            </SyntaxHighlighter>
          )}
        </div>
      )}
      <div className="approval-actions">
        <button className="primary" onClick={() => onApprove("once")}>
          Allow once
        </button>
        <button onClick={() => onApprove("session")}>Always · session</button>
        {m.canPersist && <button onClick={() => onApprove("persist")}>Always · persist</button>}
        <button className="deny" onClick={onDeny}>
          Deny
        </button>
        <span className="hint">
          <kbd>y</kbd> once <kbd>s</kbd> session <kbd>p</kbd> persist <kbd>n</kbd> deny
        </span>
      </div>
    </div>
  );
}
