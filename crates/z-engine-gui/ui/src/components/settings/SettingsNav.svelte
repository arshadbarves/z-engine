<script lang="ts" module>
  import type { SettingsTab } from "$lib/stores/ui.svelte";
  import {
    Book,
    Bot,
    Brain,
    Eye,
    Info,
    ListChecks,
    Server,
    Shield,
    Sliders,
    Sparkles,
    Workflow,
    type IconSvgElement,
  } from "$lib/ui/icons";

  export interface TabMeta {
    id: SettingsTab;
    label: string;
    hint: string;
    tone: string;
    icon: IconSvgElement;
    /** Edits layered settings, so the page shows which file it writes. */
    scoped: boolean;
  }

  export const SETTINGS_TABS: readonly TabMeta[] = [
    { id: "models", label: "Models", hint: "Main, fast & review models", tone: "settings-tone-working", icon: Brain, scoped: true },
    { id: "providers", label: "Providers", hint: "Model APIs & keys", tone: "settings-tone-shell", icon: Sparkles, scoped: true },
    { id: "permissions", label: "Permissions", hint: "Mode, rules & folders", tone: "settings-tone-shell", icon: Shield, scoped: true },
    { id: "hooks", label: "Hooks", hint: "Commands on agent events", tone: "settings-tone-attention", icon: Workflow, scoped: true },
    { id: "extensions", label: "Agents & Commands", hint: "Agents, commands, skills, rules", tone: "settings-tone-working", icon: Bot, scoped: false },
    { id: "mcp", label: "MCP", hint: "External tool servers", tone: "settings-tone-attention", icon: Server, scoped: true },
    { id: "verification", label: "Verification", hint: "Checks & continuations", tone: "settings-tone-working", icon: ListChecks, scoped: true },
    { id: "memory", label: "Memory", hint: "AGENTS.md instructions", tone: "settings-tone-accent", icon: Book, scoped: false },
    { id: "advanced", label: "Advanced", hint: "Context, limits, web, shell, LSP", tone: "settings-tone-accent", icon: Sliders, scoped: true },
    { id: "appearance", label: "Appearance", hint: "Report detail & response style", tone: "settings-tone-accent", icon: Eye, scoped: true },
    { id: "about", label: "About & Updates", hint: "Version, updates & files", tone: "settings-tone-attention", icon: Info, scoped: false },
  ];
</script>

<script lang="ts">
  import Icon, { Search } from "$lib/ui/icons";

  type Props = { tab: SettingsTab; version: string | null; updateAvailable: boolean; onSelect: (tab: SettingsTab) => void };
  let { tab, version, updateAvailable, onSelect }: Props = $props();

  let search = $state("");
  const filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    if (!q) return SETTINGS_TABS;
    return SETTINGS_TABS.filter((t) => t.label.toLowerCase().includes(q) || t.hint.toLowerCase().includes(q));
  });
</script>

<aside class="sidebar settings-nav-island" aria-label="Settings navigation">
  <div class="prefs-search-wrap">
    <Icon icon={Search} size={13} class="prefs-search-icon" />
    <input type="text" bind:value={search} placeholder="Search settings…" spellcheck={false} />
    {#if search}
      <button type="button" class="prefs-search-clear" onclick={() => (search = "")} aria-label="Clear search">✕</button>
    {/if}
  </div>

  <nav class="settings-nav">
    {#each filtered as t (t.id)}
      <button
        type="button"
        class={`settings-nav-btn${tab === t.id ? " active" : ""}`}
        aria-current={tab === t.id ? "page" : undefined}
        onclick={() => onSelect(t.id)}
      >
        <span class={`settings-nav-icon ${t.tone}`}><Icon icon={t.icon} size={15} /></span>
        <span class="settings-nav-copy">
          <em>{t.label}</em>
          <small>{t.hint}</small>
        </span>
        {#if t.id === "about" && updateAvailable}
          <span class="update-dot" role="status" aria-label="Update available"></span>
        {/if}
      </button>
    {/each}
  </nav>

  <div class="settings-rail-foot">
    <span>{version ? `v${version}` : "Z Engine"}</span>
  </div>
</aside>
