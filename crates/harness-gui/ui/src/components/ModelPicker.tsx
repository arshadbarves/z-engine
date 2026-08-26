import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { ChevronDown, Search, Sparkles } from "lucide-react";
import { modelStore } from "../lib/events";
import { setModel } from "../lib/commands";
import { shortModel } from "../lib/util";
import { catalogStore, fmtLimit } from "../lib/catalog";

/** Provider-grouped model picker backed by the models.dev catalog
 * (merged with local overrides). Falls back to presets while offline. */
export function ModelPicker() {
  const model = useSyncExternalStore(modelStore.subscribe, () => modelStore.getSnapshot());
  const catalog = useSyncExternalStore(catalogStore.subscribe, () => catalogStore.getSnapshot());
  const [open, setOpen] = useState(false);
  const [custom, setCustom] = useState("");
  const [query, setQuery] = useState("");

  useEffect(() => {
    if (open) void catalogStore.ensure();
  }, [open]);

  async function pick(id: string) {
    setOpen(false);
    setQuery("");
    if (id === model) return;
    try {
      await setModel(id);
      modelStore.set(id);
    } catch (e) {
      console.error(e);
    }
  }

  const groups = useMemo(() => {
    const q = query.trim().toLowerCase();
    const out: { provider: string; items: { id: string; name: string; context?: number; output?: number; reasoning: boolean }[] }[] = [];
    if (!catalog) return out;
    for (const [pid, prov] of Object.entries(catalog)) {
      const items = Object.entries(prov.models)
        .filter(([id, m]) => {
          if (id === model) return false;
          if (!q) return true;
          return (
            id.toLowerCase().includes(q) ||
            m.name.toLowerCase().includes(q) ||
            prov.name.toLowerCase().includes(q)
          );
        })
        .slice(0, 40)
        .map(([id, m]) => ({ id, name: m.name, context: m.context, output: m.output, reasoning: m.reasoning }));
      if (items.length > 0) out.push({ provider: prov.name || pid, items });
    }
    // Providers with shorter names first keeps the big aggregators on top.
    out.sort((a, b) => a.provider.localeCompare(b.provider));
    return out;
  }, [catalog, query, model]);

  return (
    <div className="model-picker">
      {open && <div className="popover-backdrop" onClick={() => setOpen(false)} />}
      <button className="mode model-btn" onClick={() => setOpen((o) => !o)} title="Switch model">
        <Sparkles size={11} />
        <span>{shortModel(model) || "model"}</span>
        <ChevronDown size={9} strokeWidth={2.4} />
      </button>
      {open && (
        <div className="popover popover-wide" role="menu">
          <div className="popover-head">Model</div>
          <div className="popover-current">{model || "(default from config)"}</div>
          <div className="pop-search">
            <Search size={11} />
            <input
              value={query}
              onChange={(e) => setQuery(e.currentTarget.value)}
              placeholder="Filter models…"
              spellCheck={false}
              autoFocus
              onKeyDown={(e) => e.key === "Escape" && setOpen(false)}
            />
          </div>
          <div className="popover-scroll">
            {groups.length === 0 && !query && (
              <div className="pop-note">loading catalog…</div>
            )}
            {groups.map((g) => (
              <div key={g.provider}>
                <div className="palette-group">{g.provider}</div>
                {g.items.map((m) => (
                  <button
                    key={`${g.provider}-${m.id}`}
                    className="popover-item model-item"
                    role="menuitem"
                    onClick={() => void pick(m.id)}
                  >
                    <span className="model-name">{m.name}</span>
                    <span className="popover-sub">
                      {m.reasoning ? "reasoning · " : ""}
                      {[fmtLimit(m.context), fmtLimit(m.output)].filter(Boolean).join(" / ") || m.id}
                    </span>
                  </button>
                ))}
              </div>
            ))}
          </div>
          <form
            className="popover-custom"
            onSubmit={(e) => {
              e.preventDefault();
              const id = custom.trim();
              if (id) void pick(id);
              setCustom("");
            }}
          >
            <input
              value={custom}
              onChange={(e) => setCustom(e.currentTarget.value)}
              placeholder="Custom model id…"
              spellCheck={false}
            />
            <button type="submit" disabled={!custom.trim()}>
              Set
            </button>
          </form>
        </div>
      )}
    </div>
  );
}
