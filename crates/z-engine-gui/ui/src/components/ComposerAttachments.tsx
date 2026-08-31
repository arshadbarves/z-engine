import { X } from "../lib/icons";

function fileName(p: string): string {
  const i = p.lastIndexOf("/");
  return i >= 0 ? p.slice(i + 1) : p;
}

function extLabel(p: string): string {
  const n = fileName(p);
  const d = n.lastIndexOf(".");
  return d > 0 ? n.slice(d + 1).toUpperCase() : "FILE";
}

function FileIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      width={16}
      height={16}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
      <path d="M14 2v6h6" />
      <path d="M9 13h6M9 17h4" />
    </svg>
  );
}

export function ComposerAttachments({
  attachments,
  images,
  onRemoveAttachment,
  onRemoveImage,
}: {
  attachments: string[];
  images: string[];
  onRemoveAttachment: (path: string) => void;
  onRemoveImage: (index: number) => void;
}) {
  if (attachments.length === 0 && images.length === 0) return null;

  return (
    <>
      {attachments.length > 0 && (
        <div className="attachments">
          {attachments.map((p) => (
            <span key={p} className="attachment">
              <button
                className="att-x"
                title={`Remove ${p}`}
                onClick={() => onRemoveAttachment(p)}
                type="button"
              >
                <X size={9} strokeWidth={2.4} />
              </button>
              <span className="att-icon">
                <FileIcon />
              </span>
              <span className="att-text">
                <span className="att-name">{fileName(p)}</span>
                <span className="att-ext">{extLabel(p)}</span>
              </span>
            </span>
          ))}
        </div>
      )}

      {images.length > 0 && (
        <div className="attachments img-chips">
          {images.map((url, i) => (
            <span key={i} className="attachment img-chip">
              <button
                className="att-x"
                title="Remove image"
                onClick={() => onRemoveImage(i)}
                type="button"
              >
                <X size={9} strokeWidth={2.4} />
              </button>
              <img src={url} alt={`paste ${i + 1}`} />
            </span>
          ))}
        </div>
      )}
    </>
  );
}
