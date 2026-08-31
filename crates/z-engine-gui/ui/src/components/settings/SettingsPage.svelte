<script lang="ts">
  import { getConfig, type HarnessConfig } from "$lib/commands";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { updateStore } from "$lib/updateStore";
  import Icon, { Info, Server, Shield, Sliders, X } from "$lib/ui/icons";
  import LogoMark from "../chrome/LogoMark.svelte";
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
      if (e.key === "Escape") onClose();
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
    <aside class="settings-rail">
      <div class="settings-brand">
        <LogoMark size={22} />
        <div>
          <strong>Settings</strong>
          <span>Z Engine</span>
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

    <section class="settings-main">
      <header class="settings-main-head">
        <div>
          <h2>{active.label}</h2>
          <p>{active.hint}</p>
        </div>
        <button type="button" class="settings-close" onclick={onClose} aria-label="Close settings">
          <Icon icon={X} size={14} />
          <kbd>Esc</kbd>
        </button>
      </header>
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
