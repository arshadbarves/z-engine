<script lang="ts">
  import {
    listMcpServers,
    removeMcpServer,
    saveMcpServer,
    testMcpServer,
    type McpServerInfo,
  } from "$lib/commands";

  function splitArgs(raw: string): string[] {
    return raw.trim().split(/\s+/).filter(Boolean);
  }

  let servers = $state<McpServerInfo[]>([]);
  let testing = $state<string | null>(null);
  let result = $state<Record<string, string>>({});
  let name = $state("");
  let command = $state("");
  let args = $state("");

  async function refresh() {
    try {
      servers = await listMcpServers();
    } catch {
      servers = [];
    }
  }

  $effect(() => {
    let active = true;
    listMcpServers()
      .then((res) => {
        if (active) servers = res;
      })
      .catch(() => {
        if (active) servers = [];
      });
    return () => {
      active = false;
    };
  });

  async function add() {
    const n = name.trim();
    const cmd = command.trim();
    if (!n || !cmd) return;
    await saveMcpServer(n, cmd, splitArgs(args));
    name = "";
    command = "";
    args = "";
    await refresh();
  }

  async function remove(n: string) {
    await removeMcpServer(n);
    const next = { ...result };
    delete next[n];
    result = next;
    await refresh();
  }

  async function test(n: string) {
    testing = n;
    try {
      const tools = await testMcpServer(n);
      result = {
        ...result,
        [n]: `${tools.length} tool${tools.length === 1 ? "" : "s"}: ${tools.join(", ") || "(none)"}`,
      };
    } catch (e) {
      result = { ...result, [n]: `failed: ${String(e)}` };
    } finally {
      testing = null;
    }
  }
</script>

<div class="tab-body">
  <section class="settings-group">
    <h3>Servers</h3>
    <p class="form-note">
      Stdio MCP servers for this project. Test spawns the server and lists tools. New chats pick them up.
    </p>
    {#if servers.length === 0}
      <p class="none">No MCP servers yet.</p>
    {/if}
    {#each servers as s}
      <div class="mcp-row">
        <div class="mcp-head">
          <strong>{s.name}</strong>
          <code class="mcp-cmd">{[s.command, ...s.args].join(" ")}</code>
          <button disabled={testing === s.name} onclick={() => void test(s.name)} type="button">
            {testing === s.name ? "Testing…" : "Test"}
          </button>
          <button class="mini" title={`Remove ${s.name}`} onclick={() => void remove(s.name)} type="button">
            ✕
          </button>
        </div>
        {#if result[s.name]}
          <code class="mcp-result">{result[s.name]}</code>
        {/if}
      </div>
    {/each}
    <form
      class="mcp-add"
      onsubmit={(e) => {
        e.preventDefault();
        void add();
      }}
    >
      <div class="settings-card">
        <label class="form-row">
          <span class="form-label-title">Name</span>
          <span class="form-label-desc">Short id used in config, e.g. filesystem</span>
          <input bind:value={name} spellcheck={false} placeholder="filesystem" />
        </label>
        <label class="form-row">
          <span class="form-label-title">Command</span>
          <span class="form-label-desc">Executable that speaks MCP over stdio</span>
          <input bind:value={command} spellcheck={false} placeholder="npx" />
        </label>
        <label class="form-row">
          <span class="form-label-title">Args</span>
          <span class="form-label-desc">Space-separated arguments</span>
          <input
            bind:value={args}
            spellcheck={false}
            placeholder="-y @modelcontextprotocol/server-filesystem ."
          />
        </label>
      </div>
      <div class="tab-actions">
        <button class="primary" type="submit" disabled={!name.trim() || !command.trim()}>
          Add server
        </button>
      </div>
    </form>
  </section>
</div>
