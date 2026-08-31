<script lang="ts">
  import type { SlashCommand } from "$lib/slash";
  import Icon, {
    Eye,
    FileCode,
    FileText,
    Folder,
    FolderGit2,
    Info,
    LoaderCircle,
    Minimize2,
    Search,
    Sliders,
    Sparkles,
    Target,
    Wrench,
    Zap,
  } from "$lib/ui/icons";

  type Props = {
    showSlash: boolean;
    slashMatches: SlashCommand[] | null;
    slashSel: number;
    onSelectSlash: (name: string) => void;
    onHoverSlash: (index: number) => void;
    showFiles: boolean;
    files: string[] | null;
    fileSel: number;
    onSelectFile: (path: string) => void;
    onHoverFile: (index: number) => void;
  };

  let {
    showSlash,
    slashMatches,
    slashSel,
    onSelectSlash,
    onHoverSlash,
    showFiles,
    files,
    fileSel,
    onSelectFile,
    onHoverFile,
  }: Props = $props();

  let slashListEl: HTMLDivElement | undefined = $state();
  let fileListEl: HTMLDivElement | undefined = $state();

  $effect(() => {
    void slashSel;
    if (showSlash && slashListEl) {
      const activeEl = slashListEl.querySelector(".cmd-pop-item.sel");
      if (activeEl) {
        activeEl.scrollIntoView({ block: "nearest", behavior: "smooth" });
      }
    }
  });

  $effect(() => {
    void fileSel;
    if (showFiles && fileListEl) {
      const activeEl = fileListEl.querySelector(".file-pop-item.sel");
      if (activeEl) {
        activeEl.scrollIntoView({ block: "nearest", behavior: "smooth" });
      }
    }
  });

  function getExt(filename: string) {
    const i = filename.lastIndexOf(".");
    return i !== -1 ? filename.slice(i + 1).toLowerCase() : "";
  }

  function splitPath(fullPath: string) {
    const clean = fullPath.replace(/^\/+/, "");
    const idx = clean.lastIndexOf("/");
    if (idx === -1) {
      return { dir: "", name: clean, ext: getExt(clean) };
    }
    return {
      dir: clean.slice(0, idx + 1),
      name: clean.slice(idx + 1),
      ext: getExt(clean.slice(idx + 1)),
    };
  }

  function getFileTypeInfo(ext: string) {
    switch (ext) {
      case "ts":
      case "tsx":
        return { icon: FileCode, label: "TypeScript", colorClass: "ext-ts" };
      case "js":
      case "jsx":
        return { icon: FileCode, label: "JavaScript", colorClass: "ext-js" };
      case "svelte":
        return { icon: Sparkles, label: "Svelte", colorClass: "ext-svelte" };
      case "rs":
        return { icon: Wrench, label: "Rust", colorClass: "ext-rs" };
      case "json":
      case "toml":
      case "yaml":
      case "yml":
        return { icon: Sliders, label: "Config", colorClass: "ext-cfg" };
      case "md":
      case "txt":
        return { icon: FileText, label: "Docs", colorClass: "ext-doc" };
      case "css":
        return { icon: FileText, label: "Style", colorClass: "ext-css" };
      case "png":
      case "jpg":
      case "svg":
        return { icon: Eye, label: "Asset", colorClass: "ext-img" };
      default:
        return { icon: FileCode, label: ext.toUpperCase() || "File", colorClass: "ext-default" };
    }
  }

  function getCommandInfo(name: string) {
    switch (name) {
      case "goal":
        return { icon: Target, category: "GOAL", colorClass: "cmd-goal" };
      case "compact":
        return { icon: Minimize2, category: "CONTEXT", colorClass: "cmd-compact" };
      case "notes":
        return { icon: FileText, category: "CONTEXT", colorClass: "cmd-notes" };
      case "cost":
        return { icon: Zap, category: "USAGE", colorClass: "cmd-cost" };
      case "status":
        return { icon: Sliders, category: "SYSTEM", colorClass: "cmd-status" };
      case "help":
        return { icon: Info, category: "HELP", colorClass: "cmd-help" };
      case "browser":
        return { icon: Eye, category: "TOOL", colorClass: "cmd-tool" };
      default:
        return { icon: Sparkles, category: "CUSTOM", colorClass: "cmd-custom" };
    }
  }
</script>

{#if showSlash || showFiles}
  {#if showSlash && slashMatches}
    <div class="composer-pop command-pop" role="listbox" aria-label="Slash commands">
      <div class="cmd-pop-header">
        <div class="cmd-pop-header-title">
          <Icon icon={Sparkles} size={13} class="cmd-glow-icon" />
          <span>Slash Commands</span>
        </div>
        <span class="cmd-count-pill">{slashMatches.length}</span>
      </div>

      <div class="cmd-pop-list" bind:this={slashListEl}>
        {#if slashMatches.length === 0}
          <div class="pop-empty">
            <Icon icon={Search} size={14} />
            <span>No matching commands</span>
          </div>
        {/if}
        {#each slashMatches as c, i}
          {@const { icon, category, colorClass } = getCommandInfo(c.name)}
          {@const isSel = i === slashSel}
          <button
            role="option"
            aria-selected={isSel}
            class={`cmd-pop-item ${colorClass}${isSel ? " sel" : ""}`}
            onmouseenter={() => onHoverSlash(i)}
            onclick={() => onSelectSlash(c.name)}
          >
            <div class="cmd-icon-box">
              <Icon {icon} size={13} strokeWidth={1.8} />
            </div>
            <div class="cmd-info-col">
              <div class="cmd-title-row">
                <span class="cmd-name">/{c.name}</span>
                <span class="cmd-cat-tag">{c.custom ? "CUSTOM" : category}</span>
              </div>
              <span class="cmd-desc">{c.desc}</span>
            </div>
            {#if isSel}
              <div class="cmd-enter-pill">
                <kbd>↵</kbd>
              </div>
            {/if}
          </button>
        {/each}
      </div>

      <div class="cmd-pop-footer">
        <div class="cmd-footer-left">
          <span class="footer-hint"><kbd>↑↓</kbd> navigate</span>
          <span class="footer-hint"><kbd>↵</kbd> run</span>
        </div>
        <span class="footer-hint"><kbd>Esc</kbd> close</span>
      </div>
    </div>
  {/if}

  {#if showFiles}
    <div class="composer-pop file-pop" role="listbox" aria-label="Matching project files">
      <div class="file-pop-header">
        <div class="file-pop-header-title">
          <Icon icon={FolderGit2} size={13} class="file-header-icon" />
          <span>Workspace Files</span>
        </div>
        {#if files !== null}
          <span class="file-count-pill">{files.length}</span>
        {/if}
      </div>

      <div class="file-pop-list" bind:this={fileListEl}>
        {#if files === null}
          <div class="pop-loading">
            <Icon icon={LoaderCircle} size={14} class="spin" />
            <span>Searching workspace…</span>
          </div>
        {:else if files.length === 0}
          <div class="pop-empty">
            <Icon icon={Search} size={14} />
            <span>No files found</span>
          </div>
        {:else}
          {#each files as f, i}
            {@const { dir, name, ext } = splitPath(f)}
            {@const { icon, colorClass } = getFileTypeInfo(ext)}
            {@const isSel = i === fileSel}
            <button
              role="option"
              aria-selected={isSel}
              class={`file-pop-item ${colorClass}${isSel ? " sel" : ""}`}
              onmouseenter={() => onHoverFile(i)}
              onclick={() => onSelectFile(f)}
            >
              <div class="file-icon-badge">
                <Icon {icon} size={13} strokeWidth={1.8} />
              </div>
              <div class="file-details">
                <div class="file-name-row">
                  <span class="file-name-bold">{name}</span>
                  {#if ext}
                    <span class="file-ext-chip">{ext}</span>
                  {/if}
                </div>
                {#if dir}
                  <div class="file-breadcrumb" title={dir}>
                    <Icon icon={Folder} size={10} class="crumb-folder-icon" />
                    <span>{dir}</span>
                  </div>
                {/if}
              </div>
              {#if isSel}
                <div class="file-insert-pill">
                  <kbd>↵</kbd>
                </div>
              {/if}
            </button>
          {/each}
        {/if}
      </div>

      <div class="file-pop-footer">
        <div class="file-footer-left">
          <span class="footer-hint"><kbd>↑↓</kbd> navigate</span>
          <span class="footer-hint"><kbd>↵</kbd> insert</span>
        </div>
        <span class="footer-hint"><kbd>Esc</kbd> close</span>
      </div>
    </div>
  {/if}
{/if}
