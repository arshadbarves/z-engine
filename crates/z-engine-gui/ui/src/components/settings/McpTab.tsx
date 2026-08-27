import { useEffect, useState } from "react";
import { listMcpServers, testMcpServer, type McpServerInfo } from "../../lib/commands";

export function McpTab() {
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
      setResult((r) => ({
        ...r,
        [name]: `${tools.length} tool${tools.length === 1 ? "" : "s"}: ${tools.join(", ") || "(none)"}`,
      }));
    } catch (e) {
      setResult((r) => ({ ...r, [name]: `failed: ${String(e)}` }));
    } finally {
      setTesting(null);
    }
  }

  return (
    <div className="tab-body">
      <p className="form-note">
        Stdio MCP servers from config. Test spawns the server and calls tools/list.
      </p>
      {servers.length === 0 && (
        <p className="none">
          No MCP servers configured — add an <code>[mcp.servers.&lt;name&gt;]</code> table to your
          config file.
        </p>
      )}
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
