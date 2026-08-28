import { useEffect, useState } from "react";
import {
  listMcpServers,
  removeMcpServer,
  saveMcpServer,
  testMcpServer,
  type McpServerInfo,
} from "../../lib/commands";

function splitArgs(raw: string): string[] {
  return raw
    .trim()
    .split(/\s+/)
    .filter(Boolean);
}

export function McpTab() {
  const [servers, setServers] = useState<McpServerInfo[]>([]);
  const [testing, setTesting] = useState<string | null>(null);
  const [result, setResult] = useState<Record<string, string>>({});
  const [name, setName] = useState("");
  const [command, setCommand] = useState("");
  const [args, setArgs] = useState("");

  async function refresh() {
    try {
      setServers(await listMcpServers());
    } catch {
      setServers([]);
    }
  }

  useEffect(() => {
    let active = true;
    listMcpServers()
      .then((res) => {
        if (active) setServers(res);
      })
      .catch(() => {
        if (active) setServers([]);
      });
    return () => {
      active = false;
    };
  }, []);

  async function add() {
    const n = name.trim();
    const cmd = command.trim();
    if (!n || !cmd) return;
    await saveMcpServer(n, cmd, splitArgs(args));
    setName("");
    setCommand("");
    setArgs("");
    await refresh();
  }

  async function remove(n: string) {
    await removeMcpServer(n);
    setResult((r) => {
      const next = { ...r };
      delete next[n];
      return next;
    });
    await refresh();
  }

  async function test(n: string) {
    setTesting(n);
    try {
      const tools = await testMcpServer(n);
      setResult((r) => ({
        ...r,
        [n]: `${tools.length} tool${tools.length === 1 ? "" : "s"}: ${tools.join(", ") || "(none)"}`,
      }));
    } catch (e) {
      setResult((r) => ({ ...r, [n]: `failed: ${String(e)}` }));
    } finally {
      setTesting(null);
    }
  }

  return (
    <div className="tab-body">
      <section className="settings-group">
        <h3>Servers</h3>
        <p className="form-note">
          Stdio MCP servers for this project. Test spawns the server and lists tools. New chats pick
          them up.
        </p>
        {servers.length === 0 && <p className="none">No MCP servers yet.</p>}
      {servers.map((s) => (
        <div key={s.name} className="mcp-row">
          <div className="mcp-head">
            <strong>{s.name}</strong>
            <code className="mcp-cmd">{[s.command, ...s.args].join(" ")}</code>
            <button disabled={testing === s.name} onClick={() => void test(s.name)} type="button">
              {testing === s.name ? "Testing…" : "Test"}
            </button>
            <button className="mini" title={`Remove ${s.name}`} onClick={() => void remove(s.name)} type="button">
              ✕
            </button>
          </div>
          {result[s.name] && <code className="mcp-result">{result[s.name]}</code>}
        </div>
      ))}
      <form
        className="mcp-add"
        onSubmit={(e) => {
          e.preventDefault();
          void add();
        }}
      >
        <div className="settings-card">
          <label className="form-row">
            <span className="form-label-title">Name</span>
            <span className="form-label-desc">Short id used in config, e.g. filesystem</span>
            <input
              value={name}
              onChange={(e) => setName(e.currentTarget.value)}
              spellCheck={false}
              placeholder="filesystem"
            />
          </label>
          <label className="form-row">
            <span className="form-label-title">Command</span>
            <span className="form-label-desc">Executable that speaks MCP over stdio</span>
            <input
              value={command}
              onChange={(e) => setCommand(e.currentTarget.value)}
              spellCheck={false}
              placeholder="npx"
            />
          </label>
          <label className="form-row">
            <span className="form-label-title">Args</span>
            <span className="form-label-desc">Space-separated arguments</span>
            <input
              value={args}
              onChange={(e) => setArgs(e.currentTarget.value)}
              spellCheck={false}
              placeholder="-y @modelcontextprotocol/server-filesystem ."
            />
          </label>
        </div>
        <div className="tab-actions">
          <button className="primary" type="submit" disabled={!name.trim() || !command.trim()}>
            Add server
          </button>
        </div>
      </form>
      </section>
    </div>
  );
}
