import { useEffect, useState } from "react";
import { ChevronDown, ChevronRight, FileCode, RefreshCw, X } from "lucide-react";
import { diffForFile, listChangedFiles, type ChangedFile } from "../lib/commands";
import { looksLikeDiff } from "../lib/diffParse";
import { DiffView } from "./DiffView";

/** Codex-style review panel: working-tree changes vs HEAD, with a
 * unified diff per file (full content for untracked files). */
export function DiffPanel({ onClose }: { onClose: () => void }) {
  const [files, setFiles] = useState<ChangedFile[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [openPath, setOpenPath] = useState<string | null>(null);
  const [diff, setDiff] = useState<string>("");
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => {
    let active = true;
    void listChangedFiles()
      .then((f) => {
        if (active) {
          setFiles(f);
          setError(null);
        }
      })
      .catch((e) => {
        if (active) setError(String(e));
      });
    return () => {
      active = false;
    };
  }, []);

  async function refresh() {
    setRefreshing(true);
    try {
      const f = await listChangedFiles();
      setFiles(f);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setRefreshing(false);
    }
  }

  async function toggle(path: string) {
    if (openPath === path) {
      setOpenPath(null);
      return;
    }
    setOpenPath(path);
    setDiff("");
    try {
      setDiff(await diffForFile(path));
    } catch (e) {
      setDiff(`(no diff available: ${String(e)})`);
    }
  }

  return (
    <aside className="diff-panel">
      <div className="diff-head">
        <div className="diff-head-left">
          <span className="diff-title">Workbench Changes</span>
          {files && files.length > 0 && (
            <span className="diff-count-badge">{files.length}</span>
          )}
        </div>
        <div className="diff-head-actions">
          <button
            type="button"
            className={`icon-btn${refreshing ? " spinning" : ""}`}
            title="Refresh changes"
            onClick={() => void refresh()}
          >
            <RefreshCw size={12} />
          </button>
          <button type="button" className="icon-btn" title="Close review pane" onClick={onClose}>
            <X size={13} />
          </button>
        </div>
      </div>
      <div className="diff-body">
        {files === null && !error && <div className="sess-empty">Checking git status…</div>}
        {error && <div className="sess-empty">Git status unavailable: {error}</div>}
        {files?.length === 0 && (
          <div className="sess-empty">Working tree is clean — no modified files.</div>
        )}
        {files?.map((f) => {
          const isOpen = openPath === f.path;
          return (
            <div key={f.path} className={`diff-file${isOpen ? " is-open" : ""}`}>
              <button
                type="button"
                className={`diff-file-head status-${f.status}`}
                onClick={() => void toggle(f.path)}
              >
                {isOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
                <FileCode size={13} className="diff-file-icon" />
                <span className="diff-path">{f.path}</span>
                <span className={`badge ${f.status}`}>{f.status}</span>
              </button>
              {isOpen &&
                (diff === "" ? (
                  <pre className="diff-text">Loading diff…</pre>
                ) : looksLikeDiff(diff) ? (
                  <DiffView text={diff} />
                ) : (
                  <pre className="diff-text">{diff}</pre>
                ))}
            </div>
          );
        })}
      </div>
    </aside>
  );
}
