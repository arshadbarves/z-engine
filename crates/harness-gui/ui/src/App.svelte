<script lang="ts">
  import { onMount, tick } from "svelte";
  import { messages, busy, initEvents, submitLocal } from "./lib/events";
  import { submit, abort } from "./lib/commands";

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
    <div class="side-note">desktop v0.1</div>
  </aside>

  <section class="chat">
    <div class="transcript" bind:this={transcriptEl}>
      {#each $messages as m (m.id)}
        <div class={`msg ${m.kind}${m.streaming ? " streaming" : ""}`}>
          {#if m.kind === "assistant"}
            {@html m.text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/\n/g, "<br/>")}
          {:else}
            {m.text}
          {/if}
        </div>
      {/each}
    </div>

    <div class="composer">
      <textarea
        rows="2"
        placeholder={($busy ? "working… (Stop to abort)" : "type a task — Shift+Enter for newline")}
        bind:value={input}
        onkeydown={onKeydown}
        disabled={$busy && false}
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
  .send { background: #00b7c7; color: #082226; }
  .send:disabled { opacity: 0.4; cursor: default; }
  .stop { background: #553333; color: #ffb3b3; }
</style>
