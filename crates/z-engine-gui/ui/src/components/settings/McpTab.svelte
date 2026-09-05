<script lang="ts">
  import {
    listMcpServers,
    removeMcpServer,
    saveMcpServer,
    testMcpServer,
    type McpServerInfo,
  } from "$lib/commands";
  import Icon, {
    AlertTriangle,
    CheckCircle2,
    LoaderCircle,
    Plus,
    RefreshCw,
    Server,
    Trash2,
  } from "$lib/ui/icons";

  function splitArgs(raw: string): string[] {
    return raw.trim().split(/\s+/).filter(Boolean);
  }

  let servers = $state<McpServerInfo[]>([]);
  let testing = $state<string | null>(null);
  let result = $state<Record<string, { ok: boolean; message: string }>>({});
  let name = $state("");
  let command = $state("");
  let args = $state("");
  let saving = $state(false);

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
    saving = true;
    try {
      await saveMcpServer(n, cmd, splitArgs(args));
      name = "";
      command = "";
      args = "";
      await refresh();
    } finally {
      saving = false;
    }
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
        [n]: {
          ok: true,
          message: `${tools.length} tool${tools.length === 1 ? "" : "s"} detected: ${tools.join(", ") || "(none)"}`,
        },
      };
    } catch (e) {
      result = {
        ...result,
        [n]: {
          ok: false,
          message: String(e),
        },
      };
    } finally {
      testing = null;
    }
  }
</script>

<div class="tab-body mcp-tab">
  <!-- MCP Header Overview -->
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>Tool Integrations (MCP)</h3>
      <span class="settings-group-sub">
        Connect external servers to provide the assistant with custom tools, databases, and APIs
      </span>
    </div>

    <div class="settings-card permission-status-card">
      <div class="permission-status-icon mcp-brand-icon">
        <Icon icon={Server} size={20} />
      </div>
      <div class="permission-status-copy">
        <span class="permission-status-title">Model Context Protocol Enabled</span>
        <p class="permission-status-desc">
          Z Engine speaks stdio MCP to communicate with local servers. New chat sessions
          automatically load the tools provided by these integrations.
        </p>
      </div>
    </div>
  </section>

  <!-- Configured Servers List -->
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>Configured Integrations ({servers.length})</h3>
      <span class="settings-group-sub">
        Active tool servers available across your workspaces
      </span>
    </div>

    <div class="settings-card">
      {#if servers.length === 0}
        <div class="permission-empty-card">
          <Icon icon={Server} size={22} class="permission-empty-icon" />
          <div class="permission-empty-text">
            <strong>No MCP servers connected</strong>
            <p>Add a server integration below using npx, uvx, or a local executable.</p>
          </div>
        </div>
      {:else}
        <div class="mcp-servers-list">
          {#each servers as s}
            {@const res = result[s.name]}
            <div class="mcp-server-item">
              <div class="mcp-server-main">
                <div class="mcp-server-info">
                  <div class="mcp-title-row">
                    <span class="mcp-server-badge">
                      <Icon icon={Server} size={12} />
                    </span>
                    <strong class="mcp-server-name">{s.name}</strong>
                  </div>
                  <code class="mcp-server-cmd">{[s.command, ...s.args].join(" ")}</code>
                </div>

                <div class="mcp-server-actions">
                  <button
                    type="button"
                    class="mcp-test-btn"
                    disabled={testing === s.name}
                    onclick={() => void test(s.name)}
                    title={`Test connection to ${s.name}`}
                  >
                    <Icon
                      icon={testing === s.name ? LoaderCircle : RefreshCw}
                      size={12}
                      class={testing === s.name ? "spin" : undefined}
                    />
                    <span>{testing === s.name ? "Testing…" : "Test"}</span>
                  </button>
                  <button
                    type="button"
                    class="permission-delete-btn"
                    title={`Remove ${s.name}`}
                    onclick={() => void remove(s.name)}
                    aria-label={`Remove server ${s.name}`}
                  >
                    <Icon icon={Trash2} size={13} />
                  </button>
                </div>
              </div>

              {#if res}
                <div class={`mcp-result-banner${res.ok ? " ok" : " err"}`}>
                  <Icon icon={res.ok ? CheckCircle2 : AlertTriangle} size={13} />
                  <span>{res.message}</span>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </section>

  <!-- Add Server Form -->
  <section class="settings-group">
    <div class="settings-group-header">
      <h3>Add New Tool Integration</h3>
      <span class="settings-group-sub">Configure an executable that communicates via stdio MCP</span>
    </div>

    <form
      class="settings-card mcp-form-card"
      onsubmit={(e) => {
        e.preventDefault();
        void add();
      }}
    >
      <label class="form-row">
        <span class="form-label-title">Integration Name</span>
        <span class="form-label-desc">Unique identifier for this tool server (e.g. filesystem, github, postgres)</span>
        <input bind:value={name} spellcheck={false} placeholder="filesystem" />
      </label>

      <label class="form-row">
        <span class="form-label-title">Command</span>
        <span class="form-label-desc">Executable command or binary path (e.g. npx, uvx, or /usr/local/bin/...)</span>
        <input bind:value={command} spellcheck={false} placeholder="npx" />
      </label>

      <label class="form-row">
        <span class="form-label-title">Arguments</span>
        <span class="form-label-desc">Space-separated arguments and flags passed to the command</span>
        <input
          bind:value={args}
          spellcheck={false}
          placeholder="-y @modelcontextprotocol/server-filesystem ."
        />
      </label>

      <div class="mcp-card-footer">
        <button
          class="primary mcp-submit-btn"
          type="submit"
          disabled={!name.trim() || !command.trim() || saving}
        >
          <Icon icon={Plus} size={13} />
          <span>Add Tool Integration</span>
        </button>
      </div>
    </form>
  </section>
</div>
