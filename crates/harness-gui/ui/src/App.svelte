<script lang="ts">
  import { onMount, tick } from "svelte";
  import { messages, busy, initEvents, submitLocal } from "./lib/events";
import { submit, abort, set_mode, invoke } from "./lib/commands";
import { writable } from "svelte/store";

interface SessionEntry {
  path: string;
  ulid: string;
  firstUserMsg: string | null;
  modifiedMs: number;
}
const sessions = writable<SessionEntry[]>([]);
const rules = writable<string[]>([]);
let showSettings = false;
let newRule = "";

async function refreshSessions() {
  try {
    const list = (await invoke("list_sessions")) as SessionEntry[];
    list.sort((a, b) => Number(b.modifiedMs) - Number(a.modifiedMs));
    sessions.set(list);
  } catch (e) {
    console.error(e);
  }
}

async function openSession(path: string) {
  messages.set([]);
  await invoke("start_session", { resumePath: path });
  await refreshSessions();
}

async function newTask() {
  messages.set([]);
  await invoke("start_session", { resumePath: null });
  await refreshSessions();
}

async function delSession(path: string) {
  if (!confirm("Delete this session transcript?")) return;
  await invoke("delete_session", { path });
  await refreshSessions();
}

async function refreshRules() {
  try {
    rules.set((await invoke("list_permission_rules")) as string[]);
  } catch (e) {
    console.error(e);
  }
}

async function addRule() {
  const r = newRule.trim();
  if (!r) return;
  await invoke("save_permission_rule", { rule: r });
  newRule = "";
  await refreshRules();
}

async function delRule(r: string) {
  await invoke("remove_permission_rule", { rule: r });
  await refreshRules();
}

async function toggleSettings() {
  showSettings = !showSettings;
  if (showSettings) await refreshRules();
}

function relTime(ms: number): string {
  const d = Date.now() - ms;
  if (d < 60_000) return "now";
  if (d < 3_600_000) return `${Math.floor(d / 60_000)}m`;
  if (d < 86_400_000) return `${Math.floor(d / 3_600_000)}h`;
  return `${Math.floor(d / 86_400_000)}d`;
}

  let input = "";
  let transcriptEl: HTMLElement;

  async function send() {
    const text = input.trim();
    if (!text || $busy) return;
    input = "";
    submitLocal(text);
    busy.set(true);
    try {
      await submit(text);
    } catch (e) {
      console.error(e);
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void send();
    }
  }

  onMount(async () => {
    await initEvents();
    // autoscroll whenever messages change
    messages.subscribe(async () => {
      await tick();
      transcriptEl?.scrollTo({ top: transcriptEl.scrollHeight });
    });
  });
</script>

<main class="app">
  <aside class="sidebar">
    <div class="brand">harness</div>
    <button class="newtask" onclick={() => void newTask()}>＋ New task</button>
    <div class="sess-head">
      <span>sessions</span>
      <button class="mini" title="refresh" onclick={() => void refreshSessions()}>↻</button>
    </div>
    <div style="flex:1"></div>
    <button class="gear" onclick={() => void toggleSettings()}>⚙ Settings</button>
    <div class="sessions">
      {#each $sessions as s (s.path)}
        <div
          class="session"
          role="button"
          tabindex="0"
          onclick={() => void openSession(s.path)}
          onkeydown={(e) => e.key === "Enter" && void openSession(s.path)}
        >
          <div class="sess-preview">{s.firstUserMsg ?? "(empty)"}</div>
          <div class="sess-meta">
            <span>{s.ulid.slice(0, 6)}</span>
            <span>{relTime(Number(s.modifiedMs))}</span>
            <button
              class="del"
              title="delete"
              onclick={(e) => {
                e.stopPropagation();
                void delSession(s.path);
              }}>✕</button
            >
          </div>
        </div>
      {:else}
        <div class="sess-empty">no sessions yet</div>
      {/each}
    </div>
    <div class="spacer"></div>
    <div class="side-note">TUI remains available for<br />keyboard-only use</div>
  </aside>

  <section class="chat">
    <div class="transcript" bind:this={transcriptEl}>
      {#each $messages as m (m.id)}
        <div class={`msg ${m.kind}${m.streaming ? " streaming" : ""}`}>
          {#if m.kind === "assistant"}
            {@html m.text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/\n/g, "<br/>")}
          {:else if m.kind === "approval"}
            <div class="approval-body">{m.text}</div>
            <div class="approval-actions">
              <button class="ok" onclick={() => invoke("approve_with_rule", m.bashCommand ? {
                  id: m.approvalId, decision: "session",
                  rule: (m.bashCommand ?? "").split(/\s+/).slice(0, 2).join(" ") + "*",
                } : { id: m.approvalId, decision: "session", rule: "bash*" })}>
                2 · Always (session)
              </button>
              {#if m.canPersist}
                <button class="ok" onclick={() => invoke("approve_with_rule", {
                    id: m.approvalId, decision: "persist",
                    rule: m.suggestedRule ?? "bash*",
                  })}>
                  3 · Persist
                </button>
              {/if}
              <button class="deny" onclick={() => invoke("deny", { id: m.approvalId })}>
                4 · Deny
              </button>
              <span class="hint">1/y = once only</span>
            </div>
          {:else}
            {m.text}
          {/if}
        </div>
      {/each}
    </div>

    <div class="composer">
      <select
        class="mode"
        title="permission mode"
        onchange={(e) => set_mode((e.currentTarget as HTMLSelectElement).value)}
      >
        <option value="normal">normal</option>
        <option value="accept-edits">auto-accept edits</option>
        <option value="plan">plan</option>
      </select>
      <textarea
        rows="2"
        placeholder={($busy ? "working… (Stop to abort)" : "type a task — Shift+Enter for newline")}
        bind:value={input}
        onkeydown={onKeydown}
      />
      {#if $busy}
        <button class="stop" onclick={() => abort()}>■ Stop</button>
      {:else}
        <button class="send" onclick={() => void send()} disabled={!input.trim()}>Send ⏎</button>
      {/if}
    </div>
  </section>
</main>

<style>
  :global(html, body) { height: 100%; margin: 0; }
  :global(#app) { height: 100%; }
  .app {
    display: grid;
    grid-template-columns: 220px 1fr;
    height: 100%;
    font-family: ui-sans-serif, -apple-system, "Segoe UI", sans-serif;
    background: #14161a;
    color: #e8eaed;
  }
  .sidebar {
    background: #0f1114;
    border-right: 1px solid #23262b;
    padding: 14px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .brand {
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #00b7c7;
    margin-bottom: 8px;
  }
  .side-note { color: #666; font-size: 12px; }
  .newtask {
    background: #10262a;
    color: #7fd7e1;
    border: 1px solid #14505a;
    border-radius: 8px;
    padding: 8px;
    cursor: pointer;
    font-weight: 600;
  }
  .sess-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 10px;
    color: #666;
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.08em;
  }
  .mini { background: none; border: none; color: #7fd7e1; cursor: pointer; }
  .sessions { overflow-y: auto; max-height: 55vh; display: flex; flex-direction: column; gap: 4px; }
  .session {
    padding: 7px 8px;
    border-radius: 8px;
    cursor: pointer;
    border: 1px solid transparent;
  }
  .session:hover, .session:focus { background: #191d22; border-color: #2a2e35; outline: none; }
  .sess-preview {
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sess-meta {
    display: flex;
    gap: 8px;
    color: #666;
    font-size: 11px;
    align-items: center;
  }
  .del {
    margin-left: auto;
    background: none;
    border: none;
    color: #a55;
    cursor: pointer;
  }
  .del:hover { color: #ff7070; }
  .sess-empty { color: #555; font-size: 12px; }
  .spacer { flex: 1; }
  .chat { display: flex; flex-direction: column; min-width: 0; height: 100%; }
  .transcript {
    flex: 1;
    overflow-y: auto;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .msg { white-space: pre-wrap; word-break: break-word; line-height: 1.5; }
  .msg.user { color: #7fd7e1; border-left: 3px solid #00b7c7; padding-left: 8px; }
  .msg.assistant { color: #e8eaed; }
  .msg.thinking { color: #888; font-style: italic; }
  .msg.tool { color: #e5c07f; font-size: 13px; }
  .msg.notice { color: #9aa0a6; font-size: 13px; }
  .msg.approval {
    color: #ffd479;
    border: 1px solid #6a5b23;
    border-radius: 8px;
    padding: 8px 10px;
    background: #1d1a10;
  }
  .msg.error { color: #ff7070; }
  .streaming::after { content: "▌"; opacity: 0.6; }
  .composer {
    display: flex;
    gap: 8px;
    padding: 10px 12px;
    border-top: 1px solid #23262b;
    background: #101216;
  }
  textarea {
    flex: 1;
    resize: none;
    background: #171a1f;
    color: inherit;
    border: 1px solid #2a2e35;
    border-radius: 8px;
    padding: 8px 10px;
    font: inherit;
  }
  button {
    border: none;
    border-radius: 8px;
    padding: 0 16px;
    font-weight: 600;
    cursor: pointer;
  }
  .mode {
    background: #171a1f;
    color: #e8eaed;
    border: 1px solid #2a2e35;
    border-radius: 8px;
    padding: 6px;
    align-self: end;
  }
  .send { background: #00b7c7; color: #082226; }
  .send:disabled { opacity: 0.4; cursor: default; }
  .stop { background: #553333; color: #ffb3b3; }
</style>
