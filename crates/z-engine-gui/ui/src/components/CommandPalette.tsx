import { useEffect, useMemo, useRef, useState } from "react";
import {
  FolderGit2,
  MessageSquare,
  Search,
  X,
  type IconComponent,
} from "../lib/icons";
import type { SessionEntry } from "../lib/util";
import { sessionLabel } from "../lib/sessionList";
import { wsBasename } from "../lib/workspaces";

export interface PaletteItem {
  label: string;
  hint?: string;
  keywords: string;
  group?: string;
  icon?: IconComponent;
  shortcut?: string;
  run: () => void;
}

/** Subsequence fuzzy match: every query char must appear in order.
 * Score = total span of matched positions (lower is better). */
function fuzzyScore(query: string, item: PaletteItem): number | null {
  const q = query.trim().toLowerCase();
  if (!q) return Number.POSITIVE_INFINITY; // everything matches when empty
  const hay = `${item.label} ${item.keywords} ${item.group ?? ""}`.toLowerCase();
  let hi = 0;
  let prev = -1;
  let first = -1;
  for (const ch of q) {
    const idx = hay.indexOf(ch, prev + 1);
    if (idx === -1) return null;
    if (first === -1) first = idx;
    if (idx === prev + 1) {
      if (hi - first > 64) return null;
    }
    prev = idx;
    hi = idx;
  }
  return hi - Math.max(0, first - 8);
}

export function CommandPalette({
  isClosing = false,
  onClose,
  sessions,
  workspaces,
  activeWorkspace,
  actions,
  onOpenSession,
  onActivateWorkspace,
}: {
  isClosing?: boolean;
  onClose: () => void;
  sessions: SessionEntry[];
  workspaces: string[];
  activeWorkspace: string | null;
  actions: PaletteItem[];
  onOpenSession: (path: string, projectRoot?: string | null) => void;
  onActivateWorkspace: (root: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [sel, setSel] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);

  const items = useMemo(() => {
    const sessionItems: PaletteItem[] = sessions.slice(0, 8).map((s) => ({
      label: sessionLabel(s.firstUserMsg),
      hint: s.projectRoot ? wsBasename(s.projectRoot) : "Chat",
      keywords: `session chat ${s.ulid} ${s.projectRoot ?? ""}`,
      group: "Recent Chats",
      icon: MessageSquare,
      run: () => onOpenSession(s.path, s.projectRoot),
    }));

    const wsItems: PaletteItem[] = workspaces.map((root) => ({
      label: wsBasename(root),
      hint: root === activeWorkspace ? "Active Workspace" : "Switch Workspace",
      keywords: `workspace project folder ${root}`,
      group: "Workspaces",
      icon: FolderGit2,
      run: () => onActivateWorkspace(root),
    }));

    return [...actions, ...wsItems, ...sessionItems]
      .map((item) => ({ item, score: fuzzyScore(query, item) }))
      .filter(({ score }) => score !== null)
      .sort((a, b) => (a.score as number) - (b.score as number))
      .map(({ item }) => item);
  }, [query, actions, workspaces, activeWorkspace, onOpenSession, onActivateWorkspace, sessions]);

  const selIndex = Math.min(sel, Math.max(0, items.length - 1));

  // Scroll active item into view smoothly
  useEffect(() => {
    const activeEl = listRef.current?.querySelector(".palette-row.is-selected");
    if (activeEl) {
      activeEl.scrollIntoView({ block: "nearest" });
    }
  }, [selIndex]);

  function run(i: number) {
    const item = items[i];
    if (item) {
      onClose();
      item.run();
    }
  }

  // Render grouped, preserving filtered/sorted order within each group.
  const groups: { name: string | undefined; items: PaletteItem[] }[] = [];
  for (const item of items) {
    const g = groups.find((x) => x.name === item.group);
    if (g) g.items.push(item);
    else groups.push({ name: item.group, items: [item] });
  }

  let flatIndex = -1;

  return (
    <div
      className={`palette-backdrop${isClosing ? " is-closing" : ""}`}
      onMouseDown={onClose}
    >
      <div
        className={`palette-spotlight${isClosing ? " is-closing" : ""}`}
        onMouseDown={(e) => e.stopPropagation()}
      >
        {/* Search Header Bar */}
        <div className="palette-header">
          <div className="palette-search-icon-box">
            <Search size={15} strokeWidth={2} />
          </div>
          <input
            autoFocus
            className="palette-input"
            value={query}
            onChange={(e) => {
              setQuery(e.currentTarget.value);
              setSel(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setSel((s) => (s + 1) % Math.max(1, items.length));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setSel((s) => (s - 1 + items.length) % Math.max(1, items.length));
              } else if (e.key === "Enter") {
                e.preventDefault();
                run(selIndex);
              } else if (e.key === "Escape") {
                e.preventDefault();
                onClose();
              }
            }}
            placeholder="Type a command or search actions, chats, workspaces…"
            spellCheck={false}
          />
          {query ? (
            <button
              type="button"
              className="palette-clear-btn"
              title="Clear search"
              onClick={() => {
                setQuery("");
                setSel(0);
              }}
            >
              <X size={13} strokeWidth={2} />
            </button>
          ) : (
            <span className="palette-count-chip">{items.length}</span>
          )}
        </div>

        {/* Results List View */}
        <div className="palette-body" ref={listRef}>
          {items.length === 0 ? (
            <div className="palette-empty-state">
              <div className="palette-empty-icon">
                <Search size={22} strokeWidth={1.5} />
              </div>
              <span className="palette-empty-title">No matching results</span>
              <span className="palette-empty-sub">
                Try typing an action name, workspace, or session keyword.
              </span>
            </div>
          ) : (
            groups.map((g) => (
              <div key={g.name ?? "_general"} className="palette-section">
                {g.name && <div className="palette-section-title">{g.name}</div>}
                {g.items.map((item) => {
                  flatIndex += 1;
                  const i = flatIndex;
                  const isSelected = i === selIndex;
                  const Icon = item.icon ?? Search;

                  return (
                    <button
                      key={`${item.label}-${i}`}
                      type="button"
                      className={`palette-row${isSelected ? " is-selected" : ""}`}
                      onMouseEnter={() => setSel(i)}
                      onClick={() => run(i)}
                    >
                      <div className="palette-row-icon-box">
                        <Icon size={14} strokeWidth={1.8} />
                      </div>

                      <div className="palette-row-content">
                        <span className="palette-row-label">{item.label}</span>
                        {item.hint && (
                          <span className="palette-row-hint">{item.hint}</span>
                        )}
                      </div>

                      <div className="palette-row-trailing">
                        {item.shortcut ? (
                          <kbd className="palette-shortcut-badge">{item.shortcut}</kbd>
                        ) : isSelected ? (
                          <kbd className="palette-shortcut-badge">↵</kbd>
                        ) : null}
                      </div>
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>

        {/* Bottom Status Deck */}
        <div className="palette-footer">
          <div className="palette-footer-shortcuts">
            <span className="footer-shortcut-item">
              <kbd>↑↓</kbd> Navigate
            </span>
            <span className="footer-shortcut-item">
              <kbd>↵</kbd> Execute
            </span>
            <span className="footer-shortcut-item">
              <kbd>Esc</kbd> Close
            </span>
          </div>

          <div className="palette-footer-brand">
            <span className="footer-brand-text">Z Engine Spotlight</span>
          </div>
        </div>
      </div>
    </div>
  );
}

