import { ArrowDown, GitCompare, PanelLeft, Search } from "lucide-react";
import { ContextMeter } from "./ContextMeter";

export function ChatHeader({
  title,
  titleHint,
  diffOpen,
  sidebarOpen,
  onToggleSidebar,
  onPalette,
  onToggleDiff,
}: {
  title: string;
  titleHint?: string;
  diffOpen: boolean;
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
  onPalette: () => void;
  onToggleDiff: () => void;
}) {
  return (
    <header className="chat-head">
      <div className="head-left">
        <button
          type="button"
          className="icon-btn"
          title={sidebarOpen ? "Hide sidebar (⌘B)" : "Show sidebar (⌘B)"}
          onClick={onToggleSidebar}
        >
          <PanelLeft size={13} />
        </button>
        <button type="button" className="icon-btn" title="Search (⌘K)" onClick={onPalette}>
          <Search size={13} />
        </button>
        <div className="chat-title" title={titleHint}>
          {title}
        </div>
      </div>
      <div className="head-controls">
        <ContextMeter />
        <button
          className={`icon-btn${diffOpen ? " active" : ""}`}
          title="Review uncommitted git changes vs HEAD"
          onClick={onToggleDiff}
        >
          <GitCompare size={12} />
        </button>
      </div>
    </header>
  );
}

export function JumpLatest({ onJump }: { onJump: () => void }) {
  return (
    <button type="button" className="jump-latest" title="Jump to latest" onClick={onJump}>
      <ArrowDown size={16} />
    </button>
  );
}
