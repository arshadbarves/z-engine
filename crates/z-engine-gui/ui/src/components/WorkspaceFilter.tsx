import { useEffect, useRef, useState } from "react";
import { Check, ChevronDown, Folder, FolderPlus, Plus, Trash2 } from "lucide-react";
import { wsBasename } from "../lib/workspaces";

export function WorkspaceFilter({
  workspaces,
  selectedWs,
  onSelectWs,
  onAddWorkspace,
  onRemoveWorkspace,
  onActivateWorkspace,
}: {
  workspaces: string[];
  selectedWs: string | "all";
  onSelectWs: (ws: string | "all") => void;
  onAddWorkspace: () => void;
  onRemoveWorkspace: (root: string) => void;
  onActivateWorkspace: (root: string | null) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function handlePointerDown(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    window.addEventListener("pointerdown", handlePointerDown);
    return () => window.removeEventListener("pointerdown", handlePointerDown);
  }, [open]);

  if (workspaces.length === 0) return null;

  const currentLabel = selectedWs === "all" ? "All Projects" : wsBasename(selectedWs);

  return (
    <div className="sess-ws-filter-bar" ref={ref}>
      <button
        type="button"
        className={`sess-ws-pill${open ? " active" : ""}`}
        onClick={() => setOpen(!open)}
        title="Filter by project workspace"
      >
        <Folder size={12} className="sess-ws-icon" />
        <span className="sess-ws-label">{currentLabel}</span>
        <ChevronDown size={11} className="sess-ws-chevron" />
      </button>

      <button
        type="button"
        className="sess-ws-add-btn"
        title="Add project folder…"
        onClick={onAddWorkspace}
      >
        <Plus size={12} />
      </button>

      {open && (
        <div className="sess-ws-menu" role="menu">
          <div className="sess-ws-menu-header">Projects</div>
          <button
            type="button"
            className={`sess-ws-menu-item${selectedWs === "all" ? " selected" : ""}`}
            onClick={() => {
              onSelectWs("all");
              setOpen(false);
            }}
          >
            <div className="sess-ws-menu-item-left">
              <Folder size={12} />
              <span>All Projects</span>
            </div>
            {selectedWs === "all" && <Check size={12} className="sess-ws-check" />}
          </button>

          {workspaces.map((root) => (
            <div key={root} className="sess-ws-menu-row">
              <button
                type="button"
                className={`sess-ws-menu-item${selectedWs === root ? " selected" : ""}`}
                onClick={() => {
                  onSelectWs(root);
                  onActivateWorkspace(root);
                  setOpen(false);
                }}
                title={root}
              >
                <div className="sess-ws-menu-item-left">
                  <Folder size={12} />
                  <span className="sess-ws-name">{wsBasename(root)}</span>
                </div>
                {selectedWs === root && <Check size={12} className="sess-ws-check" />}
              </button>
              <button
                type="button"
                className="sess-ws-menu-remove"
                title="Remove project from list"
                onClick={(e) => {
                  e.stopPropagation();
                  onRemoveWorkspace(root);
                  if (selectedWs === root) onSelectWs("all");
                }}
              >
                <Trash2 size={11} />
              </button>
            </div>
          ))}

          <div className="sess-ws-menu-divider" />
          <button
            type="button"
            className="sess-ws-menu-action"
            onClick={() => {
              setOpen(false);
              onAddWorkspace();
            }}
          >
            <FolderPlus size={13} />
            <span>Add Workspace Folder…</span>
          </button>
        </div>
      )}
    </div>
  );
}
