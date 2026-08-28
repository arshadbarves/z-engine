import type { SlashCommand } from "../lib/slash";

export function ComposerPopovers({
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
}: {
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
}) {
  if (!showSlash && !showFiles) return null;

  return (
    <>
      {showSlash && slashMatches && (
        <div className="composer-pop" role="listbox" aria-label="Slash commands">
          {slashMatches.map((c, i) => (
            <button
              key={c.name}
              role="option"
              aria-selected={i === slashSel}
              className={`pop-item${i === slashSel ? " sel" : ""}`}
              onMouseEnter={() => onHoverSlash(i)}
              onClick={() => onSelectSlash(c.name)}
            >
              <span className="pop-name">/{c.name}</span>
              <span className="pop-desc">{c.desc}</span>
            </button>
          ))}
        </div>
      )}

      {showFiles && (
        <div className="composer-pop" role="listbox" aria-label="Matching project files">
          {files === null && <div className="pop-note">searching…</div>}
          {files !== null && files.length === 0 && (
            <div className="pop-note">no matching files</div>
          )}
          {files?.map((f, i) => (
            <button
              key={f}
              role="option"
              aria-selected={i === fileSel}
              className={`pop-item mono${i === fileSel ? " sel" : ""}`}
              onMouseEnter={() => onHoverFile(i)}
              onClick={() => onSelectFile(f)}
            >
              <span className="pop-name">{f}</span>
            </button>
          ))}
        </div>
      )}
    </>
  );
}
