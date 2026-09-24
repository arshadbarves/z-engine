<script lang="ts">
  import { untrack } from "svelte";
  import { activeProjectRoot } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import type { SettingsTab } from "$lib/stores/ui.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { updateStore } from "$lib/updateStore";
  import Icon, { ChevronLeft } from "$lib/ui/icons";
  import WindowControlsMaybe from "../chrome/WindowControlsMaybe.svelte";
  import AboutTab from "./AboutTab.svelte";
  import AdvancedTab from "./AdvancedTab.svelte";
  import AppearanceTab from "./AppearanceTab.svelte";
  import ExtensionsTab from "./ExtensionsTab.svelte";
  import HooksTab from "./HooksTab.svelte";
  import McpTab from "./McpTab.svelte";
  import MemoryTab from "./MemoryTab.svelte";
  import ModelsTab from "./ModelsTab.svelte";
  import PermissionsTab from "./PermissionsTab.svelte";
  import ProvidersTab from "./ProvidersTab.svelte";
  import ScopeBar from "./ScopeBar.svelte";
  import SettingsNav, { SETTINGS_TABS } from "./SettingsNav.svelte";
  import SettingsNotices from "./SettingsNotices.svelte";
  import VerificationTab from "./VerificationTab.svelte";
  import "../../settings.css";

  type Props = { isClosing?: boolean; initialTab?: SettingsTab; onClose: () => void };

  let { isClosing = false, initialTab = "providers", onClose }: Props = $props();
  let tab = $state<SettingsTab>(untrack(() => initialTab));
  const update = bindStore(updateStore);
  const active = $derived(SETTINGS_TABS.find((t) => t.id === tab) ?? SETTINGS_TABS[0]);
  const settings = $derived(settingsStore.settings);

  $effect(() => {
    const root = activeProjectRoot();
    untrack(() => void settingsStore.open(root));
  });

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape" && !e.defaultPrevented) {
        e.preventDefault();
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class={`settings-overlay${isClosing ? " is-closing" : ""}`} role="presentation">
  <div class={`settings-page${isClosing ? " is-closing" : ""}`} role="dialog" tabindex="-1" aria-label="Settings">
    <header class="app-titlebar settings-topbar" data-tauri-drag-region>
      <div class="titlebar-side" data-tauri-drag-region>
        <button type="button" class="icon-btn settings-back-btn" title="Back (Esc)" onclick={onClose} aria-label="Back">
          <Icon icon={ChevronLeft} size={15} strokeWidth={1.8} />
        </button>
        <div class="settings-breadcrumb">
          <span class="settings-breadcrumb-root">Settings</span>
          <span class="settings-breadcrumb-sep">/</span>
          <span class="settings-breadcrumb-leaf">{active.label}</span>
        </div>
      </div>
      <div class="titlebar-side" data-tauri-drag-region>
        <WindowControlsMaybe />
      </div>
    </header>

    <div class="app-body settings-body">
      <SettingsNav
        {tab}
        version={settingsStore.info?.version ?? null}
        updateAvailable={Boolean(update.current.info?.available)}
        onSelect={(next) => (tab = next)}
      />

      <section class="canvas-pane settings-canvas-pane">
        <div class="settings-pane-head">
          <div class="head-left">
            <div class={`settings-head-badge ${active.tone}`}>
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
            {#if tab !== "about"}<SettingsNotices />{/if}
            {#if tab === "about"}
              <AboutTab />
            {:else if !settings}
              <div class="settings-loading">
                {settingsStore.loadError ? "Settings are unavailable." : "Loading preferences…"}
              </div>
            {:else}
              {#if active.scoped}<ScopeBar />{/if}
              {#if tab === "models"}
                <ModelsTab {settings} />
              {:else if tab === "providers"}
                <ProvidersTab {settings} />
              {:else if tab === "permissions"}
                <PermissionsTab {settings} />
              {:else if tab === "hooks"}
                <HooksTab />
              {:else if tab === "extensions"}
                <ExtensionsTab />
              {:else if tab === "mcp"}
                <McpTab />
              {:else if tab === "verification"}
                <VerificationTab {settings} />
              {:else if tab === "memory"}
                <MemoryTab />
              {:else if tab === "advanced"}
                <AdvancedTab {settings} />
              {:else}
                <AppearanceTab {settings} />
              {/if}
            {/if}
          </div>
        </div>
      </section>
    </div>
  </div>
</div>
