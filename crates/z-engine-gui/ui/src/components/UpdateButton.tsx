import { useEffect, useRef, useSyncExternalStore } from "react";
import {
  Download,
  ExternalLink,
  LoaderCircle,
  Sparkles,
  X,
  ArrowRight,
} from "lucide-react";
import { updateStore } from "../lib/updateStore";

function fmtBytes(bytes: number): string {
  if (bytes < 1024 * 1024) {
    return `${Math.round(bytes / 1024)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Redesigned psychology-centered update button and modal popover. */
export function UpdateButton() {
  const { info, installing, progress, popoverOpen } = useSyncExternalStore(
    updateStore.subscribe,
    () => updateStore.getSnapshot(),
  );
  const popoverRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!popoverOpen) return;
    function onPointerDown(e: MouseEvent) {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        updateStore.setPopoverOpen(false);
      }
    }
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") updateStore.setPopoverOpen(false);
    }
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [popoverOpen]);

  if (!info?.available) return null;

  const pct = progress?.percentage != null ? Math.round(progress.percentage) : null;
  const isDownloading = installing && progress?.phase === "downloading";
  const isInstalling =
    installing && (progress?.phase === "installing" || progress?.phase === "ready");

  const headerLabel = isInstalling
    ? "Installing…"
    : isDownloading
      ? pct != null
        ? `${pct}%`
        : "Downloading…"
      : `Update v${info.latest}`;

  return (
    <div className="update-btn-container" ref={popoverRef}>
      <button
        type="button"
        className={`update-header-pill${installing ? " installing" : ""}${
          popoverOpen ? " active" : ""
        }`}
        title={`Version v${info.latest} available`}
        aria-label={`Version v${info.latest} available`}
        onClick={() => updateStore.setPopoverOpen(!popoverOpen)}
      >
        {installing ? (
          <LoaderCircle size={12} className="spin update-spin-icon" />
        ) : (
          <Sparkles size={12} className="update-sparkle-icon" />
        )}
        <span className="update-header-text">{headerLabel}</span>
        <span className="update-header-dot" aria-hidden="true" />
      </button>

      {popoverOpen && (
        <div
          className="update-popover"
          role="dialog"
          aria-modal="false"
          aria-label="Software Update"
        >
          <div className="update-popover-head">
            <div className="update-head-title">
              <div className="update-icon-box">
                <Sparkles size={14} />
              </div>
              <div>
                <h4 className="update-main-title">Software Update</h4>
                <div className="update-version-row">
                  <span className="update-ver-curr">v{info.current}</span>
                  <ArrowRight size={10} className="update-ver-arr" />
                  <span className="update-ver-new">v{info.latest}</span>
                </div>
              </div>
            </div>
            <button
              type="button"
              className="update-close-btn"
              title="Close"
              onClick={() => updateStore.setPopoverOpen(false)}
            >
              <X size={13} />
            </button>
          </div>

          <div className="update-popover-body">
            {info.releaseNotes ? (
              <div className="update-notes-card">
                <div className="update-notes-label">What's New</div>
                <div className="update-notes-scroll">{info.releaseNotes}</div>
              </div>
            ) : (
              <p className="update-desc">
                A new version of Z Engine is ready to install with performance improvements
                and bug fixes.
              </p>
            )}

            {installing && (
              <div className="update-progress-deck">
                <div className="update-progress-top">
                  <span className="update-phase-text">
                    {isInstalling
                      ? "Applying update & restarting…"
                      : "Downloading update package…"}
                  </span>
                  <span className="update-pct-text">{pct != null ? `${pct}%` : ""}</span>
                </div>
                <div className="update-progress-track">
                  <div
                    className={`update-progress-bar${
                      pct == null ? " indeterminate" : ""
                    }`}
                    style={{ width: `${pct ?? 30}%` }}
                  />
                </div>
                {progress?.totalBytes != null && progress.totalBytes > 0 && (
                  <div className="update-bytes-text">
                    {fmtBytes(progress.downloadedBytes)} / {fmtBytes(progress.totalBytes)}
                  </div>
                )}
              </div>
            )}
          </div>

          <div className="update-popover-actions">
            <button
              type="button"
              className="update-action-primary"
              disabled={installing}
              onClick={() => void updateStore.install()}
            >
              {installing ? (
                <>
                  <LoaderCircle size={13} className="spin" />
                  <span>Installing…</span>
                </>
              ) : (
                <>
                  <Download size={13} />
                  <span>Update & Restart</span>
                </>
              )}
            </button>

            {info.url && (
              <button
                type="button"
                className="update-action-secondary"
                title="Download directly from GitHub Releases in browser"
                onClick={() => updateStore.openRelease()}
              >
                <ExternalLink size={12} />
                <span>GitHub Releases</span>
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
