import { useEffect, useState } from "react";
import {
  getConfig,
  listMcpServers,
  listPermissionRules,
  removeCostOverride,
  removePermissionRule,
  saveGeneral,
  savePermissionRule,
  setCostOverride,
  testMcpServer,
  type HarnessConfig,
  type McpServerInfo,
} from "../lib/commands";
import { configStore } from "../lib/configStore";
import { modelStore } from "../lib/events";

type Tab = "general" | "permissions" | "mcp" | "cost";

const TABS: Array<{ id: Tab; label: string }> = [
  { id: "general", label: "General" },
  { id: "permissions", label: "Permissions" },
  { id: "mcp", label: "MCP servers" },
  { id: "cost", label: "Cost" },
];

function GeneralTab({ cfg }: { cfg: HarnessConfig }) {
  const [model, setModel] = useState(cfg.model);
  const [baseUrl, setBaseUrl] = useState(cfg.baseUrl ?? "");
  const [maxCtx, setMaxCtx] = useState(String(cfg.maxContextTokens));
  const [review, setReview] = useState(Boolean(cfg.reviewEnabled));
  const [saved, setSaved] = useState(false);

  async function save() {
    await saveGeneral({
      model: model.trim() || null,
      baseUrl: baseUrl.trim() || null,
      maxContextTokens: Number(maxCtx) > 0 ? Number(maxCtx) : null,
      review,
    });
    if (model.trim()) modelStore.set(model.trim());
    configStore.set(await getConfig());
    setSaved(true);
    setTimeout(() => setSaved(false), 1600);
  }

  return (
    <div className="tab-body">
      <label className="form-row">
        <span>Model id</span>
        <input value={model} onChange={(e) => setModel(e.currentTarget.value)} spellCheck={false} />
      </label>
      <label className="form-row">
        <span>Base URL</span>
        <input value={baseUrl} onChange={(e) => setBaseUrl(e.currentTarget.value)} spellCheck={false} />
      </label>
      <label className="form-row">
        <span>Max context tokens</span>
        <input
          type="number"
          value={maxCtx}
          onChange={(e) => setMaxCtx(e.currentTarget.value)}
        />
      </label>
      <label className="form-row check">
        <input
          type="checkbox"
          checked={review}
          onChange={(e) => setReview(e.currentTarget.checked)}
        />
        <span>Post-edit reviewer pass</span>
      </label>
      <p className="form-note">
        Written to <code>.harness/config.toml</code>; the model applies to new turns immediately.
      </p>
      <div className="tab-actions">
        <button className="primary" onClick={() => void save()}>
          {saved ? "Saved ✓" : "Save"}
        </button>
      </div>
    </div>
  );
}

function PermissionsTab() {
  const [rules, setRules] = useState<string[]>([]);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    listPermissionRules()
      .then(setRules)
      .catch(() => setRules([]));
  }, []);

  async function add() {
    const rule = draft.trim();
    if (!rule) return;
    await savePermissionRule(rule);
    setDraft("");
    setRules(await listPermissionRules());
  }

  async function remove(rule: string) {
    await removePermissionRule(rule);
    setRules(await listPermissionRules());
  }

  return (
    <div className="tab-body">
      <p className="form-note">Bash prefix rules — matching commands skip approval.</p>
      <ul className="rule-list">
        {rules.map((r) => (
          <li key={r}>
            <code>{r}</code>
            <button className="mini" title={`Remove ${r}`} onClick={() => void remove(r)}>
              ✕
            </button>
          </li>
        ))}
        {rules.length === 0 && <li className="none">No rules yet.</li>}
      </ul>
      <form
        className="inline-form"
        onSubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <input
          value={draft}
          onChange={(e) => setDraft(e.currentTarget.value)}
          placeholder='e.g. "cargo test*"'
          spellCheck={false}
        />
        <button type="submit" disabled={!draft.trim()}>
          Add rule
        </button>
      </form>
    </div>
  );
}

function McpTab() {
  const [servers, setServers] = useState<McpServerInfo[]>([]);
  const [testing, setTesting] = useState<string | null>(null);
  const [result, setResult] = useState<Record<string, string>>({});

  useEffect(() => {
    listMcpServers()
      .then(setServers)
      .catch(() => setServers([]));
  }, []);

  async function test(name: string) {
    setTesting(name);
    try {
      const tools = await testMcpServer(name);
      setResult((r) => ({ ...r, [name]: `${tools.length} tool${tools.length === 1 ? "" : "s"}: ${tools.join(", ") || "(none)"}` }));
    } catch (e) {
      setResult((r) => ({ ...r, [name]: `failed: ${String(e)}` }));
    } finally {
      setTesting(null);
    }
  }

  return (
    <div className="tab-body">
      <p className="form-note">Stdio MCP servers from config. Test spawns the server and calls tools/list.</p>
      {servers.length === 0 && <p className="none">No MCP servers configured — add an <code>[mcp.servers.&lt;name&gt;]</code> table to your config file.</p>}
      {servers.map((s) => (
        <div key={s.name} className="mcp-row">
          <div className="mcp-head">
            <strong>{s.name}</strong>
            <code className="mcp-cmd">{[s.command, ...s.args].join(" ")}</code>
            <button disabled={testing === s.name} onClick={() => void test(s.name)}>
              {testing === s.name ? "Testing…" : "Test"}
            </button>
          </div>
          {result[s.name] && <code className="mcp-result">{result[s.name]}</code>}
        </div>
      ))}
    </div>
  );
}

function CostTab({ cfg }: { cfg: HarnessConfig }) {
  const overrides = cfg.costOverrides ?? {};
  const [model, setModel] = useState("");
  const [usdIn, setUsdIn] = useState("");
  const [usdOut, setUsdOut] = useState("");

  async function refresh() {
    configStore.set(await getConfig());
  }

  async function add() {
    const m = model.trim();
    const i = Number(usdIn);
    const o = Number(usdOut);
    if (!m || !(i >= 0) || !(o >= 0)) return;
    await setCostOverride(m, i, o);
    setModel("");
    setUsdIn("");
    setUsdOut("");
    await refresh();
  }

  async function remove(m: string) {
    await removeCostOverride(m);
    await refresh();
  }

  return (
    <div className="tab-body">
      <p className="form-note">
        Per-model USD per million tokens. Exact-id overrides beat the built-in family table; unknown models show tokens only.
      </p>
      <ul className="rule-list">
        {Object.entries(overrides).map(([m, p]) => (
          <li key={m}>
            <code>
              {m} · ${p.usdPerMtokInput}/${p.usdPerMtokOutput}
            </code>
            <button className="mini" title={`Remove override for ${m}`} onClick={() => void remove(m)}>
              ✕
            </button>
          </li>
        ))}
        {Object.keys(overrides).length === 0 && (
          <li className="none">No overrides — using the built-in table.</li>
        )}
      </ul>
      <form
        className="inline-form cost-form"
        onSubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <input value={model} onChange={(e) => setModel(e.currentTarget.value)} placeholder="model id" spellCheck={false} />
        <input value={usdIn} onChange={(e) => setUsdIn(e.currentTarget.value)} placeholder="$ in / MTok" type="number" step="any" />
        <input value={usdOut} onChange={(e) => setUsdOut(e.currentTarget.value)} placeholder="$ out / MTok" type="number" step="any" />
        <button type="submit" disabled={!model.trim()}>
          Add override
        </button>
      </form>
    </div>
  );
}

export function SettingsModal({ onClose }: { onClose: () => void }) {
  const [tab, setTab] = useState<Tab>("general");
  const [cfg, setCfg] = useState<HarnessConfig | null>(null);

  useEffect(() => {
    getConfig()
      .then(setCfg)
      .catch(console.error);
  }, []);

  return (
    <div className="modal-overlay" onMouseDown={onClose}>
      <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
        <header className="modal-head">
          <h2>Settings</h2>
          <button className="icon-btn" onClick={onClose} title="Close">
            <svg viewBox="0 0 24 24" width={14} height={14} fill="none" stroke="currentColor"
              strokeWidth={2} strokeLinecap="round" aria-hidden>
              <path d="M18 6L6 18M6 6l12 12" />
            </svg>
          </button>
        </header>
        <nav className="tabs">
          {TABS.map((t) => (
            <button
              key={t.id}
              className={`tab${tab === t.id ? " active" : ""}`}
              onClick={() => setTab(t.id)}
            >
              {t.label}
            </button>
          ))}
        </nav>
        {!cfg && <div className="tab-body">Loading…</div>}
        {cfg && tab === "general" && <GeneralTab key="g" cfg={cfg} />}
        {cfg && tab === "permissions" && <PermissionsTab key="p" />}
        {cfg && tab === "mcp" && <McpTab key="m" />}
        {cfg && tab === "cost" && <CostTab key="c" cfg={cfg} />}
      </div>
    </div>
  );
}
