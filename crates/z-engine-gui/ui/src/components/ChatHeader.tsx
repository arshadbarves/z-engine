import { Command, GitCompare, Layers } from "lucide-react";

export function ChatHeader({
  title,
  titleHint,
  busy,
  diffOpen,
  onPalette,
  onCompact,
  onToggleDiff,
}: {
  title: string;
  titleHint?: string;
  busy: boolean;
  diffOpen: boolean;
  onPalette: () => void;
  onCompact: () => void;
  onToggleDiff: () => void;
}) {
  return (
    <header className="chat-head">
      <div className="chat-title" title={titleHint}>
        {title}
      </div>
      <div className="head-controls">
        <button className="icon-btn" title="Command palette (⌘K)" onClick={onPalette}>
          <Command size={12} />
        </button>
        <button
          className="icon-btn"
          title="/compact — force context compaction"
          disabled={busy}
          onClick={onCompact}
        >
          <Layers size={12} />
        </button>
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
