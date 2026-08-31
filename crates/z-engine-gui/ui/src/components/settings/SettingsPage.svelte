<script lang="ts">
  import { getConfig, type HarnessConfig } from "$lib/commands";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { updateStore } from "$lib/updateStore";
  import Icon, { ChevronLeft, Info, Server, Shield, Sliders } from "$lib/ui/icons";
  import LogoMark from "../chrome/LogoMark.svelte";
  import WindowControlsMaybe from "../chrome/WindowControlsMaybe.svelte";
  import AboutTab from "./AboutTab.svelte";
  import GeneralTab from "./GeneralTab.svelte";
  import McpTab from "./McpTab.svelte";
  import PermissionsTab from "./PermissionsTab.svelte";
  import "../../settings.css";

  type Tab = "general" | "permissions" | "mcp" | "about";
  type Props = { isClosing?: boolean; onClose: () => void };

  let { isClosing = false, onClose }: Props = $props();
  let tab = $state<Tab>("general");
  let cfg = $state<HarnessConfig | null>(null);
  const update = bindStore(updateStore);

  const tabs: Array<{ id: Tab; label: string; hint: string; icon: typeof Sliders }> = [
    { id: "general", label: "General", hint: "Provider & model", icon: Sliders },
    { id: "permissions", label: "Permissions", hint: "Allow rules", icon: Shield },
    { id: "mcp", label: "MCP", hint: "External tools", icon: Server },
    { id: "about", label: "About", hint: "Version & paths", icon: Info },
  ];

  const active = $derived(tabs.find((t) => t.id === tab) ?? tabs[0]);

  $effect(() => {
    void tab;
    getConfig()
      .then((c) => {
        cfg = c;
      })
      .catch(console.error);
  });

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div
  class={`settings-overlay${isClosing ? " is-closing" : ""}`}
  role="presentation"
>
  <div
    class={`settings-page${isClosing ? " is-closing" : ""}`}
    role="dialog"
    tabindex="-1"
    aria-label="Settings"
  >
    <header class="app-topbar settings-topbar" data-tauri-drag-region>
      <div class="topbar-left" data-tauri-drag-region>
        <button
          type="button"
          class="icon-btn settings-back-btn"
          title="Back (Esc)"
          onclick={onClose}
          aria-label="Back"
        >
          <Icon icon={ChevronLeft} size={15} strokeWidth={1.8} />
        </button>
        <div class="settings-breadcrumb">
          <span class="settings-breadcrumb-root">Settings</span>
          <span class="settings-breadcrumb-sep">/</span>
          <span class="settings-breadcrumb-leaf">{active.label}</span>
        </div>
      </div>

      <div class="topbar-center" data-tauri-drag-region>
        <div class="settings-topbar-pill">
          <LogoMark size={14} />
          <span>Z Engine Preferences</span>
        </div>
      </div>

      <div class="topbar-right" data-tauri-drag-region>
        {#if cfg?.version}
          <span class="settings-version-chip">v{cfg.version}</span>
        {/if}
        <WindowControlsMaybe />
      </div>
    </header>

    <div class="app-body settings-body">
      <aside class="sidebar settings-nav-island">
        <div class="sidebar-top-bar">
          <div class="sidebar-brand-pill">
            <LogoMark size={18} />
            <span class="sidebar-brand-text">Preferences</span>
          </div>
        </div>

        <nav class="settings-nav" aria-label="Settings sections">
          {#each tabs as t}
            <button
              type="button"
              class={`settings-nav-btn${tab === t.id ? " active" : ""}`}
              onclick={() => (tab = t.id)}
            >
              <span class="settings-nav-icon">
                <Icon icon={t.icon} size={14} />
              </span>
              <span class="settings-nav-copy">
                <em>{t.label}</em>
                <small>{t.hint}</small>
              </span>
              {#if t.id === "about" && update.current.info?.available}
                <span class="update-dot" role="status" aria-label="Update available"></span>
              {/if}
            </button>
          {/each}
        </nav>

        <div class="settings-rail-foot">
          <span>{cfg?.version ? `v${cfg.version}` : "Z Engine"}</span>
        </div>
      </aside>

      <section class="canvas-pane settings-canvas-pane">
        <div class="chat-head settings-pane-head">
          <div class="head-left">
            <div class="settings-head-badge">
              <Icon icon={active.icon} size={15} />
            </div>
            <div class="settings-head-text">
              <h2>{active.label}</h2>
              <p>{active.hint}</p>
            </div>
          </div>
        </div>

        <div class="settings-content-wrap">
          <div class="settings-content-body">
            {#if !cfg}
              <div class="settings-loading">Loading settings…</div>
            {:else if tab === "general"}
              <GeneralTab {cfg} />
            {:else if tab === "permissions"}
              <PermissionsTab />
            {:else if tab === "mcp"}
              <McpTab />
            {:else}
              <AboutTab {cfg} />
            {/if}
          </div>
        </div>
      </section>
    </div>
  </div>
</div>

