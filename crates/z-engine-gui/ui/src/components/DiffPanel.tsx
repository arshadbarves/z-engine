import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { diffForFile, listChangedFiles, type ChangedFile } from "../lib/commands";

/** Codex-style review panel: working-tree changes vs HEAD, with a
 * unified diff per file (full content for untracked files). */
export function DiffPanel({ onClose }: { onClose: () => void }) {
  const [files, setFiles] = useState<ChangedFile[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [openPath, setOpenPath] = useState<string | null>(null);
  const [diff, setDiff] = useState<string>("");

  useEffect(() => {
    void listChangedFiles()
      .then((f) => setFiles(f))
      .catch((e) => setError(String(e)));
  }, []);

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
        <span className="diff-title">Review changes</span>
        <button className="icon-btn" title="Close" onClick={onClose}>
          <X size={12} />
        </button>
      </div>
      <div className="diff-body">
        {files === null && !error && <div className="sess-empty">loading…</div>}
        {error && <div className="sess-empty">git unavailable: {error}</div>}
        {files?.length === 0 && (
          <div className="sess-empty">No changes in the working tree.</div>
        )}
        {files?.map((f) => (
          <div key={f.path} className="diff-file">
            <button
              className={`diff-file-head status-${f.status}`}
              onClick={() => void toggle(f.path)}
            >
              <span className={`badge ${f.status}`}>{f.status}</span>
              <span className="diff-path">{f.path}</span>
            </button>
            {openPath === f.path && (
              <pre className="diff-text">{diff || "…"}</pre>
            )}
          </div>
        ))}
      </div>
    </aside>
  );
}
