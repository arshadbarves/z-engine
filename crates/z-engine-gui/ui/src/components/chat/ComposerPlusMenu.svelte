<script lang="ts">
  import { Menu, Tooltip } from "$lib/ui";
  import Icon, { Bookmark, FileText, ImageIcon, Plus, SquareTerminal, Terminal, Zap, type IconSvgElement } from "$lib/ui/icons";

  /**
   * Everything the composer can do beyond typing, one click away, with the
   * character that does it while typing, so the shortcuts are learned.
   */
  type Props = {
    showTerminal: boolean;
    onAttach: () => void;
    onInsert: (prefix: string) => void;
    onShowTerminal: () => void;
  };
  let { showTerminal, onAttach, onInsert, onShowTerminal }: Props = $props();

  const ITEMS: Array<{ label: string; hint: string; icon: IconSvgElement; prefix: string }> = [
    { label: "Mention a file or agent", hint: "@", icon: FileText, prefix: "@" },
    { label: "Run a command", hint: "/", icon: Zap, prefix: "/" },
    { label: "Save a note to memory", hint: "#", icon: Bookmark, prefix: "# " },
    { label: "Run a shell command yourself", hint: "!", icon: SquareTerminal, prefix: "!" },
  ];
</script>

<Menu.Root>
  <Tooltip text="Attach and more">
    <Menu.Trigger class="composer-icon-btn composer-plus" aria-label="Attach and more">
      <Icon icon={Plus} size={15} strokeWidth={2} />
    </Menu.Trigger>
  </Tooltip>
  <Menu.Portal>
    <Menu.Content class="menu composer-menu" side="top" align="start" sideOffset={8}>
      <Menu.Item class="menu-item" onSelect={onAttach}>
        <span class="menu-item-icon"><Icon icon={ImageIcon} size={14} /></span>
        <span class="menu-item-label">Attach images</span>
        <span class="menu-item-hint">paste or drop</span>
      </Menu.Item>
      {#each ITEMS as item (item.prefix)}
        <Menu.Item class="menu-item" onSelect={() => onInsert(item.prefix)}>
          <span class="menu-item-icon"><Icon icon={item.icon} size={14} /></span>
          <span class="menu-item-label">{item.label}</span>
          <kbd class="kbd menu-item-hint">{item.hint}</kbd>
        </Menu.Item>
      {/each}
      {#if showTerminal}
        <Menu.Separator class="menu-sep" />
        <Menu.Item class="menu-item" onSelect={onShowTerminal}>
          <span class="menu-item-icon"><Icon icon={Terminal} size={14} /></span>
          <span class="menu-item-label">Show the terminal</span>
        </Menu.Item>
      {/if}
    </Menu.Content>
  </Menu.Portal>
</Menu.Root>
