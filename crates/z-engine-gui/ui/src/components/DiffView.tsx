import { parseGitDiff } from "../lib/diffParse";

function fileName(path: string): string {
  const i = path.lastIndexOf("/");
  return i >= 0 ? path.slice(i + 1) : path;
}

function dirName(path: string): string {
  const i = path.lastIndexOf("/");
  return i > 0 ? path.slice(0, i) : "";
}

function sign(kind: "add" | "del" | "ctx"): string {
  if (kind === "add") return "+";
  if (kind === "del") return "−";
  return "";
}

/** GitHub-style inline diff: file bar, line numbers, +/- gutters. */
export function DiffView({ text }: { text: string }) {
  const d = parseGitDiff(text);
  const name = d.path ? fileName(d.path) : "diff";
  const dir = d.path ? dirName(d.path) : "";

  return (
    <div className="diff-view">
      <div className="diff-filebar">
        <span className="diff-file-name">{name}</span>
        {dir ? <span className="diff-file-dir">{dir}</span> : null}
        <span className="diff-stat">
          {d.added > 0 ? <span className="add">+{d.added}</span> : null}
          {d.deleted > 0 ? <span className="del">−{d.deleted}</span> : null}
        </span>
      </div>
      <div className="diff-rows">
        {d.rows.map((r, i) =>
          r.kind === "hunk" ? (
            <div key={i} className="diff-row hunk">
              {r.newNo != null ? `line ${r.newNo}` : "···"}
            </div>
          ) : (
            <div key={i} className={`diff-row ${r.kind}`}>
              <span className="diff-no">{r.oldNo ?? ""}</span>
              <span className="diff-no">{r.newNo ?? ""}</span>
              <span className="diff-sign">{sign(r.kind)}</span>
              <span className="diff-code">{r.text || "\u00a0"}</span>
            </div>
          ),
        )}
      </div>
    </div>
  );
}
