import { useMemo, useState } from "react";
import type { SessionEntry } from "../lib/util";
import { wsBasename } from "../lib/workspaces";

export interface PaletteItem {
  label: string;
  hint?: string;
  keywords: string;
  /** Section this item renders under; ungrouped items go last. */
  group?: string;
  run: () => void;
}

/** Subsequence fuzzy match: every query char must appear in order.
 * Score = total span of matched positions (lower is better). */
function fuzzyScore(query: string, item: PaletteItem): number | null {
  const q = query.trim().toLowerCase();
  if (!q) return Number.POSITIVE_INFINITY; // everything matches when empty
  const hay = `${item.label} ${item.keywords}`.toLowerCase();
  let hi = 0;
  let prev = -1;
  let first = -1;
  for (const ch of q) {
    const idx = hay.indexOf(ch, prev + 1);
    if (idx === -1) return null;
    if (first === -1) first = idx;
    if (idx === prev + 1) {
      // contiguity bonus is implicit: keep span tight
      if (hi - first > 64) return null;
    }
    prev = idx;
    hi = idx;
  }
  // Prefer matches near the start of the haystack.
  return hi - Math.max(0, first - 8);
}

export function CommandPalette({
  onClose,
  sessions,
  workspaces,
  activeWorkspace,
  actions,
  onOpenSession,
  onActivateWorkspace,
}: {
  onClose: () => void;
  sessions: SessionEntry[];
  workspaces: string[];
  activeWorkspace: string | null;
  actions: PaletteItem[];
  onOpenSession: (path: string) => void;
  onActivateWorkspace: (root: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [sel, setSel] = useState(0);

  const items = useMemo(() => {
    const sessionItems: PaletteItem[] = sessions.slice(0, 6).map((s) => ({
      label: s.firstUserMsg ?? "(empty)",
      hint: "open session",
      keywords: `session ${s.ulid}`,
      group: "Sessions",
      // Route through openSession (not bare startSession) so the
      // transcript replays into the chat area.
      run: () => onOpenSession(s.path),
    }));
    const wsItems: PaletteItem[] = workspaces.map((root) => ({
      label: `Workspace · ${wsBasename(root)}`,
      hint: root === activeWorkspace ? "active" : "set active",
      keywords: `workspace project ${root}`,
      group: "Workspaces",
      run: () => onActivateWorkspace(root),
    }));
    return [...actions, ...wsItems, ...sessionItems]
      .map((item) => ({ item, score: fuzzyScore(query, item) }))
      .filter(({ score }) => score !== null)
      .sort((a, b) => (a.score as number) - (b.score as number))
      .map(({ item }) => item);
  }, [query, actions, workspaces, activeWorkspace, onOpenSession, onActivateWorkspace, sessions]);

  const selIndex = Math.min(sel, Math.max(0, items.length - 1));

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
    <div className="palette-overlay" onMouseDown={onClose}>
      <div className="palette" onMouseDown={(e) => e.stopPropagation()}>
        <input
          autoFocus
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
          placeholder="Type a command…"
          spellCheck={false}
        />
        <div className="palette-list">
          {items.length === 0 && <div className="pop-note">no matches</div>}
          {groups.map((g) => (
            <div key={g.name ?? "_commands"}>
              {g.name && <div className="palette-group">{g.name}</div>}
              {g.items.map((item) => {
                flatIndex += 1;
                const i = flatIndex;
                return (
                  <button
                    key={`${item.label}-${i}`}
                    className={`pop-item${i === selIndex ? " sel" : ""}`}
                    onMouseEnter={() => setSel(i)}
                    onClick={() => run(i)}
                  >
                    <span className="pop-name">{item.label}</span>
                    {item.hint && <span className="pop-desc">{item.hint}</span>}
                  </button>
                );
              })}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
