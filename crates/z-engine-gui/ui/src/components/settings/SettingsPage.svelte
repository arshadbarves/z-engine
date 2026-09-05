<script lang="ts">
  import { getConfig, type HarnessConfig } from "$lib/commands";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { updateStore } from "$lib/updateStore";
  import Icon, {
    ChevronLeft,
    Info,
    Search,
    Server,
    Shield,
    Sliders,
    Sparkles,
  } from "$lib/ui/icons";
  import WindowControlsMaybe from "../chrome/WindowControlsMaybe.svelte";
  import AboutTab from "./AboutTab.svelte";
  import GeneralTab from "./GeneralTab.svelte";
  import McpTab from "./McpTab.svelte";
  import PermissionsTab from "./PermissionsTab.svelte";
  import ProvidersTab from "./ProvidersTab.svelte";
  import "../../settings.css";

  type Tab = "providers" | "general" | "permissions" | "mcp" | "about";
  type Props = { isClosing?: boolean; onClose: () => void };

  let { isClosing = false, onClose }: Props = $props();
  let tab = $state<Tab>("providers");
  let search = $state("");
  let cfg = $state<HarnessConfig | null>(null);
  const update = bindStore(updateStore);

  const tabs: Array<{
    id: Tab;
    label: string;
    hint: string;
    color: string;
    icon: typeof Sliders;
  }> = [
    {
      id: "providers",
      label: "Providers",
      hint: "AI models & API keys",
      color: "#6366f1",
      icon: Sparkles,
    },
    {
      id: "general",
      label: "General & Agent",
      hint: "Code review & context limits",
      color: "#00d68f",
      icon: Sliders,
    },
    {
      id: "permissions",
      label: "Permissions",
      hint: "Terminal approvals & safety",
      color: "#38bdf8",
      icon: Shield,
    },
    {
      id: "mcp",
      label: "Integrations",
      hint: "External MCP tool servers",
      color: "#a78bfa",
      icon: Server,
    },
    {
      id: "about",
      label: "About & Updates",
      hint: "Version, updates & storage",
      color: "#f5a623",
      icon: Info,
    },
  ];

  const filteredTabs = $derived(
    search.trim()
      ? tabs.filter(
          (t) =>
            t.label.toLowerCase().includes(search.toLowerCase()) ||
            t.hint.toLowerCase().includes(search.toLowerCase()),
        )
      : tabs,
  );

  const active = $derived(tabs.find((t) => t.id === tab) ?? tabs[0]);

  $effect(() => {
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
    <!-- Topbar (App & Prompt Inspector UX/UI Style: No Duplicate Center Pill) -->
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

      <!-- Clean empty center drag region: Zero Duplicate Topbar Pill! -->
      <div class="topbar-center" data-tauri-drag-region></div>

      <div class="topbar-right" data-tauri-drag-region>
        <WindowControlsMaybe />
      </div>
    </header>

    <!-- App Body: Left Sidebar Island + Right Canvas Pane -->
    <div class="app-body settings-body">
      <!-- Left Navigation Sidebar Island -->
      <aside class="sidebar settings-nav-island" aria-label="Settings navigation">
        <div class="prefs-search-wrap">
          <Icon icon={Search} size={13} class="prefs-search-icon" />
          <input
            type="text"
            bind:value={search}
            placeholder="Search settings…"
            spellcheck={false}
          />
          {#if search}
            <button
              type="button"
              class="prefs-search-clear"
              onclick={() => (search = "")}
              aria-label="Clear search"
            >
              ✕
            </button>
          {/if}
        </div>

        <nav class="settings-nav">
          {#each filteredTabs as t}
            <button
              type="button"
              class={`settings-nav-btn${tab === t.id ? " active" : ""}`}
              onclick={() => (tab = t.id)}
            >
              <span class="settings-nav-icon" style={`color: ${t.color}`}>
                <Icon icon={t.icon} size={15} />
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

      <!-- Right Canvas Island -->
      <section class="canvas-pane settings-canvas-pane">
        <div class="settings-pane-head">
          <div class="head-left">
            <div class="settings-head-badge" style={`color: ${active.color}`}>
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
              <div class="settings-loading">Loading preferences…</div>
            {:else if tab === "providers"}
              <ProvidersTab {cfg} />
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
