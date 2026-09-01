<script lang="ts">
  import { parseChangelog, type ChangelogRelease } from "$lib/domain/changelog";
  import Icon, {
    ChevronDown,
    ChevronRight,
    FileText,
    Search,
    Sparkles,
    X,
  } from "$lib/ui/icons";

  type Props = {
    markdown: string;
    currentVersion?: string;
  };

  let { markdown, currentVersion = "" }: Props = $props();

  let searchQuery = $state("");
  let collapsedState = $state<Record<string, boolean>>({});

  const releases = $derived(parseChangelog(markdown));

  function toggleRelease(version: string) {
    collapsedState = {
      ...collapsedState,
      [version]: !isExpanded(version),
    };
  }

  function isExpanded(version: string): boolean {
    if (searchQuery.trim()) return true;
    if (collapsedState[version] !== undefined) {
      return !collapsedState[version];
    }
    // Expand the latest release by default
    return releases.length > 0 && releases[0]?.version === version;
  }

  const filteredReleases = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return releases;
    return releases
      .map((rel) => {
        const matchesVersion = rel.version.toLowerCase().includes(q);
        const matchedSections = rel.sections
          .map((sec) => ({
            ...sec,
            items: sec.items.filter(
              (item) =>
                item.toLowerCase().includes(q) ||
                sec.title.toLowerCase().includes(q) ||
                matchesVersion,
            ),
          }))
          .filter((sec) => sec.items.length > 0);

        if (matchesVersion || matchedSections.length > 0) {
          return {
            ...rel,
            sections: matchedSections.length > 0 ? matchedSections : rel.sections,
          };
        }
        return null;
      })
      .filter((r): r is ChangelogRelease => r !== null);
  });

  function formatItem(text: string): { bold: string; body: string } {
    const boldMatch = text.match(/^\*\*([^*]+)\*\*:\s*(.+)$/);
    if (boldMatch) {
      return { bold: boldMatch[1] ?? "", body: boldMatch[2] ?? "" };
    }
    return { bold: "", body: text };
  }
</script>

<div class="changelog-viewer">
  <div class="changelog-search-bar">
    <Icon icon={Search} size={12} class="changelog-search-icon" />
    <input
      type="text"
      bind:value={searchQuery}
      placeholder="Search updates and release notes…"
      spellcheck={false}
      class="changelog-search-input"
    />
    {#if searchQuery}
      <button
        type="button"
        class="changelog-clear-btn"
        onclick={() => (searchQuery = "")}
        title="Clear search"
      >
        <Icon icon={X} size={10} />
      </button>
    {/if}
  </div>

  <div class="changelog-timeline">
    {#if filteredReleases.length === 0}
      <div class="changelog-empty">
        <Icon icon={FileText} size={20} class="empty-tag-icon" />
        <span>No matching release notes found</span>
      </div>
    {/if}

    {#each filteredReleases as rel (rel.version)}
      {@const open = isExpanded(rel.version)}
      {@const isCurrent = currentVersion && (currentVersion === rel.version || currentVersion === `v${rel.version}`)}
      <div class={`changelog-release-card${open ? " is-open" : ""}${rel.isLatest ? " is-latest" : ""}`}>
        <button
          type="button"
          class="changelog-release-header"
          onclick={() => toggleRelease(rel.version)}
        >
          <div class="release-header-left">
            <Icon
              icon={open ? ChevronDown : ChevronRight}
              size={12}
              class="release-chevron"
            />
            <div class="release-version-wrap">
              <span class="release-version">v{rel.version}</span>
              {#if rel.isLatest}
                <span class="release-pill latest">
                  <Icon icon={Sparkles} size={9} />
                  <span>Latest</span>
                </span>
              {/if}
              {#if isCurrent}
                <span class="release-pill current">Installed</span>
              {/if}
            </div>
          </div>
          {#if rel.date}
            <span class="release-date">{rel.date}</span>
          {/if}
        </button>

        {#if open}
          <div class="changelog-release-body">
            {#each rel.sections as sec}
              <div class="changelog-section">
                <div class="section-badge-wrap">
                  <span class={`section-tag ${sec.kind}`}>{sec.title}</span>
                </div>
                <ul class="section-list">
                  {#each sec.items as item}
                    {@const formatted = formatItem(item)}
                    <li class="section-item">
                      {#if formatted.bold}
                        <strong class="item-lead">{formatted.bold}:</strong>
                        <span class="item-body">{formatted.body}</span>
                      {:else}
                        <span class="item-body">{formatted.body}</span>
                      {/if}
                    </li>
                  {/each}
                </ul>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  </div>
</div>
