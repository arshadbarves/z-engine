import { useState } from "react";
import { GitBranch, X } from "lucide-react";

/** Name prompt for a new git worktree: creates
 * `.z-engine/worktrees/<name>` on branch `zengine/<name>`, registers it as
 * a workspace and starts a task there (handled by the caller). */
export function WorktreeModal({
  onClose,
  onCreate,
}: {
  onClose: () => void;
  onCreate: (name: string) => void;
}) {
  const [name, setName] = useState("");
  const slug = name.toLowerCase().replace(/[^a-z0-9-]/g, "");
  return (
    <div className="palette-overlay" onMouseDown={onClose}>
      <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <GitBranch size={13} />
          <span>New task in a git worktree</span>
          <button className="icon-btn" onClick={onClose}>
            <X size={12} />
          </button>
        </div>
        <p className="modal-sub">
          Creates an isolated checkout at{" "}
          <code>.z-engine/worktrees/{slug || "<name>"}</code> on branch{" "}
          <code>zengine/{slug || "<name>"}</code>, then starts a session there.
          The main working tree stays untouched.
        </p>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (slug) onCreate(slug);
          }}
        >
          <input
            autoFocus
            value={name}
            onChange={(e) => setName(e.currentTarget.value)}
            placeholder="worktree name (e.g. fix-login)"
            spellCheck={false}
            onKeyDown={(e) => e.key === "Escape" && onClose()}
          />
          <div className="modal-actions">
            <button type="button" className="btn-ghost" onClick={onClose}>
              Cancel
            </button>
            <button type="submit" disabled={!slug} className="btn-primary">
              Create &amp; start
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
